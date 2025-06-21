use crate::bookings::Booking;
use crate::database::{DataBase, Storage};
use crate::index::render_main;
use crate::telescope::{TelescopeCollectionHandle, TelescopeHandle};
use crate::telescope_routes::state;
use crate::telescopes::{
    ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo, TelescopeTarget,
};
use crate::user::User;
use askama::Template;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use axum::{
    Router,
    routing::{get, post},
};
use chrono::Utc;
use serde::Deserialize;

#[derive(Clone)]
struct ObserveState<StorageType: Storage> {
    telescopes: TelescopeCollectionHandle,
    database: DataBase<StorageType>,
}

pub fn routes(
    telescopes: TelescopeCollectionHandle,
    database: DataBase<impl Storage + 'static>,
) -> Router {
    let observe_routes = Router::new()
        .route("/", get(get_observe))
        .route("/set-target", post(set_target))
        .route("/observe", post(start_observe));
    Router::new()
        .nest("/{telescope_id}", observe_routes)
        .with_state(ObserveState {
            telescopes,
            database,
        })
}

#[derive(Deserialize, Debug)]
struct Target {
    x: f64, // Degrees
    y: f64, // Degrees
    coordinate_system: String,
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

async fn set_target<StorageType>(
    State(state): State<ObserveState<StorageType>>,
    Path(telescope_id): Path<String>,
    Form(target): Form<Target>,
) -> Result<impl IntoResponse, ObserveError>
where
    StorageType: Storage,
{
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
        "horizontal" => TelescopeTarget::Horizontal {
            azimuth: x_rad,
            elevation: y_rad,
        },
        coordinate_system => {
            return Err(ObserveError::BadQuery(format!(
                "Unkown coordinate system {}",
                coordinate_system
            )));
        }
    };

    let mut telescope = state
        .telescopes
        .get(&telescope_id)
        .await
        .ok_or(ObserveError::TelescopeNotFound("fake".to_string()))?;
    telescope.set_target(target).await?;
    let content = observe(telescope.clone()).await?;
    Ok(Html(content))
}

async fn start_observe<StorageType>(
    State(state): State<ObserveState<StorageType>>,
    Path(telescope_id): Path<String>,
) -> Result<impl IntoResponse, ObserveError>
where
    StorageType: Storage,
{
    let mut telescope = state
        .telescopes
        .get(&telescope_id)
        .await
        .ok_or(ObserveError::TelescopeNotFound("fake".to_string()))?;
    telescope
        .set_receiver_configuration(ReceiverConfiguration { integrate: true })
        .await?;
    let content = observe(telescope.clone()).await?;
    Ok(Html(content))
}

fn has_active_booking(user: &User, bookings: &[Booking]) -> bool {
    let now = Utc::now();
    for booking in bookings {
        if booking.user_name != user.name {
            continue;
        }
        if now > booking.start_time && now < booking.end_time {
            return true;
        }
    }
    false
}

async fn get_observe<StorageType>(
    Extension(user): Extension<Option<User>>,
    State(state): State<ObserveState<StorageType>>,
    Path(telescope_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ObserveError>
where
    StorageType: Storage,
{
    let data_model = state
        .database
        .get_data()
        .await
        .expect("As long as no one is manually editing the database, this should never fail.");
    let bookings = data_model.bookings;
    if user.is_none() || !has_active_booking(user.as_ref().unwrap(), &bookings) {
        let content = DontObserveTemplate {}
            .render()
            .expect("Template rendering should always succeed");
        let content = if headers.get("hx-request").is_some() {
            content
        } else {
            render_main(user.clone(), content)
        };
        return Ok(Html(content));
    }
    let telescope = state
        .telescopes
        .get(&telescope_id)
        .await
        .ok_or(ObserveError::TelescopeNotFound("fake".to_string()))?;
    let content = observe(telescope.clone()).await?;
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(user, content)
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
        TelescopeTarget::Horizontal { .. } => "horizontal",
        TelescopeTarget::Parked => "equatorial",
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
        TelescopeTarget::Horizontal { azimuth, elevation } => (
            azimuth.to_degrees().to_string(),
            elevation.to_degrees().to_string(),
        ),
        TelescopeTarget::Parked => (String::new(), String::new()),
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

#[derive(Template)]
#[template(path = "dont_observe.html", escape = "none")]
struct DontObserveTemplate {}
