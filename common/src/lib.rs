use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WeatherInfo {
    pub temperature: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TelescopeTarget
{
    Equatorial {
        ra: f32, // in radians
        dec: f32, // in radians
    },
    Galactic {
        l: f32, // in radians
        b: f32, // in radians
    },
}
