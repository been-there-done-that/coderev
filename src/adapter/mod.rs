//! Language Adapter Framework
//!
//! Each language provides a Tree-sitter grammar and a `.scm` query pack
//! that maps AST nodes to UIR tags. The core engine never sees language-specific logic.
//!
//! For files without a language adapter, the chunker provides Coderev-style coverage.

pub mod framework;
pub mod python;
pub mod javascript;
pub mod chunker;

pub use framework::{LanguageAdapter, AdapterResult, ParsedFile, AdapterRegistry, default_registry};
pub use chunker::DocumentChunker;

