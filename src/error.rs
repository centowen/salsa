use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::error;

pub struct InternalError {
    pub message: String,
}

impl InternalError {
    pub fn new(message: String) -> InternalError {
        InternalError { message }
    }
}

impl IntoResponse for InternalError {
    fn into_response(self) -> Response {
        // (thak): I find it somewhat dubious to log here in the conversion
        // function ... but I can't deny it's convenient.
        error!(
            "Error encountered while processiong request: {}",
            self.message
        );
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
