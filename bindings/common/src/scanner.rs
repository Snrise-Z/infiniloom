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
    // Empty patterns shouldn't match anything - this is a defensive check
    // to avoid unexpected behavior with empty include/exclude patterns
    if pattern.is_empty() {
        return false;
    }

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

    // ============================================================================
    // scan_repository tests - comprehensive coverage
    // ============================================================================

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
    fn test_scan_multiple_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(dir.path().join("utils.py"), "def utils(): pass").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 3);
    }

    #[test]
    fn test_scan_nested_directories() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/utils")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("src/utils/helper.rs"), "pub fn help() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 2);
        assert!(repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("main.rs")));
        assert!(repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("helper.rs")));
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
    fn test_skip_binary_extensions() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("image.png"), "fake image").unwrap();
        fs::write(dir.path().join("archive.zip"), "fake archive").unwrap();
        fs::write(dir.path().join("lib.dll"), "fake dll").unwrap();
        fs::write(dir.path().join("app.exe"), "fake exe").unwrap();
        fs::write(dir.path().join("source.rs"), "fn main() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("source.rs"));
    }

    #[test]
    fn test_scan_with_hidden_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".hidden.rs"), "fn hidden() {}").unwrap();
        fs::write(dir.path().join("visible.rs"), "fn visible() {}").unwrap();

        // Default: skip hidden
        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();
        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("visible.rs"));

        // Include hidden
        let config = ScanConfig { include_hidden: true, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();
        assert_eq!(repo.files.len(), 2);
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

    #[test]
    fn test_scan_without_contents() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScanConfig { read_contents: false, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].content.is_none());
    }

    #[test]
    fn test_scan_respects_max_file_size() {
        let dir = tempdir().unwrap();
        // Create a file larger than max_file_size
        let large_content = "x".repeat(2000);
        fs::write(dir.path().join("large.rs"), &large_content).unwrap();
        fs::write(dir.path().join("small.rs"), "fn small() {}").unwrap();

        let config = ScanConfig { max_file_size: 1000, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();

        // Large file should be skipped
        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("small.rs"));
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let result = scan_repository(Path::new("/nonexistent/path/12345"), ScanConfig::default());
        // Should return error or empty repo
        assert!(result.is_err() || result.unwrap().files.is_empty());
    }

    #[test]
    fn test_scan_config_defaults() {
        let config = ScanConfig::default();
        assert!(!config.include_hidden);
        assert!(config.respect_gitignore);
        assert!(config.read_contents);
        assert_eq!(config.max_file_size, 50 * 1024 * 1024); // 50MB
        assert!(!config.skip_symbols);
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

    // ============================================================================
    // matches_pattern tests - comprehensive coverage
    // ============================================================================

    #[test]
    fn test_matches_pattern_basic() {
        assert!(matches_pattern("src/main.rs", "*.rs"));
        assert!(matches_pattern("src/main.rs", "**/*.rs"));
        assert!(matches_pattern("src/test/main.rs", "**/main.rs"));
        assert!(!matches_pattern("src/main.ts", "*.rs"));
    }

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("main.rs", "main.rs"));
        assert!(matches_pattern("src/main.rs", "src/main.rs"));
        assert!(!matches_pattern("main.rs", "main.ts"));
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        assert!(matches_pattern("test.rs", "*.rs"));
        assert!(matches_pattern("main.rs", "*.rs"));
        assert!(matches_pattern("x.rs", "*.rs"));
        assert!(!matches_pattern("test.ts", "*.rs"));
    }

    #[test]
    fn test_matches_pattern_double_star() {
        assert!(matches_pattern("main.rs", "**/*.rs"));
        assert!(matches_pattern("src/main.rs", "**/*.rs"));
        assert!(matches_pattern("deep/nested/path/main.rs", "**/*.rs"));
        assert!(!matches_pattern("main.ts", "**/*.rs"));
    }

    #[test]
    fn test_matches_pattern_double_star_prefix() {
        assert!(matches_pattern("src/main.rs", "**/main.rs"));
        assert!(matches_pattern("a/b/c/main.rs", "**/main.rs"));
        assert!(matches_pattern("main.rs", "**/main.rs"));
    }

    #[test]
    fn test_matches_pattern_character_class() {
        assert!(matches_pattern("test.rs", "test.[rt]s"));
        assert!(matches_pattern("test.ts", "test.[rt]s"));
        assert!(!matches_pattern("test.js", "test.[rt]s"));
    }

    #[test]
    fn test_matches_pattern_question_mark() {
        assert!(matches_pattern("test.rs", "tes?.rs"));
        assert!(matches_pattern("tesa.rs", "tes?.rs"));
        assert!(!matches_pattern("testing.rs", "tes?.rs"));
    }

    #[test]
    fn test_matches_pattern_empty() {
        assert!(!matches_pattern("test.rs", ""));
        assert!(!matches_pattern("", "*.rs"));
        assert!(!matches_pattern("", ""));
    }

    #[test]
    fn test_matches_pattern_special_chars() {
        // Patterns with special characters
        assert!(matches_pattern("file[1].rs", "file[[]1].rs"));
    }

    // ============================================================================
    // matches_any_pattern tests - comprehensive coverage
    // ============================================================================

    #[test]
    fn test_matches_any_pattern_basic() {
        let patterns = vec!["*.rs", "*.ts"];
        assert!(matches_any_pattern("main.rs", &patterns));
        assert!(matches_any_pattern("main.ts", &patterns));
        assert!(!matches_any_pattern("main.py", &patterns));
    }

    #[test]
    fn test_matches_any_pattern_empty_patterns() {
        let patterns: Vec<&str> = vec![];
        assert!(!matches_any_pattern("main.rs", &patterns));
    }

    #[test]
    fn test_matches_any_pattern_single_pattern() {
        let patterns = vec!["*.rs"];
        assert!(matches_any_pattern("main.rs", &patterns));
        assert!(!matches_any_pattern("main.ts", &patterns));
    }

    #[test]
    fn test_matches_any_pattern_multiple_matches() {
        let patterns = vec!["*.rs", "main.*", "**/*.rs"];
        // All patterns match "main.rs"
        assert!(matches_any_pattern("main.rs", &patterns));
    }

    #[test]
    fn test_matches_any_pattern_nested_paths() {
        let patterns = vec!["**/*.test.ts", "**/spec.js"];
        assert!(matches_any_pattern("src/foo.test.ts", &patterns));
        assert!(matches_any_pattern("deep/nested/spec.js", &patterns));
        assert!(!matches_any_pattern("src/main.ts", &patterns));
    }

    #[test]
    fn test_matches_any_pattern_empty_path() {
        let patterns = vec!["*.rs"];
        assert!(!matches_any_pattern("", &patterns));
    }

    // ============================================================================
    // Language detection tests
    // ============================================================================

    #[test]
    fn test_scan_language_detection() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("test.py"), "def main(): pass").unwrap();
        fs::write(dir.path().join("test.ts"), "function main() {}").unwrap();
        fs::write(dir.path().join("test.js"), "function main() {}").unwrap();
        fs::write(dir.path().join("test.go"), "func main() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 5);

        for file in &repo.files {
            match file.relative_path.as_str() {
                p if p.ends_with(".rs") => assert_eq!(file.language, Some("rust".to_string())),
                p if p.ends_with(".py") => assert_eq!(file.language, Some("python".to_string())),
                p if p.ends_with(".ts") => {
                    assert_eq!(file.language, Some("typescript".to_string()))
                },
                p if p.ends_with(".js") => {
                    assert_eq!(file.language, Some("javascript".to_string()))
                },
                p if p.ends_with(".go") => assert_eq!(file.language, Some("go".to_string())),
                _ => panic!("Unexpected file"),
            }
        }
    }

    // ============================================================================
    // Symbol extraction tests
    // ============================================================================

    #[test]
    fn test_scan_extracts_symbols() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("test.rs"),
            r#"
pub fn public_function() {}
fn private_function() {}
pub struct MyStruct {
    field: i32,
}
"#,
        )
        .unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        let symbols = &repo.files[0].symbols;
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name == "public_function"));
    }

    // ============================================================================
    // Edge cases and error handling
    // ============================================================================

    #[test]
    fn test_scan_file_with_special_name() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file with spaces.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("file-with-dashes.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("file_with_underscores.rs"), "fn main() {}").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 3);
    }

    #[test]
    fn test_scan_empty_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("empty.rs"), "").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0]
            .content
            .as_ref()
            .map(|c| c.is_empty())
            .unwrap_or(false));
    }

    #[test]
    fn test_scan_unicode_content() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("unicode.py"), "def greet(): print('こんにちは世界')").unwrap();

        let config = ScanConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0]
            .content
            .as_ref()
            .map(|c| c.contains("こんにちは"))
            .unwrap_or(false));
    }
}
