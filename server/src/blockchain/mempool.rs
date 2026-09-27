use super::{Chain, Transaction};

/// Why `Mempool::try_add_transaction` refused a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Failed `Transaction::validation_error` - bad signature, bad
    /// structural fields, etc. Carries that method's reason string.
    Invalid(&'static str),
    /// `amount` exceeds the sender's confirmed balance minus whatever they
    /// already have pending in this mempool.
    InsufficientBalance,
    /// A transaction with the same id is already pending.
    Duplicate,
}

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

    /// Admits a transaction, checking (in order): structural/signature
    /// validity, whether it's already pending, and whether the sender can
    /// actually afford it right now.
    ///
    /// "Afford it right now" means against `chain`'s confirmed balance
    /// *minus* whatever else this sender already has pending - without
    /// that subtraction, two transactions each individually affordable
    /// against the confirmed balance could together overspend it (e.g.
    /// balance 100, two pending transactions of 70 each: both look
    /// affordable against the confirmed balance alone, but not together).
    ///
    /// This is a safety net independent of the API layer, which also
    /// validates before ever calling this - the mempool shouldn't have to
    /// trust every caller to have checked first.
    pub fn try_add_transaction(&mut self, transaction: Transaction, chain: &Chain) -> Result<(), RejectionReason> {
        if let Some(reason) = transaction.validation_error() {
            return Err(RejectionReason::Invalid(reason));
        }

        if self.transactions.iter().any(|t| t.id == transaction.id) {
            return Err(RejectionReason::Duplicate);
        }

        if transaction.amount > self.available_balance(chain, &transaction.sender) {
            return Err(RejectionReason::InsufficientBalance);
        }

        self.transactions.push(transaction);
        Ok(())
    }

    /// `address`'s confirmed balance minus whatever it already has pending
    /// in this mempool - what it can actually still afford to spend.
    pub fn available_balance(&self, chain: &Chain, address: &str) -> i64 {
        chain.balance_of(address) - self.pending_amount_from(address)
    }

    fn pending_amount_from(&self, sender: &str) -> i64 {
        self.transactions.iter().filter(|t| t.sender == sender).map(|t| t.amount).sum()
    }

    /// All currently pending transactions, in the order they were added.
    pub fn pending_transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    /// Removes exactly the given transactions (matched by id) from the
    /// pending list - used after they've been durably persisted in a mined
    /// block. Only removes what's asked for, so any transaction submitted
    /// after mining started (and not included in that block) stays
    /// pending.
    pub fn remove_transactions(&mut self, mined: &[Transaction]) {
        let mined_ids: std::collections::HashSet<&str> = mined.iter().map(|t| t.id.as_str()).collect();
        self.transactions.retain(|t| !mined_ids.contains(t.id.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::blockchain::Block;
    use crate::config::MINING_DIFFICULTY;
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

    /// A chain where `test_wallet()` has a confirmed balance of `amount`,
    /// credited by a throwaway "faucet" wallet. This project has no
    /// coinbase/reward mechanism (a deliberate scope decision - see the
    /// README), so this is the only way to give a test wallet spendable
    /// balance: mine a block
    /// directly, bypassing the mempool's own balance check entirely (the
    /// same way `insert_block` never validates - see its doc comment).
    fn funded_chain(amount: i64) -> Chain {
        let faucet = Wallet::generate();
        let mut chain = Chain::new();
        let credit = Transaction::signed_by(&faucet, test_wallet().address(), amount);
        let mined = Block::mine(&chain.blocks[0], vec![credit], MINING_DIFFICULTY);
        chain.blocks.push(mined);
        chain
    }

    #[test]
    fn pending_transactions_starts_empty() {
        let mempool = Mempool::new();

        assert!(mempool.pending_transactions().is_empty());
    }

    #[test]
    fn adding_a_transaction_makes_it_pending() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        assert_eq!(mempool.try_add_transaction(tx(10), &chain), Ok(()));

        assert_eq!(mempool.pending_transactions().to_vec(), vec![tx(10)]);
    }

    #[test]
    fn duplicate_transaction_is_not_added_twice() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        assert_eq!(mempool.try_add_transaction(tx(10), &chain), Ok(()));
        assert_eq!(
            mempool.try_add_transaction(tx(10), &chain),
            Err(RejectionReason::Duplicate)
        );

        assert_eq!(mempool.pending_transactions().len(), 1);
    }

    #[test]
    fn transactions_with_different_amounts_are_not_duplicates() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        assert_eq!(mempool.try_add_transaction(tx(10), &chain), Ok(()));
        assert_eq!(mempool.try_add_transaction(tx(20), &chain), Ok(()));

        assert_eq!(mempool.pending_transactions().len(), 2);
    }

    #[test]
    fn invalid_transaction_is_not_added() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);
        let mut invalid = tx(10);
        invalid.amount = 0;

        assert!(matches!(
            mempool.try_add_transaction(invalid, &chain),
            Err(RejectionReason::Invalid(_))
        ));
        assert!(mempool.pending_transactions().is_empty());
    }

    #[test]
    fn remove_transactions_only_removes_the_given_ones() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);
        mempool.try_add_transaction(tx(10), &chain).unwrap();
        mempool.try_add_transaction(tx(20), &chain).unwrap();

        // Only "clear" the amount-10 transaction, as if it were the one
        // just mined into a block.
        mempool.remove_transactions(&[tx(10)]);

        let remaining = mempool.pending_transactions();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].amount, 20);
    }

    #[test]
    fn unconfirmed_mempool_transaction_does_not_affect_confirmed_balance() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        mempool.try_add_transaction(tx(40), &chain).unwrap();

        // Still pending, not mined - the chain itself is untouched.
        assert_eq!(chain.balance_of(&test_wallet().address()), 100);
    }

    #[test]
    fn transaction_exceeding_confirmed_balance_is_rejected() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        assert_eq!(
            mempool.try_add_transaction(tx(150), &chain),
            Err(RejectionReason::InsufficientBalance)
        );
        assert!(mempool.pending_transactions().is_empty());
    }

    #[test]
    fn pending_transactions_reduce_spendable_balance() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        mempool.try_add_transaction(tx(70), &chain).unwrap();

        assert_eq!(mempool.available_balance(&chain, &test_wallet().address()), 30);
    }

    #[test]
    fn multiple_pending_transactions_cannot_overspend_confirmed_balance() {
        let mut mempool = Mempool::new();
        let chain = funded_chain(100);

        // balance = 100, first pending tx = 70 - fine on its own.
        assert_eq!(mempool.try_add_transaction(tx(70), &chain), Ok(()));

        // Only 30 left after the first pending tx - 50 is not affordable,
        // even though 50 < 100 (the confirmed balance alone).
        assert_eq!(
            mempool.try_add_transaction(tx(50), &chain),
            Err(RejectionReason::InsufficientBalance)
        );

        assert_eq!(mempool.pending_transactions().len(), 1);
    }
}
