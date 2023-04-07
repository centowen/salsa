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
    pub fn from_data(data: T) -> Self {
        Self {
            cache: Arc::new(RwLock::new(data)),
            filename: None,
        }
    }

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
