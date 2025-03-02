use std::fs::read_to_string;

use crate::template::HtmlTemplate;
use askama::Template;
use axum::response::{Html, IntoResponse, Response};

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    content: String,
}

pub async fn get_index() -> Response {
    // TODO: Read this file at startup.
    Html(render_main(
        read_to_string("assets/welcome.html").expect("Reading static data should always work"),
    ))
    .into_response()
}

pub fn render_main(content: String) -> String {
    IndexTemplate { content }
        .render()
        .expect("Template should always succeed")
}
