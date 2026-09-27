use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::blockchain::{Block, Chain, Transaction};
use crate::db;
use crate::error::AppError;

use super::AppState;

/// GET /api/blocks - every stored block, in chain order.
pub async fn list_blocks(State(state): State<AppState>) -> Result<Json<Vec<Block>>, AppError> {
    let blocks = db::load_all_blocks(&state.pool).await?;
    Ok(Json(blocks))
}

/// GET /api/blocks/{index} - a single block, or 404 if it doesn't exist.
pub async fn get_block(
    State(state): State<AppState>,
    Path(index): Path<u64>,
) -> Result<Json<Block>, AppError> {
    let block = db::find_block(&state.pool, index)
        .await?
        .ok_or(AppError::BlockNotFound(index))?;

    Ok(Json(block))
}

#[derive(Serialize)]
pub struct ValidationResponse {
    valid: bool,
}

/// GET /api/chain/valid - loads the chain from storage and validates it.
pub async fn chain_valid(State(state): State<AppState>) -> Result<Json<ValidationResponse>, AppError> {
    let blocks = db::load_all_blocks(&state.pool).await?;
    let chain = Chain::from_blocks(blocks);

    Ok(Json(ValidationResponse {
        valid: chain.is_valid(),
    }))
}

/// The request body for `POST /api/transactions`. Kept separate from
/// `Transaction` so a client can never supply its own `id` - the server
/// always derives it from the other fields.
///
/// A real client signs locally (with its own `Wallet`, never sent to the
/// server) and submits the already-signed result here; the server only
/// ever verifies, it never signs on anyone's behalf.
#[derive(Deserialize)]
pub struct TransactionRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: i64,
    pub sender_public_key: String,
    pub signature: String,
}

/// POST /api/transactions - validates (including the signature) and queues
/// a transaction, returning it (with its computed id) on success, or 400
/// with a reason on failure.
pub async fn submit_transaction(
    State(state): State<AppState>,
    Json(request): Json<TransactionRequest>,
) -> Result<Json<Transaction>, AppError> {
    let transaction = Transaction::new(
        request.sender,
        request.recipient,
        request.amount,
        request.sender_public_key,
        request.signature,
    );

    if let Some(reason) = transaction.validation_error() {
        return Err(AppError::InvalidTransaction(reason));
    }

    let mut mempool = state.mempool.lock().map_err(|_| AppError::MempoolPoisoned)?;
    mempool.add_transaction(transaction.clone());

    Ok(Json(transaction))
}

/// GET /api/transactions/pending - every transaction currently queued in
/// the mempool.
pub async fn pending_transactions(
    State(state): State<AppState>,
) -> Result<Json<Vec<Transaction>>, AppError> {
    let mempool = state.mempool.lock().map_err(|_| AppError::MempoolPoisoned)?;
    Ok(Json(mempool.pending_transactions().to_vec()))
}
