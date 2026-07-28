use std::env;

use axum::{http::StatusCode, routing::get, Router};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let app = Router::new()
        .route("/", get("ok"))
        .route("/health", get("ok"))
        .fallback((StatusCode::NOT_FOUND, "not found"));

    println!("wyd-server listening on http://{bind_addr}");

    axum::serve(listener, app).await
}
