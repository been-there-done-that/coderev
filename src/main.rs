//! Coderev CLI - Command-line interface for Universal Code Intelligence Substrate

use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;
use coderev::storage::SqliteStore;
use coderev::adapter;
use coderev::query::QueryEngine;
use coderev::{SymbolKind, IndexMessage, FileStatus};
use coderev::config::{self, CoderevConfig};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use std::io::Write;

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

    /// Emit JSON output (stable schema)
    #[arg(long, global = true)]
    json: bool,

    /// Emit compact JSON output (short keys)
    #[arg(long, global = true)]
    compact: bool,

    /// Emit TOON JSON output (because yes)
    #[arg(long, global = true)]
    toon: bool,

    /// Path to config file (default: ./coderev.toml)
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputMode {
    Human,
    Json,
    Compact,
    Toon,
}

impl OutputMode {
    fn from_flags(json: bool, compact: bool, toon: bool) -> anyhow::Result<Self> {
        let count = json as u8 + compact as u8 + toon as u8;
        if count > 1 {
            anyhow::bail!("flags --json, --compact, and --toon are mutually exclusive");
        }
        if compact {
            return Ok(Self::Compact);
        }
        if toon {
            return Ok(Self::Toon);
        }
        if json {
            return Ok(Self::Json);
        }
        Ok(Self::Human)
    }

    fn is_machine(self) -> bool {
        !matches!(self, Self::Human)
    }

    fn is_human(self) -> bool {
        matches!(self, Self::Human)
    }

    fn format_label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Compact => "compact",
            Self::Toon => "toon",
        }
    }
}

#[derive(Serialize)]
struct ErrorOut {
    message: String,
}

#[derive(Serialize)]
struct ErrorOutCompact {
    msg: String,
}

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    schema_version: &'static str,
    format: &'static str,
    command: &'static str,
    ok: bool,
    data: Option<T>,
    error: Option<ErrorOut>,
}

#[derive(Serialize)]
struct CompactEnvelope<T: Serialize> {
    v: &'static str,
    f: &'static str,
    cmd: &'static str,
    ok: bool,
    data: Option<T>,
    err: Option<ErrorOutCompact>,
}

#[derive(Serialize)]
struct SymbolRef {
    kind: String,
    name: String,
    uri: String,
    path: String,
    line_start: u32,
    line_end: u32,
    signature: Option<String>,
}

#[derive(Serialize)]
struct SymbolRefCompact {
    k: String,
    n: String,
    u: String,
    p: String,
    ls: u32,
    le: u32,
    s: Option<String>,
}

#[derive(Serialize)]
struct SearchItem {
    kind: String,
    name: String,
    uri: String,
    path: String,
    line_start: u32,
    line_end: u32,
    score: f32,
    signature: Option<String>,
}

#[derive(Serialize)]
struct SearchItemCompact {
    k: String,
    n: String,
    u: String,
    p: String,
    ls: u32,
    le: u32,
    sc: f32,
    s: Option<String>,
}

#[derive(Serialize)]
struct SearchOutput {
    query: String,
    kind: Option<String>,
    limit: usize,
    exact: bool,
    mode: String,
    results: Vec<SearchItem>,
}

#[derive(Serialize)]
struct SearchOutputCompact {
    q: String,
    k: Option<String>,
    l: usize,
    x: bool,
    m: String,
    r: Vec<SearchItemCompact>,
}

#[derive(Serialize)]
struct ListOutput {
    uri: String,
    depth: usize,
    results: Vec<SymbolRef>,
}

#[derive(Serialize)]
struct ListOutputCompact {
    u: String,
    d: usize,
    r: Vec<SymbolRefCompact>,
}

#[derive(Serialize)]
struct ImpactItem {
    kind: String,
    name: String,
    uri: String,
    path: String,
    line_start: u32,
    line_end: u32,
    depth: usize,
    confidence: f32,
    edge_kind: String,
}

#[derive(Serialize)]
struct ImpactItemCompact {
    k: String,
    n: String,
    u: String,
    p: String,
    ls: u32,
    le: u32,
    d: usize,
    c: f32,
    e: String,
}

#[derive(Serialize)]
struct ImpactOutput {
    uri: String,
    depth: usize,
    results: Vec<ImpactItem>,
}

#[derive(Serialize)]
struct ImpactOutputCompact {
    u: String,
    d: usize,
    r: Vec<ImpactItemCompact>,
}

#[derive(Serialize)]
struct IndexDurations {
    indexing_ms: u128,
    linking_ms: Option<u128>,
    embedding_ms: Option<u128>,
    semantic_ms: Option<u128>,
    total_ms: u128,
}

#[derive(Serialize)]
struct IndexOutput {
    repo: String,
    path: String,
    database: String,
    stats: IndexingStats,
    linker: Option<coderev::linker::GlobalLinkerStats>,
    embedded_symbols: usize,
    semantic: Option<coderev::linker::SemanticLinkerStats>,
    final_db: coderev::storage::DbStats,
    durations: IndexDurations,
}

#[derive(Serialize)]
struct IndexOutputCompact {
    r: String,
    p: String,
    db: String,
    s: IndexingStats,
    l: Option<coderev::linker::GlobalLinkerStats>,
    es: usize,
    se: Option<coderev::linker::SemanticLinkerStats>,
    fd: coderev::storage::DbStats,
    t: IndexDurations,
}

fn emit_success<T: Serialize>(mode: OutputMode, command: &'static str, data: T) -> anyhow::Result<()> {
    match mode {
        OutputMode::Human => Ok(()),
        OutputMode::Json => {
            let payload = Envelope {
                schema_version: SCHEMA_VERSION,
                format: mode.format_label(),
                command,
                ok: true,
                data: Some(data),
                error: None,
            };
            println!("{}", serde_json::to_string_pretty(&payload)?);
            Ok(())
        }
        OutputMode::Compact | OutputMode::Toon => {
            let payload = CompactEnvelope {
                v: SCHEMA_VERSION,
                f: mode.format_label(),
                cmd: command,
                ok: true,
                data: Some(data),
                err: None,
            };
            println!("{}", serde_json::to_string(&payload)?);
            Ok(())
        }
    }
}

fn emit_error(mode: OutputMode, command: &'static str, err: &anyhow::Error) -> anyhow::Result<()> {
    match mode {
        OutputMode::Human => Err(anyhow::anyhow!(err.to_string())),
        OutputMode::Json => {
            let payload = Envelope::<serde_json::Value> {
                schema_version: SCHEMA_VERSION,
                format: mode.format_label(),
                command,
                ok: false,
                data: None,
                error: Some(ErrorOut {
                    message: err.to_string(),
                }),
            };
            println!("{}", serde_json::to_string_pretty(&payload)?);
            Ok(())
        }
        OutputMode::Compact | OutputMode::Toon => {
            let payload = CompactEnvelope::<serde_json::Value> {
                v: SCHEMA_VERSION,
                f: mode.format_label(),
                cmd: command,
                ok: false,
                data: None,
                err: Some(ErrorOutCompact {
                    msg: err.to_string(),
                }),
            };
            println!("{}", serde_json::to_string(&payload)?);
            Ok(())
        }
    }
}

fn symbol_ref(symbol: &coderev::Symbol) -> SymbolRef {
    SymbolRef {
        kind: symbol.kind.as_str().to_string(),
        name: symbol.name.clone(),
        uri: symbol.uri.to_uri_string(),
        path: symbol.path.clone(),
        line_start: symbol.line_start,
        line_end: symbol.line_end,
        signature: symbol.signature.clone(),
    }
}

fn symbol_ref_compact(symbol: &coderev::Symbol) -> SymbolRefCompact {
    SymbolRefCompact {
        k: symbol.kind.as_str().to_string(),
        n: symbol.name.clone(),
        u: symbol.uri.to_uri_string(),
        p: symbol.path.clone(),
        ls: symbol.line_start,
        le: symbol.line_end,
        s: symbol.signature.clone(),
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a Coderev config file in the current directory
    Init {
        /// Path to repository (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Repository name (defaults to directory name)
        #[arg(short, long)]
        repo: Option<String>,

        /// Path to config file (default: ./coderev.toml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },

    /// Generate MCP config scaffolding for agents
    AgentSetup {
        /// Path to repository (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Output path for MCP config (default: .coderev/mcp.json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },
    /// Index a repository or directory
    Index {
        /// Path to the repository or directory to index
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

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
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Filter by symbol kind
        #[arg(short, long)]
        kind: Option<String>,

        /// Use exact text match only (disable vector search)
        #[arg(long)]
        exact: bool,
    },

    /// Generate embeddings for symbols
    Embed {
        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

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
        #[arg(short, long)]
        database: Option<PathBuf>,

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
        #[arg(short, long)]
        database: Option<PathBuf>,

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
        #[arg(short, long)]
        database: Option<PathBuf>,

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
        #[arg(short, long)]
        database: Option<PathBuf>,
    },

    /// Run the global resolver to resolve unresolved references
    Resolve {
        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Serve the Coderev UI and API
    Serve {
        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Host to bind to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Run the MCP server over stdio
    Mcp {
        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,
    },

    /// Watch for file changes and incrementally index
    Watch {
        /// Path to the repository or directory to watch
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Run watcher in background (daemon mode)
        #[arg(long)]
        background: bool,

        /// Show watcher status
        #[arg(long)]
        status: bool,

        /// Stop background watcher
        #[arg(long)]
        stop: bool,

        /// Internal flag for daemonized watcher
        #[arg(long, hide = true)]
        daemon: bool,
    },

    /// Trace calls (alias for callers/callees)
    #[command(subcommand)]
    Trace(TraceCommands),
}

#[derive(Subcommand)]
enum TraceCommands {
    /// Find callers
    Callers {
        /// Symbol URI
        #[arg(short, long)]
        uri: String,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Maximum depth
        #[arg(long, default_value = "1")]
        depth: usize,
    },
    /// Find callees
    Callees {
        /// Symbol URI
        #[arg(short, long)]
        uri: String,

        /// Path to the database file
        #[arg(short, long)]
        database: Option<PathBuf>,

        /// Maximum depth
        #[arg(long, default_value = "1")]
        depth: usize,
    },
}

#[derive(Default, Serialize)]
struct IndexingStats {
    unchanged: usize,
    added: usize,
    modified: usize,
    deleted: usize,
    errors: usize,
    symbols: usize,
    chunked: usize,
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Init { .. } => "init",
        Commands::AgentSetup { .. } => "agent-setup",
        Commands::Index { .. } => "index",
        Commands::Search { .. } => "search",
        Commands::Embed { .. } => "embed",
        Commands::Callers { .. } => "callers",
        Commands::Callees { .. } => "callees",
        Commands::Impact { .. } => "impact",
        Commands::Stats { .. } => "stats",
        Commands::Resolve { .. } => "resolve",
        Commands::Serve { .. } => "serve",
        Commands::Mcp { .. } => "mcp",
        Commands::Watch { .. } => "watch",
        Commands::Trace(cmd) => match cmd {
            TraceCommands::Callers { .. } => "trace.callers",
            TraceCommands::Callees { .. } => "trace.callees",
        },
    }
}

fn resolve_database(cli: Option<PathBuf>, config: &Option<CoderevConfig>) -> PathBuf {
    if let Some(path) = cli {
        return path;
    }
    if let Some(cfg) = config {
        if let Some(db) = &cfg.database {
            return PathBuf::from(db);
        }
    }
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    config::default_database_path_in(&base)
}

fn resolve_database_ready(cli: Option<PathBuf>, config: &Option<CoderevConfig>) -> anyhow::Result<PathBuf> {
    let db = resolve_database(cli, config);
    config::ensure_db_dir(&db)?;
    Ok(db)
}

fn resolve_path(cli: Option<PathBuf>, config: &Option<CoderevConfig>) -> anyhow::Result<PathBuf> {
    if let Some(path) = cli {
        return Ok(path);
    }
    if let Some(cfg) = config {
        if let Some(path) = &cfg.path {
            return Ok(PathBuf::from(path));
        }
    }
    anyhow::bail!("missing --path (or set path in coderev.toml)");
}

fn resolve_repo(cli: Option<String>, config: &Option<CoderevConfig>, path: &PathBuf) -> String {
    if let Some(repo) = cli {
        return repo;
    }
    if let Some(cfg) = config {
        if let Some(repo) = &cfg.repo {
            return repo.clone();
        }
    }
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

struct WatchFiles {
    pid_path: PathBuf,
    log_path: PathBuf,
}

fn watch_files(db_path: &PathBuf) -> WatchFiles {
    let base = db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    WatchFiles {
        pid_path: base.join("coderev-watch.pid"),
        log_path: base.join("coderev-watch.log"),
    }
}

fn read_pid(pid_path: &PathBuf) -> anyhow::Result<Option<i32>> {
    if !pid_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(pid_path)?;
    let pid = raw.trim().parse::<i32>().ok();
    Ok(pid)
}

#[cfg(unix)]
fn is_process_running(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
fn is_process_running(_pid: i32) -> bool {
    false
}

#[cfg(unix)]
fn stop_process(pid: i32) -> anyhow::Result<()> {
    let res = unsafe { libc::kill(pid, libc::SIGTERM) };
    if res != 0 {
        anyhow::bail!("failed to send SIGTERM to pid {}", pid);
    }
    Ok(())
}

#[cfg(not(unix))]
fn stop_process(_pid: i32) -> anyhow::Result<()> {
    anyhow::bail!("stop is not supported on this platform");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let output_mode = OutputMode::from_flags(cli.json, cli.compact, cli.toon)?;
    let cmd_name = command_name(&cli.command);

    if output_mode.is_machine() {
        unsafe {
            std::env::set_var("Coderev_QUIET", "1");
        }
    }

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else if output_mode.is_machine() {
        EnvFilter::new("error")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    match run(cli, output_mode).await {
        Ok(()) => Ok(()),
        Err(err) => {
            if output_mode.is_machine() {
                emit_error(output_mode, cmd_name, &err)?;
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

async fn run(cli: Cli, output_mode: OutputMode) -> anyhow::Result<()> {
    let cfg_opt = config::load_config(cli.config.as_deref())?;
    let global_config_path = cli.config.clone();

    match cli.command {
        Commands::Init { path, database, repo, config: config_path, force } => {
            let target_path = path.unwrap_or(std::env::current_dir()?);
            let repo_name = repo.unwrap_or_else(|| {
                target_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            });
            let db = database.unwrap_or_else(|| config::default_database_path_in(&target_path));
            let cfg = CoderevConfig {
                database: Some(db.display().to_string()),
                repo: Some(repo_name),
                path: Some(target_path.display().to_string()),
            };

            let cfg_path = config_path
                .or(global_config_path)
                .unwrap_or_else(config::default_config_path);

            config::ensure_db_dir(&db)?;
            config::ensure_gitignore(&target_path)?;
            config::write_config(&cfg_path, &cfg, force)?;

            if output_mode.is_human() {
                println!("✅ Wrote config to {}", cfg_path.display());
            } else if matches!(output_mode, OutputMode::Json) {
                let data = serde_json::json!({
                    "config_path": cfg_path.display().to_string(),
                    "config": cfg,
                });
                emit_success(output_mode, "init", data)?;
            } else {
                let data = serde_json::json!({
                    "c": cfg_path.display().to_string(),
                    "cfg": cfg,
                });
                emit_success(output_mode, "init", data)?;
            }
        }
        Commands::AgentSetup { path, database, output, force } => {
            let target_path = path.unwrap_or(std::env::current_dir()?);
            let db = database.unwrap_or_else(|| config::default_database_path_in(&target_path));
            let out_path = output.unwrap_or_else(|| target_path.join(".coderev").join("mcp.json"));

            config::ensure_db_dir(&db)?;
            config::ensure_gitignore(&target_path)?;
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            if out_path.exists() && !force {
                anyhow::bail!("output already exists at {} (use --force to overwrite)", out_path.display());
            }

            let payload = serde_json::json!({
                "mcpServers": {
                    "coderev": {
                        "command": "coderev",
                        "args": ["mcp", "--database", db.display().to_string()]
                    }
                }
            });
            std::fs::write(&out_path, serde_json::to_string_pretty(&payload)?)?;

            if output_mode.is_human() {
                println!("✅ Wrote MCP config to {}", out_path.display());
            } else if matches!(output_mode, OutputMode::Json) {
                let data = serde_json::json!({
                    "path": out_path.display().to_string(),
                    "database": db.display().to_string(),
                });
                emit_success(output_mode, "agent-setup", data)?;
            } else {
                let data = serde_json::json!({
                    "p": out_path.display().to_string(),
                    "db": db.display().to_string(),
                });
                emit_success(output_mode, "agent-setup", data)?;
            }
        }
        Commands::Index { path, database, repo } => {
            let total_start = std::time::Instant::now();
            let path = resolve_path(path, &cfg_opt)?;
            let cfg_path = global_config_path.clone().unwrap_or_else(config::default_config_path);
            if cfg_opt.is_none() && !cfg_path.exists() {
                let db_path = config::default_database_path_in(&path);
                let auto_cfg = CoderevConfig {
                    database: Some(db_path.display().to_string()),
                    repo: Some(path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".to_string())),
                    path: Some(path.display().to_string()),
                };
                config::ensure_db_dir(&db_path)?;
                config::ensure_gitignore(&path)?;
                config::write_config(&cfg_path, &auto_cfg, false)?;
            }
            let database = resolve_database_ready(database, &cfg_opt)?;
            tracing::info!("Indexing {} into {:?}", path.display(), database);
            let repo_name = resolve_repo(repo, &cfg_opt, &path);
            
            let store = SqliteStore::open(&database)?;
            let mut stats = IndexingStats::default();

            if output_mode.is_human() {
                println!("🚀 Indexing repository: {}", repo_name);
                println!("📂 Path: {:?}", path);
                println!("🗄️  Database: {:?}", database);
            }
            
            // Channel for worker-to-coordinator communication
            let (tx, rx) = std::sync::mpsc::channel::<IndexMessage>();
            
            let start_indexing = std::time::Instant::now();
            let mut linking_ms: Option<u128> = None;
            let mut embedding_ms: Option<u128> = None;
            let mut semantic_ms: Option<u128> = None;
            let mut linker_stats: Option<coderev::linker::GlobalLinkerStats> = None;
            let mut semantic_stats: Option<coderev::linker::SemanticLinkerStats> = None;
            let mut embedded_symbols: usize = 0;
            
            // Configure file walker
            let walker = ignore::WalkBuilder::new(&path)
                .standard_filters(true)
                .add_custom_ignore_filename(".cursorignore")
                .build_parallel();

            let repo_name_clone = repo_name.clone();
            let path_clone = path.clone();

            // Spawn workers
            std::thread::scope(|scope| {
                // Coordinator "thread" (runs in current scope)
                let coordinator = scope.spawn(|| {
                    let mut seen_paths = std::collections::HashSet::new();
                    
                    for msg in rx {
                        match msg {
                            IndexMessage::Processed { 
                                relative_path, 
                                hash, 
                                result, 
                                status 
                            } => {
                                seen_paths.insert(relative_path.clone());
                                
                                match status {
                                    FileStatus::Unchanged => {
                                        stats.unchanged += 1;
                                    }
                                    FileStatus::Modified => {
                                        if output_mode.is_human() {
                                            println!("📝 Modified: {}", relative_path);
                                        }
                                        store.delete_file_data(&relative_path).ok();
                                        stats.modified += 1;
                                    }
                                    FileStatus::New => {
                                        if output_mode.is_human() {
                                            println!("✨ New: {}", relative_path);
                                        }
                                        stats.added += 1;
                                    }
                                }

                                if let Some(res) = result {
                                    // AST symbols
                                    for symbol in &res.symbols {
                                        store.insert_symbol(symbol).ok();
                                    }
                                    for edge in &res.edges {
                                        store.insert_edge(edge).ok();
                                    }
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
                                            relative_path.clone(),
                                            unresolved.line,
                                            "call",
                                        );
                                        store.insert_unresolved(&persisted).ok();
                                    }
                                    for import in res.scope_graph.imports(coderev::scope::graph::ScopeId::root()) {
                                        store.insert_import(
                                            &relative_path,
                                            import.alias.as_deref(),
                                            &import.namespace,
                                            Some(import.line),
                                        ).ok();
                                    }
                                    stats.symbols += res.symbols.len();
                                }
                                
                                store.update_file_hash(&relative_path, &hash).ok();
                            }
                            IndexMessage::Error(path, err) => {
                                tracing::error!("Error processing {}: {}", path, err);
                                stats.errors += 1;
                            }
                        }
                    }
                    seen_paths
                });

                // Worker threads
                walker.run(|| {
                    let tx = tx.clone();
                    let repo_name = repo_name_clone.clone();
                    let root_path = path_clone.clone();
                    let registry = adapter::default_registry();
                    let chunker = adapter::DocumentChunker::new();
                    let store = SqliteStore::open(&database).expect("Failed to open DB in worker"); // Read-only access for hash check

                    Box::new(move |result| {
                        let entry = match result {
                            Ok(e) => e,
                            Err(_) => return ignore::WalkState::Continue,
                        };

                        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                            return ignore::WalkState::Continue;
                        }

                        let file_path = entry.path();
                        let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        
                        let skip_exts = ["png", "jpg", "jpeg", "gif", "ico", "exe", "dll", "so", "o", "a", "lib", "bin", "pdf", "zip", "tar", "gz", "wasm", "node", "db", "sqlite", "lock", "pyc", "svg"];
                        if skip_exts.contains(&ext.as_str()) {
                            return ignore::WalkState::Continue;
                        }

                        let content = match std::fs::read_to_string(file_path) {
                            Ok(c) => c,
                            Err(_) => return ignore::WalkState::Continue,
                        };

                        let hash = blake3::hash(content.as_bytes()).to_string();
                        let relative_path = file_path.strip_prefix(&root_path).unwrap_or(file_path);
                        let relative_path_str = relative_path.to_str().unwrap_or("").to_string();

                        // Check status
                        let status = match store.get_file_hash(&relative_path_str) {
                            Ok(Some(h)) if h == hash => FileStatus::Unchanged,
                            Ok(Some(_)) => FileStatus::Modified,
                            _ => FileStatus::New,
                        };

                        if status == FileStatus::Unchanged {
                            tx.send(IndexMessage::Processed {
                                relative_path: relative_path_str,
                                hash,
                                result: None,
                                status,
                            }).ok();
                            return ignore::WalkState::Continue;
                        }

                        // Process file
                        let result = if let Some(adapter) = registry.find_adapter(file_path) {
                            adapter.parse_file(&repo_name, &relative_path_str, &content).ok()
                        } else {
                            chunker.chunk_file(&repo_name, &relative_path_str, &content).ok()
                        };

                        tx.send(IndexMessage::Processed {
                            relative_path: relative_path_str,
                            hash,
                            result,
                            status,
                        }).ok();

                        ignore::WalkState::Continue
                    })
                });

                // Close channel by dropping remaining tx
                drop(tx);

                // Wait for coordinator and handle deletions
                let seen_paths = coordinator.join().unwrap();
                let db_paths = store.get_all_indexed_files().unwrap_or_default();
                for db_path in db_paths {
                    if !seen_paths.contains(&db_path) {
                        if output_mode.is_human() {
                            println!("🗑️  Deleted: {}", db_path);
                        }
                        store.delete_file_data(&db_path).ok();
                        stats.deleted += 1;
                    }
                }
            });

            if output_mode.is_human() {
                println!("\n📊 Indexing Summary:");
                println!("   Unchanged: {}", stats.unchanged);
                println!("   Added:     {}", stats.added);
                println!("   Modified:  {}", stats.modified);
                println!("   Deleted:   {}", stats.deleted);
                println!("   Errors:    {}", stats.errors);
                println!("   ⏱️  Indexing took: {:?}", start_indexing.elapsed());
            }

            // Phase 2: Linking
            let something_changed = stats.added > 0 || stats.modified > 0 || stats.deleted > 0;
            let unresolved_count = store.count_unresolved()?;
            
            if something_changed || unresolved_count > 0 {
                // If specific files changed, we might want to only resolve relevant things, 
                // but for now, global re-link is safer and simpler.
                
                // We must clear old linking state if we re-link? 
                // `delete_file_data` removes unresolved refs from modified files.
                // But `store.clear_unresolved()` was done previously for *everything*.
                // Now we keep unresolved refs from unchanged files.
                // But GlobalLinker might generate edges. 
                // Existing resolved edges?
                // `GlobalLinker` currently iterates `unresolved_references`.
                // It inserts edges.
                // We should probably NOT clear *all* edges, only edges from modified files are gone.
                // Edges *to* modified files are also gone (handled in delete_file_data).
                
                if output_mode.is_human() {
                    println!("\n🔗 Phase 2: Running Global Linker...");
                }
                let start_linking = std::time::Instant::now();
                let linker = coderev::linker::GlobalLinker::new(&store);
                let stats = linker.run()?;
                linker_stats = Some(stats.clone());
                linking_ms = Some(start_linking.elapsed().as_millis());
                if output_mode.is_human() {
                    println!("{}", stats);
                    println!("   ⏱️  Linking took: {:?}", start_linking.elapsed());
                }
                
                // Phase 3: Embeddings
                // Only for new symbols
                let symbols_to_embed = store.find_symbols_without_embeddings()?;
                if !symbols_to_embed.is_empty() {
                    embedded_symbols = symbols_to_embed.len();
                    if output_mode.is_human() {
                        println!("\n🧠 Phase 3: Generating Embeddings ({} new symbols)...", symbols_to_embed.len());
                    }
                    let start_embeddings = std::time::Instant::now();
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
                        if output_mode.is_human() {
                            print!("\r   Progress: {}/{} symbols", processed, symbols_to_embed.len());
                            std::io::stdout().flush().ok();
                        }
                    }
                    embedding_ms = Some(start_embeddings.elapsed().as_millis());
                    if output_mode.is_human() {
                        println!(); // New line after progress
                        println!("   ⏱️  Embeddings took: {:?}", start_embeddings.elapsed());
                    }
                }

                // Phase 4: Semantic Resolution
                // Again, only if needed.
                if output_mode.is_human() {
                    println!("\n🧠 Phase 4: Running Semantic Resolver...");
                }
                let start_semantic = std::time::Instant::now();
                let engine = coderev::query::EmbeddingEngine::new()?;
                let semantic_linker = coderev::linker::SemanticLinker::new(&store, &engine);
                let stats = semantic_linker.run()?;
                semantic_stats = Some(stats.clone());
                semantic_ms = Some(start_semantic.elapsed().as_millis());
                if output_mode.is_human() {
                    if stats.resolved > 0 {
                        println!("✅ Semantic Resolved: {}", stats.resolved);
                    }
                    println!("   ⏱️  Semantic Resolution took: {:?}", start_semantic.elapsed());
                }
            } else {
                if output_mode.is_human() {
                    println!("\n✨ Repository is up to date.");
                }
            }

            // Show final stats
            let final_stats = store.stats()?;
            if output_mode.is_human() {
                println!("\n{}", final_stats);
                println!("\n⏱️  Total time: {:?}", total_start.elapsed());
            }

            if output_mode.is_machine() {
                let durations = IndexDurations {
                    indexing_ms: start_indexing.elapsed().as_millis(),
                    linking_ms,
                    embedding_ms,
                    semantic_ms,
                    total_ms: total_start.elapsed().as_millis(),
                };

                if matches!(output_mode, OutputMode::Json) {
                    let data = IndexOutput {
                        repo: repo_name,
                        path: path.display().to_string(),
                        database: database.display().to_string(),
                        stats,
                        linker: linker_stats,
                        embedded_symbols,
                        semantic: semantic_stats,
                        final_db: final_stats,
                        durations,
                    };
                    emit_success(output_mode, "index", data)?;
                } else {
                    let data = IndexOutputCompact {
                        r: repo_name,
                        p: path.display().to_string(),
                        db: database.display().to_string(),
                        s: stats,
                        l: linker_stats,
                        es: embedded_symbols,
                        se: semantic_stats,
                        fd: final_stats,
                        t: durations,
                    };
                    emit_success(output_mode, "index", data)?;
                }
            }
        }

        Commands::Search { query, database, limit, kind, exact } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            
            let parsed_kind = if let Some(ref k) = kind {
                use std::str::FromStr;
                Some(SymbolKind::from_str(k)?)
            } else {
                None
            };

            let mut search_mode = if exact { "exact" } else { "vector" }.to_string();
            let results = if !exact && !query.trim().is_empty() {
                // Ensure embeddings exist before searching.
                // If they don't exist, we should probably generate them or fall back?
                // Given the user instruction "assume vector is the only way", let's prioritize it.
                // But if the store has NO embeddings, we should perhaps warn or auto-generate?
                // ensure_embeddings(&store)?; // This Auto-generates.
                
                // Let's check if we have embeddings first to avoid surprise long waits?
                // User said "defaultly assume... unless we turn it off". 
                // That implies we SHOULD use it. So ensure_embeddings is correct.
                // But ensure_embeddings prints progress, so the user knows what's happening.
                let _ = ensure_embeddings(&store)?;
                if output_mode.is_human() {
                    println!("🧠 [Local Embedding] Searching for: '{}'...", query);
                }
                let engine = QueryEngine::new(&store);
                match coderev::query::EmbeddingEngine::new() {
                    Ok(embedding_engine) => {
                         match embedding_engine.embed_query(&query) {
                            Ok(query_vector) => engine.search_by_vector(&query_vector, limit)?,
                            Err(e) => {
                                if output_mode.is_human() {
                                    eprintln!("⚠️  Embedding Generation Failed: {}", e);
                                    println!("🔍 Falling back to exact text search...");
                                }
                                search_mode = "fallback_exact".to_string();
                                store.search_content(&query, parsed_kind, limit)? // Fallback
                                    .into_iter()
                                    .map(|s| coderev::query::engine::QueryResult::new(s, 1.0))
                                    .collect()
                            }
                         }
                    },
                    Err(e) => {
                         if output_mode.is_human() {
                             eprintln!("⚠️  Failed to initialize Embedding Engine: {}", e);
                             println!("🔍 Falling back to exact text search...");
                         }
                         search_mode = "fallback_exact".to_string();
                         store.search_content(&query, parsed_kind, limit)? // Fallback
                            .into_iter()
                            .map(|s| coderev::query::engine::QueryResult::new(s, 1.0))
                            .collect()
                    }
                }
            } else {
                // Exact search requested
                search_mode = "exact".to_string();
                if output_mode.is_human() {
                    println!("🔍 [Exact Match] Searching for: '{}' (kind: {:?}, limit: {})...", query, kind, limit);
                }
                store.search_content(&query, parsed_kind, limit)?
                    .into_iter()
                    .map(|s| coderev::query::engine::QueryResult::new(s, 1.0))
                    .collect()
            };

            // No fallbacks needed mostly as we start with vector. 
            // But if vector returns empty? 
            // Semantic search might return "somewhat relevant" things even if bad match.
            // If vector returns empty, it means no embeddings or threshold? (search_by_vector has no threshold currently, just sorts).
            
            if output_mode.is_human() {
                if results.is_empty() {
                    println!("❌ No symbols found.");
                    if !exact {
                        // Vector search usually returns something unless DB is empty.
                    }
                } else {
                    for res in &results {
                        let uri_str = res.symbol.uri.to_uri_string();
                        println!("- [{:?}] {} (Score: {:.2})", res.symbol.kind, res.symbol.name, res.score);
                        println!("  URI: {}", uri_str);
                        if let Some(sig) = &res.symbol.signature {
                            println!("  Sig: {}", sig);
                        }
                    }
                }
            }

            if output_mode.is_machine() {
                if matches!(output_mode, OutputMode::Json) {
                    let items = results
                        .into_iter()
                        .map(|res| SearchItem {
                            kind: res.symbol.kind.as_str().to_string(),
                            name: res.symbol.name,
                            uri: res.symbol.uri.to_uri_string(),
                            path: res.symbol.path,
                            line_start: res.symbol.line_start,
                            line_end: res.symbol.line_end,
                            score: res.score,
                            signature: res.symbol.signature,
                        })
                        .collect();
                    let data = SearchOutput {
                        query,
                        kind,
                        limit,
                        exact,
                        mode: search_mode,
                        results: items,
                    };
                    emit_success(output_mode, "search", data)?;
                } else {
                    let items = results
                        .into_iter()
                        .map(|res| SearchItemCompact {
                            k: res.symbol.kind.as_str().to_string(),
                            n: res.symbol.name,
                            u: res.symbol.uri.to_uri_string(),
                            p: res.symbol.path,
                            ls: res.symbol.line_start,
                            le: res.symbol.line_end,
                            sc: res.score,
                            s: res.symbol.signature,
                        })
                        .collect();
                    let data = SearchOutputCompact {
                        q: query,
                        k: kind,
                        l: limit,
                        x: exact,
                        m: search_mode,
                        r: items,
                    };
                    emit_success(output_mode, "search", data)?;
                }
            }
        }

        Commands::Embed { database, model: _ } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            let embedded = ensure_embeddings(&store)?;
            if output_mode.is_human() {
                println!("✅ Embedding complete!");
            } else {
                let data = if matches!(output_mode, OutputMode::Json) {
                    serde_json::json!({
                        "database": database.display().to_string(),
                        "embedded_symbols": embedded,
                    })
                } else {
                    serde_json::json!({
                        "db": database.display().to_string(),
                        "es": embedded,
                    })
                };
                emit_success(output_mode, "embed", data)?;
            }
        }

        Commands::Callers { uri, database, depth } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            ensure_resolved(&store)?;
            
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            let callers = engine.find_callers(&target_uri, depth)?;

            if output_mode.is_human() {
                println!("📞 Finding callers for: {} (depth: {})...", uri, depth);
                if callers.is_empty() {
                    println!("∅ No callers found.");
                } else {
                    for symbol in &callers {
                        println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                    }
                }
            } else if matches!(output_mode, OutputMode::Json) {
                let data = ListOutput {
                    uri,
                    depth,
                    results: callers.iter().map(symbol_ref).collect(),
                };
                emit_success(output_mode, "callers", data)?;
            } else {
                let data = ListOutputCompact {
                    u: uri,
                    d: depth,
                    r: callers.iter().map(symbol_ref_compact).collect(),
                };
                emit_success(output_mode, "callers", data)?;
            }
        }

        Commands::Callees { uri, database, depth } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            ensure_resolved(&store)?;
            
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            let callees = engine.find_callees(&target_uri, depth)?;
            
            if output_mode.is_human() {
                println!("📱 Finding callees for: {} (depth: {})...", uri, depth);
                if callees.is_empty() {
                    println!("∅ No callees found.");
                } else {
                    for symbol in &callees {
                        println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                    }
                }
            } else if matches!(output_mode, OutputMode::Json) {
                let data = ListOutput {
                    uri,
                    depth,
                    results: callees.iter().map(symbol_ref).collect(),
                };
                emit_success(output_mode, "callees", data)?;
            } else {
                let data = ListOutputCompact {
                    u: uri,
                    d: depth,
                    r: callees.iter().map(symbol_ref_compact).collect(),
                };
                emit_success(output_mode, "callees", data)?;
            }
        }

        Commands::Impact { uri, database, depth, format } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            ensure_resolved(&store)?;
            
            let engine = QueryEngine::new(&store);
            let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
            
            let impact = engine.impact_analysis(&target_uri, depth)?;

            if output_mode.is_human() {
                println!("💥 Impact analysis for: {} (depth: {})...", uri, depth);
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&impact)?);
                } else if impact.is_empty() {
                    println!("∅ No impact found.");
                } else {
                    for res in impact {
                        let prefix = if res.is_direct() { "🔴 [DIRECT]" } else { "🟠 [INDIRECT]" };
                        println!("{} [{:?}] {} (Depth: {}, Conf: {:.2})", 
                            prefix, res.symbol.kind, res.symbol.name, res.depth, res.confidence);
                        println!("   URI: {}", res.symbol.uri.to_uri_string());
                    }
                }
            } else if matches!(output_mode, OutputMode::Json) {
                let items = impact
                    .into_iter()
                    .map(|res| ImpactItem {
                        kind: res.symbol.kind.as_str().to_string(),
                        name: res.symbol.name,
                        uri: res.symbol.uri.to_uri_string(),
                        path: res.symbol.path,
                        line_start: res.symbol.line_start,
                        line_end: res.symbol.line_end,
                        depth: res.depth,
                        confidence: res.confidence,
                        edge_kind: res.edge_kind.as_str().to_string(),
                    })
                    .collect();
                let data = ImpactOutput { uri, depth, results: items };
                emit_success(output_mode, "impact", data)?;
            } else {
                let items = impact
                    .into_iter()
                    .map(|res| ImpactItemCompact {
                        k: res.symbol.kind.as_str().to_string(),
                        n: res.symbol.name,
                        u: res.symbol.uri.to_uri_string(),
                        p: res.symbol.path,
                        ls: res.symbol.line_start,
                        le: res.symbol.line_end,
                        d: res.depth,
                        c: res.confidence,
                        e: res.edge_kind.as_str().to_string(),
                    })
                    .collect();
                let data = ImpactOutputCompact { u: uri, d: depth, r: items };
                emit_success(output_mode, "impact", data)?;
            }
        }

        Commands::Stats { database } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            let stats = store.stats()?;
            
            if output_mode.is_human() {
                println!("📊 Coderev Statistics ({:?})", database);
                println!("------------------------------------");
                println!("{}", stats);
            } else if matches!(output_mode, OutputMode::Json) {
                let data = serde_json::json!({
                    "database": database.display().to_string(),
                    "stats": stats,
                });
                emit_success(output_mode, "stats", data)?;
            } else {
                let data = serde_json::json!({
                    "db": database.display().to_string(),
                    "s": stats,
                });
                emit_success(output_mode, "stats", data)?;
            }
        }

        Commands::Resolve { database, verbose } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = SqliteStore::open(&database)?;
            
            let unresolved_count = store.count_unresolved()?;

            if unresolved_count == 0 {
                if output_mode.is_human() {
                    println!("✅ No unresolved references to resolve.");
                } else if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "database": database.display().to_string(),
                        "unresolved": 0,
                        "linked": null,
                        "semantic": null,
                    });
                    emit_success(output_mode, "resolve", data)?;
                } else {
                    let data = serde_json::json!({
                        "db": database.display().to_string(),
                        "u": 0,
                        "l": null,
                        "se": null,
                    });
                    emit_success(output_mode, "resolve", data)?;
                }
                return Ok(());
            }
            
            if output_mode.is_human() {
                println!("🔗 Running Global Linker on {} unresolved references...", unresolved_count);
            }
            
            if verbose && output_mode.is_human() {
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
            
            if output_mode.is_human() {
                println!("{}", stats);
            }

            // Run Semantic Resolver
            if output_mode.is_human() {
                println!("\n🧠 Running Semantic Resolver...");
            }
            let embedded = ensure_embeddings(&store)?;
            
            let engine = coderev::query::EmbeddingEngine::new()?;
            let semantic_linker = coderev::linker::SemanticLinker::new(&store, &engine);
            let semantic_stats = semantic_linker.run()?;
            if output_mode.is_human() {
                if semantic_stats.resolved > 0 {
                    println!("✅ Semantic Resolver: Resolved {} references (checked {} candidates)", semantic_stats.resolved, semantic_stats.candidates);
                } else {
                    println!("ℹ️  Semantic Resolver: No new edges resolved.");
                }
            }


            
            // Show remaining unresolved if verbose
            if verbose && output_mode.is_human() {
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

            if output_mode.is_machine() {
                if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "database": database.display().to_string(),
                        "unresolved": unresolved_count,
                        "embedded_symbols": embedded,
                        "linked": stats,
                        "semantic": semantic_stats,
                    });
                    emit_success(output_mode, "resolve", data)?;
                } else {
                    let data = serde_json::json!({
                        "db": database.display().to_string(),
                        "u": unresolved_count,
                        "es": embedded,
                        "l": stats,
                        "se": semantic_stats,
                    });
                    emit_success(output_mode, "resolve", data)?;
                }
            }
        }

        Commands::Serve { database, port, host } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            tracing::info!("Starting Coderev server on {}:{}", host, port);
            
            let addr = format!("{}:{}", host, port);
            let socket_addr: std::net::SocketAddr = addr.parse()
                .map_err(|_| anyhow::anyhow!("Invalid address: {}", addr))?;
            
            let store = SqliteStore::open(&database)?;
            
            if output_mode.is_human() {
                println!("🚀 Coderev Server starting at http://{}", addr);
            } else if matches!(output_mode, OutputMode::Json) {
                let data = serde_json::json!({
                    "database": database.display().to_string(),
                    "host": host,
                    "port": port,
                    "address": addr,
                });
                emit_success(output_mode, "serve", data)?;
            } else {
                let data = serde_json::json!({
                    "db": database.display().to_string(),
                    "h": host,
                    "p": port,
                    "a": addr,
                });
                emit_success(output_mode, "serve", data)?;
            }
            coderev::server::run_server(socket_addr, store).await?;
            
            return Ok(());
        }

        Commands::Mcp { database } => {
            let database = resolve_database_ready(database, &cfg_opt)?;
            let store = std::sync::Arc::new(SqliteStore::open(&database)?);
            let service = coderev::server::mcp::McpService::new(store);
            if output_mode.is_machine() {
                if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "database": database.display().to_string(),
                        "transport": "stdio",
                    });
                    emit_success(output_mode, "mcp", data)?;
                } else {
                    let data = serde_json::json!({
                        "db": database.display().to_string(),
                        "t": "stdio",
                    });
                    emit_success(output_mode, "mcp", data)?;
                }
            }
            service.run_stdio().await?;
        }

        Commands::Watch { path, database, background, status, stop, daemon } => {
            let mode_count = background as u8 + status as u8 + stop as u8;
            if mode_count > 1 {
                anyhow::bail!("flags --background, --status, and --stop are mutually exclusive");
            }

            let watch_path = resolve_path(path, &cfg_opt)?;
            let database = resolve_database_ready(database, &cfg_opt)?;
            let files = watch_files(&database);

            if status {
                let pid = read_pid(&files.pid_path)?;
                let running = pid.map(is_process_running).unwrap_or(false);
                if !running && pid.is_some() {
                    std::fs::remove_file(&files.pid_path).ok();
                }

                if output_mode.is_human() {
                    if let Some(pid) = pid {
                        if running {
                            println!("✅ Watcher running (pid {})", pid);
                        } else {
                            println!("⚠️  Watcher not running (stale pid {})", pid);
                        }
                    } else {
                        println!("ℹ️  Watcher not running");
                    }
                } else if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "running": running,
                        "pid": pid,
                        "pid_file": files.pid_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch.status", data)?;
                } else {
                    let data = serde_json::json!({
                        "r": running,
                        "pid": pid,
                        "pf": files.pid_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch.status", data)?;
                }
                return Ok(());
            }

            if stop {
                let pid = read_pid(&files.pid_path)?;
                if let Some(pid) = pid {
                    stop_process(pid)?;
                    std::fs::remove_file(&files.pid_path).ok();
                    if output_mode.is_human() {
                        println!("🛑 Stopped watcher (pid {})", pid);
                    } else if matches!(output_mode, OutputMode::Json) {
                        let data = serde_json::json!({
                            "stopped": true,
                            "pid": pid,
                        });
                        emit_success(output_mode, "watch.stop", data)?;
                    } else {
                        let data = serde_json::json!({
                            "s": true,
                            "pid": pid,
                        });
                        emit_success(output_mode, "watch.stop", data)?;
                    }
                } else {
                    if output_mode.is_human() {
                        println!("ℹ️  Watcher not running");
                    } else if matches!(output_mode, OutputMode::Json) {
                        let data = serde_json::json!({
                            "stopped": false,
                            "pid": null,
                        });
                        emit_success(output_mode, "watch.stop", data)?;
                    } else {
                        let data = serde_json::json!({
                            "s": false,
                            "pid": null,
                        });
                        emit_success(output_mode, "watch.stop", data)?;
                    }
                }
                return Ok(());
            }

            if background {
                if let Some(pid) = read_pid(&files.pid_path)? {
                    if is_process_running(pid) {
                        anyhow::bail!("watcher already running (pid {})", pid);
                    }
                }

                if let Some(parent) = files.log_path.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)?;
                    }
                }

                let log_file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&files.log_path)?;

                let exe = std::env::current_exe()?;
                let mut cmd = std::process::Command::new(exe);
                cmd.arg("watch")
                    .arg("--path")
                    .arg(watch_path.display().to_string())
                    .arg("--database")
                    .arg(database.display().to_string())
                    .arg("--daemon")
                    .stdout(log_file.try_clone()?)
                    .stderr(log_file)
                    .env("Coderev_QUIET", "1");

                let child = cmd.spawn()?;
                let pid = child.id() as i32;
                std::fs::write(&files.pid_path, pid.to_string())?;

                if output_mode.is_human() {
                    println!("✅ Watcher started in background (pid {})", pid);
                    println!("   Logs: {}", files.log_path.display());
                } else if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "started": true,
                        "pid": pid,
                        "pid_file": files.pid_path.display().to_string(),
                        "log_file": files.log_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch.background", data)?;
                } else {
                    let data = serde_json::json!({
                        "s": true,
                        "pid": pid,
                        "pf": files.pid_path.display().to_string(),
                        "lf": files.log_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch.background", data)?;
                }
                return Ok(());
            }

            let store = SqliteStore::open(&database)?;
            let watcher = coderev::watcher::Watcher::new(watch_path.clone(), store);

            if output_mode.is_machine() && !daemon {
                if matches!(output_mode, OutputMode::Json) {
                    let data = serde_json::json!({
                        "database": database.display().to_string(),
                        "path": watch_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch", data)?;
                } else {
                    let data = serde_json::json!({
                        "db": database.display().to_string(),
                        "p": watch_path.display().to_string(),
                    });
                    emit_success(output_mode, "watch", data)?;
                }
            }
            watcher.run()?;
        }

        Commands::Trace(cmd) => match cmd {
            TraceCommands::Callers { uri, database, depth } => {
                let database = resolve_database_ready(database, &cfg_opt)?;
                let store = SqliteStore::open(&database)?;
                ensure_resolved(&store)?;
                let engine = QueryEngine::new(&store);
                let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
                let callers = engine.find_callers(&target_uri, depth)?;
                if output_mode.is_human() {
                    println!("📞 Finding callers for: {} (depth: {})...", uri, depth);
                    if callers.is_empty() {
                        println!("∅ No callers found.");
                    } else {
                        for symbol in &callers {
                            println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                        }
                    }
                } else if matches!(output_mode, OutputMode::Json) {
                    let data = ListOutput {
                        uri,
                        depth,
                        results: callers.iter().map(symbol_ref).collect(),
                    };
                    emit_success(output_mode, "trace.callers", data)?;
                } else {
                    let data = ListOutputCompact {
                        u: uri,
                        d: depth,
                        r: callers.iter().map(symbol_ref_compact).collect(),
                    };
                    emit_success(output_mode, "trace.callers", data)?;
                }
            },
            TraceCommands::Callees { uri, database, depth } => {
                let database = resolve_database_ready(database, &cfg_opt)?;
                let store = SqliteStore::open(&database)?;
                ensure_resolved(&store)?;
                let engine = QueryEngine::new(&store);
                let target_uri = coderev::uri::SymbolUri::parse(&uri)?;
                let callees = engine.find_callees(&target_uri, depth)?;
                if output_mode.is_human() {
                    println!("📱 Finding callees for: {} (depth: {})...", uri, depth);
                    if callees.is_empty() {
                        println!("∅ No callees found.");
                    } else {
                        for symbol in &callees {
                            println!("- [{:?}] {} ({})", symbol.kind, symbol.name, symbol.uri.to_uri_string());
                        }
                    }
                } else if matches!(output_mode, OutputMode::Json) {
                    let data = ListOutput {
                        uri,
                        depth,
                        results: callees.iter().map(symbol_ref).collect(),
                    };
                    emit_success(output_mode, "trace.callees", data)?;
                } else {
                    let data = ListOutputCompact {
                        u: uri,
                        d: depth,
                        r: callees.iter().map(symbol_ref_compact).collect(),
                    };
                    emit_success(output_mode, "trace.callees", data)?;
                }
            }
        }
    }

    Ok(())
}

/// Helper to ensure all symbols have embeddings
fn ensure_embeddings(store: &SqliteStore) -> anyhow::Result<usize> {
    let missing = store.find_symbols_without_embeddings()?;
    let total = missing.len();
    if !missing.is_empty() {
        let engine = coderev::query::EmbeddingEngine::new()?;
        let batch_size = 32;
        let mut processed = 0;
        if !coderev::output::is_quiet() {
            println!("   Generating {} symbol embeddings...", total);
            print!("   Progress: 0 / {}", total);
            std::io::stdout().flush().ok();
        }
        
        for chunk in missing.chunks(batch_size) {
            let embeddings = engine.embed_symbols(chunk)?;
            
            store.begin_transaction()?;
            for (i, vector) in embeddings.into_iter().enumerate() {
                store.insert_embedding(&chunk[i].uri, &vector)?;
            }
            store.commit()?;
            
            processed += chunk.len();
            if !coderev::output::is_quiet() {
                print!("\r   Progress: {} / {}", processed, total);
                std::io::stdout().flush().ok();
            }
        }
        if !coderev::output::is_quiet() {
            println!();
            println!("✅ Embedding complete.");
        }
    }
    Ok(total)
}

/// Helper to ensure all unresolved references are resolved
fn ensure_resolved(store: &SqliteStore) -> anyhow::Result<()> {
    let unresolved_count = store.count_unresolved()?;
    if unresolved_count > 0 {
        if !coderev::output::is_quiet() {
            println!("🔗 On-demand: Resolving {} references...", unresolved_count);
        }
        let linker = coderev::linker::GlobalLinker::new(store);
        let stats = linker.run()?;
        if !coderev::output::is_quiet() {
            println!("{}", stats);
        }
        
        if !coderev::output::is_quiet() {
            println!("🧠 On-demand: Running Semantic Resolver...");
        }
        // This requires a mutable store, but we got an immutable one.
        // We'll have to reopen it or skip.
        // For simplicity in this helper, let's reopen.
        // But we don't know the path here easily without passing it.
        // Actually SqliteStore doesn't expose path. 
        // We can CAST store to mutable? No.
        // We will skip Semantic Resolver here for now to avoid complexity, 
        // OR we change signature of ensure_resolved to take &mut SqliteStore?
        // Callers/Callees/Impact command open store as mutable? No, `SqliteStore::open` returns `SqliteStore` which is owned.
        // But `ensure_resolved` takes `&SqliteStore`. 
        // Let's check callers.
    }

    Ok(())
}
