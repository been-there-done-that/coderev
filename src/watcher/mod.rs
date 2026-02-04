use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use crate::storage::SqliteStore;
use crate::ui;
use owo_colors::OwoColorize;

pub struct Watcher {
    path: PathBuf,
    store: SqliteStore,
    config: Option<crate::config::CoderevConfig>,
    embedding_engine: std::sync::Arc<std::sync::Mutex<Option<crate::query::EmbeddingEngine>>>,
    ignore_filter: crate::ignore::IgnoreFilter,
}

impl Watcher {
    pub fn new(path: PathBuf, store: SqliteStore, config: Option<crate::config::CoderevConfig>) -> Self {
        let extra_excludes = config.as_ref().and_then(|c| c.exclude.as_deref());
        let ignore_filter = crate::ignore::IgnoreFilter::new(&path, extra_excludes);
        
        Self { 
            path, 
            store,
            config,
            embedding_engine: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ignore_filter,
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        let (tx, rx) = channel();
        
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        
        watcher.watch(&self.path, RecursiveMode::Recursive)?;
        
        if !ui::is_quiet() {
            let repo_name = self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "repo".to_string());
            ui::header(&format!("Watching {}", repo_name));
            println!("📄 Path: {}", self.path.display().style(ui::theme().info.clone()));
            println!("👀 Standing by for changes...");
            println!();
        }
        
        // No async spawn needed here. We load lazily in process_file.
        
        for res in rx {
            match res {
                Ok(event) => {
                    self.handle_event(event);
                },
                Err(e) => {
                    if !ui::is_quiet() {
                        ui::error(&format!("watch error: {:?}", e));
                    }
                }
            }
        }
        
        Ok(())
    }

    fn handle_event(&self, event: notify::Event) {
        use notify::EventKind;
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.is_file() {
                        self.process_file(&path);
                    }
                }
            },
            EventKind::Remove(_) => {
                for path in event.paths {
                    self.remove_file(&path);
                }
            },
            _ => {}
        }
    }

    fn remove_file(&self, path: &std::path::Path) {
        if self.ignore_filter.is_ignored(path, false) {
             return;
        }

        let relative_path = path.strip_prefix(&self.path).unwrap_or(path);
        let relative_path_str = relative_path.to_str().unwrap_or("").to_string();
        
        if let Some(cfg) = &self.config {
            if cfg.should_skip(path, &self.path) {
                return;
            }
        }

        if !ui::is_quiet() {
            ui::file_deleted(&relative_path_str);
        }
        self.store.delete_file_data(&relative_path_str).ok();
    }

    fn process_file(&self, path: &std::path::Path) {
        // 0. Pre-filtering
        if self.ignore_filter.is_ignored(path, false) {
            return;
        }

        let relative_path = path.strip_prefix(&self.path).unwrap_or(path);
        let relative_path_str = relative_path.to_str().unwrap_or("").to_string();
        
        if let Some(cfg) = &self.config {
            if cfg.should_skip(path, &self.path) {
                return;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let hash = blake3::hash(content.as_bytes()).to_string();
        
        // Check if changed
        let mut is_new = true;
        if let Ok(Some(existing_hash)) = self.store.get_file_hash(&relative_path_str) {
            if existing_hash == hash {
                return;
            }
            is_new = false;
        }

        if !ui::is_quiet() {
            if is_new {
                ui::file_new(&relative_path_str);
            } else {
                ui::file_modified(&relative_path_str);
            }
        }

        // Delete old data
        self.store.delete_file_data(&relative_path_str).ok();

        // Parse
        let registry = crate::adapter::default_registry();
        let chunker = crate::adapter::DocumentChunker::new();
        
        // Assuming simple repo name
        let repo_name = self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "repo".to_string());

        let result = if let Some(adapter) = registry.find_adapter(path) {
            adapter.parse_file(&repo_name, &relative_path_str, &content).ok()
        } else {
            chunker.chunk_file(&repo_name, &relative_path_str, &content).ok()
        };

        if let Some(res) = result {
            for symbol in &res.symbols {
                self.store.insert_symbol(symbol).ok();
            }
            for edge in &res.edges {
                self.store.insert_edge(edge).ok();
            }
            // Insert unresolved refs
             for unresolved in res.scope_graph.unresolved_references() {
                 let (receiver, name) = if let Some((r, n)) = unresolved.name.rsplit_once('.') {
                     (Some(r.to_string()), n.to_string())
                 } else {
                     (None, unresolved.name.clone())
                 };

                 let persisted = crate::storage::PersistedUnresolvedReference::new(
                     unresolved.from_uri.to_uri_string(),
                     name,
                     receiver,
                     unresolved.scope.0 as i64,
                     relative_path_str.clone(),
                     unresolved.line,
                     "call",
                 );
                 self.store.insert_unresolved(&persisted).ok();
             }
             for import in res.scope_graph.imports(crate::scope::graph::ScopeId::root()) {
                 self.store.insert_import(
                     &relative_path_str,
                     import.alias.as_deref(),
                     &import.namespace,
                     Some(import.line),
                 ).ok();
             }

             // --- Background Linking & Embedding ---
             // 1. Resolve references for this file
             let linker = crate::linker::GlobalLinker::new(&self.store);
             if let Ok(stats) = linker.resolve_file(&relative_path_str) {
             if !ui::is_quiet() && stats.resolved > 0 {
                 ui::success(&format!("Resolved {} references in background", stats.resolved));
             }
             }

             // 2. Generate Embeddings (if not chunks)
             // We want to embed the newly inserted symbols.
             if let Ok(symbols) = self.store.find_symbols_by_file(&relative_path_str) {
                if !symbols.is_empty() {
                     // Check if cached, or initialize
                     let mut engine_guard = self.embedding_engine.lock().unwrap();
                      if engine_guard.is_none() {
                          match crate::query::EmbeddingEngine::new() {
                              Ok(e) => {
                                  *engine_guard = Some(e);
                                  if !ui::is_quiet() {
                                      ui::success("Embedding engine initialized");
                                  }
                              },
                              Err(e) => {
                                  if !ui::is_quiet() { 
                                      ui::error(&format!("Failed to init embedding engine: {}", e)); 
                                  }
                              }
                          }
                      }

                     if let Some(engine) = engine_guard.as_ref() {
                         if let Ok(embeddings) = engine.embed_symbols(&symbols) {
                             self.store.insert_embeddings_batch(&symbols, &embeddings).ok();
                              if !ui::is_quiet() {
                                  ui::success(&format!("Generated {} embeddings", embeddings.len()));
                              }
                         }
                     }
                }
             }
        }
        
        self.store.update_file_hash(&relative_path_str, &hash).ok();
        if !ui::is_quiet() {
            ui::success(&format!("Processed: {}", relative_path_str));
        }
    }
}
