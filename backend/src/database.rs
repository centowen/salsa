use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io;
use std::io::Write;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::{RwLock, RwLockReadGuard};

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

#[derive(Debug, Clone)]
pub struct DataBase<T> {
    cache: Arc<RwLock<T>>,
    filename: Option<String>,
}

impl<T> DataBase<T>
where
    T: DeserializeOwned + Serialize + Default,
{
    /// Create an in-memory database.
    ///
    /// Changes to the data in the returned database are never written to any
    /// file.
    pub fn from_data(data: T) -> Self {
        Self {
            cache: Arc::new(RwLock::new(data)),
            filename: None,
        }
    }

    /// Create a database with the contents of the file at `filename`.
    ///
    /// If the file doesn't exist an attempt is made to create it with the
    /// `T::default()` as contents.
    ///
    /// Any changes to the data in the database (using e.g.
    /// [`DataBase::set_data`]) are automatically written to the file.
    ///
    /// # Errors
    ///
    /// This function can return an `Err` for a number of reasons.
    ///
    /// - if the file exists but can not be read for any reason,
    /// - if the contents of the file can not be deserialized into type `T`, or
    ///   if `T::default()` can not be serialized,
    /// - if the file doesn't exist and can't be created.
    pub async fn from_file(filename: &str) -> Result<Self, DataBaseError> {
        let data = match fs::read_to_string(filename).await {
            Ok(serialized_data) => serde_json::from_str::<T>(&serialized_data)?,
            Err(error) => {
                if matches!(error.kind(), io::ErrorKind::NotFound) {
                    let data = T::default();
                    let mut file = std::fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(filename)?;
                    file.write(serde_json::to_string_pretty(&data)?.as_bytes())?;
                    data
                } else {
                    Err(error)?
                }
            }
        };
        Ok(Self {
            cache: Arc::new(RwLock::new(data)),
            filename: Some(filename.to_string()),
        })
    }

    /// Locks the database for reading and returns its contents.
    ///
    /// The returned value is an guard implementing [`Deref`](std::ops::Deref)
    /// into the contained data. The database is locked for reading while the
    /// returned guard is alive.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::database::DataBase;
    ///
    /// let db = DataBase::<Vec<i32>>::from_data(vec![42]);
    /// {
    ///     let data = db.get_data().await;
    ///     assert_eq!(*data, vec![42])
    /// } // <-- read lock released here
    /// ```
    pub async fn get_data(&self) -> RwLockReadGuard<T> {
        self.cache.read().await
    }

    /// Locks the database for writing and sets its contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::database::DataBase;
    ///
    /// let db = DataBase::<Vec<i32>>::from_data(vec![]);
    /// db.set_data(vec![42]).await.unwrap();
    /// ```
    pub async fn set_data(&mut self, data: T) -> Result<(), DataBaseError> {
        let mut cache = self.cache.write().await;
        if let Some(filename) = &self.filename {
            fs::write(&filename, serde_json::to_string_pretty(&data)?).await?;
        }
        *cache = data;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::DataBase;

    #[tokio::test]
    async fn test_get_data() {
        let db = DataBase::<Vec<i32>>::from_data(vec![42]);
        let data = db.get_data().await;
        assert_eq!(*data, vec![42])
    }

    #[tokio::test]
    async fn test_set_data() {
        let mut db = DataBase::<Vec<i32>>::from_data(vec![]);
        db.set_data(vec![42])
            .await
            .expect("should be able to set db data.");
        let data = db.get_data().await;
        assert_eq!(*data, vec![42])
    }
}
