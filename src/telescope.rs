use crate::coords::Direction;
use crate::telescopes::{
    ReceiverConfiguration, ReceiverError, TelescopeDefinition, TelescopeError, TelescopeInfo,
    TelescopeTarget, TelescopeType,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use crate::database::{DataBase, DataBaseError, Storage};

pub const TELESCOPE_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

#[async_trait]
pub trait Telescope: Send + Sync {
    async fn get_direction(&self) -> Result<Direction, TelescopeError>;
    async fn get_target(&self) -> Result<TelescopeTarget, TelescopeError>;
    async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError>;
    async fn set_receiver_configuration(
        &mut self,
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError>;
    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError>;
    async fn update(&mut self, delta_time: Duration) -> Result<(), TelescopeError>;
    async fn restart(&mut self) -> Result<(), TelescopeError>;
}

#[derive(Clone)]
pub struct TelescopeContainer {
    pub telescope: Arc<Mutex<dyn Telescope>>,
}

pub type TelescopeCollection = Arc<RwLock<HashMap<String, TelescopeContainer>>>;

fn start_telescope_service(telescope: Arc<Mutex<dyn Telescope>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            {
                let mut telescope = telescope.clone().lock_owned().await;
                if let Err(error) = telescope.update(TELESCOPE_UPDATE_INTERVAL).await {
                    log::error!("Failed to update telescope: {}", error);
                }
            }
            tokio::time::sleep(TELESCOPE_UPDATE_INTERVAL).await;
        }
    })
}

fn create_telescope(telescope_definition: TelescopeDefinition) -> TelescopeContainer {
    log::info!("Creating telescope {}", telescope_definition.name);
    let telescope: Arc<Mutex<dyn Telescope>> = match telescope_definition.telescope_type {
        TelescopeType::Salsa { definition } => {
            Arc::new(Mutex::new(crate::salsa_telescope::create(
                telescope_definition.name.clone(),
                definition.controller_address.clone(),
                definition.receiver_address.clone(),
            )))
        }
        TelescopeType::Fake { .. } => Arc::new(Mutex::new(crate::fake_telescope::create(
            telescope_definition.name.clone(),
        ))),
    };

    // TODO: The join handle is dropped here (and the service isn't really able to be stopped
    // either). We should keep track of the telescope service and have a method to shut it down.
    let _service: Option<_> = if telescope_definition.enabled {
        Some(start_telescope_service(telescope.clone()))
    } else {
        None
    };

    TelescopeContainer { telescope }
}

pub async fn create_telescope_collection<T>(
    database: &DataBase<T>,
) -> Result<TelescopeCollection, DataBaseError>
where
    T: Storage,
{
    let telescope_definitions = database.get_data().await?.telescopes;

    let telescopes: HashMap<_, _> = telescope_definitions
        .into_iter()
        .map(|telescope_definition| {
            (
                telescope_definition.name.clone(),
                create_telescope(telescope_definition),
            )
        })
        .collect();

    Ok(Arc::new(RwLock::new(telescopes)))
}
