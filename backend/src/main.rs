use crate::telescope::{create_telescope_collection, TELESCOPE_UPDATE_INTERVAL};
use clap::Parser;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::http::header;
use warp::http::Method;
use warp::Filter;

mod booking_routes;
mod fake_telescope;
mod frontend_routes;
mod salsa_telescope;
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

    #[arg(short, long)]
    telescope_address: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();

    let telescopes = create_telescope_collection();
    {
        let mut telescopes = telescopes.write().await;
        telescopes.insert(
            "fake".to_string(),
            Arc::new(Mutex::new(fake_telescope::create("fake".to_string()))),
        );
        telescopes.insert(
            //"brage".to_string(),
            "vale".to_string(),
            Arc::new(Mutex::new(salsa_telescope::create(
                //"192.168.5.12:23".to_string(), // torre
                //"192.168.5.10:23".to_string(), // brage
                "192.168.5.11:23".to_string(), // vale
            ))),
        );
    }

    let telescope_services = {
        let telescopes: Vec<_> = {
            let telescopes = telescopes.read().await;
            telescopes.values().cloned().collect()
        };
        log::info!("Starting {} telescope services", telescopes.len());
        telescopes
            .into_iter()
            .map(|telescope| {
                log::info!("Starting telescope service for telescope");
                tokio::spawn(async move {
                    loop {
                        {
                            let mut telescope = telescope.clone().lock_owned().await;
                            if let Err(error) = telescope.update(TELESCOPE_UPDATE_INTERVAL).await {
                                log::error!("Failed to update telescope: {}", error);
                            }
                        }
                        tokio::time::sleep(TELESCOPE_UPDATE_INTERVAL).await;
                    }
                })
            })
            .collect::<Vec<_>>()
    };
    log::info!("Started {} telescope services", telescope_services.len());

    let weather_routes = warp::path!("api" / "weather").map(weather::get_weather_info);

    let routes = frontend_routes::routes(args.frontend_path.clone())
        .or(weather_routes)
        .or(booking_routes::routes())
        .or(telescope_routes::routes(telescopes))
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
    for telescope_service in telescope_services {
        telescope_service
            .await
            .expect("Could not join telescope service at end of program");
    }
}
