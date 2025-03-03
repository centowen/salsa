use crate::index::render_main;
use crate::telescope::TelescopeCollection;
use crate::telescope_routes::{TelescopeNotFound, state};
use crate::telescopes::{TelescopeInfo, TelescopeStatus, TelescopeTarget};
use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
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
        "galactic" => TelescopeTarget::Galactic {
            longitude: x_rad,
            latitude: y_rad,
        },
        "equatorial" => TelescopeTarget::Equatorial {
            right_ascension: x_rad,
            declination: y_rad,
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
#[template(path = "observe.html", escape = "none")]
struct ObserveTemplate {
    info: TelescopeInfo,
    status: String,
    target_mode: String,
    commanded_x: f64,
    commanded_y: f64,
    state_html: String,
}

async fn observe(telescopes: TelescopeCollection) -> Result<String, TelescopeNotFound> {
    // We have to be a little careful about the locking.
    let telescopes_lock = telescopes.read().await;
    let telescope = telescopes_lock.get("fake").ok_or(TelescopeNotFound)?;
    // First extract all data needed for the primary template.
    let (info, status, target_mode, commanded_x, commanded_y) = {
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
            TelescopeTarget::Equatorial {
                right_ascension: ra,
                declination: dec,
            } => (ra, dec),
            TelescopeTarget::Galactic {
                longitude: l,
                latitude: b,
            } => (l, b),
            _ => (
                info.current_horizontal.azimuth,
                info.current_horizontal.altitude,
            ),
        };
        let status = match &info.status {
            TelescopeStatus::Idle => "Idle".to_string(),
            TelescopeStatus::Slewing => "Slewing".to_string(),
            TelescopeStatus::Tracking => "Tracking".to_string(),
        };
        (info, status, target_mode, commanded_x, commanded_y)
    };
    // After releasing all locks render the state subtemplate.
    let state_html = state(telescope.clone()).await?;
    // Finally we can render the full template.
    Ok(ObserveTemplate {
        info,
        status,
        target_mode,
        commanded_x,
        commanded_y,
        state_html,
    }
    .render()
    .expect("Template rendering should always succeed"))
}
