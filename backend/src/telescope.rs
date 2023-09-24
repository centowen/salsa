use async_trait::async_trait;
use common::{
    Direction, Location, ObservedSpectra, ReceiverConfiguration, ReceiverError, TelescopeError,
    TelescopeInfo, TelescopeStatus, TelescopeTarget,
};
use rand::Rng;
use rand_distr::StandardNormal;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct FakeTelescope {
    pub target: TelescopeTarget,
    pub horizontal: Direction,
    pub location: Location,
    pub most_recent_error: Option<TelescopeError>,
    pub receiver_configuration: ReceiverConfiguration,
    pub current_spectra: Vec<ObservedSpectra>,
}

const PARKING_HORIZONTAL: Direction = Direction {
    azimuth: 0.0,
    altitude: PI / 2.0,
};
pub const TELESCOPE_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
pub const FAKE_TELESCOPE_SLEWING_SPEED: f64 = PI / 10.0;
pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0 / 180. * PI;
pub const FAKE_TELESCOPE_CHANNELS: usize = 400;
pub const FAKE_TELESCOPE_CHANNEL_WIDTH: f32 = 2e6f32 / FAKE_TELESCOPE_CHANNELS as f32;
pub const FAKE_TELESCOPE_FIRST_CHANNEL: f32 =
    1.420e9f32 - FAKE_TELESCOPE_CHANNEL_WIDTH * FAKE_TELESCOPE_CHANNELS as f32 / 2f32;
pub const FAKE_TELESCOPE_NOISE: f32 = 2f32;

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
        receiver_configuration: ReceiverConfiguration { integrate: false },
        current_spectra: vec![],
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

pub fn create_fake_spectra(integration_time: Duration) -> ObservedSpectra {
    let mut rng = rand::thread_rng();

    let frequencies: Vec<f32> = (0..FAKE_TELESCOPE_CHANNELS)
        .map(|channel| channel as f32 * FAKE_TELESCOPE_CHANNEL_WIDTH + FAKE_TELESCOPE_FIRST_CHANNEL)
        .collect();
    let spectra: Vec<f32> = vec![5f32; FAKE_TELESCOPE_CHANNELS]
        .into_iter()
        .map(|value| {
            value + FAKE_TELESCOPE_NOISE * rng.sample::<f32, StandardNormal>(StandardNormal)
        })
        .collect();

    ObservedSpectra {
        frequencies,
        spectra,
        observation_time: integration_time,
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
    async fn set_receiver_configuration(
        &self,
        id: &str,
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError>;
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
        telescope.receiver_configuration.integrate = false;
        telescope.current_spectra.clear();

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

    async fn set_receiver_configuration(
        &self,
        _id: &str,
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        let mut telescope = self.clone().lock_owned().await;
        if receiver_configuration.integrate && !telescope.receiver_configuration.integrate {
            log::info!("Starting integration");
            telescope.receiver_configuration.integrate = true;
        } else if !receiver_configuration.integrate && telescope.receiver_configuration.integrate {
            log::info!("Stopping integration");
            telescope.receiver_configuration.integrate = false;
        }
        Ok(telescope.receiver_configuration)
    }

    async fn get_info(&self, _id: &str) -> Result<TelescopeInfo, TelescopeError> {
        let (
            location,
            target,
            current_horizontal,
            most_recent_error,
            receiver_configuration,
            current_spectra,
        ) = {
            let telescope = self.lock().await;
            (
                telescope.location,
                telescope.target,
                telescope.horizontal,
                telescope.most_recent_error.clone(),
                telescope.receiver_configuration,
                telescope.current_spectra.clone(),
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

        let latest_observation = if current_spectra.is_empty() {
            None
        } else {
            let mut latest_observation = ObservedSpectra {
                frequencies: vec![0f32; FAKE_TELESCOPE_CHANNELS],
                spectra: vec![0f32; FAKE_TELESCOPE_CHANNELS],
                observation_time: Duration::from_secs(0),
            };
            for integration in &current_spectra {
                latest_observation.spectra = latest_observation
                    .spectra
                    .into_iter()
                    .zip(integration.spectra.iter())
                    .map(|(a, b)| a + b)
                    .collect();
                latest_observation.observation_time += integration.observation_time;
            }
            latest_observation.frequencies = current_spectra[0].frequencies.clone();
            latest_observation.spectra = latest_observation
                .spectra
                .into_iter()
                .map(|value| value / current_spectra.len() as f32)
                .collect();
            Some(latest_observation)
        };

        Ok(TelescopeInfo {
            status,
            current_horizontal,
            commanded_horizontal: target_horizontal,
            current_target: target,
            most_recent_error,
            measurement_in_progress: receiver_configuration.integrate,
            latest_observation,
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

        if telescope.receiver_configuration.integrate {
            telescope
                .current_spectra
                .push(create_fake_spectra(delta_time))
        }

        Ok(())
    }
}
