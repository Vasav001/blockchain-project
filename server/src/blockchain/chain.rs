use super::{current_timestamp, Block};
use crate::config::MINING_DIFFICULTY;

/// An ordered, append-only list of blocks starting from the genesis block.
#[derive(Debug)]
pub struct Chain {
    pub blocks: Vec<Block>,
}

impl Chain {
    /// Starts a new chain containing only the genesis block.
    ///
    /// Not yet called from `main` - the running app currently only reads
    /// chains back via `from_blocks`. This will be used once mining starts
    /// minting chains from scratch, in a later phase.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Chain {
            blocks: vec![Block::genesis()],
        }
    }

    /// Appends a new block carrying `data`, linked to the current last
    /// block, with `nonce = 0` (i.e. no mining/Proof-of-Work).
    ///
    /// The index and previous-hash are derived from the current last block,
    /// so there's exactly one place (`Block::new`) that builds a `Block`.
    ///
    /// Not called from `main` - real blocks are only ever produced by
    /// `Block::mine`. This stays around for tests that only care about
    /// index/previous-hash linkage, not full chain validity (an unmined
    /// block will essentially never satisfy `MINING_DIFFICULTY`).
    #[allow(dead_code)]
    pub fn add_block(&mut self, data: String) {
        let previous = self
            .blocks
            .last()
            .expect("chain always contains at least the genesis block");

        let block = Block::new(
            previous.index + 1,
            current_timestamp(),
            data,
            previous.hash.clone(),
            Vec::new(),
            0,
        );
        self.blocks.push(block);
    }

    /// Rebuilds a chain from blocks already loaded from storage. `blocks`
    /// is expected to already be in chain order (ascending index); this
    /// does not itself validate the chain - call `is_valid()` after.
    pub fn from_blocks(blocks: Vec<Block>) -> Self {
        Chain { blocks }
    }

    /// Verifies the whole chain: every block's stored hash must match its
    /// recomputed hash, indexes must be sequential from zero, each block's
    /// `previous_hash` must match its predecessor's `hash`, the first block
    /// must be exactly the expected genesis block, and every block after
    /// genesis must satisfy the configured mining difficulty (genesis is
    /// exempt - it's a fixed, deterministic special case, never mined).
    pub fn is_valid(&self) -> bool {
        for (i, block) in self.blocks.iter().enumerate() {
            if !block.hash_matches() {
                return false;
            }

            if i == 0 {
                if *block != Block::genesis() {
                    return false;
                }
                continue;
            }

            let previous = &self.blocks[i - 1];
            if block.index != previous.index + 1 || block.previous_hash != previous.hash {
                return false;
            }

            if !block.satisfies_difficulty(MINING_DIFFICULTY) {
                return false;
            }
        }

        true
    }

    /// `address`'s confirmed balance: replays every transaction in every
    /// block, in order - each one decreases the sender's running total by
    /// `amount` and increases the recipient's by `amount`. A wallet that's
    /// never appeared in a confirmed transaction has balance `0`.
    ///
    /// There's no mining reward and no genesis allocation in this phase
    /// (explicitly out of scope - see the project's phase history), so
    /// every wallet starts at zero and can only ever have what it's
    /// received. This is a pure replay with no validation of its own: it
    /// doesn't re-check signatures or re-enforce "sender must have had
    /// enough balance at the time" - that's `Mempool::try_add_transaction`'s
    /// job, applied once, when a transaction is first admitted. Balance
    /// isn't part of `is_valid()` either, by the same reasoning `Chain`
    /// already applies to transaction signatures (see `is_valid`'s doc
    /// comment in earlier phases): confirmed chain integrity (hashes,
    /// links, PoW) and economic admission policy are different concerns.
    pub fn balance_of(&self, address: &str) -> i64 {
        let mut balance: i64 = 0;

        for block in &self.blocks {
            for transaction in &block.transactions {
                if transaction.sender == address {
                    balance -= transaction.amount;
                }
                if transaction.recipient == address {
                    balance += transaction.amount;
                }
            }
        }

        balance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::Transaction;
    use crate::wallet::Wallet;

    /// Mines a real block on top of `chain`'s current last block and
    /// appends it - the test equivalent of `add_block`, but one that
    /// actually satisfies Proof-of-Work, for tests that need `is_valid()`
    /// to be true.
    fn mine_next_block(chain: &mut Chain, transactions: Vec<Transaction>) {
        let mined = Block::mine(chain.blocks.last().unwrap(), transactions, MINING_DIFFICULTY);
        chain.blocks.push(mined);
    }

    #[test]
    fn new_chain_contains_exactly_one_genesis_block() {
        let chain = Chain::new();

        assert_eq!(chain.blocks.len(), 1);
        assert_eq!(chain.blocks[0], Block::genesis());
    }

    #[test]
    fn from_blocks_rebuilds_a_valid_chain() {
        let mut original = Chain::new();
        mine_next_block(&mut original, Vec::new());

        let rebuilt = Chain::from_blocks(original.blocks);

        assert!(rebuilt.is_valid());
    }

    #[test]
    fn adding_a_block_increases_the_chain_length() {
        let mut chain = Chain::new();

        chain.add_block("first".to_string());

        assert_eq!(chain.blocks.len(), 2);
    }

    #[test]
    fn new_block_points_to_the_previous_blocks_hash() {
        let mut chain = Chain::new();
        let genesis_hash = chain.blocks[0].hash.clone();

        chain.add_block("first".to_string());

        assert_eq!(chain.blocks[1].previous_hash, genesis_hash);
    }

    #[test]
    fn block_indexes_are_sequential() {
        let mut chain = Chain::new();

        chain.add_block("first".to_string());
        chain.add_block("second".to_string());
        chain.add_block("third".to_string());

        let indexes: Vec<u64> = chain.blocks.iter().map(|b| b.index).collect();
        assert_eq!(indexes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_mined_chain_passes_is_valid() {
        let mut chain = Chain::new();
        mine_next_block(&mut chain, Vec::new());
        mine_next_block(&mut chain, Vec::new());

        assert!(chain.is_valid());
    }

    #[test]
    fn an_unmined_block_fails_proof_of_work_validation() {
        let mut chain = Chain::new();
        // `add_block` sets nonce = 0 and never searches for a satisfying
        // hash - vanishingly unlikely to meet the real difficulty.
        chain.add_block("first".to_string());

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_block_data_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        mine_next_block(&mut chain, Vec::new());
        assert!(chain.is_valid());

        chain.blocks[1].data = "tampered".to_string();

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_a_stored_block_hash_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        mine_next_block(&mut chain, Vec::new());
        assert!(chain.is_valid());

        chain.blocks[1].hash = "0".repeat(64);

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_previous_hash_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        mine_next_block(&mut chain, Vec::new());
        mine_next_block(&mut chain, Vec::new());
        assert!(chain.is_valid());

        chain.blocks[2].previous_hash = "1".repeat(64);

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_a_block_index_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        mine_next_block(&mut chain, Vec::new());
        assert!(chain.is_valid());

        chain.blocks[1].index = 5;

        assert!(!chain.is_valid());
    }

    #[test]
    fn mined_chain_with_signed_transactions_is_valid() {
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        let mut chain = Chain::new();
        mine_next_block(&mut chain, vec![tx]);

        assert!(chain.is_valid());
    }

    #[test]
    fn empty_chain_gives_every_address_zero_balance() {
        let chain = Chain::new();

        assert_eq!(chain.balance_of("anyone"), 0);
    }

    #[test]
    fn receiving_a_transaction_increases_balance() {
        let faucet = Wallet::generate();
        let alice = Wallet::generate();
        let credit = Transaction::signed_by(&faucet, alice.address(), 100);

        let mut chain = Chain::new();
        mine_next_block(&mut chain, vec![credit]);

        assert_eq!(chain.balance_of(&alice.address()), 100);
    }

    #[test]
    fn sending_a_transaction_decreases_balance() {
        let faucet = Wallet::generate();
        let alice = Wallet::generate();
        let bob = Wallet::generate();

        let mut chain = Chain::new();
        mine_next_block(&mut chain, vec![Transaction::signed_by(&faucet, alice.address(), 100)]);
        mine_next_block(&mut chain, vec![Transaction::signed_by(&alice, bob.address(), 30)]);

        assert_eq!(chain.balance_of(&alice.address()), 70);
        assert_eq!(chain.balance_of(&bob.address()), 30);
    }

    #[test]
    fn multiple_transactions_are_replayed_in_order() {
        let faucet = Wallet::generate();
        let alice = Wallet::generate();
        let bob = Wallet::generate();

        let mut chain = Chain::new();
        mine_next_block(&mut chain, vec![Transaction::signed_by(&faucet, alice.address(), 100)]);
        mine_next_block(
            &mut chain,
            vec![
                Transaction::signed_by(&alice, bob.address(), 20),
                Transaction::signed_by(&alice, bob.address(), 15),
            ],
        );
        mine_next_block(&mut chain, vec![Transaction::signed_by(&bob, alice.address(), 5)]);

        assert_eq!(chain.balance_of(&alice.address()), 100 - 20 - 15 + 5);
        assert_eq!(chain.balance_of(&bob.address()), 20 + 15 - 5);
    }

    #[test]
    fn different_wallets_have_independent_balances() {
        let faucet = Wallet::generate();
        let alice = Wallet::generate();
        let bob = Wallet::generate();

        let mut chain = Chain::new();
        mine_next_block(&mut chain, vec![Transaction::signed_by(&faucet, alice.address(), 40)]);
        mine_next_block(&mut chain, vec![Transaction::signed_by(&faucet, bob.address(), 15)]);

        assert_eq!(chain.balance_of(&alice.address()), 40);
        assert_eq!(chain.balance_of(&bob.address()), 15);
    }
}
