use crate::telescope::Telescope;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{
    Direction, Location, ObservedSpectra, ReceiverConfiguration, ReceiverError, TelescopeError,
    TelescopeInfo, TelescopeStatus, TelescopeTarget, Measurement
};
use rand::Rng;
use rand_distr::StandardNormal;
use std::f64::consts::PI;
use std::time::Duration;

const FAKE_TELESCOPE_PARKING_HORIZONTAL: Direction = Direction {
    azimuth: 0.0,
    altitude: PI / 2.0,
};
pub const FAKE_TELESCOPE_SLEWING_SPEED: f64 = PI / 10.0;
pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0 / 180. * PI;
pub const FAKE_TELESCOPE_CHANNELS: usize = 400;
pub const FAKE_TELESCOPE_CHANNEL_WIDTH: f32 = 2e6f32 / FAKE_TELESCOPE_CHANNELS as f32;
pub const FAKE_TELESCOPE_FIRST_CHANNEL: f32 =
    1.420e9f32 - FAKE_TELESCOPE_CHANNEL_WIDTH * FAKE_TELESCOPE_CHANNELS as f32 / 2f32;
pub const FAKE_TELESCOPE_NOISE: f32 = 2f32;

pub struct FakeTelescope {
    pub target: TelescopeTarget,
    pub horizontal: Direction,
    pub location: Location,
    pub most_recent_error: Option<TelescopeError>,
    pub receiver_configuration: ReceiverConfiguration,
    pub current_spectra: Vec<ObservedSpectra>,
    pub name: String,
}

pub fn create(name: String) -> FakeTelescope {
    FakeTelescope {
        target: TelescopeTarget::Parked,
        horizontal: FAKE_TELESCOPE_PARKING_HORIZONTAL,
        location: Location {
            longitude: 0.20802143022, //(11.0+55.0/60.0+7.5/3600.0) * PI / 180.0. Sign positive, handled in gmst calc
            latitude: 1.00170457462,  //(57.0+23.0/60.0+36.4/3600.0) * PI / 180.0
        },
        most_recent_error: None,
        receiver_configuration: ReceiverConfiguration { integrate: false },
        current_spectra: vec![],
        name,
    }
}

#[async_trait]
impl Telescope for FakeTelescope {
    async fn measure(&self, measurement: &mut Measurement) -> Result<(), ReceiverError> {
        todo!();
    }
    async fn get_direction(&self) -> Result<Direction, TelescopeError> {
        Ok(self.horizontal)
    }

    async fn get_target(&self) -> Result<TelescopeTarget, TelescopeError> {
        Ok(self.target)
    }

    async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        self.most_recent_error = None;
        self.receiver_configuration.integrate = false;
        self.current_spectra.clear();

        let target_horizontal =
            calculate_target_horizontal(self.location, Utc::now(), target, self.horizontal);
        if target_horizontal.altitude < LOWEST_ALLOWED_ALTITUDE {
            log::info!(
                "Refusing to set target for telescope {} to {:?}. Target is below horizon",
                &self.name,
                &target
            );
            self.target = TelescopeTarget::Stopped;
            Err(TelescopeError::TargetBelowHorizon)
        } else {
            log::info!(
                "Setting target for telescope {} to {:?}",
                &self.name,
                &target
            );
            self.target = target;
            Ok(target)
        }
    }

    async fn set_receiver_configuration(
        &mut self,
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        if receiver_configuration.integrate && !self.receiver_configuration.integrate {
            log::info!("Starting integration");
            self.receiver_configuration.integrate = true;
        } else if !receiver_configuration.integrate && self.receiver_configuration.integrate {
            log::info!("Stopping integration");
            self.receiver_configuration.integrate = false;
        }
        Ok(self.receiver_configuration)
    }

    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        let target_horizontal =
            calculate_target_horizontal(self.location, Utc::now(), self.target, self.horizontal);

        let horizontal_offset_squared = (target_horizontal.azimuth - self.horizontal.azimuth)
            .powi(2)
            + (target_horizontal.altitude - self.horizontal.altitude).powi(2);
        let status = {
            if self.target == TelescopeTarget::Stopped {
                TelescopeStatus::Idle
            } else if horizontal_offset_squared > 0.2f64.to_radians().powi(2) {
                TelescopeStatus::Slewing
            } else if self.target == TelescopeTarget::Parked {
                TelescopeStatus::Idle
            } else {
                TelescopeStatus::Tracking
            }
        };

        let latest_observation = if self.current_spectra.is_empty() {
            None
        } else {
            let mut latest_observation = ObservedSpectra {
                frequencies: vec![0f32; FAKE_TELESCOPE_CHANNELS],
                spectra: vec![0f32; FAKE_TELESCOPE_CHANNELS],
                observation_time: Duration::from_secs(0),
            };
            for integration in &self.current_spectra {
                latest_observation.spectra = latest_observation
                    .spectra
                    .into_iter()
                    .zip(integration.spectra.iter())
                    .map(|(a, b)| a + b)
                    .collect();
                latest_observation.observation_time += integration.observation_time;
            }
            latest_observation.frequencies = self.current_spectra[0].frequencies.clone();
            latest_observation.spectra = latest_observation
                .spectra
                .into_iter()
                .map(|value| value / self.current_spectra.len() as f32)
                .collect();
            Some(latest_observation)
        };

        Ok(TelescopeInfo {
            status,
            current_horizontal: self.horizontal,
            commanded_horizontal: Some(target_horizontal),
            current_target: self.target,
            most_recent_error: self.most_recent_error.clone(),
            measurement_in_progress: self.receiver_configuration.integrate,
            latest_observation,
        })
    }

    async fn update(&mut self, delta_time: Duration) -> Result<(), TelescopeError> {
        let now = Utc::now();
        let current_horizontal = self.horizontal;
        let target_horizontal =
            calculate_target_horizontal(self.location, now, self.target, current_horizontal);

        if target_horizontal.altitude < LOWEST_ALLOWED_ALTITUDE {
            self.target = TelescopeTarget::Stopped;
            log::info!(
                "Stopping telescope since target {:?} set below horizon.",
                &self.target
            );
            self.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
        } else {
            let max_delta_angle = FAKE_TELESCOPE_SLEWING_SPEED * delta_time.as_secs_f64();
            self.horizontal.azimuth += (target_horizontal.azimuth - current_horizontal.azimuth)
                .clamp(-max_delta_angle, max_delta_angle);
            self.horizontal.altitude += (target_horizontal.altitude - current_horizontal.altitude)
                .clamp(-max_delta_angle, max_delta_angle);
        }

        if self.receiver_configuration.integrate {
            self.current_spectra.push(create_fake_spectra(delta_time))
        }

        Ok(())
    }

    async fn restart(&mut self) -> Result<(), TelescopeError> {
        self.most_recent_error = None;
        self.receiver_configuration.integrate = false;
        self.current_spectra.clear();
        Ok(())
    }
}

fn create_fake_spectra(integration_time: Duration) -> ObservedSpectra {
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

fn calculate_target_horizontal(
    location: Location,
    when: DateTime<Utc>,
    target: TelescopeTarget,
    current_horizontal: Direction,
) -> Direction {
    match target {
        TelescopeTarget::Equatorial { ra, dec } => {
            common::coords::horizontal_from_equatorial(location, when, ra, dec)
        }
        TelescopeTarget::Galactic { l, b } => {
            common::coords::horizontal_from_galactic(location, when, l, b)
        }
        TelescopeTarget::Stopped => current_horizontal,
        TelescopeTarget::Parked => FAKE_TELESCOPE_PARKING_HORIZONTAL,
    }
}
