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

    /// Search symbols by content (searches within the code/document content)
    pub fn search_content(&self, query: &str, kind: Option<SymbolKind>, limit: usize) -> Result<Vec<Symbol>> {
        let pattern = format!("%{}%", query);
        
        let sql = if kind.is_some() {
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content 
             FROM symbols 
             WHERE (content LIKE ?1 OR name LIKE ?1 OR doc LIKE ?1) AND kind = ?2
             LIMIT ?3"
        } else {
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content 
             FROM symbols 
             WHERE content LIKE ?1 OR name LIKE ?1 OR doc LIKE ?1
             LIMIT ?2"
        };
        
        let mut stmt = self.conn.prepare(sql)?;
        
        let symbols: Vec<Symbol> = if let Some(k) = kind {
            stmt.query_map(params![pattern, k.as_str(), limit as i64], |row| self.row_to_symbol(row))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map(params![pattern, limit as i64], |row| self.row_to_symbol(row))?
                .filter_map(|r| r.ok())
                .collect()
        };
        
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

    /// Search for symbols by vector similarity
    pub fn search_by_vector(&self, query_vector: &[f32], limit: usize) -> Result<Vec<(Symbol, f32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, vector FROM embeddings"
        )?;
        
        // Fetch all candidates
        let candidates = stmt.query_map([], |row| {
            let uri_str: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let vector: Vec<f32> = blob.chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok((uri_str, vector))
        })?;

        let mut scored_results = Vec::new();
        for candidate in candidates {
            if let Ok((uri_str, vector)) = candidate {
                let score = self.cosine_similarity(query_vector, &vector);
                scored_results.push((uri_str, score));
            }
        }

        // Sort by score descending
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top N and fetch symbols
        let mut final_results = Vec::new();
        for (uri_str, score) in scored_results.into_iter().take(limit) {
            let uri = SymbolUri::parse(&uri_str)?;
            if let Some(symbol) = self.get_symbol(&uri)? {
                final_results.push((symbol, score));
            }
        }

        Ok(final_results)
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
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
            unresolved: self.count_unresolved()?,
        })
    }

    // ========== Unresolved Reference Operations ==========

    /// Insert an unresolved reference
    pub fn insert_unresolved(&self, unresolved: &PersistedUnresolvedReference) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO unresolved_references (from_uri, name, receiver, scope_id, file_path, line, ref_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                unresolved.from_uri,
                unresolved.name,
                unresolved.receiver,
                unresolved.scope_id,
                unresolved.file_path,
                unresolved.line,
                unresolved.ref_kind,
            ],
        )?;
        Ok(())
    }

    /// Get unresolved references by name
    pub fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<PersistedUnresolvedReference>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind FROM unresolved_references WHERE name = ?1"
        )?;
        
        let refs = stmt
            .query_map([name], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Get all unresolved references
    pub fn get_all_unresolved(&self) -> Result<Vec<PersistedUnresolvedReference>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind FROM unresolved_references"
        )?;
        
        let refs = stmt
            .query_map([], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Get unresolved references for a specific file
    pub fn get_unresolved_in_file(&self, file_path: &str) -> Result<Vec<PersistedUnresolvedReference>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind FROM unresolved_references WHERE file_path = ?1"
        )?;
        
        let refs = stmt
            .query_map([file_path], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Delete an unresolved reference by ID
    pub fn delete_unresolved(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM unresolved_references WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Clear all unresolved references
    pub fn clear_unresolved(&self) -> Result<()> {
        self.conn.execute("DELETE FROM unresolved_references", [])?;
        Ok(())
    }

    /// Count unresolved references
    pub fn count_unresolved(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM unresolved_references", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Helper to convert a row to PersistedUnresolvedReference
    fn row_to_unresolved(&self, row: &rusqlite::Row) -> rusqlite::Result<PersistedUnresolvedReference> {
        Ok(PersistedUnresolvedReference {
            id: row.get(0)?,
            from_uri: row.get(1)?,
            name: row.get(2)?,
            receiver: row.get(3)?,
            scope_id: row.get(4)?,
            file_path: row.get(5)?,
            line: row.get(6)?,
            ref_kind: row.get(7)?,
        })
    }
}

/// Persisted unresolved reference (stored in DB)
#[derive(Debug, Clone)]
pub struct PersistedUnresolvedReference {
    pub id: i64,
    pub from_uri: String,
    pub name: String,
    pub receiver: Option<String>,
    pub scope_id: i64,
    pub file_path: String,
    pub line: u32,
    pub ref_kind: String,
}

impl PersistedUnresolvedReference {
    /// Create a new unresolved reference for insertion (id will be set by DB)
    pub fn new(
        from_uri: String,
        name: String,
        receiver: Option<String>,
        scope_id: i64,
        file_path: String,
        line: u32,
        ref_kind: &str,
    ) -> Self {
        Self {
            id: 0, // Set by DB
            from_uri,
            name,
            receiver,
            scope_id,
            file_path,
            line,
            ref_kind: ref_kind.to_string(),
        }
    }

    /// Check if this is a call reference
    pub fn is_call(&self) -> bool {
        self.ref_kind == "call"
    }

    /// Check if this is an inheritance reference
    pub fn is_inheritance(&self) -> bool {
        self.ref_kind == "inherits"
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DbStats {
    pub symbols: usize,
    pub edges: usize,
    pub embeddings: usize,
    pub unresolved: usize,
}

impl std::fmt::Display for DbStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Database Statistics:")?;
        writeln!(f, "  Symbols: {}", self.symbols)?;
        writeln!(f, "  Edges: {}", self.edges)?;
        writeln!(f, "  Embeddings: {}", self.embeddings)?;
        writeln!(f, "  Unresolved: {}", self.unresolved)
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
