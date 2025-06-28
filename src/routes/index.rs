use std::fs::read_to_string;

use askama::Template;
use axum::{
    Extension,
    response::{Html, IntoResponse, Response},
};

use crate::models::user::User;

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    name: String,
    content: String,
}

pub async fn get_index(Extension(user): Extension<Option<User>>) -> Response {
    Html(render_main(
        user,
        // TODO: Read this file at startup.
        read_to_string("assets/welcome.html").expect("Reading static data should always work"),
    ))
    .into_response()
}

pub fn render_main(user: Option<User>, content: String) -> String {
    IndexTemplate {
        name: match user {
            Some(User {
                id: _,
                name,
                provider,
            }) => format!("{} ({})", name, provider),
            None => String::new(),
        },
        content,
    }
    .render()
    .expect("Template should always succeed")
}
