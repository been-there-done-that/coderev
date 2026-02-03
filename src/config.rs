use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoderevConfig {
    pub database: Option<String>,
    pub repo: Option<String>,
    pub path: Option<String>,
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("coderev.toml")
}

pub fn default_database_path_in(base: &Path) -> PathBuf {
    base.join(".coderev").join("coderev.db")
}

pub fn load_config(path: Option<&Path>) -> anyhow::Result<Option<CoderevConfig>> {
    let path = path.map(Path::to_path_buf).unwrap_or_else(default_config_path);
    if !path.exists() {
        return Ok(None);
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
