mod error;

use anyhow::Context;
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse};
use tracing_subscriber::{
    EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
use tracing::Level;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

const L1: i32 = 1;
const L2: i32 = 2;

const L1_PERCENTAGE: i32 = 10;
const L2_PERCENTAGE: i32 = 5;

// const PAYMENT_STATUS_AUTHORIZED: &str = "authorized";
const PAYMENT_STATUS_CAPTURED: &str = "captured";
// const PAYMENT_STATUS_REFUNDED: &str = "refunded";
// const PAYMENT_STATUS_VOIDED: &str = "voided";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = create_pool().await?;
    let app_state = AppState { pool };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8000);
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await?;

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/balances/{user_id}", get(get_balance_handler))
        .route("/purchases", post(create_purchase_handler))
        .route("/process/{id}", post(process_purchase_handler))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http()
                    .make_span_with(
                        DefaultMakeSpan::new()
                            .level(Level::INFO) // span level
                    )
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    println!("Listening on 0.0.0.0:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_pool() -> Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;
    Ok(pool)
}

#[derive(Deserialize)]
struct CreatePurchaseRequest {
    user_id: i64,
    amount: i64,
    status: String,
    id: Option<Uuid>,
}

#[derive(Serialize)]
struct CreatePurchaseResponse {
    id: Uuid,
}

async fn get_balance_handler(
    State(st): State<AppState>,
    Path(user_id): Path<i64>,
) -> Result<Json<serde_json::Value>, String> {
    let row = sqlx::query!(
        r#"SELECT balance FROM balances WHERE user_id = $1"#,
        user_id
    )
        .fetch_optional(&st.pool)
        .await
        .map_err(|e| e.to_string())?;
    let balance: i64 = row
        .map(|r| r.balance)
        .unwrap_or(0);

    Ok(Json(
        serde_json::json!({ "user_id": user_id, "balance": balance }),
    ))
}

async fn create_purchase_handler(
    State(st): State<AppState>,
    Json(req): Json<CreatePurchaseRequest>,
) -> Result<(StatusCode, Json<CreatePurchaseResponse>), AppError> {
    let id = req.id.unwrap_or_else(Uuid::new_v4);

    if req.amount < 0 {
        return Err(AppError::BadRequest("amount must be >= 0".into()));
    }

    let res = sqlx::query!(
        r#"INSERT INTO purchases (id, user_id, amount, status) VALUES ($1, $2, $3, $4)"#,
        id,
        req.user_id,
        req.amount,
        req.status
    )
    .execute(&st.pool)
    .await;

    match res {
        Ok(_) => Ok((StatusCode::CREATED, Json(CreatePurchaseResponse { id }))),
        Err(e) => {
            // NOTE: 23505 = unique_violation
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23505") {
                    return Err(AppError::Conflict("purchase already exists".into()));
                }
            }
            Err(AppError::Internal(e.into()))
        }
    }
}

async fn process_purchase_handler(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, String> {
    process_purchase(&st.pool, id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({ "processed": id })))
}

async fn process_purchase(pool: &PgPool, purchase_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;

    let rec =
        sqlx::query!(
            r#"SELECT id, user_id, amount, status FROM purchases WHERE id = $1 FOR UPDATE"#,
            purchase_id
        )
            .fetch_one(tx.as_mut())
            .await?;

    if rec.status.as_str() != PAYMENT_STATUS_CAPTURED {
        tx.commit().await?;
        return Ok(());
    }

    let buyer_id: i64 = rec.user_id;
    let amount: i64 = rec.amount;

    let l1 = active_referrer(&mut tx, buyer_id).await?;
    let l2 = match l1 {
        Some(u) => active_referrer(&mut tx, u).await?,
        None => None,
    };

    if let Some(u1) = l1 {
        let amt = percent_of(amount, L1_PERCENTAGE);
        let has_been_rewarded = has_rewarded(&mut tx, purchase_id, u1, L1).await?;

        if amt > 0 && !has_been_rewarded {
            insert_reward(&mut tx, purchase_id, buyer_id, u1, 1, amt).await?;
            add_balance(&mut tx, u1, amt).await?;
        }
    }
    if let Some(u2) = l2 {
        let amt = percent_of(amount, L2_PERCENTAGE);
        let has_been_rewarded = has_rewarded(&mut tx, purchase_id, u2, L2).await?;

        if amt > 0 && !has_been_rewarded {
            insert_reward(&mut tx, purchase_id, buyer_id, u2, 2, amt).await?;
            add_balance(&mut tx, u2, amt).await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

fn percent_of(amount: i64, percent: i32) -> i64 {
    ((amount as i128 * percent as i128) / 100) as i64
}

async fn active_referrer(tx: &mut Transaction<'_, Postgres>, user_id: i64) -> Result<Option<i64>> {
    let row = sqlx::query!(r#"SELECT referrer_id FROM users WHERE id = $1"#, user_id)
        .fetch_one(tx.as_mut()) // <- use underlying connection from Transaction
        .await?;

    let referrer_id = row.referrer_id;
    if let Some(rid) = referrer_id {
        if let Some(r2) = sqlx::query!(r#"SELECT is_active FROM users WHERE id = $1"#, rid)
            .fetch_optional(tx.as_mut())
            .await?
        {
            if r2.is_active {
                return Ok(Some(rid));
            }
        }
    }
    Ok(None)
}

async fn has_rewarded(tx: &mut Transaction<'_, Postgres>, purchase_id: Uuid, beneficiary_user_id: i64, level: i32) -> Result<bool> {
    let id = sqlx::query_scalar!(
        r#"SELECT 1 FROM rewards where purchase_id = $1 AND beneficiary_user_id = $2 AND level = $3"#,
        purchase_id,
        beneficiary_user_id,
        level
    )
        .fetch_optional(tx.as_mut())
        .await?;
    Ok(id.is_some())
}

async fn insert_reward(
    tx: &mut Transaction<'_, Postgres>,
    purchase_id: Uuid,
    user_id: i64,
    beneficiary_user_id: i64,
    level: i32,
    amount: i64,
) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO rewards (purchase_id, user_id, beneficiary_user_id, level, amount) VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (purchase_id, beneficiary_user_id, level) DO NOTHING"#,
        purchase_id,
        user_id,
        beneficiary_user_id,
        level,
        amount
    )
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn add_balance(tx: &mut Transaction<'_, Postgres>, user_id: i64, delta: i64) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO balances (user_id, balance) VALUES ($1, $2)
           ON CONFLICT (user_id) DO UPDATE SET balance = balances.balance + EXCLUDED.balance"#,
        user_id,
        delta
    )
    .execute(tx.as_mut())
    .await?;
    Ok(())
}
