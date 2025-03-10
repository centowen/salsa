use crate::coords::Direction;
use crate::telescope::{TelescopeCollectionHandle, TelescopeHandle};
use crate::telescopes::{ReceiverConfiguration, ReceiverError, TelescopeStatus};
use crate::telescopes::{TelescopeError, TelescopeInfo, TelescopeTarget};
use askama::Template;
use axum::extract::ws::{Message, Utf8Bytes};
use axum::{
    Router,
    extract::ws::{WebSocket, WebSocketUpgrade},
    extract::{Json, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{any, get, post},
};

pub fn routes(telescopes: TelescopeCollectionHandle) -> Router {
    let telescope_routes = Router::new()
        .route("/", get(get_telescope))
        .route("/direction", get(get_direction))
        .route("/target", get(get_target).post(set_target))
        .route("/restart", post(restart))
        .route("/receiver", post(set_receiver_configuration))
        .route("/state", get(get_state))
        .route("/spectrum", any(spectrum_handle_upgrade));
    Router::new()
        .route("/", get(get_telescopes))
        .nest("/{telescope_id}", telescope_routes)
        .with_state(telescopes)
}

async fn spectrum_handle_upgrade(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    // WebSockets come in as a regular HTTP request, that connection is then
    // upgraded to a socket.
    upgrade.on_upgrade(spectrum_handle_websocket)
}

async fn spectrum_handle_websocket(mut socket: WebSocket) {
    match socket
        .send(Message::Text(Utf8Bytes::from("hello world")))
        .await
    {
        _ => return,
    }
    // Real implementation
    // Send on socket
    // Receive ctrl messages from socket?
}

async fn get_telescopes(
    State(telescopes): State<TelescopeCollectionHandle>,
) -> Json<Vec<TelescopeInfo>> {
    let mut telescope_infos = Vec::<TelescopeInfo>::new();
    for (name, telescope) in telescopes.all().await.iter() {
        log::trace!("Checking {}", name);
        if let Ok(info) = telescope.get_info().await {
            log::trace!("Accepted {}", name);
            telescope_infos.push(info);
        } else {
            log::trace!("Rejected {}", name);
        }
    }
    Json(telescope_infos)
}

#[derive(Debug)]
pub struct TelescopeNotFound;

impl IntoResponse for TelescopeNotFound {
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, "Telescope not found".to_string()).into_response()
    }
}

// HACK: Hacky hack!
impl From<TelescopeError> for TelescopeNotFound {
    fn from(_: TelescopeError) -> Self {
        TelescopeNotFound {}
    }
}

async fn get_telescope(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<TelescopeInfo, TelescopeError>>, TelescopeNotFound> {
    let telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.get_info().await))
}

async fn get_direction(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<Direction, TelescopeError>>, TelescopeNotFound> {
    let telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.get_direction().await))
}

async fn get_target(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<TelescopeTarget, TelescopeError>>, TelescopeNotFound> {
    let telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.get_target().await))
}

async fn set_target(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
    Json(target): Json<TelescopeTarget>,
) -> Result<Json<Result<TelescopeTarget, TelescopeError>>, TelescopeNotFound> {
    let mut telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.set_target(target).await))
}

async fn restart(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<(), TelescopeError>>, TelescopeNotFound> {
    let mut telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.restart().await))
}

async fn set_receiver_configuration(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
    Json(target): Json<ReceiverConfiguration>,
) -> Result<Json<Result<ReceiverConfiguration, ReceiverError>>, TelescopeNotFound> {
    let mut telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Json(telescope.set_receiver_configuration(target).await))
}

pub async fn get_state(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(TelescopeNotFound)?;
    Ok(Html(state(telescope).await?))
}

#[derive(Template)]
#[template(path = "telescope_state.html")]
struct TelescopeStateTemplate {
    info: TelescopeInfo,
    status: String,
    direction: Direction,
}

pub async fn state(telescope: TelescopeHandle) -> Result<String, TelescopeError> {
    let info = telescope.get_info().await?;
    Ok(TelescopeStateTemplate {
        info: info.clone(),
        status: match &info.status {
            TelescopeStatus::Idle => "Idle".to_string(),
            TelescopeStatus::Slewing => "Slewing".to_string(),
            TelescopeStatus::Tracking => "Tracking".to_string(),
        },
        direction: telescope.get_direction().await?,
    }
    .render()
    .expect("Template rendering should always succeed"))
}
