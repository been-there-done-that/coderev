//! Core adapter framework
//!
//! Defines the traits and types that all language adapters must implement.

use crate::Result;
use crate::edge::Edge;
use crate::symbol::Symbol;
use crate::scope::graph::ScopeGraph;
use std::path::Path;

/// Result of parsing a file with a language adapter
#[derive(Debug, Default)]
pub struct AdapterResult {
    /// Extracted symbols
    pub symbols: Vec<Symbol>,
    /// Edges discovered during parsing (calls, contains, etc.)
    pub edges: Vec<Edge>,
    /// Scope graph for name resolution
    pub scope_graph: ScopeGraph,
}

impl AdapterResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol
    pub fn add_symbol(&mut self, symbol: Symbol) {
        self.symbols.push(symbol);
    }

    /// Add an edge
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }
}

/// A parsed file ready for indexing
#[derive(Debug)]
pub struct ParsedFile {
    /// The file path (relative to repo root)
    pub path: String,
    /// The adapter result
    pub result: AdapterResult,
}

/// Trait for language adapters
///
/// Each language adapter is responsible for:
/// 1. Identifying files it can parse
/// 2. Extracting symbols using tree-sitter
/// 3. Building edges for relationships
/// 4. Constructing a scope graph for name resolution
pub trait LanguageAdapter: Send + Sync {
    /// Get the language name (for display)
    fn language_name(&self) -> &str;

    /// Get file extensions this adapter handles
    fn file_extensions(&self) -> &[&str];

    /// Check if this adapter can handle a file
    fn can_handle(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            self.file_extensions().contains(&ext)
        } else {
            false
        }
    }

    /// Parse a file and extract symbols/edges
    fn parse_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult>;
}

/// Registry of language adapters
#[derive(Default)]
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl AdapterRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an adapter
    pub fn register(&mut self, adapter: impl LanguageAdapter + 'static) {
        self.adapters.push(Box::new(adapter));
    }

    /// Find an adapter for a file
    pub fn find_adapter(&self, path: &Path) -> Option<&dyn LanguageAdapter> {
        self.adapters
            .iter()
            .find(|a| a.can_handle(path))
            .map(|a| a.as_ref())
    }

    /// Get all registered adapters
    pub fn adapters(&self) -> &[Box<dyn LanguageAdapter>] {
        &self.adapters
    }

    /// Parse a file using the appropriate adapter
    pub fn parse_file(&self, repo: &str, path: &Path, content: &str) -> Result<Option<AdapterResult>> {
        if let Some(adapter) = self.find_adapter(path) {
            let rel_path = path.to_string_lossy();
            let result = adapter.parse_file(repo, &rel_path, content)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
}

/// Create a default registry with all built-in adapters
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(super::python::PythonAdapter::new());
    registry.register(super::javascript::JavaScriptAdapter::new());
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TestAdapter;
    
    impl LanguageAdapter for TestAdapter {
        fn language_name(&self) -> &str { "test" }
        fn file_extensions(&self) -> &[&str] { &["test"] }
        fn parse_file(&self, _repo: &str, _path: &str, _content: &str) -> Result<AdapterResult> {
            Ok(AdapterResult::new())
        }
    }

    #[test]
    fn test_registry() {
        let mut registry = AdapterRegistry::new();
        registry.register(TestAdapter);

        assert!(registry.find_adapter(Path::new("foo.test")).is_some());
        assert!(registry.find_adapter(Path::new("foo.other")).is_none());
    }
}
