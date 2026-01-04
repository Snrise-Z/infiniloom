//! Streaming API for large repository processing
//!
//! This module provides an iterator-based interface for processing large repositories
//! without loading all chunks into memory simultaneously. This is essential for:
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
//! // Process chunks as they're generated
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
//! # Memory Guarantees
//!
//! The streaming API bounds memory usage to approximately:
//! - `batch_size * avg_chunk_size` for chunk data
//! - `O(files_in_current_batch)` for file metadata
//! - `O(symbols_per_file)` for parse state
//!
//! For a typical batch_size of 100 and avg_chunk_size of 5KB, memory usage
//! is bounded to ~500KB for chunk data, regardless of repository size.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::parser::{parse_file_symbols, Language};
use crate::security::SecurityScanner;
use crate::tokenizer::{TokenModel, Tokenizer};

use super::error::EmbedError;
use super::hasher::hash_content;
use super::limits::ResourceLimits;
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
/// This iterator yields chunks one at a time as they are generated,
/// without loading the entire repository into memory.
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

            // Check for supported language
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e,
                None => continue,
            };

            if Language::from_extension(ext).is_none() {
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

    /// Process the next batch of files and fill the chunk buffer
    fn fill_buffer(&mut self) -> bool {
        if self.is_cancelled() {
            return false;
        }

        // Take a batch of files
        let batch_size = self.config.file_batch_size.min(self.pending_files.len());
        if batch_size == 0 {
            return false;
        }

        let batch: Vec<_> = (0..batch_size)
            .filter_map(|_| self.pending_files.pop_front())
            .collect();

        // Process files
        for file_path in batch {
            if self.is_cancelled() {
                break;
            }

            match self.process_file(&file_path) {
                Ok(chunks) => {
                    self.stats.files_processed += 1;
                    self.stats.chunks_generated += chunks.len();

                    for chunk in chunks {
                        self.chunk_buffer.push_back(Ok(chunk));
                    }
                },
                Err(e) => {
                    self.stats.error_count += 1;
                    let current_errors = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;

                    if e.is_skippable() && self.config.skip_on_error {
                        self.stats.files_skipped += 1;
                        // Optionally emit the error for logging
                        if !e.is_critical() {
                            self.chunk_buffer.push_back(Err(e));
                        }
                    } else if current_errors >= self.config.max_errors {
                        // Too many errors, emit and stop
                        self.chunk_buffer.push_back(Err(EmbedError::TooManyErrors {
                            count: current_errors,
                            max: self.config.max_errors,
                        }));
                        self.cancelled.store(true, Ordering::Relaxed);
                        break;
                    } else if e.is_critical() {
                        self.chunk_buffer.push_back(Err(e));
                        break;
                    }
                },
            }
        }

        !self.chunk_buffer.is_empty() || !self.pending_files.is_empty()
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

            chunks.push(EmbedChunk {
                id: hash.short_id,
                full_hash: hash.full_hash,
                content: chunk_content,
                tokens,
                kind: symbol.kind.into(),
                source: ChunkSource {
                    repo: self.repo_id.clone(),
                    file: relative_path.clone(),
                    lines: ((context_start + 1) as u32, context_end as u32),
                    symbol: symbol.name.clone(),
                    fqn: Some(fqn),
                    language: language.clone(),
                    parent: symbol.parent.clone(),
                    visibility: symbol.visibility.into(),
                    is_test: self.is_test_code(path, symbol),
                },
                context: ChunkContext {
                    docstring: symbol.docstring.clone(),
                    comments: Vec::new(),
                    signature: symbol.signature.clone(),
                    calls: symbol.calls.clone(),
                    called_by: Vec::new(),
                    imports: Vec::new(),
                    tags: Vec::new(),
                    lines_of_code: 0,
                    max_nesting_depth: 0,
                },
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

    /// Detect language from file path
    fn detect_language(&self, path: &Path) -> String {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Language::from_extension)
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

        if let Some(ref parent) = symbol.parent {
            format!("{}::{}::{}", module_path, parent, symbol.name)
        } else {
            format!("{}::{}", module_path, symbol.name)
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
        assert_eq!(chunks[0].source.repo.namespace, "github.com/test");
        assert_eq!(chunks[0].source.repo.name, "my-repo");
    }
}
