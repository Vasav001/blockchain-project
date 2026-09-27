use super::Transaction;

/// An in-memory holding area for transactions that haven't been mined into
/// a block yet. This is application state, not database state - it isn't
/// persisted, and is expected to start empty again after a restart.
#[derive(Debug, Default)]
pub struct Mempool {
    transactions: Vec<Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Mempool::default()
    }

    /// Adds a transaction, unless it fails validation (including signature
    /// verification) or one with the same id is already pending. A
    /// transaction's id is derived entirely from its signed content, so an
    /// identical id means an identical, already-verified transaction -
    /// there's nothing new to add.
    ///
    /// This check is a safety net independent of the API layer, which also
    /// validates before ever calling this - the mempool shouldn't have to
    /// trust every caller to have checked first.
    ///
    /// Returns `true` if it was added, `false` otherwise.
    pub fn add_transaction(&mut self, transaction: Transaction) -> bool {
        if !transaction.is_valid() {
            return false;
        }

        if self.transactions.iter().any(|t| t.id == transaction.id) {
            return false;
        }

        self.transactions.push(transaction);
        true
    }

    /// All currently pending transactions, in the order they were added.
    pub fn pending_transactions(&self) -> &[Transaction] {
        &self.transactions
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::wallet::Wallet;

    /// A single shared wallet for these tests. Ed25519 signing is
    /// deterministic, so two `tx(10)` calls sharing a wallet produce
    /// identical ids - required for the duplicate-detection tests below to
    /// mean anything.
    fn test_wallet() -> &'static Wallet {
        static WALLET: OnceLock<Wallet> = OnceLock::new();
        WALLET.get_or_init(Wallet::generate)
    }

    fn tx(amount: i64) -> Transaction {
        Transaction::signed_by(test_wallet(), "bob".to_string(), amount)
    }

    #[test]
    fn pending_transactions_starts_empty() {
        let mempool = Mempool::new();

        assert!(mempool.pending_transactions().is_empty());
    }

    #[test]
    fn adding_a_transaction_makes_it_pending() {
        let mut mempool = Mempool::new();

        assert!(mempool.add_transaction(tx(10)));

        assert_eq!(mempool.pending_transactions().to_vec(), vec![tx(10)]);
    }

    #[test]
    fn duplicate_transaction_is_not_added_twice() {
        let mut mempool = Mempool::new();

        assert!(mempool.add_transaction(tx(10)));
        assert!(!mempool.add_transaction(tx(10)));

        assert_eq!(mempool.pending_transactions().len(), 1);
    }

    #[test]
    fn transactions_with_different_amounts_are_not_duplicates() {
        let mut mempool = Mempool::new();

        assert!(mempool.add_transaction(tx(10)));
        assert!(mempool.add_transaction(tx(20)));

        assert_eq!(mempool.pending_transactions().len(), 2);
    }

    #[test]
    fn invalid_transaction_is_not_added() {
        let mut mempool = Mempool::new();
        let mut invalid = tx(10);
        invalid.amount = 0;

        assert!(!mempool.add_transaction(invalid));
        assert!(mempool.pending_transactions().is_empty());
    }
}
