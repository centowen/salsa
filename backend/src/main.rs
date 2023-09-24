use warp::http::header;
use warp::http::Method;
use warp::Filter;

mod weather;

#[tokio::main]
async fn main() {
    let weather_info = warp::path!("weather");
    let weather_info_routes = weather_info.map(weather::get_weather_info);

    let routes = weather_info_routes.with(
        warp::cors()
            .allow_credentials(true)
            .allow_methods(vec![Method::GET])
            .allow_headers(vec![header::CONTENT_TYPE])
            .expose_headers(vec![header::LINK])
            .max_age(300)
            // .allow_origin("http://localhost"),
            .allow_any_origin(),
    );

    warp::serve(routes).run(([127, 0, 0, 1], 3000)).await;
}
