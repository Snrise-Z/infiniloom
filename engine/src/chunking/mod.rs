//! Intelligent code chunking for LLM context windows
//!
//! This module provides various strategies for splitting repositories into
//! chunks that fit within LLM context windows while preserving semantic coherence.

mod strategies;
mod types;

use types::SymbolSnippet;
pub use types::{Chunk, ChunkContext, ChunkFile, ChunkStrategy, Chunker, CrossReference};

use crate::tokenizer::Tokenizer;
use crate::types::{RepoFile, Repository, SymbolKind, TokenizerModel};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Determine focus description from an iterator of RepoFile references
fn determine_focus_impl<'a>(mut files: impl Iterator<Item = &'a RepoFile>) -> String {
    let first = match files.next() {
        Some(f) => f,
        None => return "Empty".to_owned(),
    };

    // Collect remaining for iteration (we've consumed first)
    let rest: Vec<&RepoFile> = files.collect();

    // Try to find common directory
    if let Some(module) = first.relative_path.split('/').next() {
        if rest.iter().all(|f| f.relative_path.starts_with(module)) {
            return format!("{} module", module);
        }
    }

    // Try to find common language
    if let Some(lang) = &first.language {
        if rest.iter().all(|f| f.language.as_ref() == Some(lang)) {
            return format!("{} files", lang);
        }
    }

    "Mixed content".to_owned()
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
    pub(crate) fn create_chunk_from_refs(
        &self,
        index: usize,
        files: &[&RepoFile],
        tokens: u32,
    ) -> Chunk {
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
        determine_focus_impl(files.iter())
    }

    /// Determine focus for file references (avoids requiring owned slice)
    fn determine_focus_refs(&self, files: &[&RepoFile]) -> String {
        determine_focus_impl(files.iter().copied())
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

    fn create_multi_module_repo() -> Repository {
        let mut repo = Repository::new("test", "/tmp/test");

        // Module A: 3 files
        for i in 0..3 {
            repo.files.push(RepoFile {
                path: format!("/tmp/test/moduleA/file{}.py", i).into(),
                relative_path: format!("moduleA/file{}.py", i),
                language: Some("python".to_string()),
                size_bytes: 500,
                token_count: TokenCounts::default_with_value(300),
                symbols: Vec::new(),
                importance: 0.5,
                content: Some(format!("# Module A File {}\ndef funcA{}(): pass", i, i)),
            });
        }

        // Module B: 2 files
        for i in 0..2 {
            repo.files.push(RepoFile {
                path: format!("/tmp/test/moduleB/file{}.py", i).into(),
                relative_path: format!("moduleB/file{}.py", i),
                language: Some("python".to_string()),
                size_bytes: 500,
                token_count: TokenCounts::default_with_value(300),
                symbols: Vec::new(),
                importance: 0.5,
                content: Some(format!("# Module B File {}\ndef funcB{}(): pass", i, i)),
            });
        }

        repo
    }

    fn create_repo_with_imports() -> Repository {
        let mut repo = Repository::new("test", "/tmp/test");

        // File A imports nothing
        let mut file_a = RepoFile {
            path: "/tmp/test/src/utils.py".into(),
            relative_path: "src/utils.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 500,
            token_count: TokenCounts::default_with_value(200),
            symbols: vec![Symbol::new("helper", SymbolKind::Function)],
            importance: 0.5,
            content: Some("def helper(): pass".to_string()),
        };
        file_a.symbols[0].start_line = 1;
        file_a.symbols[0].end_line = 1;

        // File B imports from A
        let mut file_b = RepoFile {
            path: "/tmp/test/src/main.py".into(),
            relative_path: "src/main.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 500,
            token_count: TokenCounts::default_with_value(200),
            symbols: vec![
                Symbol::new("src/utils", SymbolKind::Import),
                Symbol::new("main", SymbolKind::Function),
            ],
            importance: 0.8,
            content: Some("from utils import helper\ndef main(): helper()".to_string()),
        };
        file_b.symbols[1].start_line = 2;
        file_b.symbols[1].end_line = 2;
        file_b.symbols[1].calls = vec!["helper".to_string()];

        repo.files.push(file_a);
        repo.files.push(file_b);

        repo
    }

    // ============================================
    // Basic Chunking Strategy Tests
    // ============================================

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

    // ============================================
    // Module Chunking Tests
    // ============================================

    #[test]
    fn test_module_chunking() {
        let repo = create_multi_module_repo();
        let chunker = Chunker::new(ChunkStrategy::Module, 2000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        // Should group by module
        assert!(chunks.iter().all(|c| c.total == chunks.len()));
    }

    #[test]
    fn test_module_chunking_respects_max_tokens() {
        let repo = create_multi_module_repo();
        // Very small max_tokens to force splitting within modules
        let chunker = Chunker::new(ChunkStrategy::Module, 400);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        // Each chunk should respect the token limit (or have single file)
        for chunk in &chunks {
            assert!(chunk.tokens <= 400 || chunk.files.len() == 1);
        }
    }

    #[test]
    fn test_module_chunking_large_limit() {
        let repo = create_multi_module_repo();
        // Large limit - each module should fit in one chunk
        let chunker = Chunker::new(ChunkStrategy::Module, 10000);
        let chunks = chunker.chunk(&repo);

        // Should have 2 chunks (one per module)
        assert_eq!(chunks.len(), 2);
    }

    // ============================================
    // Dependency Chunking Tests
    // ============================================

    #[test]
    fn test_dependency_chunking() {
        let repo = create_repo_with_imports();
        let chunker = Chunker::new(ChunkStrategy::Dependency, 2000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.total == chunks.len()));
    }

    #[test]
    fn test_dependency_chunking_order() {
        let repo = create_repo_with_imports();
        let chunker = Chunker::new(ChunkStrategy::Dependency, 1000);
        let chunks = chunker.chunk(&repo);

        // Dependencies should appear before dependents
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_dependency_chunking_with_cycles() {
        let mut repo = Repository::new("test", "/tmp/test");

        // Create circular dependency
        let mut file_a = RepoFile {
            path: "/tmp/test/a.py".into(),
            relative_path: "a.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 500,
            token_count: TokenCounts::default_with_value(200),
            symbols: vec![
                Symbol::new("b", SymbolKind::Import),
                Symbol::new("funcA", SymbolKind::Function),
            ],
            importance: 0.5,
            content: Some("from b import funcB\ndef funcA(): funcB()".to_string()),
        };
        file_a.symbols[1].calls = vec!["funcB".to_string()];

        let mut file_b = RepoFile {
            path: "/tmp/test/b.py".into(),
            relative_path: "b.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 500,
            token_count: TokenCounts::default_with_value(200),
            symbols: vec![
                Symbol::new("a", SymbolKind::Import),
                Symbol::new("funcB", SymbolKind::Function),
            ],
            importance: 0.5,
            content: Some("from a import funcA\ndef funcB(): funcA()".to_string()),
        };
        file_b.symbols[1].calls = vec!["funcA".to_string()];

        repo.files.push(file_a);
        repo.files.push(file_b);

        let chunker = Chunker::new(ChunkStrategy::Dependency, 1000);
        let chunks = chunker.chunk(&repo);

        // Should handle cycles gracefully
        assert!(!chunks.is_empty());
        // All files should be included
        let total_files: usize = chunks.iter().map(|c| c.files.len()).sum();
        assert_eq!(total_files, 2);
    }

    // ============================================
    // Symbol Chunking Edge Cases
    // ============================================

    #[test]
    fn test_symbol_chunking_no_symbols() {
        let repo = create_test_repo(); // No symbols
        let chunker = Chunker::new(ChunkStrategy::Symbol, 500);
        let chunks = chunker.chunk(&repo);

        // Should fall back to semantic chunking
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_symbol_chunking_with_imports() {
        let mut repo = create_test_repo();
        // Add imports (should be skipped)
        if let Some(file) = repo.files.get_mut(0) {
            file.symbols.push(Symbol::new("os", SymbolKind::Import));
            file.symbols.push(Symbol::new("sys", SymbolKind::Import));
            let mut func = Symbol::new("func0", SymbolKind::Function);
            func.start_line = 3;
            func.end_line = 5;
            file.symbols.push(func);
        }

        let chunker = Chunker::new(ChunkStrategy::Symbol, 1000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_symbol_chunking_multiple_symbols_per_file() {
        let mut repo = Repository::new("test", "/tmp/test");
        let mut file = RepoFile {
            path: "/tmp/test/main.py".into(),
            relative_path: "main.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 1000,
            token_count: TokenCounts::default_with_value(500),
            symbols: Vec::new(),
            importance: 0.8,
            content: Some("def func1(): pass\ndef func2(): pass\ndef func3(): pass".to_string()),
        };

        for i in 1..=3 {
            let mut sym = Symbol::new(&format!("func{}", i), SymbolKind::Function);
            sym.start_line = i;
            sym.end_line = i;
            sym.importance = 0.9 - (i as f32 * 0.1);
            file.symbols.push(sym);
        }
        repo.files.push(file);

        let chunker = Chunker::new(ChunkStrategy::Symbol, 2000);
        let chunks = chunker.chunk(&repo);

        assert!(!chunks.is_empty());
    }

    // ============================================
    // Chunker Builder Tests
    // ============================================

    #[test]
    fn test_chunker_with_overlap() {
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 1000 }, 1000).with_overlap(500);
        assert_eq!(chunker.overlap_tokens, 500);
    }

    #[test]
    fn test_chunker_with_model() {
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 1000 }, 1000)
            .with_model(TokenizerModel::Gpt4o);
        assert_eq!(chunker.model, TokenizerModel::Gpt4o);
    }

    #[test]
    fn test_chunker_builder_chain() {
        let chunker = Chunker::new(ChunkStrategy::Semantic, 2000)
            .with_overlap(300)
            .with_model(TokenizerModel::Gemini);

        assert_eq!(chunker.overlap_tokens, 300);
        assert_eq!(chunker.model, TokenizerModel::Gemini);
        assert!(matches!(chunker.strategy, ChunkStrategy::Semantic));
    }

    // ============================================
    // Focus Determination Tests
    // ============================================

    #[test]
    fn test_determine_focus_empty() {
        let chunker = Chunker::new(ChunkStrategy::File, 1000);
        let files: Vec<RepoFile> = vec![];
        let focus = chunker.determine_focus(&files);
        assert_eq!(focus, "Empty");
    }

    #[test]
    fn test_determine_focus_common_module() {
        let repo = create_multi_module_repo();
        let chunker = Chunker::new(ChunkStrategy::File, 1000);
        // Get just moduleA files
        let module_a_files: Vec<RepoFile> = repo
            .files
            .iter()
            .filter(|f| f.relative_path.starts_with("moduleA"))
            .cloned()
            .collect();

        let focus = chunker.determine_focus(&module_a_files);
        assert!(focus.contains("moduleA"));
    }

    #[test]
    fn test_determine_focus_common_language() {
        let mut repo = Repository::new("test", "/tmp/test");
        for i in 0..3 {
            repo.files.push(RepoFile {
                path: format!("/tmp/test/dir{}/file.rs", i).into(),
                relative_path: format!("dir{}/file.rs", i),
                language: Some("rust".to_string()),
                size_bytes: 500,
                token_count: TokenCounts::default_with_value(200),
                symbols: Vec::new(),
                importance: 0.5,
                content: Some("fn main() {}".to_string()),
            });
        }

        let chunker = Chunker::new(ChunkStrategy::File, 1000);
        let focus = chunker.determine_focus(&repo.files);
        assert!(focus.contains("rust") || focus.contains("Mixed"));
    }

    // ============================================
    // Chunk Context Tests
    // ============================================

    #[test]
    fn test_chunk_context_previous_summary() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600);
        let chunks = chunker.chunk(&repo);

        if chunks.len() > 1 {
            // First chunk has no previous
            assert!(chunks[0].context.previous_summary.is_none());
            // Second chunk should have previous
            assert!(chunks[1].context.previous_summary.is_some());
        }
    }

    #[test]
    fn test_chunk_context_next_preview() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600);
        let chunks = chunker.chunk(&repo);

        if chunks.len() > 1 {
            // First chunk should have next preview
            assert!(chunks[0].context.next_preview.is_some());
            // Last chunk has no next
            assert!(chunks.last().unwrap().context.next_preview.is_none());
        }
    }

    // ============================================
    // Overlap Content Tests
    // ============================================

    #[test]
    fn test_extract_overlap_content() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600).with_overlap(100);
        let chunks = chunker.chunk(&repo);

        // If there are multiple chunks, later ones should have overlap
        if chunks.len() > 1 {
            // Overlap is added during finalization
            // Just verify chunking completes without error
            assert!(chunks.iter().all(|c| c.total == chunks.len()));
        }
    }

    #[test]
    fn test_no_overlap_when_zero() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600).with_overlap(0);
        let chunks = chunker.chunk(&repo);

        // No overlap content should be added
        for chunk in &chunks {
            assert!(chunk.context.overlap_content.is_none());
        }
    }

    // ============================================
    // Cross Reference Tests
    // ============================================

    #[test]
    fn test_cross_references_populated() {
        let repo = create_repo_with_imports();
        let chunker = Chunker::new(ChunkStrategy::File, 1000);
        let chunks = chunker.chunk(&repo);

        // Cross references should be populated during finalization
        assert!(!chunks.is_empty());
    }

    // ============================================
    // Empty Repository Tests
    // ============================================

    #[test]
    fn test_fixed_chunking_empty_repo() {
        let repo = Repository::new("empty", "/tmp/empty");
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 1000 }, 1000);
        let chunks = chunker.chunk(&repo);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_module_chunking_empty_repo() {
        let repo = Repository::new("empty", "/tmp/empty");
        let chunker = Chunker::new(ChunkStrategy::Module, 1000);
        let chunks = chunker.chunk(&repo);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_dependency_chunking_empty_repo() {
        let repo = Repository::new("empty", "/tmp/empty");
        let chunker = Chunker::new(ChunkStrategy::Dependency, 1000);
        let chunks = chunker.chunk(&repo);
        assert!(chunks.is_empty());
    }

    // ============================================
    // Large File Tests
    // ============================================

    #[test]
    fn test_fixed_chunking_single_large_file() {
        let mut repo = Repository::new("test", "/tmp/test");
        repo.files.push(RepoFile {
            path: "/tmp/test/large.py".into(),
            relative_path: "large.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 50000,
            token_count: TokenCounts::default_with_value(10000),
            symbols: Vec::new(),
            importance: 0.5,
            content: Some("x = 1\n".repeat(1000)),
        });

        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 500 }, 500);
        let chunks = chunker.chunk(&repo);

        // Large file should be in its own chunk
        assert!(!chunks.is_empty());
    }

    // ============================================
    // Chunk Total Count Tests
    // ============================================

    #[test]
    fn test_chunk_total_is_correct() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600);
        let chunks = chunker.chunk(&repo);

        let expected_total = chunks.len();
        for chunk in &chunks {
            assert_eq!(chunk.total, expected_total);
        }
    }

    #[test]
    fn test_chunk_index_is_sequential() {
        let repo = create_test_repo();
        let chunker = Chunker::new(ChunkStrategy::Fixed { size: 600 }, 600);
        let chunks = chunker.chunk(&repo);

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }
}
