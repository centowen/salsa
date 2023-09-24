use axum::{routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use database::create_database_from_directory;
use std::net::SocketAddr;
use telescope::create_telescope_collection;
use tower_http::services::ServeDir;

mod booking_routes;
mod database;
mod fake_telescope;
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

    #[clap(short, long)]
    key_file_path: Option<String>,

    #[clap(short, long)]
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
        .route("/api/ping", get(ping))
        .route("/api/weather", get(weather::get_weather_info))
        .nest("/api/telescopes", telescope_routes::routes(telescopes))
        .nest("/api/bookings", booking_routes::routes(database.clone()));

    if let Some(frontend_path) = args.frontend_path {
        let frontend_service = ServeDir::new(frontend_path);
        app = app.fallback_service(frontend_service)
    }
    // .route("/api/token", post(token))
    // .route("/api/info", get(info))

    log::info!("listening on {}", addr);
    if let Some(key_file_path) = args.key_file_path {
        let cert_file_path = args.cert_file_path.unwrap();
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

async fn ping() -> &'static str {
    log::info!("ping");
    "pong"
}
