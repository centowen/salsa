use crate::template::HtmlTemplate;
use askama::Template;
use axum::response::IntoResponse;
use axum::{routing::get, Router};

pub fn routes() -> Router {
    Router::new().route("/", get(get_observe))
}

#[derive(Template)]
#[template(path = "observe.html")]
struct ObserveTemplate {}

async fn get_observe() -> impl IntoResponse {
    HtmlTemplate(ObserveTemplate {})
}
