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
    type StorageConfiguration: Clone + Send + Sync;

    async fn create(
        configuration: &Self::StorageConfiguration,
        key: &str,
        data: &[u8],
    ) -> Result<Self, DataBaseError>;
    async fn read(&self) -> Result<Vec<u8>, DataBaseError>;
    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryStorage {
    data: Vec<u8>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    type StorageConfiguration = ();

    async fn create(
        _configuration: &Self::StorageConfiguration,
        _key: &str,
        data: &[u8],
    ) -> Result<Self, DataBaseError> {
        Ok(Self { data: data.into() })
    }

    async fn read(&self) -> Result<Vec<u8>, DataBaseError> {
        Ok(self.data.clone())
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError> {
        self.data = data.into();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileStorage {
    path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct FileStorageConfiguration {
    path: std::path::PathBuf,
}

#[async_trait]
impl Storage for FileStorage {
    type StorageConfiguration = FileStorageConfiguration;

    async fn create(
        configuration: &Self::StorageConfiguration,
        key: &str,
        data: &[u8],
    ) -> Result<Self, DataBaseError> {
        let path = configuration.path.join(format!("{}.json", key));
        let mut file = fs::File::create(&path).await?;
        file.write_all(&data).await?;
        Ok(Self { path })
    }

    async fn read(&self) -> Result<Vec<u8>, DataBaseError> {
        let mut file = fs::File::open(&self.path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(data)
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError> {
        let mut file = fs::File::create(&self.path).await?;
        file.write_all(data).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DataBase<StorageType>
where
    StorageType: Storage,
{
    storage: Arc<RwLock<HashMap<&'static str, StorageType>>>,
    storage_configuration: StorageType::StorageConfiguration,
}

/// Create an in-memory database.
///
/// Changes to the data in the returned database are never written to any
/// file.
pub fn create_in_memory_database() -> DataBase<InMemoryStorage> {
    DataBase::<InMemoryStorage> {
        storage: Arc::new(RwLock::new(HashMap::new())),
        storage_configuration: (),
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

    Ok(DataBase::<FileStorage> {
        storage: Arc::new(RwLock::new(HashMap::new())),
        storage_configuration: FileStorageConfiguration {
            path: std::path::Path::new(directory).to_owned(),
        },
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
    pub async fn get_data<T>(&self, key: &'static str) -> Result<T, DataBaseError>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let storages = self.storage.read().await;
        match storages.get(key) {
            Some(storage) => {
                let data = storage.read().await?;
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
    pub async fn update_data<T, F>(&self, key: &'static str, f: F) -> Result<(), DataBaseError>
    where
        T: DeserializeOwned + Serialize + Default,
        F: FnOnce(T) -> T,
    {
        let mut storages = self.storage.write().await;

        match storages.get_mut(key) {
            Some(storage) => {
                let data = storage.read().await?;
                let value = serde_json::from_slice(&data)?;
                let value = f(value);
                let data = serde_json::to_vec(&value)?;
                storage.write(&data).await?;
            }
            None => {
                let value = T::default();
                let value = f(value);
                let data = serde_json::to_vec(&value)?;

                storages.insert(
                    key,
                    StorageType::create(&self.storage_configuration, key, &data).await?,
                );
            }
        };

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
}
