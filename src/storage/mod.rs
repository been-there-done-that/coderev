//! Storage Layer - SQLite-backed persistence
//!
//! System of record is SQLite with tables:
//! - symbols(uri, kind, name, path, doc, content)
//! - edges(from_uri, to_uri, type, confidence)
//! - embeddings(uri, vector)

pub mod schema;
pub mod sqlite;

pub use sqlite::SqliteStore;
