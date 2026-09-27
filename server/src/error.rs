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
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BlockNotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidTransaction(_) => StatusCode::BAD_REQUEST,
            AppError::MempoolPoisoned => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
