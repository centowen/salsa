use async_trait::async_trait;
use deadpool_sqlite::{CreatePoolError, PoolError};
use log::debug;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
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
    async fn read(&self) -> Result<Option<Vec<u8>>, DataBaseError>;
    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryStorage {
    data: Vec<u8>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn read(&self) -> Result<Option<Vec<u8>>, DataBaseError> {
        if self.data.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.data.clone()))
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError> {
        self.data = data.to_vec();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileStorage {
    file_path: std::path::PathBuf,
}

#[async_trait]
impl Storage for FileStorage {
    async fn read(&self) -> Result<Option<Vec<u8>>, DataBaseError> {
        let mut file = fs::File::open(&self.file_path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(Some(data))
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), DataBaseError> {
        let mut file = fs::File::create(&self.file_path).await?;
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

#[derive(Debug, Error)]
pub enum SqliteDatabaseError {
    #[error("Could not open database: {source}")]
    RusqliteError {
        #[from]
        source: rusqlite::Error,
    },
    #[error("Could not setup database connection pool: {source}")]
    CreatePoolError {
        #[from]
        source: CreatePoolError,
    },
    #[error("Could not setup database connection pool: {source}")]
    PoolError {
        #[from]
        source: PoolError,
    },
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

pub fn apply_migrations(connection: &mut Connection) -> Result<(), SqliteDatabaseError> {
    let report = embedded::migrations::runner().run(connection).unwrap();
    debug!("Applied migrations\n{:?}", report);
    Ok(())
}

pub fn create_sqlite_database_on_disk(
    file_path: impl Into<PathBuf>,
) -> Result<Connection, SqliteDatabaseError> {
    let file_path = file_path.into();
    let mut connection = Connection::open(&file_path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

/// Create an in-memory database.
///
/// Changes to the data in the returned database are never written to any
/// file.
#[allow(dead_code)]
pub fn create_in_memory_database() -> DataBase<InMemoryStorage> {
    let store = InMemoryStorage { data: Vec::new() };
    DataBase::<InMemoryStorage> {
        storage: Arc::new(RwLock::new(store)),
    }
}

/// Create a database with the contents of the directory at `directory`.
///
/// The database support multiple keys with each key having its own file.
pub async fn create_database_from_directory(
    file_path: &str,
) -> Result<DataBase<FileStorage>, DataBaseError> {
    let file_path = std::path::Path::new(file_path).to_owned();
    let storage = FileStorage { file_path };

    Ok(DataBase::<FileStorage> {
        storage: Arc::new(RwLock::new(storage)),
    })
}

use crate::bookings::Booking;
use crate::telescopes::TelescopeDefinition;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DataModel {
    pub bookings: Vec<Booking>,
    pub telescopes: Vec<TelescopeDefinition>,
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
    pub async fn get_data(&self) -> Result<DataModel, DataBaseError> {
        let storage = self.storage.read().await;
        match storage.read().await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(DataModel::default()),
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
    /// ## let booking = Booking { id: 42, ..Default::default()}
    /// let db = create_in_memory_database();
    /// db.update_data(|mut datamodel| datamodel.bookings.push(booking)).await.unwrap();
    /// let data = db.get_data().await.unwrap();
    /// assert_eq!(data, DataModel{bookings: vec![booking], ..Default::default()});
    /// ```
    pub async fn update_data<F>(&self, f: F) -> Result<(), DataBaseError>
    where
        F: FnOnce(DataModel) -> DataModel,
    {
        let mut storage_handle = self.storage.write().await;

        let value = match storage_handle.read().await? {
            Some(data) => serde_json::from_slice(&data)?,
            None => DataModel::default(),
        };

        let value = f(value);
        let data = serde_json::to_vec(&value)?;
        storage_handle.write(&data).await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use chrono::{Duration, Utc};

    use super::*;

    #[tokio::test]
    async fn given_no_previous_write_then_get_data_returns_default() {
        let db = create_in_memory_database();
        let data = db.get_data().await.expect("should be able to get db data");
        assert_eq!(DataModel::default(), data);
    }

    #[tokio::test]
    async fn test_get_data() {
        let booking = Booking {
            start_time: Utc::now(),
            end_time: Utc::now() + Duration::hours(1),
            telescope_name: "test".to_string(),
            user_name: "test".to_string(),
        };
        let db = create_in_memory_database();
        db.update_data(|mut data_model| {
            data_model.bookings.push(booking.clone());
            data_model
        })
        .await
        .expect("should be able to set db data");
        let data = db.get_data().await.expect("should be able to get db data");
        assert_eq!(data.bookings, vec![booking]);
    }

    #[tokio::test]
    async fn test_update_data() {
        let booking1 = Booking {
            start_time: Utc::now(),
            end_time: Utc::now() + Duration::hours(1),
            telescope_name: "test1".to_string(),
            user_name: "test".to_string(),
        };
        let booking2 = Booking {
            start_time: Utc::now(),
            end_time: Utc::now() + Duration::hours(1),
            telescope_name: "test2".to_string(),
            user_name: "test".to_string(),
        };
        let db = create_in_memory_database();
        db.update_data(|mut data_model| {
            data_model.bookings.push(booking1.clone());
            data_model
        })
        .await
        .expect("should be able to set db data");
        db.update_data(|mut data_model| {
            data_model.bookings.push(booking2.clone());
            data_model
        })
        .await
        .expect("should be able to set db data");
        let data = db.get_data().await.expect("should be able to get db data");
        assert_eq!(data.bookings, vec![booking1, booking2]);
    }
}
