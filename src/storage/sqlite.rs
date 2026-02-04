//! SQLite storage implementation

use std::path::Path;
use std::sync::Mutex;
use rusqlite::{Connection, params, OptionalExtension};
use serde::Serialize;
use crate::{Result, Error};
use crate::edge::{Edge, EdgeKind};
use crate::symbol::{Symbol, SymbolKind};
use crate::uri::SymbolUri;
use super::schema;

/// SQLite-backed storage for the symbol graph
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open a database file (creates if doesn't exist)
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn: Mutex::new(conn) };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn: Mutex::new(conn) };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Helper to lock the connection and handle errors
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| {
            Error::Storage(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Mutex lock failed: {}", e)),
            ))
        })
    }

    /// Initialize the database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        for stmt in schema::all_schema_statements() {
            conn.execute(stmt, [])?;
        }
        // Apply performance pragmas after schema init
        for pragma in schema::PERFORMANCE_PRAGMAS {
            conn.execute(pragma, []).ok(); // Ignore errors (some pragmas may not apply)
        }
        Ok(())
    }

    // ========== Symbol Operations ==========

    /// Insert or replace a symbol
    pub fn insert_symbol(&self, symbol: &Symbol) -> Result<()> {
        self.lock_conn()?.execute(
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
        self.lock_conn()?
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let words: Vec<String> = query
            .split_whitespace()
            .map(|w| format!("%{}%", w))
            .collect();
        
        if words.is_empty() {
            return self.get_recent_symbols(limit);
        }

        let mut conditions = Vec::new();
        for i in 1..=words.len() {
            conditions.push(format!("(content LIKE ?{} OR name LIKE ?{} OR doc LIKE ?{})", i, i, i));
        }
        
        let words_count = words.len();
        let mut sql = format!(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content 
             FROM symbols 
             WHERE {}",
            conditions.join(" AND ")
        );

        if kind.is_some() {
            sql.push_str(&format!(" AND kind = ?{}", words_count + 1));
            sql.push_str(&format!(" LIMIT ?{}", words_count + 2));
        } else {
            sql.push_str(&format!(" LIMIT ?{}", words_count + 1));
        }

        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        
        let mut params: Vec<rusqlite::types::Value> = words.into_iter()
            .map(|w| rusqlite::types::Value::Text(w))
            .collect();

        if let Some(k) = kind {
            params.push(rusqlite::types::Value::Text(k.as_str().to_string()));
            params.push(rusqlite::types::Value::Integer(limit as i64));
        } else {
            params.push(rusqlite::types::Value::Integer(limit as i64));
        }

        let mut symbols: Vec<Symbol> = stmt.query_map(rusqlite::params_from_iter(params), |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        // Sort by file boost descending
        symbols.sort_by(|a, b| {
            let boost_a = self.get_file_boost(&a.path);
            let boost_b = self.get_file_boost(&b.path);
            boost_b.partial_cmp(&boost_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        Ok(symbols)
    }

    /// Get recent symbols (limit N)
    pub fn get_recent_symbols(&self, limit: usize) -> Result<Vec<Symbol>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content 
             FROM symbols 
             ORDER BY path ASC, line_start ASC
             LIMIT ?1"
        )?;
        
        let symbols = stmt
            .query_map([limit], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
            
        Ok(symbols)
    }

    /// Count all symbols
    pub fn count_symbols(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Find all symbols that don't have an embedding yet
    pub fn find_symbols_without_embeddings(&self) -> Result<Vec<Symbol>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT uri, kind, name, path, line_start, line_end, doc, signature, content 
            FROM symbols 
            WHERE uri NOT IN (SELECT uri FROM embeddings)
            "#
        )?;
        
        let symbols = stmt
            .query_map([], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
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
        self.lock_conn()?.execute(
            r#"
            INSERT OR REPLACE INTO edges (from_uri, to_uri, kind, confidence, resolution_mode)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                edge.from_uri.to_uri_string(),
                edge.to_uri.to_uri_string(),
                edge.kind.as_str(),
                edge.confidence,
                edge.resolution_mode,
            ],

        )?;
        Ok(())
    }

    /// Get edges from a symbol
    pub fn get_edges_from(&self, uri: &SymbolUri) -> Result<Vec<Edge>> {
        let uri_str = uri.to_uri_string();
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence, resolution_mode FROM edges WHERE from_uri = ?1"

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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence, resolution_mode FROM edges WHERE to_uri = ?1"

        )?;
        
        let edges = stmt
            .query_map([uri_str], |row| self.row_to_edge(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(edges)
    }

    /// Get edges by kind
    pub fn get_edges_by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT from_uri, to_uri, kind, confidence, resolution_mode FROM edges WHERE kind = ?1"

        )?;
        
        let edges = stmt
            .query_map([kind.as_str()], |row| self.row_to_edge(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(edges)
    }

    /// Count all edges
    pub fn count_edges(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Helper to convert a row to an Edge
    fn row_to_edge(&self, row: &rusqlite::Row) -> rusqlite::Result<Edge> {
        let from_str: String = row.get(0)?;
        let to_str: String = row.get(1)?;
        let kind_str: String = row.get(2)?;
        let confidence: f32 = row.get(3)?;
        let resolution_mode: String = row.get(4)?;

        let from_uri = SymbolUri::parse(&from_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        
        let to_uri = SymbolUri::parse(&to_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let kind: EdgeKind = kind_str.parse().map_err(|e: Error| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let mut edge = Edge::with_confidence(from_uri, to_uri, kind, confidence);
        edge.resolution_mode = resolution_mode;
        Ok(edge)
    }


    // ========== Embedding Operations ==========

    /// Insert or replace an embedding
    pub fn insert_embedding(&self, uri: &SymbolUri, vector: &[f32]) -> Result<()> {
        let uri_str = uri.to_uri_string();
        let blob: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        
        self.lock_conn()?.execute(
            "INSERT OR REPLACE INTO embeddings (uri, vector) VALUES (?1, ?2)",
            params![uri_str, blob],
        )?;
        Ok(())
    }

    /// Get an embedding by URI
    pub fn get_embedding(&self, uri: &SymbolUri) -> Result<Option<Vec<f32>>> {
        let uri_str = uri.to_uri_string();
        
        let result: Option<Vec<u8>> = self.lock_conn()?
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

    /// Insert a callsite embedding
    pub fn insert_callsite_embedding(&self, reference_id: i64, vector: &[f32]) -> Result<()> {
        let blob: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        
        self.lock_conn()?.execute(
            "INSERT OR REPLACE INTO callsite_embeddings (reference_id, vector) VALUES (?1, ?2)",
            params![reference_id, blob],
        )?;
        Ok(())
    }

    /// Count callsite embeddings
    pub fn count_callsite_embeddings(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row("SELECT COUNT(*) FROM callsite_embeddings", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Clear callsite embeddings
    pub fn clear_callsite_embeddings(&self) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM callsite_embeddings", [])?;
        Ok(())
    }



    /// Count embeddings
    pub fn count_embeddings(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get all embeddings for caching
    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT uri, vector FROM embeddings")?;
        
        let it = stmt.query_map([], |row| {
            let uri: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let vector: Vec<f32> = blob.chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok((uri, vector))
        })?;
        
        let mut results = Vec::new();
        for res in it {
            results.push(res?);
        }
        Ok(results)
    }

    /// Search for symbols by vector similarity
    pub fn search_by_vector(&self, query_vector: &[f32], limit: usize) -> Result<Vec<(Symbol, f32)>> {
        let scored_results = {
            let conn = self.lock_conn()?;
            let mut stmt = conn.prepare(
                "SELECT e.uri, e.vector, s.path FROM embeddings e JOIN symbols s ON e.uri = s.uri"
            )?;
            
            // Fetch all candidates
            let candidates = stmt.query_map([], |row| {
                let uri_str: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let path: String = row.get(2)?;
                let vector: Vec<f32> = blob.chunks(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok((uri_str, vector, path))
            })?;
            
            let mut results = Vec::new();
            for candidate in candidates {
                if let Ok((uri_str, vector, path)) = candidate {
                    let base_score = self.cosine_similarity(query_vector, &vector);
                    let boost = self.get_file_boost(&path);
                    let boosted_score = base_score * boost;
                    
                    results.push((uri_str, boosted_score));
                }
            }
            
            // Sort by boosted score descending
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            results
        };
        
        // Take top N and fetch symbols (outside the lock)
        let mut final_results = Vec::new();
        for (uri_str, score) in scored_results.into_iter().take(limit) {
            let uri = SymbolUri::parse(&uri_str)?;
            if let Some(symbol) = self.get_symbol(&uri)? {
                final_results.push((symbol, score));
            }
        }
        Ok(final_results)
    }

    fn get_file_boost(&self, path: &str) -> f32 {
        let path_lower = path.to_lowercase();
        
        // Priority extensions (Code)
        if path_lower.ends_with(".rs") || path_lower.ends_with(".py") || 
           path_lower.ends_with(".js") || path_lower.ends_with(".ts") ||
           path_lower.ends_with(".tsx") || path_lower.ends_with(".jsx") ||
           path_lower.ends_with(".go") || path_lower.ends_with(".c") ||
           path_lower.ends_with(".cpp") || path_lower.ends_with(".h") {
            return 1.2;
        }
        
        // Documentation
        if path_lower.ends_with(".md") || path_lower.ends_with(".txt") || 
           path_lower.contains("readme") {
            return 0.9;
        }
        
        // Config/Data
        if path_lower.ends_with(".json") || path_lower.ends_with(".yaml") || 
           path_lower.ends_with(".yml") || path_lower.ends_with(".toml") {
            return 0.7;
        }
        
        // Assets/Build/Lock files (Low priority)
        if path_lower.ends_with(".svg") || path_lower.ends_with(".png") || 
           path_lower.ends_with(".lock") || path_lower.ends_with(".sum") ||
           path_lower.contains("node_modules") || path_lower.contains("target/") {
            return 0.5;
        }
        
        1.0 // Default
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
    pub fn begin_transaction(&self) -> Result<()> {
        self.lock_conn()?.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    /// Commit a transaction
    pub fn commit(&self) -> Result<()> {
        self.lock_conn()?.execute("COMMIT", [])?;
        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback(&self) -> Result<()> {
        self.lock_conn()?.execute("ROLLBACK", [])?;
        Ok(())
    }

    /// Delete all data (for re-indexing)
    pub fn clear_all(&self) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM embeddings", [])?;
        self.lock_conn()?.execute("DELETE FROM edges", [])?;
        self.lock_conn()?.execute("DELETE FROM symbols", [])?;
        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DbStats> {
        Ok(DbStats {
            symbols: self.count_symbols()?,
            edges: self.count_edges()?,
            embeddings: self.count_embeddings()?,
            unresolved: self.count_unresolved()?,
            imports: self.count_imports()?,
            callsite_embeddings: self.count_callsite_embeddings()?,
        })

    }

    // ========== Import Operations ==========

    /// Insert an import
    pub fn insert_import(&self, file_path: &str, alias: Option<&str>, target_namespace: &str, line: Option<u32>) -> Result<()> {
        self.lock_conn()?.execute(
            "INSERT INTO imports (file_path, alias, target_namespace, line) VALUES (?1, ?2, ?3, ?4)",
            params![file_path, alias, target_namespace, line],
        )?;
        Ok(())
    }

    /// Get imports for a file
    pub fn get_imports_for_file(&self, file_path: &str) -> Result<Vec<Import>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, file_path, alias, target_namespace, line FROM imports WHERE file_path = ?1"
        )?;
        
        let imports = stmt
            .query_map([file_path], |row| {
                Ok(Import {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    alias: row.get(2)?,
                    target_namespace: row.get(3)?,
                    line: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(imports)
    }

    /// Clear all imports
    pub fn clear_imports(&self) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM imports", [])?;
        Ok(())
    }

    /// Count imports
    pub fn count_imports(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row("SELECT COUNT(*) FROM imports", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // ========== Ambiguous Reference Operations ==========

    /// Insert an ambiguous reference candidate
    pub fn insert_ambiguous_reference(&self, reference_id: i64, candidate_uri: &str, score: f32) -> Result<()> {
        self.lock_conn()?.execute(
            "INSERT INTO ambiguous_references (reference_id, candidate_uri, score) VALUES (?1, ?2, ?3)",
            params![reference_id, candidate_uri, score],
        )?;
        Ok(())
    }

    /// Clear all ambiguous references
    pub fn clear_ambiguous_references(&self) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM ambiguous_references", [])?;
        Ok(())
    }

    // ========== Advanced Symbol Lookups for Linker ==========

    /// Find symbols by name within a specific file (Local Resolution)
    pub fn find_symbols_by_name_and_file(&self, name: &str, file_path: &str) -> Result<Vec<Symbol>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE name = ?1 AND path = ?2"
        )?;
        
        let symbols = stmt
            .query_map([name, file_path], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }

    /// Find symbols by name where the URI suggests it belongs to a namespace (Import Resolution)
    /// This uses LIKE query on URI: codescope://%/{target_namespace}%
    pub fn find_symbols_by_name_and_container_pattern(&self, name: &str, namespace_pattern: &str) -> Result<Vec<Symbol>> {
        let pattern = format!("%/{}%", namespace_pattern);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT uri, kind, name, path, line_start, line_end, doc, signature, content FROM symbols WHERE name = ?1 AND uri LIKE ?2"
        )?;
        
        let symbols = stmt
            .query_map([name, &pattern], |row| self.row_to_symbol(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(symbols)
    }


    // ========== Unresolved Reference Operations ==========

    /// Insert an unresolved reference
    pub fn insert_unresolved(&self, unresolved: &PersistedUnresolvedReference) -> Result<()> {
        self.lock_conn()?.execute(
            r#"
            INSERT INTO unresolved_references (from_uri, name, receiver, scope_id, file_path, line, ref_kind, is_external)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                unresolved.from_uri,
                unresolved.name,
                unresolved.receiver,
                unresolved.scope_id,
                unresolved.file_path,
                unresolved.line,
                unresolved.ref_kind,
                unresolved.is_external,
            ],
        )?;
        Ok(())
    }

    /// Get unresolved references by name
    pub fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<PersistedUnresolvedReference>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind, is_external FROM unresolved_references WHERE name = ?1"
        )?;
        
        let refs = stmt
            .query_map([name], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Get all unresolved references
    pub fn get_all_unresolved(&self) -> Result<Vec<PersistedUnresolvedReference>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind, is_external FROM unresolved_references"
        )?;
        
        let refs = stmt
            .query_map([], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Get unresolved references for a specific file
    pub fn get_unresolved_in_file(&self, file_path: &str) -> Result<Vec<PersistedUnresolvedReference>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_uri, name, receiver, scope_id, file_path, line, ref_kind, is_external FROM unresolved_references WHERE file_path = ?1"
        )?;
        
        let refs = stmt
            .query_map([file_path], |row| self.row_to_unresolved(row))?
            .filter_map(|r| r.ok())
            .collect();
        
        Ok(refs)
    }

    /// Delete an unresolved reference by ID
    pub fn delete_unresolved(&self, id: i64) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM unresolved_references WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Clear all unresolved references
    pub fn clear_unresolved(&self) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM unresolved_references", [])?;
        Ok(())
    }

    /// Count unresolved references (active ones, not marked as external)
    pub fn count_unresolved(&self) -> Result<usize> {
        let count: i64 = self.lock_conn()?.query_row(
            "SELECT COUNT(*) FROM unresolved_references WHERE is_external = 0", 
            [], 
            |row| row.get(0)
        )?;
        Ok(count as usize)
    }

    /// Mark an unresolved reference as external
    pub fn mark_unresolved_as_external(&self, id: i64) -> Result<()> {
        self.lock_conn()?.execute("UPDATE unresolved_references SET is_external = 1 WHERE id = ?1", [id])?;
        Ok(())
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
            is_external: row.get(8)?,
        })
    }
}

impl SqliteStore {

    // ========== Incremental Indexing Operations ==========

    /// Get the stored hash for a file
    pub fn get_file_hash(&self, path: &str) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT hash FROM file_hash WHERE path = ?1")?;
        let hash: Option<String> = stmt
            .query_row([path], |row| row.get(0))
            .optional()?;
        Ok(hash)
    }

    /// Update the hash for a file
    pub fn update_file_hash(&self, path: &str, hash: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
            
        self.lock_conn()?.execute(
            "INSERT OR REPLACE INTO file_hash (path, hash, last_modified) VALUES (?1, ?2, ?3)",
            params![path, hash, now],
        )?;
        Ok(())
    }

    /// Remove a file from hash tracking (and cascade delete symbols via logic if needed, 
    /// though we rely on `delete_symbols_for_file` usually)
    pub fn remove_file_hash(&self, path: &str) -> Result<()> {
        self.lock_conn()?.execute("DELETE FROM file_hash WHERE path = ?1", [path])?;
        Ok(())
    }
    
    /// Delete all symbols and related data for a file
    pub fn delete_file_data(&self, path: &str) -> Result<()> {
        // 1. Find symbols in this file to delete edges/embeddings
        // Note: cascading deletes might handle some, but let's be explicit where needed
        // For now, let's just delete symbols. The detailed cleanup might need more complex logic
        // if we want to be perfect, but "nuke and rebuild" style for a file works too.
        
        // Delete symbols (and relying on manual cleanup for edges if no cascade)
        // Edges don't have FK to symbols(uri) in the schema currently enforced? 
        // Actually schema doesn't have FKs for edges.
        // So we should delete edges where from_uri belongs to this file.
        
        // Get all symbols for this file first
        let symbols = self.find_symbols_in_file(path)?;
        for symbol in symbols {
            let uri_str = symbol.uri.to_uri_string();
            // Delete edges from this symbol
            self.lock_conn()?.execute("DELETE FROM edges WHERE from_uri = ?1", [&uri_str])?;
            // Delete edges to this symbol? Maybe keep them but they point to nowhere?
            // Better to delete them to avoid dangling links.
            self.lock_conn()?.execute("DELETE FROM edges WHERE to_uri = ?1", [&uri_str])?;
            // Delete embeddings
            self.lock_conn()?.execute("DELETE FROM embeddings WHERE uri = ?1", [&uri_str])?;
        }

        // Delete symbols
        self.lock_conn()?.execute("DELETE FROM symbols WHERE path = ?1", [path])?;
        
        // Manually delete dependent tables for unresolved refs (in case FK cascade is not active)
        let refs = self.get_unresolved_in_file(path)?;
        for r in refs {
            self.lock_conn()?.execute("DELETE FROM ambiguous_references WHERE reference_id = ?1", [r.id])?;
            self.lock_conn()?.execute("DELETE FROM callsite_embeddings WHERE reference_id = ?1", [r.id])?;
        }
        
        // Delete unresolved refs
        self.lock_conn()?.execute("DELETE FROM unresolved_references WHERE file_path = ?1", [path])?;
        
        // Delete imports
        self.lock_conn()?.execute("DELETE FROM imports WHERE file_path = ?1", [path])?;
        
        // Delete file hash
        self.remove_file_hash(path)?;

        Ok(())
    }

    /// Get all indexed file paths
    pub fn get_all_indexed_files(&self) -> Result<Vec<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT path FROM file_hash")?;
        let paths = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(paths)
    }
}

/// Persisted unresolved reference (stored in DB)
#[derive(Debug, Clone, Serialize)]
pub struct PersistedUnresolvedReference {
    pub id: i64,
    pub from_uri: String,
    pub name: String,
    pub receiver: Option<String>,
    pub scope_id: i64,
    pub file_path: String,
    pub line: u32,
    pub ref_kind: String,
    pub is_external: bool,
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
            is_external: false,
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

/// Import record
#[derive(Debug, Clone, Serialize)]
pub struct Import {
    pub id: i64,
    pub file_path: String,
    pub alias: Option<String>,
    pub target_namespace: String,
    pub line: Option<u32>,
}

/// Ambiguous reference record
#[derive(Debug, Clone)]
pub struct AmbiguousReference {
    pub id: i64,
    pub reference_id: i64,
    pub candidate_uri: String,
    pub score: f32,
}


/// Database statistics
#[derive(Debug, Clone, Serialize)]
pub struct DbStats {
    pub symbols: usize,
    pub edges: usize,
    pub embeddings: usize,
    pub unresolved: usize,
    pub imports: usize,
    pub callsite_embeddings: usize,
}



impl std::fmt::Display for DbStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Database Statistics:")?;
        writeln!(f, "  Symbols: {}", self.symbols)?;
        writeln!(f, "  Edges: {}", self.edges)?;
        writeln!(f, "  Embeddings: {}", self.embeddings)?;
        writeln!(f, "  Unresolved: {}", self.unresolved)?;
        writeln!(f, "  Imports: {}", self.imports)?;
        writeln!(f, "  Callsite Embeddings: {}", self.callsite_embeddings)

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

    #[test]
    fn test_multi_word_search() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        let s = sample_symbol("PostgresEngine", 10);
        store.insert_symbol(&s).unwrap();
        
        // Match both words
        let results = store.search_content("postgre engine", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "PostgresEngine");
        
        // One word in name, one in content
        let mut s2 = sample_symbol("MyClass", 20);
        s2.content = "this is a special engine".to_string();
        store.insert_symbol(&s2).unwrap();
        
        let results = store.search_content("myclass engine", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyClass");
        
        // No match if one word missing
        let results = store.search_content("myclass missing", None, 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_boosting() {
        let store = SqliteStore::open_in_memory().unwrap();
        
        // Symbol in code
        let s_code = Symbol::new("testrepo", "src/main.rs", SymbolKind::Callable, "my_func", 1, 5, "fn my_func() {}");
        store.insert_symbol(&s_code).unwrap();
        
        // Symbol in doc
        let s_doc = Symbol::new("testrepo", "README.md", SymbolKind::Namespace, "my_func", 1, 1, "Doc about my_func");
        store.insert_symbol(&s_doc).unwrap();
        
        // Search should return the .rs file first due to boost (1.2 vs 0.9)
        let results = store.search_content("my_func", None, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[1].path, "README.md");
    }
}
