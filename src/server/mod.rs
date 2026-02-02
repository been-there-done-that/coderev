use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use crate::storage::SqliteStore;
use std::sync::Arc;

pub mod routes;

pub struct AppState {
    pub store: Arc<SqliteStore>,
}

pub async fn run_server(addr: SocketAddr, store: SqliteStore) -> anyhow::Result<()> {
    let state = Arc::new(AppState {
        store: Arc::new(store),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/v1/index", post(routes::handle_index))
        .route("/api/v1/search", get(routes::handle_search))
        .route("/api/v1/trace", get(routes::handle_trace))
        .route("/api/v1/impact", get(routes::handle_impact))
        .route("/api/v1/stats", get(routes::handle_stats))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
