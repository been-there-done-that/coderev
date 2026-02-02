//! # Coderev - Universal Code Intelligence Substrate
//!
//! Language-agnostic AST-native semantic code graph for the AI era.
//!
//! Coderev provides:
//! - Universal Intermediate Representation (UIR) for code symbols
//! - Language-agnostic symbol graph with deterministic and probabilistic edges
//! - Tree-sitter based parsing with pluggable language adapters
//! - SQLite-backed storage with optional vector search
//! - Query engine for code intelligence operations

pub mod uri;
pub mod symbol;
pub mod edge;
pub mod graph;
pub mod scope;
pub mod storage;
pub mod adapter;
pub mod query;
pub mod linker;


// Re-exports for convenient access
pub use uri::SymbolUri;
pub use symbol::{Symbol, SymbolKind};
pub use edge::{Edge, EdgeKind};
pub use graph::SymbolGraph;
pub use storage::SqliteStore;

/// Result type alias for Coderev operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for Coderev operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error("Storage error: {0}")]
    Storage(#[from] rusqlite::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Adapter error: {0}")]
    Adapter(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
}
