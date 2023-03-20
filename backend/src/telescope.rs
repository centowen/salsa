use async_trait::async_trait;
use common::{
    Direction, ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo, TelescopeTarget,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

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
}

pub type TelescopeContainer = Arc<Mutex<dyn Telescope>>;
pub type TelescopeCollection = Arc<RwLock<HashMap<String, TelescopeContainer>>>;

pub fn create_telescope_collection() -> TelescopeCollection {
    Arc::new(RwLock::new(HashMap::new()))
}
