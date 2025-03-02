use rand::Rng;
use rand::rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WeatherInfo {
    pub temperature: f64,
}

pub async fn get_weather_info() -> String {
    // TODO: Read temperature from relevant endpoint
    let mut rng = rng();
    let weather_info = WeatherInfo {
        temperature: rng.random_range(3.1..5.2),
    };
    serde_json::to_string(&weather_info).unwrap()
}
