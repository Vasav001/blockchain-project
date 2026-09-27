use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{current_timestamp, Transaction};

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
    pub nonce: u64,
    pub hash: String,
}

impl Block {
    /// Builds a new block, computing its hash from the given fields.
    ///
    /// This does no mining - `nonce` is whatever the caller passes, and the
    /// resulting hash may or may not satisfy the configured difficulty.
    /// Use `Block::mine` to produce a block that actually satisfies
    /// Proof-of-Work.
    pub fn new(
        index: u64,
        timestamp: u64,
        data: String,
        previous_hash: String,
        transactions: Vec<Transaction>,
        nonce: u64,
    ) -> Self {
        let hash = Self::calculate_hash(index, timestamp, &data, &previous_hash, &transactions, nonce);
        Block {
            index,
            timestamp,
            data,
            previous_hash,
            transactions,
            nonce,
            hash,
        }
    }

    /// The first block in the chain.
    ///
    /// Fields are fixed (not derived from the current time, and `nonce` is
    /// always `0`) so that every run produces byte-for-byte the same
    /// genesis block and hash. Genesis is never mined - it's the one
    /// deterministic special case `Chain::is_valid` exempts from the
    /// Proof-of-Work check (see that method).
    pub fn genesis() -> Self {
        Block::new(0, 0, "genesis block".to_string(), "0".repeat(64), Vec::new(), 0)
    }

    /// Mines a new block on top of `previous`, containing `transactions`:
    /// repeatedly increments `nonce` and recomputes the hash until it has
    /// at least `difficulty` leading hex-zero characters.
    ///
    /// Filters out any invalid transaction first - the mempool already
    /// only ever holds valid ones (see `Mempool::try_add_transaction`), so
    /// in practice this never removes anything, but the mining boundary
    /// itself should never blindly trust its input.
    ///
    /// This is a plain, synchronous loop: no threads, no async worker, no
    /// abstraction beyond what's needed to search for a nonce.
    pub fn mine(previous: &Block, transactions: Vec<Transaction>, difficulty: usize) -> Self {
        let transactions: Vec<Transaction> = transactions.into_iter().filter(|t| t.is_valid()).collect();

        let index = previous.index + 1;
        let timestamp = current_timestamp();
        let data = "mined block".to_string();
        let previous_hash = previous.hash.clone();

        let mut nonce = 0u64;
        loop {
            let hash = Self::calculate_hash(index, timestamp, &data, &previous_hash, &transactions, nonce);
            if Self::hash_satisfies_difficulty(&hash, difficulty) {
                return Block {
                    index,
                    timestamp,
                    data,
                    previous_hash,
                    transactions,
                    nonce,
                    hash,
                };
            }
            nonce += 1;
        }
    }

    /// Whether this block's hash satisfies `difficulty` leading hex-zero
    /// characters - the Proof-of-Work check `Chain::is_valid` runs against
    /// every non-genesis block.
    pub fn satisfies_difficulty(&self, difficulty: usize) -> bool {
        Self::hash_satisfies_difficulty(&self.hash, difficulty)
    }

    fn hash_satisfies_difficulty(hash: &str, difficulty: usize) -> bool {
        hash.starts_with(&"0".repeat(difficulty))
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
                self.nonce,
            )
    }

    /// SHA-256 hash of the block's content fields, hex-encoded.
    ///
    /// Fixed-width big-endian encoding is used for the integers so the byte
    /// layout fed to the hasher is unambiguous and reproducible. Each
    /// transaction contributes its own `id` (itself already a hash of that
    /// transaction's content) rather than re-hashing its raw fields here.
    /// `nonce` is the field mining searches over: changing it is the only
    /// thing that changes the hash without changing what the block means.
    fn calculate_hash(
        index: u64,
        timestamp: u64,
        data: &str,
        previous_hash: &str,
        transactions: &[Transaction],
        nonce: u64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(index.to_be_bytes());
        hasher.update(timestamp.to_be_bytes());
        hasher.update(data.as_bytes());
        hasher.update(previous_hash.as_bytes());
        for transaction in transactions {
            hasher.update(transaction.id.as_bytes());
        }
        hasher.update(nonce.to_be_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;

    #[test]
    fn genesis_block_has_expected_fields() {
        let genesis = Block::genesis();

        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.timestamp, 0);
        assert_eq!(genesis.data, "genesis block");
        assert_eq!(genesis.previous_hash, "0".repeat(64));
        assert!(genesis.transactions.is_empty());
        assert_eq!(genesis.nonce, 0);
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
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);
        let b = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);

        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn changing_data_changes_the_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);
        let b = Block::new(1, 1_000, "goodbye".to_string(), "abc".to_string(), Vec::new(), 0);

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn changing_previous_hash_changes_the_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);
        let b = Block::new(1, 1_000, "hello".to_string(), "xyz".to_string(), Vec::new(), 0);

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn changing_transactions_changes_the_hash() {
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);
        let b = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), vec![tx], 0);

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn changing_nonce_changes_the_hash() {
        let a = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);
        let b = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 1);

        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn mined_block_satisfies_the_configured_difficulty() {
        let previous = Block::genesis();

        let mined = Block::mine(&previous, Vec::new(), 4);

        assert!(mined.satisfies_difficulty(4));
        assert!(mined.hash.starts_with("0000"));
    }

    #[test]
    fn block_with_an_unsatisfying_nonce_fails_the_difficulty_check() {
        // Vanishingly unlikely nonce=0 satisfies a real difficulty by luck;
        // this just documents that an unmined block generally won't.
        let block = Block::new(1, 1_000, "hello".to_string(), "abc".to_string(), Vec::new(), 0);

        assert!(!block.satisfies_difficulty(4));
    }

    #[test]
    fn mining_preserves_index_and_previous_hash() {
        let previous = Block::genesis();

        let mined = Block::mine(&previous, Vec::new(), 4);

        assert_eq!(mined.index, previous.index + 1);
        assert_eq!(mined.previous_hash, previous.hash);
    }

    #[test]
    fn mined_block_contains_the_given_transactions() {
        let previous = Block::genesis();
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        let mined = Block::mine(&previous, vec![tx.clone()], 4);

        assert_eq!(mined.transactions, vec![tx]);
    }

    #[test]
    fn mining_filters_out_invalid_transactions() {
        let previous = Block::genesis();
        let wallet = Wallet::generate();
        let mut invalid = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        invalid.amount = 0;

        let mined = Block::mine(&previous, vec![invalid], 4);

        assert!(mined.transactions.is_empty());
    }
}
