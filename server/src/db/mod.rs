mod block_repository;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

pub use block_repository::{find_block, has_blocks, insert_block, latest_block, load_all_blocks};

/// Each migration's fixed version number, name, and SQL, embedded into the
/// binary at compile time so the app doesn't depend on a filesystem path.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        1,
        "create_blocks_table",
        include_str!("../../migrations/0001_create_blocks_table.sql"),
    ),
    (
        2,
        "create_block_transactions_table",
        include_str!("../../migrations/0002_create_block_transactions_table.sql"),
    ),
    (
        3,
        "add_sender_public_key_to_block_transactions",
        include_str!("../../migrations/0003_add_sender_public_key_to_block_transactions.sql"),
    ),
    (
        4,
        "add_signature_to_block_transactions",
        include_str!("../../migrations/0004_add_signature_to_block_transactions.sql"),
    ),
    (
        5,
        "add_nonce_to_blocks",
        include_str!("../../migrations/0005_add_nonce_to_blocks.sql"),
    ),
];

/// Reads `DATABASE_URL` if set, otherwise falls back to the project's
/// default SQLite file under `server/data/`.
pub fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/blockchain.db".to_string())
}

/// Opens (creating the file if needed) the SQLite database at `database_url`
/// and applies any migrations that haven't run yet.
pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // SQLite ignores foreign key constraints unless a connection turns them
    // on explicitly - without this, `block_transactions.block_index`
    // referencing `blocks` would be declarative only, never enforced.
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

/// Runs any entries in `MIGRATIONS` not yet recorded in `schema_migrations`,
/// in order. Hand-rolled instead of `sqlx::migrate!`, and using only
/// runtime queries, per the project's earlier architectural decisions.
async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    for (version, name, sql) in MIGRATIONS.iter().copied() {
        let already_applied: Option<i64> =
            sqlx::query_scalar("SELECT version FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_optional(pool)
                .await?;

        if already_applied.is_some() {
            continue;
        }

        sqlx::query(sql).execute(pool).await?;
        sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
            .bind(version)
            .bind(name)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Opens an isolated in-memory database with migrations applied, for tests.
///
/// `max_connections(1)` keeps every query on the same connection: SQLite's
/// `:memory:` database only lives as long as the connection that created it,
/// so a pool handing out a second connection would see an empty database.
#[cfg(test)]
pub(crate) async fn test_pool() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("in-memory database URL is always valid")
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open in-memory test database");

    run_migrations(&pool)
        .await
        .expect("failed to run migrations on test database");

    pool
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn migrations_apply_cleanly_to_a_fresh_database() {
        let pool = test_pool().await;

        let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(applied, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn migrations_upgrade_an_existing_pre_phase_7_database() {
        // Simulate a database left behind by the Phase 6 version of this
        // app: only migrations 1-2 applied, so `block_transactions` has no
        // `sender_public_key`/`signature` columns yet, and it already holds
        // a row written before those columns existed.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (version, name, sql) in MIGRATIONS.iter().copied().take(2) {
            sqlx::query(sql).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
                .bind(version)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        sqlx::query(
            "INSERT INTO blocks (block_index, timestamp, data, previous_hash, hash)
             VALUES (0, 0, 'genesis block', ?, 'somehash')",
        )
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO block_transactions
                (block_index, transaction_index, transaction_id, sender, recipient, amount)
             VALUES (0, 0, 'txid', 'alice', 'bob', 10)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // This is the actual upgrade: running the full migration set
        // against a database that's already partway through it.
        run_migrations(&pool).await.unwrap();

        let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(applied, vec![1, 2, 3, 4, 5]);

        // The pre-existing row should have picked up the new columns with
        // the migrations' declared defaults, not been dropped or corrupted.
        let (sender_public_key, signature): (String, String) = sqlx::query_as(
            "SELECT sender_public_key, signature FROM block_transactions
             WHERE block_index = 0 AND transaction_index = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(sender_public_key, "");
        assert_eq!(signature, "");
    }

    #[tokio::test]
    async fn migrations_upgrade_an_existing_phase_7_database() {
        // Simulate a database left behind by the Phase 7 version of this
        // app: migrations 1-4 applied (full transaction/signature schema),
        // but no `nonce` column on `blocks` yet, and it already holds a
        // genesis row written before that column existed.
        //
        // Decision (Phase 8): old, non-mined blocks are simply read back
        // with `nonce = 0` via this column's `DEFAULT 0` - the same
        // approach already used for `sender_public_key`/`signature` in
        // Phase 7. This does *not* retroactively make an old block's
        // stored hash "correct" under the new nonce-including hash
        // formula (see `Block::calculate_hash`); in practice the only
        // block ever persisted before mining existed is genesis, and a
        // fresh `data/blockchain.db` (gitignored, disposable dev state)
        // is the practical remedy if `/api/chain/valid` ever reports an
        // old database as invalid after this upgrade.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (version, name, sql) in MIGRATIONS.iter().copied().take(4) {
            sqlx::query(sql).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
                .bind(version)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        sqlx::query(
            "INSERT INTO blocks (block_index, timestamp, data, previous_hash, hash)
             VALUES (0, 0, 'genesis block', ?, 'somehash')",
        )
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

        run_migrations(&pool).await.unwrap();

        let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(applied, vec![1, 2, 3, 4, 5]);

        let nonce: i64 = sqlx::query_scalar("SELECT nonce FROM blocks WHERE block_index = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(nonce, 0);

        // The pre-existing block is still readable through the repository
        // layer, not just raw SQL.
        let loaded = crate::db::find_block(&pool, 0).await.unwrap().unwrap();
        assert_eq!(loaded.nonce, 0);
    }
}
