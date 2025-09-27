use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// The application state.
#[derive(Clone)]
pub struct AppState {
    /// The database pool.
    pub pool: PgPool,
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

/// A referral.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Referral {
    /// The ID of the referral.
    pub id: i64,
    /// The ID of the user who was referred.
    pub user_id: i64,
    /// The ID of the user who referred.
    pub referrer_id: i64,
    /// The timestamp when the referral was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A referral code.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReferralCode {
    /// The ID of the referral code.
    pub id: i64,
    /// The ID of the user who owns the referral code.
    pub user_id: i64,
    /// The referral code.
    pub code: String,
    /// The timestamp when the referral code was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
