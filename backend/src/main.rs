use crate::telescope::{create_telescope, TelescopeControl, TELESCOPE_UPDATE_INTERVAL};
use clap::Parser;
use std::net::Ipv4Addr;
use warp::http::header;
use warp::http::Method;
use warp::Filter;

mod frontend_routes;
mod telescope;
mod telescope_routes;
mod weather;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to packaged wasm app
    #[arg(short, long)]
    frontend_path: Option<String>,

    /// Ip to listen to
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();

    let telescope = create_telescope();

    let telescope_service = tokio::spawn({
        let telescope = telescope.clone();
        async move {
            loop {
                if let Err(error) = telescope.update(TELESCOPE_UPDATE_INTERVAL).await {
                    log::error!("Failed to update telescope: {}", error);
                }
                tokio::time::sleep(TELESCOPE_UPDATE_INTERVAL).await;
            }
        }
    });

    let weather_routes = warp::path!("api" / "weather").map(weather::get_weather_info);

    let routes = frontend_routes::routes(args.frontend_path.clone())
        .or(weather_routes)
        .or(telescope_routes::routes(telescope))
        .with(
            warp::cors()
                .allow_credentials(true)
                .allow_methods(vec![Method::HEAD, Method::GET, Method::POST])
                .allow_headers(vec![header::CONTENT_TYPE])
                .expose_headers(vec![header::LINK])
                .max_age(300)
                // .allow_origin("http://localhost")
                .allow_any_origin(),
        );

    let ip = match args.ip.parse::<Ipv4Addr>() {
        Ok(ip) => ip,
        Err(error) => {
            log::error!("Cannot parse ip \"{}\": {}", args.ip, error);
            return;
        }
    };

    warp::serve(routes).run((ip, 3000)).await;
    telescope_service
        .await
        .expect("Could not join telescope service at end of program");
}
