use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

pub mod coords;

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

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct TelescopeInfo {
    pub status: TelescopeStatus,
    pub commanded_horizontal: Direction,
    pub current_horizontal: Direction,
    pub current_target: TelescopeTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TelescopeError {
    TargetBelowHorizon { telescope_id: String },
}

impl Display for TelescopeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error_message = match self {
            TelescopeError::TargetBelowHorizon { telescope_id: id } => format!(
                "Failed to set target for telescope {}, target is below horizon.",
                id
            ),
        };
        f.write_str(&error_message)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct Location {
    pub longitude: f64,
    pub latitude: f64,
}
