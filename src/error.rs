use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::error;

use crate::responses::RequestMeta;

pub const E_BAD_AMOUNT: &str = "BAD_AMOUNT";
pub const E_DB_FAILURE: &str = "DB_FAILURE";
pub const E_PURCHASE_CONFLICT: &str = "PURCHASE_CONFLICT";
pub const E_PROCESS_FAILURE: &str = "PROCESS_FAILURE";

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Conflict(String),
    Internal(anyhow::Error),
}

#[derive(Debug)]
pub struct ApiErrorWithMeta {
    error: ApiError,
    meta: RequestMeta,
    code: Option<String>,
}

impl ApiError {
    pub fn with_meta(self, meta: RequestMeta) -> ApiErrorWithMeta {
        ApiErrorWithMeta {
            error: self,
            meta,
            code: None,
        }
    }
}

impl ApiErrorWithMeta {
    pub fn with_code(mut self, code: &str) -> Self {
        self.code = Some(code.to_string());
        self
    }
}

impl IntoResponse for ApiErrorWithMeta {
    fn into_response(self) -> Response {
        let (status, error_message) = match self.error {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Internal(e) => {
                error!("internal error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };

        let mut body = json!({
            "request_id": self.meta.request_id,
            "error": error_message,
        });
        if let Some(code) = self.code {
            body["code"] = json!(code);
        }

        (status, Json(body)).into_response()
    }
}
