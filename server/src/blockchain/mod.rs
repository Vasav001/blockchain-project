mod block;
mod chain;
mod mempool;
mod transaction;

use std::time::{SystemTime, UNIX_EPOCH};

pub use block::Block;
pub use chain::Chain;
pub use mempool::{Mempool, RejectionReason};
pub use transaction::Transaction;

/// Current Unix time in seconds. Shared by every place that builds a
/// non-genesis block (`Chain::add_block`, `Block::mine`) - genesis alone
/// uses a fixed timestamp instead, so it stays reproducible.
///
/// Private, but still reachable from sibling submodules (`block`, `chain`)
/// via `super::current_timestamp` - Rust visibility extends to a module's
/// descendants by default, no `pub` needed for that.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before UNIX_EPOCH")
        .as_secs()
}
