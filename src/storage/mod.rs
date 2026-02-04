//! Storage Layer - SQLite-backed persistence
//!
//! System of record is SQLite with tables:
//! - symbols(uri, kind, name, path, doc, content)
//! - edges(from_uri, to_uri, type, confidence)
//! - embeddings(uri, vector)
//! - unresolved_references(from_uri, name, receiver, scope_id, file_path, line, ref_kind)

pub mod schema;
pub mod sqlite;

pub use sqlite::{SqliteStore, PersistedUnresolvedReference, DbStats, Import};

