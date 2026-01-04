//! Core chunking logic for embedding generation
//!
//! This module generates deterministic, content-addressable code chunks from
//! a repository. It uses thread-local parsers for parallel processing and
//! enforces resource limits for DoS protection.
//!
//! # Thread Safety
//!
//! The chunker uses thread-local parsers to avoid mutex contention during
//! parallel file processing. Each Rayon worker thread gets its own parser
//! instance.
//!
//! # Determinism Guarantees
//!
//! 1. Files are processed in sorted lexicographic order
//! 2. Symbols within files are sorted by (line, name)
//! 3. Output chunks are sorted by (file, line, id)
//! 4. All hash computations use integer-only math (no floats)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::parser::{parse_file_symbols, Language};
use crate::security::SecurityScanner;
use crate::tokenizer::{TokenModel, Tokenizer};
use crate::types::Symbol;

use super::error::EmbedError;
use super::hasher::hash_content;
use super::hierarchy::{HierarchyBuilder, HierarchyConfig};
use super::limits::ResourceLimits;
use super::progress::ProgressReporter;
use super::types::{
    ChunkContext, ChunkKind, ChunkPart, ChunkSource, EmbedChunk, EmbedSettings, RepoIdentifier,
    Visibility,
};

/// Core chunker for generating embedding chunks
pub struct EmbedChunker {
    settings: EmbedSettings,
    limits: ResourceLimits,
    tokenizer: Tokenizer,
    security_scanner: Option<SecurityScanner>,
    /// Repository identifier for multi-tenant RAG
    repo_id: RepoIdentifier,
}

impl EmbedChunker {
    /// Create a new chunker with the given settings and limits
    pub fn new(settings: EmbedSettings, limits: ResourceLimits) -> Self {
        // Initialize security scanner if secret scanning is enabled
        let security_scanner = if settings.scan_secrets {
            Some(SecurityScanner::new())
        } else {
            None
        };

        Self {
            settings,
            limits,
            tokenizer: Tokenizer::new(),
            security_scanner,
            repo_id: RepoIdentifier::default(),
        }
    }

    /// Create a new chunker with default limits
    pub fn with_defaults(settings: EmbedSettings) -> Self {
        Self::new(settings, ResourceLimits::default())
    }

    /// Set the repository identifier for multi-tenant RAG
    ///
    /// This identifier is attached to all generated chunks, enabling:
    /// - Multi-repository search with proper attribution
    /// - Access control filtering by repository
    /// - Cross-repository dependency tracking
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chunker = EmbedChunker::with_defaults(settings)
    ///     .with_repo_id(RepoIdentifier::new("github.com/myorg", "auth-service"));
    /// ```
    pub fn with_repo_id(mut self, repo_id: RepoIdentifier) -> Self {
        self.repo_id = repo_id;
        self
    }

    /// Set the repository identifier (mutable borrow version)
    pub fn set_repo_id(&mut self, repo_id: RepoIdentifier) {
        self.repo_id = repo_id;
    }

    /// Get the current repository identifier
    pub fn repo_id(&self) -> &RepoIdentifier {
        &self.repo_id
    }

    /// Generate all chunks for a repository
    ///
    /// # Guarantees
    ///
    /// 1. Deterministic output (same input = same output)
    /// 2. Thread-safe parallel processing
    /// 3. Resource limits enforced
    /// 4. Errors collected, not swallowed
    pub fn chunk_repository(
        &self,
        repo_path: &Path,
        progress: &dyn ProgressReporter,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate repo path
        let repo_root = self.validate_repo_path(repo_path)?;

        // Phase 1: Discover files (deterministic order)
        progress.set_phase("Scanning repository...");
        let mut files = self.discover_files(&repo_root)?;
        files.sort(); // Critical for determinism
        progress.set_total(files.len());

        if files.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        // Check file limit
        if !self.limits.check_file_count(files.len()) {
            return Err(EmbedError::TooManyFiles {
                count: files.len(),
                max: self.limits.max_files,
            });
        }

        // Phase 2: Process files in parallel
        progress.set_phase("Parsing and chunking...");
        let chunk_count = AtomicUsize::new(0);
        let processed = AtomicUsize::new(0);

        // Collect results AND errors (don't swallow errors)
        let results: Vec<Result<Vec<EmbedChunk>, (PathBuf, EmbedError)>> = files
            .par_iter()
            .map(|file| {
                let result = self.chunk_file(file, &repo_root);

                // Update progress
                let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
                progress.set_progress(done);

                match result {
                    Ok(chunks) => {
                        // Atomically check and update chunk count to prevent race conditions
                        // Use compare_exchange loop to ensure we don't exceed limits
                        let chunks_to_add = chunks.len();
                        loop {
                            // Use Acquire ordering for proper synchronization on ARM
                            let current = chunk_count.load(Ordering::Acquire);
                            let new_count = current + chunks_to_add;

                            // Check chunk limit BEFORE incrementing
                            if !self.limits.check_chunk_count(new_count) {
                                return Err((
                                    file.clone(),
                                    EmbedError::TooManyChunks {
                                        count: new_count,
                                        max: self.limits.max_total_chunks,
                                    },
                                ));
                            }

                            // Try to atomically update the counter
                            // Use AcqRel/Acquire ordering for correctness on ARM architectures
                            match chunk_count.compare_exchange(
                                current,
                                new_count,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            ) {
                                Ok(_) => break,     // Successfully updated
                                Err(_) => continue, // Another thread updated, retry
                            }
                        }

                        Ok(chunks)
                    },
                    Err(e) => Err((file.clone(), e)),
                }
            })
            .collect();

        // Separate successes and failures
        let mut all_chunks = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(chunks) => all_chunks.extend(chunks),
                Err((path, err)) => errors.push((path, err)),
            }
        }

        // Report errors (fail on critical, warn on non-critical)
        if !errors.is_empty() {
            let critical: Vec<_> = errors
                .iter()
                .filter(|(_, e)| e.is_critical())
                .cloned()
                .collect();

            if !critical.is_empty() {
                return Err(EmbedError::from_file_errors(critical));
            }

            // Non-critical errors: log warning, continue
            for (path, err) in &errors {
                if err.is_skippable() {
                    progress.warn(&format!("Skipped {}: {}", path.display(), err));
                }
            }
        }

        // Check if any chunks were generated
        if all_chunks.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        // Phase 3: Build reverse call graph (called_by)
        progress.set_phase("Building call graph...");
        self.populate_called_by(&mut all_chunks);

        // Phase 4: Build hierarchy summaries (if enabled)
        if self.settings.enable_hierarchy {
            progress.set_phase("Building hierarchy summaries...");
            let hierarchy_config = HierarchyConfig {
                min_children_for_summary: self.settings.hierarchy_min_children,
                ..Default::default()
            };
            let builder = HierarchyBuilder::with_config(hierarchy_config);

            // Enrich existing chunks with hierarchy metadata tags
            builder.enrich_chunks(&mut all_chunks);

            // Generate summary chunks for containers (classes, structs, etc.)
            let mut summaries = builder.build_hierarchy(&all_chunks);

            // Count tokens for summary chunks
            let token_model = self.parse_token_model(&self.settings.token_model);
            for summary in &mut summaries {
                summary.tokens = self.tokenizer.count(&summary.content, token_model);
            }

            all_chunks.extend(summaries);
        }

        // Phase 5: Sort for deterministic output
        // Note: par_sort_by is unstable, but our comparison uses multiple tiebreakers
        // to guarantee no two elements ever compare equal, making stability irrelevant.
        // Order: file → start line → end line → symbol name → chunk ID
        progress.set_phase("Sorting chunks...");
        all_chunks.par_sort_by(|a, b| {
            a.source
                .file
                .cmp(&b.source.file)
                .then_with(|| a.source.lines.0.cmp(&b.source.lines.0))
                .then_with(|| a.source.lines.1.cmp(&b.source.lines.1))
                .then_with(|| a.source.symbol.cmp(&b.source.symbol))
                .then_with(|| a.id.cmp(&b.id)) // Content-addressable ID as final tiebreaker
        });

        progress.set_phase("Complete");
        Ok(all_chunks)
    }

    /// Populate the called_by field for all chunks by building a reverse call graph
    ///
    /// This scans all chunks' `calls` fields and creates reverse mappings to populate
    /// the `called_by` field, enabling bidirectional navigation of the call graph.
    fn populate_called_by(&self, chunks: &mut [EmbedChunk]) {
        use std::collections::{BTreeMap, BTreeSet};

        // Build map: callee_name -> set of caller FQNs
        // Using BTreeMap/BTreeSet for deterministic iteration order and O(log n) deduplication
        let mut reverse_calls: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        // First pass: collect all calls (deduplicated via BTreeSet)
        for chunk in chunks.iter() {
            let caller_fqn = chunk.source.fqn.as_deref().unwrap_or(&chunk.source.symbol);
            for callee in &chunk.context.calls {
                reverse_calls
                    .entry(callee.clone())
                    .or_default()
                    .insert(caller_fqn.to_owned());
            }
        }

        // Second pass: populate called_by
        for chunk in chunks.iter_mut() {
            // Try to match by FQN first, then by symbol name
            let fqn = chunk.source.fqn.as_deref().unwrap_or("");
            let symbol = &chunk.source.symbol;

            // Use BTreeSet for O(log n) deduplication instead of Vec::contains() which is O(n)
            let mut called_by_set: BTreeSet<String> = BTreeSet::new();

            // Check if anyone calls this symbol by FQN
            if let Some(callers) = reverse_calls.get(fqn) {
                called_by_set.extend(callers.iter().cloned());
            }

            // Also check if anyone calls by just the symbol name (common for unqualified calls)
            if let Some(callers) = reverse_calls.get(symbol) {
                called_by_set.extend(callers.iter().cloned());
            }

            // BTreeSet is already sorted, convert to Vec
            chunk.context.called_by = called_by_set.into_iter().collect();
        }
    }

    /// Chunk a single file using thread-local resources
    fn chunk_file(&self, path: &Path, repo_root: &Path) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate file size
        let metadata = std::fs::metadata(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        if !self.limits.check_file_size(metadata.len()) {
            return Err(EmbedError::FileTooLarge {
                path: path.to_path_buf(),
                size: metadata.len(),
                max: self.limits.max_file_size,
            });
        }

        // Read file
        let mut content = std::fs::read_to_string(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Check for excessively long lines (e.g., minified files)
        // This prevents memory issues from processing single-line 10MB files
        if let Some(max_line_len) = content.lines().map(|l| l.len()).max() {
            if !self.limits.check_line_length(max_line_len) {
                return Err(EmbedError::LineTooLong {
                    path: path.to_path_buf(),
                    length: max_line_len,
                    max: self.limits.max_line_length,
                });
            }
        }

        // Get relative path (safe, validated)
        let relative_path = self.safe_relative_path(path, repo_root)?;

        // Security scanning (if enabled)
        if let Some(ref scanner) = self.security_scanner {
            let findings = scanner.scan(&content, &relative_path);
            if !findings.is_empty() {
                // Check if we should fail on secrets
                if self.settings.fail_on_secrets {
                    let files = findings
                        .iter()
                        .map(|f| format!("  {}:{} - {}", f.file, f.line, f.kind.name()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Err(EmbedError::SecretsDetected { count: findings.len(), files });
                }

                // Redact secrets if configured
                if self.settings.redact_secrets {
                    content = scanner.redact_content(&content, &relative_path);
                }
            }
        }
        let language = self.detect_language(path);

        // Use thread-local parser (from parser module)
        let mut symbols = parse_file_symbols(&content, path);

        // Sort symbols deterministically (stable sort preserves parser order for equal elements)
        symbols.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| a.end_line.cmp(&b.end_line))
                .then_with(|| a.name.cmp(&b.name))
        });

        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::with_capacity(symbols.len() + 2);

        for symbol in &symbols {
            // Skip imports if configured
            if !self.settings.include_imports
                && matches!(symbol.kind, crate::types::SymbolKind::Import)
            {
                continue;
            }

            // Extract content with context
            let (chunk_content, start_line, end_line) =
                self.extract_symbol_content(&lines, symbol, self.settings.context_lines);

            // Count tokens
            let token_model = self.parse_token_model(&self.settings.token_model);
            let tokens = self.tokenizer.count(&chunk_content, token_model);

            // Handle large symbols (with depth-limited splitting)
            if self.settings.max_tokens > 0 && tokens > self.settings.max_tokens {
                let split_chunks = self.split_large_symbol(
                    &chunk_content,
                    symbol,
                    &relative_path,
                    &language,
                    start_line,
                    0, // Initial depth
                )?;
                chunks.extend(split_chunks);
            } else {
                // Generate hash (single pass)
                let hash = hash_content(&chunk_content);

                // Extract context (with complexity metrics)
                let context = self.extract_context(symbol, &chunk_content);

                // Compute fully qualified name for symbol disambiguation
                let fqn = self.compute_fqn(&relative_path, symbol);

                chunks.push(EmbedChunk {
                    id: hash.short_id,
                    full_hash: hash.full_hash,
                    content: chunk_content,
                    tokens,
                    kind: symbol.kind.into(),
                    source: ChunkSource {
                        repo: self.repo_id.clone(),
                        file: relative_path.clone(),
                        lines: (start_line, end_line),
                        symbol: symbol.name.clone(),
                        fqn: Some(fqn),
                        language: language.clone(),
                        parent: symbol.parent.clone(),
                        visibility: symbol.visibility.into(),
                        is_test: self.is_test_code(path, symbol),
                    },
                    context,
                    part: None,
                });
            }
        }

        // Handle top-level code if configured
        if self.settings.include_top_level && !symbols.is_empty() {
            if let Some(top_level) =
                self.extract_top_level(&lines, &symbols, &relative_path, &language)
            {
                chunks.push(top_level);
            }
        }

        Ok(chunks)
    }

    /// Extract symbol content with context lines
    fn extract_symbol_content(
        &self,
        lines: &[&str],
        symbol: &Symbol,
        context_lines: u32,
    ) -> (String, u32, u32) {
        // Convert to 0-indexed, clamped to bounds
        let start_line = symbol.start_line.saturating_sub(1) as usize;
        let end_line = (symbol.end_line as usize).min(lines.len());

        // Add context lines (clamped)
        let context_start = start_line.saturating_sub(context_lines as usize);
        let context_end = (end_line + context_lines as usize).min(lines.len());

        // Extract content
        let content = lines[context_start..context_end].join("\n");

        // Return 1-indexed line numbers
        (content, (context_start + 1) as u32, context_end as u32)
    }

    /// Split a large symbol into multiple chunks at line boundaries
    ///
    /// This implements overlap between consecutive chunks for context continuity.
    /// Each chunk (except the first) includes `overlap_tokens` worth of lines from
    /// the end of the previous chunk. This helps RAG systems understand context
    /// when retrieving individual chunks.
    fn split_large_symbol(
        &self,
        content: &str,
        symbol: &Symbol,
        file: &str,
        language: &str,
        base_line: u32,
        depth: u32,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Depth limit to prevent stack overflow
        if !self.limits.check_recursion_depth(depth) {
            return Err(EmbedError::RecursionLimitExceeded {
                depth,
                max: self.limits.max_recursion_depth,
                context: format!("splitting symbol {}", symbol.name),
            });
        }

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Calculate target lines per chunk using INTEGER math only
        let token_model = self.parse_token_model(&self.settings.token_model);
        let total_tokens = self.tokenizer.count(content, token_model) as usize;
        let target_tokens = self.settings.max_tokens as usize;

        if total_tokens == 0 || target_tokens == 0 {
            return Ok(Vec::new());
        }

        // INTEGER division: (total_lines * target_tokens) / total_tokens
        let target_lines = ((total_lines * target_tokens) / total_tokens).max(1);

        // Calculate overlap lines from overlap_tokens setting
        // Estimate: overlap_lines = (total_lines * overlap_tokens) / total_tokens
        let overlap_tokens = self.settings.overlap_tokens as usize;
        let overlap_lines = if overlap_tokens > 0 && total_tokens > 0 {
            ((total_lines * overlap_tokens) / total_tokens)
                .max(1)
                .min(target_lines / 2)
        } else {
            0
        };

        let mut chunks = Vec::new();
        let mut current_start = 0usize;
        let mut part_num = 1u32;

        // Parent ID for linking parts
        let parent_hash = hash_content(content);

        while current_start < total_lines {
            // Calculate content boundaries
            // For parts after the first, include overlap from the previous chunk
            let content_start = if part_num > 1 && overlap_lines > 0 {
                current_start.saturating_sub(overlap_lines)
            } else {
                current_start
            };
            let content_end = (current_start + target_lines).min(total_lines);

            let part_content = lines[content_start..content_end].join("\n");

            let tokens = self.tokenizer.count(&part_content, token_model);

            // Only create chunk if above minimum
            if tokens >= self.settings.min_tokens {
                let hash = hash_content(&part_content);

                // Track actual overlap lines included (for metadata)
                let actual_overlap = if part_num > 1 {
                    current_start.saturating_sub(content_start) as u32
                } else {
                    0
                };

                chunks.push(EmbedChunk {
                    id: hash.short_id,
                    full_hash: hash.full_hash,
                    content: part_content,
                    tokens,
                    kind: ChunkKind::FunctionPart, // or ClassPart based on symbol.kind
                    source: ChunkSource {
                        repo: self.repo_id.clone(),
                        file: file.to_owned(),
                        lines: (
                            base_line + content_start as u32,
                            base_line + content_end as u32 - 1,
                        ),
                        symbol: format!("{}_part{}", symbol.name, part_num),
                        fqn: None,
                        language: language.to_owned(),
                        parent: Some(symbol.name.clone()),
                        visibility: symbol.visibility.into(),
                        is_test: false,
                    },
                    context: ChunkContext {
                        signature: symbol.signature.clone(), // Include in every part for context
                        // Propagate docstring to ALL parts for better RAG retrieval
                        // Each part should be self-contained for semantic search
                        docstring: symbol.docstring.clone(),
                        ..Default::default()
                    },
                    part: Some(ChunkPart {
                        part: part_num,
                        of: 0, // Updated after all parts
                        parent_id: parent_hash.short_id.clone(),
                        parent_signature: symbol.signature.clone().unwrap_or_default(),
                        overlap_lines: actual_overlap,
                    }),
                });

                part_num += 1;
            }

            current_start = content_end;
        }

        // Update total part count
        let total_parts = chunks.len() as u32;
        for chunk in &mut chunks {
            if let Some(ref mut part) = chunk.part {
                part.of = total_parts;
            }
        }

        Ok(chunks)
    }

    /// Extract top-level code (code outside symbols)
    fn extract_top_level(
        &self,
        lines: &[&str],
        symbols: &[Symbol],
        file: &str,
        language: &str,
    ) -> Option<EmbedChunk> {
        if lines.is_empty() || symbols.is_empty() {
            return None;
        }

        // Find lines not covered by any symbol
        let mut covered = vec![false; lines.len()];
        for symbol in symbols {
            let start = symbol.start_line.saturating_sub(1) as usize;
            let end = (symbol.end_line as usize).min(lines.len());
            for i in start..end {
                covered[i] = true;
            }
        }

        // Collect uncovered lines
        let top_level_lines: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !covered[*i])
            .map(|(_, line)| *line)
            .collect();

        if top_level_lines.is_empty() {
            return None;
        }

        let content = top_level_lines.join("\n").trim().to_owned();
        if content.is_empty() {
            return None;
        }

        let token_model = self.parse_token_model(&self.settings.token_model);
        let tokens = self.tokenizer.count(&content, token_model);

        if tokens < self.settings.min_tokens {
            return None;
        }

        let hash = hash_content(&content);

        Some(EmbedChunk {
            id: hash.short_id,
            full_hash: hash.full_hash,
            content,
            tokens,
            kind: ChunkKind::TopLevel,
            source: ChunkSource {
                repo: self.repo_id.clone(),
                file: file.to_owned(),
                lines: (1, lines.len() as u32),
                symbol: "<top_level>".to_owned(),
                fqn: None,
                language: language.to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
            },
            context: ChunkContext::default(),
            part: None,
        })
    }

    /// Extract semantic context for retrieval
    fn extract_context(&self, symbol: &Symbol, content: &str) -> ChunkContext {
        ChunkContext {
            docstring: symbol.docstring.clone(),
            comments: Vec::new(), // TODO: Extract inline comments
            signature: symbol.signature.clone(),
            calls: symbol.calls.clone(),
            called_by: Vec::new(), // Populated from dependency graph
            imports: Vec::new(),   // Populated from file-level
            tags: self.generate_tags(symbol),
            lines_of_code: self.count_lines_of_code(content),
            max_nesting_depth: self.calculate_nesting_depth(content),
        }
    }

    /// Count lines of code (excluding blank lines and simple comments)
    fn count_lines_of_code(&self, content: &str) -> u32 {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                // Skip blank lines and pure comment lines
                !trimmed.is_empty()
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("/*")
                    && !trimmed.starts_with('*')
            })
            .count() as u32
    }

    /// Calculate maximum nesting depth based on brace/indent patterns
    ///
    /// For brace-based languages (Rust, JS, Go, etc.): counts {}, (), [] nesting
    /// For indentation-based languages (Python, Haskell): counts indent levels
    fn calculate_nesting_depth(&self, content: &str) -> u32 {
        // First try brace-based nesting
        let brace_depth = self.calculate_brace_depth(content);

        // If no braces found (or very few), calculate indentation-based depth
        // This handles Python, Haskell, and other whitespace-sensitive languages
        if brace_depth <= 1 {
            let indent_depth = self.calculate_indent_depth(content);
            // Use the larger of the two (some Python code also uses brackets)
            brace_depth.max(indent_depth)
        } else {
            brace_depth
        }
    }

    /// Calculate nesting depth based on brace pairs
    fn calculate_brace_depth(&self, content: &str) -> u32 {
        let mut max_depth = 0u32;
        let mut current_depth = 0i32;

        for ch in content.chars() {
            match ch {
                '{' | '(' | '[' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth as u32);
                },
                '}' | ')' | ']' => {
                    current_depth = (current_depth - 1).max(0);
                },
                _ => {},
            }
        }

        max_depth
    }

    /// Calculate nesting depth based on indentation levels
    /// Used for Python, Haskell, and other whitespace-sensitive languages
    fn calculate_indent_depth(&self, content: &str) -> u32 {
        let mut max_depth = 0u32;
        let mut base_indent: Option<usize> = None;

        for line in content.lines() {
            // Skip empty lines and comment-only lines
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("--") {
                continue;
            }

            // Count leading whitespace (spaces or tabs)
            let leading_spaces = line.len() - line.trim_start().len();

            // Set base indent from first non-empty line
            if base_indent.is_none() {
                base_indent = Some(leading_spaces);
            }

            // Calculate relative depth (assuming 4-space or 1-tab = 1 level)
            let base = base_indent.unwrap_or(0);
            if leading_spaces >= base {
                let relative_indent = leading_spaces - base;
                // Normalize: assume 4 spaces or 1 tab per level
                let depth = (relative_indent / 4).max(relative_indent / 2) as u32;
                max_depth = max_depth.max(depth + 1); // +1 because base level is 1
            }
        }

        max_depth
    }

    /// Auto-generate semantic tags for improved RAG retrieval
    ///
    /// Tags are generated based on symbol names, signatures, and common patterns.
    /// These help with semantic search and filtering in vector databases.
    fn generate_tags(&self, symbol: &Symbol) -> Vec<String> {
        let mut tags = Vec::new();
        let signature = symbol.signature.as_deref().unwrap_or("");
        let name_lower = symbol.name.to_lowercase();

        // Async/concurrent code
        // Rust: async/await, JavaScript: async/await, Python: async/await
        // Kotlin: suspend, Go: goroutines (go keyword detected in signature)
        if signature.contains("async")
            || signature.contains("await")
            || signature.contains("suspend")
        // Kotlin coroutines
        {
            tags.push("async".to_owned());
        }
        if name_lower.contains("thread")
            || name_lower.contains("mutex")
            || name_lower.contains("lock")
            || name_lower.contains("spawn")
            || name_lower.contains("parallel")
            || name_lower.contains("goroutine")
            || name_lower.contains("channel")
            || signature.contains("Mutex")
            || signature.contains("RwLock")
            || signature.contains("Arc")
            || signature.contains("chan ")      // Go channels
            || signature.contains("<-chan")     // Go receive-only channel
            || signature.contains("chan<-")     // Go send-only channel
            || signature.contains("sync.")      // Go sync package
            || signature.contains("WaitGroup")
        // Go WaitGroup
        {
            tags.push("concurrency".to_owned());
        }

        // Security-related
        if name_lower.contains("password")
            || name_lower.contains("token")
            || name_lower.contains("secret")
            || name_lower.contains("auth")
            || name_lower.contains("crypt")
            || name_lower.contains("hash")
            || name_lower.contains("permission")
            || signature.contains("password")
            || signature.contains("token")
            || signature.contains("secret")
        {
            tags.push("security".to_owned());
        }

        // Error handling
        if signature.contains("Error")
            || signature.contains("Result")
            || name_lower.contains("error")
            || name_lower.contains("exception")
            || name_lower.contains("panic")
            || name_lower.contains("unwrap")
        {
            tags.push("error-handling".to_owned());
        }

        // Database
        if name_lower.contains("query")
            || name_lower.contains("sql")
            || name_lower.contains("database")
            || name_lower.contains("db_")
            || name_lower.starts_with("db")
            || name_lower.contains("repository")
            || name_lower.contains("transaction")
        {
            tags.push("database".to_owned());
        }

        // HTTP/API
        if name_lower.contains("http")
            || name_lower.contains("request")
            || name_lower.contains("response")
            || name_lower.contains("endpoint")
            || name_lower.contains("route")
            || name_lower.contains("handler")
            || name_lower.contains("middleware")
        {
            tags.push("http".to_owned());
        }

        // CLI/Commands
        if name_lower.contains("command")
            || name_lower.contains("cli")
            || name_lower.contains("arg")
            || name_lower.contains("flag")
            || name_lower.contains("option")
            || name_lower.contains("subcommand")
        {
            tags.push("cli".to_owned());
        }

        // Configuration
        if name_lower.contains("config")
            || name_lower.contains("setting")
            || name_lower.contains("preference")
            || name_lower.contains("option")
            || name_lower.contains("env")
        {
            tags.push("config".to_owned());
        }

        // Logging
        if name_lower.contains("log")
            || name_lower.contains("trace")
            || name_lower.contains("debug")
            || name_lower.contains("warn")
            || name_lower.contains("info")
            || name_lower.contains("metric")
        {
            tags.push("logging".to_owned());
        }

        // Caching
        if name_lower.contains("cache")
            || name_lower.contains("memoize")
            || name_lower.contains("invalidate")
        {
            tags.push("cache".to_owned());
        }

        // Validation
        if name_lower.contains("valid")
            || name_lower.contains("check")
            || name_lower.contains("verify")
            || name_lower.contains("assert")
            || name_lower.contains("sanitize")
        {
            tags.push("validation".to_owned());
        }

        // Serialization
        if name_lower.contains("serial")
            || name_lower.contains("deserial")
            || name_lower.contains("json")
            || name_lower.contains("xml")
            || name_lower.contains("yaml")
            || name_lower.contains("toml")
            || name_lower.contains("encode")
            || name_lower.contains("decode")
            || name_lower.contains("parse")
            || name_lower.contains("format")
        {
            tags.push("serialization".to_owned());
        }

        // File I/O
        if name_lower.contains("file")
            || name_lower.contains("read")
            || name_lower.contains("write")
            || name_lower.contains("path")
            || name_lower.contains("dir")
            || name_lower.contains("fs")
            || name_lower.contains("io")
        {
            tags.push("io".to_owned());
        }

        // Networking
        if name_lower.contains("socket")
            || name_lower.contains("connect")
            || name_lower.contains("network")
            || name_lower.contains("tcp")
            || name_lower.contains("udp")
            || name_lower.contains("client")
            || name_lower.contains("server")
        {
            tags.push("network".to_owned());
        }

        // Initialization/Setup
        if name_lower == "new"
            || name_lower == "init"
            || name_lower == "setup"
            || name_lower == "create"
            || name_lower.starts_with("new_")
            || name_lower.starts_with("init_")
            || name_lower.starts_with("create_")
            || name_lower.ends_with("_new")
        {
            tags.push("init".to_owned());
        }

        // Cleanup/Teardown
        if name_lower.contains("cleanup")
            || name_lower.contains("teardown")
            || name_lower.contains("close")
            || name_lower.contains("dispose")
            || name_lower.contains("shutdown")
            || name_lower == "drop"
        {
            tags.push("cleanup".to_owned());
        }

        // Test
        if symbol.name.starts_with("test_")
            || symbol.name.ends_with("_test")
            || symbol.name.contains("Test")
            || name_lower.contains("mock")
            || name_lower.contains("stub")
            || name_lower.contains("fixture")
        {
            tags.push("test".to_owned());
        }

        // Deprecated (check signature for attributes)
        if signature.contains("deprecated") || signature.contains("Deprecated") {
            tags.push("deprecated".to_owned());
        }

        // Public API marker (useful for filtering to public interfaces)
        if signature.starts_with("pub fn")
            || signature.starts_with("pub async fn")
            || signature.starts_with("export")
        {
            tags.push("public-api".to_owned());
        }

        // Machine Learning / AI
        if name_lower.contains("model")
            || name_lower.contains("train")
            || name_lower.contains("predict")
            || name_lower.contains("inference")
            || name_lower.contains("neural")
            || name_lower.contains("embedding")
            || name_lower.contains("classifier")
            || name_lower.contains("regressor")
            || name_lower.contains("optimizer")
            || name_lower.contains("loss")
            || name_lower.contains("gradient")
            || name_lower.contains("backprop")
            || name_lower.contains("forward")
            || name_lower.contains("layer")
            || name_lower.contains("activation")
            || name_lower.contains("weight")
            || name_lower.contains("bias")
            || name_lower.contains("epoch")
            || name_lower.contains("batch")
            || signature.contains("torch")
            || signature.contains("tensorflow")
            || signature.contains("keras")
            || signature.contains("sklearn")
            || signature.contains("nn.")
            || signature.contains("nn::")
        {
            tags.push("ml".to_owned());
        }

        // Data Science / Data Processing
        if name_lower.contains("dataframe")
            || name_lower.contains("dataset")
            || name_lower.contains("tensor")
            || name_lower.contains("numpy")
            || name_lower.contains("pandas")
            || name_lower.contains("array")
            || name_lower.contains("matrix")
            || name_lower.contains("vector")
            || name_lower.contains("feature")
            || name_lower.contains("preprocess")
            || name_lower.contains("normalize")
            || name_lower.contains("transform")
            || name_lower.contains("pipeline")
            || name_lower.contains("etl")
            || name_lower.contains("aggregate")
            || name_lower.contains("groupby")
            || name_lower.contains("pivot")
            || signature.contains("pd.")
            || signature.contains("np.")
            || signature.contains("DataFrame")
            || signature.contains("ndarray")
        {
            tags.push("data-science".to_owned());
        }

        tags
    }

    /// Compute fully qualified name for a symbol
    ///
    /// Format: `file_path::parent::symbol_name`
    /// - file_path: Relative path with extension stripped and slashes replaced with ::
    /// - parent: Parent symbol name if any (e.g., class for a method)
    /// - symbol_name: The symbol's own name
    fn compute_fqn(&self, file: &str, symbol: &Symbol) -> String {
        // Convert file path to module-like format: src/auth/login.rs -> src::auth::login
        let module_path = file
            .strip_suffix(".rs")
            .or_else(|| file.strip_suffix(".py"))
            .or_else(|| file.strip_suffix(".ts"))
            .or_else(|| file.strip_suffix(".tsx"))
            .or_else(|| file.strip_suffix(".js"))
            .or_else(|| file.strip_suffix(".jsx"))
            .or_else(|| file.strip_suffix(".go"))
            .or_else(|| file.strip_suffix(".java"))
            .or_else(|| file.strip_suffix(".c"))
            .or_else(|| file.strip_suffix(".cpp"))
            .or_else(|| file.strip_suffix(".h"))
            .or_else(|| file.strip_suffix(".hpp"))
            .or_else(|| file.strip_suffix(".rb"))
            .or_else(|| file.strip_suffix(".php"))
            .or_else(|| file.strip_suffix(".cs"))
            .or_else(|| file.strip_suffix(".swift"))
            .or_else(|| file.strip_suffix(".kt"))
            .or_else(|| file.strip_suffix(".scala"))
            .unwrap_or(file)
            .replace(['\\', '/'], "::"); // Normalize path separators

        if let Some(ref parent) = symbol.parent {
            format!("{}::{}::{}", module_path, parent, symbol.name)
        } else {
            format!("{}::{}", module_path, symbol.name)
        }
    }

    /// Detect if code is test code
    fn is_test_code(&self, path: &Path, symbol: &Symbol) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Path-based detection
        if path_str.contains("test") || path_str.contains("spec") || path_str.contains("__tests__")
        {
            return true;
        }

        // Symbol-based detection
        let name = symbol.name.to_lowercase();
        if name.starts_with("test_") || name.ends_with("_test") || name.contains("_test_") {
            return true;
        }

        false
    }

    /// Validate repository path
    fn validate_repo_path(&self, path: &Path) -> Result<PathBuf, EmbedError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure it's a directory
        if !canonical.is_dir() {
            return Err(EmbedError::NotADirectory { path: path.to_path_buf() });
        }

        Ok(canonical)
    }

    /// Get safe relative path, validate no traversal
    fn safe_relative_path(&self, path: &Path, repo_root: &Path) -> Result<String, EmbedError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure path is within repo root
        if !canonical.starts_with(repo_root) {
            return Err(EmbedError::PathTraversal {
                path: canonical,
                repo_root: repo_root.to_path_buf(),
            });
        }

        // Return relative path with forward slashes (cross-platform)
        Ok(canonical
            .strip_prefix(repo_root)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .replace('\\', "/"))
    }

    /// Discover all files in repository
    fn discover_files(&self, repo_root: &Path) -> Result<Vec<PathBuf>, EmbedError> {
        use glob::Pattern;
        use ignore::WalkBuilder;

        let mut files = Vec::new();

        // Compile and validate include patterns (fail fast on invalid patterns)
        let mut include_patterns = Vec::new();
        for pattern_str in &self.settings.include_patterns {
            match Pattern::new(pattern_str) {
                Ok(pattern) => include_patterns.push(pattern),
                Err(e) => {
                    return Err(EmbedError::InvalidPattern {
                        pattern: pattern_str.clone(),
                        reason: e.to_string(),
                    });
                },
            }
        }

        // Compile and validate exclude patterns (fail fast on invalid patterns)
        let mut exclude_patterns = Vec::new();
        for pattern_str in &self.settings.exclude_patterns {
            match Pattern::new(pattern_str) {
                Ok(pattern) => exclude_patterns.push(pattern),
                Err(e) => {
                    return Err(EmbedError::InvalidPattern {
                        pattern: pattern_str.clone(),
                        reason: e.to_string(),
                    });
                },
            }
        }

        let walker = WalkBuilder::new(repo_root)
            .hidden(false) // Include hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(true)
            .git_exclude(true)
            .follow_links(false) // Security: Don't follow symlinks to prevent escaping repo
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| EmbedError::IoError {
                path: repo_root.to_path_buf(),
                source: std::io::Error::other(e.to_string()),
            })?;

            let path = entry.path();

            // Only process files
            if !path.is_file() {
                continue;
            }

            // Get relative path for pattern matching
            let relative_path = path
                .strip_prefix(repo_root)
                .unwrap_or(path)
                .to_string_lossy();

            // Check include patterns (if any, file must match at least one)
            if !include_patterns.is_empty()
                && !include_patterns.iter().any(|p| p.matches(&relative_path))
            {
                continue;
            }

            // Check exclude patterns (if any match, skip file)
            if exclude_patterns.iter().any(|p| p.matches(&relative_path)) {
                continue;
            }

            // Skip test files unless include_tests is true
            if !self.settings.include_tests && self.is_test_file(path) {
                continue;
            }

            // Only process supported languages
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e,
                None => continue,
            };
            if Language::from_extension(ext).is_none() {
                continue;
            }

            files.push(path.to_path_buf());
        }

        Ok(files)
    }

    /// Check if a file is a test file based on path patterns
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Common test directory patterns (handle both Unix and Windows separators)
        if path_str.contains("/tests/")
            || path_str.contains("\\tests\\")
            || path_str.contains("/test/")
            || path_str.contains("\\test\\")
            || path_str.contains("/__tests__/")
            || path_str.contains("\\__tests__\\")
            || path_str.contains("/spec/")
            || path_str.contains("\\spec\\")
        {
            return true;
        }

        // Common test file patterns
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        filename.starts_with("test_")
            || filename.ends_with("_test.rs")
            || filename.ends_with("_test.py")
            || filename.ends_with("_test.go")
            || filename.ends_with(".test.ts")
            || filename.ends_with(".test.js")
            || filename.ends_with(".test.tsx")
            || filename.ends_with(".test.jsx")
            || filename.ends_with(".spec.ts")
            || filename.ends_with(".spec.js")
            || filename.ends_with("_spec.rb")
    }

    /// Detect language from file path
    fn detect_language(&self, path: &Path) -> String {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Language::from_extension)
            .map_or_else(|| "unknown".to_owned(), |l| l.display_name().to_owned())
    }

    /// Parse token model string
    fn parse_token_model(&self, model: &str) -> TokenModel {
        TokenModel::from_model_name(model).unwrap_or(TokenModel::Claude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::progress::QuietProgress;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_chunker_creation() {
        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();
        let chunker = EmbedChunker::new(settings, limits);
        assert!(chunker.settings.max_tokens > 0);
    }

    #[test]
    fn test_chunk_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let rust_code = r#"
/// A test function
fn hello() {
    println!("Hello, world!");
}

fn goodbye() {
    println!("Goodbye!");
}
"#;
        create_test_file(temp_dir.path(), "test.rs", rust_code);

        let settings = EmbedSettings::default();
        let chunker = EmbedChunker::with_defaults(settings);
        let progress = QuietProgress;

        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Should have at least 2 chunks (hello and goodbye functions)
        assert!(!chunks.is_empty());

        // Check that chunks are sorted
        for i in 1..chunks.len() {
            assert!(chunks[i - 1].source.file <= chunks[i].source.file);
        }
    }

    #[test]
    fn test_determinism() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "a.rs", "fn foo() {}");
        create_test_file(temp_dir.path(), "b.rs", "fn bar() {}");

        let settings = EmbedSettings::default();
        let progress = QuietProgress;

        let results: Vec<Vec<EmbedChunk>> = (0..3)
            .map(|_| {
                let chunker = EmbedChunker::with_defaults(settings.clone());
                chunker
                    .chunk_repository(temp_dir.path(), &progress)
                    .unwrap()
            })
            .collect();

        // All runs should produce identical results
        for i in 1..results.len() {
            assert_eq!(results[0].len(), results[i].len());
            for j in 0..results[0].len() {
                assert_eq!(results[0][j].id, results[i][j].id);
            }
        }
    }

    #[test]
    fn test_file_too_large() {
        let temp_dir = TempDir::new().unwrap();
        // Create a file larger than 100 bytes
        let large_content = "x".repeat(200);
        create_test_file(temp_dir.path(), "large.rs", &large_content);

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default().with_max_file_size(100);
        let chunker = EmbedChunker::new(settings, limits);
        let progress = QuietProgress;

        // Should skip the file (warning) and return empty
        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        // The chunker should produce an error about no chunks generated
        // because the only file was skipped
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let settings = EmbedSettings::default();
        let chunker = EmbedChunker::with_defaults(settings);
        let progress = QuietProgress;

        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        assert!(matches!(result, Err(EmbedError::NoChunksGenerated { .. })));
    }

    #[test]
    fn test_language_detection() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        assert_eq!(chunker.detect_language(Path::new("test.rs")), "Rust");
        assert_eq!(chunker.detect_language(Path::new("test.py")), "Python");
        assert_eq!(chunker.detect_language(Path::new("test.unknown")), "unknown");
    }

    #[test]
    fn test_is_test_code() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let test_symbol = Symbol::new("test_foo", crate::types::SymbolKind::Function);
        assert!(chunker.is_test_code(Path::new("foo.rs"), &test_symbol));

        let normal_symbol = Symbol::new("foo", crate::types::SymbolKind::Function);
        assert!(!chunker.is_test_code(Path::new("src/lib.rs"), &normal_symbol));

        // Test path-based detection
        assert!(chunker.is_test_code(Path::new("tests/test_foo.rs"), &normal_symbol));
    }

    #[test]
    fn test_generate_tags() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("authenticate_user", crate::types::SymbolKind::Function);
        symbol.signature = Some("async fn authenticate_user(password: &str)".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"async".to_owned()));
        assert!(tags.contains(&"security".to_owned()));
    }

    #[test]
    fn test_generate_tags_kotlin_suspend() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("fetchData", crate::types::SymbolKind::Function);
        symbol.signature = Some("suspend fun fetchData(): Result<Data>".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"async".to_owned()), "Kotlin suspend should be tagged as async");
    }

    #[test]
    fn test_generate_tags_go_concurrency() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("processMessages", crate::types::SymbolKind::Function);
        symbol.signature = Some("func processMessages(ch chan string)".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(
            tags.contains(&"concurrency".to_owned()),
            "Go channels should be tagged as concurrency"
        );
    }

    #[test]
    fn test_generate_tags_ml() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test ML training function
        let mut symbol = Symbol::new("train_model", crate::types::SymbolKind::Function);
        symbol.signature = Some("def train_model(epochs: int, batch_size: int)".to_owned());
        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"ml".to_owned()), "train_model should be tagged as ml");

        // Test neural network layer
        let mut symbol2 = Symbol::new("forward_pass", crate::types::SymbolKind::Function);
        symbol2.signature = Some("def forward_pass(self, x: torch.Tensor)".to_owned());
        let tags2 = chunker.generate_tags(&symbol2);
        assert!(
            tags2.contains(&"ml".to_owned()),
            "torch.Tensor in signature should be tagged as ml"
        );

        // Test classifier
        let mut symbol3 = Symbol::new("ImageClassifier", crate::types::SymbolKind::Class);
        symbol3.signature = Some("class ImageClassifier(nn.Module)".to_owned());
        let tags3 = chunker.generate_tags(&symbol3);
        assert!(tags3.contains(&"ml".to_owned()), "nn.Module should be tagged as ml");
    }

    #[test]
    fn test_generate_tags_data_science() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test DataFrame operation
        let mut symbol = Symbol::new("preprocess_dataframe", crate::types::SymbolKind::Function);
        symbol.signature = Some("def preprocess_dataframe(df: pd.DataFrame)".to_owned());
        let tags = chunker.generate_tags(&symbol);
        assert!(
            tags.contains(&"data-science".to_owned()),
            "DataFrame should be tagged as data-science"
        );

        // Test numpy array
        let mut symbol2 = Symbol::new("normalize_array", crate::types::SymbolKind::Function);
        symbol2.signature = Some("def normalize_array(arr: np.ndarray)".to_owned());
        let tags2 = chunker.generate_tags(&symbol2);
        assert!(
            tags2.contains(&"data-science".to_owned()),
            "np.ndarray should be tagged as data-science"
        );

        // Test ETL pipeline
        let symbol3 = Symbol::new("run_etl_pipeline", crate::types::SymbolKind::Function);
        let tags3 = chunker.generate_tags(&symbol3);
        assert!(tags3.contains(&"data-science".to_owned()), "etl should be tagged as data-science");
    }

    #[test]
    fn test_brace_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test simple nesting
        let code = "fn foo() { if x { if y { } } }";
        assert_eq!(chunker.calculate_brace_depth(code), 3);

        // Test no nesting
        let flat = "let x = 1;";
        assert_eq!(chunker.calculate_brace_depth(flat), 0);

        // Test deep nesting with all bracket types
        let deep = "fn f() { let a = vec![HashMap::new()]; }";
        assert!(chunker.calculate_brace_depth(deep) >= 2);
    }

    #[test]
    fn test_indent_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test Python-style indentation (4 spaces per level)
        let python_code = r#"
def foo():
    if x:
        if y:
            do_something()
        else:
            other()
"#;
        let depth = chunker.calculate_indent_depth(python_code);
        assert!(depth >= 3, "Should detect indentation nesting, got {}", depth);

        // Test flat code
        let flat = "x = 1\ny = 2\n";
        assert!(chunker.calculate_indent_depth(flat) <= 1);
    }

    #[test]
    fn test_combined_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Brace-based should win for languages like Rust
        let rust_code = "fn foo() { if x { match y { A => {}, B => {} } } }";
        let depth = chunker.calculate_nesting_depth(rust_code);
        assert!(depth >= 3, "Should use brace depth for Rust-like code");

        // Indent-based should win for Python-like code (few braces)
        let python_code = "def foo():\n    if x:\n        y()\n";
        let depth = chunker.calculate_nesting_depth(python_code);
        assert!(depth >= 1, "Should use indent depth for Python-like code");
    }

    #[test]
    fn test_lines_of_code() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let code = r#"
// This is a comment
fn foo() {
    let x = 1;

    // Another comment
    let y = 2;
}
"#;
        let loc = chunker.count_lines_of_code(code);
        // Should count: fn foo() {, let x = 1;, let y = 2;, }
        // Should skip: empty lines and comments
        assert!((4..=5).contains(&loc), "LOC should be ~4, got {}", loc);
    }

    #[test]
    fn test_line_too_long_error() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with a very long line (simulating minified code)
        let long_line = "x".repeat(50_000);
        let content = format!("fn foo() {{ {} }}", long_line);
        create_test_file(temp_dir.path(), "minified.rs", &content);

        let settings = EmbedSettings::default();
        // Use strict line length limit
        let limits = ResourceLimits::default().with_max_line_length(10_000);
        let chunker = EmbedChunker::new(settings, limits);
        let progress = QuietProgress;

        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        // Should fail due to line too long
        assert!(result.is_err(), "Should reject files with very long lines");
    }

    #[test]
    fn test_hierarchical_chunking_integration() {
        let temp_dir = TempDir::new().unwrap();

        // Create a Rust file with a struct that has multiple methods
        let rust_code = r#"
/// A user account
pub struct User {
    pub name: String,
    pub email: String,
}

impl User {
    /// Create a new user
    pub fn new(name: String, email: String) -> Self {
        Self { name, email }
    }

    /// Get the user's display name
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Validate the user's email
    pub fn validate_email(&self) -> bool {
        self.email.contains('@')
    }
}
"#;
        create_test_file(temp_dir.path(), "user.rs", rust_code);

        // Test WITHOUT hierarchy
        let settings_no_hierarchy = EmbedSettings { enable_hierarchy: false, ..Default::default() };
        let chunker_no_hierarchy = EmbedChunker::with_defaults(settings_no_hierarchy);
        let progress = QuietProgress;
        let chunks_no_hierarchy = chunker_no_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Test WITH hierarchy
        let settings_with_hierarchy = EmbedSettings {
            enable_hierarchy: true,
            hierarchy_min_children: 2,
            ..Default::default()
        };
        let chunker_with_hierarchy = EmbedChunker::with_defaults(settings_with_hierarchy);
        let chunks_with_hierarchy = chunker_with_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Hierarchy should produce more chunks (original + summaries)
        assert!(
            chunks_with_hierarchy.len() >= chunks_no_hierarchy.len(),
            "Hierarchy should produce at least as many chunks: {} vs {}",
            chunks_with_hierarchy.len(),
            chunks_no_hierarchy.len()
        );

        // Check for ContainerSummary chunks when hierarchy is enabled
        let summary_chunks: Vec<_> = chunks_with_hierarchy
            .iter()
            .filter(|c| matches!(c.kind, ChunkKind::Module)) // Summary chunks use Module kind
            .collect();

        // If we have container types with enough children, we should have summaries
        // Note: This depends on the parser correctly identifying struct + impl methods
        if !summary_chunks.is_empty() {
            // Summary chunks should have content referencing children
            for summary in &summary_chunks {
                assert!(!summary.content.is_empty(), "Summary chunk should have content");
            }
        }

        // Verify determinism with hierarchy enabled
        let chunks_with_hierarchy_2 = chunker_with_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();
        assert_eq!(
            chunks_with_hierarchy.len(),
            chunks_with_hierarchy_2.len(),
            "Hierarchical chunking should be deterministic"
        );
        for (c1, c2) in chunks_with_hierarchy
            .iter()
            .zip(chunks_with_hierarchy_2.iter())
        {
            assert_eq!(c1.id, c2.id, "Chunk IDs should be identical across runs");
        }
    }
}
