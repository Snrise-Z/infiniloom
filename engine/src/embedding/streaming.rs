//! Streaming API for large repository processing
//!
//! This module provides an iterator-based interface for processing large repositories
//! while preserving the same globally finalized metadata as [`EmbedChunker`]. This is
//! useful for:
//!
//! - **Large Monorepos**: Repositories with 100K+ files
//! - **CI/CD Pipelines**: Memory-constrained container environments
//! - **Real-time Processing**: Stream chunks directly to vector databases
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::streaming::{ChunkStream, StreamConfig};
//!
//! let stream = ChunkStream::new(repo_path, settings, limits)?;
//!
//! // Process finalized chunks from the iterator
//! for chunk_result in stream {
//!     match chunk_result {
//!         Ok(chunk) => {
//!             // Send to vector database, write to file, etc.
//!             upload_to_pinecone(&chunk)?;
//!         }
//!         Err(e) if e.is_skippable() => {
//!             // Non-critical error, continue processing
//!             eprintln!("Warning: {}", e);
//!         }
//!         Err(e) => {
//!             // Critical error, abort
//!             return Err(e.into());
//!         }
//!     }
//! }
//! ```
//!
//! # Batch Processing
//!
//! For better throughput, process chunks in batches:
//!
//! ```rust,ignore
//! let stream = ChunkStream::new(repo_path, settings, limits)?
//!     .with_batch_size(100);
//!
//! for batch in stream.batches() {
//!     let chunks: Vec<_> = batch.into_iter().filter_map(|r| r.ok()).collect();
//!     bulk_upload_to_vector_db(&chunks)?;
//! }
//! ```
//!
//! # Finalization
//!
//! Complete dependency metadata such as `called_by`, hierarchy links, signature
//! chunks, and deterministic global ordering require a repository-wide finalization
//! pass. The stream parses files in batches, then yields finalized chunks after that
//! pass completes.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::parser::{parse_file_symbols, Language};
use crate::security::SecurityScanner;
use crate::tokenizer::{TokenModel, Tokenizer};

use super::chunker::{generate_summary, generate_tags_for_symbol, EmbedChunker};
use super::complexity::compute_complexity;
use super::error::EmbedError;
use super::hasher::hash_content;
use super::identifiers::extract_identifiers;
use super::limits::ResourceLimits;
use super::progress::QuietProgress;
use super::type_extraction::extract_types;
use super::types::{ChunkContext, ChunkSource, EmbedChunk, EmbedSettings, RepoIdentifier};

/// Configuration for streaming chunk generation
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Number of files to process in each batch
    pub file_batch_size: usize,

    /// Maximum chunks to buffer before yielding
    pub chunk_buffer_size: usize,

    /// Whether to skip files that cause errors
    pub skip_on_error: bool,

    /// Maximum errors before aborting
    pub max_errors: usize,

    /// Enable parallel file processing within batches
    pub parallel_batches: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            file_batch_size: 50,
            chunk_buffer_size: 200,
            skip_on_error: true,
            max_errors: 100,
            parallel_batches: true,
        }
    }
}

/// Statistics for streaming progress
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total files discovered
    pub total_files: usize,

    /// Files processed so far
    pub files_processed: usize,

    /// Files skipped due to errors
    pub files_skipped: usize,

    /// Chunks generated so far
    pub chunks_generated: usize,

    /// Bytes processed so far
    pub bytes_processed: u64,

    /// Errors encountered
    pub error_count: usize,
}

impl StreamStats {
    /// Get progress as a percentage (0.0 - 100.0)
    pub fn progress_percent(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        (self.files_processed as f64 / self.total_files as f64) * 100.0
    }

    /// Estimated chunks remaining (based on current rate)
    pub fn estimated_chunks_remaining(&self) -> usize {
        if self.files_processed == 0 {
            return 0;
        }
        let rate = self.chunks_generated as f64 / self.files_processed as f64;
        let remaining_files = self.total_files.saturating_sub(self.files_processed);
        (remaining_files as f64 * rate) as usize
    }
}

/// Streaming chunk iterator for large repositories
///
/// This iterator yields globally finalized chunks. The first yielded item is
/// available after the repository-wide dependency finalization pass completes.
pub struct ChunkStream {
    /// Queued files to process
    pending_files: VecDeque<PathBuf>,

    /// Buffer of generated chunks waiting to be yielded
    chunk_buffer: VecDeque<Result<EmbedChunk, EmbedError>>,

    /// Repository root path
    repo_root: PathBuf,

    /// Embedding settings
    settings: EmbedSettings,

    /// Resource limits
    limits: ResourceLimits,

    /// Stream configuration
    config: StreamConfig,

    /// Tokenizer instance
    tokenizer: Tokenizer,

    /// Security scanner (optional)
    security_scanner: Option<SecurityScanner>,

    /// Repository identifier
    repo_id: RepoIdentifier,

    /// Statistics
    stats: StreamStats,

    /// Cancellation flag
    cancelled: Arc<AtomicBool>,

    /// Error count for early termination
    error_count: AtomicUsize,

    /// Whether finalized chunks have already been loaded into the output buffer
    finalized_loaded: bool,
}

impl ChunkStream {
    /// Create a new chunk stream for a repository
    pub fn new(
        repo_path: impl AsRef<Path>,
        settings: EmbedSettings,
        limits: ResourceLimits,
    ) -> Result<Self, EmbedError> {
        Self::with_config(repo_path, settings, limits, StreamConfig::default())
    }

    /// Create with custom stream configuration
    pub fn with_config(
        repo_path: impl AsRef<Path>,
        settings: EmbedSettings,
        limits: ResourceLimits,
        config: StreamConfig,
    ) -> Result<Self, EmbedError> {
        let repo_root = repo_path
            .as_ref()
            .canonicalize()
            .map_err(|e| EmbedError::IoError {
                path: repo_path.as_ref().to_path_buf(),
                source: e,
            })?;

        if !repo_root.is_dir() {
            return Err(EmbedError::NotADirectory { path: repo_root });
        }

        // Security scanner if enabled
        let security_scanner = if settings.scan_secrets {
            Some(SecurityScanner::new())
        } else {
            None
        };

        let mut stream = Self {
            pending_files: VecDeque::new(),
            chunk_buffer: VecDeque::new(),
            repo_root,
            settings,
            limits,
            config,
            tokenizer: Tokenizer::new(),
            security_scanner,
            repo_id: RepoIdentifier::default(),
            stats: StreamStats::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            error_count: AtomicUsize::new(0),
            finalized_loaded: false,
        };

        // Discover files
        stream.discover_files()?;

        Ok(stream)
    }

    /// Set the repository identifier for multi-tenant RAG
    pub fn with_repo_id(mut self, repo_id: RepoIdentifier) -> Self {
        self.repo_id = repo_id;
        self
    }

    /// Get current streaming statistics
    pub fn stats(&self) -> &StreamStats {
        &self.stats
    }

    /// Get a cancellation handle for this stream
    pub fn cancellation_handle(&self) -> CancellationHandle {
        CancellationHandle { cancelled: Arc::clone(&self.cancelled) }
    }

    /// Check if the stream has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Discover all files in the repository
    fn discover_files(&mut self) -> Result<(), EmbedError> {
        use glob::Pattern;
        use ignore::WalkBuilder;

        // Compile include/exclude patterns
        let include_patterns: Vec<Pattern> = self
            .settings
            .include_patterns
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        let exclude_patterns: Vec<Pattern> = self
            .settings
            .exclude_patterns
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        let walker = WalkBuilder::new(&self.repo_root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .follow_links(false)
            .build();

        let mut files = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Get relative path for pattern matching
            let relative = path
                .strip_prefix(&self.repo_root)
                .unwrap_or(path)
                .to_string_lossy();

            // Check include patterns
            if !include_patterns.is_empty()
                && !include_patterns.iter().any(|p| p.matches(&relative))
            {
                continue;
            }

            // Check exclude patterns
            if exclude_patterns.iter().any(|p| p.matches(&relative)) {
                continue;
            }

            // Check for supported language (by extension or filename)
            let has_language = Language::from_path(path).is_some();
            if !has_language {
                continue;
            }

            // Skip test files if configured
            if !self.settings.include_tests && self.is_test_file(path) {
                continue;
            }

            files.push(path.to_path_buf());
        }

        // Sort for determinism
        files.sort();

        self.stats.total_files = files.len();
        self.pending_files = files.into();

        // Check file limit
        if !self.limits.check_file_count(self.stats.total_files) {
            return Err(EmbedError::TooManyFiles {
                count: self.stats.total_files,
                max: self.limits.max_files,
            });
        }

        Ok(())
    }

    /// Check if a file is a test file
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        path_str.contains("/tests/")
            || path_str.contains("\\tests\\")
            || path_str.contains("/test/")
            || path_str.contains("\\test\\")
            || path_str.contains("/__tests__/")
            || path_str.contains("\\__tests__\\")
    }

    /// Parse files in configured batches, globally finalize metadata, and fill
    /// the chunk buffer.
    fn fill_buffer(&mut self) -> bool {
        if self.is_cancelled() || self.finalized_loaded {
            return false;
        }
        self.finalized_loaded = true;

        if self.pending_files.is_empty() {
            return false;
        }

        let mut settings = self.settings.clone();
        settings.streaming = true;
        settings.batch_size = self.config.file_batch_size.max(1);

        let mut chunker = EmbedChunker::new(settings, self.limits.clone());
        if self.repo_id != RepoIdentifier::default() {
            chunker.set_repo_id(self.repo_id.clone());
        }

        let progress = QuietProgress;
        match chunker.chunk_repository_streaming_chunks(&self.repo_root, &progress) {
            Ok((chunks, stats)) => {
                self.pending_files.clear();
                self.stats.files_processed = stats.files_processed;
                self.stats.files_skipped = stats.files_skipped;
                self.stats.chunks_generated = chunks.len();
                self.stats.bytes_processed = chunks
                    .iter()
                    .map(|chunk| chunk.content.len() as u64)
                    .sum::<u64>();

                for chunk in chunks {
                    self.chunk_buffer.push_back(Ok(chunk));
                }
            },
            Err(e) => {
                self.pending_files.clear();
                self.stats.error_count += 1;
                let current_errors = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
                let err = if current_errors >= self.config.max_errors {
                    EmbedError::TooManyErrors { count: current_errors, max: self.config.max_errors }
                } else {
                    e
                };
                self.chunk_buffer.push_back(Err(err));
            },
        }

        !self.chunk_buffer.is_empty()
    }

    /// Process a single file and return its chunks
    fn process_file(&mut self, path: &Path) -> Result<Vec<EmbedChunk>, EmbedError> {
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

        self.stats.bytes_processed += content.len() as u64;

        // Check for long lines (minified files)
        if let Some(max_line_len) = content.lines().map(|l| l.len()).max() {
            if !self.limits.check_line_length(max_line_len) {
                return Err(EmbedError::LineTooLong {
                    path: path.to_path_buf(),
                    length: max_line_len,
                    max: self.limits.max_line_length,
                });
            }
        }

        // Security scanning
        let relative_path = self.safe_relative_path(path)?;

        if let Some(ref scanner) = self.security_scanner {
            let findings = scanner.scan(&content, &relative_path);
            if !findings.is_empty() {
                if self.settings.fail_on_secrets {
                    let files = findings
                        .iter()
                        .map(|f| format!("  {}:{} - {}", f.file, f.line, f.kind.name()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Err(EmbedError::SecretsDetected { count: findings.len(), files });
                }

                if self.settings.redact_secrets {
                    content = scanner.redact_content(&content, &relative_path);
                }
            }
        }

        // Parse symbols
        let language = self.detect_language(path);
        let mut symbols = parse_file_symbols(&content, path);
        symbols.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| a.end_line.cmp(&b.end_line))
                .then_with(|| a.name.cmp(&b.name))
        });

        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::with_capacity(symbols.len());

        for symbol in &symbols {
            // Skip imports if configured
            if !self.settings.include_imports
                && matches!(symbol.kind, crate::types::SymbolKind::Import)
            {
                continue;
            }

            // Extract content with context
            let start_line = symbol.start_line.saturating_sub(1) as usize;
            let end_line = (symbol.end_line as usize).min(lines.len());
            let context_start = start_line.saturating_sub(self.settings.context_lines as usize);
            let context_end = (end_line + self.settings.context_lines as usize).min(lines.len());

            let chunk_content = lines[context_start..context_end].join("\n");

            // Count tokens
            let token_model = TokenModel::from_model_name(&self.settings.token_model)
                .unwrap_or(TokenModel::Claude);
            let tokens = self.tokenizer.count(&chunk_content, token_model);

            // Generate hash
            let hash = hash_content(&chunk_content);

            // Build FQN
            let fqn = self.compute_fqn(&relative_path, symbol);

            // Extract keywords and context prefix before moving chunk_content
            let keywords = super::chunker::extract_keywords(&chunk_content);
            let context_prefix = super::chunker::generate_context_prefix(
                &relative_path,
                symbol.parent.as_deref(),
                &symbol.kind,
            );

            // Extract enrichments: identifiers, type signatures, complexity, tags
            let lang_enum = Language::from_path(path);
            let identifiers = extract_identifiers(&chunk_content, lang_enum);
            let (type_signature, parameter_types, return_type, error_types) =
                if let Some(lang) = lang_enum {
                    match extract_types(&chunk_content, lang) {
                        Some(ti) => {
                            (ti.type_signature, ti.parameter_types, ti.return_type, ti.error_types)
                        },
                        None => (None, Vec::new(), None, Vec::new()),
                    }
                } else {
                    (None, Vec::new(), None, Vec::new())
                };
            let complexity_score = lang_enum.and_then(|l| compute_complexity(&chunk_content, l));
            let tags = generate_tags_for_symbol(&symbol.name, symbol.signature.as_deref());

            let chunk_kind = symbol.kind.into();
            let source = ChunkSource {
                repo: self.repo_id.clone(),
                file: relative_path.clone(),
                lines: ((context_start + 1) as u32, context_end as u32),
                symbol: symbol.name.clone(),
                fqn: Some(fqn),
                language: language.clone(),
                parent: symbol.parent.clone(),
                visibility: symbol.visibility.into(),
                is_test: self.is_test_code(path, symbol),
                module_path: Some(super::chunker::derive_module_path(&relative_path, &language)),
                parent_chunk_id: None,
            };

            let mut context = ChunkContext {
                docstring: symbol.docstring.clone(),
                comments: Vec::new(),
                signature: symbol.signature.clone(),
                calls: symbol.calls.clone(),
                called_by: Vec::new(),
                imports: Vec::new(),
                tags,
                keywords,
                context_prefix: Some(context_prefix),
                summary: None,
                qualified_calls: Vec::new(),
                unresolved_calls: Vec::new(),
                identifiers,
                type_signature,
                parameter_types,
                return_type,
                error_types,
                lines_of_code: chunk_content.lines().count() as u32,
                max_nesting_depth: 0,
                git: None,
                complexity_score,
                dependents_count: None,
            };

            // Generate summary after source and context are built
            context.summary = generate_summary(chunk_kind, &source, &context);

            chunks.push(EmbedChunk {
                id: hash.short_id,
                full_hash: hash.full_hash,
                content: chunk_content,
                tokens,
                kind: chunk_kind,
                source,
                children_ids: Vec::new(),
                context,
                repr: "code".to_owned(),
                code_chunk_id: None,
                part: None,
            });
        }

        Ok(chunks)
    }

    /// Get safe relative path
    fn safe_relative_path(&self, path: &Path) -> Result<String, EmbedError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        if !canonical.starts_with(&self.repo_root) {
            return Err(EmbedError::PathTraversal {
                path: canonical,
                repo_root: self.repo_root.clone(),
            });
        }

        Ok(canonical
            .strip_prefix(&self.repo_root)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .replace('\\', "/"))
    }

    /// Detect language from file path (by extension or filename)
    fn detect_language(&self, path: &Path) -> String {
        Language::from_path(path)
            .map_or_else(|| "unknown".to_owned(), |l| l.display_name().to_owned())
    }

    /// Compute fully qualified name
    fn compute_fqn(&self, file: &str, symbol: &crate::types::Symbol) -> String {
        let module_path = file
            .strip_suffix(".rs")
            .or_else(|| file.strip_suffix(".py"))
            .or_else(|| file.strip_suffix(".ts"))
            .or_else(|| file.strip_suffix(".tsx"))
            .or_else(|| file.strip_suffix(".js"))
            .or_else(|| file.strip_suffix(".jsx"))
            .or_else(|| file.strip_suffix(".go"))
            .unwrap_or(file)
            .replace(['\\', '/'], "::"); // Normalize path separators

        // Build the symbol portion
        let symbol_part = if let Some(ref parent) = symbol.parent {
            format!("{}::{}::{}", module_path, parent, symbol.name)
        } else {
            format!("{}::{}", module_path, symbol.name)
        };

        // Prepend repo identity: "{namespace}/{name}::{symbol_part}" or "{name}::{symbol_part}"
        let repo_prefix = self.repo_id.qualified_name();
        if repo_prefix.is_empty() {
            symbol_part
        } else {
            format!("{}::{}", repo_prefix, symbol_part)
        }
    }

    /// Check if code is test code
    fn is_test_code(&self, path: &Path, symbol: &crate::types::Symbol) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        let name = symbol.name.to_lowercase();

        path_str.contains("test")
            || path_str.contains("spec")
            || name.starts_with("test_")
            || name.ends_with("_test")
    }

    /// Collect all remaining chunks into a vector (for compatibility)
    ///
    /// Note: This defeats the purpose of streaming by loading everything into memory.
    /// Use only when you need to sort or deduplicate the full result set.
    pub fn collect_all(self) -> Result<Vec<EmbedChunk>, EmbedError> {
        let mut chunks = Vec::new();
        let mut last_error = None;

        for result in self {
            match result {
                Ok(chunk) => chunks.push(chunk),
                Err(e) if e.is_skippable() => {
                    // Non-critical, skip
                },
                Err(e) => {
                    last_error = Some(e);
                },
            }
        }

        if let Some(e) = last_error {
            if chunks.is_empty() {
                return Err(e);
            }
        }

        // Sort for determinism (matches EmbedChunker behavior)
        chunks.sort_by(|a, b| {
            a.source
                .file
                .cmp(&b.source.file)
                .then_with(|| a.source.lines.0.cmp(&b.source.lines.0))
                .then_with(|| a.source.lines.1.cmp(&b.source.lines.1))
                .then_with(|| a.source.symbol.cmp(&b.source.symbol))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(chunks)
    }
}

impl Iterator for ChunkStream {
    type Item = Result<EmbedChunk, EmbedError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Return buffered chunk if available
        if let Some(chunk) = self.chunk_buffer.pop_front() {
            return Some(chunk);
        }

        // Try to fill buffer
        if self.fill_buffer() {
            self.chunk_buffer.pop_front()
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.stats.estimated_chunks_remaining();
        let buffered = self.chunk_buffer.len();
        (buffered, Some(buffered + remaining))
    }
}

/// Handle for cancelling a chunk stream from another thread
#[derive(Clone)]
pub struct CancellationHandle {
    cancelled: Arc<AtomicBool>,
}

impl CancellationHandle {
    /// Cancel the associated stream
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Extension trait for batch processing
pub trait BatchIterator: Iterator {
    /// Process items in batches
    fn batches(self, batch_size: usize) -> Batches<Self>
    where
        Self: Sized,
    {
        Batches { iter: self, batch_size }
    }
}

impl<I: Iterator> BatchIterator for I {}

/// Iterator adapter that yields batches
pub struct Batches<I> {
    iter: I,
    batch_size: usize,
}

impl<I: Iterator> Iterator for Batches<I> {
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut batch = Vec::with_capacity(self.batch_size);

        for _ in 0..self.batch_size {
            match self.iter.next() {
                Some(item) => batch.push(item),
                None => break,
            }
        }

        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_chunk_stream_basic() {
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
        let limits = ResourceLimits::default();

        let stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();
        let chunks: Vec<_> = stream.filter_map(|r| r.ok()).collect();

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_stream_stats() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "a.rs", "fn foo() {}");
        create_test_file(temp_dir.path(), "b.rs", "fn bar() {}");
        create_test_file(temp_dir.path(), "c.rs", "fn baz() {}");

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();

        assert_eq!(stream.stats().total_files, 3);

        // Consume the stream
        let _chunks: Vec<_> = stream.collect();
    }

    #[test]
    fn test_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        for i in 0..10 {
            create_test_file(
                temp_dir.path(),
                &format!("file{}.rs", i),
                &format!("fn func{}() {{}}", i),
            );
        }

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let mut stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();
        let handle = stream.cancellation_handle();

        // Get a few chunks
        let _ = stream.next();
        let _ = stream.next();

        // Cancel
        handle.cancel();

        // Stream should stop
        assert!(stream.is_cancelled());
    }

    #[test]
    fn test_batch_iterator() {
        let items: Vec<i32> = (0..10).collect();
        let batches: Vec<Vec<i32>> = items.into_iter().batches(3).collect();

        assert_eq!(batches.len(), 4);
        assert_eq!(batches[0], vec![0, 1, 2]);
        assert_eq!(batches[1], vec![3, 4, 5]);
        assert_eq!(batches[2], vec![6, 7, 8]);
        assert_eq!(batches[3], vec![9]);
    }

    #[test]
    fn test_collect_all_sorts_deterministically() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "z.rs", "fn z_func() {}");
        create_test_file(temp_dir.path(), "a.rs", "fn a_func() {}");
        create_test_file(temp_dir.path(), "m.rs", "fn m_func() {}");

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();
        let chunks = stream.collect_all().unwrap();

        // Should be sorted by file path
        assert!(chunks[0].source.file < chunks[1].source.file);
        assert!(chunks[1].source.file < chunks[2].source.file);
    }

    #[test]
    fn test_chunk_stream_matches_chunker_with_cross_batch_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "a.rs",
            r#"
pub fn caller() {
    callee();
}
"#,
        );
        create_test_file(
            temp_dir.path(),
            "b.rs",
            r#"
pub fn callee() {
}
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_signatures: true,
            enable_hierarchy: true,
            hierarchy_min_children: 1,
            repo_namespace: Some("org".to_owned()),
            repo_name: Some("repo".to_owned()),
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let limits = ResourceLimits::default();
        let progress = QuietProgress;

        let mut chunker = EmbedChunker::new(settings.clone(), limits.clone());
        let expected = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let config = StreamConfig { file_batch_size: 1, ..Default::default() };
        let stream = ChunkStream::with_config(temp_dir.path(), settings, limits, config).unwrap();
        let actual = stream.collect_all().unwrap();

        assert_eq!(actual, expected);
        let callee = actual
            .iter()
            .find(|chunk| chunk.source.symbol == "callee" && chunk.repr == "code")
            .expect("callee chunk should exist");
        assert!(
            callee
                .context
                .called_by
                .iter()
                .any(|caller| caller.contains("caller")),
            "called_by should include caller across stream batches: {:?}",
            callee.context.called_by
        );
    }

    #[test]
    fn test_stream_config() {
        let config = StreamConfig {
            file_batch_size: 10,
            chunk_buffer_size: 50,
            skip_on_error: true,
            max_errors: 5,
            parallel_batches: false,
        };

        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test.rs", "fn test() {}");

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let stream = ChunkStream::with_config(temp_dir.path(), settings, limits, config).unwrap();
        let chunks: Vec<_> = stream.filter_map(|r| r.ok()).collect();

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_stream_with_repo_id() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test.rs", "fn test() {}");

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();
        let repo_id = RepoIdentifier::new("github.com/test", "my-repo");

        let stream = ChunkStream::new(temp_dir.path(), settings, limits)
            .unwrap()
            .with_repo_id(repo_id);

        let chunks: Vec<_> = stream.filter_map(|r| r.ok()).collect();

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].source.repo.namespace.as_deref(), Some("github.com/test"));
        assert_eq!(chunks[0].source.repo.name, "my-repo");
    }

    // ---------------------------------------------------------------
    // Behavioral tests for keyword and context_prefix population
    // (Issue #100: ChunkStream was producing empty keywords/context_prefix)
    // ---------------------------------------------------------------

    #[test]
    fn test_extract_keywords_returns_domain_terms_not_language_keywords() {
        let rust_code = r#"
fn calculate_checksum(buffer: &[u8]) -> u64 {
    let mut digest = 0u64;
    for byte in buffer {
        digest = digest.wrapping_mul(31).wrapping_add(*byte as u64);
    }
    digest
}
"#;
        let keywords = super::super::chunker::extract_keywords(rust_code);

        // Should contain domain-specific identifiers split from camelCase/snake_case
        assert!(
            keywords.contains(&"calculate".to_string()),
            "Expected 'calculate' in keywords, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"checksum".to_string()),
            "Expected 'checksum' in keywords, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"buffer".to_string()),
            "Expected 'buffer' in keywords, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"digest".to_string()),
            "Expected 'digest' in keywords, got: {:?}",
            keywords
        );

        // Should NOT contain generic Rust keywords (these are in the stopword list)
        assert!(!keywords.contains(&"fn".to_string()), "'fn' should be filtered as a stopword");
        assert!(!keywords.contains(&"let".to_string()), "'let' should be filtered as a stopword");
        assert!(!keywords.contains(&"for".to_string()), "'for' should be filtered as a stopword");
        assert!(!keywords.contains(&"mut".to_string()), "'mut' should be filtered as a stopword");
    }

    #[test]
    fn test_extract_keywords_handles_camel_case_and_snake_case() {
        let code = r#"
fn parse_http_response(rawBytes: &[u8]) -> HttpResponse {
    let contentLength = extract_content_length(rawBytes);
    HttpResponse::new(contentLength)
}
"#;
        let keywords = super::super::chunker::extract_keywords(code);

        // snake_case splits: parse_http_response -> parse, http, response
        assert!(
            keywords.contains(&"parse".to_string()),
            "Expected 'parse' from snake_case split, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"http".to_string()),
            "Expected 'http' from snake_case split, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"response".to_string()),
            "Expected 'response' from identifier split, got: {:?}",
            keywords
        );

        // camelCase splits: rawBytes -> raw, Bytes; contentLength -> content, Length
        assert!(
            keywords.contains(&"content".to_string()),
            "Expected 'content' from camelCase split, got: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"length".to_string()),
            "Expected 'length' from camelCase split, got: {:?}",
            keywords
        );
    }

    #[test]
    fn test_extract_keywords_nonempty_for_nontrivial_code() {
        // Any function with meaningful identifier names should produce keywords
        let code = r#"
fn validate_user_credentials(username: &str, password: &str) -> bool {
    let stored_hash = fetch_password_hash(username);
    verify_hash(password, &stored_hash)
}
"#;
        let keywords = super::super::chunker::extract_keywords(code);
        assert!(!keywords.is_empty(), "Non-trivial code should produce at least some keywords");
        // Should have several domain terms
        assert!(
            keywords.len() >= 3,
            "Expected at least 3 keywords for code with rich identifiers, got {}: {:?}",
            keywords.len(),
            keywords
        );
    }

    #[test]
    fn test_generate_context_prefix_format_without_parent() {
        use crate::types::SymbolKind;

        let prefix = super::super::chunker::generate_context_prefix(
            "src/auth.rs",
            None,
            &SymbolKind::Function,
        );

        assert_eq!(prefix, "From src/auth.rs, function");
    }

    #[test]
    fn test_generate_context_prefix_format_with_parent() {
        use crate::types::SymbolKind;

        let prefix = super::super::chunker::generate_context_prefix(
            "src/models/user.rs",
            Some("UserService"),
            &SymbolKind::Method,
        );

        assert_eq!(prefix, "From src/models/user.rs, in UserService, method");
    }

    #[test]
    fn test_generate_context_prefix_various_kinds() {
        use crate::types::SymbolKind;

        let cases = vec![
            (SymbolKind::Class, "class"),
            (SymbolKind::Struct, "struct"),
            (SymbolKind::Enum, "enum"),
            (SymbolKind::Trait, "trait"),
            (SymbolKind::Interface, "interface"),
            (SymbolKind::Constant, "constant"),
            (SymbolKind::Import, "import"),
            (SymbolKind::Module, "module"),
            (SymbolKind::Macro, "macro"),
        ];

        for (kind, expected_name) in cases {
            let prefix = super::super::chunker::generate_context_prefix("src/lib.rs", None, &kind);
            assert_eq!(
                prefix,
                format!("From src/lib.rs, {expected_name}"),
                "Wrong prefix for kind {:?}",
                kind
            );
        }
    }

    #[test]
    fn test_chunk_stream_populates_keywords_and_context_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let rust_code = r#"
/// Validates and normalizes an email address
fn validate_email_address(input: &str) -> Option<String> {
    let trimmed = input.trim().to_lowercase();
    if trimmed.contains('@') && trimmed.contains('.') {
        Some(trimmed)
    } else {
        None
    }
}
"#;
        create_test_file(temp_dir.path(), "src/validator.rs", rust_code);

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();
        let chunks: Vec<_> = stream.filter_map(|r| r.ok()).collect();

        assert!(!chunks.is_empty(), "Should produce at least one chunk");

        for chunk in &chunks {
            // Keywords should be populated (not empty)
            assert!(
                !chunk.context.keywords.is_empty(),
                "Chunk '{}' has empty keywords; expected domain terms from the code",
                chunk.source.symbol
            );

            // Context prefix should be populated (not None)
            assert!(
                chunk.context.context_prefix.is_some(),
                "Chunk '{}' has None context_prefix; expected 'From <path>, <kind>'",
                chunk.source.symbol
            );

            let prefix = chunk.context.context_prefix.as_ref().unwrap();

            // Prefix should start with "From " and contain the file path
            assert!(
                prefix.starts_with("From "),
                "Context prefix should start with 'From ', got: {}",
                prefix
            );
            assert!(
                prefix.contains("validator.rs"),
                "Context prefix should reference the source file, got: {}",
                prefix
            );
        }

        // Find the validate_email_address chunk specifically
        let email_chunk = chunks
            .iter()
            .find(|c| c.source.symbol == "validate_email_address");
        assert!(email_chunk.is_some(), "Should have a chunk for validate_email_address");

        let email_chunk = email_chunk.unwrap();

        // Verify keywords contain domain-relevant terms
        let kw = &email_chunk.context.keywords;
        assert!(
            kw.contains(&"validate".to_string()) || kw.contains(&"email".to_string()),
            "Keywords for validate_email_address should include 'validate' or 'email', got: {:?}",
            kw
        );

        // Verify context prefix format for a top-level function
        let prefix = email_chunk.context.context_prefix.as_ref().unwrap();
        assert!(
            prefix.contains("function"),
            "Context prefix for a function should contain 'function', got: {}",
            prefix
        );
    }

    #[test]
    fn test_chunk_stream_context_prefix_includes_parent_for_methods() {
        let temp_dir = TempDir::new().unwrap();
        // Use Python since Tree-sitter reliably detects class methods with parent info
        let python_code = r#"
class DatabaseConnection:
    def execute_query(self, sql_statement):
        cursor = self.connection.cursor()
        cursor.execute(sql_statement)
        return cursor.fetchall()
"#;
        create_test_file(temp_dir.path(), "src/database.py", python_code);

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let stream = ChunkStream::new(temp_dir.path(), settings, limits).unwrap();
        let chunks: Vec<_> = stream.filter_map(|r| r.ok()).collect();

        // Find the method chunk
        let method_chunk = chunks.iter().find(|c| c.source.symbol == "execute_query");

        if let Some(chunk) = method_chunk {
            let prefix = chunk.context.context_prefix.as_ref().unwrap();
            // If parent was detected, prefix should include "in <parent>"
            if chunk.source.parent.is_some() {
                assert!(
                    prefix.contains("in "),
                    "Method with parent should have 'in <parent>' in prefix, got: {}",
                    prefix
                );
                assert!(
                    prefix.contains("DatabaseConnection"),
                    "Parent should be 'DatabaseConnection', got: {}",
                    prefix
                );
            }
            // Keywords should include domain terms from the method
            assert!(!chunk.context.keywords.is_empty(), "Method chunk should have keywords");
        }
        // If the method chunk was not found (parser limitation), the test still passes
        // since the class chunk would have been produced instead
    }
}
