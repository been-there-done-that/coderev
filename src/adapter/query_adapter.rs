//! Query-based Language Adapter
//!
//! This module provides a declarative way to define language support using
//! Tree-sitter query files (.scm). Instead of writing Rust code for each
//! language, you define S-expression queries that capture symbols.
//!
//! Query capture naming convention:
//! - `callable.name` → function/method name
//! - `callable.def` → full function definition
//! - `container.name` → class/struct name
//! - `container.def` → full class definition
//! - `value.name` → variable/constant name
//! - `call.name` → called function name
//! - `call.receiver` → object being called on (for method calls)
//! - `import.module` → imported module
//! - `inherits.base` → base class name
//! - `docstring.function` → function docstring

use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use crate::scope::graph::{ScopeId, UnresolvedReference, Import};
use super::framework::AdapterResult;
use tree_sitter::{Parser, Query, QueryCursor, Language, Node};
use std::collections::HashMap;

/// A language adapter that uses Tree-sitter queries for extraction
pub struct QueryAdapter {
    language: Language,
    language_name: String,
    extensions: Vec<String>,
    query: Query,
}

impl QueryAdapter {
    /// Create a new query adapter from a language and query string
    pub fn new(
        language: Language,
        language_name: &str,
        extensions: &[&str],
        query_source: &str,
    ) -> Result<Self> {
        let query = Query::new(&language, query_source)
            .map_err(|e| Error::Adapter(format!("Query parse error: {}", e)))?;

        Ok(Self {
            language,
            language_name: language_name.to_string(),
            extensions: extensions.iter().map(|s| s.to_string()).collect(),
            query,
        })
    }

    /// Create a Python query adapter with embedded queries
    pub fn python() -> Result<Self> {
        let language = tree_sitter_python::LANGUAGE.into();
        let query_source = include_str!("../../queries/python.scm");
        Self::new(language, "Python", &["py", "pyi"], query_source)
    }

    /// Create a JavaScript query adapter with embedded queries
    pub fn javascript() -> Result<Self> {
        let language = tree_sitter_javascript::LANGUAGE.into();
        let query_source = include_str!("../../queries/javascript.scm");
        Self::new(language, "JavaScript", &["js", "jsx", "mjs", "cjs"], query_source)
    }

    /// Create a Rust query adapter with embedded queries
    pub fn rust() -> Result<Self> {
        let language = tree_sitter_rust::LANGUAGE.into();
        let query_source = include_str!("../../queries/rust.scm");
        Self::new(language, "Rust", &["rs"], query_source)
    }

    /// Create a Go query adapter with embedded queries
    pub fn go() -> Result<Self> {
        let language = tree_sitter_go::LANGUAGE.into();
        let query_source = include_str!("../../queries/go.scm");
        Self::new(language, "Go", &["go"], query_source)
    }

    /// Get all supported query adapters
    pub fn all() -> Vec<Result<Self>> {
        vec![
            Self::python(),
            Self::javascript(),
            Self::rust(),
            Self::go(),
        ]
    }

    /// Parse a file and extract symbols using queries
    pub fn parse_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)
            .map_err(|e| Error::Adapter(format!("Failed to set language: {}", e)))?;

        let tree = parser.parse(content, None)
            .ok_or_else(|| Error::Adapter("Failed to parse file".to_string()))?;

        let root = tree.root_node();
        let source_bytes = content.as_bytes();

        let mut result = AdapterResult::new();
        let mut cursor = QueryCursor::new();

        // Create the namespace symbol for the file
        let file_name = path.rsplit('/').next().unwrap_or(path);
        let module_name = file_name.strip_suffix(".py").unwrap_or(file_name);
        let namespace_uri = SymbolUri::new(repo, path, SymbolKind::Namespace, module_name, 1);
        let namespace_symbol = Symbol::new(
            repo, path, SymbolKind::Namespace, module_name, 1, 
            root.end_position().row as u32 + 1, ""
        );
        result.add_symbol(namespace_symbol);

        // Track captured symbols for building edges
        let mut symbols: HashMap<String, SymbolUri> = HashMap::new();
        let mut current_class: Option<(String, SymbolUri)> = None;
        let mut current_function: Option<(String, SymbolUri)> = None;

        // Process all query matches
        let mut matches = cursor.matches(&self.query, root, source_bytes);
        for query_match in matches {
            let mut captures: HashMap<&str, Node> = HashMap::new();
            
            for capture in query_match.captures {
                let capture_name = self.query.capture_names()[capture.index as usize];
                captures.insert(capture_name, capture.node);
            }

            // Process callable (function/method) definitions
            if let Some(name_node) = captures.get("callable.name") {
                let name = name_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let def_node = captures.get("callable.def").copied().unwrap_or(*name_node);
                
                let start_line = def_node.start_position().row as u32 + 1;
                let end_line = def_node.end_position().row as u32 + 1;
                let fn_content = def_node.utf8_text(source_bytes).unwrap_or("").to_string();
                
                // Build signature from params and return type
                let params = captures.get("callable.params")
                    .map(|n| n.utf8_text(source_bytes).unwrap_or("()"))
                    .unwrap_or("()");
                let return_type = captures.get("callable.return_type")
                    .map(|n| format!(" -> {}", n.utf8_text(source_bytes).unwrap_or("")))
                    .unwrap_or_default();
                let signature = format!("{}{}", params, return_type);

                let uri = SymbolUri::new(repo, path, SymbolKind::Callable, &name, start_line);
                let mut symbol = Symbol::new(repo, path, SymbolKind::Callable, &name, start_line, end_line, &fn_content)
                    .with_signature(signature);

                // Get docstring if available
                if let Some(body_node) = captures.get("callable.body") {
                    if let Some(docstring) = self.extract_docstring(*body_node, source_bytes) {
                        symbol = symbol.with_doc(docstring);
                    }
                }

                result.add_symbol(symbol);
                
                // Add Defines edge from namespace
                result.add_edge(Edge::new(namespace_uri.clone(), uri.clone(), EdgeKind::Defines));
                
                // If inside a class, add Contains edge
                if let Some((_, ref class_uri)) = current_class {
                    result.add_edge(Edge::new(class_uri.clone(), uri.clone(), EdgeKind::Contains));
                }

                symbols.insert(name.clone(), uri.clone());
                current_function = Some((name, uri));
            }

            // Process method definitions (inside classes)
            if let Some(name_node) = captures.get("method.name") {
                let name = name_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let def_node = captures.get("method.def").copied().unwrap_or(*name_node);
                
                let start_line = def_node.start_position().row as u32 + 1;
                let end_line = def_node.end_position().row as u32 + 1;
                let fn_content = def_node.utf8_text(source_bytes).unwrap_or("").to_string();
                
                let params = captures.get("method.params")
                    .map(|n| n.utf8_text(source_bytes).unwrap_or("()"))
                    .unwrap_or("()");
                let return_type = captures.get("method.return_type")
                    .map(|n| format!(" -> {}", n.utf8_text(source_bytes).unwrap_or("")))
                    .unwrap_or_default();
                let signature = format!("{}{}", params, return_type);

                let uri = SymbolUri::new(repo, path, SymbolKind::Callable, &name, start_line);
                let symbol = Symbol::new(repo, path, SymbolKind::Callable, &name, start_line, end_line, &fn_content)
                    .with_signature(signature);

                result.add_symbol(symbol);
                
                // Add Contains edge from current class if any
                if let Some((_, ref class_uri)) = current_class {
                    result.add_edge(Edge::new(class_uri.clone(), uri.clone(), EdgeKind::Contains));
                }

                symbols.insert(name.clone(), uri.clone());
                current_function = Some((name, uri));
            }

            // Process container (class) definitions
            if let Some(name_node) = captures.get("container.name") {
                let name = name_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let def_node = captures.get("container.def").copied().unwrap_or(*name_node);
                
                let start_line = def_node.start_position().row as u32 + 1;
                let end_line = def_node.end_position().row as u32 + 1;
                let class_content = def_node.utf8_text(source_bytes).unwrap_or("").to_string();

                let uri = SymbolUri::new(repo, path, SymbolKind::Container, &name, start_line);
                let mut symbol = Symbol::new(repo, path, SymbolKind::Container, &name, start_line, end_line, &class_content);

                // Get docstring
                if let Some(body_node) = captures.get("container.body") {
                    if let Some(docstring) = self.extract_docstring(*body_node, source_bytes) {
                        symbol = symbol.with_doc(docstring);
                    }
                }

                result.add_symbol(symbol);
                
                // Add Defines edge from namespace
                result.add_edge(Edge::new(namespace_uri.clone(), uri.clone(), EdgeKind::Defines));

                symbols.insert(name.clone(), uri.clone());
                current_class = Some((name, uri));
            }

            // Process calls
            if let Some(call_name_node) = captures.get("call.name") {
                let call_name = call_name_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let call_line = call_name_node.start_position().row as u32 + 1;
                
                let receiver = captures.get("call.receiver")
                    .map(|n| n.utf8_text(source_bytes).unwrap_or("").to_string());

                // Create unresolved reference
                if let Some((_, ref caller_uri)) = current_function {
                    let full_name = if let Some(ref recv) = receiver {
                        format!("{}.{}", recv, call_name)
                    } else {
                        call_name.clone()
                    };
                    
                    result.scope_graph.add_reference(UnresolvedReference {
                        scope: ScopeId(0),
                        name: full_name,
                        line: call_line,
                        from_uri: caller_uri.clone(),
                    });
                }
            }

            // Process inheritance
            if let Some(base_node) = captures.get("inherits.base") {
                let base_name = base_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let line = base_node.start_position().row as u32 + 1;
                
                if let Some((_, ref class_uri)) = current_class {
                    result.scope_graph.add_reference(UnresolvedReference {
                        scope: ScopeId(0),
                        name: base_name,
                        line,
                        from_uri: class_uri.clone(),
                    });
                }
            }

            // Process imports
            if let Some(module_node) = captures.get("import.module") {
                let module = module_node.utf8_text(source_bytes).unwrap_or("").to_string();
                
                let line = module_node.start_position().row as u32 + 1;
                result.scope_graph.add_import(
                    ScopeId(0),
                    Import {
                        namespace: module,
                        symbols: vec![],
                        alias: None,
                        line,
                    },
                );

            }
            
            if let Some(module_node) = captures.get("import.from_module") {
                let from_module = module_node.utf8_text(source_bytes).unwrap_or("").to_string();
                let name = captures.get("import.name")
                    .map(|n| n.utf8_text(source_bytes).unwrap_or("").to_string());
                let alias = captures.get("import.alias")
                    .map(|n| n.utf8_text(source_bytes).unwrap_or("").to_string());
                
                let import_symbols = name.map(|n| vec![n]).unwrap_or_default();
                
                let line = module_node.start_position().row as u32 + 1;
                result.scope_graph.add_import(
                    ScopeId(0),
                    Import {
                        namespace: from_module,
                        symbols: import_symbols,
                        alias,
                        line,
                    },
                );

            }
        }

        // Suppress unused variable warning
        let _ = symbols;

        Ok(result)
    }

    /// Extract docstring from a block node
    fn extract_docstring(&self, block: Node, source: &[u8]) -> Option<String> {
        // First child of a block that is an expression_statement containing a string
        if let Some(first_child) = block.named_child(0) {
            if first_child.kind() == "expression_statement" {
                if let Some(string_node) = first_child.named_child(0) {
                    if string_node.kind() == "string" {
                        let text = string_node.utf8_text(source).ok()?;
                        // Strip quotes
                        let trimmed = text.trim_matches(|c| c == '"' || c == '\'');
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }

    /// Get language name
    pub fn language_name(&self) -> &str {
        &self.language_name
    }

    /// Get supported file extensions
    pub fn file_extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Check if this adapter can handle a file extension
    pub fn can_handle_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e == ext)
    }
}

/// Implement LanguageAdapter trait for QueryAdapter
impl super::framework::LanguageAdapter for QueryAdapter {
    fn language_name(&self) -> &str {
        &self.language_name
    }

    fn file_extensions(&self) -> &[&str] {
        // Return static slices based on language
        match self.language_name.as_str() {
            "Python" => &["py", "pyi"],
            "JavaScript" => &["js", "jsx", "mjs", "cjs"],
            "Rust" => &["rs"],
            "Go" => &["go"],
            _ => &[],
        }
    }

    fn parse_file(&self, repo: &str, path: &str, content: &str) -> crate::Result<super::framework::AdapterResult> {
        QueryAdapter::parse_file(self, repo, path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_query_adapter() {
        let adapter = QueryAdapter::python().expect("Failed to create Python adapter");
        
        let source = r#"
def hello(name: str) -> str:
    """Say hello to someone."""
    return f"Hello, {name}!"

class Greeter:
    """A greeter class."""
    
    def greet(self, name: str) -> str:
        return hello(name)
"#;

        let result = adapter.parse_file("test", "hello.py", source)
            .expect("Failed to parse");

        // Should have: namespace, hello function, Greeter class, greet method
        assert!(result.symbols.len() >= 3, "Expected at least 3 symbols, got {}", result.symbols.len());
        
        // Check that we found the function
        let hello_fn = result.symbols.iter()
            .find(|s| s.name == "hello" && s.kind == SymbolKind::Callable);
        assert!(hello_fn.is_some(), "Should find hello function");
        
        // Check that we found the class
        let greeter_class = result.symbols.iter()
            .find(|s| s.name == "Greeter" && s.kind == SymbolKind::Container);
        assert!(greeter_class.is_some(), "Should find Greeter class");
    }

    #[test]
    fn test_rust_query_adapter() {
        let adapter = QueryAdapter::rust().expect("Failed to create Rust adapter");
        
        let source = r#"
fn main() {
    hello();
}

fn hello() {
    println!("Hello");
}

struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
"#;

        let result = adapter.parse_file("test", "main.rs", source)
            .expect("Failed to parse");

        assert!(result.symbols.len() >= 4, "Expected at least 4 symbols, got {}", result.symbols.len());
        
        let main_fn = result.symbols.iter().find(|s| s.name == "main");
        assert!(main_fn.is_some());
        
        let hello_fn = result.symbols.iter().find(|s| s.name == "hello");
        assert!(hello_fn.is_some());
        
        let point_struct = result.symbols.iter().find(|s| s.name == "Point");
        assert!(point_struct.is_some());
    }

    #[test]
    fn test_javascript_query_adapter() {
        let adapter = QueryAdapter::javascript().expect("Failed to create JS adapter");
        
        let source = r#"
import { sum } from "./math.js";

export function greet(name) {
    console.log("Hello " + name);
}

const result = sum(1, 2);
"#;

        let result = adapter.parse_file("test", "app.js", source)
            .expect("Failed to parse");

        assert!(result.symbols.len() >= 2, "Expected at least 2 symbols, got {}", result.symbols.len());
        
        let greet_fn = result.symbols.iter().find(|s| s.name == "greet");
        assert!(greet_fn.is_some());
    }
}
