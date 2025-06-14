use std::fs::read_to_string;

use askama::Template;
use axum::{
    Extension,
    response::{Html, IntoResponse, Response},
};

use crate::user::User;

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    name: String,
    content: String,
}

pub async fn get_index(Extension(user): Extension<User>) -> Response {
    // TODO: Read this file at startup.
    Html(render_main(
        user.name,
        read_to_string("assets/welcome.html").expect("Reading static data should always work"),
    ))
    .into_response()
}

pub fn render_main(name: String, content: String) -> String {
    IndexTemplate { name, content }
        .render()
        .expect("Template should always succeed")
}
