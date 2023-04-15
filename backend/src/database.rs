use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum DataBaseError {
    #[error("could not open database")]
    IoError {
        #[from]
        source: io::Error,
    },
    #[error("invalid database format")]
    DecodingError {
        #[from]
        source: serde_json::Error,
    },
}

#[async_trait]
pub trait Storage: Sized + Clone + Send + Sync {
    async fn read(&self, key: &str) -> Result<Option<Vec<u8>>, DataBaseError>;
    async fn write(&mut self, key: &str, data: &[u8]) -> Result<(), DataBaseError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryStorage {
    data: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn read(&self, key: &str) -> Result<Option<Vec<u8>>, DataBaseError> {
        Ok(self.data.get(key).cloned())
    }

    async fn write(&mut self, key: &str, data: &[u8]) -> Result<(), DataBaseError> {
        self.data.insert(key.to_string(), data.to_vec());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileStorage {
    directory_path: std::path::PathBuf,
}

impl FileStorage {
    fn get_path(&self, key: &str) -> std::path::PathBuf {
        self.directory_path.join(format!("{}.json", key))
    }
}

#[async_trait]
impl Storage for FileStorage {
    async fn read(
        &self,
        key: &str,
    ) -> Result<Option<Vec<u8>>, DataBaseError> {
        let mut file = fs::File::open(self.get_path(key)).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(Some(data))
    }

    async fn write(
        &mut self,
        key: &str,
         data: &[u8]
        ) -> Result<(), DataBaseError> {
        let mut file = fs::File::create(self.get_path(key)).await?;
        file.write_all(data).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DataBase<StorageType>
where
    StorageType: Storage,
{
    storage: Arc<RwLock<StorageType>>,
}

/// Create an in-memory database.
///
/// Changes to the data in the returned database are never written to any
/// file.
pub fn create_in_memory_database() -> DataBase<InMemoryStorage> {
    let store = InMemoryStorage { data: HashMap::new() };
    DataBase::<InMemoryStorage> {
        storage: Arc::new(RwLock::new(store)),
    }
}

/// Create a database with the contents of the directory at `directory`.
///
/// The database support multiple keys with each key having its own file.
pub async fn create_database_from_directory(
    directory: &str,
) -> Result<DataBase<FileStorage>, DataBaseError> {
    if !fs::try_exists(directory).await? {
        fs::create_dir(directory).await?;
    }

    let storage = FileStorage {
        directory_path: std::path::Path::new(directory).to_owned(),
    };

    Ok(DataBase::<FileStorage> {
        storage: Arc::new(RwLock::new(storage)),
    })
}

impl<StorageType> DataBase<StorageType>
where
    StorageType: Storage,
{
    /// Locks the database for reading and returns its contents.
    /// If no data is found, the default value is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::database::{DataBase, create_in_memory_database};
    ///
    /// let db = create_in_memory_database();
    /// db.update_data::<Vec<i32>>("numbers", |mut v| v.push(42)).await.unwrap();
    /// let data = db.get_data::<Vec<i32>("numbers").await.unwrap();
    /// assert_eq!(data, vec![42]);
    /// ```
    pub async fn get_data<T>(&self, key: &str) -> Result<T, DataBaseError>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let storage = self.storage.read().await;
        match storage.read(key).await? {
            Some(data) => {
                Ok(serde_json::from_slice(&data)?)
            }
            None => Ok(T::default()),
        }
    }

    /// Locks the database for writing and runs the supplied function on the
    /// data.
    ///
    /// The function has mutable access to the data. After it returns, any
    /// changes made to data are written to the database file (if any).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::database::{DataBase, create_in_memory_database};
    ///
    /// let db = create_in_memory_database();
    /// db.update_data::<Vec<i32>>("numbers", |mut v| v.push(42)).await.unwrap();
    /// let data = db.get_data::<Vec<i32>>("numbers").await.unwrap();
    /// assert_eq!(data, vec![42]);
    /// ```
    pub async fn update_data<T, F>(&self, key: &str, f: F) -> Result<(), DataBaseError>
    where
        T: DeserializeOwned + Serialize + Default,
        F: FnOnce(T) -> T,
    {
        let mut storage = self.storage.write().await;

        let value = match storage.read(key).await? {
            Some(data) => serde_json::from_slice(&data)?,
            None => T::default(),
        };

        let value = f(value);
        let data = serde_json::to_vec(&value)?;
        storage.write(key, &data).await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_data() {
        let db = create_in_memory_database();
        db.update_data("numbers", |mut numbers: Vec<i32>| {
            numbers.push(42);
            numbers
        })
        .await
        .expect("msshould be able to set db data");
        let data = db
            .get_data::<Vec<i32>>("numbers")
            .await
            .expect("should be able to get db data");
        assert_eq!(data, vec![42])
    }

    #[tokio::test]
    async fn test_update_data() {
        let db = create_in_memory_database();
        db.update_data::<Vec<i32>, _>("numbers", |mut v| {
            v.push(42);
            v
        })
        .await
        .expect("should be able to update db data");
        let data = db
            .get_data::<Vec<i32>>("numbers")
            .await
            .expect("should be able to get db data");
        assert_eq!(data, vec![42])
    }

    #[tokio::test]
    async fn test_update_multiple_times() {
        let db = create_in_memory_database();
        for i in 0..10 {
            db.update_data::<Vec<i32>, _>("numbers", |mut v| {
                v.push(i);
                v
            })
            .await
            .expect("should be able to update db data");
        }
        let data = db
            .get_data::<Vec<i32>>("numbers")
            .await
            .expect("should be able to get db data");
        assert_eq!(data, (0..10).collect::<Vec<i32>>())
    }
}
