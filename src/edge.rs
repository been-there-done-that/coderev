//! Edge types - Universal relationship representation
//!
//! All code relationships reduce to six edge types:
//! - `Defines`: namespace → symbol
//! - `Contains`: container → symbol
//! - `Calls`: callable → callable
//! - `References`: symbol → symbol
//! - `Inherits`: container → container
//! - `Exports`: namespace → symbol

use crate::uri::SymbolUri;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Universal edge kinds - all code relationships map to these types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Namespace defines a symbol (file → function, module → class)
    Defines,
    /// Container contains a symbol (class → method, struct → field)
    Contains,
    /// Callable calls another callable
    Calls,
    /// Symbol references another symbol (any usage)
    References,
    /// Container inherits from another container (class extends class)
    Inherits,
    /// Namespace exports a symbol (public API)
    Exports,
}

impl EdgeKind {
    /// Get the string representation of the edge kind
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Defines => "defines",
            EdgeKind::Contains => "contains",
            EdgeKind::Calls => "calls",
            EdgeKind::References => "references",
            EdgeKind::Inherits => "inherits",
            EdgeKind::Exports => "exports",
        }
    }

    /// Get all edge kinds
    pub fn all() -> &'static [EdgeKind] {
        &[
            EdgeKind::Defines,
            EdgeKind::Contains,
            EdgeKind::Calls,
            EdgeKind::References,
            EdgeKind::Inherits,
            EdgeKind::Exports,
        ]
    }

    /// Check if this edge kind implies a dependency relationship
    pub fn is_dependency(&self) -> bool {
        matches!(self, EdgeKind::Calls | EdgeKind::References | EdgeKind::Inherits)
    }

    /// Get the reverse edge kind for inverse graph traversal
    pub fn reverse(&self) -> EdgeKind {
        match self {
            EdgeKind::Defines => EdgeKind::Defines,  // No natural inverse
            EdgeKind::Contains => EdgeKind::Contains, // No natural inverse
            EdgeKind::Calls => EdgeKind::Calls,       // Caller/callee is symmetric in structure
            EdgeKind::References => EdgeKind::References,
            EdgeKind::Inherits => EdgeKind::Inherits,
            EdgeKind::Exports => EdgeKind::Exports,
        }
    }
}

impl FromStr for EdgeKind {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "defines" | "define" => Ok(EdgeKind::Defines),
            "contains" | "contain" => Ok(EdgeKind::Contains),
            "calls" | "call" => Ok(EdgeKind::Calls),
            "references" | "reference" | "ref" => Ok(EdgeKind::References),
            "inherits" | "inherit" | "extends" => Ok(EdgeKind::Inherits),
            "exports" | "export" => Ok(EdgeKind::Exports),
            _ => Err(crate::Error::InvalidUri(format!("Unknown edge kind: {}", s))),
        }
    }
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An edge in the code graph representing a relationship between symbols.
///
/// Edges can be:
/// - **Deterministic** (`confidence = 1.0`): Produced by static analysis
/// - **Probabilistic** (`confidence < 1.0`): Produced by semantic resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source symbol URI
    pub from_uri: SymbolUri,
    /// Target symbol URI
    pub to_uri: SymbolUri,
    /// Type of relationship
    pub kind: EdgeKind,
    /// Confidence score (1.0 = deterministic, <1.0 = probabilistic)
    pub confidence: f32,
}

impl Edge {
    /// Create a new deterministic edge (confidence = 1.0)
    pub fn new(from_uri: SymbolUri, to_uri: SymbolUri, kind: EdgeKind) -> Self {
        Self {
            from_uri,
            to_uri,
            kind,
            confidence: 1.0,
        }
    }

    /// Create a new probabilistic edge with a specific confidence
    pub fn with_confidence(from_uri: SymbolUri, to_uri: SymbolUri, kind: EdgeKind, confidence: f32) -> Self {
        Self {
            from_uri,
            to_uri,
            kind,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Check if this is a deterministic edge
    pub fn is_deterministic(&self) -> bool {
        (self.confidence - 1.0).abs() < f32::EPSILON
    }

    /// Check if this is a probabilistic edge
    pub fn is_probabilistic(&self) -> bool {
        !self.is_deterministic()
    }

    /// Create a reversed edge (swap from/to)
    pub fn reversed(&self) -> Self {
        Self {
            from_uri: self.to_uri.clone(),
            to_uri: self.from_uri.clone(),
            kind: self.kind,
            confidence: self.confidence,
        }
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.from_uri == other.from_uri 
            && self.to_uri == other.to_uri 
            && self.kind == other.kind
    }
}

impl Eq for Edge {}

impl std::hash::Hash for Edge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.from_uri.hash(state);
        self.to_uri.hash(state);
        self.kind.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolKind;

    fn sample_uri(name: &str, line: u32) -> SymbolUri {
        SymbolUri::new("repo", "src/main.py", SymbolKind::Callable, name, line)
    }

    #[test]
    fn test_edge_kind_roundtrip() {
        for kind in EdgeKind::all() {
            let s = kind.as_str();
            let parsed: EdgeKind = s.parse().unwrap();
            assert_eq!(*kind, parsed);
        }
    }

    #[test]
    fn test_deterministic_edge() {
        let from = sample_uri("caller", 10);
        let to = sample_uri("callee", 20);
        let edge = Edge::new(from, to, EdgeKind::Calls);
        
        assert!(edge.is_deterministic());
        assert!(!edge.is_probabilistic());
        assert_eq!(edge.confidence, 1.0);
    }

    #[test]
    fn test_probabilistic_edge() {
        let from = sample_uri("user", 10);
        let to = sample_uri("maybe_target", 20);
        let edge = Edge::with_confidence(from, to, EdgeKind::References, 0.75);
        
        assert!(!edge.is_deterministic());
        assert!(edge.is_probabilistic());
        assert_eq!(edge.confidence, 0.75);
    }

    #[test]
    fn test_edge_reversed() {
        let from = sample_uri("a", 10);
        let to = sample_uri("b", 20);
        let edge = Edge::new(from.clone(), to.clone(), EdgeKind::Calls);
        let reversed = edge.reversed();

        assert_eq!(reversed.from_uri, to);
        assert_eq!(reversed.to_uri, from);
    }
}
