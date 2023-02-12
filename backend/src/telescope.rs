use async_trait::async_trait;
use common::{Direction, Location, TelescopeInfo, TelescopeStatus, TelescopeTarget};
use std::f64::consts::PI;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct FakeTelescope {
    pub target: TelescopeTarget,
    pub horizontal: Direction,
    pub last_update: Instant,
    pub location: Location,
}

const PARKING_HORIZONTAL: Direction = Direction {
    azimuth: 0.0,
    altitude: PI / 2.0,
};

type FakeTelescopeControl = Arc<Mutex<FakeTelescope>>;

pub fn create_telescope_control() -> FakeTelescopeControl {
    Arc::new(Mutex::new(FakeTelescope {
        target: TelescopeTarget::Parked,
        horizontal: PARKING_HORIZONTAL,
        last_update: Instant::now(),
        location: Location {
            longitude: astro::angle::deg_frm_dms(-11, 55, 4.0).to_radians(),
            latitude: astro::angle::deg_frm_dms(57, 23, 35.0).to_radians(),
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
    async fn get_info(&self, _id: &str) -> Result<TelescopeInfo, TelescopeError>;
}

#[async_trait]
impl TelescopeControl for FakeTelescopeControl {
    async fn get_direction(&self, _id: &str) -> Result<Direction, TelescopeError> {
        let telescope = self.lock().await;
        Ok(telescope.horizontal)
    }

    async fn get_target(&self, _id: &str) -> Result<TelescopeTarget, TelescopeError> {
        let telescope = self.lock().await;
        Ok(telescope.target)
    }

    async fn set_target(&self, id: &str, target: TelescopeTarget) -> Result<(), TelescopeError> {
        let mut telescope = self.clone().lock_owned().await;
        log::info!("Setting target for telescope {} to {:?}", id, &target);
        telescope.target = target;
        telescope.last_update = Instant::now();
        Ok(())
    }

    async fn get_info(&self, _id: &str) -> Result<TelescopeInfo, TelescopeError> {
        let (location, target, current_horizontal) = {
            let telescope = self.lock().await;
            (telescope.location, telescope.target, telescope.horizontal)
        };

        let commanded_horizontal = match target {
            TelescopeTarget::Equatorial { ra, dec } => {
                common::coords::get_horizontal_eq(location, ra, dec)
            }
            TelescopeTarget::Galactic { l, b } => {
                common::coords::get_horizontal_gal(location, l, b)
            }
            TelescopeTarget::Stopped => current_horizontal,
            TelescopeTarget::Parked => PARKING_HORIZONTAL,
        };

        let status = {
            let telescope = self.lock().await;
            match telescope.target {
                TelescopeTarget::Parked | TelescopeTarget::Stopped => TelescopeStatus::Idle,
                _ => {
                    if Instant::now() - telescope.last_update < Duration::from_secs(10) {
                        TelescopeStatus::Slewing
                    } else {
                        TelescopeStatus::Tracking
                    }
                }
            }
        };
        Ok(TelescopeInfo {
            status,
            current_horizontal,
            commanded_horizontal,
        })
    }
}
