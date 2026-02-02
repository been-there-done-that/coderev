//! Database schema definitions

/// SQL to create the symbols table
pub const CREATE_SYMBOLS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS symbols (
    uri TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    doc TEXT,
    signature TEXT,
    content TEXT NOT NULL
)
"#;

/// SQL to create the edges table
pub const CREATE_EDGES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_uri TEXT NOT NULL,
    to_uri TEXT NOT NULL,
    kind TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    UNIQUE(from_uri, to_uri, kind)
)
"#;

/// SQL to create the embeddings table
pub const CREATE_EMBEDDINGS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS embeddings (
    uri TEXT PRIMARY KEY,
    vector BLOB NOT NULL
)
"#;

/// SQL to create the unresolved_references table
/// Stores call sites and references that need global resolution
pub const CREATE_UNRESOLVED_REFERENCES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS unresolved_references (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_uri TEXT NOT NULL,
    name TEXT NOT NULL,
    receiver TEXT,
    scope_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    ref_kind TEXT NOT NULL DEFAULT 'call'
)
"#;

/// SQL to create the imports table
pub const CREATE_IMPORTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS imports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    alias TEXT,
    target_namespace TEXT NOT NULL,
    line INTEGER
)
"#;

/// SQL to create the ambiguous_references table
pub const CREATE_AMBIGUOUS_REFERENCES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS ambiguous_references (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    reference_id INTEGER NOT NULL,
    candidate_uri TEXT NOT NULL,
    score REAL DEFAULT 0.0,
    FOREIGN KEY(reference_id) REFERENCES unresolved_references(id)
)
"#;

/// SQL to create indexes
pub const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_symbols_path ON symbols(path)",
    "CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name)",
    "CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind)",
    "CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_uri)",
    "CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_uri)",
    "CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind)",
    "CREATE INDEX IF NOT EXISTS idx_unresolved_name ON unresolved_references(name)",
    "CREATE INDEX IF NOT EXISTS idx_unresolved_file ON unresolved_references(file_path)",
    "CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_path)",
    "CREATE INDEX IF NOT EXISTS idx_ambiguous_ref ON ambiguous_references(reference_id)",
];

/// All schema creation statements
pub fn all_schema_statements() -> Vec<&'static str> {
    let mut stmts = vec![
        CREATE_SYMBOLS_TABLE,
        CREATE_EDGES_TABLE,
        CREATE_EMBEDDINGS_TABLE,
        CREATE_UNRESOLVED_REFERENCES_TABLE,
        CREATE_IMPORTS_TABLE,
        CREATE_AMBIGUOUS_REFERENCES_TABLE,
    ];
    stmts.extend(CREATE_INDEXES.iter().copied());
    stmts
}
