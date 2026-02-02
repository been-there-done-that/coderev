//! Python language adapter (DEPRECATED)
//!
//! Extracts symbols from Python source files using tree-sitter.
//!
//! **DEPRECATED**: Use `QueryAdapter::python()` instead, which is declarative
//! and uses the same Tree-sitter queries as all other languages.

use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use crate::scope::graph::{ScopeId, ScopeKind, UnresolvedReference, Import};
use super::framework::{LanguageAdapter, AdapterResult};
use tree_sitter::{Parser, Node};

/// Python language adapter (DEPRECATED - use QueryAdapter::python() instead)
#[deprecated(since = "0.2.0", note = "Use QueryAdapter::python() instead")]
pub struct PythonAdapter {
    parser: std::sync::Mutex<Parser>,
}

impl PythonAdapter {
    /// Create a new Python adapter
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("Error loading Python grammar");
        
        Self {
            parser: std::sync::Mutex::new(parser),
        }
    }

    /// Extract function definition
    fn extract_function(&self, node: Node, source: &str, repo: &str, path: &str) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;
        
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        
        let content = node.utf8_text(source.as_bytes()).ok()?;
        
        // Extract signature
        let params_node = node.child_by_field_name("parameters");
        let return_type = node.child_by_field_name("return_type");
        
        let signature = match (params_node, return_type) {
            (Some(p), Some(r)) => {
                let params = p.utf8_text(source.as_bytes()).ok()?;
                let ret = r.utf8_text(source.as_bytes()).ok()?;
                Some(format!("{} -> {}", params, ret))
            }
            (Some(p), None) => {
                Some(p.utf8_text(source.as_bytes()).ok()?.to_string())
            }
            _ => None,
        };

        // Extract docstring
        let doc = self.extract_docstring(node, source);

        let mut symbol = Symbol::new(repo, path, SymbolKind::Callable, name, start_line, end_line, content);
        if let Some(sig) = signature {
            symbol = symbol.with_signature(sig);
        }
        if let Some(d) = doc {
            symbol = symbol.with_doc(d);
        }
        
        Some(symbol)
    }

    /// Extract class definition
    fn extract_class(&self, node: Node, source: &str, repo: &str, path: &str) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;
        
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        
        let content = node.utf8_text(source.as_bytes()).ok()?;
        let doc = self.extract_docstring(node, source);

        let mut symbol = Symbol::new(repo, path, SymbolKind::Container, name, start_line, end_line, content);
        if let Some(d) = doc {
            symbol = symbol.with_doc(d);
        }
        
        Some(symbol)
    }

    /// Extract docstring from a node
    fn extract_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Look for expression_statement with string as first child of body
        let body = node.child_by_field_name("body")?;
        let first_stmt = body.named_child(0)?;
        
        if first_stmt.kind() == "expression_statement" {
            let expr = first_stmt.named_child(0)?;
            if expr.kind() == "string" {
                let doc = expr.utf8_text(source.as_bytes()).ok()?;
                // Remove quotes
                let doc = doc.trim_matches('"').trim_matches('\'').trim();
                return Some(doc.to_string());
            }
        }
        None
    }

    /// Walk the AST and extract all symbols
    fn walk_tree(&self, node: Node, source: &str, repo: &str, path: &str, result: &mut AdapterResult, current_scope: ScopeId, parent_uri: Option<&SymbolUri>) {
        let mut cursor = node.walk();
        
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    self.process_function(child, source, repo, path, result, current_scope, parent_uri);
                }
                "decorated_definition" => {
                    // Extract function/class from inside the decorator
                    let mut inner_cursor = child.walk();
                    for inner_child in child.named_children(&mut inner_cursor) {
                        match inner_child.kind() {
                            "function_definition" => {
                                self.process_function(inner_child, source, repo, path, result, current_scope, parent_uri);
                            }
                            "class_definition" => {
                                self.process_class(inner_child, source, repo, path, result, current_scope, parent_uri);
                            }
                            _ => {}
                        }
                    }
                }
                "class_definition" => {
                    self.process_class(child, source, repo, path, result, current_scope, parent_uri);
                }
                "import_statement" | "import_from_statement" => {
                    self.extract_import(child, source, result, current_scope);
                }
                "call" => {
                    if let Some(parent) = parent_uri {
                        self.extract_call(child, source, parent, result, current_scope);
                    }
                    // Continue walking children of the call (e.g., arguments)
                    self.walk_tree(child, source, repo, path, result, current_scope, parent_uri);
                }
                _ => {
                    // Recurse into other nodes
                    self.walk_tree(child, source, repo, path, result, current_scope, parent_uri);
                }
            }
        }
    }

    fn process_function(&self, node: Node, source: &str, repo: &str, path: &str, result: &mut AdapterResult, scope: ScopeId, parent_uri: Option<&SymbolUri>) {
        if let Some(symbol) = self.extract_function(node, source, repo, path) {
            let uri = symbol.uri.clone();
            if let Some(parent) = parent_uri {
                result.add_edge(Edge::new(parent.clone(), uri.clone(), EdgeKind::Defines));
            }
            result.scope_graph.add_definition(scope, &symbol.name, uri.clone());
            let func_scope = result.scope_graph.add_scope(scope, ScopeKind::Function);
            result.add_symbol(symbol);
            if let Some(body) = node.child_by_field_name("body") {
                self.walk_tree(body, source, repo, path, result, func_scope, Some(&uri));
            }
        }
    }

    fn process_class(&self, node: Node, source: &str, repo: &str, path: &str, result: &mut AdapterResult, scope: ScopeId, parent_uri: Option<&SymbolUri>) {
        if let Some(symbol) = self.extract_class(node, source, repo, path) {
            let uri = symbol.uri.clone();
            if let Some(parent) = parent_uri {
                result.add_edge(Edge::new(parent.clone(), uri.clone(), EdgeKind::Defines));
            }
            result.scope_graph.add_definition(scope, &symbol.name, uri.clone());
            let class_scope = result.scope_graph.add_scope(scope, ScopeKind::Class);
            if let Some(bases) = node.child_by_field_name("superclasses") {
                self.extract_inheritance(bases, source, &uri, result, class_scope);
            }
            result.add_symbol(symbol);
            if let Some(body) = node.child_by_field_name("body") {
                self.walk_tree(body, source, repo, path, result, class_scope, Some(&uri));
            }
        }
    }

    /// Extract import statement
    fn extract_import(&self, node: Node, source: &str, result: &mut AdapterResult, scope: ScopeId) {
        match node.kind() {
            "import_statement" => {
                // import foo, bar
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                        if let Ok(name) = child.utf8_text(source.as_bytes()) {
                            result.scope_graph.add_import(scope, Import {
                                namespace: name.to_string(),
                                symbols: vec![],
                                alias: None,
                            });
                        }
                    }
                }
            }
            "import_from_statement" => {
                // from foo import bar, baz
                let module = node.child_by_field_name("module_name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("");
                
                let mut symbols = Vec::new();
                let mut cursor = node.walk();
                
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "dotted_name" && child.start_byte() > node.child_by_field_name("module_name").map(|n| n.end_byte()).unwrap_or(0) {
                        if let Ok(name) = child.utf8_text(source.as_bytes()) {
                            symbols.push(name.to_string());
                        }
                    }
                }
                
                result.scope_graph.add_import(scope, Import {
                    namespace: module.to_string(),
                    symbols,
                    alias: None,
                });
            }
            _ => {}
        }
    }

    /// Extract inheritance relationships
    fn extract_inheritance(&self, bases_node: Node, source: &str, class_uri: &SymbolUri, result: &mut AdapterResult, scope: ScopeId) {
        let mut cursor = bases_node.walk();
        for child in bases_node.named_children(&mut cursor) {
            if let Ok(base_name) = child.utf8_text(source.as_bytes()) {
                // Add unresolved reference for the base class
                result.scope_graph.add_reference(UnresolvedReference {
                    scope,
                    name: base_name.to_string(),
                    from_uri: class_uri.clone(),
                    line: child.start_position().row as u32 + 1,
                });
            }
        }
    }

    /// Extract call expression
    fn extract_call(&self, node: Node, source: &str, caller: &SymbolUri, result: &mut AdapterResult, scope: ScopeId) {
        if let Some(func_node) = node.child_by_field_name("function") {
            if let Ok(func_name) = func_node.utf8_text(source.as_bytes()) {
                // Add unresolved reference for the call
                result.scope_graph.add_reference(UnresolvedReference {
                    scope,
                    name: func_name.to_string(),
                    from_uri: caller.clone(),
                    line: node.start_position().row as u32 + 1,
                });
            }
        }
    }
}

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAdapter for PythonAdapter {
    fn language_name(&self) -> &str {
        "Python"
    }

    fn file_extensions(&self) -> &[&str] {
        &["py", "pyi"]
    }

    fn parse_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult> {
        let mut parser = self.parser.lock().map_err(|_| Error::Adapter("Failed to lock parser".to_string()))?;
        let tree = parser.parse(content, None)
            .ok_or_else(|| Error::Adapter("Failed to parse file".to_string()))?;
        
        let mut result = AdapterResult::new();
        
        // Create root file/namespace symbol
        let file_symbol = Symbol::new(
            repo,
            path,
            SymbolKind::Namespace,
            path.rsplit('/').next().unwrap_or(path).trim_end_matches(".py"),
            1,
            content.lines().count() as u32,
            content,
        );
        let file_uri = file_symbol.uri.clone();
        result.add_symbol(file_symbol);
        
        let root_scope = ScopeId::root();
        
        // Recursive walk
        self.walk_tree(tree.root_node(), content, repo, path, &mut result, root_scope, Some(&file_uri));
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_extraction() {
        let adapter = PythonAdapter::new();
        let content = r#"
def hello():
    print("hello")

class Foo:
    def bar(self):
        pass

def world():
    pass
"#;
        
        let result = adapter.parse_file("test", "test.py", content).unwrap();
        
        // Should have: namespace + 2 functions + 1 class + 1 method
        assert!(result.symbols.len() >= 4);
        
        let callables: Vec<_> = result.symbols.iter().filter(|s| s.kind == SymbolKind::Callable).collect();
        assert!(callables.len() >= 3); // hello, bar, world
    }
}
