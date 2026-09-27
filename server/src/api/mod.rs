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
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub mempool: Arc<Mutex<Mempool>>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        AppState {
            pool,
            mempool: Arc::new(Mutex::new(Mempool::new())),
        }
    }
}

/// The blockchain API routes. `main.rs` merges this with the unrelated
/// `/health` route.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/blocks", get(handlers::list_blocks))
        .route("/api/blocks/{index}", get(handlers::get_block))
        .route("/api/chain/valid", get(handlers::chain_valid))
        .route("/api/transactions", post(handlers::submit_transaction))
        .route(
            "/api/transactions/pending",
            get(handlers::pending_transactions),
        )
        .with_state(state)
}
