use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::blockchain::{Block, Transaction};
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

/// A router backed by an isolated in-memory DB, seeded with two blocks.
async fn test_app_with_two_blocks() -> axum::Router {
    let pool = db::test_pool().await;

    let genesis = Block::genesis();
    db::insert_block(&pool, &genesis).await.unwrap();

    let second = Block::new(1, 1_000, "second".to_string(), genesis.hash.clone(), Vec::new());
    db::insert_block(&pool, &second).await.unwrap();

    router(AppState::new(pool))
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
    assert_eq!(json["data"], "second");
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
async fn valid_transaction_is_accepted() {
    let app = test_app_with_two_blocks().await;
    let wallet = Wallet::generate();

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
async fn invalid_transaction_is_rejected() {
    let app = test_app_with_two_blocks().await;
    let wallet = Wallet::generate();

    let mut payload = signed_transaction_request(&wallet, "bob", 10);
    payload["amount"] = serde_json::json!(0);

    let (status, json) = post(app, "/api/transactions", payload).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn unsigned_transaction_is_rejected() {
    let app = test_app_with_two_blocks().await;
    let wallet = Wallet::generate();

    let mut payload = signed_transaction_request(&wallet, "bob", 10);
    payload["signature"] = serde_json::json!("00".repeat(64));

    let (status, json) = post(app, "/api/transactions", payload).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "invalid transaction: signature is invalid");
}

#[tokio::test]
async fn pending_transactions_reflects_submitted_transactions() {
    let app = test_app_with_two_blocks().await;
    let wallet = Wallet::generate();

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
