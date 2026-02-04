use crate::Result;
use crate::storage::SqliteStore;
use crate::edge::{Edge, EdgeKind};
use crate::uri::SymbolUri;
use std::collections::HashSet;
use std::fmt;
use crate::ui::{ProgressMessage, ProgressPhase};

#[derive(Debug, Clone, serde::Serialize)]
pub struct GlobalLinkerStats {
    pub resolved: usize,
    pub ambiguous: usize,
    pub external: usize,
    pub total: usize,
}

impl fmt::Display for GlobalLinkerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Global Linker Stats:")?;
        writeln!(f, "  Total References: {}", self.total)?;
        writeln!(f, "  ✅ Resolved: {}", self.resolved)?;
        writeln!(f, "  🤔 Ambiguous: {}", self.ambiguous)?;
        writeln!(f, "  🌍 External: {}", self.external)
    }
}

pub struct GlobalLinker<'a> {
    store: &'a SqliteStore,
    progress_tx: Option<crossbeam::channel::Sender<ProgressMessage>>,
}

impl<'a> GlobalLinker<'a> {
    pub fn new(store: &'a SqliteStore) -> Self {
        Self { 
            store,
            progress_tx: None,
        }
    }

    pub fn with_progress(mut self, tx: crossbeam::channel::Sender<ProgressMessage>) -> Self {
        self.progress_tx = Some(tx);
        self
    }

    pub fn run(&self) -> Result<GlobalLinkerStats> {
        let unresolved = self.store.get_all_unresolved()?;
        let total = unresolved.len();
        if let Some(ref tx) = self.progress_tx {
            tx.send(ProgressMessage::Started { phase: ProgressPhase::Linking, total }).ok();
        }
        self.resolve_references(unresolved)
    }

    pub fn resolve_file(&self, file_path: &str) -> Result<GlobalLinkerStats> {
        let unresolved = self.store.get_unresolved_in_file(file_path)?;
        self.resolve_references(unresolved)
    }

    fn resolve_references(&self, unresolved: Vec<crate::storage::PersistedUnresolvedReference>) -> Result<GlobalLinkerStats> {
        let total = unresolved.len();
        let mut resolved = 0;
        let mut ambiguous = 0;
        let mut external = 0;

        let mut i = 0;
        for ref_item in unresolved {
            i += 1;
            if i % 100 == 0 {
                if let Some(ref tx) = self.progress_tx {
                    tx.send(ProgressMessage::Progress {
                        phase: ProgressPhase::Linking,
                        current: i,
                        file: None,
                    }).ok();
                }
            }

            let mut candidates = HashSet::new(); 
            let mut matched_uri = None;
            
            // --- Step 1: Local Resolution ---
            // "Look for symbols in the same file"
            let local_matches = self.store.find_symbols_by_name_and_file(&ref_item.name, &ref_item.file_path)?;
            
            if local_matches.len() == 1 {
                matched_uri = Some(local_matches[0].uri.clone());
            } else if local_matches.len() > 1 {
                for sym in local_matches { candidates.insert(sym.uri); }
            }

            // --- Step 2: Import Resolution ---
            if matched_uri.is_none() && candidates.is_empty() {
                let imports = self.store.get_imports_for_file(&ref_item.file_path)?;
                
                for import in imports {
                    // Case A: Import has an alias that matches the receiver or the name(if no receiver)
                    match self.try_resolve_import(&ref_item, &import)? {
                        Some(uri) => { matched_uri = Some(uri); break; },
                        None => { /* continue checking other imports */ }
                    }
                }
            }

            // --- Step 3: Global Resolution ---
            if matched_uri.is_none() && candidates.is_empty() {
                 let global_matches = self.store.find_symbols_by_name(&ref_item.name)?;
                 
                 // If we have a receiver (e.g., Type in Type::method), try to filter global matches by that parent
                 if let Some(ref recv) = ref_item.receiver {
                     // Try to resolve the receiver globally first
                     let receiver_matches = self.store.find_symbols_by_name(recv)?;
                     
                     // If we found the receiver (e.g. the struct "Type"), check if any of our candidate methods belong to it
                     let mut filtered_candidates = Vec::new();
                     for method_sym in &global_matches {
                         // We need to check if method_sym is a child of any receiver_match
                         let incoming = self.store.get_edges_to(&method_sym.uri)?;
                         let incoming_contains = incoming.iter().filter(|e| e.kind == EdgeKind::Contains);
                         for edge in incoming_contains {
                             for recv_sym in &receiver_matches {
                                 if edge.from_uri == recv_sym.uri {
                                     filtered_candidates.push(method_sym.clone());
                                     break;
                                 }
                             }
                         }
                     }
                     
                     if filtered_candidates.len() == 1 {
                         matched_uri = Some(filtered_candidates[0].uri.clone());
                         // Clear other candidates as we found a specific one
                         candidates.clear();
                     } else if !filtered_candidates.is_empty() {
                         // We narrowed it down, but still ambiguous (e.g. multiple "Type" structs?)
                         for sym in filtered_candidates { candidates.insert(sym.uri); }
                     } else {
                         // Receiver filtering didn't help (maybe receiver not found or no relation), fallback to raw name matches
                         if global_matches.len() == 1 {
                              matched_uri = Some(global_matches[0].uri.clone());
                         } else if global_matches.len() > 1 {
                              for sym in global_matches { candidates.insert(sym.uri); }
                         }
                     }
                 } else {
                     // No receiver, standard name match
                     if global_matches.len() == 1 {
                          matched_uri = Some(global_matches[0].uri.clone());
                     } else if global_matches.len() > 1 {
                          for sym in global_matches { candidates.insert(sym.uri); }
                     }
                 }
             }

             // --- Final Decision ---
             if let Some(uri) = matched_uri {
                 let from_res = SymbolUri::parse(&ref_item.from_uri);
                 if let Ok(from) = from_res {
                     let kind = if ref_item.ref_kind == "inherits" { EdgeKind::Inherits } else { EdgeKind::Calls };
                     let edge = Edge::with_confidence(from, uri, kind, 1.0);
                     self.store.insert_edge(&edge)?;
                     self.store.delete_unresolved(ref_item.id)?; 
                     resolved += 1;
                 }
             } else if !candidates.is_empty() {
                 ambiguous += 1;
                 for uri in candidates {
                     self.store.insert_ambiguous_reference(ref_item.id, &uri.to_uri_string(), 0.0)?;
                 }
             } else {
                 external += 1;
                 // Mark as external so Semantic Resolver skips it
                 if let Err(e) = self.store.mark_unresolved_as_external(ref_item.id) {
                     tracing::warn!("Failed to mark reference {} as external: {}", ref_item.id, e);
                 }
             }
        }

        Ok(GlobalLinkerStats { resolved, ambiguous, external, total })
    }

    fn try_resolve_import(&self, ref_item: &crate::storage::PersistedUnresolvedReference, import: &crate::storage::Import) -> Result<Option<SymbolUri>> {
        let mut matches_import = false;
        
        if let Some(ref recv) = ref_item.receiver {
            if let Some(ref alias) = import.alias {
                if alias == recv { matches_import = true; }
            } else {
                let namespace_leaf = import.target_namespace.split('.').last().unwrap_or(&import.target_namespace);
                if namespace_leaf == recv { matches_import = true; }
            }
        } else {
            // No receiver. e.g. "sqrt(2)".
            // Let's implement module-alias matching first.
            if let Some(ref alias) = import.alias {
                if alias == &ref_item.name { matches_import = true; }
            }
        }

        if matches_import {
            let target = &import.target_namespace;
            // Search for symbol in that namespace
             let namespace_matches = self.store.find_symbols_by_name_and_container_pattern(&ref_item.name, target)?;
             
             if namespace_matches.len() == 1 {
                 return Ok(Some(namespace_matches[0].uri.clone()));
             }
        }
        Ok(None)
    }
}
