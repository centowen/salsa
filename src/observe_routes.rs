use crate::coords::Direction;
use crate::index::render_main;
use crate::telescope::TelescopeCollection;
use crate::telescopes::{TelescopeError, TelescopeInfo, TelescopeStatus, TelescopeTarget};
use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{
    Router,
    routing::{get, post},
};
use serde::Deserialize;

pub fn routes(telescopes: TelescopeCollection) -> Router {
    Router::new()
        .route("/", get(get_observe))
        .with_state(telescopes.clone())
        .route("/", post(post_observe))
        .with_state(telescopes.clone())
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

#[derive(Deserialize, Debug)]
struct Target {
    x: f64, // Degrees
    y: f64, // Degrees
    coordinate_system: String,
}

async fn post_observe(
    State(telescopes): State<TelescopeCollection>,
    Form(target): Form<Target>,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let x_rad = target.x.to_radians();
    let y_rad = target.y.to_radians();
    let target = match target.coordinate_system.as_str() {
        "galactic" => TelescopeTarget::Galactic { l: x_rad, b: y_rad },
        "equatorial" => TelescopeTarget::Equatorial {
            ra: x_rad,
            dec: y_rad,
        },
        _ => return Err(TelescopeNotFound {}), // TODO: Proper errors!
    };
    {
        let telescopes_lock = telescopes.read().await;
        let telescope = telescopes_lock.get("fake").ok_or(TelescopeNotFound)?;
        let mut telescope = telescope.telescope.clone().lock_owned().await;
        telescope.set_target(target).await?;
    }
    let content = observe(telescopes).await?;
    Ok(Html(content).into_response())
}

async fn get_observe(
    State(telescopes): State<TelescopeCollection>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let content = observe(telescopes).await?;
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(content)
    };
    Ok(Html(content).into_response())
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

async fn observe(telescopes: TelescopeCollection) -> Result<String, TelescopeNotFound> {
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

    Ok(ObserveTemplate {
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
    .expect("Template rendering should always succeed"))
}
