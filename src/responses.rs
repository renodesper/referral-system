use axum::{
    Json,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct RequestMeta {
    pub request_id: String,
    pub request_at: String, // RFC3339
    pub timestamp: i64,     // unix seconds
                            // NOTE: we only set `code` on errors, so keep it out of success responses.
                            // (Errors carry their own meta with `code`; see error.rs)
}

fn new_meta() -> RequestMeta {
    let now: DateTime<Utc> = Utc::now();
    RequestMeta {
        request_id: Uuid::new_v4().to_string(),
        request_at: now.to_rfc3339(),
        timestamp: now.timestamp(),
    }
}

use axum::body::Body;

// Middleware: attaches RequestMeta into request extensions
pub async fn meta_middleware(mut req: Request<Body>, next: Next) -> Response {
    let meta = new_meta();
    req.extensions_mut().insert(meta);
    next.run(req).await
}

#[derive(Clone, Debug, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

#[derive(Serialize)]
pub struct SuccessEnvelope<T> {
    pub message: String,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
    pub meta: RequestMeta,
}

pub struct ApiOk<T> {
    status: axum::http::StatusCode,
    body: SuccessEnvelope<T>,
}

impl<T> ApiOk<T> {
    pub fn ok(message: impl Into<String>, data: T, meta: RequestMeta) -> Self {
        Self {
            status: axum::http::StatusCode::OK,
            body: SuccessEnvelope {
                message: message.into(),
                data,
                pagination: None,
                meta,
            },
        }
    }
    pub fn created(message: impl Into<String>, data: T, meta: RequestMeta) -> Self {
        Self {
            status: axum::http::StatusCode::CREATED,
            body: SuccessEnvelope {
                message: message.into(),
                data,
                pagination: None,
                meta,
            },
        }
    }
}

impl<T: Serialize> IntoResponse for ApiOk<T> {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
