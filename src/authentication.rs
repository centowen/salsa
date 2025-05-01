use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};

#[derive(Clone)]
pub struct User {
    pub name: String,
}

pub async fn authenticate(mut request: Request, next: Next) -> Result<Response, StatusCode> {
    // TODO: Insert real authentication here.
    request.extensions_mut().insert(User {
        name: String::from("frood"),
    });
    Ok(next.run(request).await)
}
