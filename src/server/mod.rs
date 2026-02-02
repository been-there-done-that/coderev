use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{Query, State},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use crate::storage::SqliteStore;
use std::path::PathBuf;

use tower_http::services::ServeDir;

pub mod routes;

/// Server state
pub struct AppState {
    pub database_path: PathBuf,
}

pub async fn start_server(port: u16, database_path: PathBuf) -> anyhow::Result<()> {
    let state = Arc::new(AppState {
        database_path: database_path.clone(),
    });

    let app = Router::new()
        .route("/stats", get(routes::get_stats))
        .route("/search", get(routes::search))
        .route("/callers", get(routes::get_callers))
        .route("/symbol", get(routes::get_symbol))
        .route("/impact", get(routes::get_impact))
        .route("/graph", get(routes::get_graph))
        .fallback_service(ServeDir::new("ui/build"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on {}", addr);
    println!("🌍 Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
