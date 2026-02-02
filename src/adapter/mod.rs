//! Language Adapter Framework
//!
//! Each language provides a Tree-sitter grammar and a `.scm` query pack
//! that maps AST nodes to UIR tags. The core engine never sees language-specific logic.
//!
//! For files without a language adapter, the chunker provides Coderev-style coverage.
//!
//! The preferred way to add language support is via QueryAdapter with .scm query files.

pub mod framework;
pub mod python;
pub mod javascript;
pub mod chunker;
pub mod query_adapter;

pub use framework::{LanguageAdapter, AdapterResult, ParsedFile, AdapterRegistry, default_registry};
pub use chunker::DocumentChunker;
pub use query_adapter::QueryAdapter;


