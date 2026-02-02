use crate::Result;
use crate::storage::sqlite::{SqliteStore, PersistedUnresolvedReference};
use crate::query::embedding::EmbeddingEngine;
use crate::edge::{Edge, EdgeKind};
use crate::uri::SymbolUri;
use std::io::Write;

pub struct SemanticLinkerStats {
    pub resolved: usize,
    pub candidates: usize,
    pub total: usize,
}

pub struct SemanticLinker<'a> {
    store: &'a SqliteStore,
    embedding_engine: &'a EmbeddingEngine,
    threshold: f32,
}

impl<'a> SemanticLinker<'a> {
    pub fn new(store: &'a SqliteStore, embedding_engine: &'a EmbeddingEngine) -> Self {
        Self {
            store,
            embedding_engine,
            threshold: 0.6, // Default threshold lowered for practical recall
        }
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

        for (i, ref_item) in unresolved.into_iter().enumerate() {
            if i % 10 == 0 {
                print!("\r   Progress: {}/{} references", i + 1, total);
                std::io::stdout().flush().ok();
            }

            // Skip external references
            if ref_item.is_external {
                continue;
            }
            
            // Check if we already have a deterministic edge for this call
            // We can approximate this by checking if any 'Calls' edge exists from this source
            // But we don't know the exact target if it's unresolved.
            // However, Phase 2 Global Linker inserts edges. If an edge exists, we might want to skip.
            // But unresolved_references are NOT removed by Phase 2.
            // So we need to check if the reference is truly unresolved.
            // Phase 2 *should* have removed it from `unresolved_references` table if it resolved it?
            // Wait, looking at Global Linker:
            // It does NOT remove from unresolved_references. It just inserts edges.
            // And stores ambiguous references in ambiguous_references table.
            
            // To avoid duplicate work/edges, we should check if there are outgoing edges from this call site?
            // The call site is identified by (file, line, name).
            // `edges` table links `from_uri` -> `to_uri`.
            // `from_uri` is the caller (e.g. function).
            // If the caller calls multiple things, we have multiple edges.
            // We can't easily know which edge corresponds to which call site without more metadata.
            
            // However, the prompt says: "Phase 3 NEVER overwrites Phase 2."
            // Meaning if Phase 2 resolved it (confidence 1.0), we shouldn't degrade it.
            // But if Phase 2 failed (no edge, or ambiguous), we help.
            
            // How do we know if Phase 2 resolved it?
            // We can check if `ambiguous_references` has entries for this `ref_item.id`.
            // If so, it's ambiguous.
            // If not, and no edge exists? 
            
            // A simple heuristic:
            // If we find a high-confidence semantic match, we add it.
            // If a static edge already exists to the SAME target, we do nothing (or db handles unique constraint).
            // IF a static edge exists to a DIFFERENT target, we add ours as probabilistic.
            // If no edge exists, we add ours.

            // 1. Generate embedding for call site
            let context = self.get_context(&ref_item)?;
            let imports = self.store.get_imports_for_file(&ref_item.file_path)?;
            let import_strings: Vec<String> = imports.iter().map(|i| format!("{} as {}", i.target_namespace, i.alias.as_deref().unwrap_or("*"))).collect();
            
            let vector = self.embedding_engine.embed_call_site(
                &ref_item.name,
                &context,
                &import_strings
            )?;

            // Store callsite embedding
            self.store.insert_callsite_embedding(ref_item.id, &vector)?;

            // Search
            let results = self.store.search_by_vector(&vector, 5)?; // Top 5
            
            for (symbol, score) in results {
                if score >= self.threshold {
                    // Create probabilistic edge
                    let edge = Edge::with_confidence(
                        SymbolUri::parse(&ref_item.from_uri)?,
                        symbol.uri,
                        EdgeKind::Calls,
                        score
                    );
                    
                    // We rely on SQLite INSERT OR REPLACE (or IGNORE) to handle duplicates.
                    // But wait, our schema has UNIQUE(from, to, kind).
                    // If we try to insert a prob edge (0.8) but a static edge (1.0) exists for same (from, to),
                    // INSERT OR REPLACE will OVERWRITE 1.0 with 0.8 ! That is bad.
                    // We must NOT overwrite higher confidence edges.
                    
                    // Logic: Check if edge exists.
                    let edges = self.store.get_edges_from(&edge.from_uri)?;
                    let existing = edges.iter().find(|e| e.to_uri == edge.to_uri && e.kind == edge.kind);
                    
                    if let Some(existing_edge) = existing {
                        if existing_edge.confidence >= edge.confidence {
                            // Keep existing higher/equal confidence edge
                            continue;
                        }
                    }
                    
                    self.store.insert_edge(&edge)?;
                    resolved += 1;
                }
                candidates_count += 1;
            }
        }

        println!("\r   Progress: {}/{} references", total, total);
        Ok(SemanticLinkerStats { resolved, candidates: candidates_count, total })
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
