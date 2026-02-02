//! SQLite storage implementation

use std::path::Path;
use rusqlite::{Connection, params, OptionalExtension};
use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use super::schema;

/// SQLite-backed storage for the symbol graph
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Open a database file (creates if doesn't exist)
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Initialize the database schema
    fn initialize_schema(&self) -> Result<()> {
        for stmt in schema::all_schema_statements() {
            self.conn.execute(stmt, [])?;
        }
        Ok(())
    }

    // ========== Symbol Operations ==========

    /// Insert or replace a symbol
    pub fn insert_symbol(&self, symbol: &Symbol) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO symbols (uri, kind, name, path, line_start, line_end, doc, signature, content)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                symbol.uri.to_uri_string(),
                symbol.kind.as_str(),
                symbol.name,
                symbol.path,
                symbol.line_start,
                symbol.line_end,
                symbol.doc,
                symbol.signature,
                symbol.content,
            ],
        )?;
        Ok(())
    }

    /// Get a symbol by URI
    pub fn get_symbol(&self, uri: &SymbolUri) -> Result<Option<Symbol>> {
        let uri_str = uri.to_uri_string();
        self.conn
            .query_row(
                "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE uri = ?1",
                [&uri_str],
                |row| self.row_to_symbol(row),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Find symbols by name
    pub fn find_symbols_by_name(&self, name: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE name = ?1"
        )?;
        
        let symbols = stmt
            .query_map([name], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }

    /// Find symbols by name pattern (LIKE query)
    pub fn find_symbols_by_name_pattern(&self, pattern: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE name LIKE ?1"
        )?;
        
        let symbols = stmt
            .query_map([pattern], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }

    /// Find all symbols in a file
    pub fn find_symbols_in_file(&self, path: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE path = ?1 ORDER BY line_start"
        )?;
        
        let symbols = stmt
            .query_map([path], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }

    /// Find symbols by kind
    pub fn find_symbols_by_kind(&self, kind: SymbolKind) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE kind = ?1"
        )?;
        
        let symbols = stmt
            .query_map([kind.as_str()], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }

    /// Count all symbols
    pub fn count_symbols(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Helper to convert a row to a Symbol
    fn row_to_symbol(&self, row: &rusqlite::Row) -> rusqlite::Result<Symbol> {
        let uri_str: String = row.get(0)?;
        let kind_str: String = row.get(1)?;
        
        let uri = SymbolUri::parse(&uri_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        
        let kind: SymbolKind = kind_str.parse().map_err(|e: Error| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Symbol {
            uri,
            kind,
            name: row.get(2)?,
            path: row.get(3)?,
            line_start: row.get(4)?,
            line_end: row.get(5)?,
            doc: row.get(6)?,
            signature: row.get(7)?,
            content: row.get(8)?,
        })
    }

    // ========== Edge Operations ==========

    /// Insert or replace an edge
    pub fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO edges (from_uri, to_uri, kind, confidence)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                edge.from_uri.to_uri_string(),
                edge.to_uri.to_uri_string(),
                edge.kind.as_str(),
                edge.confidence,
            ],
        )?;
        Ok(())
    }

    /// Get edges from a symbol
    pub fn get_edges_from(&self, uri: &SymbolUri) -> Result<Vec<Edge>> {
        let uri_str = uri.to_uri_string();
        let mut stmt = self.conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence FROM edges WHERE from_uri = ?1"
        )?;
        
        let edges = stmt
            .query_map([uri_str], |row| self.row_to_edge(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(edges)
    }

    /// Get edges to a symbol (reverse lookup)
    pub fn get_edges_to(&self, uri: &SymbolUri) -> Result<Vec<Edge>> {
        let uri_str = uri.to_uri_string();
        let mut stmt = self.conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence FROM edges WHERE to_uri = ?1"
        )?;
        
        let edges = stmt
            .query_map([uri_str], |row| self.row_to_edge(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(edges)
    }

    /// Get edges by kind
    pub fn get_edges_by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence FROM edges WHERE kind = ?1"
        )?;
        
        let edges = stmt
            .query_map([kind.as_str()], |row| self.row_to_edge(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(edges)
    }

    /// Count all edges
    pub fn count_edges(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Helper to convert a row to an Edge
    fn row_to_edge(&self, row: &rusqlite::Row) -> rusqlite::Result<Edge> {
        let from_str: String = row.get(0)?;
        let to_str: String = row.get(1)?;
        let kind_str: String = row.get(2)?;
        let confidence: f32 = row.get(3)?;

        let from_uri = SymbolUri::parse(&from_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        
        let to_uri = SymbolUri::parse(&to_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let kind: EdgeKind = kind_str.parse().map_err(|e: Error| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Edge::with_confidence(from_uri, to_uri, kind, confidence))
    }

    // ========== Embedding Operations ==========

    /// Insert or replace an embedding
    pub fn insert_embedding(&self, uri: &SymbolUri, vector: &[f32]) -> Result<()> {
        let uri_str = uri.to_uri_string();
        let blob: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (uri, vector) VALUES (?1, ?2)",
            params![uri_str, blob],
        )?;
        Ok(())
    }

    /// Get an embedding by URI
    pub fn get_embedding(&self, uri: &SymbolUri) -> Result<Option<Vec<f32>>> {
        let uri_str = uri.to_uri_string();
        
        let result: Option<Vec<u8>> = self.conn
            .query_row(
                "SELECT vector FROM embeddings WHERE uri = ?1",
                [uri_str],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result.map(|blob| {
            blob.chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        }))
    }

    /// Count embeddings
    pub fn count_embeddings(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // ========== Bulk Operations ==========

    /// Begin a transaction for bulk operations
    pub fn begin_transaction(&mut self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    /// Commit a transaction
    pub fn commit(&mut self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback(&mut self) -> Result<()> {
        self.conn.execute("ROLLBACK", [])?;
        Ok(())
    }

    /// Delete all data (for re-indexing)
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM embeddings", [])?;
        self.conn.execute("DELETE FROM edges", [])?;
        self.conn.execute("DELETE FROM symbols", [])?;
        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DbStats> {
        Ok(DbStats {
            symbols: self.count_symbols()?,
            edges: self.count_edges()?,
            embeddings: self.count_embeddings()?,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DbStats {
    pub symbols: usize,
    pub edges: usize,
    pub embeddings: usize,
}

impl std::fmt::Display for DbStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Database Statistics:")?;
        writeln!(f, "  Symbols: {}", self.symbols)?;
        writeln!(f, "  Edges: {}", self.edges)?;
        writeln!(f, "  Embeddings: {}", self.embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_symbol(name: &str, line: u32) -> Symbol {
        Symbol::new("repo", "src/main.py", SymbolKind::Callable, name, line, line + 5, "def test(): pass")
    }

    #[test]
    fn test_symbol_crud() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let symbol = sample_symbol("my_func", 10);
        let uri = symbol.uri.clone();
        
        store.insert_symbol(&symbol).unwrap();
        
        let retrieved = store.get_symbol(&uri).unwrap().unwrap();
        assert_eq!(retrieved.name, "my_func");
        assert_eq!(retrieved.line_start, 10);
    }

    #[test]
    fn test_find_by_name() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        store.insert_symbol(&sample_symbol("foo", 10)).unwrap();
        store.insert_symbol(&sample_symbol("bar", 20)).unwrap();
        store.insert_symbol(&sample_symbol("foo", 30)).unwrap();
        
        let foos = store.find_symbols_by_name("foo").unwrap();
        assert_eq!(foos.len(), 2);
    }

    #[test]
    fn test_edge_crud() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let caller = sample_symbol("caller", 10);
        let callee = sample_symbol("callee", 20);
        
        let caller_uri = caller.uri.clone();
        let callee_uri = callee.uri.clone();
        
        store.insert_symbol(&caller).unwrap();
        store.insert_symbol(&callee).unwrap();
        
        let edge = Edge::new(caller_uri.clone(), callee_uri.clone(), EdgeKind::Calls);
        store.insert_edge(&edge).unwrap();
        
        let edges_from = store.get_edges_from(&caller_uri).unwrap();
        assert_eq!(edges_from.len(), 1);
        assert_eq!(edges_from[0].kind, EdgeKind::Calls);
        
        let edges_to = store.get_edges_to(&callee_uri).unwrap();
        assert_eq!(edges_to.len(), 1);
    }

    #[test]
    fn test_embedding_crud() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let symbol = sample_symbol("with_embedding", 10);
        let uri = symbol.uri.clone();
        
        store.insert_symbol(&symbol).unwrap();
        
        let vector = vec![0.1, 0.2, 0.3, 0.4];
        store.insert_embedding(&uri, &vector).unwrap();
        
        let retrieved = store.get_embedding(&uri).unwrap().unwrap();
        assert_eq!(retrieved.len(), 4);
        assert!((retrieved[0] - 0.1).abs() < 0.001);
    }
}
