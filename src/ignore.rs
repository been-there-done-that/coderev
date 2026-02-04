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

        // 2. Add config excludes (which now include defaults)
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
