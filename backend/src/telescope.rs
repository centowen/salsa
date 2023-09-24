use async_trait::async_trait;
use common::{
    Direction, Location, TelescopeError, TelescopeInfo, TelescopeStatus, TelescopeTarget,
};
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct FakeTelescope {
    pub target: TelescopeTarget,
    pub horizontal: Direction,
    pub location: Location,
    pub most_recent_error: Option<TelescopeError>,
}

const PARKING_HORIZONTAL: Direction = Direction {
    azimuth: 0.0,
    altitude: PI / 2.0,
};
pub const TELESCOPE_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
pub const FAKE_TELESCOPE_SLEWING_SPEED: f64 = PI / 10.0;
pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0 / 180. * PI;

type FakeTelescopeControl = Arc<Mutex<FakeTelescope>>;

pub fn create_telescope() -> FakeTelescopeControl {
    Arc::new(Mutex::new(FakeTelescope {
        target: TelescopeTarget::Parked,
        horizontal: PARKING_HORIZONTAL,
        location: Location {
            longitude: astro::angle::deg_frm_dms(-11, 55, 4.0).to_radians(),
            latitude: astro::angle::deg_frm_dms(57, 23, 35.0).to_radians(),
        },
        most_recent_error: None,
    }))
}

pub fn calculate_target_horizontal(
    location: Location,
    target: TelescopeTarget,
    current_horizontal: Direction,
) -> Direction {
    match target {
        TelescopeTarget::Equatorial { ra, dec } => {
            common::coords::get_horizontal_eq(location, ra, dec)
        }
        TelescopeTarget::Galactic { l, b } => common::coords::get_horizontal_gal(location, l, b),
        TelescopeTarget::Stopped => current_horizontal,
        TelescopeTarget::Parked => PARKING_HORIZONTAL,
    }
}

#[async_trait]
pub trait TelescopeControl: Send + Sync {
    async fn get_direction(&self, id: &str) -> Result<Direction, TelescopeError>;
    async fn get_target(&self, id: &str) -> Result<TelescopeTarget, TelescopeError>;
    async fn set_target(
        &self,
        id: &str,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError>;
    async fn get_info(&self, _id: &str) -> Result<TelescopeInfo, TelescopeError>;
    async fn update(&self, delta_time: Duration) -> Result<(), TelescopeError>;
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

    async fn set_target(
        &self,
        id: &str,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        let mut telescope = self.clone().lock_owned().await;

        telescope.most_recent_error = None;

        let target_horizontal =
            calculate_target_horizontal(telescope.location, target, telescope.horizontal);
        if target_horizontal.altitude < LOWEST_ALLOWED_ALTITUDE {
            log::info!(
                "Refusing to set target for telescope {} to {:?}. Target is below horizon",
                id,
                &target
            );
            telescope.target = TelescopeTarget::Stopped;
            Err(TelescopeError::TargetBelowHorizon)
        } else {
            log::info!("Setting target for telescope {} to {:?}", id, &target);
            telescope.target = target;
            Ok(target)
        }
    }

    async fn get_info(&self, _id: &str) -> Result<TelescopeInfo, TelescopeError> {
        let (location, target, current_horizontal, most_recent_error) = {
            let telescope = self.lock().await;
            (
                telescope.location,
                telescope.target,
                telescope.horizontal,
                telescope.most_recent_error.clone(),
            )
        };

        let target_horizontal = calculate_target_horizontal(location, target, current_horizontal);

        let horizontal_offset_squared = (target_horizontal.azimuth - current_horizontal.azimuth)
            .powi(2)
            + (target_horizontal.altitude - current_horizontal.altitude).powi(2);
        let status = {
            if target == TelescopeTarget::Stopped {
                TelescopeStatus::Idle
            } else if horizontal_offset_squared > 0.2f64.to_radians().powi(2) {
                TelescopeStatus::Slewing
            } else if target == TelescopeTarget::Parked {
                TelescopeStatus::Idle
            } else {
                TelescopeStatus::Tracking
            }
        };

        Ok(TelescopeInfo {
            status,
            current_horizontal,
            commanded_horizontal: target_horizontal,
            current_target: target,
            most_recent_error,
        })
    }

    async fn update(&self, delta_time: Duration) -> Result<(), TelescopeError> {
        let mut telescope = self.lock().await;
        let current_horizontal = telescope.horizontal;
        let target_horizontal =
            calculate_target_horizontal(telescope.location, telescope.target, current_horizontal);

        if target_horizontal.altitude < LOWEST_ALLOWED_ALTITUDE {
            telescope.target = TelescopeTarget::Stopped;
            log::info!(
                "Stopping telescope since target {:?} set below horizon.",
                &telescope.target
            );
            telescope.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
        } else {
            let max_delta_angle = FAKE_TELESCOPE_SLEWING_SPEED * delta_time.as_secs_f64();
            telescope.horizontal.azimuth += (target_horizontal.azimuth
                - current_horizontal.azimuth)
                .clamp(-max_delta_angle, max_delta_angle);
            telescope.horizontal.altitude += (target_horizontal.altitude
                - current_horizontal.altitude)
                .clamp(-max_delta_angle, max_delta_angle);
        }

        Ok(())
    }
}
