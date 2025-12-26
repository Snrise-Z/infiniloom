//! Shared scanner types and utilities
//!
//! This module provides common types and functions used by both the CLI scanner
//! and the bindings scanner. It centralizes duplicate code for:
//! - Scanner configuration
//! - File metadata collection
//! - Binary file detection
//!
//! The actual scanning implementations remain separate due to architectural
//! differences (pipelined vs simple parallel), but they share these common types.

mod common;

pub use common::{is_binary_content, is_binary_extension, BINARY_EXTENSIONS};

use std::path::PathBuf;

/// Runtime configuration for repository scanning
///
/// This is the operational config used during scanning, as opposed to
/// `crate::config::ScanConfig` which is for configuration file settings.
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
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: true,
            max_file_size: 50 * 1024 * 1024, // 50MB
            skip_symbols: false,
        }
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
        Self {
            path,
            relative_path,
            size_bytes: None,
            language: None,
        }
    }

    /// Create FileInfo with size information
    pub fn with_size(path: PathBuf, relative_path: String, size_bytes: u64) -> Self {
        Self {
            path,
            relative_path,
            size_bytes: Some(size_bytes),
            language: None,
        }
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
        let info = FileInfo::with_size(PathBuf::from("/path/to/file.rs"), "file.rs".to_string(), 1024);
        assert_eq!(info.size_bytes, Some(1024));
    }

    #[test]
    fn test_file_info_with_language() {
        let info = FileInfo::new(PathBuf::from("/path/to/file.rs"), "file.rs".to_string())
            .with_language(Some("Rust".to_string()));
        assert_eq!(info.language, Some("Rust".to_string()));
    }
}
