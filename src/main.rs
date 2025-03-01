use axum::{Router, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use database::create_database_from_directory;
use std::net::SocketAddr;
use telescope::create_telescope_collection;
use tower_http::services::ServeDir;

mod bookings;
mod coords;
mod database;
mod fake_telescope;
mod index;
mod observe_routes;
mod salsa_telescope;
mod telescope;
mod telescope_api_routes;
mod telescope_controller;
mod telescope_routes;
mod telescope_tracker;
mod telescopes;
mod template;
mod weather;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "KEY_FILE_PATH")]
    key_file_path: Option<String>,

    #[arg(short, long, env = "CERT_FILE_PATH")]
    cert_file_path: Option<String>,
    s: Option<String>,
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

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let mut app = Router::new()
        .route("/", get(index::get_index))
        .nest("/observe", observe_routes::routes(telescopes.clone()))
        .route("/weather", get(weather::get_weather_info))
        .nest("/bookings", bookings::routes::routes(database.clone()))
        .nest("/telescopes", telescope_routes::routes(telescopes.clone()))
        .nest("/api/telescopes", telescope_api_routes::routes(telescopes))
        .nest(
            "/api/bookings",
            bookings::api_routes::routes(database.clone()),
        );

    let assets_path = "assets";
    log::info!("serving asserts from {}", assets_path);
    let assets_service = ServeDir::new(assets_path);
    app = app.fallback_service(assets_service);

    log::info!("listening on {}", addr);
    if let Some(key_file_path) = args.key_file_path {
        let cert_file_path = args.cert_file_path.unwrap();
        log::info!(
            "using tls with key file {} and cert file {}",
            key_file_path,
            cert_file_path
        );
        let tls = RustlsConfig::from_pem_file(cert_file_path, key_file_path)
            .await
            .unwrap();
        axum_server::bind_rustls(addr, tls)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}
