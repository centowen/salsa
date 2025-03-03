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
pub trait Telescope: Send {
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

// Hide all synchronization for accessing a telescope inside this type only
// exposing an async api. This is more ergonomic and makes it impossible to
// create deadlocks in client code.
#[derive(Clone)]
pub struct TelescopeHandle {
    telescope: Arc<Mutex<dyn Telescope>>,
}

// FIXME: Maybe this can be implemented by a macro based on the Telescope trait?
// It's pure boilerplate.
impl TelescopeHandle {
    pub async fn get_direction(&self) -> Result<Direction, TelescopeError> {
        let guard = self.telescope.lock().await;
        guard.get_direction().await
    }
    pub async fn get_target(&self) -> Result<TelescopeTarget, TelescopeError> {
        let guard = self.telescope.lock().await;
        guard.get_target().await
    }
    pub async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        let mut guard = self.telescope.lock().await;
        guard.set_target(target).await
    }
    pub async fn set_receiver_configuration(
        &mut self,
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        let mut guard = self.telescope.lock().await;
        guard
            .set_receiver_configuration(receiver_configuration)
            .await
    }
    pub async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        let guard = self.telescope.lock().await;
        guard.get_info().await
    }
    #[allow(dead_code)] // TODO: Remove when used.
    pub async fn update(&mut self, delta_time: Duration) -> Result<(), TelescopeError> {
        let mut guard = self.telescope.lock().await;
        guard.update(delta_time).await
    }
    pub async fn restart(&mut self) -> Result<(), TelescopeError> {
        let mut guard = self.telescope.lock().await;
        guard.restart().await
    }
}

type TelescopeCollection = Arc<RwLock<HashMap<String, TelescopeHandle>>>;

// Hide all synchronization for handling telescopes inside this type. Exposes an
// async api without any client-visible locks for managing the collection of
// telescopes.
#[derive(Clone)]
pub struct TelescopeCollectionHandle {
    telescopes: TelescopeCollection,
}

impl TelescopeCollectionHandle {
    pub async fn get(&self, id: &str) -> Option<TelescopeHandle> {
        let telescopes_read_lock = self.telescopes.read().await;
        telescopes_read_lock.get(id).cloned()
    }

    pub async fn all(&self) -> Vec<(String, TelescopeHandle)> {
        let telescopes_read_lock = self.telescopes.read().await;
        telescopes_read_lock
            .iter()
            .map(|(name, t)| (name.clone(), t.clone()))
            .collect()
    }
}

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

fn create_telescope(telescope_definition: TelescopeDefinition) -> TelescopeHandle {
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

    TelescopeHandle { telescope }
}

pub async fn create_telescope_collection<T>(
    database: &DataBase<T>,
) -> Result<TelescopeCollectionHandle, DataBaseError>
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

    Ok(TelescopeCollectionHandle {
        telescopes: Arc::new(RwLock::new(telescopes)),
    })
}
