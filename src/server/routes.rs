use axum::{
    extract::{Query, State},
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::server::AppState;
use crate::query::QueryEngine;
use crate::SymbolUri;
use crate::SymbolKind;
use std::sync::Arc;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub kind: Option<String>,
    pub vector: Option<bool>,
}

#[derive(Deserialize)]
pub struct UriParams {
    pub uri: String,
    pub depth: Option<usize>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn handle_index(State(_state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // For now, index the current directory where the binary is run
    // In a real scenario, this would probably take a path from the request
    Ok(Json(serde_json::json!({"status": "Indexing started (sync for now)"})))
}

pub async fn handle_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params.limit.unwrap_or(10);
    let vector = params.vector.unwrap_or(false);
    let kind = params.kind.and_then(|k| SymbolKind::from_str(&k).ok());

    let engine = QueryEngine::new(&state.store);
    
    let results = if vector {
        let embedding_engine = crate::query::EmbeddingEngine::new()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: format!("Failed to initialize embedding engine: {}", e) })))?;
        
        let query_vector = embedding_engine.embed_query(&params.query)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: format!("Failed to embed query: {}", e) })))?;
        
        engine.search_by_vector(&query_vector, limit)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
            .into_iter()
            .map(|r| r.symbol)
            .collect()
    } else {
        state.store.search_content(&params.query, kind, limit)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
    };

    Ok(Json(serde_json::to_value(&results).unwrap()))
}

pub async fn handle_trace(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UriParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uri = SymbolUri::parse(&params.uri)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;
    let depth = params.depth.unwrap_or(1);

    let engine = QueryEngine::new(&state.store);
    
    let callers = engine.find_callers(&uri, depth)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;
    
    let callees = engine.find_callees(&uri, depth)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    Ok(Json(serde_json::json!({
        "callers": serde_json::to_value(&callers).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?,
        "callees": serde_json::to_value(&callees).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?,
    })))
}

pub async fn handle_impact(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UriParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uri = SymbolUri::parse(&params.uri)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;
    let depth = params.depth.unwrap_or(3);

    let engine = QueryEngine::new(&state.store);
    let impact = engine.impact_analysis(&uri, depth)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    Ok(Json(serde_json::to_value(&impact).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?))
}

pub async fn handle_stats(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let stats = state.store.stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;
    
    Ok(Json(serde_json::to_value(&stats).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?))
}
