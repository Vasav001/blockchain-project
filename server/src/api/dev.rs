//! Development/demo-only endpoint(s). Not part of the normal application
//! surface: this project has no coinbase/reward mechanism, so a wallet can
//! only ever spend what it's actually received, and there is otherwise no
//! way to give one a starting balance (see the README's "Known
//! limitations"). `POST /api/dev/fund` exists purely to make local demos
//! and manual testing practical, replacing the previous approach (a
//! temporary `#[cfg(test)]` snippet compiled into `server/src/db/mod.rs`
//! by hand for each use).
//!
//! Only registered in the router when `config::dev_funding_enabled()` is
//! true (`DEV_FUNDING_ENABLED=true`) - see `api::router`. When disabled,
//! this route simply doesn't exist, so it 404s exactly like any other
//! unknown path; there is no separate "disabled" response to weaken or
//! bypass.
//!
//! It deliberately reuses the same building blocks the real transaction
//! and mining paths use, rather than writing new blockchain logic:
//! `Transaction::signed_by` (signs with a throwaway, one-off `Wallet` -
//! the "sender" of a funding transaction has no meaning beyond satisfying
//! the transaction format), the same `Transaction::validation_error`
//! structural checks `POST /api/transactions` runs, `Block::mine` (so the
//! funding block satisfies the same Proof-of-Work every other block
//! does), and `db::insert_block` (the same atomic block+transactions
//! persistence `POST /api/mine` uses). Normal transaction/mempool
//! validation is untouched - this only ever constructs transactions that
//! already pass it.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::blockchain::{Block, Transaction};
use crate::config::MINING_DIFFICULTY;
use crate::db;
use crate::error::AppError;
use crate::wallet::Wallet;

use super::AppState;

#[derive(Deserialize)]
pub struct DevFundRequest {
    pub address: String,
    pub amount: i64,
}

/// POST /api/dev/fund - mints `amount` to `address` from a throwaway
/// keypair generated on the spot, mines it into a real block, and
/// persists it.
pub async fn fund(
    State(state): State<AppState>,
    Json(request): Json<DevFundRequest>,
) -> Result<Json<Block>, AppError> {
    let faucet = Wallet::generate();
    let transaction: Transaction = Transaction::signed_by(&faucet, request.address, request.amount);

    // Same structural validation `POST /api/transactions` runs (positive
    // amount, non-empty address, sender != recipient, signature checks
    // out) - this can only fail here on a bad `address`/`amount`, since
    // the signature and sender/public-key pairing are always
    // self-consistent (we just generated and signed with `faucet`
    // ourselves).
    if let Some(reason) = transaction.validation_error() {
        return Err(AppError::InvalidTransaction(reason));
    }

    let previous = db::latest_block(&state.pool).await?.ok_or(AppError::ChainEmpty)?;
    let funded = Block::mine(&previous, vec![transaction], MINING_DIFFICULTY);
    db::insert_block(&state.pool, &funded).await?;

    Ok(Json(funded))
}
