//! Intelligent code chunking for LLM context windows

use crate::types::{RepoFile, Repository, SymbolKind, TokenizerModel};
use serde::Serialize;

/// A chunk of repository content
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Chunk index (0-based)
    pub index: usize,
    /// Total number of chunks
    pub total: usize,
    /// Focus/theme of this chunk
    pub focus: String,
    /// Token count for this chunk
    pub tokens: u32,
    /// Files included in this chunk
    pub files: Vec<ChunkFile>,
    /// Context information
    pub context: ChunkContext,
}

/// A file within a chunk
#[derive(Debug, Clone, Serialize)]
pub struct ChunkFile {
    /// Relative file path
    pub path: String,
    /// File content (may be compressed)
    pub content: String,
    /// Token count
    pub tokens: u32,
    /// Whether content is truncated
    pub truncated: bool,
}

/// Context for chunk continuity
#[derive(Debug, Clone, Serialize)]
pub struct ChunkContext {
    /// Summary of previous chunks
    pub previous_summary: Option<String>,
    /// Current focus description
    pub current_focus: String,
    /// Preview of next chunk
    pub next_preview: Option<String>,
    /// Cross-references to other chunks
    pub cross_references: Vec<CrossReference>,
    /// Overlap content from previous chunk (for context continuity)
    pub overlap_content: Option<String>,
}

/// Reference to symbol in another chunk
#[derive(Debug, Clone, Serialize)]
pub struct CrossReference {
    /// Symbol name
    pub symbol: String,
    /// Chunk containing the symbol
    pub chunk_index: usize,
    /// File containing the symbol
    pub file: String,
}

#[derive(Debug, Clone)]
struct SymbolSnippet {
    file_path: String,
    symbol_name: String,
    start_line: u32,
    content: String,
    tokens: u32,
    importance: f32,
}

/// Chunking strategy
#[derive(Debug, Clone, Copy, Default)]
pub enum ChunkStrategy {
    /// Fixed token size chunks
    Fixed {
        /// Maximum tokens per chunk
        size: u32,
    },
    /// One file per chunk
    File,
    /// Group by module/directory
    Module,
    /// Group by symbols (AST-based)
    Symbol,
    /// Group by semantic similarity
    #[default]
    Semantic,
    /// Group by dependency order
    Dependency,
}

/// Chunker for splitting repositories
pub struct Chunker {
    /// Chunking strategy
    strategy: ChunkStrategy,
    /// Maximum tokens per chunk
    max_tokens: u32,
    /// Overlap tokens between chunks
    overlap_tokens: u32,
    /// Target model for token counting
    model: TokenizerModel,
}

impl Chunker {
    /// Create a new chunker
    pub fn new(strategy: ChunkStrategy, max_tokens: u32) -> Self {
        Self { strategy, max_tokens, overlap_tokens: 200, model: TokenizerModel::Claude }
    }

    /// Set overlap tokens
    pub fn with_overlap(mut self, tokens: u32) -> Self {
        self.overlap_tokens = tokens;
        self
    }

    /// Set target model
    pub fn with_model(mut self, model: TokenizerModel) -> Self {
        self.model = model;
        self
    }

    /// Chunk a repository
    pub fn chunk(&self, repo: &Repository) -> Vec<Chunk> {
        match self.strategy {
            ChunkStrategy::Fixed { size } => self.fixed_chunk(repo, size),
            ChunkStrategy::File => self.file_chunk(repo),
            ChunkStrategy::Module => self.module_chunk(repo),
            ChunkStrategy::Symbol => self.symbol_chunk(repo),
            ChunkStrategy::Semantic => self.semantic_chunk(repo),
            ChunkStrategy::Dependency => self.dependency_chunk(repo),
        }
    }

    /// Fixed-size chunking
    fn fixed_chunk(&self, repo: &Repository, size: u32) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_files = Vec::new();
        let mut current_tokens = 0u32;

        for file in &repo.files {
            let file_tokens = file.token_count.get(self.model);

            if current_tokens + file_tokens > size && !current_files.is_empty() {
                chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
                current_files.clear();
                current_tokens = 0;
            }

            current_files.push(file.clone());
            current_tokens += file_tokens;
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
        }

        self.finalize_chunks(chunks, repo)
    }

    /// One file per chunk
    fn file_chunk(&self, repo: &Repository) -> Vec<Chunk> {
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
    fn module_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        use std::collections::HashMap;

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
    fn symbol_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        use crate::tokenizer::Tokenizer;

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
    fn semantic_chunk(&self, repo: &Repository) -> Vec<Chunk> {
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
    fn dependency_chunk(&self, repo: &Repository) -> Vec<Chunk> {
        use std::collections::{HashMap, HashSet, VecDeque};

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
        let mut current_files = Vec::new();
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
                chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
                current_files.clear();
                current_tokens = 0;
                current_deps.clear();
            }

            current_files.push(file.clone());
            current_tokens += file_tokens;
            current_deps.insert(idx);
        }

        if !current_files.is_empty() {
            chunks.push(self.create_chunk(chunks.len(), &current_files, current_tokens));
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

    fn create_chunk(&self, index: usize, files: &[RepoFile], tokens: u32) -> Chunk {
        let focus = self.determine_focus(files);

        Chunk {
            index,
            total: 0, // Updated in finalize
            focus: focus.clone(),
            tokens,
            files: files
                .iter()
                .map(|f| ChunkFile {
                    path: f.relative_path.clone(),
                    content: f.content.clone().unwrap_or_default(),
                    tokens: f.token_count.get(self.model),
                    truncated: false,
                })
                .collect(),
            context: ChunkContext {
                previous_summary: None,
                current_focus: focus,
                next_preview: None,
                cross_references: Vec::new(),
                overlap_content: None,
            },
        }
    }

    fn build_symbol_chunk(
        &self,
        index: usize,
        snippets: &[SymbolSnippet],
        tokenizer: &crate::tokenizer::Tokenizer,
    ) -> Chunk {
        use std::collections::BTreeMap;

        let focus = self.determine_symbol_focus(snippets);
        let mut by_file: BTreeMap<&str, Vec<&SymbolSnippet>> = BTreeMap::new();

        for snippet in snippets {
            by_file
                .entry(snippet.file_path.as_str())
                .or_default()
                .push(snippet);
        }

        let mut files = Vec::new();
        let mut total_tokens = 0u32;

        for (path, mut entries) in by_file {
            entries.sort_by(|a, b| {
                a.start_line
                    .cmp(&b.start_line)
                    .then_with(|| a.symbol_name.cmp(&b.symbol_name))
            });

            let mut content = String::new();
            for entry in entries {
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                content.push_str(&entry.content);
            }

            let tokens = tokenizer.count(&content, self.model);
            total_tokens += tokens;

            files.push(ChunkFile { path: path.to_owned(), content, tokens, truncated: false });
        }

        Chunk {
            index,
            total: 0,
            focus: focus.clone(),
            tokens: total_tokens,
            files,
            context: ChunkContext {
                previous_summary: None,
                current_focus: focus,
                next_preview: None,
                cross_references: Vec::new(),
                overlap_content: None,
            },
        }
    }

    fn determine_focus(&self, files: &[RepoFile]) -> String {
        if files.is_empty() {
            return "Empty".to_owned();
        }

        // Try to find common directory
        let first_path = &files[0].relative_path;
        if let Some(module) = first_path.split('/').next() {
            if files.iter().all(|f| f.relative_path.starts_with(module)) {
                return format!("{} module", module);
            }
        }

        // Try to find common language
        if let Some(lang) = &files[0].language {
            if files.iter().all(|f| f.language.as_ref() == Some(lang)) {
                return format!("{} files", lang);
            }
        }

        "Mixed content".to_owned()
    }

    fn determine_symbol_focus(&self, snippets: &[SymbolSnippet]) -> String {
        if snippets.is_empty() {
            return "Symbols".to_owned();
        }

        let mut names: Vec<String> = snippets
            .iter()
            .take(3)
            .map(|snippet| snippet.symbol_name.clone())
            .collect();

        let suffix = if snippets.len() > names.len() {
            format!(" +{} more", snippets.len() - names.len())
        } else {
            String::new()
        };

        if names.len() == 1 {
            format!("Symbol: {}{}", names.remove(0), suffix)
        } else {
            format!("Symbols: {}{}", names.join(", "), suffix)
        }
    }

    fn get_overlap_files(&self, files: &[RepoFile]) -> Vec<RepoFile> {
        // Keep files that might be needed for context
        // For now, just keep the last file if it's small enough
        files
            .last()
            .filter(|f| f.token_count.get(self.model) < self.overlap_tokens)
            .cloned()
            .into_iter()
            .collect()
    }

    fn finalize_chunks(&self, mut chunks: Vec<Chunk>, repo: &Repository) -> Vec<Chunk> {
        let total = chunks.len();

        // First pass: collect the focus strings and overlap content we need
        let focus_strs: Vec<String> = chunks.iter().map(|c| c.focus.clone()).collect();

        // Extract overlap content from each chunk for the next one
        let overlap_contents: Vec<Option<String>> = if self.overlap_tokens > 0 {
            chunks
                .iter()
                .map(|chunk| self.extract_overlap_content(chunk))
                .collect()
        } else {
            vec![None; chunks.len()]
        };

        for (i, chunk) in chunks.iter_mut().enumerate() {
            chunk.total = total;

            // Add previous summary
            if i > 0 {
                chunk.context.previous_summary = Some(format!("Previous: {}", focus_strs[i - 1]));

                // Add overlap content from previous chunk
                if let Some(ref overlap) = overlap_contents[i - 1] {
                    chunk.context.overlap_content = Some(format!(
                        "<!-- [OVERLAP FROM PREVIOUS CHUNK] -->\n{}\n<!-- [END OVERLAP] -->",
                        overlap
                    ));
                }
            }

            // Add next preview
            if i + 1 < total {
                chunk.context.next_preview = Some(format!("Next: Chunk {}", i + 2));
            }
        }

        self.populate_cross_references(&mut chunks, repo);

        chunks
    }

    fn populate_cross_references(&self, chunks: &mut [Chunk], repo: &Repository) {
        use std::collections::{HashMap, HashSet};

        const MAX_REFS: usize = 25;

        #[derive(Clone)]
        struct SymbolLocation {
            chunk_index: usize,
            file: String,
        }

        let file_lookup: HashMap<&str, &RepoFile> = repo
            .files
            .iter()
            .map(|file| (file.relative_path.as_str(), file))
            .collect();

        let mut symbol_index: HashMap<String, Vec<SymbolLocation>> = HashMap::new();
        let mut seen_symbols: HashSet<(String, usize, String)> = HashSet::new();

        for (chunk_index, chunk) in chunks.iter().enumerate() {
            for chunk_file in &chunk.files {
                if let Some(repo_file) = file_lookup.get(chunk_file.path.as_str()) {
                    for symbol in &repo_file.symbols {
                        if symbol.kind == SymbolKind::Import {
                            continue;
                        }
                        let key = (symbol.name.clone(), chunk_index, chunk_file.path.clone());
                        if seen_symbols.insert(key) {
                            symbol_index.entry(symbol.name.clone()).or_default().push(
                                SymbolLocation { chunk_index, file: chunk_file.path.clone() },
                            );
                        }
                    }
                }
            }
        }

        for (chunk_index, chunk) in chunks.iter_mut().enumerate() {
            let mut refs: Vec<CrossReference> = Vec::new();
            let mut seen_refs: HashSet<(String, usize, String)> = HashSet::new();

            'files: for chunk_file in &chunk.files {
                if let Some(repo_file) = file_lookup.get(chunk_file.path.as_str()) {
                    for symbol in &repo_file.symbols {
                        for called in &symbol.calls {
                            if let Some(targets) = symbol_index.get(called) {
                                for target in targets {
                                    if target.chunk_index == chunk_index {
                                        continue;
                                    }
                                    let key = (
                                        called.to_owned(),
                                        target.chunk_index,
                                        target.file.clone(),
                                    );
                                    if seen_refs.insert(key) {
                                        refs.push(CrossReference {
                                            symbol: called.to_owned(),
                                            chunk_index: target.chunk_index,
                                            file: target.file.clone(),
                                        });
                                        if refs.len() >= MAX_REFS {
                                            break 'files;
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(ref base) = symbol.extends {
                            if let Some(targets) = symbol_index.get(base) {
                                for target in targets {
                                    if target.chunk_index == chunk_index {
                                        continue;
                                    }
                                    let key =
                                        (base.to_owned(), target.chunk_index, target.file.clone());
                                    if seen_refs.insert(key) {
                                        refs.push(CrossReference {
                                            symbol: base.to_owned(),
                                            chunk_index: target.chunk_index,
                                            file: target.file.clone(),
                                        });
                                        if refs.len() >= MAX_REFS {
                                            break 'files;
                                        }
                                    }
                                }
                            }
                        }

                        for iface in &symbol.implements {
                            if let Some(targets) = symbol_index.get(iface) {
                                for target in targets {
                                    if target.chunk_index == chunk_index {
                                        continue;
                                    }
                                    let key =
                                        (iface.to_owned(), target.chunk_index, target.file.clone());
                                    if seen_refs.insert(key) {
                                        refs.push(CrossReference {
                                            symbol: iface.to_owned(),
                                            chunk_index: target.chunk_index,
                                            file: target.file.clone(),
                                        });
                                        if refs.len() >= MAX_REFS {
                                            break 'files;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            refs.sort_by(|a, b| {
                a.chunk_index
                    .cmp(&b.chunk_index)
                    .then_with(|| a.symbol.cmp(&b.symbol))
                    .then_with(|| a.file.cmp(&b.file))
            });
            if refs.len() > MAX_REFS {
                refs.truncate(MAX_REFS);
            }

            chunk.context.cross_references = refs;
        }
    }

    /// Extract content from the end of a chunk for overlap
    fn extract_overlap_content(&self, chunk: &Chunk) -> Option<String> {
        use crate::tokenizer::Tokenizer;

        if self.overlap_tokens == 0 || chunk.files.is_empty() {
            return None;
        }

        let tokenizer = Tokenizer::new();
        let mut overlap_parts = Vec::new();
        let mut remaining_tokens = self.overlap_tokens;
        let token_model = self.model;

        // Take content from the last files until we've accumulated enough tokens
        for file in chunk.files.iter().rev() {
            if remaining_tokens == 0 {
                break;
            }

            let file_tokens = tokenizer.count(&file.content, token_model);
            if file_tokens <= remaining_tokens {
                // Include entire file
                overlap_parts.push(format!("// From: {}\n{}", file.path, file.content));
                remaining_tokens = remaining_tokens.saturating_sub(file_tokens);
            } else {
                // Include partial file (last N lines that fit)
                let lines: Vec<&str> = file.content.lines().collect();
                let mut partial_lines = Vec::new();
                let mut partial_tokens = 0u32;

                for line in lines.iter().rev() {
                    let line_tokens = tokenizer.count(line, token_model);
                    if partial_tokens + line_tokens > remaining_tokens {
                        break;
                    }
                    partial_lines.push(*line);
                    partial_tokens += line_tokens;
                }

                if !partial_lines.is_empty() {
                    partial_lines.reverse();
                    let partial_content = partial_lines.join("\n");
                    overlap_parts
                        .push(format!("// From: {} (partial)\n{}", file.path, partial_content));
                }
                remaining_tokens = 0;
            }
        }

        if overlap_parts.is_empty() {
            None
        } else {
            overlap_parts.reverse();
            Some(overlap_parts.join("\n\n"))
        }
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::types::{Symbol, SymbolKind, TokenCounts, Visibility};

    fn create_test_repo() -> Repository {
        let mut repo = Repository::new("test", "/tmp/test");

        for i in 0..5 {
            repo.files.push(RepoFile {
                path: format!("/tmp/test/src/file{}.py", i).into(),
                relative_path: format!("src/file{}.py", i),
                language: Some("python".to_string()),
                size_bytes: 1000,
                token_count: TokenCounts {
                    o200k: 480,
                    cl100k: 490,
                    claude: 500,
                    gemini: 470,
                    llama: 460,
                    mistral: 460,
                    deepseek: 460,
                    qwen: 460,
                    cohere: 465,
                    grok: 460,
                },
                symbols: Vec::new(),
                importance: 0.5,
                content: Some(format!("# File {}\ndef func{}(): pass", i, i)),
            });
        }

        repo
    }

    #[test]
    fn test_fixed_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 1000 }, 1000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        assert!(chunks
            .iter()
            .all(|c| c.tokens <= 1000 || c.files.len() == 1));
    }

    #[test]
    fn test_file_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::File, 8000);
        let chunks = chunker.chunk(&repo);

        assert_eq!(chunks.len(), repo.files.len());
    }

    #[test]
    fn test_semantic_chunking() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Semantic, 2000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        // All chunks should have correct total
        assert!(chunks.iter().all(|c| c.total == chunks.len()));
    }

    #[test]
    fn test_symbol_chunking() {
        let mut repo = create_test_repo();
        if let Some(file) = repo.files.get_mut(0) {
            let mut symbol = Symbol::new("func0", SymbolKind::Function);
            symbol.start_line = 1;
            symbol.end_line = 1;
            symbol.visibility = Visibility::Public;
            file.symbols.push(symbol);
        }

        let chunker = Chunker::new(ChunkStrategy::Symbol, 500);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.total == chunks.len()));
    }
}
