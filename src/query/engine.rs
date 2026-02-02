//! Query engine implementation
//!
//! Provides high-level query operations:
//! - Text search for symbols
//! - Call graph traversal (callers/callees)
//! - Impact analysis (BFS over dependency edges)
//! - Interface implementation search

use std::collections::{HashSet, VecDeque};
use crate::Result;
use crate::edge::EdgeKind;
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use crate::storage::SqliteStore;

/// Query result with relevance scoring
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub symbol: Symbol,
    pub score: f32,
}

impl QueryResult {
    pub fn new(symbol: Symbol, score: f32) -> Self {
        Self { symbol, score }
    }
}

/// Query engine for code intelligence operations
pub struct QueryEngine<'a> {
    store: &'a SqliteStore,
}

impl<'a> QueryEngine<'a> {
    /// Create a new query engine
    pub fn new(store: &'a SqliteStore) -> Self {
        Self { store }
    }

    /// Search for symbols by name pattern
    ///
    /// Uses SQL LIKE for pattern matching.
    /// Use `%` as wildcard: `%auth%` matches anything with "auth"
    pub fn search_by_name(&self, pattern: &str, limit: usize) -> Result<Vec<QueryResult>> {
        let like_pattern = if pattern.contains('%') {
            pattern.to_string()
        } else {
            format!("%{}%", pattern)
        };

        let symbols = self.store.find_symbols_by_name_pattern(&like_pattern)?;
        
        let results: Vec<_> = symbols
            .into_iter()
            .take(limit)
            .map(|s| {
                // Simple scoring: exact match = 1.0, partial match = 0.5
                let score = if s.name.to_lowercase() == pattern.to_lowercase() {
                    1.0
                } else {
                    0.5
                };
                QueryResult::new(s, score)
            })
            .collect();

        Ok(results)
    }

    /// Search for symbols by vector similarity
    pub fn search_by_vector(&self, vector: &[f32], limit: usize) -> Result<Vec<QueryResult>> {
        let results = self.store.search_by_vector(vector, limit)?;
        
        let query_results = results
            .into_iter()
            .map(|(symbol, score)| QueryResult::new(symbol, score))
            .collect();
            
        Ok(query_results)
    }

    /// Search for symbols by kind and optional name pattern
    pub fn search_by_kind(&self, kind: SymbolKind, name_pattern: Option<&str>, limit: usize) -> Result<Vec<Symbol>> {
        let all_of_kind = self.store.find_symbols_by_kind(kind)?;
        
        let filtered: Vec<_> = match name_pattern {
            Some(pattern) => {
                let lower_pattern = pattern.to_lowercase();
                all_of_kind
                    .into_iter()
                    .filter(|s| s.name.to_lowercase().contains(&lower_pattern))
                    .take(limit)
                    .collect()
            }
            None => all_of_kind.into_iter().take(limit).collect(),
        };

        Ok(filtered)
    }

    /// Find all callers of a function (symbols that call this)
    pub fn find_callers(&self, uri: &SymbolUri, depth: usize) -> Result<Vec<Symbol>> {
        self.traverse_edges(uri, EdgeKind::Calls, TraversalDirection::Incoming, depth)
    }

    /// Find all callees of a function (symbols this calls)
    pub fn find_callees(&self, uri: &SymbolUri, depth: usize) -> Result<Vec<Symbol>> {
        self.traverse_edges(uri, EdgeKind::Calls, TraversalDirection::Outgoing, depth)
    }

    /// Find all references to a symbol
    pub fn find_references(&self, uri: &SymbolUri) -> Result<Vec<Symbol>> {
        self.traverse_edges(uri, EdgeKind::References, TraversalDirection::Incoming, 1)
    }

    /// Find all symbols that inherit from a container
    pub fn find_subclasses(&self, uri: &SymbolUri) -> Result<Vec<Symbol>> {
        self.traverse_edges(uri, EdgeKind::Inherits, TraversalDirection::Incoming, 1)
    }

    /// Find parent classes/interfaces
    pub fn find_superclasses(&self, uri: &SymbolUri) -> Result<Vec<Symbol>> {
        self.traverse_edges(uri, EdgeKind::Inherits, TraversalDirection::Outgoing, 1)
    }

    /// Impact analysis: find all symbols affected by changes to this symbol
    ///
    /// Traverses the reverse dependency graph using BFS up to `depth` levels.
    /// Returns symbols that would potentially break if this symbol changes.
    pub fn impact_analysis(&self, uri: &SymbolUri, depth: usize) -> Result<Vec<ImpactResult>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        queue.push_back((uri.clone(), 0usize));
        visited.insert(uri.clone());

        while let Some((current_uri, current_depth)) = queue.pop_front() {
            if current_depth > depth {
                continue;
            }

            // Get all incoming edges (symbols that depend on current)
            for edge_kind in [EdgeKind::Calls, EdgeKind::References, EdgeKind::Inherits] {
                let edges = self.store.get_edges_to(&current_uri)?;
                
                for edge in edges.iter().filter(|e| e.kind == edge_kind) {
                    if !visited.contains(&edge.from_uri) {
                        visited.insert(edge.from_uri.clone());
                        
                        if let Some(symbol) = self.store.get_symbol(&edge.from_uri)? {
                            results.push(ImpactResult {
                                symbol,
                                depth: current_depth + 1,
                                edge_kind,
                                confidence: edge.confidence,
                            });
                        }

                        if current_depth + 1 < depth {
                            queue.push_back((edge.from_uri.clone(), current_depth + 1));
                        }
                    }
                }
            }
        }

        // Sort by depth, then by confidence
        results.sort_by(|a, b| {
            a.depth.cmp(&b.depth)
                .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
        });

        Ok(results)
    }

    /// Find symbols contained by a container (e.g., methods in a class)
    pub fn find_members(&self, container_uri: &SymbolUri) -> Result<Vec<Symbol>> {
        self.traverse_edges(container_uri, EdgeKind::Contains, TraversalDirection::Outgoing, 1)
    }

    /// Find the container of a symbol (e.g., class that contains a method)
    pub fn find_container(&self, uri: &SymbolUri) -> Result<Option<Symbol>> {
        let edges = self.store.get_edges_to(uri)?;
        
        for edge in edges {
            if edge.kind == EdgeKind::Contains {
                if let Some(symbol) = self.store.get_symbol(&edge.from_uri)? {
                    return Ok(Some(symbol));
                }
            }
        }
        
        Ok(None)
    }

    /// Generic edge traversal
    fn traverse_edges(
        &self,
        start: &SymbolUri,
        kind: EdgeKind,
        direction: TraversalDirection,
        depth: usize,
    ) -> Result<Vec<Symbol>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        queue.push_back((start.clone(), 0usize));
        visited.insert(start.clone());

        while let Some((current_uri, current_depth)) = queue.pop_front() {
            if current_depth >= depth {
                continue;
            }

            let edges = match direction {
                TraversalDirection::Outgoing => self.store.get_edges_from(&current_uri)?,
                TraversalDirection::Incoming => self.store.get_edges_to(&current_uri)?,
            };

            for edge in edges.iter().filter(|e| e.kind == kind) {
                let next_uri = match direction {
                    TraversalDirection::Outgoing => &edge.to_uri,
                    TraversalDirection::Incoming => &edge.from_uri,
                };

                if !visited.contains(next_uri) {
                    visited.insert(next_uri.clone());
                    
                    if let Some(symbol) = self.store.get_symbol(next_uri)? {
                        results.push(symbol);
                    }

                    if current_depth + 1 < depth {
                        queue.push_back((next_uri.clone(), current_depth + 1));
                    }
                }
            }
        }

        Ok(results)
    }
}

/// Direction for edge traversal
#[derive(Debug, Clone, Copy)]
enum TraversalDirection {
    Outgoing,
    Incoming,
}

/// Result of impact analysis
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ImpactResult {
    pub symbol: Symbol,
    pub depth: usize,
    pub edge_kind: EdgeKind,
    pub confidence: f32,
}

impl ImpactResult {
    /// Check if this is a direct impact (depth 1)
    pub fn is_direct(&self) -> bool {
        self.depth == 1
    }

    /// Check if this is a high-confidence impact
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.9
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Edge;

    fn sample_symbol(name: &str, kind: SymbolKind, line: u32) -> Symbol {
        Symbol::new("repo", "src/main.py", kind, name, line, line + 5, "...")
    }

    #[test]
    fn test_search_by_name() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        store.insert_symbol(&sample_symbol("authenticate", SymbolKind::Callable, 10)).unwrap();
        store.insert_symbol(&sample_symbol("authorize", SymbolKind::Callable, 20)).unwrap();
        store.insert_symbol(&sample_symbol("validate", SymbolKind::Callable, 30)).unwrap();
        
        let engine = QueryEngine::new(&store);
        
        let results = engine.search_by_name("auth", 10).unwrap();
        assert_eq!(results.len(), 2); // authenticate, authorize
    }

    #[test]
    fn test_callers_callees() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let a = sample_symbol("a", SymbolKind::Callable, 10);
        let b = sample_symbol("b", SymbolKind::Callable, 20);
        let c = sample_symbol("c", SymbolKind::Callable, 30);
        
        store.insert_symbol(&a).unwrap();
        store.insert_symbol(&b).unwrap();
        store.insert_symbol(&c).unwrap();
        
        // a calls b, b calls c
        store.insert_edge(&Edge::new(a.uri.clone(), b.uri.clone(), EdgeKind::Calls)).unwrap();
        store.insert_edge(&Edge::new(b.uri.clone(), c.uri.clone(), EdgeKind::Calls)).unwrap();
        
        let engine = QueryEngine::new(&store);
        
        // Callers of b
        let callers = engine.find_callers(&b.uri, 1).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "a");
        
        // Callees of b
        let callees = engine.find_callees(&b.uri, 1).unwrap();
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "c");
    }

    #[test]
    fn test_impact_analysis() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let core = sample_symbol("core_util", SymbolKind::Callable, 10);
        let service = sample_symbol("service", SymbolKind::Callable, 20);
        let handler = sample_symbol("handler", SymbolKind::Callable, 30);
        
        store.insert_symbol(&core).unwrap();
        store.insert_symbol(&service).unwrap();
        store.insert_symbol(&handler).unwrap();
        
        // handler -> service -> core_util
        store.insert_edge(&Edge::new(service.uri.clone(), core.uri.clone(), EdgeKind::Calls)).unwrap();
        store.insert_edge(&Edge::new(handler.uri.clone(), service.uri.clone(), EdgeKind::Calls)).unwrap();
        
        let engine = QueryEngine::new(&store);
        
        // Changing core_util affects service (depth 1) and handler (depth 2)
        let impact = engine.impact_analysis(&core.uri, 3).unwrap();
        assert_eq!(impact.len(), 2);
        
        let direct: Vec<_> = impact.iter().filter(|r| r.is_direct()).collect();
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].symbol.name, "service");
    }
}
