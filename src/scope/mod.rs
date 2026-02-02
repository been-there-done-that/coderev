//! Scope Graph - Universal name binding model
//!
//! Coderev uses a language-agnostic binding model where adapters emit facts
//! and the core builds a scope graph for resolution.

pub mod graph;
pub mod resolver;

pub use graph::{ScopeGraph, ScopeId, ScopeKind};
pub use resolver::NameResolver;
