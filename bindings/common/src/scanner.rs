//! Repository scanner for language bindings
//!
//! This module wraps the unified scanner from the engine with bindings-specific defaults:
//! - Accurate token counting via tiktoken (for API use cases)
//! - Pipelined scanning for large repositories
//! - Batching to prevent stack overflow on very large repos

use anyhow::Result;
use std::path::Path;

use infiniloom_engine::scanner::{scan_repository as unified_scan, ScannerConfig};
use infiniloom_engine::types::Repository;

/// Configuration for repository scanning (bindings-specific)
///
/// This provides a simpler configuration interface for the language bindings.
pub struct ScanConfig {
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

impl Default for ScanConfig {
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

impl From<ScanConfig> for ScannerConfig {
    fn from(config: ScanConfig) -> Self {
        ScannerConfig {
            include_hidden: config.include_hidden,
            respect_gitignore: config.respect_gitignore,
            read_contents: config.read_contents,
            max_file_size: config.max_file_size,
            skip_symbols: config.skip_symbols,
            // Bindings use accurate token counting by default
            accurate_tokens: true,
            use_mmap: true,
            use_pipelining: true,
            ..Default::default()
        }
    }
}

/// Scan a repository and return a Repository struct
///
/// Uses the unified scanner from engine with bindings-specific defaults:
/// - Accurate token counting (tiktoken)
/// - Pipelined processing for large repos
pub fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let scanner_config: ScannerConfig = config.into();
    unified_scan(path, scanner_config)
}

/// Simple glob pattern matching for include/exclude patterns
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    if let Ok(glob) = glob::Pattern::new(pattern) {
        if glob.matches(path) {
            return true;
        }
    }
    // Also check if pattern matches any path component
    if let Some(suffix) = pattern.strip_prefix("**/") {
        if let Ok(glob) = glob::Pattern::new(suffix) {
            // Check against each component and suffix of path
            for (i, _) in path.match_indices('/') {
                if glob.matches(&path[i + 1..]) {
                    return true;
                }
            }
            if glob.matches(path) {
                return true;
            }
        }
    }
    false
}

/// Check if a path matches any of the given patterns
pub fn matches_any_pattern(path: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| matches_pattern(path, p))
}

// Re-export commonly used items for convenience
pub use infiniloom_engine::scanner::{
    is_binary_extension, FileInfo, ScannerConfig as EngineScannerConfig,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_scan_empty_dir() {
        let dir = tempdir().unwrap();
        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();
        assert_eq!(repo.files.len(), 0);
    }

    #[test]
    fn test_scan_single_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("test.rs"));
        assert_eq!(repo.files[0].language, Some("rust".to_string()));
    }

    #[test]
    fn test_skip_binary_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("binary.exe"), "not really binary").unwrap();
        fs::write(dir.path().join("source.rs"), "fn main() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("source.rs"));
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("src/main.rs", "*.rs"));
        assert!(matches_pattern("src/main.rs", "**/*.rs"));
        assert!(matches_pattern("src/test/main.rs", "**/main.rs"));
        assert!(!matches_pattern("src/main.ts", "*.rs"));
    }

    #[test]
    fn test_matches_any_pattern() {
        let patterns = vec!["*.rs", "*.ts"];
        assert!(matches_any_pattern("main.rs", &patterns));
        assert!(matches_any_pattern("main.ts", &patterns));
        assert!(!matches_any_pattern("main.py", &patterns));
    }

    #[test]
    fn test_scan_config_to_scanner_config() {
        let config = ScanConfig {
            include_hidden: true,
            respect_gitignore: false,
            read_contents: true,
            max_file_size: 1000,
            skip_symbols: true,
        };

        let scanner_config: ScannerConfig = config.into();
        assert!(scanner_config.include_hidden);
        assert!(!scanner_config.respect_gitignore);
        assert!(scanner_config.read_contents);
        assert_eq!(scanner_config.max_file_size, 1000);
        assert!(scanner_config.skip_symbols);
        // Bindings defaults
        assert!(scanner_config.accurate_tokens);
        assert!(scanner_config.use_mmap);
    }

    #[test]
    fn test_scan_skip_symbols() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScanConfig { skip_symbols: true, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].symbols.is_empty());
    }
}
