//! Document Chunker - Fallback coverage for non-AST files
//!
//! For files without a language adapter (SQL, YAML, Markdown, Terraform, Bash, etc.),
//! we chunk the content and store as document symbols. This provides Coderev-style
//! coverage while the AST adapters provide compiler-level precision.
//!
//! Chunking strategy:
//! - Split into ~500 token chunks with overlap
//! - Preserve logical boundaries (paragraphs, sections)
//! - Store metadata about chunk position

use crate::Result;
use crate::symbol::{Symbol, SymbolKind};
use super::framework::AdapterResult;

/// Default chunk size in characters (roughly ~500 tokens? No, 256 tokens is ~1000 chars)
/// We use 1000 to match the embedding model's effective window.
const DEFAULT_CHUNK_SIZE: usize = 1000;

/// Overlap between chunks to preserve context
const DEFAULT_OVERLAP: usize = 100;

/// Minimum chunk size to avoid tiny fragments
const MIN_CHUNK_SIZE: usize = 100;

/// Document chunker for non-AST files
pub struct DocumentChunker {
    chunk_size: usize,
    overlap: usize,
}

impl Default for DocumentChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentChunker {
    /// Create a new document chunker with default settings
    pub fn new() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            overlap: DEFAULT_OVERLAP,
        }
    }

    /// Create a chunker with custom settings
    pub fn with_settings(chunk_size: usize, overlap: usize) -> Self {
        Self {
            chunk_size: chunk_size.max(MIN_CHUNK_SIZE),
            overlap: overlap.min(chunk_size / 2),
        }
    }

    /// Chunk a document into symbols
    pub fn chunk_file(&self, repo: &str, path: &str, content: &str) -> Result<AdapterResult> {
        let mut result = AdapterResult::new();
        
        // Skip empty files
        if content.trim().is_empty() {
            return Ok(result);
        }

        let chunks = self.split_into_chunks(content);
        let total_chunks = chunks.len();
        
        for (idx, chunk) in chunks.into_iter().enumerate() {
            let chunk_name = if total_chunks == 1 {
                // Single chunk = just use filename
                path.rsplit('/').next().unwrap_or(path).to_string()
            } else {
                // Multiple chunks = filename#chunk_N
                format!("{}#chunk_{}", 
                    path.rsplit('/').next().unwrap_or(path),
                    idx + 1)
            };

            let symbol = Symbol::new(
                repo,
                path,
                SymbolKind::Document,
                &chunk_name,
                chunk.start_line,
                chunk.end_line,
                &chunk.content,
            );
            
            result.add_symbol(symbol);
        }

        Ok(result)
    }

    /// Split content into chunks preserving logical boundaries
    fn split_into_chunks(&self, content: &str) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.is_empty() {
            return chunks;
        }

        let total_len = content.len();
        
        // For small files, return as single chunk
        if total_len <= self.chunk_size {
            return vec![Chunk {
                content: content.to_string(),
                start_line: 1,
                end_line: lines.len() as u32,
            }];
        }

        // Break into chunks at natural boundaries
        let mut current_chunk = String::new();
        let mut chunk_start_line: u32 = 1;
        let mut current_line: u32 = 0;

        for (idx, line) in lines.iter().enumerate() {
            current_line = (idx + 1) as u32;
            
            // Add line to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);

            // Check if we should break here
            if current_chunk.len() >= self.chunk_size {
                // Look for a natural break point
                let break_at = self.find_break_point(&current_chunk);
                
                if break_at < current_chunk.len() && break_at > MIN_CHUNK_SIZE {
                    // Split at break point
                    let chunk_content = current_chunk[..break_at].to_string();
                    let chunk_lines = chunk_content.lines().count() as u32;
                    
                    chunks.push(Chunk {
                        content: chunk_content.clone(),
                        start_line: chunk_start_line,
                        end_line: chunk_start_line + chunk_lines - 1,
                    });

                    // Start new chunk with overlap
                    let raw_overlap_start = if break_at > self.overlap {
                        break_at - self.overlap
                    } else {
                        0
                    };
                    
                    // Align to char boundary
                    let mut overlap_start = raw_overlap_start;
                    while !current_chunk.is_char_boundary(overlap_start) && overlap_start > 0 {
                        overlap_start -= 1;
                    }
                    
                    current_chunk = current_chunk[overlap_start..].to_string();
                    // Calculate start line of the new overlapping chunk based on the current line we are at
                    chunk_start_line = current_line.saturating_sub(current_chunk.lines().count() as u32).saturating_add(1);
                } else {
                    // No good break point, force split
                    chunks.push(Chunk {
                        content: current_chunk.clone(),
                        start_line: chunk_start_line,
                        end_line: current_line,
                    });
                    
                    current_chunk.clear();
                    chunk_start_line = current_line + 1;
                }
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() && current_chunk.len() >= MIN_CHUNK_SIZE {
            chunks.push(Chunk {
                content: current_chunk,
                start_line: chunk_start_line,
                end_line: current_line,
            });
        }

        chunks
    }

    /// Find a natural break point in the content
    fn find_break_point(&self, content: &str) -> usize {
        // We want to break roughly at chunk_size
        let target_len = self.chunk_size;
        
        // Define search window around the target length (chunk_size +/- 500)
        let mut start_scan = if target_len > 500 { target_len - 500 } else { 0 };
        // Align start_scan
        while !content.is_char_boundary(start_scan) && start_scan > 0 {
             start_scan -= 1;
        }

        let mut end_scan = (target_len + 500).min(content.len());
        // Align end_scan
        while !content.is_char_boundary(end_scan) && end_scan < content.len() {
             end_scan += 1;
        }
        
        // If content is smaller than where we start scanning, just return content length
        if start_scan >= content.len() {
            return content.len();
        }
        
        let search_area = &content[start_scan..end_scan];

        // Priority 1: double newline (paragraph break)
        if let Some(pos) = search_area.rfind("\n\n") {
            return start_scan + pos + 2;
        }
        
        // Priority 2: single newline
        if let Some(pos) = search_area.rfind('\n') {
            return start_scan + pos + 1;
        }
        
        // Priority 3: space (word boundary) - helpful for minified files without newlines
        if let Some(pos) = search_area.rfind(' ') {
            return start_scan + pos + 1;
        }

        // Fallback: hard cut at chunk_size (or content length if smaller)
        // Ensure fallback is also on char boundary
        let mut fallback = target_len.min(content.len());
        while !content.is_char_boundary(fallback) && fallback > 0 {
             fallback -= 1;
        }
        fallback
    }

    /// Get the file extensions this chunker handles
    pub fn supported_extensions() -> &'static [&'static str] {
        &[
            // Config files
            "yaml", "yml", "json", "toml", "ini", "cfg", "conf",
            // SQL
            "sql",
            // Shell/Scripts
            "sh", "bash", "zsh", "fish",
            // Documentation
            "md", "rst", "txt", "adoc",
            // Infrastructure
            "tf", "hcl", "dockerfile", "containerfile",
            // Data
            "csv", "xml",
            // Other
            "env", "properties", "gradle",
        ]
    }

    /// Check if a file extension is supported by the chunker
    pub fn supports_extension(ext: &str) -> bool {
        Self::supported_extensions().contains(&ext.to_lowercase().as_str())
    }
}

/// A chunk of a document
#[derive(Debug, Clone)]
struct Chunk {
    content: String,
    start_line: u32,
    end_line: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_file_single_chunk() {
        let chunker = DocumentChunker::new();
        let content = "SELECT * FROM users;\nSELECT * FROM orders;";
        
        let result = chunker.chunk_file("repo", "query.sql", content).unwrap();
        
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].kind, SymbolKind::Document);
        assert_eq!(result.symbols[0].name, "query.sql");
    }

    #[test]
    fn test_large_file_multiple_chunks() {
        let chunker = DocumentChunker::with_settings(100, 20);
        let content = (0..50).map(|i| format!("Line {} with some content here\n", i)).collect::<String>();
        
        let result = chunker.chunk_file("repo", "large.sql", &content).unwrap();
        
        assert!(result.symbols.len() > 1);
        for sym in &result.symbols {
            assert_eq!(sym.kind, SymbolKind::Document);
        }
    }

    #[test]
    fn test_empty_file() {
        let chunker = DocumentChunker::new();
        let result = chunker.chunk_file("repo", "empty.sql", "").unwrap();
        
        assert_eq!(result.symbols.len(), 0);
    }

    #[test]
    fn test_supported_extensions() {
        assert!(DocumentChunker::supports_extension("sql"));
        assert!(DocumentChunker::supports_extension("yaml"));
        assert!(DocumentChunker::supports_extension("md"));
        assert!(DocumentChunker::supports_extension("tf"));
        assert!(!DocumentChunker::supports_extension("py")); // Handled by adapter
        assert!(!DocumentChunker::supports_extension("rs")); // Handled by adapter
    }
}
