use crate::coords::Direction;
use crate::index::render_main;
use crate::telescope::TelescopeCollection;
use crate::telescopes::{TelescopeError, TelescopeInfo, TelescopeStatus, TelescopeTarget};
use askama::Template;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{Router, routing::get};

pub fn routes(telescopes: TelescopeCollection) -> Router {
    Router::new()
        .route("/", get(get_observe))
        .with_state(telescopes)
}

#[derive(Debug)]
struct TelescopeNotFound;
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

#[derive(Template)]
#[template(path = "observe.html")]
struct ObserveTemplate {
    info: TelescopeInfo,
    status: String,
    target_mode: String,
    direction: Direction,
    commanded_x: f64,
    commanded_y: f64,
}

async fn get_observe(
    headers: HeaderMap,
    State(telescopes): State<TelescopeCollection>,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let telescopes = telescopes.read().await;
    let telescope = telescopes.get("fake").ok_or(TelescopeNotFound)?;
    let telescope = telescope.telescope.clone().lock_owned().await;
    let info = telescope.get_info().await?;
    let target_mode = match &info.current_target {
        TelescopeTarget::Equatorial { .. } => "equatorial",
        TelescopeTarget::Galactic { .. } => "galactic",
        TelescopeTarget::Parked => "equatorial",
        TelescopeTarget::Stopped => "equatorial",
    }
    .to_string();
    let (commanded_x, commanded_y) = match info.current_target {
        TelescopeTarget::Equatorial { ra, dec } => (ra, dec),
        TelescopeTarget::Galactic { l, b } => (l, b),
        _ => (
            info.current_horizontal.azimuth,
            info.current_horizontal.altitude,
        ),
    };

    let content = ObserveTemplate {
        info: info.clone(),
        status: match &info.status {
            TelescopeStatus::Idle => "Idle".to_string(),
            TelescopeStatus::Slewing => "Slewing".to_string(),
            TelescopeStatus::Tracking => "Tracking".to_string(),
        },
        target_mode,
        direction: telescope.get_direction().await?,
        commanded_x,
        commanded_y,
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(content)
    };
    Ok(Html(content).into_response())
}
