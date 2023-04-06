use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io;
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
    T: DeserializeOwned + Serialize,
{
    pub fn from_data(data: T) -> Self {
        Self {
            cache: Arc::new(RwLock::new(data)),
            filename: None,
        }
    }

    pub async fn from_file(filename: &str) -> Result<Self, DataBaseError> {
        // TODO: Create file if it doesn't exist
        // (tokio::fs::OpenOptions.create_new()) or fall back to in-memory.
        let serialized_data = fs::read_to_string(filename).await?;
        let data = serde_json::from_str::<T>(&serialized_data)?;
        Ok(Self {
            cache: Arc::new(RwLock::new(data)),
            filename: Some(filename.to_string()),
        })
    }

    pub async fn get_data(&self) -> RwLockReadGuard<T> {
        self.cache.read().await
    }

    pub async fn set_data(&mut self, data: T) -> Result<(), DataBaseError> {
        let mut cache = self.cache.write().await;
        if let Some(filename) = &self.filename {
            fs::write(&filename, serde_json::to_string_pretty(&data)?).await?;
        }
        *cache = data;
        Ok(())
    }
}
