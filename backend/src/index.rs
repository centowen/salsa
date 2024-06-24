use crate::template::HtmlTemplate;
use askama::Template;
use axum::response::IntoResponse;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

pub async fn get_index() -> impl IntoResponse {
    HtmlTemplate(IndexTemplate {})
}
