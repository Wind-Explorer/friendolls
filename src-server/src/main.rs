use axum::{Router, http::StatusCode, routing::get};
use friendolls_common::DEFAULT_SERVER_PORT;

mod network;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let bind_addr =
        std::env::var("BIND_ADDR").unwrap_or_else(|_| format!("127.0.0.1:{DEFAULT_SERVER_PORT}"));
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let app = Router::new()
        .route("/", get("ok"))
        .route("/health", get("ok"))
        .merge(network::routes())
        .fallback((StatusCode::NOT_FOUND, "not found"));

    println!("friendolls-server listening on http://{bind_addr}");

    axum::serve(listener, app).await
}
