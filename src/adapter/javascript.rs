//! JavaScript language adapter
//!
//! Extracts symbols from JavaScript/TypeScript source files.

use crate::Result;
use crate::symbol::{Symbol, SymbolKind};
use super::framework::{LanguageAdapter, AdapterResult};

/// JavaScript language adapter
pub struct JavaScriptAdapter;

impl JavaScriptAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for JavaScriptAdapter {
    fn language_name(&self) -> &str {
        "JavaScript"
    }

    fn file_extensions(&self) -> &[&str] {
        &["js", "ts", "jsx", "tsx"]
    }

    fn parse_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult> {
        let mut result = AdapterResult::new();
        
        // Create root file symbol
        let file_symbol = Symbol::new(
            repo,
            path,
            SymbolKind::Namespace,
            path.rsplit('/').next().unwrap_or(path).split('.').next().unwrap_or(path),
            1,
            content.lines().count() as u32,
            content,
        );
        result.add_symbol(file_symbol);
        
        Ok(result)
    }
}
