//! Chunking strategy implementations
//!
//! Different algorithms for splitting repositories into chunks.

use super::types::{Chunk, Chunker, SymbolSnippet};
use crate::tokenizer::Tokenizer;
use crate::types::{RepoFile, Repository, SymbolKind};
use std::collections::{HashMap, HashSet, VecDeque};

impl Chunker {
    /// Fixed-size chunking
    pub(crate) fn fixed_chunk(&self, repo: &Repository, size: u32) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_files: Vec<&RepoFile> = Vec::new();
        let mut current_tokens = 0u32;

        for file in &repo.files {
            let file_tokens = file.token_count.get(self.model);

            if current_tokens + file_tokens > size && !current_files.is_empty() {
                chunks.push(self.create_chunk_from_refs(
                    chunks.len(),
                    &current_files,
                    current_tokens,
                ));
                current_files.clear();
                current_tokens = 0;
            }

            current_files.push(file);
            current_tokens += file_tokens;
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk_from_refs(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks, repo)
    }

    /// One file per chunk
    pub(crate) fn file_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        let chunks: Vec<_> = repo
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                self.create_chunk(i, std::slice::from_ref(file), file.token_count.get(self.model))
            })
            .collect();

        self.finalize_chunks(chunks, repo)
    }

    /// Group by module/directory, respecting max_tokens limit
    pub(crate) fn module_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        let mut modules: HashMap<String, Vec<RepoFile>> = HashMap::new();

        for file in &repo.files {
            let module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root")
                .to_owned();

            modules.entry(module).or_default().push(file.clone());
        }

        // Sort modules for consistent ordering
        let mut sorted_modules: Vec<_> = modules.into_iter().collect();
        sorted_modules.sort_by(|a, b| a.0.cmp(&b.0));

        let mut chunks = Vec::new();

        for (_module_name, mut files) in sorted_modules {
            // Sort files within module by path
            files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

            let module_tokens: u32 = files.iter().map(|f| f.token_count.get(self.model)).sum();

            if module_tokens <= self.max_tokens {
                // Module fits in one chunk
                chunks.push(self.create_chunk(chunks.len(), &files, module_tokens));
            } else {
                // Module exceeds max_tokens - split it into multiple chunks
                let mut current_files = Vec::new();
                let mut current_tokens = 0u32;

                for file in files {
                    let file_tokens = file.token_count.get(self.model);

                    // If adding this file would exceed limit and we have files, create chunk
                    if current_tokens + file_tokens > self.max_tokens && !current_files.is_empty() {
                        chunks.push(self.create_chunk(
                            chunks.len(),
                            &current_files,
                            current_tokens,
                        ));
                        current_files = Vec::new();
                        current_tokens = 0;
                    }

                    // Add file to current chunk (even if it alone exceeds max_tokens)
                    current_files.push(file);
                    current_tokens += file_tokens;
                }

                // Don't forget remaining files
                if !current_files.is_empty() {
                    chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
                }
            }
        }

        self.finalize_chunks(chunks, repo)
    }

    /// Symbol-based chunking - groups by key symbols with small context
    pub(crate) fn symbol_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        const CONTEXT_LINES: u32 = 2;
        let tokenizer = Tokenizer::new();
        let mut snippets: Vec<SymbolSnippet> = Vec::new();

        for file in &repo.files {
            let content = match &file.content {
                Some(content) => content,
                None => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len() as u32;
            if total_lines == 0 {
                continue;
            }

            for symbol in &file.symbols {
                if symbol.kind == SymbolKind::Import {
                    continue;
                }

                let snippet_content = if symbol.start_line > 0
                    && symbol.end_line >= symbol.start_line
                    && symbol.start_line <= total_lines
                {
                    let start = symbol.start_line.saturating_sub(CONTEXT_LINES).max(1);
                    let end = symbol
                        .end_line
                        .max(symbol.start_line)
                        .saturating_add(CONTEXT_LINES)
                        .min(total_lines);
                    let start_idx = start.saturating_sub(1) as usize;
                    let end_idx = end.saturating_sub(1) as usize;
                    if start_idx > end_idx || end_idx >= lines.len() {
                        continue;
                    }

                    let mut snippet = String::new();
                    snippet.push_str(&format!(
                        "// {}: {} (lines {}-{})\n",
                        symbol.kind.name(),
                        symbol.name,
                        start,
                        end
                    ));
                    snippet.push_str(&lines[start_idx..=end_idx].join("\n"));
                    snippet
                } else if let Some(ref sig) = symbol.signature {
                    format!("// {}: {}\n{}", symbol.kind.name(), symbol.name, sig.trim())
                } else {
                    continue;
                };

                let tokens = tokenizer.count(&snippet_content, self.model);
                let importance = (symbol.importance * 0.7) + (file.importance * 0.3);

                snippets.push(SymbolSnippet {
                    file_path: file.relative_path.clone(),
                    symbol_name: symbol.name.clone(),
                    start_line: symbol.start_line,
                    content: snippet_content,
                    tokens,
                    importance,
                });
            }
        }

        if snippets.is_empty() {
            return self.semantic_chunk(repo);
        }

        snippets.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.tokens.cmp(&b.tokens))
                .then_with(|| a.file_path.cmp(&b.file_path))
        });

        let mut chunks: Vec<Chunk> = Vec::new();
        let mut current: Vec<SymbolSnippet> = Vec::new();
        let mut current_tokens = 0u32;

        for snippet in snippets {
            if current_tokens + snippet.tokens > self.max_tokens && !current.is_empty() {
                chunks.push(self.build_symbol_chunk(chunks.len(), &current, &tokenizer));
                current.clear();
                current_tokens = 0;
            }

            current_tokens += snippet.tokens;
            current.push(snippet);
        }

        if !current.is_empty() {
            chunks.push(self.build_symbol_chunk(chunks.len(), &current, &tokenizer));
        }

        self.finalize_chunks(chunks, repo)
    }

    /// Semantic chunking (group related files)
    pub(crate) fn semantic_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_files = Vec::new();
        let mut current_tokens = 0u32;
        let mut current_module: Option<String> = None;

        // Sort files by path for better grouping
        let mut sorted_files: Vec<_> = repo.files.iter().collect();
        sorted_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        for file in sorted_files {
            let file_tokens = file.token_count.get(self.model);
            let file_module = file.relative_path.split('/').next().map(String::from);

            // Check if we should start a new chunk
            let should_split = current_tokens + file_tokens > self.max_tokens
                || (current_module.is_some()
                    && file_module.is_some()
                    && current_module != file_module
                    && current_tokens > self.max_tokens / 2);

            if should_split && !current_files.is_empty() {
                chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));

                // Keep some overlap for context
                current_files = self.get_overlap_files(&current_files);
                current_tokens = current_files
                    .iter()
                    .map(|f| f.token_count.get(self.model))
                    .sum();
            }

            current_files.push(file.clone());
            current_tokens += file_tokens;
            current_module = file_module;
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks, repo)
    }

    /// Dependency-based chunking - groups files by their import dependencies
    /// Files are ordered so that dependencies appear before dependents
    pub(crate) fn dependency_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        // Build a map of file path to index
        let file_indices: HashMap<&str, usize> = repo
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.relative_path.as_str(), i))
            .collect();

        // Build dependency graph: file_idx -> set of dependent file indices
        // Also track reverse: file_idx -> set of files it imports from
        let mut imports_from: Vec<HashSet<usize>> = vec![HashSet::new(); repo.files.len()];
        let mut imported_by: Vec<HashSet<usize>> = vec![HashSet::new(); repo.files.len()];

        for (idx, file) in repo.files.iter().enumerate() {
            // Look at symbols to find imports
            for symbol in &file.symbols {
                if symbol.kind == SymbolKind::Import {
                    // Try to resolve the import to a file in the repo
                    let import_name = &symbol.name;

                    // Check various path patterns
                    let potential_paths = Self::resolve_import_paths(import_name, file);

                    for potential in potential_paths {
                        if let Some(&target_idx) = file_indices.get(potential.as_str()) {
                            if target_idx != idx {
                                imports_from[idx].insert(target_idx);
                                imported_by[target_idx].insert(idx);
                            }
                        }
                    }
                }
            }
        }

        // Topological sort using Kahn's algorithm
        let mut in_degree: Vec<usize> = imports_from.iter().map(|deps| deps.len()).collect();
        let mut queue: VecDeque<usize> = in_degree
            .iter()
            .enumerate()
            .filter_map(|(i, &d)| if d == 0 { Some(i) } else { None })
            .collect();

        let mut sorted_indices: Vec<usize> = Vec::with_capacity(repo.files.len());
        let mut sorted_set: HashSet<usize> = HashSet::with_capacity(repo.files.len());

        while let Some(idx) = queue.pop_front() {
            sorted_indices.push(idx);
            sorted_set.insert(idx);
            for &dependent in &imported_by[idx] {
                in_degree[dependent] -= 1;
                if in_degree[dependent] == 0 {
                    queue.push_back(dependent);
                }
            }
        }

        // Handle any cycles by adding remaining files (files in cycles)
        // Using HashSet for O(1) lookups instead of O(n) Vec::contains
        if sorted_indices.len() < repo.files.len() {
            for idx in 0..repo.files.len() {
                if !sorted_set.contains(&idx) {
                    sorted_indices.push(idx);
                }
            }
        }

        // Now chunk the sorted files, trying to keep related files together
        let mut chunks = Vec::new();
        let mut current_files: Vec<&RepoFile> = Vec::new();
        let mut current_tokens = 0u32;
        let mut current_deps: HashSet<usize> = HashSet::new();

        for &idx in &sorted_indices {
            let file = &repo.files[idx];
            let file_tokens = file.token_count.get(self.model);

            // Check if this file depends on files in the current chunk
            let depends_on_current = imports_from[idx].iter().any(|d| current_deps.contains(d));

            // Should we start a new chunk?
            let should_split = current_tokens + file_tokens > self.max_tokens
                && !current_files.is_empty()
                && !depends_on_current; // Try to keep dependent files together

            if should_split {
                chunks.push(self.create_chunk_from_refs(
                    chunks.len(),
                    &current_files,
                    current_tokens,
                ));
                current_files.clear();
                current_tokens = 0;
                current_deps.clear();
            }

            current_files.push(file);
            current_tokens += file_tokens;
            current_deps.insert(idx);
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk_from_refs(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks, repo)
    }

    /// Resolve an import name to potential file paths
    fn resolve_import_paths(import_name: &str, source_file: &RepoFile) -> Vec<String> {
        let mut paths = Vec::new();
        let source_dir = source_file
            .relative_path
            .rsplit_once('/')
            .map(|(d, _)| d)
            .unwrap_or("");

        // Convert import to potential paths (handles various languages)
        let normalized = import_name.replace("::", "/").replace(['.', '\\'], "/");

        // Try with common extensions
        let extensions = ["py", "js", "ts", "tsx", "jsx", "rs", "go", "java", "rb"];
        for ext in extensions {
            // Absolute import
            paths.push(format!("{}.{}", normalized, ext));
            paths.push(format!("{}/index.{}", normalized, ext));
            paths.push(format!("{}/mod.{}", normalized, ext));

            // Relative to source file
            if !source_dir.is_empty() {
                paths.push(format!("{}/{}.{}", source_dir, normalized, ext));
            }
        }

        // Also try the exact path if it looks like a file
        if import_name.contains('/') || import_name.contains('.') {
            paths.push(import_name.to_owned());
        }

        paths
    }
}
