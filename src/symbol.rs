//! Symbol types - Universal Intermediate Representation (UIR)
//!
//! All languages are mapped into five universal symbol types:
//! - `Namespace`: File, module, package
//! - `Container`: Class, struct, trait, object
//! - `Callable`: Function, method, constructor, macro
//! - `Value`: Field, variable, constant
//! - `Document`: Chunked text document (for non-AST files)

use crate::{Error, Result};
use crate::uri::SymbolUri;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Universal symbol kinds - all languages map to these four types.
///
/// This abstraction allows the core engine to operate on code
/// from any language without language-specific logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    /// File, module, package - the organizational unit
    Namespace,
    /// Class, struct, trait, object - types that contain other symbols
    Container,
    /// Function, method, constructor, macro - executable code
    Callable,
    /// Field, variable, constant - data holders
    Value,
    /// Chunked text document - for non-AST files (SQL, YAML, Markdown)
    Document,
}

impl SymbolKind {
    /// Get the string representation of the symbol kind
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Namespace => "namespace",
            SymbolKind::Container => "container",
            SymbolKind::Callable => "callable",
            SymbolKind::Value => "value",
            SymbolKind::Document => "document",
        }
    }

    /// Get all symbol kinds
    pub fn all() -> &'static [SymbolKind] {
        &[
            SymbolKind::Namespace,
            SymbolKind::Container,
            SymbolKind::Callable,
            SymbolKind::Value,
            SymbolKind::Document,
        ]
    }
}

impl FromStr for SymbolKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "namespace" | "ns" | "module" | "package" | "file" => Ok(SymbolKind::Namespace),
            "container" | "class" | "struct" | "trait" | "interface" => Ok(SymbolKind::Container),
            "callable" | "function" | "method" | "fn" | "def" => Ok(SymbolKind::Callable),
            "value" | "field" | "variable" | "var" | "const" | "let" => Ok(SymbolKind::Value),
            "document" | "doc" | "chunk" | "text" => Ok(SymbolKind::Document),
            _ => Err(Error::InvalidUri(format!("Unknown symbol kind: {}", s))),
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A symbol in the code graph.
///
/// Represents any named entity in code: functions, classes, variables, etc.
/// All symbols have a URI that uniquely identifies them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Unique identifier for this symbol
    pub uri: SymbolUri,
    /// The kind of symbol (namespace, container, callable, value)
    pub kind: SymbolKind,
    /// Symbol name (just the identifier, not fully qualified)
    pub name: String,
    /// File path relative to repository root
    pub path: String,
    /// Starting line number (1-indexed)
    pub line_start: u32,
    /// Ending line number (1-indexed, inclusive)
    pub line_end: u32,
    /// Documentation string (docstring, JSDoc, etc.)
    pub doc: Option<String>,
    /// Function/method signature (for callables)
    pub signature: Option<String>,
    /// Full source code content of the symbol
    pub content: String,
}

impl Symbol {
    /// Create a new symbol with minimal required fields
    pub fn new(
        repo: impl Into<String>,
        path: impl Into<String>,
        kind: SymbolKind,
        name: impl Into<String>,
        line_start: u32,
        line_end: u32,
        content: impl Into<String>,
    ) -> Self {
        let repo = repo.into();
        let path = path.into();
        let name = name.into();
        let content = content.into();

        let uri = SymbolUri::new(&repo, &path, kind, &name, line_start);

        Self {
            uri,
            kind,
            name,
            path,
            line_start,
            line_end,
            doc: None,
            signature: None,
            content,
        }
    }

    /// Set the documentation string
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Set the signature
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// Get a short description for display
    pub fn short_description(&self) -> String {
        if let Some(sig) = &self.signature {
            format!("{} {} {}", self.kind, self.name, sig)
        } else {
            format!("{} {}", self.kind, self.name)
        }
    }

    /// Get the text to embed for semantic search
    pub fn embedding_text(&self) -> String {
        let mut parts = Vec::new();
        
        // Include signature if available
        if let Some(sig) = &self.signature {
            parts.push(sig.clone());
        } else {
            parts.push(self.name.clone());
        }

        // Include doc if available
        if let Some(doc) = &self.doc {
            parts.push(doc.clone());
        }

        // Include content (truncated for large symbols)
        let content_preview = if self.content.len() > 1000 {
            format!("{}...", &self.content[..1000])
        } else {
            self.content.clone()
        };
        parts.push(content_preview);

        parts.join("\n")
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl Eq for Symbol {}

impl std::hash::Hash for Symbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_roundtrip() {
        for kind in SymbolKind::all() {
            let s = kind.as_str();
            let parsed: SymbolKind = s.parse().unwrap();
            assert_eq!(*kind, parsed);
        }
    }

    #[test]
    fn test_symbol_kind_aliases() {
        assert_eq!(SymbolKind::from_str("class").unwrap(), SymbolKind::Container);
        assert_eq!(SymbolKind::from_str("function").unwrap(), SymbolKind::Callable);
        assert_eq!(SymbolKind::from_str("module").unwrap(), SymbolKind::Namespace);
        assert_eq!(SymbolKind::from_str("const").unwrap(), SymbolKind::Value);
    }

    #[test]
    fn test_symbol_creation() {
        let symbol = Symbol::new(
            "myrepo",
            "src/auth.py",
            SymbolKind::Callable,
            "validate_token",
            10,
            25,
            "def validate_token(token: str) -> bool:\n    pass",
        )
        .with_doc("Validates an authentication token")
        .with_signature("(token: str) -> bool");

        assert_eq!(symbol.name, "validate_token");
        assert_eq!(symbol.kind, SymbolKind::Callable);
        assert!(symbol.doc.is_some());
        assert!(symbol.signature.is_some());
    }
}
