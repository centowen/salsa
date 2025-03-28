use crate::index::render_main;
use crate::telescope::{TelescopeCollectionHandle, TelescopeHandle};
use crate::telescope_routes::state;
use crate::telescopes::{
    ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo, TelescopeTarget,
};
use askama::Template;
use axum::Form;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{
    Router,
    routing::{get, post},
};
use serde::Deserialize;

pub fn routes(telescopes: TelescopeCollectionHandle) -> Router {
    let observe_routes = Router::new()
        .route("/", get(get_observe))
        .route("/", post(post_observe));
    Router::new()
        .nest("/{telescope_id}", observe_routes)
        .with_state(telescopes)
}

#[derive(Deserialize, Debug)]
struct Target {
    x: f64, // Degrees
    y: f64, // Degrees
    coordinate_system: String,
    action: String,
}

enum ObserveError {
    BadQuery(String),
    TelescopeError(TelescopeError),
    ReceiverError(ReceiverError),
    TelescopeNotFound(String),
}

impl IntoResponse for ObserveError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ObserveError::BadQuery(message) => error_response(message),
            ObserveError::TelescopeError(telescope_error) => telescope_error.into_response(),
            ObserveError::TelescopeNotFound(id) => {
                error_response(format!("Could not find telescope {}", id))
            }
            ObserveError::ReceiverError(receiver_error) => receiver_error.into_response(),
        }
    }
}

impl From<TelescopeError> for ObserveError {
    fn from(telescope_error: TelescopeError) -> Self {
        ObserveError::TelescopeError(telescope_error)
    }
}

impl IntoResponse for TelescopeError {
    fn into_response(self) -> Response {
        error_response(format!("{self}"))
    }
}

impl From<ReceiverError> for ObserveError {
    fn from(receiver_error: ReceiverError) -> Self {
        ObserveError::ReceiverError(receiver_error)
    }
}

impl IntoResponse for ReceiverError {
    fn into_response(self) -> Response {
        error_response(format!("{self}"))
    }
}

fn error_response(message: String) -> Response {
    // Create a response that will specifically update the error box on the page.
    Response::builder()
        .status(StatusCode::OK) // Needs to be ok to be picked up by htmx.
        .header("HX-Retarget", "#errors")
        .body(Body::from(message))
        .expect("Building a response should never fail")
}

async fn post_observe(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
    Form(target): Form<Target>,
) -> Result<impl IntoResponse, ObserveError> {
    let x_rad = target.x.to_radians();
    let y_rad = target.y.to_radians();
    let target = match (target.action.as_str(), target.coordinate_system.as_str()) {
        ("go", "galactic") => TelescopeTarget::Galactic {
            longitude: x_rad,
            latitude: y_rad,
        },
        ("go", "equatorial") => TelescopeTarget::Equatorial {
            right_ascension: x_rad,
            declination: y_rad,
        },
        ("park", _) => TelescopeTarget::Parked,
        ("go", coordinate_system) => {
            return Err(ObserveError::BadQuery(format!(
                "Unkown coordinate system {}",
                coordinate_system
            )));
        }
        (action, _) => {
            return Err(ObserveError::BadQuery(format!("Unkown action {}", action)));
        }
    };

    let mut telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(ObserveError::TelescopeNotFound("fake".to_string()))?;
    telescope.set_target(target).await?;
    // FIXME: Don't set integrate to true immediately, control from UI instead.
    telescope
        .set_receiver_configuration(ReceiverConfiguration { integrate: true })
        .await?;
    let content = observe(telescope.clone()).await?;
    Ok(Html(content))
}

async fn get_observe(
    State(telescopes): State<TelescopeCollectionHandle>,
    Path(telescope_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ObserveError> {
    let telescope = telescopes
        .get(&telescope_id)
        .await
        .ok_or(ObserveError::TelescopeNotFound("fake".to_string()))?;
    let content = observe(telescope.clone()).await?;
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(content)
    };
    Ok(Html(content))
}

#[derive(Template)]
#[template(path = "observe.html", escape = "none")]
struct ObserveTemplate {
    info: TelescopeInfo,
    target_mode: String,
    commanded_x: String,
    commanded_y: String,
    state_html: String,
}

async fn observe(telescope: TelescopeHandle) -> Result<String, TelescopeError> {
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
            right_ascension,
            declination,
        } => (
            right_ascension.to_degrees().to_string(),
            declination.to_degrees().to_string(),
        ),
        TelescopeTarget::Galactic {
            longitude,
            latitude,
        } => (
            longitude.to_degrees().to_string(),
            latitude.to_degrees().to_string(),
        ),
        TelescopeTarget::Parked => (String::new(), String::new()),
        _ => (
            info.current_horizontal.azimuth.to_degrees().to_string(),
            info.current_horizontal.altitude.to_degrees().to_string(),
        ),
    };
    let state_html = state(telescope.clone()).await?;
    Ok(ObserveTemplate {
        info,
        target_mode,
        commanded_x,
        commanded_y,
        state_html,
    }
    .render()
    .expect("Template rendering should always succeed"))
}
