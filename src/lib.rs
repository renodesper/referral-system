//! The main library for the referral system.

mod api;
mod error;
mod responses;
mod types;

use anyhow::Context;
use anyhow::Result;
pub use api::init_router;
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
pub use types::{AppState, Referral, ReferralCode};
use uuid::Uuid;

/// The percentage for the first level referrer.
pub const L1_PERCENTAGE: i32 = 10;
/// The percentage for the second level referrer.
pub const L2_PERCENTAGE: i32 = 5;

// const PAYMENT_STATUS_AUTHORIZED: &str = "authorized";
const PAYMENT_STATUS_CAPTURED: &str = "captured";
// const PAYMENT_STATUS_REFUNDED: &str = "refunded";
// const PAYMENT_STATUS_VOIDED: &str = "voided";

/// Initializes the database pool.
pub async fn init_pool() -> Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;
    Ok(pool)
}

/// Processes a purchase and distributes the rewards to the referrers.
pub async fn process_purchase(pool: &PgPool, purchase_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;

    let rec = sqlx::query!(
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
        if amt > 0 && insert_reward(&mut tx, purchase_id, buyer_id, u1, 1, amt).await? {
            add_balance(&mut tx, u1, amt).await?;
        }
    }
    if let Some(u2) = l2 {
        let amt = percent_of(amount, L2_PERCENTAGE);
        if amt > 0 && insert_reward(&mut tx, purchase_id, buyer_id, u2, 2, amt).await? {
            add_balance(&mut tx, u2, amt).await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub fn percent_of(amount: i64, percent: i32) -> i64 {
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

async fn insert_reward(
    tx: &mut Transaction<'_, Postgres>,
    purchase_id: Uuid,
    user_id: i64,
    beneficiary_user_id: i64,
    level: i32,
    amount: i64,
) -> Result<bool> {
    let res = sqlx::query!(
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

    Ok(res.rows_affected() == 1)
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
