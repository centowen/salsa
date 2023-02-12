use serde::{Deserialize, Serialize};

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
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub struct Location {
    pub longitude: f64,
    pub latitude: f64,
}
