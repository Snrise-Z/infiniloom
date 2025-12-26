//! Dependency graph building and PageRank computation.
//!
//! Contains functions for building the symbol/file dependency graph
//! and computing importance scores via PageRank.

use super::types::{COMMON_KEYWORDS, IDENT_RE};
use crate::index::types::{DepGraph, Language, SymbolIndex};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Graph builder helper
pub(super) struct GraphBuilder<'a> {
    repo_root: &'a Path,
}

impl<'a> GraphBuilder<'a> {
    pub(super) fn new(repo_root: &'a Path) -> Self {
        Self { repo_root }
    }

    /// Build dependency graph from index
    pub(super) fn build_graph(
        &self,
        index: &SymbolIndex,
        file_path_to_id: &HashMap<String, u32>,
        symbol_calls: &[(u32, Vec<String>)],
        graph: &mut DepGraph,
    ) {
        let mut file_imports_by_file: HashMap<u32, Vec<u32>> = HashMap::new();

        // Resolve file imports and build edges
        for file in &index.files {
            for import in &file.imports {
                // Try to resolve the import to a file in the repository
                if let Some(resolved_id) =
                    self.resolve_import(&import.source, &file.path, file_path_to_id)
                {
                    graph.add_file_import(file.id.as_u32(), resolved_id);
                    file_imports_by_file
                        .entry(file.id.as_u32())
                        .or_default()
                        .push(resolved_id);
                }
            }
        }

        // Build symbol name to ID lookup for call resolution
        let mut symbol_name_to_ids: HashMap<&str, Vec<u32>> = HashMap::new();
        for sym in &index.symbols {
            symbol_name_to_ids
                .entry(&sym.name)
                .or_default()
                .push(sym.id.as_u32());
        }

        // Resolve function calls to symbol IDs and add call edges
        for (caller_id, call_names) in symbol_calls {
            let caller = &index.symbols[*caller_id as usize];
            let caller_file_id = caller.file_id;
            let imported_file_ids = file_imports_by_file
                .get(&caller_file_id.as_u32())
                .map(|ids| ids.iter().copied().collect::<HashSet<u32>>());

            for call_name in call_names {
                if let Some(callee_ids) = symbol_name_to_ids.get(call_name.as_str()) {
                    // Prefer symbols in the same file, then any match
                    let callee_id = callee_ids
                        .iter()
                        .find(|&&id| index.symbols[id as usize].file_id == caller_file_id)
                        .or_else(|| {
                            imported_file_ids.as_ref().and_then(|imports| {
                                callee_ids.iter().find(|&&id| {
                                    imports.contains(&index.symbols[id as usize].file_id.as_u32())
                                })
                            })
                        })
                        .or_else(|| callee_ids.first())
                        .copied();

                    if let Some(callee_id) = callee_id {
                        // Don't add self-calls
                        if callee_id != *caller_id {
                            graph.add_call(*caller_id, callee_id);
                        }
                    }
                }
            }
        }

        self.add_symbol_reference_edges(index, &file_imports_by_file, &symbol_name_to_ids, graph);
    }

    fn add_symbol_reference_edges(
        &self,
        index: &SymbolIndex,
        file_imports_by_file: &HashMap<u32, Vec<u32>>,
        symbol_name_to_ids: &HashMap<&str, Vec<u32>>,
        graph: &mut DepGraph,
    ) {
        let mut added: HashSet<(u32, u32)> = HashSet::new();

        for file in &index.files {
            let content = match fs::read_to_string(self.repo_root.join(&file.path)) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let imported_file_ids = file_imports_by_file
                .get(&file.id.as_u32())
                .map(|ids| ids.iter().copied().collect::<HashSet<u32>>());

            for (line_idx, line) in content.lines().enumerate() {
                if self.should_skip_reference_line(line, file.language) {
                    continue;
                }
                let line_no = line_idx as u32 + 1;
                let referencer = match index.find_symbol_at_line(file.id, line_no) {
                    Some(symbol) => symbol,
                    None => continue,
                };

                for mat in IDENT_RE.find_iter(line) {
                    let name = mat.as_str();
                    if name.len() <= 1 || self.is_reference_keyword(name) {
                        continue;
                    }
                    if referencer.span.start_line == line_no && referencer.name == name {
                        continue;
                    }

                    let target_id = symbol_name_to_ids.get(name).and_then(|candidate_ids| {
                        candidate_ids
                            .iter()
                            .find(|&&id| index.symbols[id as usize].file_id == file.id)
                            .or_else(|| {
                                imported_file_ids.as_ref().and_then(|imports| {
                                    candidate_ids.iter().find(|&&id| {
                                        imports
                                            .contains(&index.symbols[id as usize].file_id.as_u32())
                                    })
                                })
                            })
                            .or_else(|| candidate_ids.first())
                            .copied()
                    });

                    if let Some(target_id) = target_id {
                        if target_id != referencer.id.as_u32()
                            && added.insert((referencer.id.as_u32(), target_id))
                        {
                            graph.add_symbol_ref(referencer.id.as_u32(), target_id);
                        }
                    }
                }
            }
        }
    }

    fn should_skip_reference_line(&self, line: &str, language: Language) -> bool {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            return true;
        }

        let comment_prefixes: &[&str] = match language {
            Language::Python | Language::R => &["#"],
            Language::Bash => &["#"],
            Language::Ruby => &["#"],
            Language::Lua => &["--"],
            Language::JavaScript
            | Language::TypeScript
            | Language::C
            | Language::Cpp
            | Language::CSharp
            | Language::Go
            | Language::Java
            | Language::Php
            | Language::Kotlin
            | Language::Swift
            | Language::Scala => &["//"],
            _ => &["//"],
        };

        if comment_prefixes.iter().any(|p| trimmed.starts_with(p)) {
            return true;
        }

        let import_prefixes: &[&str] = match language {
            Language::Python => &["import ", "from "],
            Language::Rust => &["use "],
            Language::Go => &["import "],
            Language::Java => &["import "],
            Language::JavaScript | Language::TypeScript => &["import ", "export ", "require("],
            _ => &[],
        };

        import_prefixes.iter().any(|p| trimmed.starts_with(p))
    }

    fn is_reference_keyword(&self, name: &str) -> bool {
        COMMON_KEYWORDS.contains(name)
    }

    /// Resolve an import path to a file ID
    ///
    /// Handles both absolute and relative imports:
    /// - `./utils` resolves relative to the importing file's directory
    /// - `../shared` resolves to parent directory
    /// - `module` resolves using various strategies (src/, extensions, etc.)
    fn resolve_import(
        &self,
        source: &str,
        importing_file: &str,
        file_path_to_id: &HashMap<String, u32>,
    ) -> Option<u32> {
        // Handle relative imports (./foo, ../bar)
        if source.starts_with("./") || source.starts_with("../") {
            let import_dir = Path::new(importing_file).parent().unwrap_or(Path::new(""));

            // Strip leading ./ for resolution
            let relative_source = source.strip_prefix("./").unwrap_or(source);

            // Resolve the relative path
            let resolved = import_dir.join(relative_source);
            let resolved_str = resolved.to_string_lossy();
            let resolved_str = resolved_str.as_ref();

            // Try different extensions for the relative path
            let relative_candidates = [
                resolved_str.to_owned(),
                format!("{}.ts", resolved_str),
                format!("{}.js", resolved_str),
                format!("{}.tsx", resolved_str),
                format!("{}.jsx", resolved_str),
                format!("{}/index.ts", resolved_str),
                format!("{}/index.js", resolved_str),
                format!("{}.py", resolved_str),
                format!("{}/__init__.py", resolved_str),
            ];

            for candidate in relative_candidates {
                // Normalize path (remove ../ segments)
                let normalized = self.normalize_path(&candidate);
                if let Some(&id) = file_path_to_id.get(&normalized) {
                    return Some(id);
                }
            }
        }

        // Try absolute resolution strategies
        let candidates = [
            source.to_owned(),
            format!("{}.rs", source.replace("::", "/")),
            format!("{}/mod.rs", source.replace("::", "/")),
            format!("{}.py", source.replace(".", "/")),
            format!("{}/__init__.py", source.replace(".", "/")),
            format!("{}.ts", source),
            format!("{}.js", source),
            format!("{}/index.ts", source),
            format!("{}/index.js", source),
            format!("src/{}.rs", source.replace("::", "/")),
            format!("src/{}.py", source.replace(".", "/")),
            format!("src/{}.ts", source),
            format!("src/{}.js", source),
        ];

        for candidate in candidates {
            if let Some(&id) = file_path_to_id.get(&candidate) {
                return Some(id);
            }
        }

        None
    }

    /// Normalize a path by resolving . and .. segments
    fn normalize_path(&self, path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => continue,
                ".." => {
                    parts.pop();
                },
                _ => parts.push(part),
            }
        }
        parts.join("/")
    }

    /// Compute PageRank for files and symbols
    pub(super) fn compute_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        // Compute file-level PageRank
        self.compute_file_pagerank(index, graph);

        // Compute symbol-level PageRank
        self.compute_symbol_pagerank(index, graph);
    }

    /// Compute PageRank for files based on import graph
    fn compute_file_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        let n = index.files.len();
        if n == 0 {
            return;
        }

        let damping = 0.85f32;
        let iterations = 20;
        let initial_rank = 1.0 / n as f32;

        let mut ranks: Vec<f32> = vec![initial_rank; n];
        let mut new_ranks: Vec<f32> = vec![0.0; n];

        // Build adjacency for PageRank
        let mut outgoing: Vec<Vec<u32>> = vec![vec![]; n];
        for &(from, to) in &graph.file_imports {
            if (from as usize) < n && (to as usize) < n {
                outgoing[from as usize].push(to);
            }
        }

        for _ in 0..iterations {
            // Reset new ranks
            for r in &mut new_ranks {
                *r = (1.0 - damping) / n as f32;
            }

            // Distribute rank
            for (i, neighbors) in outgoing.iter().enumerate() {
                if !neighbors.is_empty() {
                    let contribution = damping * ranks[i] / neighbors.len() as f32;
                    for &j in neighbors {
                        new_ranks[j as usize] += contribution;
                    }
                } else {
                    // Dangling node: distribute to all
                    let contribution = damping * ranks[i] / n as f32;
                    for r in &mut new_ranks {
                        *r += contribution;
                    }
                }
            }

            std::mem::swap(&mut ranks, &mut new_ranks);
        }

        graph.file_pagerank = ranks;
    }

    /// Compute PageRank for symbols based on call graph
    fn compute_symbol_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        let n = index.symbols.len();
        if n == 0 {
            graph.symbol_pagerank = Vec::new();
            return;
        }

        let damping = 0.85f32;
        let iterations = 20;
        let initial_rank = 1.0 / n as f32;

        let mut ranks: Vec<f32> = vec![initial_rank; n];
        let mut new_ranks: Vec<f32> = vec![0.0; n];

        // Build adjacency for symbol PageRank using call graph
        // A symbol's importance is determined by how many other symbols call it
        let mut outgoing: Vec<Vec<u32>> = vec![vec![]; n];
        for &(caller, callee) in &graph.calls {
            if (caller as usize) < n && (callee as usize) < n {
                outgoing[caller as usize].push(callee);
            }
        }

        // Also consider symbol references
        for &(from, to) in &graph.symbol_refs {
            if (from as usize) < n && (to as usize) < n {
                // Avoid duplicate edges
                if !outgoing[from as usize].contains(&to) {
                    outgoing[from as usize].push(to);
                }
            }
        }

        for _ in 0..iterations {
            // Reset new ranks
            for r in &mut new_ranks {
                *r = (1.0 - damping) / n as f32;
            }

            // Distribute rank
            for (i, neighbors) in outgoing.iter().enumerate() {
                if !neighbors.is_empty() {
                    let contribution = damping * ranks[i] / neighbors.len() as f32;
                    for &j in neighbors {
                        new_ranks[j as usize] += contribution;
                    }
                } else {
                    // Dangling node: distribute to all (but with smaller contribution)
                    let contribution = damping * ranks[i] / n as f32;
                    for r in &mut new_ranks {
                        *r += contribution;
                    }
                }
            }

            std::mem::swap(&mut ranks, &mut new_ranks);
        }

        graph.symbol_pagerank = ranks;
    }
}
