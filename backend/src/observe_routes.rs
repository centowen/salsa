use crate::telescope::TelescopeCollection;
use crate::template::HtmlTemplate;
use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Router};
use common::{Direction, TelescopeError};

pub fn routes(telescopes: TelescopeCollection) -> Router {
    Router::new().route("/", get(get_observe))
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
    telescope_name: String,
    direction: Direction,
}

async fn get_observe(
    State(telescopes): State<TelescopeCollection>,
) -> Result<impl IntoResponse, TelescopeNotFound> {
    let telescopes = telescopes.read().await;
    let telescope = telescopes.get("fake").ok_or(TelescopeNotFound)?;
    let telescope = telescope.telescope.clone().lock_owned().await;
    let info = telescope.get_info().await?;
    Ok(HtmlTemplate(ObserveTemplate {
        telescope_name: info.id,
        direction: telescope.get_direction().await?,
    }))
}
