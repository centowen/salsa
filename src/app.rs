use async_session::MemoryStore;
use authentication::{AuthenticationState, extract_session};
use axum::middleware;
use axum::{Router, routing::get};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::authentication;
use crate::bookings;
use crate::database::{create_database_from_directory, create_sqlite_database_on_disk};
use crate::index;
use crate::observe_routes;
use crate::telescope::create_telescope_collection;
use crate::telescope_routes;

pub async fn create_app() -> Router {
    let database = create_database_from_directory("database.json")
        .await
        .expect("failed to create database");

    let database_connection = Arc::new(Mutex::new(
        create_sqlite_database_on_disk("database.sqlite3")
            .expect("failed to create sqlite database"),
    ));

    let store = MemoryStore::new();

    let telescopes = create_telescope_collection(&database)
        .await
        .expect("failed to create telescopes");

    let mut app = Router::new()
        .route("/", get(index::get_index))
        .nest(
            "/auth",
            authentication::routes(database_connection.clone(), store.clone()),
        )
        .nest(
            "/observe",
            observe_routes::routes(telescopes.clone(), database.clone()),
        )
        .nest("/bookings", bookings::routes::routes(database.clone()))
        .nest("/telescope", telescope_routes::routes(telescopes.clone()))
        .layer(TraceLayer::new_for_http())
        .route_layer(middleware::from_fn_with_state(
            AuthenticationState {
                database_connection,
                store,
            },
            extract_session,
        ));

    let assets_path = "assets";
    log::debug!("serving asserts from {}", assets_path);
    let assets_service = ServeDir::new(assets_path);
    app = app.fallback_service(assets_service);
    app
}
