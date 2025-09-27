use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{
    ApiError, ApiErrorWithMeta, E_BAD_AMOUNT, E_DB_FAILURE, E_PROCESS_FAILURE, E_PURCHASE_CONFLICT,
};
use crate::process_purchase;
use crate::responses::{ApiOk, RequestMeta, meta_middleware};

/// The application state.
#[derive(Clone)]
pub struct AppState {
    /// The database pool.
    pub pool: PgPool,
    /// The application configuration.
    pub config: Config,
}

/// The request to create a new purchase.
#[derive(Deserialize)]
pub struct CreatePurchaseRequest {
    /// The ID of the user who made the purchase.
    pub user_id: i64,
    /// The amount of the purchase.
    pub amount: i64,
    /// The status of the purchase.
    pub status: String,
    /// The ID of the purchase.
    pub id: Option<Uuid>,
}

/// The response after creating a new purchase.
#[derive(Serialize)]
pub struct CreatePurchaseResponse {
    /// The ID of the purchase.
    pub id: Uuid,
}

/// The response for a user's balance.
#[derive(Serialize)]
pub struct BalanceResponse {
    /// The ID of the user.
    pub user_id: i64,
    /// The user's balance.
    pub balance: i64,
}

/// The response after processing a purchase.
#[derive(Serialize)]
pub struct ProcessResponse {
    /// The ID of the processed purchase.
    pub processed: Uuid,
}

pub fn init_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/balances/{user_id}", get(get_balance_handler))
        .route("/purchases", post(create_purchase_handler))
        .route("/process/{id}", post(process_purchase_handler))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(middleware::from_fn(meta_middleware))
}

async fn get_balance_handler(
    State(st): State<AppState>,
    Path(user_id): Path<i64>,
    Extension(meta): Extension<RequestMeta>,
) -> Result<ApiOk<BalanceResponse>, ApiErrorWithMeta> {
    let row = sqlx::query!(
        r#"SELECT balance FROM balances WHERE user_id = $1"#,
        user_id
    )
    .fetch_optional(&st.pool)
    .await
    .map_err(|e| {
        ApiError::Internal(e.into())
            .with_meta(meta.clone())
            .with_code(E_DB_FAILURE)
    })?;

    let balance: i64 = row.map(|r| r.balance).unwrap_or(0);

    Ok(ApiOk::ok(
        "balance fetched",
        BalanceResponse { user_id, balance },
        meta,
    ))
}

async fn create_purchase_handler(
    State(st): State<AppState>,
    Extension(meta): Extension<RequestMeta>,
    Json(req): Json<CreatePurchaseRequest>,
) -> Result<ApiOk<CreatePurchaseResponse>, ApiErrorWithMeta> {
    let id = req.id.unwrap_or_else(Uuid::new_v4);

    if req.amount < 0 {
        return Err(ApiError::BadRequest("amount must be >= 0".into())
            .with_meta(meta)
            .with_code(E_BAD_AMOUNT));
    }

    sqlx::query!(
        r#"INSERT INTO purchases (id, user_id, amount, status) VALUES ($1, $2, $3, $4)"#,
        id,
        req.user_id,
        req.amount,
        req.status
    )
    .execute(&st.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.code().as_deref() == Some("23505") {
                return ApiError::Conflict("purchase already exists".into())
                    .with_meta(meta.clone())
                    .with_code(E_PURCHASE_CONFLICT);
            }
        }
        ApiError::Internal(e.into())
            .with_meta(meta.clone())
            .with_code(E_DB_FAILURE)
    })?;

    Ok(ApiOk::created(
        "purchase created",
        CreatePurchaseResponse { id },
        meta,
    ))
}

async fn process_purchase_handler(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(meta): Extension<RequestMeta>,
) -> Result<ApiOk<ProcessResponse>, ApiErrorWithMeta> {
    process_purchase(&st.pool, id).await.map_err(|e| {
        ApiError::Internal(e)
            .with_meta(meta.clone())
            .with_code(E_PROCESS_FAILURE)
    })?;

    Ok(ApiOk::ok(
        "purchase processed",
        ProcessResponse { processed: id },
        meta,
    ))
}
