use std::time::Duration;

use common::WeatherInfo;
use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::platform::time::sleep;
use yew::virtual_dom::AttrValue;
use yew::Callback;

const UPDATE_INTERVAL: Duration = Duration::from_secs(10);

pub fn emit_weather_info(weather_cb: Callback<AttrValue>) {
    // Spawn a background task that will fetch a joke and send it to the component.
    spawn_local(async move {
        loop {
            log::info!("Fetching weather info");
            let response = Request::get("http://localhost:3000/weather").send().await;
            if response.is_ok() {
                let response = response.ok().unwrap();
                let weather_info: Result<WeatherInfo, _> = response.json().await;
                if weather_info.is_ok() {
                    // Emit it to the component
                    weather_cb.emit(AttrValue::from(format!(
                        "{:.2}",
                        weather_info.ok().unwrap().temperature
                    )));
                } else {
                    let error = weather_info.err().unwrap();
                    log::error!("Failed to parse weather info from backend: {}", error);
                }
            } else {
                let error = response.err().unwrap();
                log::error!("Failed to fetch weather info from backend: {}", error);
            }

            sleep(UPDATE_INTERVAL).await;
        }
    });
}
