use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;
use crate::storage::SqliteStore;

pub struct Watcher {
    path: PathBuf,
    store: SqliteStore,
}

impl Watcher {
    pub fn new(path: PathBuf, store: SqliteStore) -> Self {
        Self { path, store }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        let (tx, rx) = channel();
        
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        
        watcher.watch(&self.path, RecursiveMode::Recursive)?;
        
        println!("👀 Watching for changes in {:?}...", self.path);
        
        for res in rx {
            match res {
                Ok(event) => {
                    self.handle_event(event);
                },
                Err(e) => println!("watch error: {:?}", e),
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
        let relative_path = path.strip_prefix(&self.path).unwrap_or(path);
        let relative_path_str = relative_path.to_str().unwrap_or("").to_string();
        println!("🗑️  File removed: {}", relative_path_str);
        self.store.delete_file_data(&relative_path_str).ok();
    }

    fn process_file(&self, path: &std::path::Path) {
        let relative_path = path.strip_prefix(&self.path).unwrap_or(path);
        let relative_path_str = relative_path.to_str().unwrap_or("").to_string();
        
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let skip_exts = ["png", "jpg", "jpeg", "gif", "ico", "exe", "dll", "so", "o", "a", "lib", "bin", "pdf", "zip", "tar", "gz", "wasm", "node", "db", "sqlite", "lock", "pyc", "svg", "git"];
        if skip_exts.contains(&ext.as_str()) {
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let hash = blake3::hash(content.as_bytes()).to_string();
        
        // Check if changed
        if let Ok(Some(existing_hash)) = self.store.get_file_hash(&relative_path_str) {
            if existing_hash == hash {
                return;
            }
        }

        println!("📝 Processing change: {}", relative_path_str);

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
        }
        
        self.store.update_file_hash(&relative_path_str, &hash).ok();
        println!("✅ Updated: {}", relative_path_str);
    }
}
