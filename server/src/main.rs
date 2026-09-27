use axum::{routing::get, Router};

mod api;
mod blockchain;
mod db;
mod error;
mod wallet;

use blockchain::Block;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = db::init_pool(&db::database_url())
        .await
        .expect("failed to initialize database");
    tracing::info!("database ready");

    // Seed the genesis block on first run only - `has_blocks` is what keeps
    // a restart from inserting a second one.
    if !db::has_blocks(&pool)
        .await
        .expect("failed to check for existing blocks")
    {
        db::insert_block(&pool, &Block::genesis())
            .await
            .expect("failed to insert genesis block");
        tracing::info!("inserted genesis block");
    }

    let app = build_app(api::AppState::new(pool));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("failed to bind to 127.0.0.1:8080");

    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("server error");
}

/// Assembles the full app: the standalone `/health` check plus the
/// blockchain API routes. Factored out so tests can build the same router
/// without going through `main`.
fn build_app(state: api::AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(api::router(state))
}

async fn health() -> &'static str {
    "OK"
}

#[cfg(test)]
mod tests {
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let pool = db::test_pool().await;
        let app = build_app(api::AppState::new(pool));

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"OK");
    }
}
