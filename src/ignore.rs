use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

pub struct IgnoreFilter {
    inner: Gitignore,
}

impl IgnoreFilter {
    pub fn new(root: &Path, extra_excludes: Option<&[String]>) -> Self {
        let mut builder = GitignoreBuilder::new(root);
        
        // 1. Load from .gitignore and .ignore
        builder.add(root.join(".gitignore"));
        builder.add(root.join(".ignore"));

        // 2. Add defaults (global)
        let defaults = [
            // Noise directories
            "target/", "node_modules/", "venv/", ".venv/", "vendor/", 
            "dist/", "build/", "out/", "coverage/", "__pycache__/", "egg-info/",
            ".git/", ".coderev/", ".vscode/", ".idea/",
            
            // Database files
            "*.db", "*.sqlite", "*.sqlite3", "*.wal", "*.shm",
            
            // Noise extensions
            "*.lock", "*.log", "*.pyc", "*.pyo", "*.pyd", "*.class", "*.jar",
            "*.png", "*.jpg", "*.jpeg", "*.gif", "*.ico", "*.svg", "*.webp", "*.avif",
            "*.mp4", "*.webm", "*.mp3", "*.wav",
            "*.exe", "*.dll", "*.so", "*.dylib", "*.o", "*.a", "*.lib", "*.bin",
            "*.pdf", "*.zip", "*.tar", "*.gz", "*.7z", "*.rar", "*.wasm", "*.node",
        ];

        for pattern in defaults {
            // We ignore errors here as these correspond to static valid patterns
             builder.add_line(None, pattern).ok();
        }

        // 3. Add user config excludes
        if let Some(excludes) = extra_excludes {
            for pattern in excludes {
                builder.add_line(None, pattern).ok();
            }
        }

        Self {
            inner: builder.build().unwrap_or_else(|_| Gitignore::empty()),
        }
    }

    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        self.inner.matched(path, is_dir).is_ignore()
    }
}
