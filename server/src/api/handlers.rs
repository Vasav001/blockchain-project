use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::blockchain::{Block, Chain, RejectionReason, Transaction};
use crate::config::MINING_DIFFICULTY;
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

/// POST /api/transactions - validates (signature, structural rules, and
/// now confirmed-balance affordability) and queues a transaction,
/// returning it (with its computed id) on success, or 400 with a reason on
/// failure.
///
/// `state.submission_lock` is held across loading the chain and admitting
/// the transaction into the mempool, so two concurrent submissions from
/// the same sender can't both pass an affordability check computed
/// against the same pre-submission balance (see `AppState`'s doc comment).
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

    // Cheap structural/signature check first, before touching the lock or
    // the database at all - `try_add_transaction` re-checks this too (it
    // doesn't trust callers to have done it), but there's no reason to pay
    // for a DB round trip on an obviously malformed request.
    if let Some(reason) = transaction.validation_error() {
        return Err(AppError::InvalidTransaction(reason));
    }

    let _submission_guard = state.submission_lock.lock().await;

    let blocks = db::load_all_blocks(&state.pool).await?;
    let chain = Chain::from_blocks(blocks);

    let mut mempool = state.mempool.lock().map_err(|_| AppError::MempoolPoisoned)?;
    mempool
        .try_add_transaction(transaction.clone(), &chain)
        .map_err(|reason| match reason {
            RejectionReason::Invalid(message) => AppError::InvalidTransaction(message),
            RejectionReason::InsufficientBalance => AppError::InsufficientBalance,
            RejectionReason::Duplicate => AppError::DuplicateTransaction,
        })?;

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

/// POST /api/mine - mines every currently pending transaction into a new
/// block, persists it, and clears exactly those transactions from the
/// mempool. Takes no body: no private key, no transaction payload - it
/// only ever acts on what's already in the mempool.
///
/// `state.mining_lock` is held for the whole mine-then-persist-then-clear
/// sequence, so two concurrent requests can't both mine the same pending
/// transactions into two different blocks.
pub async fn mine(State(state): State<AppState>) -> Result<Json<Block>, AppError> {
    let _mining_guard = state.mining_lock.lock().await;

    let pending: Vec<Transaction> = {
        let mempool = state.mempool.lock().map_err(|_| AppError::MempoolPoisoned)?;
        mempool.pending_transactions().to_vec()
    };

    if pending.is_empty() {
        return Err(AppError::NothingToMine);
    }

    let previous = db::latest_block(&state.pool).await?.ok_or(AppError::ChainEmpty)?;

    let mined = Block::mine(&previous, pending, MINING_DIFFICULTY);

    // Only after this succeeds do we touch the mempool - if persistence
    // fails, `?` returns early here and every pending transaction is still
    // sitting in the mempool, untouched.
    db::insert_block(&state.pool, &mined).await?;

    {
        let mut mempool = state.mempool.lock().map_err(|_| AppError::MempoolPoisoned)?;
        mempool.remove_transactions(&mined.transactions);
    }

    Ok(Json(mined))
}

#[derive(Serialize)]
pub struct BalanceResponse {
    address: String,
    balance: i64,
}

/// GET /api/wallets/{address}/balance - `address`'s confirmed balance,
/// replayed from the persisted chain only. A pending-but-unmined
/// transaction never affects this; an address that's never appeared in a
/// confirmed transaction reports a balance of `0`, not a 404 - any string
/// is a syntactically valid address here, there's nothing to "not find".
pub async fn wallet_balance(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<BalanceResponse>, AppError> {
    let blocks = db::load_all_blocks(&state.pool).await?;
    let chain = Chain::from_blocks(blocks);
    let balance = chain.balance_of(&address);

    Ok(Json(BalanceResponse { address, balance }))
}
