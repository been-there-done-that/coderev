//! Language Adapter Framework
//!
//! Each language provides a Tree-sitter grammar and a `.scm` query pack
//! that maps AST nodes to UIR tags. The core engine never sees language-specific logic.

pub mod framework;
pub mod python;
pub mod javascript;

pub use framework::{LanguageAdapter, AdapterResult, ParsedFile, AdapterRegistry, default_registry};
