use async_trait::async_trait;
use common::{Direction, TelescopeTarget};
use std::f32::consts::PI;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct FakeTelescope {
    pub target: TelescopeTarget,
    pub direction: Direction,
}

type FakeTelescopeControl = Arc<Mutex<FakeTelescope>>;

pub fn create_telescope_control() -> FakeTelescopeControl {
    Arc::new(Mutex::new(FakeTelescope {
        target: TelescopeTarget::Parked,
        direction: Direction {
            azimuth: 0.0,
            elevation: PI / 2.0,
        },
    }))
}

#[derive(Debug)]
pub struct TelescopeError {
    telescope_id: String,
}

impl Display for TelescopeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "Failed to perform action on telescope {}",
            self.telescope_id
        ))
    }
}

#[async_trait]
pub trait TelescopeControl: Send + Sync {
    async fn get_direction(&self, id: &str) -> Result<Direction, TelescopeError>;
    async fn get_target(&self, id: &str) -> Result<TelescopeTarget, TelescopeError>;
    async fn set_target(&self, id: &str, target: TelescopeTarget) -> Result<(), TelescopeError>;
}

#[async_trait]
impl TelescopeControl for FakeTelescopeControl {
    async fn get_direction(&self, _id: &str) -> Result<Direction, TelescopeError> {
        let telescope = self.lock().await;
        Ok(telescope.direction)
    }

    async fn get_target(&self, _id: &str) -> Result<TelescopeTarget, TelescopeError> {
        let telescope = self.lock().await;
        Ok(telescope.target)
    }

    async fn set_target(&self, id: &str, target: TelescopeTarget) -> Result<(), TelescopeError> {
        let mut telescope = self.clone().lock_owned().await;
        log::info!("Setting target for telescope {} to {:?}", id, &target);
        telescope.target = target;
        Ok(())
    }
}
