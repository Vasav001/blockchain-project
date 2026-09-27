use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::blockchain::{Block, Transaction};
use crate::config::MINING_DIFFICULTY;
use crate::db;
use crate::wallet::Wallet;

use super::{router, AppState};

/// Builds the JSON body a real client would POST to `/api/transactions`:
/// signs a transaction with `wallet` (reusing the same production signing
/// path as everywhere else) and serializes it. The server-side
/// `TransactionRequest` doesn't have an `id` field, so the extra `id` this
/// serializes is simply ignored on deserialization.
fn signed_transaction_request(wallet: &Wallet, recipient: &str, amount: i64) -> serde_json::Value {
    let tx = Transaction::signed_by(wallet, recipient.to_string(), amount);
    serde_json::to_value(&tx).unwrap()
}

/// A router backed by an isolated in-memory DB, seeded with two blocks -
/// genesis plus one properly mined empty block, so `/api/chain/valid`
/// reports `true` for it (an unmined block would almost never satisfy
/// `MINING_DIFFICULTY`). No wallet has any balance in this chain - use
/// `test_app_with_funded_wallets` for tests that submit a transaction.
async fn test_app_with_two_blocks() -> axum::Router {
    let pool = db::test_pool().await;

    let genesis = Block::genesis();
    db::insert_block(&pool, &genesis).await.unwrap();

    let second = Block::mine(&genesis, Vec::new(), MINING_DIFFICULTY);
    db::insert_block(&pool, &second).await.unwrap();

    router(AppState::new(pool))
}

/// A router backed by an isolated in-memory DB, seeded with genesis plus
/// one mined block per `(wallet, amount)` pair crediting that wallet from a
/// throwaway "faucet" wallet. There's no coinbase/reward mechanism in this
/// phase, so this is how tests get a wallet with spendable confirmed
/// balance (the same technique `Mempool`'s own tests use).
async fn test_app_with_funded_wallets(fundings: &[(&Wallet, i64)]) -> axum::Router {
    let (router, _) = test_app_with_funded_wallets_and_state(fundings).await;
    router
}

/// Same as `test_app_with_funded_wallets`, but also returns the `AppState`
/// directly - needed by tests that poke at the database or state
/// out-of-band (e.g. simulating a persistence failure).
async fn test_app_with_funded_wallets_and_state(fundings: &[(&Wallet, i64)]) -> (axum::Router, AppState) {
    let pool = db::test_pool().await;

    let mut previous = Block::genesis();
    db::insert_block(&pool, &previous).await.unwrap();

    let faucet = Wallet::generate();
    for (wallet, amount) in fundings {
        let credit = Transaction::signed_by(&faucet, wallet.address(), *amount);
        let block = Block::mine(&previous, vec![credit], MINING_DIFFICULTY);
        db::insert_block(&pool, &block).await.unwrap();
        previous = block;
    }

    let state = AppState::new(pool);
    (router(state.clone()), state)
}

/// Sends a GET request through the router in-process (no real TCP socket)
/// and returns the status code and parsed JSON body.
async fn get(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap();

    (status, json)
}

/// Sends a POST request with a JSON body through the router in-process and
/// returns the status code and parsed JSON body.
async fn post(app: axum::Router, uri: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap();

    (status, json)
}

#[tokio::test]
async fn list_blocks_returns_blocks_in_chain_order() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = get(app, "/api/blocks").await;

    assert_eq!(status, StatusCode::OK);
    let blocks = json.as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0]["index"], 0);
    assert_eq!(blocks[1]["index"], 1);
}

#[tokio::test]
async fn get_block_returns_the_matching_block() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = get(app, "/api/blocks/1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["index"], 1);
    assert_eq!(json["data"], "mined block");
}

#[tokio::test]
async fn get_block_returns_404_for_missing_index() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = get(app, "/api/blocks/99").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn chain_valid_returns_true_for_a_valid_chain() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = get(app, "/api/chain/valid").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["valid"], true);
}

#[tokio::test]
async fn valid_funded_transaction_is_accepted() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (status, json) = post(
        app,
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["sender"], wallet.address());
    assert_eq!(json["recipient"], "bob");
    assert_eq!(json["amount"], 10);
    assert!(json["id"].is_string());
}

#[tokio::test]
async fn unfunded_transaction_is_rejected() {
    // No funding block for this wallet - confirmed balance is 0.
    let wallet = Wallet::generate();
    let app = test_app_with_two_blocks().await;

    let (status, json) = post(
        app,
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "insufficient balance");
}

#[tokio::test]
async fn invalid_transaction_is_rejected() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let payload = signed_transaction_request(&wallet, "bob", 0);

    let (status, json) = post(app, "/api/transactions", payload).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn unsigned_transaction_is_rejected() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let mut payload = signed_transaction_request(&wallet, "bob", 10);
    payload["signature"] = serde_json::json!("00".repeat(64));

    let (status, json) = post(app, "/api/transactions", payload).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "invalid transaction: signature is invalid");
}

#[tokio::test]
async fn multiple_pending_transfers_cannot_overspend() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (first_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 70),
    )
    .await;
    assert_eq!(first_status, StatusCode::OK);

    // Only 30 left after the first pending transfer - 50 is not
    // affordable, even though 50 < 100 (the confirmed balance alone).
    let (second_status, json) = post(
        app,
        "/api/transactions",
        signed_transaction_request(&wallet, "carol", 50),
    )
    .await;

    assert_eq!(second_status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "insufficient balance");
}

#[tokio::test]
async fn pending_transactions_reflects_submitted_transactions() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (submit_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;
    assert_eq!(submit_status, StatusCode::OK);

    let (status, json) = get(app, "/api/transactions/pending").await;

    assert_eq!(status, StatusCode::OK);
    let pending = json.as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["sender"], wallet.address());
}

#[tokio::test]
async fn pending_transaction_alone_does_not_affect_balance() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (submit_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 40),
    )
    .await;
    assert_eq!(submit_status, StatusCode::OK);

    let (_, sender_balance) = get(app.clone(), &format!("/api/wallets/{}/balance", wallet.address())).await;
    assert_eq!(sender_balance["balance"], 100);

    let (_, recipient_balance) = get(app, "/api/wallets/bob/balance").await;
    assert_eq!(recipient_balance["balance"], 0);
}

#[tokio::test]
async fn mine_with_no_pending_transactions_returns_a_clear_error() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = post(app, "/api/mine", serde_json::json!({})).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "no pending transactions to mine");
}

#[tokio::test]
async fn mine_persists_a_block_containing_the_pending_transaction() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (submit_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;
    assert_eq!(submit_status, StatusCode::OK);

    let (mine_status, mined) = post(app.clone(), "/api/mine", serde_json::json!({})).await;

    assert_eq!(mine_status, StatusCode::OK);
    assert_eq!(mined["index"], 2);
    assert_eq!(mined["previous_hash"], get(app.clone(), "/api/blocks/1").await.1["hash"]);
    let hash = mined["hash"].as_str().unwrap();
    assert!(hash.starts_with(&"0".repeat(MINING_DIFFICULTY)));
    let mined_transactions = mined["transactions"].as_array().unwrap();
    assert_eq!(mined_transactions.len(), 1);
    assert_eq!(mined_transactions[0]["sender"], wallet.address());

    // The mined block is actually persisted, readable back via GET.
    let (get_status, fetched) = get(app.clone(), "/api/blocks/2").await;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(fetched, mined);

    // Its transaction was cleared from the mempool.
    let (_, pending_json) = get(app.clone(), "/api/transactions/pending").await;
    assert!(pending_json.as_array().unwrap().is_empty());

    // The chain, reloaded from storage, is still valid.
    let (_, valid_json) = get(app, "/api/chain/valid").await;
    assert_eq!(valid_json["valid"], true);
}

#[tokio::test]
async fn balances_reflect_confirmed_transactions_after_mining() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 40),
    )
    .await;

    let (mine_status, _) = post(app.clone(), "/api/mine", serde_json::json!({})).await;
    assert_eq!(mine_status, StatusCode::OK);

    let (_, sender_balance) = get(app.clone(), &format!("/api/wallets/{}/balance", wallet.address())).await;
    assert_eq!(sender_balance["balance"], 60);

    let (_, recipient_balance) = get(app, "/api/wallets/bob/balance").await;
    assert_eq!(recipient_balance["balance"], 40);
}

#[tokio::test]
async fn mine_clears_only_the_transactions_it_actually_included() {
    let wallet_a = Wallet::generate();
    let wallet_b = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet_a, 100), (&wallet_b, 100)]).await;

    post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet_a, "bob", 10),
    )
    .await;

    let (mine_status, mined) = post(app.clone(), "/api/mine", serde_json::json!({})).await;
    assert_eq!(mine_status, StatusCode::OK);
    assert_eq!(mined["transactions"].as_array().unwrap().len(), 1);

    // Submitted *after* the first mine - must not be affected by it.
    post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet_b, "carol", 5),
    )
    .await;

    let (_, pending_json) = get(app, "/api/transactions/pending").await;
    let pending = pending_json.as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["sender"], wallet_b.address());
}

#[tokio::test]
async fn persistence_failure_does_not_lose_mempool_transactions() {
    let wallet = Wallet::generate();
    let (app, state) = test_app_with_funded_wallets_and_state(&[(&wallet, 100)]).await;

    let (submit_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;
    assert_eq!(submit_status, StatusCode::OK);

    // Simulate a persistence failure: drop the table `insert_block` needs.
    sqlx::query("DROP TABLE block_transactions")
        .execute(&state.pool)
        .await
        .unwrap();

    let (mine_status, _) = post(app.clone(), "/api/mine", serde_json::json!({})).await;
    assert_eq!(mine_status, StatusCode::INTERNAL_SERVER_ERROR);

    // The transaction must still be pending - not silently dropped.
    let (_, pending_json) = get(app, "/api/transactions/pending").await;
    let pending = pending_json.as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["sender"], wallet.address());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_mining_requests_do_not_mine_the_same_transaction_twice() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 100)]).await;

    let (submit_status, _) = post(
        app.clone(),
        "/api/transactions",
        signed_transaction_request(&wallet, "bob", 10),
    )
    .await;
    assert_eq!(submit_status, StatusCode::OK);

    let (result1, result2) = tokio::join!(
        post(app.clone(), "/api/mine", serde_json::json!({})),
        post(app.clone(), "/api/mine", serde_json::json!({})),
    );

    let statuses = [result1.0, result2.0];
    let ok_count = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    let rejected_count = statuses.iter().filter(|s| **s == StatusCode::BAD_REQUEST).count();

    assert_eq!(ok_count, 1, "exactly one mining request should succeed");
    assert_eq!(rejected_count, 1, "the other should find nothing pending");

    // Exactly one new block was mined (2 seeded + 1 mined = 3), not two.
    let (_, blocks_json) = get(app, "/api/blocks").await;
    assert_eq!(blocks_json.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn wallet_balance_is_zero_for_an_unknown_address() {
    let app = test_app_with_two_blocks().await;

    let (status, json) = get(app, "/api/wallets/nobody/balance").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["address"], "nobody");
    assert_eq!(json["balance"], 0);
}

#[tokio::test]
async fn wallet_balance_reflects_confirmed_funding() {
    let wallet = Wallet::generate();
    let app = test_app_with_funded_wallets(&[(&wallet, 42)]).await;

    let (status, json) = get(app, &format!("/api/wallets/{}/balance", wallet.address())).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["address"], wallet.address());
    assert_eq!(json["balance"], 42);
}

// `DEV_FUNDING_ENABLED` is a process-wide environment variable, and Rust
// runs tests in parallel within one process by default - so every test
// below that touches it holds this lock for its duration, to avoid racing
// another such test (no other test in this crate reads or sets this
// variable, so this is the only contention point).
fn dev_funding_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[tokio::test]
async fn dev_fund_endpoint_is_absent_when_disabled() {
    let _guard = dev_funding_env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("DEV_FUNDING_ENABLED") };

    let app = test_app_with_two_blocks().await;
    // Axum's own unmatched-route 404 has an empty body (unlike our
    // AppError 404s, which are JSON) - checked directly here rather than
    // through the shared `post()` helper, which always expects JSON.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/dev/fund")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "address": "alice", "amount": 100 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dev_fund_endpoint_funds_a_wallet_when_enabled() {
    let _guard = dev_funding_env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("DEV_FUNDING_ENABLED", "true") };

    let app = test_app_with_two_blocks().await;
    let (status, json) = post(
        app.clone(),
        "/api/dev/fund",
        serde_json::json!({ "address": "alice", "amount": 100 }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["transactions"][0]["recipient"], "alice");
    assert_eq!(json["transactions"][0]["amount"], 100);
    let hash = json["hash"].as_str().unwrap();
    assert!(hash.starts_with(&"0".repeat(MINING_DIFFICULTY)));

    let (_, balance) = get(app, "/api/wallets/alice/balance").await;
    assert_eq!(balance["balance"], 100);

    unsafe { std::env::remove_var("DEV_FUNDING_ENABLED") };
}

#[tokio::test]
async fn dev_fund_endpoint_rejects_non_positive_amount() {
    let _guard = dev_funding_env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("DEV_FUNDING_ENABLED", "true") };

    let app = test_app_with_two_blocks().await;
    let (status, json) = post(
        app,
        "/api/dev/fund",
        serde_json::json!({ "address": "alice", "amount": 0 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].as_str().unwrap().contains("amount"));

    unsafe { std::env::remove_var("DEV_FUNDING_ENABLED") };
}

#[tokio::test]
async fn dev_fund_endpoint_rejects_empty_address() {
    let _guard = dev_funding_env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("DEV_FUNDING_ENABLED", "true") };

    let app = test_app_with_two_blocks().await;
    let (status, json) = post(
        app,
        "/api/dev/fund",
        serde_json::json!({ "address": "", "amount": 10 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].is_string());

    unsafe { std::env::remove_var("DEV_FUNDING_ENABLED") };
}

#[tokio::test]
async fn dev_fund_endpoint_keeps_chain_valid() {
    let _guard = dev_funding_env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("DEV_FUNDING_ENABLED", "true") };

    let app = test_app_with_two_blocks().await;
    let (fund_status, _) = post(
        app.clone(),
        "/api/dev/fund",
        serde_json::json!({ "address": "alice", "amount": 50 }),
    )
    .await;
    assert_eq!(fund_status, StatusCode::OK);

    let (_, validity) = get(app, "/api/chain/valid").await;
    assert_eq!(validity["valid"], true);

    unsafe { std::env::remove_var("DEV_FUNDING_ENABLED") };
}
