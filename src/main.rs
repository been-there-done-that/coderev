//! Coderev CLI - Command-line interface for Universal Code Intelligence Substrate

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use coderev::storage::SqliteStore;
use coderev::adapter;
use coderev::query::QueryEngine;
use coderev::SymbolKind;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "coderev")]
#[command(version = "0.1.0")]
#[command(about = "Universal Code Intelligence Substrate - Language-agnostic semantic code graph")]
#[command(long_about = r#"
Coderev builds a semantic code graph from your codebase, enabling:
  • Natural language code search
  • Call graph analysis (callers/callees)
  • Impact analysis for refactoring
  • Cross-language, cross-repo queries

Example usage:
  coderev index --path ./src
  coderev search --query "authentication validation"
  coderev callers --uri "codescope://repo/auth.py#callable:validate@10"
"#)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a repository or directory
    Index {
        /// Path to the repository or directory to index
        #[arg(short, long)]
        path: PathBuf,

        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Repository name (defaults to directory name)
        #[arg(short, long)]
        repo: Option<String>,
    },

    /// Search for symbols using natural language
    Search {
        /// Search query
        #[arg(short, long)]
        query: String,

        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Filter by symbol kind
        #[arg(short, long)]
        kind: Option<String>,

        /// Use vector search (requires embeddings)
        #[arg(short = 'V', long)]
        vector: bool,
    },

    /// Generate embeddings for symbols
    Embed {
        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Model name (placeholder for now)
        #[arg(short, long, default_value = "all-MiniLM-L6-v2")]
        model: String,
    },

    /// Find all callers of a symbol
    Callers {
        /// Symbol URI
        #[arg(short, long)]
        uri: String,

        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Maximum depth for transitive callers
        #[arg(long, default_value = "1")]
        depth: usize,
    },

    /// Find all callees of a symbol
    Callees {
        /// Symbol URI
        #[arg(short, long)]
        uri: String,

        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Maximum depth for transitive callees
        #[arg(long, default_value = "1")]
        depth: usize,
    },

    /// Analyze impact of changes to a symbol
    Impact {
        /// Symbol URI
        #[arg(short, long)]
        uri: String,

        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Maximum depth for impact traversal
        #[arg(long, default_value = "3")]
        depth: usize,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show statistics about the indexed codebase
    Stats {
        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    match cli.command {
        Commands::Index { path, database, repo } => {
            tracing::info!("Indexing {} into {:?}", path.display(), database);
            let repo_name = repo.unwrap_or_else(|| {
                path.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            });
            
            // TODO: Implement indexing
            let store = SqliteStore::open(&database)?;
            let registry = adapter::default_registry();
            let mut total_symbols = 0;
            let mut total_files = 0;
            let mut unresolved_refs = Vec::new();

            println!("🚀 Indexing repository: {}", repo_name);
            println!("📂 Path: {:?}", path);
            println!("🗄️  Database: {:?}", database);

            for entry in walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_path = entry.path();
                let _ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
                
                if let Some(adapter) = registry.find_adapter(file_path) {
                    let relative_path = file_path.strip_prefix(&path).unwrap_or(file_path);
                    println!("📄 Processing: {:?}", relative_path);

                    match std::fs::read_to_string(file_path) {
                        Ok(content) => {
                            match adapter.parse_file(&repo_name, relative_path.to_str().unwrap_or(""), &content) {
                                Ok(res) => {
                                    for symbol in &res.symbols {
                                        store.insert_symbol(symbol)?;
                                    }
                                    for edge in &res.edges {
                                        store.insert_edge(edge)?;
                                    }
                                    
                                    // Collect unresolved references for the second pass
                                    for unresolved in res.scope_graph.unresolved_references() {
                                        unresolved_refs.push(unresolved.clone());
                                    }
                                    
                                    total_symbols += res.symbols.len();
                                    total_files += 1;
                                }
                                Err(e) => {
                                    tracing::error!("Failed to parse {}: {}", file_path.display(), e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to read {}: {}", file_path.display(), e);
                        }
                    }
                }
            }
            
            // Phase 7: Multi-pass Resolution
            if !unresolved_refs.is_empty() {
                println!("🔗 Resolving {} cross-references...", unresolved_refs.len());
                let mut resolved_count = 0;
                
                for unresolved in unresolved_refs {
                    // Try to find the symbol in the database
                    // For now, we do a simple name-based lookup
                    let matches = store.find_symbols_by_name(&unresolved.name)?;
                    if let Some(target) = matches.first() {
                        let edge = coderev::edge::Edge::new(
                            unresolved.from_uri.clone(),
                            target.uri.clone(),
                            coderev::edge::EdgeKind::Calls,
                        );
                        store.insert_edge(&edge)?;
                        resolved_count += 1;
                    }
                }
                println!("✅ Resolved {} references.", resolved_count);
            }

            println!("\n✅ Indexing complete!");
            println!("📊 Total files: {}", total_files);
            println!("🔢 Total symbols: {}", total_symbols);
            println!("🗄️  Database saved to: {:?}", database);
        }

        Commands::Search { query, database, limit, kind, vector } => {
            let store = SqliteStore::open(&database)?;
            let engine = QueryEngine::new(&store);
            
            let parsed_kind = if let Some(ref k) = kind {
                use std::str::FromStr;
                Some(SymbolKind::from_str(k)?)
            } else {
                None
            };

            let results = if vector {
                println!("🧠 [Local Embedding] Searching for: '{}'...", query);
                let embedding_engine = coderev::query::EmbeddingEngine::new()?;
                let query_vector = embedding_engine.embed_query(&query)?;
                engine.search_by_vector(&query_vector, limit)?
            } else {
                println!("🔍 Searching for: '{}' (kind: {:?}, limit: {})...", query, kind, limit);
                if let Some(k) = parsed_kind {
                    engine.search_by_kind(k, Some(&query), limit)?
                        .into_iter()
                        .map(|s| coderev::query::engine::QueryResult::new(s, 1.0))
                        .collect()
                } else {
                    engine.search_by_name(&query, limit)?
                }
            };

            if results.is_empty() {
                println!("❌ No symbols found.");
            } else {
                for res in results {
                    let uri_str = res.symbol.uri.to_uri_string();
                    println!("- [{:?}] {} (Score: {:.2})", res.symbol.kind, res.symbol.name, res.score);
                    println!("  URI: {}", uri_str);
                    if let Some(sig) = &res.symbol.signature {
                        println!("  Sig: {}", sig);
                    }
                }
            }
        }

        Commands::Embed { database, model: _ } => {
            let mut store = SqliteStore::open(&database)?;
            let symbols = store.find_symbols_by_name_pattern("%")?; // Get all symbols
            
            if symbols.is_empty() {
                println!("∅ No symbols found in database to embed.");
                return Ok(());
            }

            println!("🧠 Initializing real local embedding model (all-MiniLM-L6-v2)...");
            let engine = coderev::query::EmbeddingEngine::new()?;
            
            println!("🛰️  Generating embeddings for {} symbols in batches...", symbols.len());
            
            // Batch processing (32 symbols at a time)
            let batch_size = 32;
            let mut processed = 0;
            
            for chunk in symbols.chunks(batch_size) {
                let embeddings = engine.embed_symbols(chunk)?;
                
                // Store embeddings in a transaction for speed
                store.begin_transaction()?;
                for (i, vector) in embeddings.into_iter().enumerate() {
                    store.insert_embedding(&chunk[i].uri, &vector)?;
                }
                store.commit()?;
                
                processed += chunk.len();
                println!("  Progress: {}/{}", processed, symbols.len());
            }
            
            println!("✅ Embedding complete! All symbols now have real vectors.");
        }

        Commands::Callers { uri, database, depth } => {
            let store = SqliteStore::open(&database)?;
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            println!("📞 Finding callers for: {} (depth: {})...", uri, depth);
            let callers = engine.find_callers(&target_uri, depth)?;
            
            if callers.is_empty() {
                println!("∅ No callers found.");
            } else {
                for symbol in callers {
                    println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                }
            }
        }

        Commands::Callees { uri, database, depth } => {
            let store = SqliteStore::open(&database)?;
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            println!("📱 Finding callees for: {} (depth: {})...", uri, depth);
            let callees = engine.find_callees(&target_uri, depth)?;
            
            if callees.is_empty() {
                println!("∅ No callees found.");
            } else {
                for symbol in callees {
                    println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                }
            }
        }

        Commands::Impact { uri, database, depth, format } => {
            let store = SqliteStore::open(&database)?;
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            println!("💥 Impact analysis for: {} (depth: {})...", uri, depth);
            let impact = engine.impact_analysis(&target_uri, depth)?;
            
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&impact)?);
            } else {
                if impact.is_empty() {
                    println!("∅ No impact found.");
                } else {
                    for res in impact {
                        let prefix = if res.is_direct() { "🔴 [DIRECT]" } else { "🟠 [INDIRECT]" };
                        println!("{} [{:?}] {} (Depth: {}, Conf: {:.2})", 
                            prefix, res.symbol.kind, res.symbol.name, res.depth, res.confidence);
                        println!("   URI: {}", res.symbol.uri.to_uri_string());
                    }
                }
            }
        }

        Commands::Stats { database } => {
            let store = SqliteStore::open(&database)?;
            let stats = store.stats()?;
            
            println!("📊 Coderev Statistics ({:?})", database);
            println!("------------------------------------");
            println!("{}", stats);
        }
    }

    Ok(())
}
