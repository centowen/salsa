use crate::coords::Direction;
use crate::telescope::{Telescope, TelescopeCollection, TelescopeContainer};
use crate::telescopes::{ReceiverConfiguration, ReceiverError, TelescopeStatus};
use crate::telescopes::{TelescopeError, TelescopeInfo, TelescopeTarget};
use askama::Template;
use axum::response::Html;
use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};

pub fn routes(telescopes: TelescopeCollection) -> Router {
    let telescope_routes = Router::new()
        .route("/", get(get_telescope))
        .route("/direction", get(get_direction))
        .route("/target", get(get_target).post(set_target))
        .route("/restart", post(restart))
        .route("/receiver", post(set_receiver_configuration))
        .route("/state", get(get_state));
    Router::new()
        .route("/", get(get_telescopes))
        .nest("/{telescope_id}", telescope_routes)
        .with_state(telescopes)
}

async fn get_telescopes(State(telescopes): State<TelescopeCollection>) -> Json<Vec<TelescopeInfo>> {
    let mut telescope_infos = Vec::<TelescopeInfo>::new();
    for (name, telescope) in telescopes.read().await.iter() {
        log::trace!("Checking {}", name);
        let telescope = telescope.telescope.lock().await;
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

async fn extract_telescope(
    telescopes: TelescopeCollection,
    id: String,
) -> Result<tokio::sync::OwnedMutexGuard<dyn Telescope>, TelescopeNotFound> {
    let telescpes = telescopes.read().await;
    let telescope = telescpes.get(&id).ok_or(TelescopeNotFound)?;
    Ok(telescope.telescope.clone().lock_owned().await)
}

async fn get_telescope(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<TelescopeInfo, TelescopeError>>, TelescopeNotFound> {
    let telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.get_info().await))
}

async fn get_direction(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<Direction, TelescopeError>>, TelescopeNotFound> {
    let telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.get_direction().await))
}

async fn get_target(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<TelescopeTarget, TelescopeError>>, TelescopeNotFound> {
    let telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.get_target().await))
}

async fn set_target(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
    Json(target): Json<TelescopeTarget>,
) -> Result<Json<Result<TelescopeTarget, TelescopeError>>, TelescopeNotFound> {
    let mut telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.set_target(target).await))
}

async fn restart(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
) -> Result<Json<Result<(), TelescopeError>>, TelescopeNotFound> {
    let mut telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.restart().await))
}

async fn set_receiver_configuration(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
    Json(target): Json<ReceiverConfiguration>,
) -> Result<Json<Result<ReceiverConfiguration, ReceiverError>>, TelescopeNotFound> {
    let mut telescope = extract_telescope(telescopes, telescope_id).await?;
    Ok(Json(telescope.set_receiver_configuration(target).await))
}

pub async fn get_state(
    State(telescopes): State<TelescopeCollection>,
    Path(telescope_id): Path<String>,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let telescopes_lock = telescopes.read().await;
    let telescope = telescopes_lock
        .get(&telescope_id)
        .ok_or(TelescopeNotFound)?;
    Ok(Html(state(telescope.clone()).await?))
}

#[derive(Template)]
#[template(path = "telescope_state.html")]
struct TelescopeStateTemplate {
    info: TelescopeInfo,
    status: String,
    direction: Direction,
}

pub async fn state(telescope: TelescopeContainer) -> Result<String, TelescopeNotFound> {
    let telescope = telescope.telescope.clone().lock_owned().await;
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
