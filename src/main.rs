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

    /// Run the global resolver to resolve unresolved references
    Resolve {
        /// Path to the database file
        #[arg(short, long, default_value = "coderev.db")]
        database: PathBuf,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
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
            
            let mut store = SqliteStore::open(&database)?;
            let registry = adapter::default_registry();
            let chunker = adapter::DocumentChunker::new();
            let mut total_symbols = 0;
            let mut total_files = 0;
            let mut total_unresolved = 0;
            let mut total_docs_chunked = 0;

            println!("🚀 Indexing repository: {}", repo_name);
            println!("📂 Path: {:?}", path);
            println!("🗄️  Database: {:?}", database);
            
            // Clear old unresolved references and imports before re-indexing
            store.clear_unresolved()?;
            store.clear_imports()?;
            store.clear_ambiguous_references()?;


            for entry in walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_path = entry.path();
                let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                
                // Skip common binary files
                let skip_exts = ["png", "jpg", "jpeg", "gif", "ico", "exe", "dll", "so", "o", "a", "lib", "bin", "pdf", "zip", "tar", "gz", "wasm", "node"];
                if skip_exts.contains(&ext.as_str()) {
                    continue;
                }

                if let Some(adapter) = registry.find_adapter(file_path) {
                    // AST-based parsing for supported languages
                    let relative_path = file_path.strip_prefix(&path).unwrap_or(file_path);
                    let relative_path_str = relative_path.to_str().unwrap_or("");
                    println!("📄 Processing (AST): {:?}", relative_path);

                    match std::fs::read_to_string(file_path) {
                        Ok(content) => {
                            match adapter.parse_file(&repo_name, relative_path_str, &content) {
                                Ok(res) => {
                                    // Phase 1: Store symbols
                                    for symbol in &res.symbols {
                                        store.insert_symbol(symbol)?;
                                    }
                                    
                                    // Phase 1: Store edges (Defines, Contains)
                                    for edge in &res.edges {
                                        store.insert_edge(edge)?;
                                    }
                                    
                                    // Phase 2: Persist unresolved references to DB
                                    for unresolved in res.scope_graph.unresolved_references() {
                                        let (receiver, name) = if let Some((r, n)) = unresolved.name.rsplit_once('.') {
                                            (Some(r.to_string()), n.to_string())
                                        } else {
                                            (None, unresolved.name.clone())
                                        };

                                        let persisted = coderev::storage::PersistedUnresolvedReference::new(
                                            unresolved.from_uri.to_uri_string(),
                                            name,
                                            receiver,

                                            unresolved.scope.0 as i64,
                                            relative_path_str.to_string(),
                                            unresolved.line,
                                            "call", // default to call, can be "inherits" for inheritance
                                        );
                                        store.insert_unresolved(&persisted)?;
                                        total_unresolved += 1;
                                    }

                                    // Phase 1: Store imports
                                    for import in res.scope_graph.imports(coderev::scope::graph::ScopeId::root()) {
                                        store.insert_import(
                                            relative_path_str,
                                            import.alias.as_deref(),
                                            &import.namespace,
                                            Some(import.line),
                                        )?;
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
                            tracing::debug!("Skipping binary/unreadable file {}: {}", file_path.display(), e);
                        }
                    }
                } else {
                    // Fallback: Chunk-based indexing for documents without AST adapter
                    let relative_path = file_path.strip_prefix(&path).unwrap_or(file_path);
                    let relative_path_str = relative_path.to_str().unwrap_or("");
                    println!("📝 Processing (Chunk): {:?}", relative_path);

                    match std::fs::read_to_string(file_path) {
                        Ok(content) => {
                            match chunker.chunk_file(&repo_name, relative_path_str, &content) {
                                Ok(res) => {
                                    for symbol in &res.symbols {
                                        store.insert_symbol(symbol)?;
                                    }
                                    total_symbols += res.symbols.len();
                                    total_docs_chunked += 1;
                                    total_files += 1;
                                }
                                Err(e) => {
                                    tracing::error!("Failed to chunk {}: {}", file_path.display(), e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Skipping unreadable file {}: {}", file_path.display(), e);
                        }
                    }
                }
            }

            println!("\n📊 Phase 1 Complete:");
            println!("   Files indexed: {}", total_files);
            println!("   Symbols: {}", total_symbols);
            println!("   Documents chunked: {}", total_docs_chunked);
            println!("   Unresolved refs: {}", total_unresolved);
            
            // Phase 2: Run Global Linker (Semantic)
            if total_unresolved > 0 {
                println!("\n🔗 Phase 2: Running Global Linker (Semantic)...");
                let linker = coderev::linker::GlobalLinker::new(&store);
                let stats = linker.run()?;
                println!("{}", stats);
            } else {
                println!("\n✅ Phase 2: No unresolved references to resolve.");
            }


            // Phase 3: Generate Semantic Embeddings
            println!("\n🧠 Phase 3: Generating Semantic Embeddings...");
            let symbols_to_embed = store.find_symbols_without_embeddings()?;
            
            if !symbols_to_embed.is_empty() {
                println!("🛰️  Generating embeddings for {} symbols in batches...", symbols_to_embed.len());
                let engine = coderev::query::EmbeddingEngine::new()?;
                
                let batch_size = 32;
                let mut processed = 0;
                
                for chunk in symbols_to_embed.chunks(batch_size) {
                    let embeddings = engine.embed_symbols(chunk)?;
                    
                    store.begin_transaction()?;
                    for (i, vector) in embeddings.into_iter().enumerate() {
                        store.insert_embedding(&chunk[i].uri, &vector)?;
                    }
                    store.commit()?;
                    
                    processed += chunk.len();
                    println!("   Progress: {}/{}", processed, symbols_to_embed.len());
                }
            } else {
                println!("✅ All symbols already have embeddings.");
            }

            println!("\n✅ Indexing complete!");
            println!("🗄️  Database saved to: {:?}", database);
            
            // Show final stats
            let final_stats = store.stats()?;
            println!("{}", final_stats);
        }

        Commands::Search { query, database, limit, kind, vector } => {
            let mut store = SqliteStore::open(&database)?;
            
            let parsed_kind = if let Some(ref k) = kind {
                use std::str::FromStr;
                Some(SymbolKind::from_str(k)?)
            } else {
                None
            };

            let results = if vector {
                // Ensure embeddings exist before searching
                ensure_embeddings(&mut store)?;
                
                println!("🧠 [Local Embedding] Searching for: '{}'...", query);
                let engine = QueryEngine::new(&store);
                let embedding_engine = coderev::query::EmbeddingEngine::new()?;
                let query_vector = embedding_engine.embed_query(&query)?;
                engine.search_by_vector(&query_vector, limit)?
            } else {
                // Default: search in name, content, and doc fields
                println!("🔍 Searching for: '{}' (kind: {:?}, limit: {})...", query, kind, limit);
                store.search_content(&query, parsed_kind, limit)?
                    .into_iter()
                    .map(|s| coderev::query::engine::QueryResult::new(s, 1.0))
                    .collect()
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
            ensure_embeddings(&mut store)?;
            println!("✅ Embedding complete!");
        }

        Commands::Callers { uri, database, depth } => {
            let store = SqliteStore::open(&database)?;
            ensure_resolved(&store)?;
            
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
            ensure_resolved(&store)?;
            
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
            ensure_resolved(&store)?;
            
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

        Commands::Resolve { database, verbose } => {
            let store = SqliteStore::open(&database)?;
            
            let unresolved_count = store.count_unresolved()?;
            if unresolved_count == 0 {
                println!("✅ No unresolved references to resolve.");
                return Ok(());
            }
            
            println!("🔗 Running Global Linker on {} unresolved references...", unresolved_count);
            
            if verbose {
                println!("\nUnresolved references:");
                for unresolved in store.get_all_unresolved()? {
                    println!("  - {} (from {} @ line {})", 
                        unresolved.name, 
                        unresolved.file_path,
                        unresolved.line);
                }
                println!();
            }
            
            let linker = coderev::linker::GlobalLinker::new(&store);
            let stats = linker.run()?;
            
            println!("{}", stats);

            
            // Show remaining unresolved if verbose
            if verbose {
                let remaining = store.get_all_unresolved()?;
                if !remaining.is_empty() {
                    println!("\nRemaining unresolved:");
                    for unresolved in remaining {
                        println!("  ❌ {} (from {} @ line {})", 
                            unresolved.name, 
                            unresolved.file_path,
                            unresolved.line);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Helper to ensure all symbols have embeddings
fn ensure_embeddings(store: &mut SqliteStore) -> anyhow::Result<()> {
    let missing = store.find_symbols_without_embeddings()?;
    if !missing.is_empty() {
        println!("🧠 On-demand: Generating embeddings for {} symbols...", missing.len());
        let engine = coderev::query::EmbeddingEngine::new()?;
        
        let batch_size = 32;
        let mut processed = 0;
        let total = missing.len();
        
        for chunk in missing.chunks(batch_size) {
            let embeddings = engine.embed_symbols(chunk)?;
            
            store.begin_transaction()?;
            for (i, vector) in embeddings.into_iter().enumerate() {
                store.insert_embedding(&chunk[i].uri, &vector)?;
            }
            store.commit()?;
            
            processed += chunk.len();
            println!("   Progress: {}/{}", processed, total);
        }
        println!("✅ Embedding complete.");
    }
    Ok(())
}

/// Helper to ensure all unresolved references are resolved
fn ensure_resolved(store: &SqliteStore) -> anyhow::Result<()> {
    let unresolved_count = store.count_unresolved()?;
    if unresolved_count > 0 {
        println!("🔗 On-demand: Resolving {} references...", unresolved_count);
        let linker = coderev::linker::GlobalLinker::new(store);
        let stats = linker.run()?;
        println!("{}", stats);
    }

    Ok(())
}
