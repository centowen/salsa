use clap::Parser;
use database::create_database_from_directory;
use std::net::Ipv4Addr;
use telescope::create_telescope_collection;
use warp::http::header;
use warp::http::Method;
use warp::Filter;

mod booking_routes;
mod database;
mod fake_telescope;
mod frontend_routes;
mod salsa_telescope;
mod telescope;
mod telescope_controller;
mod telescope_routes;
mod telescope_tracker;
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

    #[arg(short, long)]
    telescope_address: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();

    let database = create_database_from_directory("database.json")
        .await
        .expect("failed to create database");

    let telescopes = create_telescope_collection(&database)
        .await
        .expect("failed to create telescopes");

    let weather_routes = warp::path!("api" / "weather").map(weather::get_weather_info);

    let routes = frontend_routes::routes(args.frontend_path.clone())
        .or(weather_routes)
        .or(booking_routes::routes(database))
        .or(telescope_routes::routes(telescopes.clone()))
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
    {
        let mut telescopes = telescopes.write().await;
        for telescope in telescopes.values_mut() {
            if let Some(service) = telescope.service.take() {
                service
                    .await
                    .expect("Could not join telescope service at end of program");
            }
        }
    }
}
