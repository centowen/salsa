use chrono::{offset::Utc, DateTime};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
pub struct WeatherInfo {
    pub temperature: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub enum TelescopeTarget {
    Equatorial {
        ra: f64,  // in radians
        dec: f64, // in radians
    },
    Galactic {
        l: f64, // in radians
        b: f64, // in radians
    },
    Parked,
    Stopped,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct Direction {
    pub azimuth: f64,
    pub altitude: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub enum TelescopeStatus {
    Idle,
    Slewing,
    Tracking,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ObservedSpectra {
    pub frequencies: Vec<f64>,
    pub spectra: Vec<f64>,
    pub observation_time: Duration,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TelescopeInfo {
    pub id: String,
    pub status: TelescopeStatus,
    pub commanded_horizontal: Option<Direction>,
    pub current_horizontal: Direction,
    pub current_target: TelescopeTarget,
    pub most_recent_error: Option<TelescopeError>,
    pub measurement_in_progress: bool,
    pub latest_observation: Option<ObservedSpectra>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SalsaTelescopeDefinition {
    pub controller_address: String,
    pub receiver_address: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct FakeTelescopeDefinition {
    pub slewing_speed: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TelescopeType {
    Salsa {
        definition: SalsaTelescopeDefinition,
    },
    Fake {
        definition: FakeTelescopeDefinition,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TelescopeDefinition {
    pub name: String,
    pub enabled: bool,
    pub location: Location,
    pub min_altitude: f64,
    pub telescope_type: TelescopeType,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TelescopeError {
    TargetBelowHorizon,
    TelescopeIOError(String),
    TelescopeNotConnected,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub enum ReceiverError {
    IntegrationAlreadyRunning,
}

impl Display for TelescopeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TelescopeError::TargetBelowHorizon {} => {
                f.write_str("Failed to set target, target is below horizon.")
            }
            TelescopeError::TelescopeIOError(message) => f.write_str(&format!(
                "Error in communication with telescope: {}",
                message
            )),
            TelescopeError::TelescopeNotConnected => f.write_str("Telescope is not connected."),
        }
    }
}

impl From<std::io::Error> for TelescopeError {
    fn from(error: std::io::Error) -> Self {
        TelescopeError::TelescopeIOError(format!("Communication with telescope failed: {}", error))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct Location {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct ReceiverConfiguration {
    pub integrate: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Booking {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub telescope_name: String,
    pub user_name: String,
}

impl Booking {
    pub fn overlaps(&self, other: &Booking) -> bool {
        self.end_time >= other.start_time && self.start_time <= other.end_time
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum AddBookingError {
    ServiceUnavailable,
    Conflict,
    // NotFuture - booking is entirely(?) in the past
    // NonPositiveDuration - booking ends before it starts
}

pub type AddBookingResult = Result<u64, AddBookingError>;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Measurement {
    pub amps: Vec<f64>,
    pub freqs: Vec<f64>,
    //glon: f64,
    //glat: f64,
    pub start: DateTime<Utc>,
    pub duration: Duration,
    //stop: Option<DateTime<Utc>>,
    //vlsr_correction: Option<f64>,
    //telname: String,
    //tellat: f64,
    //tellon: f64,
}
