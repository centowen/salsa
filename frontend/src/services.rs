use std::time::Duration;

use common::{TelescopeError, TelescopeInfo, WeatherInfo};
use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::platform::time::sleep;
use yew::virtual_dom::AttrValue;
use yew::Callback;

const UPDATE_INTERVAL: Duration = Duration::from_secs(10);

pub fn emit_weather_info(weather_cb: Callback<AttrValue>) {
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

pub fn emit_info(
    info_callback: Callback<TelescopeInfo>,
    endpoint: String,
    update_interval: Duration,
) {
    spawn_local(async move {
        loop {
            match Request::get(&endpoint).send().await {
                Ok(response) => match response
                    .json::<Result<TelescopeInfo, TelescopeError>>()
                    .await
                    .expect("Failed to deserialize response")
                {
                    Ok(telescope_info) => {
                        info_callback.emit(telescope_info);
                    }
                    Err(error) => {
                        log::error!("Got error response from {}: {}", &endpoint, error);
                    }
                },
                Err(error) => {
                    log::error!("Failed to fetch from {}: {}", &endpoint, error);
                }
            }

            sleep(update_interval).await;
        }
    });
}
