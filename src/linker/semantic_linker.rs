use crate::Result;
use crate::storage::sqlite::{SqliteStore, PersistedUnresolvedReference};
use crate::query::embedding::EmbeddingEngine;
use crate::edge::{Edge, EdgeKind};
use crate::uri::SymbolUri;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticLinkerStats {
    pub resolved: usize,
    pub candidates: usize,
    pub total: usize,
}

use crate::ui::ProgressMessage;
use crate::ui::ProgressPhase;

pub struct SemanticLinker<'a> {
    store: &'a SqliteStore,
    embedding_engine: &'a EmbeddingEngine,
    threshold: f32,
    progress_tx: Option<crossbeam::channel::Sender<ProgressMessage>>,
}

impl<'a> SemanticLinker<'a> {
    pub fn new(store: &'a SqliteStore, embedding_engine: &'a EmbeddingEngine) -> Self {
        Self {
            store,
            embedding_engine,
            threshold: 0.6, // Default threshold lowered for practical recall
            progress_tx: None,
        }
    }

    pub fn with_progress(mut self, tx: crossbeam::channel::Sender<ProgressMessage>) -> Self {
        self.progress_tx = Some(tx);
        self
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn run(&self) -> Result<SemanticLinkerStats> {
        let unresolved = self.store.get_all_unresolved()?;
        let total = unresolved.len();
        let mut resolved = 0;
        let mut candidates_count = 0;

        // Filter out external and identify what needs embedding
        let targets: Vec<PersistedUnresolvedReference> = unresolved.into_iter()
            .filter(|r| !r.is_external)
            .collect();
        
        if targets.is_empty() {
            return Ok(SemanticLinkerStats { resolved: 0, candidates: 0, total });
        }

        // Phase 1: Cache ALL symbol embeddings in memory
        let symbol_embeddings = self.store.get_all_embeddings()?;
        let cached_symbols: Vec<(SymbolUri, Vec<f32>)> = symbol_embeddings.into_iter()
            .filter_map(|(uri_str, vec)| {
                SymbolUri::parse(&uri_str).ok().map(|uri| (uri, vec))
            })
            .collect();

        // Phase 2: Batch process targets
        let batch_size = 32;
        let mut processed = 0;
        let total_targets = targets.len();
        
        if let Some(ref tx) = self.progress_tx {
            tx.send(ProgressMessage::Started { phase: ProgressPhase::Semantic, total: total_targets }).ok();
        }

        for chunk in targets.chunks(batch_size) {
            // 1. Prepare batch for embedding
            let mut batch_data = Vec::with_capacity(chunk.len());
            for ref_item in chunk {
                let context = self.get_context(ref_item)?;
                let imports = self.store.get_imports_for_file(&ref_item.file_path)?;
                let import_strings: Vec<String> = imports.iter().map(|i| format!("{} as {}", i.target_namespace, i.alias.as_deref().unwrap_or("*"))).collect();
                batch_data.push((ref_item.name.clone(), context, import_strings));
            }

            // 2. Embed batch
            let vectors = self.embedding_engine.embed_call_sites(batch_data)?;

            // 3. Process each vector and find matches in cache
            self.store.begin_transaction()?;
            for (i, vector) in vectors.into_iter().enumerate() {
                let ref_item = &chunk[i];
                
                // Store call site embedding
                self.store.insert_callsite_embedding(ref_item.id, &vector)?;

                // 4. In-memory similarity search
                let mut matches = Vec::new();
                for (sym_uri, sym_vec) in &cached_symbols {
                    let score = self.cosine_similarity(&vector, sym_vec);
                    if score >= self.threshold {
                        matches.push((sym_uri, score));
                    }
                }

                // Sort and take top 5
                matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                
                let mut matched = false;
                for (sym_uri, score) in matches.into_iter().take(5) {
                    let edge = Edge::with_confidence(
                        SymbolUri::parse(&ref_item.from_uri)?,
                        sym_uri.clone(),
                        EdgeKind::Calls,
                        score
                    );

                    // Check if edge exists
                    let edges = self.store.get_edges_from(&edge.from_uri)?;
                    let existing = edges.iter().find(|e| e.to_uri == edge.to_uri && e.kind == edge.kind);
                    
                    if let Some(existing_edge) = existing {
                        if existing_edge.confidence >= edge.confidence {
                            continue;
                        }
                    }
                    
                    self.store.insert_edge(&edge)?;
                    matched = true;
                    resolved += 1;
                }
                
                if matched {
                    self.store.delete_unresolved(ref_item.id)?;
                } else {
                    // Mark as external so we don't retry forever
                    self.store.mark_unresolved_as_external(ref_item.id)?;
                }
                
                processed += 1;
                candidates_count += 1; // Used for "impact" or internal tracking, but 'processed' is better for UI
            }
            self.store.commit()?;
            
            if let Some(ref tx) = self.progress_tx {
                tx.send(ProgressMessage::Progress { 
                    phase: ProgressPhase::Semantic, 
                    current: processed, 
                    file: None 
                }).ok();
            }
        }

        Ok(SemanticLinkerStats { resolved, candidates: candidates_count, total })
    }
    
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    fn get_context(&self, ref_item: &PersistedUnresolvedReference) -> Result<String> {
        if let Ok(content) = std::fs::read_to_string(&ref_item.file_path) {
            let lines: Vec<&str> = content.lines().collect();
            let line_idx = (ref_item.line as usize).saturating_sub(1);
            if line_idx < lines.len() {
                // Take window of +/- 2 lines
                let start = line_idx.saturating_sub(2);
                let end = (line_idx + 3).min(lines.len());
                return Ok(lines[start..end].join("\n"));
            }
        }
        Ok(String::new())
    }
}
