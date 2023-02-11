use crate::telescope::create_telescope_control;
use warp::http::header;
use warp::http::Method;
use warp::Filter;

mod telescope;
mod telescope_routes;
mod weather;

#[tokio::main]
async fn main() {
    env_logger::init();
    let telescope = create_telescope_control();

    let weather_info = warp::path!("weather");
    let weather_info_routes = weather_info.map(weather::get_weather_info);

    let routes = weather_info_routes
        .or(telescope_routes::routes(telescope))
        .with(
            warp::cors()
                .allow_credentials(true)
                .allow_methods(vec![Method::GET, Method::POST])
                .allow_headers(vec![header::CONTENT_TYPE])
                .expose_headers(vec![header::LINK])
                .max_age(300)
                // .allow_origin("http://localhost")
                .allow_any_origin(),
        );

    warp::serve(routes).run(([127, 0, 0, 1], 3000)).await;
}
