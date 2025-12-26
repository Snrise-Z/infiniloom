//! Unified scanner module for repository scanning
//!
//! This module provides a unified scanner implementation used by both the CLI
//! and language bindings. It includes:
//!
//! - [`ScannerConfig`]: Configuration for scanning behavior
//! - [`FileInfo`]: Intermediate file metadata during scanning
//! - [`UnifiedScanner`]: Main scanner with configurable features
//! - Binary detection utilities
//!
//! # Architecture
//!
//! The scanner uses a pipelined architecture for large repositories:
//! 1. **Walk phase**: Collect file paths with the `ignore` crate
//! 2. **Read phase**: Multiple reader threads read file contents
//! 3. **Parse phase**: Parser threads extract symbols in parallel
//! 4. **Aggregate phase**: Collect results into final Repository
//!
//! For smaller repositories (< 100 files), a simpler parallel approach is used.
//!
//! # Features
//!
//! Configurable features include:
//! - Memory-mapped I/O for large files (>= 1MB)
//! - Accurate tiktoken tokenization vs fast estimation
//! - Pipelined vs simple parallel processing
//! - Batch processing to prevent stack overflow

mod common;
mod io;
mod parallel;
mod pipelined;
mod process;
mod walk;

pub use common::{is_binary_content, is_binary_extension, BINARY_EXTENSIONS};
pub use io::{smart_read_file, smart_read_file_with_options, MMAP_THRESHOLD};
pub use parallel::{scan_repository, UnifiedScanner};
pub use pipelined::scan_files_pipelined;
pub use process::{
    count_tokens, count_tokens_accurate, estimate_lines, estimate_tokens, parse_with_thread_local,
    process_file_content_only, process_file_with_content, process_file_without_content,
};
pub use walk::{collect_file_infos, collect_file_paths};

use std::path::PathBuf;

/// Runtime configuration for repository scanning
///
/// This is the operational config used during scanning, as opposed to
/// `crate::config::ScanConfig` which is for configuration file settings.
///
/// # Example
///
/// ```
/// use infiniloom_engine::scanner::ScannerConfig;
///
/// // Fast CLI-style scanning with estimation
/// let cli_config = ScannerConfig::default();
///
/// // Accurate API-style scanning with tiktoken
/// let api_config = ScannerConfig {
///     accurate_tokens: true,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Read and store file contents
    pub read_contents: bool,
    /// Maximum file size to read (bytes)
    pub max_file_size: u64,
    /// Skip symbol extraction for faster scanning
    pub skip_symbols: bool,

    // Performance tuning options
    /// Use memory-mapped I/O for files >= MMAP_THRESHOLD (1MB)
    /// Default: true
    pub use_mmap: bool,
    /// Use accurate tiktoken tokenization instead of estimation
    /// Default: false (estimation is ~80x faster)
    pub accurate_tokens: bool,
    /// Use pipelined architecture for repos >= PIPELINE_THRESHOLD files
    /// Default: true
    pub use_pipelining: bool,
    /// Maximum files to process in a single parallel batch
    /// Prevents stack overflow on repos with 75K+ files
    /// Default: 5000
    pub batch_size: usize,
}

/// Minimum number of files to trigger pipelined mode
pub const PIPELINE_THRESHOLD: usize = 100;

/// Maximum files to process in a single parallel batch to avoid stack overflow
pub const DEFAULT_BATCH_SIZE: usize = 5000;

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: true,
            max_file_size: 50 * 1024 * 1024, // 50MB
            skip_symbols: false,
            // Performance defaults
            use_mmap: true,
            accurate_tokens: false,
            use_pipelining: true,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl ScannerConfig {
    /// Create config for fast CLI-style scanning
    pub fn fast() -> Self {
        Self::default()
    }

    /// Create config for accurate API-style scanning
    pub fn accurate() -> Self {
        Self { accurate_tokens: true, ..Default::default() }
    }
}

/// Intermediate struct for collecting file info before parallel processing
///
/// Used during the initial directory walk phase before content is read.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Absolute path to the file
    pub path: PathBuf,
    /// Path relative to repository root
    pub relative_path: String,
    /// File size in bytes (if known)
    pub size_bytes: Option<u64>,
    /// Detected language (if known)
    pub language: Option<String>,
}

impl FileInfo {
    /// Create a new FileInfo with required fields
    pub fn new(path: PathBuf, relative_path: String) -> Self {
        Self { path, relative_path, size_bytes: None, language: None }
    }

    /// Create FileInfo with size information
    pub fn with_size(path: PathBuf, relative_path: String, size_bytes: u64) -> Self {
        Self { path, relative_path, size_bytes: Some(size_bytes), language: None }
    }

    /// Set the detected language
    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_config_default() {
        let config = ScannerConfig::default();
        assert!(!config.include_hidden);
        assert!(config.respect_gitignore);
        assert!(config.read_contents);
        assert_eq!(config.max_file_size, 50 * 1024 * 1024);
        assert!(!config.skip_symbols);
        // Performance defaults
        assert!(config.use_mmap);
        assert!(!config.accurate_tokens);
        assert!(config.use_pipelining);
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn test_scanner_config_fast() {
        let config = ScannerConfig::fast();
        assert!(!config.accurate_tokens);
        assert!(config.use_mmap);
        assert!(config.use_pipelining);
    }

    #[test]
    fn test_scanner_config_accurate() {
        let config = ScannerConfig::accurate();
        assert!(config.accurate_tokens);
        assert!(config.use_mmap);
        assert!(config.use_pipelining);
    }

    #[test]
    fn test_file_info_new() {
        let info = FileInfo::new(PathBuf::from("/path/to/file.rs"), "file.rs".to_string());
        assert_eq!(info.relative_path, "file.rs");
        assert!(info.size_bytes.is_none());
        assert!(info.language.is_none());
    }

    #[test]
    fn test_file_info_with_size() {
        let info =
            FileInfo::with_size(PathBuf::from("/path/to/file.rs"), "file.rs".to_string(), 1024);
        assert_eq!(info.size_bytes, Some(1024));
    }

    #[test]
    fn test_file_info_with_language() {
        let info = FileInfo::new(PathBuf::from("/path/to/file.rs"), "file.rs".to_string())
            .with_language(Some("Rust".to_string()));
        assert_eq!(info.language, Some("Rust".to_string()));
    }
}
