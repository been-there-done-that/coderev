//! Scope Graph data structure for name binding
//!
//! The scope graph tracks:
//! - Scope hierarchy (parent/child relationships)
//! - Definitions within each scope
//! - References that need resolution
//! - Import relationships

use std::collections::HashMap;
use crate::uri::SymbolUri;

/// Unique identifier for a scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

impl ScopeId {
    /// Create a root scope ID
    pub fn root() -> Self {
        Self(0)
    }
}

/// The kind of scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Module/file level scope
    Module,
    /// Class/struct scope
    Class,
    /// Function/method scope
    Function,
    /// Block scope (if, for, etc.)
    Block,
}

/// An import statement within a scope
#[derive(Debug, Clone)]
pub struct Import {
    /// The namespace being imported from
    pub namespace: String,
    /// Specific symbols imported (empty = import all)
    pub symbols: Vec<String>,
    /// Alias for the import (e.g., `import foo as bar`)
    pub alias: Option<String>,
    /// Line number of the import
    pub line: u32,
}

/// A reference to a name that needs resolution
#[derive(Debug, Clone)]
pub struct UnresolvedReference {
    /// The scope where the reference occurs
    pub scope: ScopeId,
    /// The name being referenced
    pub name: String,
    /// The symbol containing this reference
    pub from_uri: SymbolUri,
    /// Line number of the reference
    pub line: u32,
}

/// Scope graph for tracking name definitions and references
#[derive(Debug, Default)]
pub struct ScopeGraph {
    /// Next scope ID to assign
    next_id: u32,
    /// Scope hierarchy (child → parent)
    parents: HashMap<ScopeId, ScopeId>,
    /// Scope kind
    kinds: HashMap<ScopeId, ScopeKind>,
    /// Definitions: (scope, name) → URI
    definitions: HashMap<(ScopeId, String), SymbolUri>,
    /// Imports per scope
    imports: HashMap<ScopeId, Vec<Import>>,
    /// Unresolved references
    references: Vec<UnresolvedReference>,
}

impl ScopeGraph {
    /// Create a new scope graph with a root module scope
    pub fn new() -> Self {
        let mut graph = Self::default();
        // Create the root scope
        graph.kinds.insert(ScopeId::root(), ScopeKind::Module);
        graph.next_id = 1;
        graph
    }

    /// Create a new child scope
    pub fn add_scope(&mut self, parent: ScopeId, kind: ScopeKind) -> ScopeId {
        let id = ScopeId(self.next_id);
        self.next_id += 1;
        self.parents.insert(id, parent);
        self.kinds.insert(id, kind);
        id
    }

    /// Add a definition to a scope
    pub fn add_definition(&mut self, scope: ScopeId, name: impl Into<String>, uri: SymbolUri) {
        self.definitions.insert((scope, name.into()), uri);
    }

    /// Add an import to a scope
    pub fn add_import(&mut self, scope: ScopeId, import: Import) {
        self.imports.entry(scope).or_default().push(import);
    }

    /// Add an unresolved reference
    pub fn add_reference(&mut self, reference: UnresolvedReference) {
        self.references.push(reference);
    }

    /// Get the parent of a scope
    pub fn parent(&self, scope: ScopeId) -> Option<ScopeId> {
        self.parents.get(&scope).copied()
    }

    /// Get the kind of a scope
    pub fn kind(&self, scope: ScopeId) -> Option<ScopeKind> {
        self.kinds.get(&scope).copied()
    }

    /// Look up a definition in a scope (not walking parents)
    pub fn lookup_local(&self, scope: ScopeId, name: &str) -> Option<&SymbolUri> {
        self.definitions.get(&(scope, name.to_string()))
    }

    /// Look up a definition walking up the scope chain
    pub fn lookup(&self, scope: ScopeId, name: &str) -> Option<&SymbolUri> {
        let mut current = Some(scope);
        while let Some(s) = current {
            if let Some(uri) = self.lookup_local(s, name) {
                return Some(uri);
            }
            current = self.parent(s);
        }
        None
    }

    /// Get imports for a scope
    pub fn imports(&self, scope: ScopeId) -> &[Import] {
        self.imports.get(&scope).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all unresolved references
    pub fn unresolved_references(&self) -> &[UnresolvedReference] {
        &self.references
    }

    /// Get all definitions in a scope
    pub fn definitions_in_scope(&self, scope: ScopeId) -> Vec<(&String, &SymbolUri)> {
        self.definitions
            .iter()
            .filter(|((s, _), _)| *s == scope)
            .map(|((_, name), uri)| (name, uri))
            .collect()
    }

    /// Get scope chain from a scope up to root
    pub fn scope_chain(&self, scope: ScopeId) -> Vec<ScopeId> {
        let mut chain = vec![scope];
        let mut current = scope;
        while let Some(parent) = self.parent(current) {
            chain.push(parent);
            current = parent;
        }
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolKind;

    fn sample_uri(name: &str) -> SymbolUri {
        SymbolUri::new("repo", "test.py", SymbolKind::Callable, name, 1)
    }

    #[test]
    fn test_scope_hierarchy() {
        let mut graph = ScopeGraph::new();
        
        let class_scope = graph.add_scope(ScopeId::root(), ScopeKind::Class);
        let method_scope = graph.add_scope(class_scope, ScopeKind::Function);

        assert_eq!(graph.parent(method_scope), Some(class_scope));
        assert_eq!(graph.parent(class_scope), Some(ScopeId::root()));
        assert_eq!(graph.parent(ScopeId::root()), None);
    }

    #[test]
    fn test_definition_lookup() {
        let mut graph = ScopeGraph::new();
        
        let class_scope = graph.add_scope(ScopeId::root(), ScopeKind::Class);
        let method_scope = graph.add_scope(class_scope, ScopeKind::Function);

        // Define at root
        graph.add_definition(ScopeId::root(), "global_func", sample_uri("global_func"));
        
        // Define in class
        graph.add_definition(class_scope, "method", sample_uri("method"));

        // Local lookup
        assert!(graph.lookup_local(class_scope, "method").is_some());
        assert!(graph.lookup_local(class_scope, "global_func").is_none());

        // Chain lookup from method scope should find both
        assert!(graph.lookup(method_scope, "method").is_some());
        assert!(graph.lookup(method_scope, "global_func").is_some());
    }

    #[test]
    fn test_scope_chain() {
        let mut graph = ScopeGraph::new();
        
        let s1 = graph.add_scope(ScopeId::root(), ScopeKind::Class);
        let s2 = graph.add_scope(s1, ScopeKind::Function);
        let s3 = graph.add_scope(s2, ScopeKind::Block);

        let chain = graph.scope_chain(s3);
        assert_eq!(chain, vec![s3, s2, s1, ScopeId::root()]);
    }
}
