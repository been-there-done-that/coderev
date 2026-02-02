//! Symbol Graph - In-memory representation of the code graph
//!
//! Provides an in-memory graph structure for building and querying
//! before persisting to storage.

use std::collections::{HashMap, HashSet};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::Symbol;
use crate::uri::SymbolUri;

/// In-memory symbol graph for building and querying code relationships.
///
/// This structure is used during indexing to build up the graph before
/// persisting to SQLite. It can also be used for read-only query operations.
#[derive(Debug, Default)]
pub struct SymbolGraph {
    /// All symbols indexed by their URI
    symbols: HashMap<SymbolUri, Symbol>,
    /// Edges from a symbol (outgoing edges)
    edges_from: HashMap<SymbolUri, Vec<Edge>>,
    /// Edges to a symbol (incoming edges)
    edges_to: HashMap<SymbolUri, Vec<Edge>>,
    /// Symbols indexed by file path
    symbols_by_path: HashMap<String, Vec<SymbolUri>>,
    /// Symbols indexed by name (for quick lookup)
    symbols_by_name: HashMap<String, Vec<SymbolUri>>,
}

impl SymbolGraph {
    /// Create a new empty symbol graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol to the graph
    pub fn add_symbol(&mut self, symbol: Symbol) {
        let uri = symbol.uri.clone();
        let path = symbol.path.clone();
        let name = symbol.name.clone();

        // Index by path
        self.symbols_by_path
            .entry(path)
            .or_default()
            .push(uri.clone());

        // Index by name
        self.symbols_by_name
            .entry(name)
            .or_default()
            .push(uri.clone());

        // Store the symbol
        self.symbols.insert(uri, symbol);
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) {
        let from = edge.from_uri.clone();
        let to = edge.to_uri.clone();

        // Add to outgoing edges
        self.edges_from.entry(from).or_default().push(edge.clone());

        // Add to incoming edges
        self.edges_to.entry(to).or_default().push(edge);
    }

    /// Get a symbol by its URI
    pub fn get_symbol(&self, uri: &SymbolUri) -> Option<&Symbol> {
        self.symbols.get(uri)
    }

    /// Get all symbols in a file
    pub fn get_symbols_in_file(&self, path: &str) -> Vec<&Symbol> {
        self.symbols_by_path
            .get(path)
            .map(|uris| uris.iter().filter_map(|uri| self.symbols.get(uri)).collect())
            .unwrap_or_default()
    }

    /// Get all symbols with a given name
    pub fn get_symbols_by_name(&self, name: &str) -> Vec<&Symbol> {
        self.symbols_by_name
            .get(name)
            .map(|uris| uris.iter().filter_map(|uri| self.symbols.get(uri)).collect())
            .unwrap_or_default()
    }

    /// Get outgoing edges from a symbol
    pub fn get_edges_from(&self, uri: &SymbolUri) -> &[Edge] {
        self.edges_from.get(uri).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get incoming edges to a symbol
    pub fn get_edges_to(&self, uri: &SymbolUri) -> &[Edge] {
        self.edges_to.get(uri).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get outgoing edges of a specific kind
    pub fn get_edges_from_by_kind(&self, uri: &SymbolUri, kind: EdgeKind) -> Vec<&Edge> {
        self.get_edges_from(uri)
            .iter()
            .filter(|e| e.kind == kind)
            .collect()
    }

    /// Get incoming edges of a specific kind
    pub fn get_edges_to_by_kind(&self, uri: &SymbolUri, kind: EdgeKind) -> Vec<&Edge> {
        self.get_edges_to(uri)
            .iter()
            .filter(|e| e.kind == kind)
            .collect()
    }

    /// Find all callers of a callable
    pub fn find_callers(&self, uri: &SymbolUri) -> Vec<&Symbol> {
        self.get_edges_to_by_kind(uri, EdgeKind::Calls)
            .iter()
            .filter_map(|edge| self.get_symbol(&edge.from_uri))
            .collect()
    }

    /// Find all callees of a callable
    pub fn find_callees(&self, uri: &SymbolUri) -> Vec<&Symbol> {
        self.get_edges_from_by_kind(uri, EdgeKind::Calls)
            .iter()
            .filter_map(|edge| self.get_symbol(&edge.to_uri))
            .collect()
    }

    /// Find symbols that inherit from a container
    pub fn find_subclasses(&self, uri: &SymbolUri) -> Vec<&Symbol> {
        self.get_edges_to_by_kind(uri, EdgeKind::Inherits)
            .iter()
            .filter_map(|edge| self.get_symbol(&edge.from_uri))
            .collect()
    }

    /// Find parent classes/interfaces of a container
    pub fn find_superclasses(&self, uri: &SymbolUri) -> Vec<&Symbol> {
        self.get_edges_from_by_kind(uri, EdgeKind::Inherits)
            .iter()
            .filter_map(|edge| self.get_symbol(&edge.to_uri))
            .collect()
    }

    /// Perform impact analysis - find all symbols affected by changes to this symbol
    ///
    /// Uses BFS to traverse the reverse dependency graph up to `depth` levels.
    pub fn impact_analysis(&self, uri: &SymbolUri, depth: usize) -> Vec<&Symbol> {
        let mut visited = HashSet::new();
        let mut queue = vec![(uri.clone(), 0usize)];
        let mut affected = Vec::new();

        while let Some((current_uri, current_depth)) = queue.pop() {
            if current_depth > depth {
                continue;
            }

            if visited.contains(&current_uri) {
                continue;
            }
            visited.insert(current_uri.clone());

            // Add to results (skip the starting symbol)
            if current_depth > 0 {
                if let Some(symbol) = self.get_symbol(&current_uri) {
                    affected.push(symbol);
                }
            }

            // Find all symbols that depend on this one
            for edge in self.get_edges_to(&current_uri) {
                if edge.kind.is_dependency() {
                    queue.push((edge.from_uri.clone(), current_depth + 1));
                }
            }
        }

        affected
    }

    /// Get statistics about the graph
    pub fn stats(&self) -> GraphStats {
        let total_edges: usize = self.edges_from.values().map(|v| v.len()).sum();
        let deterministic_edges = self.edges_from
            .values()
            .flat_map(|v| v.iter())
            .filter(|e| e.is_deterministic())
            .count();

        GraphStats {
            total_symbols: self.symbols.len(),
            total_edges,
            deterministic_edges,
            probabilistic_edges: total_edges - deterministic_edges,
            files: self.symbols_by_path.len(),
        }
    }

    /// Get all symbols
    pub fn all_symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Get all edges
    pub fn all_edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges_from.values().flat_map(|v| v.iter())
    }
}

/// Statistics about a symbol graph
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub total_symbols: usize,
    pub total_edges: usize,
    pub deterministic_edges: usize,
    pub probabilistic_edges: usize,
    pub files: usize,
}

impl std::fmt::Display for GraphStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Symbol Graph Statistics:")?;
        writeln!(f, "  Files: {}", self.files)?;
        writeln!(f, "  Symbols: {}", self.total_symbols)?;
        writeln!(f, "  Edges: {} (deterministic: {}, probabilistic: {})",
            self.total_edges, self.deterministic_edges, self.probabilistic_edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolKind;

    fn sample_symbol(name: &str, kind: SymbolKind, line: u32) -> Symbol {
        Symbol::new("repo", "src/main.py", kind, name, line, line + 5, "...")
    }

    #[test]
    fn test_add_and_retrieve_symbol() {
        let mut graph = SymbolGraph::new();
        let symbol = sample_symbol("my_func", SymbolKind::Callable, 10);
        let uri = symbol.uri.clone();

        graph.add_symbol(symbol);

        let retrieved = graph.get_symbol(&uri).unwrap();
        assert_eq!(retrieved.name, "my_func");
    }

    #[test]
    fn test_callers_and_callees() {
        let mut graph = SymbolGraph::new();
        
        let caller = sample_symbol("caller", SymbolKind::Callable, 10);
        let callee = sample_symbol("callee", SymbolKind::Callable, 20);
        
        let caller_uri = caller.uri.clone();
        let callee_uri = callee.uri.clone();

        graph.add_symbol(caller);
        graph.add_symbol(callee);
        graph.add_edge(Edge::new(caller_uri.clone(), callee_uri.clone(), EdgeKind::Calls));

        let callers = graph.find_callers(&callee_uri);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "caller");

        let callees = graph.find_callees(&caller_uri);
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "callee");
    }

    #[test]
    fn test_impact_analysis() {
        let mut graph = SymbolGraph::new();
        
        // Create a call chain: a -> b -> c
        let a = sample_symbol("a", SymbolKind::Callable, 10);
        let b = sample_symbol("b", SymbolKind::Callable, 20);
        let c = sample_symbol("c", SymbolKind::Callable, 30);
        
        let a_uri = a.uri.clone();
        let b_uri = b.uri.clone();
        let c_uri = c.uri.clone();

        graph.add_symbol(a);
        graph.add_symbol(b);
        graph.add_symbol(c);
        
        graph.add_edge(Edge::new(a_uri.clone(), b_uri.clone(), EdgeKind::Calls));
        graph.add_edge(Edge::new(b_uri.clone(), c_uri.clone(), EdgeKind::Calls));

        // Changing c affects b (depth 1) and a (depth 2)
        let affected = graph.impact_analysis(&c_uri, 2);
        assert_eq!(affected.len(), 2);
    }
}
