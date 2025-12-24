//! Intelligent code chunking for LLM context windows
//!
//! This module provides various strategies for splitting repositories into
//! chunks that fit within LLM context windows while preserving semantic coherence.

mod strategies;
mod types;

pub use types::{Chunk, ChunkContext, ChunkFile, ChunkStrategy, Chunker, CrossReference};
use types::SymbolSnippet;

use crate::tokenizer::Tokenizer;
use crate::types::{RepoFile, Repository, SymbolKind, TokenizerModel};
use std::collections::{BTreeMap, HashMap, HashSet};

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

    // =========================================================================
    // Chunk creation helpers
    // =========================================================================

    pub(crate) fn create_chunk(&self, index: usize, files: &[RepoFile], tokens: u32) -> Chunk {
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

    /// Create a chunk from file references (avoids cloning RepoFile)
    pub(crate) fn create_chunk_from_refs(&self, index: usize, files: &[&RepoFile], tokens: u32) -> Chunk {
        let focus = self.determine_focus_refs(files);

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

    pub(crate) fn build_symbol_chunk(
        &self,
        index: usize,
        snippets: &[SymbolSnippet],
        tokenizer: &Tokenizer,
    ) -> Chunk {
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

    // =========================================================================
    // Focus determination
    // =========================================================================

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

    /// Determine focus for file references (avoids requiring owned slice)
    fn determine_focus_refs(&self, files: &[&RepoFile]) -> String {
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

    // =========================================================================
    // Overlap and finalization
    // =========================================================================

    pub(crate) fn get_overlap_files(&self, files: &[RepoFile]) -> Vec<RepoFile> {
        // Keep files that might be needed for context
        // For now, just keep the last file if it's small enough
        files
            .last()
            .filter(|f| f.token_count.get(self.model) < self.overlap_tokens)
            .cloned()
            .into_iter()
            .collect()
    }

    pub(crate) fn finalize_chunks(&self, mut chunks: Vec<Chunk>, repo: &Repository) -> Vec<Chunk> {
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
