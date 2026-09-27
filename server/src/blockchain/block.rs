use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Transaction;

/// A single block in the chain.
///
/// `hash` is derived from the other fields (see `calculate_hash`) rather than
/// being set independently, so a block's hash always matches its content.
///
/// `Serialize` is derived so the API layer can return a `Block` as JSON
/// directly; this is still just `serde`, not a dependency on Axum or SQLx.
#[derive(Debug, PartialEq, Serialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: u64,
    pub data: String,
    pub previous_hash: String,
    pub transactions: Vec<Transaction>,
    pub hash: String,
}

impl Block {
    /// Builds a new block, computing its hash from the given fields.
    ///
    /// Nothing in this phase actually calls this with a non-empty
    /// `transactions` yet - that starts once mining exists - but the field
    /// is here now so a future mined block doesn't need a struct redesign.
    pub fn new(
        index: u64,
        timestamp: u64,
        data: String,
        previous_hash: String,
        transactions: Vec<Transaction>,
    ) -> Self {
        let hash = Self::calculate_hash(index, timestamp, &data, &previous_hash, &transactions);
        Block {
            index,
            timestamp,
            data,
            previous_hash,
            transactions,
            hash,
        }
    }

    /// The first block in the chain.
    ///
    /// Fields are fixed (not derived from the current time) so that every
    /// run produces byte-for-byte the same genesis block and hash.
    pub fn genesis() -> Self {
        Block::new(0, 0, "genesis block".to_string(), "0".repeat(64), Vec::new())
    }

    /// Recomputes the hash from this block's current fields and checks it
    /// against the stored `hash`. Used by `Chain::is_valid` to detect
    /// tampering with any field after the block was constructed.
    pub fn hash_matches(&self) -> bool {
        self.hash
            == Self::calculate_hash(
                self.index,
                self.timestamp,
                &self.data,
                &self.previous_hash,
                &self.transactions,
            )
    }

    /// SHA-256 hash of the block's content fields, hex-encoded.
    ///
    /// Fixed-width big-endian encoding is used for the integers so the byte
    /// layout fed to the hasher is unambiguous and reproducible. Each
    /// transaction contributes its own `id` (itself already a hash of that
    /// transaction's content) rather than re-hashing its raw fields here.
    fn calculate_hash(
        index: u64,
        timestamp: u64,
        data: &str,
        previous_hash: &str,
        transactions: &[Transaction],
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(index.to_be_bytes());
        hasher.update(timestamp.to_be_bytes());
        hasher.update(data.as_bytes());
        hasher.update(previous_hash.as_bytes());
        for transaction in transactions {
            hasher.update(transaction.id.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_block_has_expected_fields() {
        let genesis = Block::genesis();

        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.timestamp, 0);
        assert_eq!(genesis.data, "genesis block");
        assert_eq!(genesis.previous_hash, "0".repeat(64));
        assert!(genesis.transactions.is_empty());
    }

    #[test]
    fn genesis_hash_is_non_empty() {
        let genesis = Block::genesis();

        assert!(!genesis.hash.is_empty());
        // SHA-256 hex-encoded is always 64 hex characters.
        assert_eq!(genesis.hash.len(), 64);
    }

    #[test]
    fn identical_fields_produce_the_same_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new());
        let b = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new());

        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn changing_data_changes_the_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new());
        let b = Block::new(1, 1_000, "goodbye".to_string(), "abc".to_string(), Vec::new());

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn changing_previous_hash_changes_the_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new());
        let b = Block::new(1, 1_000, "hello".to_string(), "xyz".to_string(), Vec::new());

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn changing_transactions_changes_the_hash() {
        let wallet = crate::wallet::Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new());
        let b = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), vec![tx]);

        assert_ne!(a.hash, b.hash);
    }
}
