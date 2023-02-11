use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WeatherInfo {
    pub temperature: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
pub enum TelescopeTarget {
    Equatorial {
        ra: f32,  // in radians
        dec: f32, // in radians
    },
    Galactic {
        l: f32, // in radians
        b: f32, // in radians
    },
    Parked,
    Stopped,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Direction {
    pub azimuth: f32,
    pub elevation: f32,
}
