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
    pub most_recent_error: Option<TelescopeError>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub enum TelescopeError {
    TargetBelowHorizon,
}

impl Display for TelescopeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error_message = match self {
            TelescopeError::TargetBelowHorizon {} => {
                "Failed to set target, target is below horizon."
            }
        };
        f.write_str(&error_message)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct Location {
    pub longitude: f64,
    pub latitude: f64,
}
