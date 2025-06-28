use crate::coords::{Direction, Location};
use crate::coords::{horizontal_from_equatorial, horizontal_from_galactic};
use crate::models::telescope::Telescope;
use crate::models::telescope_types::{
    ObservedSpectra, ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo,
    TelescopeStatus, TelescopeTarget,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand::Rng;
use rand_distr::StandardNormal;
use std::f64::consts::PI;
use std::time::Duration;

const FAKE_TELESCOPE_PARKING_HORIZONTAL: Direction = Direction {
    azimuth: 0.0,
    elevation: PI / 2.0,
};
pub const LOWEST_ALLOWED_ELEVATION: f64 = 5.0 / 180. * PI;

pub const FAKE_TELESCOPE_SLEWING_SPEED: f64 = PI / 10.0;
pub const FAKE_TELESCOPE_CHANNELS: usize = 400;
pub const FAKE_TELESCOPE_CHANNEL_WIDTH: f64 = 2e6f64 / FAKE_TELESCOPE_CHANNELS as f64;
pub const FAKE_TELESCOPE_FIRST_CHANNEL: f64 =
    1.420e9f64 - FAKE_TELESCOPE_CHANNEL_WIDTH * FAKE_TELESCOPE_CHANNELS as f64 / 2f64;
pub const FAKE_TELESCOPE_NOISE: f64 = 2f64;

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
    async fn get_direction(&self) -> Result<Direction, TelescopeError> {
        Ok(self.horizontal)
    }

    async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        self.most_recent_error = None;
        self.receiver_configuration.integrate = false;
        self.current_spectra.clear();

        let target_horizontal = calculate_target_horizontal(self.location, Utc::now(), target);
        if target_horizontal.elevation < LOWEST_ALLOWED_ELEVATION {
            log::info!(
                "Refusing to set target for telescope {} to {:?}. Target is below horizon",
                &self.name,
                &target
            );
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
        let target_horizontal = calculate_target_horizontal(self.location, Utc::now(), self.target);

        let horizontal_offset_squared = (target_horizontal.azimuth - self.horizontal.azimuth)
            .powi(2)
            + (target_horizontal.elevation - self.horizontal.elevation).powi(2);
        let status = {
            if horizontal_offset_squared > 0.2f64.to_radians().powi(2) {
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
                frequencies: vec![0f64; FAKE_TELESCOPE_CHANNELS],
                spectra: vec![0f64; FAKE_TELESCOPE_CHANNELS],
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
                .map(|value| value / self.current_spectra.len() as f64)
                .collect();
            Some(latest_observation)
        };
        Ok(TelescopeInfo {
            id: self.name.clone(),
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
        let target_horizontal = calculate_target_horizontal(self.location, now, self.target);

        if target_horizontal.elevation < LOWEST_ALLOWED_ELEVATION {
            log::info!(
                "Stopping telescope since target {:?} set below horizon.",
                &self.target
            );
            self.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
        } else {
            let max_delta_angle = FAKE_TELESCOPE_SLEWING_SPEED * delta_time.as_secs_f64();
            self.horizontal.azimuth += (target_horizontal.azimuth - current_horizontal.azimuth)
                .clamp(-max_delta_angle, max_delta_angle);
            self.horizontal.elevation += (target_horizontal.elevation
                - current_horizontal.elevation)
                .clamp(-max_delta_angle, max_delta_angle);
        }

        if self.receiver_configuration.integrate {
            log::trace!("Pushing spectum...");
            self.current_spectra.push(create_fake_spectra(delta_time))
        }

        Ok(())
    }
}

fn create_fake_spectra(integration_time: Duration) -> ObservedSpectra {
    let mut rng = rand::rng();

    let frequencies: Vec<f64> = (0..FAKE_TELESCOPE_CHANNELS)
        .map(|channel| channel as f64 * FAKE_TELESCOPE_CHANNEL_WIDTH + FAKE_TELESCOPE_FIRST_CHANNEL)
        .collect();
    let spectra: Vec<f64> = vec![5f64; FAKE_TELESCOPE_CHANNELS]
        .into_iter()
        .map(|value| {
            value + FAKE_TELESCOPE_NOISE * rng.sample::<f64, StandardNormal>(StandardNormal)
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
) -> Direction {
    match target {
        TelescopeTarget::Equatorial {
            right_ascension: ra,
            declination: dec,
        } => horizontal_from_equatorial(location, when, ra, dec),
        TelescopeTarget::Galactic {
            longitude: l,
            latitude: b,
        } => horizontal_from_galactic(location, when, l, b),
        TelescopeTarget::Horizontal {
            azimuth: az,
            elevation: el,
        } => Direction {
            azimuth: az,
            elevation: el,
        },
        TelescopeTarget::Parked => FAKE_TELESCOPE_PARKING_HORIZONTAL,
    }
}
