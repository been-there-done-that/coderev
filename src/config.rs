use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderevConfig {
    pub database: Option<String>,
    pub repo: Option<String>,
    pub path: Option<String>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl Default for CoderevConfig {
    fn default() -> Self {
        Self {
            database: None,
            repo: None,
            path: None,
            include: None,
            exclude: Some(Self::default_excludes()),
        }
    }
}

impl CoderevConfig {
    pub fn default_excludes() -> Vec<String> {
        vec![
            // Noise directories
            "target/".to_string(), "node_modules/".to_string(), "venv/".to_string(), 
            ".venv/".to_string(), "vendor/".to_string(), "dist/".to_string(), 
            "build/".to_string(), "out/".to_string(), "coverage/".to_string(), 
            "__pycache__/".to_string(), "egg-info/".to_string(),
            ".git/".to_string(), ".coderev/".to_string(), ".vscode/".to_string(), ".idea/".to_string(),
            
            // Database files
            "*.db".to_string(), "*.sqlite".to_string(), "*.sqlite3".to_string(), 
            "*.wal".to_string(), "*.shm".to_string(),
            
            // Noise extensions
            "*.lock".to_string(), "*.log".to_string(), "*.pyc".to_string(), 
            "*.pyo".to_string(), "*.pyd".to_string(), "*.class".to_string(), "*.jar".to_string(),
            "*.png".to_string(), "*.jpg".to_string(), "*.jpeg".to_string(), 
            "*.gif".to_string(), "*.ico".to_string(), "*.svg".to_string(), 
            "*.webp".to_string(), "*.avif".to_string(),
            "*.mp4".to_string(), "*.webm".to_string(), "*.mp3".to_string(), "*.wav".to_string(),
            "*.exe".to_string(), "*.dll".to_string(), "*.so".to_string(), 
            "*.dylib".to_string(), "*.o".to_string(), "*.a".to_string(), 
            "*.lib".to_string(), "*.bin".to_string(),
            "*.pdf".to_string(), "*.zip".to_string(), "*.tar".to_string(), 
            "*.gz".to_string(), "*.7z".to_string(), "*.rar".to_string(), 
            "*.wasm".to_string(), "*.node".to_string(),
        ]
    }
}

impl CoderevConfig {
    pub fn should_skip(&self, path: &Path, root: &Path) -> bool {
        let relative_path = path.strip_prefix(root).unwrap_or(path);
        let path_str = relative_path.to_string_lossy();

        if let Some(exclude) = &self.exclude {
            for pattern in exclude {
                // Try as regex if it looks like one (e.g. /pattern/)
                if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
                    let re_str = &pattern[1..pattern.len()-1];
                    if let Ok(re) = regex::Regex::new(re_str) {
                        if re.is_match(&path_str) {
                            return true;
                        }
                    }
                }
                
                if let Ok(glob) = glob::Pattern::new(pattern) {
                    if glob.matches(&path_str) {
                        return true;
                    }
                }
            }
        }

        if let Some(include) = &self.include {
            let mut matched = false;
            for pattern in include {
                if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
                    let re_str = &pattern[1..pattern.len()-1];
                    if let Ok(re) = regex::Regex::new(re_str) {
                        if re.is_match(&path_str) {
                            matched = true;
                            break;
                        }
                    }
                }

                if let Ok(glob) = glob::Pattern::new(pattern) {
                    if glob.matches(&path_str) {
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                return true;
            }
        }

        false
    }
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from(".coderev").join("coderev.toml")
}

pub fn default_config_path_in(base: &Path) -> PathBuf {
    base.join(".coderev").join("coderev.toml")
}

pub fn default_database_path_in(base: &Path) -> PathBuf {
    base.join(".coderev").join("coderev.db")
}

pub fn load_config(path: Option<&Path>) -> anyhow::Result<Option<CoderevConfig>> {
    let path = path.map(Path::to_path_buf).unwrap_or_else(default_config_path);
    if !path.exists() {
        // Return default config (with default excludes) if no file exists
        return Ok(Some(CoderevConfig::default()));
    }

    let contents = std::fs::read_to_string(&path)?;
    let config: CoderevConfig = toml::from_str(&contents)?;
    Ok(Some(config))
}

pub fn write_config(path: &Path, config: &CoderevConfig, force: bool) -> anyhow::Result<()> {
    if path.exists() && !force {
        anyhow::bail!("config already exists at {} (use --force to overwrite)", path.display());
    }

    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn ensure_db_dir(db_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

pub fn ensure_gitignore(project_root: &Path) -> anyhow::Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let entry = ".coderev/";

    if gitignore_path.exists() {
        let existing = std::fs::read_to_string(&gitignore_path)?;
        if existing.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
    }

    let mut content = String::new();
    if gitignore_path.exists() {
        content.push_str(&std::fs::read_to_string(&gitignore_path)?);
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }
    content.push_str(entry);
    content.push('\n');
    std::fs::write(&gitignore_path, content)?;
    Ok(())
}
