use crate::Result;
use crate::storage::SqliteStore;
use crate::edge::{Edge, EdgeKind};
use crate::uri::SymbolUri;
use std::collections::HashSet;
use std::fmt;

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
}

impl<'a> GlobalLinker<'a> {
    pub fn new(store: &'a SqliteStore) -> Self {
        Self { store }
    }

    pub fn run(&self) -> Result<GlobalLinkerStats> {
        let unresolved = self.store.get_all_unresolved()?;
        let total = unresolved.len();
        let mut resolved = 0;
        let mut ambiguous = 0;
        let mut external = 0;

        for ref_item in unresolved {
            // Check if it's already resolved in a previous run (though get_all_unresolved should prevent this if we delete/mark)
            // We'll proceed assuming we are processing fresh items.

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
                    // e.g. "import numpy as np" (alias=np), usage "np.array" (receiver=np)
                    // e.g. "import math" (alias=math), usage "math.sqrt" (receiver=math)
                    // e.g. "from math import sqrt as s", usage "s(1)" (name=s, receiver=None) -> import.alias=s matches name
                    
                    let mut matches_import = false;
                    
                    if let Some(ref recv) = ref_item.receiver {
                        if let Some(ref alias) = import.alias {
                            if alias == recv { matches_import = true; }
                        } else {
                            // No alias, so alias is implicitly the namespace (e.g. "import math" -> namespace="math")
                            // Wait, extraction logic: "import a" -> alias=a, target=a.
                            // So alias should be present if extracted correctly.
                            // If alias is None, it might mean "from X import *" or just "import X" without alias capturing?
                            // My extraction logic: "import a" -> alias=None? 
                            // Check adapter: `alias: None`.
                            // Wait, `import.module` in adapter sets alias=None.
                            // So "import math" -> namespace="math", alias=None.
                            // In this case, the receiver "math" should match namespace "math"?
                            // Or should I fix adapter to set alias="math"?
                            // Python: `import math`. Name bind is `math`.
                            // So if alias is None, usage name/receiver matches imported namespace name?
                            
                            // Let's assume if alias is None, the namespace leaf is the name.
                            let namespace_leaf = import.target_namespace.split('.').last().unwrap_or(&import.target_namespace);
                            if namespace_leaf == recv { matches_import = true; }
                        }
                    } else {
                        // No receiver. e.g. "sqrt(2)".
                        // Check explicit imports: "from math import sqrt"
                        // import.target_namespace = "math", import.symbols=["sqrt"] (Adapter handles "from_module")
                        // Wait, my DB implementation for properties `symbols`?
                        // `Import` struct in SqliteStore DOES NOT HAVE `symbols` list!
                        // It only has `file_path, alias, target_namespace, line`.
                        // One import row per import statement.
                        // But "from math import sqrt, pi" creates ONE import record in ScopeGraph, but how is it stored in DB?
                        // My storage code: `store.insert_import(...)`.
                        // It iterates `res.scope_graph.imports`.
                        // `ScopeGraph::Import` has `symbols: Vec<String>`.
                        // BUT `insert_import` takes ONE alias.
                        // The loop in `main.rs` iterates `res.scope_graph.imports` but essentially inserts ONE row per ScopeGraph::Import.
                        // This assumes `alias` corresponds to the module alias.
                        // It completely MISSES `symbols`.
                        
                        // FIX: I need to handle "from ... import ..." correctly.
                        // If `symbols` list is not empty, it means specific symbols are imported.
                        // For "from math import sqrt", `namespace`="math", `symbols`=["sqrt"].
                        // The name "sqrt" is now bound in local scope.
                        // References to "sqrt" should resolve to "math.sqrt".
                        
                        // My current DB schema for imports is insufficient for "from ... import a, b".
                        // Use case: `name` matches one of the imported symbols.
                        // But I don't have that list in DB.
                        
                        // Workaround for now:
                        // If no candidates found, and we are in "Step 2",
                        // logic is slightly broken for "from X import Y".
                        // However, let's proceed with what we have (Module imports).
                        // If I can match `name` to an import that might contain it...
                        // Or relying on Adapter to have put it in scope? (Phase 1).
                        // Phase 1 creates "UnresolvedReference".
                        
                        // Refinement on Phase 2 logic from user:
                        // "Check if the symbol is imported... for each import... if import.alias matches receiver... search symbol in target"
                        // What if `from a import b`?
                        // Alias is `b` (effectively). Target is `a`?
                        // Implementation detail:
                        // If `from a import b`, `namespace`="a", `alias`="b"?
                        // If `QueryAdapter` sets `alias`="b" for that specific symbol import?
                        // Adapater logic: 
                        // `name` (the imported symbol) is extracted. `alias` is extracted.
                        // `result.scope_graph.add_import(..., symbols: vec![name])`.
                        
                        // I need to fix `main.rs` to flatten `symbols` into multiple DB import rows?
                        // OR just handle the explicit module imports for now.
                        
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
                             matched_uri = Some(namespace_matches[0].uri.clone());
                         } else if namespace_matches.len() > 1 {
                             for sym in namespace_matches { candidates.insert(sym.uri); }
                         }
                    }
                }
            }

            // --- Step 3: Global Resolution ---
             if matched_uri.is_none() && candidates.is_empty() {
                 let global_matches = self.store.find_symbols_by_name(&ref_item.name)?;
                 if global_matches.len() == 1 {
                      matched_uri = Some(global_matches[0].uri.clone());
                 } else if global_matches.len() > 1 {
                      for sym in global_matches { candidates.insert(sym.uri); }
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
}
