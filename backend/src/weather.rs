use common::WeatherInfo;
use rand::thread_rng;
use rand::Rng;

pub async fn get_weather_info() -> String {
    // TODO: Read temperature from relevant endpoint
    let mut rng = thread_rng();
    let weather_info = WeatherInfo {
        temperature: rng.gen_range(3.1..5.2),
    };
    serde_json::to_string(&weather_info).unwrap()
}
