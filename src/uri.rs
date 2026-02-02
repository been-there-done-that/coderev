//! Symbol URI - Global, stable identity for every code symbol
//!
//! Format: `codescope://<repo>/<path>#<kind>:<name>@<line>`
//!
//! Examples:
//! - `codescope://myrepo/src/auth.py#callable:validate_token@42`
//! - `codescope://myrepo/lib/db.ts#container:DatabaseClient@10`

use crate::{Error, Result};
use crate::symbol::SymbolKind;
use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

/// Global, stable URI for every symbol in the code graph.
///
/// This URI serves as the primary key for:
/// - Symbols
/// - Edges
/// - Embeddings
/// - Search results
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolUri {
    /// Repository identifier
    pub repo: String,
    /// File path relative to repo root
    pub path: String,
    /// Symbol kind (namespace, container, callable, value)
    pub kind: SymbolKind,
    /// Symbol name
    pub name: String,
    /// Line number where symbol is defined (1-indexed)
    pub line: u32,
}

impl SymbolUri {
    /// Create a new SymbolUri
    pub fn new(repo: impl Into<String>, path: impl Into<String>, kind: SymbolKind, name: impl Into<String>, line: u32) -> Self {
        Self {
            repo: repo.into(),
            path: path.into(),
            kind,
            name: name.into(),
            line,
        }
    }

    /// Parse a URI string into a SymbolUri
    ///
    /// Expected format: `codescope://<repo>/<path>#<kind>:<name>@<line>`
    pub fn parse(uri: &str) -> Result<Self> {
        // Remove the scheme prefix
        let uri = uri.strip_prefix("codescope://")
            .ok_or_else(|| Error::InvalidUri("URI must start with codescope://".to_string()))?;

        // Split on # to separate path from fragment
        let (repo_path, fragment) = uri.split_once('#')
            .ok_or_else(|| Error::InvalidUri("URI must contain # fragment".to_string()))?;

        // Split repo from path (first / separates repo from path)
        let (repo, path) = repo_path.split_once('/')
            .ok_or_else(|| Error::InvalidUri("URI must contain repo/path".to_string()))?;

        // Parse fragment: kind:name@line
        let (kind_name, line_str) = fragment.rsplit_once('@')
            .ok_or_else(|| Error::InvalidUri("Fragment must contain @line".to_string()))?;

        let (kind_str, name) = kind_name.split_once(':')
            .ok_or_else(|| Error::InvalidUri("Fragment must contain kind:name".to_string()))?;

        let kind = SymbolKind::from_str(kind_str)?;
        let line: u32 = line_str.parse()
            .map_err(|_| Error::InvalidUri(format!("Invalid line number: {}", line_str)))?;

        Ok(Self {
            repo: repo.to_string(),
            path: path.to_string(),
            kind,
            name: name.to_string(),
            line,
        })
    }

    /// Convert to URI string
    pub fn to_uri_string(&self) -> String {
        format!(
            "codescope://{}/{}#{}:{}@{}",
            self.repo, self.path, self.kind.as_str(), self.name, self.line
        )
    }
}

impl fmt::Display for SymbolUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uri_string())
    }
}

impl FromStr for SymbolUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}


impl Serialize for SymbolUri {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_uri_string())
    }
}

impl<'de> Deserialize<'de> for SymbolUri {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SymbolUri::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_roundtrip() {
        let uri = SymbolUri::new("myrepo", "src/auth.py", SymbolKind::Callable, "validate_token", 42);
        let uri_str = uri.to_uri_string();
        assert_eq!(uri_str, "codescope://myrepo/src/auth.py#callable:validate_token@42");
        
        let parsed = SymbolUri::parse(&uri_str).unwrap();
        assert_eq!(parsed, uri);
    }

    #[test]
    fn test_uri_parse() {
        let uri = SymbolUri::parse("codescope://repo/lib/db.ts#container:DatabaseClient@10").unwrap();
        assert_eq!(uri.repo, "repo");
        assert_eq!(uri.path, "lib/db.ts");
        assert_eq!(uri.kind, SymbolKind::Container);
        assert_eq!(uri.name, "DatabaseClient");
        assert_eq!(uri.line, 10);
    }

    #[test]
    fn test_invalid_uri() {
        assert!(SymbolUri::parse("invalid").is_err());
        assert!(SymbolUri::parse("http://example.com").is_err());
        assert!(SymbolUri::parse("codescope://repo/path").is_err()); // missing fragment
    }
}
