use crate::Result;
use crate::symbol::Symbol;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

/// Engine for generating text embeddings using local transformer models
pub struct EmbeddingEngine {
    model: TextEmbedding,
}

impl EmbeddingEngine {
    /// Create a new embedding engine with the default model
    pub fn new() -> Result<Self> {
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::AllMiniLML6V2;
        options.show_download_progress = true;

        let model = TextEmbedding::try_new(options)
            .map_err(|e| crate::Error::Adapter(format!("Failed to load embedding model: {}", e)))?;
        
        Ok(Self { model })
    }

    /// Generate embeddings for a list of symbols.
    /// Returns a list of (symbol_index, vector) tuples.
    /// A single symbol might generate multiple vectors if it is large (Semantic Body Chunking).
    pub fn embed_symbols(&self, symbols: &[Symbol]) -> Result<Vec<(usize, Vec<f32>)>> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let mut inputs: Vec<String> = Vec::new();
        let mut mappings: Vec<usize> = Vec::new();

        for (i, s) in symbols.iter().enumerate() {
            // 1. Head Embedding (Name + Sig + Preview)
            // This captures the "intent" and high-level definition
            let mut text = format!("Symbol: {}\nKind: {:?}\n", s.name, s.kind);
            if let Some(sig) = &s.signature {
                text.push_str(&format!("Signature: {}\n", sig));
            }
            if !s.content.is_empty() {
                // Use first 1500 characters
                let content_preview = s.content.chars().take(1500).collect::<String>();
                text.push_str(&format!("Context: {}\n", content_preview));
            }
            inputs.push(text);
            mappings.push(i);

            // 2. Body Embeddings (Deep Content)
            // If content is larger than 1500 chars, chunk the rest
            if s.content.len() > 1500 {
                let full_content = &s.content;
                let chunk_size = 1000;
                let overlap = 100;
                
                // Start after the initial preview to avoid redundancy? 
                // Actually, redundancy is fine, but let's start at 1000 to overlap slightly with the head.
                let mut start = 1000; 

                while start < full_content.len() {
                    let end = std::cmp::min(start + chunk_size, full_content.len());
                    // Ensure we don't slice mid-char
                    let chunk_str = if let Some(s_idx) = full_content.char_indices().map(|(i, _)| i).nth(start) {
                         if let Some(e_idx) = full_content.char_indices().map(|(i, _)| i).nth(end) {
                             &full_content[s_idx..e_idx]
                         } else {
                             &full_content[s_idx..]
                         }
                    } else {
                         ""
                    };

                    if !chunk_str.trim().is_empty() {
                         let body_text = format!("Context from {}: {}\n", s.name, chunk_str);
                         inputs.push(body_text);
                         mappings.push(i);
                    }
                    
                    start += chunk_size - overlap;
                }
            }
        }

        // Generate embeddings in batch
        let embeddings = self.model.embed(inputs, None)
            .map_err(|e| crate::Error::Adapter(format!("Embedding generation failed: {}", e)))?;
        
        // Combine mapping with vectors
        let result = mappings.into_iter().zip(embeddings.into_iter()).collect();
        Ok(result)
    }

    /// Generate a single embedding for a query
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let mut embeddings = self.model.embed(vec![query.to_string()], None)
            .map_err(|e| crate::Error::Adapter(format!("Query embedding failed: {}", e)))?;
        
        Ok(embeddings.remove(0))
    }

    /// Generate embedding for a call site
    pub fn embed_call_site(&self, caller_name: &str, context: &str, imports: &[String]) -> Result<Vec<f32>> {
        let text = self.format_call_site(caller_name, context, imports);
        self.embed_query(&text)
    }

    /// Generate embeddings for a batch of call sites
    pub fn embed_call_sites(&self, batch: Vec<(String, String, Vec<String>)>) -> Result<Vec<Vec<f32>>> {
        if batch.is_empty() {
            return Ok(vec![]);
        }
        let inputs: Vec<String> = batch.into_iter()
            .map(|(name, ctx, imps)| self.format_call_site(&name, &ctx, &imps))
            .collect();
        
        let embeddings = self.model.embed(inputs, None)
            .map_err(|e| crate::Error::Adapter(format!("Batch call site embedding failed: {}", e)))?;
        
        Ok(embeddings)
    }

    fn format_call_site(&self, caller_name: &str, context: &str, imports: &[String]) -> String {
        let mut text = format!("Caller: {}\n", caller_name);
        if !context.is_empty() {
             text.push_str(&format!("Context: {}\n", context));
        }
        if !imports.is_empty() {
            text.push_str(&format!("Imports: {}\n", imports.join(", ")));
        }
        text
    }
}

