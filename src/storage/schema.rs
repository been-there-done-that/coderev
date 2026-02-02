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

/// SQL to create indexes
pub const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_symbols_path ON symbols(path)",
    "CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name)",
    "CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind)",
    "CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_uri)",
    "CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_uri)",
    "CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind)",
];

/// All schema creation statements
pub fn all_schema_statements() -> Vec<&'static str> {
    let mut stmts = vec![
        CREATE_SYMBOLS_TABLE,
        CREATE_EDGES_TABLE,
        CREATE_EMBEDDINGS_TABLE,
    ];
    stmts.extend(CREATE_INDEXES.iter().copied());
    stmts
}
