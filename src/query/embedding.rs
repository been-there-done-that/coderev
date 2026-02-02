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

    /// Generate embeddings for a batch of symbols
    pub fn embed_symbols(&self, symbols: &[Symbol]) -> Result<Vec<Vec<f32>>> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        // Prepare text inputs for embedding
        // We use a combination of name, signature, and start of content
        let inputs: Vec<String> = symbols.iter().map(|s| {
            let mut text = format!("Symbol: {}\nKind: {:?}\n", s.name, s.kind);
            if let Some(sig) = &s.signature {
                text.push_str(&format!("Signature: {}\n", sig));
            }
            if !s.content.is_empty() {
                // Use first 500 characters of content for context
                let content_preview = s.content.chars().take(500).collect::<String>();
                text.push_str(&format!("Context: {}\n", content_preview));
            }
            text
        }).collect();

        // Generate embeddings in batch
        let embeddings = self.model.embed(inputs, None)
            .map_err(|e| crate::Error::Adapter(format!("Embedding generation failed: {}", e)))?;
        
        Ok(embeddings)
    }

    /// Generate a single embedding for a query
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let mut embeddings = self.model.embed(vec![query.to_string()], None)
            .map_err(|e| crate::Error::Adapter(format!("Query embedding failed: {}", e)))?;
        
        Ok(embeddings.remove(0))
    }

    /// Generate embedding for a call site
    pub fn embed_call_site(&self, caller_name: &str, context: &str, imports: &[String]) -> Result<Vec<f32>> {
        let mut text = format!("Caller: {}\n", caller_name);
        if !context.is_empty() {
             text.push_str(&format!("Context: {}\n", context));
        }
        if !imports.is_empty() {
            text.push_str(&format!("Imports: {}\n", imports.join(", ")));
        }
        
        self.embed_query(&text)
    }
}

