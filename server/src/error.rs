use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application-wide error type for the API layer. Keeps `unwrap`/`expect`
/// out of request handlers by giving every failure a path to an HTTP
/// response via `IntoResponse`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("block {0} not found")]
    BlockNotFound(u64),

    #[error("invalid transaction: {0}")]
    InvalidTransaction(&'static str),

    #[error("internal error: mempool lock poisoned")]
    MempoolPoisoned,

    #[error("no pending transactions to mine")]
    NothingToMine,

    #[error("internal error: chain has no blocks to mine on top of")]
    ChainEmpty,

    #[error("insufficient balance")]
    InsufficientBalance,

    #[error("duplicate transaction")]
    DuplicateTransaction,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BlockNotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidTransaction(_) => StatusCode::BAD_REQUEST,
            AppError::MempoolPoisoned => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NothingToMine => StatusCode::BAD_REQUEST,
            AppError::ChainEmpty => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InsufficientBalance => StatusCode::BAD_REQUEST,
            AppError::DuplicateTransaction => StatusCode::BAD_REQUEST,
        };

        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
