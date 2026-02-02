//! Python language adapter
//!
//! Extracts symbols from Python source files using tree-sitter.

use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use crate::scope::graph::{ScopeGraph, ScopeId, ScopeKind, Import, UnresolvedReference};
use super::framework::{LanguageAdapter, AdapterResult};
use tree_sitter::{Parser, Query, QueryCursor, Node};

/// Python language adapter
pub struct PythonAdapter {
    parser: std::sync::Mutex<Parser>,
}

impl PythonAdapter {
    /// Create a new Python adapter
    pub fn new() -> Self {
        let mut parser = Parser::new();
        // Note: In a real implementation, you'd load the Python grammar here
        // parser.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
        
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
                    if let Some(symbol) = self.extract_function(child, source, repo, path) {
                        let uri = symbol.uri.clone();
                        
                        // Add defines edge from parent (namespace or class)
                        if let Some(parent) = parent_uri {
                            result.add_edge(Edge::new(parent.clone(), uri.clone(), EdgeKind::Defines));
                        }
                        
                        // Add definition to scope
                        result.scope_graph.add_definition(current_scope, &symbol.name, uri.clone());
                        
                        // Create function scope
                        let func_scope = result.scope_graph.add_scope(current_scope, ScopeKind::Function);
                        
                        result.add_symbol(symbol);
                        
                        // Process function body
                        if let Some(body) = child.child_by_field_name("body") {
                            self.walk_tree(body, source, repo, path, result, func_scope, Some(&uri));
                        }
                    }
                }
                "class_definition" => {
                    if let Some(symbol) = self.extract_class(child, source, repo, path) {
                        let uri = symbol.uri.clone();
                        
                        // Add defines edge from parent
                        if let Some(parent) = parent_uri {
                            result.add_edge(Edge::new(parent.clone(), uri.clone(), EdgeKind::Defines));
                        }
                        
                        // Add definition to scope
                        result.scope_graph.add_definition(current_scope, &symbol.name, uri.clone());
                        
                        // Create class scope
                        let class_scope = result.scope_graph.add_scope(current_scope, ScopeKind::Class);
                        
                        // Check for inheritance
                        if let Some(bases) = child.child_by_field_name("superclasses") {
                            self.extract_inheritance(bases, source, &uri, result, class_scope);
                        }
                        
                        result.add_symbol(symbol);
                        
                        // Process class body
                        if let Some(body) = child.child_by_field_name("body") {
                            self.walk_tree(body, source, repo, path, result, class_scope, Some(&uri));
                        }
                    }
                }
                "import_statement" | "import_from_statement" => {
                    self.extract_import(child, source, result, current_scope);
                }
                "call" => {
                    if let Some(parent) = parent_uri {
                        self.extract_call(child, source, parent, result, current_scope);
                    }
                }
                _ => {
                    // Recurse into other nodes
                    self.walk_tree(child, source, repo, path, result, current_scope, parent_uri);
                }
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
        // For now, return empty result - full implementation requires tree-sitter-python grammar
        // which needs to be added as a dependency
        let mut result = AdapterResult::new();
        
        // Create namespace symbol for the file
        let namespace = Symbol::new(
            repo,
            path,
            SymbolKind::Namespace,
            path.rsplit('/').next().unwrap_or(path).trim_end_matches(".py"),
            1,
            content.lines().count() as u32,
            content,
        );
        let ns_uri = namespace.uri.clone();
        result.add_symbol(namespace);
        
        // TODO: Full tree-sitter parsing
        // For now, we'll use a simple regex-based extraction as a placeholder
        self.simple_extract(repo, path, content, &mut result, &ns_uri);
        
        Ok(result)
    }
}

impl PythonAdapter {
    /// Simple extraction without full tree-sitter (placeholder)
    fn simple_extract(&self, repo: &str, path: &str, content: &str, result: &mut AdapterResult, ns_uri: &SymbolUri) {
        let lines: Vec<&str> = content.lines().collect();
        let mut current_class: Option<(String, u32)> = None;
        
        for (i, line) in lines.iter().enumerate() {
            let line_num = (i + 1) as u32;
            let trimmed = line.trim();
            
            // Simple function detection
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                if let Some(name) = Self::extract_name_from_def(trimmed) {
                    let end_line = Self::find_block_end(&lines, i);
                    let block_content = lines[i..=end_line].join("\n");
                    
                    let kind = if current_class.is_some() {
                        SymbolKind::Callable // method
                    } else {
                        SymbolKind::Callable // function
                    };
                    
                    let symbol = Symbol::new(repo, path, kind, &name, line_num, end_line as u32 + 1, block_content);
                    result.add_edge(Edge::new(ns_uri.clone(), symbol.uri.clone(), EdgeKind::Defines));
                    result.add_symbol(symbol);
                }
            }
            
            // Simple class detection
            if trimmed.starts_with("class ") {
                if let Some(name) = Self::extract_name_from_class(trimmed) {
                    let end_line = Self::find_block_end(&lines, i);
                    let block_content = lines[i..=end_line].join("\n");
                    
                    let symbol = Symbol::new(repo, path, SymbolKind::Container, &name, line_num, end_line as u32 + 1, block_content);
                    result.add_edge(Edge::new(ns_uri.clone(), symbol.uri.clone(), EdgeKind::Defines));
                    result.add_symbol(symbol);
                    
                    current_class = Some((name, end_line as u32 + 1));
                }
            }
            
            // Reset class context when we pass its end
            if let Some((_, end)) = current_class {
                if line_num > end {
                    current_class = None;
                }
            }
        }
    }

    fn extract_name_from_def(line: &str) -> Option<String> {
        let line = line.trim();
        let after_def = if line.starts_with("async def ") {
            &line[10..]
        } else if line.starts_with("def ") {
            &line[4..]
        } else {
            return None;
        };
        after_def.split('(').next().map(|s| s.trim().to_string())
    }

    fn extract_name_from_class(line: &str) -> Option<String> {
        let trimmed = line.trim_start_matches("class ");
        trimmed.split(['(', ':']).next().map(|s| s.trim().to_string())
    }

    fn find_block_end(lines: &[&str], start: usize) -> usize {
        if start >= lines.len() {
            return start;
        }
        
        let start_indent = lines[start].len() - lines[start].trim_start().len();
        
        for i in (start + 1)..lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            if indent <= start_indent && !line.trim().is_empty() {
                return i.saturating_sub(1);
            }
        }
        
        lines.len() - 1
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
