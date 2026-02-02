//! Name Resolver - Resolves references to definitions
//!
//! Resolution algorithm:
//! 1. Walk outward through scopes
//! 2. Follow imports
//! 3. Match name → definition
//! 4. If 1 match → bind (confidence 1.0)
//! 5. If multiple or none → unresolved (for semantic resolution)

use crate::edge::{Edge, EdgeKind};
use crate::uri::SymbolUri;
use super::graph::{ScopeGraph, ScopeId};

/// Result of resolving a reference
#[derive(Debug, Clone)]
pub struct ResolvedBinding {
    /// The reference source
    pub from_uri: SymbolUri,
    /// The resolved target
    pub to_uri: SymbolUri,
    /// Confidence score (1.0 = unique match, <1.0 = ambiguous)
    pub confidence: f32,
}

impl ResolvedBinding {
    /// Convert to an edge
    pub fn to_edge(&self, kind: EdgeKind) -> Edge {
        Edge::with_confidence(
            self.from_uri.clone(),
            self.to_uri.clone(),
            kind,
            self.confidence,
        )
    }
}

/// Name resolver using scope graph
pub struct NameResolver<'a> {
    scope_graph: &'a ScopeGraph,
    /// External definitions (for cross-file resolution)
    external_definitions: &'a std::collections::HashMap<String, Vec<SymbolUri>>,
}

impl<'a> NameResolver<'a> {
    /// Create a new resolver
    pub fn new(
        scope_graph: &'a ScopeGraph,
        external_definitions: &'a std::collections::HashMap<String, Vec<SymbolUri>>,
    ) -> Self {
        Self {
            scope_graph,
            external_definitions,
        }
    }

    /// Resolve all unresolved references
    pub fn resolve_all(&self) -> Vec<ResolvedBinding> {
        self.scope_graph
            .unresolved_references()
            .iter()
            .filter_map(|reference| {
                self.resolve_reference(reference.scope, &reference.name)
                    .map(|(uri, confidence)| ResolvedBinding {
                        from_uri: reference.from_uri.clone(),
                        to_uri: uri,
                        confidence,
                    })
            })
            .collect()
    }

    /// Resolve a single reference
    pub fn resolve_reference(&self, scope: ScopeId, name: &str) -> Option<(SymbolUri, f32)> {
        // 1. Try local scope chain
        if let Some(uri) = self.scope_graph.lookup(scope, name) {
            return Some((uri.clone(), 1.0));
        }

        // 2. Try imports in scope chain
        for scope_id in self.scope_graph.scope_chain(scope) {
            for import in self.scope_graph.imports(scope_id) {
                // Check if this import brings in the name
                if import.symbols.is_empty() {
                    // Wildcard import - check external definitions for namespace.name
                    let qualified = format!("{}.{}", import.namespace, name);
                    if let Some(uris) = self.external_definitions.get(&qualified) {
                        return Some(self.pick_best_match(uris));
                    }
                } else if import.symbols.contains(&name.to_string()) {
                    // Specific import
                    let qualified = format!("{}.{}", import.namespace, name);
                    if let Some(uris) = self.external_definitions.get(&qualified) {
                        return Some(self.pick_best_match(uris));
                    }
                }

                // Check alias
                if import.alias.as_deref() == Some(name) {
                    if let Some(uris) = self.external_definitions.get(&import.namespace) {
                        return Some(self.pick_best_match(uris));
                    }
                }
            }
        }

        // 3. Try global external definitions
        if let Some(uris) = self.external_definitions.get(name) {
            return Some(self.pick_best_match(uris));
        }

        // 4. Unresolved
        None
    }

    /// Pick the best match from multiple candidates
    fn pick_best_match(&self, uris: &[SymbolUri]) -> (SymbolUri, f32) {
        if uris.len() == 1 {
            (uris[0].clone(), 1.0)
        } else {
            // Multiple matches - return first with reduced confidence
            // In the future, this could use embeddings for semantic ranking
            let confidence = 1.0 / uris.len() as f32;
            (uris[0].clone(), confidence)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::graph::{ScopeGraph, ScopeKind, Import};
    use crate::symbol::SymbolKind;
    use std::collections::HashMap;

    fn sample_uri(name: &str) -> SymbolUri {
        SymbolUri::new("repo", "test.py", SymbolKind::Callable, name, 1)
    }

    #[test]
    fn test_resolve_local() {
        let mut graph = ScopeGraph::new();
        let func_scope = graph.add_scope(ScopeId::root(), ScopeKind::Function);
        
        graph.add_definition(func_scope, "local_var", sample_uri("local_var"));

        let external = HashMap::new();
        let resolver = NameResolver::new(&graph, &external);

        let result = resolver.resolve_reference(func_scope, "local_var");
        assert!(result.is_some());
        let (uri, confidence) = result.unwrap();
        assert_eq!(uri.name, "local_var");
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn test_resolve_parent_scope() {
        let mut graph = ScopeGraph::new();
        let class_scope = graph.add_scope(ScopeId::root(), ScopeKind::Class);
        let method_scope = graph.add_scope(class_scope, ScopeKind::Function);
        
        graph.add_definition(class_scope, "class_method", sample_uri("class_method"));

        let external = HashMap::new();
        let resolver = NameResolver::new(&graph, &external);

        // Should find class_method from inside the method
        let result = resolver.resolve_reference(method_scope, "class_method");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.name, "class_method");
    }

    #[test]
    fn test_resolve_import() {
        let mut graph = ScopeGraph::new();
        
        graph.add_import(ScopeId::root(), Import {
            namespace: "os".to_string(),
            symbols: vec!["path".to_string()],
            alias: None,
            line: 0,
        });


        let mut external = HashMap::new();
        external.insert("os.path".to_string(), vec![sample_uri("path")]);

        let resolver = NameResolver::new(&graph, &external);

        let result = resolver.resolve_reference(ScopeId::root(), "path");
        assert!(result.is_some());
    }

    #[test]
    fn test_ambiguous_resolution() {
        let graph = ScopeGraph::new();
        
        let mut external = HashMap::new();
        external.insert("foo".to_string(), vec![
            sample_uri("foo1"),
            sample_uri("foo2"),
        ]);

        let resolver = NameResolver::new(&graph, &external);

        let result = resolver.resolve_reference(ScopeId::root(), "foo");
        assert!(result.is_some());
        let (_, confidence) = result.unwrap();
        assert!(confidence < 1.0); // Should have reduced confidence
    }
}
