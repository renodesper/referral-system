use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::BadRequest(msg) => {
                let body = Json(json!({ "error": msg }));
                (StatusCode::BAD_REQUEST, body).into_response()
            }
            AppError::NotFound(msg) => {
                let body = Json(json!({ "error": msg }));
                (StatusCode::NOT_FOUND, body).into_response()
            }
            AppError::Conflict(msg) => {
                let body = Json(json!({ "error": msg }));
                (StatusCode::CONFLICT, body).into_response()
            }
            AppError::Internal(err) => {
                tracing::error!(error = ?err, "internal server error");

                let body = Json(json!({ "error": "internal server error" }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}
