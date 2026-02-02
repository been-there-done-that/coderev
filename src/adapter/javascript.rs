//! JavaScript language adapter
//!
//! Extracts symbols from JavaScript/TypeScript source files.

use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use crate::scope::graph::{ScopeGraph, ScopeId, ScopeKind, Import, UnresolvedReference};
use super::framework::{LanguageAdapter, AdapterResult};

/// JavaScript language adapter
pub struct JavaScriptAdapter {
    // Tree-sitter parser would go here
}

impl JavaScriptAdapter {
    /// Create a new JavaScript adapter
    pub fn new() -> Self {
        Self {}
    }

    /// Simple extraction without full tree-sitter
    fn simple_extract(&self, repo: &str, path: &str, content: &str, result: &mut AdapterResult, ns_uri: &SymbolUri) {
        let lines: Vec<&str> = content.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            let line_num = (i + 1) as u32;
            let trimmed = line.trim();
            
            // Function declarations: function name(...
            if trimmed.starts_with("function ") {
                if let Some(name) = Self::extract_function_name(trimmed) {
                    let end_line = Self::find_brace_end(&lines, i);
                    let block_content = lines[i..=end_line].join("\n");
                    
                    let symbol = Symbol::new(repo, path, SymbolKind::Callable, &name, line_num, end_line as u32 + 1, block_content);
                    result.add_edge(Edge::new(ns_uri.clone(), symbol.uri.clone(), EdgeKind::Defines));
                    result.add_symbol(symbol);
                }
            }
            
            // Arrow functions: const name = (...) =>
            if (trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("var ")) 
                && trimmed.contains("=>") {
                if let Some(name) = Self::extract_arrow_function_name(trimmed) {
                    let end_line = if trimmed.contains('{') {
                        Self::find_brace_end(&lines, i)
                    } else {
                        i
                    };
                    let block_content = lines[i..=end_line].join("\n");
                    
                    let symbol = Symbol::new(repo, path, SymbolKind::Callable, &name, line_num, end_line as u32 + 1, block_content);
                    result.add_edge(Edge::new(ns_uri.clone(), symbol.uri.clone(), EdgeKind::Defines));
                    result.add_symbol(symbol);
                }
            }
            
            // Class declarations
            if trimmed.starts_with("class ") {
                if let Some(name) = Self::extract_class_name(trimmed) {
                    let end_line = Self::find_brace_end(&lines, i);
                    let block_content = lines[i..=end_line].join("\n");
                    
                    let symbol = Symbol::new(repo, path, SymbolKind::Container, &name, line_num, end_line as u32 + 1, block_content);
                    result.add_edge(Edge::new(ns_uri.clone(), symbol.uri.clone(), EdgeKind::Defines));
                    result.add_symbol(symbol);
                }
            }
            
            // Export statements
            if trimmed.starts_with("export ") {
                // Mark as export - in full implementation, we'd track what's exported
            }
        }
    }

    fn extract_function_name(line: &str) -> Option<String> {
        // function name(...)
        let after_fn = line.trim_start_matches("function ");
        let name = after_fn.split(['(', ' ']).next()?;
        if name.is_empty() || name == "(" {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn extract_arrow_function_name(line: &str) -> Option<String> {
        // const name = (...) => or const name = async (...) =>
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() >= 2 {
            let decl = parts[0].trim();
            let name = decl.split_whitespace().last()?;
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        None
    }

    fn extract_class_name(line: &str) -> Option<String> {
        // class Name or class Name extends Base
        let after_class = line.trim_start_matches("class ");
        let name = after_class.split([' ', '{']).next()?;
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn find_brace_end(lines: &[&str], start: usize) -> usize {
        let mut depth = 0;
        let mut found_first = false;
        
        for i in start..lines.len() {
            for ch in lines[i].chars() {
                if ch == '{' {
                    depth += 1;
                    found_first = true;
                } else if ch == '}' {
                    depth -= 1;
                    if found_first && depth == 0 {
                        return i;
                    }
                }
            }
        }
        
        lines.len().saturating_sub(1)
    }
}

impl Default for JavaScriptAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAdapter for JavaScriptAdapter {
    fn language_name(&self) -> &str {
        "JavaScript"
    }

    fn file_extensions(&self) -> &[&str] {
        &["js", "jsx", "ts", "tsx", "mjs", "cjs"]
    }

    fn parse_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult> {
        let mut result = AdapterResult::new();
        
        // Create namespace symbol for the file
        let file_name = path.rsplit('/').next().unwrap_or(path);
        let module_name = file_name.split('.').next().unwrap_or(file_name);
        
        let namespace = Symbol::new(
            repo,
            path,
            SymbolKind::Namespace,
            module_name,
            1,
            content.lines().count() as u32,
            content,
        );
        let ns_uri = namespace.uri.clone();
        result.add_symbol(namespace);
        
        // Extract symbols
        self.simple_extract(repo, path, content, &mut result, &ns_uri);
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_extraction() {
        let adapter = JavaScriptAdapter::new();
        let content = r#"
function hello() {
    console.log("hello");
}

const greet = (name) => {
    return `Hello, ${name}`;
};

class Greeter {
    sayHello() {
        console.log("hello");
    }
}
"#;
        
        let result = adapter.parse_file("test", "test.js", content).unwrap();
        
        // Should have: namespace + function + arrow function + class
        assert!(result.symbols.len() >= 4);
        
        let callables: Vec<_> = result.symbols.iter().filter(|s| s.kind == SymbolKind::Callable).collect();
        assert!(callables.len() >= 2); // hello, greet
        
        let containers: Vec<_> = result.symbols.iter().filter(|s| s.kind == SymbolKind::Container).collect();
        assert!(containers.len() >= 1); // Greeter
    }

    #[test]
    fn test_arrow_function_name_extraction() {
        assert_eq!(JavaScriptAdapter::extract_arrow_function_name("const foo = () => {}"), Some("foo".to_string()));
        assert_eq!(JavaScriptAdapter::extract_arrow_function_name("let bar = async (x) => x * 2"), Some("bar".to_string()));
    }
}
