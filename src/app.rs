use async_session::MemoryStore;
use authentication::extract_session;
use axum::middleware;
use axum::{Router, routing::get};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::authentication;
use crate::bookings;
use crate::database::create_sqlite_database_on_disk;
use crate::index;
use crate::observe_routes;
use crate::telescope::{TelescopeCollectionHandle, create_telescope_collection};
use crate::telescope_routes;

// Anything that goes in here must be a handle or pointer that can be cloned.
// The underlying state itself should be shared.
#[derive(Clone)]
pub struct AppState {
    pub database_connection: Arc<Mutex<Connection>>,
    pub store: MemoryStore,
    pub telescopes: TelescopeCollectionHandle,
}

pub async fn create_app() -> Router {
    let database_connection = Arc::new(Mutex::new(
        create_sqlite_database_on_disk("database.sqlite3")
            .expect("failed to create sqlite database"),
    ));

    let store = MemoryStore::new();

    let telescopes = create_telescope_collection("telescopes.toml");

    let state = AppState {
        database_connection,
        store,
        telescopes,
    };

    let mut app = Router::new()
        .route("/", get(index::get_index))
        .nest("/auth", authentication::routes(state.clone()))
        .nest("/observe", observe_routes::routes(state.clone()))
        .nest("/bookings", bookings::routes::routes(state.clone()))
        .nest("/telescope", telescope_routes::routes(state.clone()))
        .layer(TraceLayer::new_for_http())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            extract_session,
        ));

    let assets_path = "assets";
    log::debug!("serving asserts from {}", assets_path);
    let assets_service = ServeDir::new(assets_path);
    app = app.fallback_service(assets_service);
    app
}
