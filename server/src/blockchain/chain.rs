use std::time::{SystemTime, UNIX_EPOCH};

use super::Block;

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

    /// Appends a new block carrying `data`, linked to the current last block.
    ///
    /// The index and previous-hash are derived from the current last block,
    /// so there's exactly one place (`Block::new`) that builds a `Block`.
    ///
    /// Not yet called from `main` - appending blocks starts with mining in
    /// a later phase.
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
    /// `previous_hash` must match its predecessor's `hash`, and the first
    /// block must be exactly the expected genesis block.
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
        }

        true
    }
}

#[allow(dead_code)]
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before UNIX_EPOCH")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_chain_contains_exactly_one_genesis_block() {
        let chain = Chain::new();

        assert_eq!(chain.blocks.len(), 1);
        assert_eq!(chain.blocks[0], Block::genesis());
    }

    #[test]
    fn from_blocks_rebuilds_a_valid_chain() {
        let mut original = Chain::new();
        original.add_block("first".to_string());

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
    fn a_valid_chain_passes_is_valid() {
        let mut chain = Chain::new();
        chain.add_block("first".to_string());
        chain.add_block("second".to_string());

        assert!(chain.is_valid());
    }

    #[test]
    fn modifying_block_data_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        chain.add_block("first".to_string());

        chain.blocks[1].data = "tampered".to_string();

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_a_stored_block_hash_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        chain.add_block("first".to_string());

        chain.blocks[1].hash = "0".repeat(64);

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_previous_hash_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        chain.add_block("first".to_string());
        chain.add_block("second".to_string());

        chain.blocks[2].previous_hash = "1".repeat(64);

        assert!(!chain.is_valid());
    }

    #[test]
    fn modifying_a_block_index_makes_the_chain_invalid() {
        let mut chain = Chain::new();
        chain.add_block("first".to_string());

        chain.blocks[1].index = 5;

        assert!(!chain.is_valid());
    }
}
