use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::blockchain::{Block, Transaction};

/// The row shape stored in the `blocks` table. Kept separate from `Block` so
/// the blockchain domain type never has to know about SQLx or SQLite.
struct BlockRow {
    block_index: i64,
    timestamp: i64,
    data: String,
    previous_hash: String,
    hash: String,
    nonce: i64,
}

impl sqlx::FromRow<'_, SqliteRow> for BlockRow {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(BlockRow {
            block_index: row.try_get("block_index")?,
            timestamp: row.try_get("timestamp")?,
            data: row.try_get("data")?,
            previous_hash: row.try_get("previous_hash")?,
            hash: row.try_get("hash")?,
            nonce: row.try_get("nonce")?,
        })
    }
}

/// The row shape stored in the `block_transactions` table (one row per
/// transaction, per block). Kept separate from `Transaction` for the same
/// reason as `BlockRow`.
struct TransactionRow {
    transaction_id: String,
    sender: String,
    recipient: String,
    amount: i64,
    sender_public_key: String,
    signature: String,
}

impl sqlx::FromRow<'_, SqliteRow> for TransactionRow {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(TransactionRow {
            transaction_id: row.try_get("transaction_id")?,
            sender: row.try_get("sender")?,
            recipient: row.try_get("recipient")?,
            amount: row.try_get("amount")?,
            sender_public_key: row.try_get("sender_public_key")?,
            signature: row.try_get("signature")?,
        })
    }
}

impl From<TransactionRow> for Transaction {
    fn from(row: TransactionRow) -> Self {
        // Trusts the stored id rather than recomputing it - the same "load
        // as persisted, validate separately" convention `BlockRow -> Block`
        // already follows for `hash`. (The two would agree anyway, since a
        // transaction's id is a pure function of its other fields -
        // recomputing would just be redundant work.)
        Transaction {
            sender: row.sender,
            recipient: row.recipient,
            amount: row.amount,
            sender_public_key: row.sender_public_key,
            signature: row.signature,
            id: row.transaction_id,
        }
    }
}

/// Combines a `BlockRow` with its already-loaded, already-ordered
/// transactions into a `Block`.
fn assemble_block(row: BlockRow, transactions: Vec<Transaction>) -> Block {
    Block {
        // SQLite's INTEGER is signed 64-bit; this app never produces a
        // negative index, timestamp, or nonce, so the cast back to u64 is
        // safe.
        index: row.block_index as u64,
        timestamp: row.timestamp as u64,
        data: row.data,
        previous_hash: row.previous_hash,
        transactions,
        nonce: row.nonce as u64,
        hash: row.hash,
    }
}

/// Loads a block's transactions in their original order.
async fn load_transactions(pool: &SqlitePool, block_index: u64) -> Result<Vec<Transaction>, sqlx::Error> {
    let rows: Vec<TransactionRow> = sqlx::query_as(
        "SELECT transaction_id, sender, recipient, amount, sender_public_key, signature
         FROM block_transactions
         WHERE block_index = ?
         ORDER BY transaction_index ASC",
    )
    .bind(block_index as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Transaction::from).collect())
}

/// Persists a block and all of its transactions together, in one SQLite
/// transaction: either both the block row and every transaction row are
/// written, or (on error) none of them are - a block should never end up
/// stored without the transactions it was hashed with.
///
/// Callers are responsible for only passing blocks that belong in the
/// chain; this function does no validation.
pub async fn insert_block(pool: &SqlitePool, block: &Block) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO blocks (block_index, timestamp, data, previous_hash, hash, nonce)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(block.index as i64)
    .bind(block.timestamp as i64)
    .bind(&block.data)
    .bind(&block.previous_hash)
    .bind(&block.hash)
    .bind(block.nonce as i64)
    .execute(&mut *tx)
    .await?;

    for (transaction_index, transaction) in block.transactions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO block_transactions
                (block_index, transaction_index, transaction_id, sender, recipient, amount,
                 sender_public_key, signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(block.index as i64)
        .bind(transaction_index as i64)
        .bind(&transaction.id)
        .bind(&transaction.sender)
        .bind(&transaction.recipient)
        .bind(transaction.amount)
        .bind(&transaction.sender_public_key)
        .bind(&transaction.signature)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

const BLOCK_COLUMNS: &str = "block_index, timestamp, data, previous_hash, hash, nonce";

/// Loads every stored block with its transactions, ordered by index (i.e.
/// chain order).
///
/// This issues one query for the blocks plus one query per block for its
/// transactions. Simple, and fine at this project's scale; a single JOIN
/// query would be the next step if block counts ever got large enough for
/// N+1 queries to matter.
pub async fn load_all_blocks(pool: &SqlitePool) -> Result<Vec<Block>, sqlx::Error> {
    let rows: Vec<BlockRow> = sqlx::query_as(&format!(
        "SELECT {BLOCK_COLUMNS} FROM blocks ORDER BY block_index ASC"
    ))
    .fetch_all(pool)
    .await?;

    let mut blocks = Vec::with_capacity(rows.len());
    for row in rows {
        let transactions = load_transactions(pool, row.block_index as u64).await?;
        blocks.push(assemble_block(row, transactions));
    }

    Ok(blocks)
}

/// Cheap existence check, useful for deciding whether to seed a fresh chain
/// or load an existing one from storage.
pub async fn has_blocks(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocks")
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

/// Loads a single block, with its transactions, by its index - if one
/// exists.
pub async fn find_block(pool: &SqlitePool, index: u64) -> Result<Option<Block>, sqlx::Error> {
    let row: Option<BlockRow> = sqlx::query_as(&format!(
        "SELECT {BLOCK_COLUMNS} FROM blocks WHERE block_index = ?"
    ))
    .bind(index as i64)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let transactions = load_transactions(pool, index).await?;

    Ok(Some(assemble_block(row, transactions)))
}

/// Loads the highest-index block, with its transactions - the block
/// mining builds on top of. Returns `None` only if the `blocks` table is
/// empty, which shouldn't happen in practice since genesis is always
/// seeded at startup.
pub async fn latest_block(pool: &SqlitePool) -> Result<Option<Block>, sqlx::Error> {
    let row: Option<BlockRow> = sqlx::query_as(&format!(
        "SELECT {BLOCK_COLUMNS} FROM blocks ORDER BY block_index DESC LIMIT 1"
    ))
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let transactions = load_transactions(pool, row.block_index as u64).await?;

    Ok(Some(assemble_block(row, transactions)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::Chain;
    use crate::config::MINING_DIFFICULTY;
    use crate::db::test_pool;
    use crate::wallet::Wallet;

    #[tokio::test]
    async fn inserting_and_loading_a_block_round_trips() {
        let pool = test_pool().await;
        let genesis = Block::genesis();

        insert_block(&pool, &genesis).await.unwrap();
        let loaded = load_all_blocks(&pool).await.unwrap();

        assert_eq!(loaded, vec![genesis]);
    }

    #[tokio::test]
    async fn blocks_load_in_chain_order() {
        let pool = test_pool().await;
        let mut chain = Chain::new();
        chain.add_block("first".to_string());
        chain.add_block("second".to_string());

        for block in &chain.blocks {
            insert_block(&pool, block).await.unwrap();
        }

        let loaded = load_all_blocks(&pool).await.unwrap();
        let indexes: Vec<u64> = loaded.iter().map(|b| b.index).collect();

        assert_eq!(indexes, vec![0, 1, 2]);
        assert_eq!(loaded, chain.blocks);
    }

    #[tokio::test]
    async fn has_blocks_reflects_table_contents() {
        let pool = test_pool().await;

        assert!(!has_blocks(&pool).await.unwrap());

        insert_block(&pool, &Block::genesis()).await.unwrap();

        assert!(has_blocks(&pool).await.unwrap());
    }

    #[tokio::test]
    async fn find_block_returns_the_matching_block() {
        let pool = test_pool().await;
        let mut chain = Chain::new();
        chain.add_block("first".to_string());

        for block in &chain.blocks {
            insert_block(&pool, block).await.unwrap();
        }

        let found = find_block(&pool, 1).await.unwrap();

        assert_eq!(found, Some(chain.blocks.remove(1)));
    }

    #[tokio::test]
    async fn find_block_returns_none_when_missing() {
        let pool = test_pool().await;
        insert_block(&pool, &Block::genesis()).await.unwrap();

        let found = find_block(&pool, 99).await.unwrap();

        assert_eq!(found, None);
    }

    #[tokio::test]
    async fn latest_block_returns_the_highest_index_block() {
        let pool = test_pool().await;
        let mut chain = Chain::new();
        chain.add_block("first".to_string());
        chain.add_block("second".to_string());

        for block in &chain.blocks {
            insert_block(&pool, block).await.unwrap();
        }

        let latest = latest_block(&pool).await.unwrap().unwrap();

        assert_eq!(latest.index, 2);
    }

    #[tokio::test]
    async fn loaded_chain_passes_validation() {
        let pool = test_pool().await;
        let mut chain = Chain::new();
        let mined = Block::mine(&chain.blocks[0], Vec::new(), MINING_DIFFICULTY);
        chain.blocks.push(mined);

        for block in &chain.blocks {
            insert_block(&pool, block).await.unwrap();
        }

        let loaded = Chain::from_blocks(load_all_blocks(&pool).await.unwrap());

        assert!(loaded.is_valid());
    }

    fn block_with_transactions(previous: &Block, transactions: Vec<Transaction>) -> Block {
        Block::new(
            previous.index + 1,
            previous.timestamp + 1,
            "block with transactions".to_string(),
            previous.hash.clone(),
            transactions,
            0,
        )
    }

    #[tokio::test]
    async fn block_with_zero_transactions_round_trips() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let block = block_with_transactions(&genesis, Vec::new());

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &block).await.unwrap();

        let loaded = find_block(&pool, block.index).await.unwrap().unwrap();

        assert_eq!(loaded, block);
        assert!(loaded.transactions.is_empty());
    }

    #[tokio::test]
    async fn signed_transaction_round_trips() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let block = block_with_transactions(&genesis, vec![tx]);

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &block).await.unwrap();

        let loaded = find_block(&pool, block.index).await.unwrap().unwrap();

        assert_eq!(loaded, block);
        assert!(loaded.transactions[0].is_valid());
    }

    #[tokio::test]
    async fn multiple_signed_transactions_preserve_order() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let carol = Wallet::generate();
        let transactions = vec![
            Transaction::signed_by(&alice, bob.address(), 10),
            Transaction::signed_by(&bob, carol.address(), 5),
            Transaction::signed_by(&carol, alice.address(), 1),
        ];
        let block = block_with_transactions(&genesis, transactions.clone());

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &block).await.unwrap();

        let loaded = find_block(&pool, block.index).await.unwrap().unwrap();

        assert_eq!(loaded.transactions, transactions);
    }

    #[tokio::test]
    async fn loaded_signed_block_retains_the_same_hash() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let block = block_with_transactions(&genesis, vec![tx]);
        let original_hash = block.hash.clone();

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &block).await.unwrap();

        let loaded = find_block(&pool, block.index).await.unwrap().unwrap();

        assert_eq!(loaded.hash, original_hash);
        assert!(loaded.hash_matches());
    }

    #[tokio::test]
    async fn loaded_transaction_with_tampered_signature_is_still_rejected() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let wallet = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        // Tamper with the amount after signing, without re-signing - the
        // signature (and id) on disk no longer match the stored amount.
        tx.amount = 999;
        let block = block_with_transactions(&genesis, vec![tx]);

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &block).await.unwrap();

        let loaded = find_block(&pool, block.index).await.unwrap().unwrap();

        // The block's own hash is unaffected (it only folds in the
        // transaction's `id`, which was already stale before persisting),
        // so this confirms persistence doesn't launder an invalid
        // transaction into a valid one - `Transaction::is_valid` still
        // independently catches it after a full round trip.
        assert!(loaded.hash_matches());
        assert!(!loaded.transactions[0].is_valid());
    }

    #[tokio::test]
    async fn mined_block_survives_database_round_trip() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let mined = Block::mine(&genesis, vec![tx], MINING_DIFFICULTY);

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &mined).await.unwrap();

        let loaded = find_block(&pool, mined.index).await.unwrap().unwrap();

        assert_eq!(loaded, mined);
        assert!(loaded.satisfies_difficulty(MINING_DIFFICULTY));
    }

    #[tokio::test]
    async fn loaded_mined_chain_passes_is_valid() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let mined = Block::mine(&genesis, vec![tx], MINING_DIFFICULTY);

        insert_block(&pool, &genesis).await.unwrap();
        insert_block(&pool, &mined).await.unwrap();

        let loaded = Chain::from_blocks(load_all_blocks(&pool).await.unwrap());

        assert!(loaded.is_valid());
    }

    #[tokio::test]
    async fn loaded_chain_produces_the_same_balances_as_the_original() {
        let pool = test_pool().await;
        let genesis = Block::genesis();
        let faucet = Wallet::generate();
        let alice = Wallet::generate();
        let bob = Wallet::generate();

        let funded = Block::mine(&genesis, vec![Transaction::signed_by(&faucet, alice.address(), 100)], MINING_DIFFICULTY);
        let spent = Block::mine(&funded, vec![Transaction::signed_by(&alice, bob.address(), 30)], MINING_DIFFICULTY);

        let original = Chain::from_blocks(vec![genesis, funded, spent]);
        for block in &original.blocks {
            insert_block(&pool, block).await.unwrap();
        }

        let loaded = Chain::from_blocks(load_all_blocks(&pool).await.unwrap());

        assert_eq!(loaded.balance_of(&alice.address()), original.balance_of(&alice.address()));
        assert_eq!(loaded.balance_of(&bob.address()), original.balance_of(&bob.address()));
        assert_eq!(loaded.balance_of(&alice.address()), 70);
        assert_eq!(loaded.balance_of(&bob.address()), 30);
    }
}
