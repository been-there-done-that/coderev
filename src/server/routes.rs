use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::storage::SqliteStore;
use crate::query::QueryEngine;
use crate::uri::SymbolUri;
use super::AppState;

#[derive(Serialize)]
pub struct StatsResponse {
    symbols: usize,
    edges: usize,
    embeddings: usize,
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Json<StatsResponse> {
    // Open DB for each request roughly? Or keep pool? 
    // SqliteStore opens connection. Rusqlite connection is not thread-safe to share without Mutex.
    // Opening it per request is safer for now given our architecture (embedded sqlite).
    // Or we could verify if we can share a r2d2 pool. 
    // For now, let's open per request.
    
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(StatsResponse { symbols: 0, edges: 0, embeddings: 0 }),
    };
    
    let stats = store.stats().unwrap_or(crate::storage::sqlite::DbStats {
        symbols: 0, edges: 0, embeddings: 0, unresolved: 0, imports: 0, callsite_embeddings: 0
    });
    
    Json(StatsResponse {
        symbols: stats.symbols,
        edges: stats.edges,
        embeddings: stats.embeddings,
    })
}

#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
    limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchResult {
    name: String,
    uri: String,
    kind: String,
    path: String,
    line: u32,
    score: f32,
    content: String,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<Vec<SearchResult>> {
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };
    
    let limit = params.limit.unwrap_or(10);
    
    // Optimize empty search to just return recent/top symbols
    let results = if params.q.trim().is_empty() {
         store.get_recent_symbols(limit).unwrap_or_default()
    } else {
         store.search_content(&params.q, None, limit).unwrap_or_default()
    };
    
    let response = results.into_iter().map(|s| SearchResult {
        name: s.name,
        uri: s.uri.to_uri_string(),
        kind: format!("{:?}", s.kind),
        path: s.path,
        line: s.line_start,
        score: 1.0, 
        content: s.content,
    }).collect();
    
    Json(response)
}

#[derive(Deserialize)]
pub struct CallersParams {
    uri: String,
    depth: Option<usize>,
}

#[derive(Serialize)]
pub struct CallerResult {
    name: String,
    uri: String,
    from_uri: String,
}

pub async fn get_callers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallersParams>,
) -> Json<Vec<CallerResult>> {
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };
    
    let engine = QueryEngine::new(&store);
    let target = match SymbolUri::parse(&params.uri) {
        Ok(u) => u,
        Err(_) => return Json(vec![]),
    };
    
    let callers = engine.find_callers(&target, params.depth.unwrap_or(1)).unwrap_or_default();
    
    let response = callers.into_iter().map(|s| CallerResult {
        name: s.name,
        uri: s.uri.to_uri_string(),
        from_uri: s.uri.to_uri_string(), // Simplify: caller is returned as symbol
    }).collect();
    
    Json(response)
}

#[derive(Deserialize)]
pub struct SymbolParams {
    uri: String,
}

#[derive(Serialize)]
pub struct SymbolResult {
    symbol: crate::symbol::Symbol,
    edges_out: Vec<crate::edge::Edge>,
    edges_in: Vec<crate::edge::Edge>,
}

pub async fn get_symbol(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SymbolParams>,
) -> Json<Option<SymbolResult>> {
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(None),
    };

    let uri = match SymbolUri::parse(&params.uri) {
        Ok(u) => u,
        Err(_) => return Json(None),
    };

    let symbol = match store.get_symbol(&uri).unwrap_or(None) {
        Some(s) => s,
        None => return Json(None),
    };

    let edges_out = store.get_edges_from(&uri).unwrap_or_default();
    let edges_in = store.get_edges_to(&uri).unwrap_or_default();

    Json(Some(SymbolResult {
        symbol,
        edges_out,
        edges_in,
    }))
}

#[derive(Deserialize)]
pub struct ImpactParams {
    uri: String,
    depth: Option<usize>,
}

#[derive(Serialize)]
pub struct ImpactResult {
    symbol: crate::symbol::Symbol,
    depth: usize,
    confidence: f32,
    is_direct: bool,
}

pub async fn get_impact(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ImpactParams>,
) -> Json<Vec<ImpactResult>> {
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    let engine = QueryEngine::new(&store);
    let target = match SymbolUri::parse(&params.uri) {
        Ok(u) => u,
        Err(_) => return Json(vec![]),
    };

    let impact = engine.impact_analysis(&target, params.depth.unwrap_or(3)).unwrap_or_default();

    let response = impact.into_iter().map(|res| {
        let is_direct = res.is_direct();
        ImpactResult {
            symbol: res.symbol,
            depth: res.depth,
            confidence: res.confidence,
            is_direct,
        }
    }).collect();

    Json(response)
}

#[derive(Deserialize)]
pub struct GraphParams {
    uri: String,
    depth: Option<usize>,
}

#[derive(Serialize)]
pub struct GraphResult {
    nodes: Vec<crate::symbol::Symbol>,
    edges: Vec<crate::edge::Edge>,
}

pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GraphParams>,
) -> Json<GraphResult> {
    let store = match SqliteStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return Json(GraphResult { nodes: vec![], edges: vec![] }),
    };

    // Simple neighborhood graph: center node + callers + callees (depth 1)
    let uri = match SymbolUri::parse(&params.uri) {
        Ok(u) => u,
        Err(_) => return Json(GraphResult { nodes: vec![], edges: vec![] }),
    };
    
    let center = match store.get_symbol(&uri).unwrap_or(None) {
        Some(s) => s,
        None => return Json(GraphResult { nodes: vec![], edges: vec![] }),
    };
    
    let mut nodes = vec![center];
    let mut edges = vec![];
    let mut visited_uris = std::collections::HashSet::new();
    visited_uris.insert(params.uri.clone());

    // Get outgoing edges (Callees)
    let out_edges = store.get_edges_from(&uri).unwrap_or_default();
    for edge in out_edges {
        if !visited_uris.contains(&edge.to_uri.to_uri_string()) {
             if let Ok(Some(target)) = store.get_symbol(&edge.to_uri) {
                 nodes.push(target);
                 visited_uris.insert(edge.to_uri.to_uri_string());
             }
        }
        edges.push(edge);
    }
    
    // Get incoming edges (Callers)
    let in_edges = store.get_edges_to(&uri).unwrap_or_default();
    for edge in in_edges {
        if !visited_uris.contains(&edge.from_uri.to_uri_string()) {
             if let Ok(Some(source)) = store.get_symbol(&edge.from_uri) {
                 nodes.push(source);
                 visited_uris.insert(edge.from_uri.to_uri_string());
             }
        }
        edges.push(edge);
    }

    Json(GraphResult { nodes, edges })
}
