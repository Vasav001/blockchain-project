mod dev;
mod handlers;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;

use crate::blockchain::Mempool;

/// Shared state handed to every handler via Axum's `State` extractor.
///
/// `pool` is SQLx's own cheaply-cloneable connection pool. `mempool` needs
/// its own sharing/mutability story: `Arc` so every clone of `AppState`
/// (Axum clones it per request) points at the *same* mempool, and `Mutex`
/// because `Mempool` itself has no interior mutability. The critical
/// sections that lock it are just `Vec` pushes/reads with no `.await`
/// inside them, so a plain `std::sync::Mutex` is the right tool here (a
/// `tokio::sync::Mutex` is only needed when a lock must be held across an
/// `.await` point).
///
/// `mining_lock` and `submission_lock` are *separate* locks from `mempool`
/// and from each other, each serializing only one kind of request against
/// itself:
/// - `mining_lock`: concurrent `/api/mine` requests, so two of them can't
///   both mine the same pending transactions into two different blocks.
/// - `submission_lock`: concurrent `POST /api/transactions` requests, so
///   two submissions from the same sender can't both pass a "sender can
///   afford this" check computed against the same stale mempool/chain
///   state (see `handlers::submit_transaction`).
///
/// Transaction submissions and reads keep working while a mine is in
/// progress, and vice versa - only same-kind requests contend.
///
/// Both are `tokio::sync::Mutex` rather than `std::sync::Mutex` because
/// each handler holds its lock across an `.await` (loading the chain
/// and/or persisting a block) - the whole check-then-act sequence must be
/// atomic from a concurrent request's point of view, not just the
/// in-memory part of it.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub mempool: Arc<Mutex<Mempool>>,
    pub mining_lock: Arc<tokio::sync::Mutex<()>>,
    pub submission_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        AppState {
            pool,
            mempool: Arc::new(Mutex::new(Mempool::new())),
            mining_lock: Arc::new(tokio::sync::Mutex::new(())),
            submission_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

/// The blockchain API routes. `main.rs` merges this with the unrelated
/// `/health` route.
pub fn router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/api/blocks", get(handlers::list_blocks))
        .route("/api/blocks/{index}", get(handlers::get_block))
        .route("/api/chain/valid", get(handlers::chain_valid))
        .route("/api/transactions", post(handlers::submit_transaction))
        .route(
            "/api/transactions/pending",
            get(handlers::pending_transactions),
        )
        .route("/api/mine", post(handlers::mine))
        .route(
            "/api/wallets/{address}/balance",
            get(handlers::wallet_balance),
        );

    // Only registered - not just guarded inside the handler - when enabled,
    // so a normal deployment has no route at all for this path (a plain
    // 404, indistinguishable from any other unknown path).
    if crate::config::dev_funding_enabled() {
        router = router.route("/api/dev/fund", post(dev::fund));
    }

    router.with_state(state)
}
