//! Global Symbol Resolver
//!
//! The resolver performs global name resolution across the entire repository.
//! It turns UnresolvedReferences into concrete Calls/Inherits/References edges.
//!
//! Resolution order:
//! 1. Local scope (same file, same container)
//! 2. Imports (imported namespace)
//! 3. Container methods (if receiver is known)
//! 4. Inheritance (walk base classes)
//! 5. Global name match (fallback)

use std::collections::HashMap;
use crate::Result;
use crate::edge::{Edge, EdgeKind};
use crate::storage::{SqliteStore, PersistedUnresolvedReference};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;

/// Result of resolving a reference
#[derive(Debug)]
pub enum ResolutionResult {
    /// Single unambiguous match
    Resolved {
        target_uri: SymbolUri,
        confidence: f32,
        strategy: ResolutionStrategy,
    },
    /// Multiple possible matches
    Ambiguous {
        candidates: Vec<SymbolUri>,
    },
    /// No matches found
    Unresolved,
}

/// Strategy used to resolve a reference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStrategy {
    /// Found in the same file
    LocalScope,
    /// Found via import
    Import,
    /// Found as container method (receiver.method())
    ContainerMethod,
    /// Found in inheritance chain
    Inheritance,
    /// Global name match
    GlobalName,
}

impl ResolutionStrategy {
    /// Get the confidence score for this strategy
    pub fn confidence(&self) -> f32 {
        match self {
            Self::LocalScope => 1.0,
            Self::Import => 0.95,
            Self::ContainerMethod => 0.9,
            Self::Inheritance => 0.85,
            Self::GlobalName => 0.7,
        }
    }
}

/// Global symbol index for fast lookups
pub struct SymbolIndex {
    /// name → [SymbolUri] (all symbols with this name)
    name_to_uris: HashMap<String, Vec<SymbolUri>>,
    /// qualified_name (path/name) → SymbolUri
    qualified_name_to_uri: HashMap<String, SymbolUri>,
    /// container URI → methods/members URIs
    container_methods: HashMap<SymbolUri, Vec<SymbolUri>>,
    /// file path → symbol URIs in that file
    file_symbols: HashMap<String, Vec<SymbolUri>>,
    /// All symbols (for lookups)
    symbols: HashMap<SymbolUri, Symbol>,
}

impl SymbolIndex {
    /// Build the symbol index from the store
    pub fn build_from_store(store: &SqliteStore) -> Result<Self> {
        let mut name_to_uris: HashMap<String, Vec<SymbolUri>> = HashMap::new();
        let mut qualified_name_to_uri: HashMap<String, SymbolUri> = HashMap::new();
        let mut container_methods: HashMap<SymbolUri, Vec<SymbolUri>> = HashMap::new();
        let mut file_symbols: HashMap<String, Vec<SymbolUri>> = HashMap::new();
        let mut symbols: HashMap<SymbolUri, Symbol> = HashMap::new();

        // Get all symbols
        let all_symbols = store.find_symbols_by_name_pattern("%")?;

        for symbol in all_symbols {
            let uri = symbol.uri.clone();
            
            // Index by name
            name_to_uris
                .entry(symbol.name.clone())
                .or_default()
                .push(uri.clone());
            
            // Index by qualified name (path + name)
            let qualified = format!("{}:{}", symbol.path, symbol.name);
            qualified_name_to_uri.insert(qualified, uri.clone());
            
            // Index by file
            file_symbols
                .entry(symbol.path.clone())
                .or_default()
                .push(uri.clone());
            
            // Store symbol
            symbols.insert(uri, symbol);
        }

        // Build container → methods index from edges
        let contains_edges = store.get_edges_by_kind(EdgeKind::Contains)?;
        for edge in contains_edges {
            container_methods
                .entry(edge.from_uri.clone())
                .or_default()
                .push(edge.to_uri);
        }

        // Also build from Defines edges (parent defines child)
        let defines_edges = store.get_edges_by_kind(EdgeKind::Defines)?;
        for edge in defines_edges {
            container_methods
                .entry(edge.from_uri.clone())
                .or_default()
                .push(edge.to_uri);
        }

        Ok(Self {
            name_to_uris,
            qualified_name_to_uri,
            container_methods,
            file_symbols,
            symbols,
        })
    }

    /// Find symbols by name
    pub fn find_by_name(&self, name: &str) -> Vec<&SymbolUri> {
        self.name_to_uris
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Find symbols in a specific file
    pub fn find_in_file(&self, path: &str, name: &str) -> Option<&SymbolUri> {
        self.file_symbols.get(path).and_then(|uris| {
            uris.iter().find(|uri| {
                self.symbols.get(*uri).map(|s| s.name == name).unwrap_or(false)
            })
        })
    }

    /// Find methods in a container
    pub fn find_method(&self, container: &SymbolUri, name: &str) -> Option<&SymbolUri> {
        self.container_methods.get(container).and_then(|methods| {
            methods.iter().find(|uri| {
                self.symbols.get(*uri).map(|s| s.name == name).unwrap_or(false)
            })
        })
    }

    /// Find all containers (classes) with a method of this name
    pub fn find_containers_with_method(&self, method_name: &str) -> Vec<(&SymbolUri, &SymbolUri)> {
        let mut results = Vec::new();
        for (container_uri, methods) in &self.container_methods {
            for method_uri in methods {
                if let Some(symbol) = self.symbols.get(method_uri) {
                    if symbol.name == method_name {
                        results.push((container_uri, method_uri));
                    }
                }
            }
        }
        results
    }

    /// Get a symbol by URI
    pub fn get_symbol(&self, uri: &SymbolUri) -> Option<&Symbol> {
        self.symbols.get(uri)
    }

    /// Count total symbols indexed
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Global resolver for cross-file name resolution
pub struct Resolver<'a> {
    store: &'a SqliteStore,
    index: SymbolIndex,
}

impl<'a> Resolver<'a> {
    /// Create a new resolver
    pub fn new(store: &'a SqliteStore) -> Result<Self> {
        let index = SymbolIndex::build_from_store(store)?;
        Ok(Self { store, index })
    }

    /// Resolve all unresolved references in the store
    pub fn resolve_all(&self) -> Result<ResolverStats> {
        let unresolved = self.store.get_all_unresolved()?;
        let mut stats = ResolverStats::default();
        
        stats.total = unresolved.len();

        for unresolved_ref in &unresolved {
            match self.resolve_one(unresolved_ref) {
                ResolutionResult::Resolved { target_uri, confidence, strategy } => {
                    // Determine edge kind based on ref_kind
                    let edge_kind = if unresolved_ref.is_inheritance() {
                        EdgeKind::Inherits
                    } else {
                        EdgeKind::Calls
                    };

                    // Parse the from_uri
                    if let Ok(from_uri) = SymbolUri::parse(&unresolved_ref.from_uri) {
                        let edge = Edge::with_confidence(
                            from_uri,
                            target_uri,
                            edge_kind,
                            confidence,
                        );
                        self.store.insert_edge(&edge)?;
                        
                        // Delete the resolved reference
                        self.store.delete_unresolved(unresolved_ref.id)?;
                        
                        stats.resolved += 1;
                        match strategy {
                            ResolutionStrategy::LocalScope => stats.by_local += 1,
                            ResolutionStrategy::Import => stats.by_import += 1,
                            ResolutionStrategy::ContainerMethod => stats.by_container += 1,
                            ResolutionStrategy::Inheritance => stats.by_inheritance += 1,
                            ResolutionStrategy::GlobalName => stats.by_global += 1,
                        }
                    }
                }
                ResolutionResult::Ambiguous { candidates } => {
                    stats.ambiguous += 1;
                    tracing::debug!(
                        "Ambiguous reference: {} has {} candidates",
                        unresolved_ref.name,
                        candidates.len()
                    );
                }
                ResolutionResult::Unresolved => {
                    stats.unresolved += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Resolve a single reference using the resolution chain
    fn resolve_one(&self, unresolved: &PersistedUnresolvedReference) -> ResolutionResult {
        // Extract just the function name (handle cases like "obj.method" or "module.func")
        let name = extract_simple_name(&unresolved.name);
        let receiver = extract_receiver(&unresolved.name);

        // 1. Local scope: same file
        if let Some(uri) = self.index.find_in_file(&unresolved.file_path, &name) {
            return ResolutionResult::Resolved {
                target_uri: uri.clone(),
                confidence: ResolutionStrategy::LocalScope.confidence(),
                strategy: ResolutionStrategy::LocalScope,
            };
        }

        // 2. Container method (if receiver is known: receiver.method())
        if let Some(recv) = receiver {
            // Try to find the receiver as a known container
            let candidates = self.index.find_by_name(&recv);
            for container_uri in candidates {
                if let Some(method_uri) = self.index.find_method(container_uri, &name) {
                    return ResolutionResult::Resolved {
                        target_uri: method_uri.clone(),
                        confidence: ResolutionStrategy::ContainerMethod.confidence(),
                        strategy: ResolutionStrategy::ContainerMethod,
                    };
                }
            }
        }

        // 3. Global name match
        let global_matches: Vec<_> = self.index.find_by_name(&name);
        
        match global_matches.len() {
            0 => ResolutionResult::Unresolved,
            1 => ResolutionResult::Resolved {
                target_uri: global_matches[0].clone(),
                confidence: ResolutionStrategy::GlobalName.confidence(),
                strategy: ResolutionStrategy::GlobalName,
            },
            _ => {
                // Multiple matches - try to disambiguate
                
                // Prefer Callable over Container
                let callables: Vec<_> = global_matches
                    .iter()
                    .filter(|uri| {
                        self.index
                            .get_symbol(uri)
                            .map(|s| s.kind == SymbolKind::Callable)
                            .unwrap_or(false)
                    })
                    .collect();

                if callables.len() == 1 {
                    return ResolutionResult::Resolved {
                        target_uri: (*callables[0]).clone(),
                        confidence: ResolutionStrategy::GlobalName.confidence() * 0.9,
                        strategy: ResolutionStrategy::GlobalName,
                    };
                }

                // Still ambiguous
                ResolutionResult::Ambiguous {
                    candidates: global_matches.into_iter().cloned().collect(),
                }
            }
        }
    }
}

/// Extract the simple name from a potentially qualified name
/// e.g., "self.method" → "method", "module.func" → "func"
fn extract_simple_name(name: &str) -> String {
    name.rsplit('.').next().unwrap_or(name).to_string()
}

/// Extract the receiver from a qualified name
/// e.g., "self.method" → Some("self"), "func" → None
fn extract_receiver(name: &str) -> Option<String> {
    let parts: Vec<_> = name.split('.').collect();
    if parts.len() > 1 {
        Some(parts[0].to_string())
    } else {
        None
    }
}

/// Statistics from the resolution pass
#[derive(Debug, Default)]
pub struct ResolverStats {
    pub total: usize,
    pub resolved: usize,
    pub ambiguous: usize,
    pub unresolved: usize,
    pub by_local: usize,
    pub by_import: usize,
    pub by_container: usize,
    pub by_inheritance: usize,
    pub by_global: usize,
}

impl std::fmt::Display for ResolverStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Resolution Statistics:")?;
        writeln!(f, "  Total references: {}", self.total)?;
        writeln!(f, "  ✅ Resolved: {} ({:.1}%)", 
            self.resolved, 
            if self.total > 0 { self.resolved as f64 / self.total as f64 * 100.0 } else { 0.0 })?;
        writeln!(f, "  ⚠️  Ambiguous: {}", self.ambiguous)?;
        writeln!(f, "  ❌ Unresolved: {}", self.unresolved)?;
        writeln!(f, "  Resolution breakdown:")?;
        writeln!(f, "    Local scope: {}", self.by_local)?;
        writeln!(f, "    Import: {}", self.by_import)?;
        writeln!(f, "    Container method: {}", self.by_container)?;
        writeln!(f, "    Inheritance: {}", self.by_inheritance)?;
        writeln!(f, "    Global name: {}", self.by_global)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_name() {
        assert_eq!(extract_simple_name("func"), "func");
        assert_eq!(extract_simple_name("self.method"), "method");
        assert_eq!(extract_simple_name("module.submodule.func"), "func");
    }

    #[test]
    fn test_extract_receiver() {
        assert_eq!(extract_receiver("func"), None);
        assert_eq!(extract_receiver("self.method"), Some("self".to_string()));
        assert_eq!(extract_receiver("obj.method"), Some("obj".to_string()));
    }

    #[test]
    fn test_symbol_index_build() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        // Add some test symbols
        let sym1 = Symbol::new("repo", "src/main.py", SymbolKind::Callable, "foo", 1, 10, "def foo(): pass");
        let sym2 = Symbol::new("repo", "src/main.py", SymbolKind::Callable, "bar", 11, 20, "def bar(): pass");
        let sym3 = Symbol::new("repo", "src/other.py", SymbolKind::Callable, "foo", 1, 10, "def foo(): pass");
        
        store.insert_symbol(&sym1).unwrap();
        store.insert_symbol(&sym2).unwrap();
        store.insert_symbol(&sym3).unwrap();
        
        let index = SymbolIndex::build_from_store(&store).unwrap();
        
        assert_eq!(index.len(), 3);
        
        // Two symbols named "foo"
        let foos = index.find_by_name("foo");
        assert_eq!(foos.len(), 2);
        
        // One "foo" in main.py
        let foo_in_main = index.find_in_file("src/main.py", "foo");
        assert!(foo_in_main.is_some());
    }
}
