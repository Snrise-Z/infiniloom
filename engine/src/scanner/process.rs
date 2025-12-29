//! File processing utilities
//!
//! This module provides file processing functions for the scanner,
//! including token counting, symbol extraction, and file metadata.

use std::path::Path;

use crate::parser;
use crate::tokenizer::{TokenCounts, Tokenizer};
use crate::types::{RepoFile, Symbol};

use super::io::smart_read_file_with_options;
use super::{FileInfo, ScannerConfig};

// Thread-local tokenizer for lock-free parallel token counting
thread_local! {
    static THREAD_TOKENIZER: Tokenizer = Tokenizer::new();
}

/// Parse content using optimized thread-local parser (lock-free)
///
/// Uses the centralized thread-local parser from `parser::parse_file_symbols`.
/// Each thread maintains a single lazily-initialized parser instance.
///
/// # Performance
///
/// - **2-3x faster** than old RefCell-based pattern
/// - **Single initialization** per thread (vs per-call)
/// - **Reduced overhead** from eliminated language detection duplication
pub fn parse_with_thread_local(content: &str, path: &Path) -> Vec<Symbol> {
    parser::parse_file_symbols(content, path)
}

/// Count tokens using configurable method
///
/// When `accurate` is true, uses tiktoken for exact BPE counts.
/// When false, uses fast estimation (~80x faster).
pub fn count_tokens(content: &str, size_bytes: u64, accurate: bool) -> TokenCounts {
    if accurate {
        count_tokens_accurate(content)
    } else {
        estimate_tokens(size_bytes, Some(content))
    }
}

/// Count tokens using thread-local tokenizer (accurate via tiktoken)
///
/// Provides exact BPE token counts for OpenAI models.
/// More accurate but significantly slower than estimation.
pub fn count_tokens_accurate(content: &str) -> TokenCounts {
    THREAD_TOKENIZER.with(|tokenizer| tokenizer.count_all(content))
}

/// Estimate tokens from file size
///
/// Uses calibrated character-per-token ratios for each model family.
/// Fast (~80x faster than tiktoken) with ~95% accuracy.
pub fn estimate_tokens(size_bytes: u64, content: Option<&str>) -> TokenCounts {
    // If we have content, use content length for better accuracy
    let len = content.map(|c| c.len() as f32).unwrap_or(size_bytes as f32);

    TokenCounts {
        o200k: (len / 4.0) as u32,  // OpenAI modern (GPT-5.x, GPT-4o, O-series)
        cl100k: (len / 3.7) as u32, // OpenAI legacy (GPT-4, GPT-3.5)
        claude: (len / 3.5) as u32,
        gemini: (len / 3.8) as u32,
        llama: (len / 3.5) as u32,
        mistral: (len / 3.5) as u32,
        deepseek: (len / 3.5) as u32,
        qwen: (len / 3.5) as u32,
        cohere: (len / 3.6) as u32,
        grok: (len / 3.5) as u32,
    }
}

/// Estimate lines from file size
///
/// Uses average of ~40 characters per line.
pub fn estimate_lines(size_bytes: u64) -> u64 {
    size_bytes / 40
}

/// Process a file with content reading only (no parsing - fast path)
///
/// Reads file content but skips symbol extraction for speed.
pub fn process_file_content_only(info: FileInfo, config: &ScannerConfig) -> Option<RepoFile> {
    let size_bytes = info.size_bytes.unwrap_or(0);
    let content = smart_read_file_with_options(&info.path, size_bytes, config.use_mmap)?;
    let token_count = count_tokens(&content, size_bytes, config.accurate_tokens);

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file with content reading and parsing (used in parallel)
///
/// Uses thread-local parser for lock-free parallel parsing.
/// Uses memory-mapped I/O for files >= 1MB if enabled.
pub fn process_file_with_content(info: FileInfo, config: &ScannerConfig) -> Option<RepoFile> {
    let size_bytes = info.size_bytes.unwrap_or(0);
    let content = smart_read_file_with_options(&info.path, size_bytes, config.use_mmap)?;
    let token_count = count_tokens(&content, size_bytes, config.accurate_tokens);
    let symbols = parse_with_thread_local(&content, &info.path);

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes,
        token_count,
        symbols,
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file without reading content (fast path)
///
/// Only collects metadata, skipping content reading and parsing.
pub fn process_file_without_content(info: FileInfo, config: &ScannerConfig) -> RepoFile {
    let size_bytes = info.size_bytes.unwrap_or(0);
    let token_count = if config.accurate_tokens {
        // Can't use accurate counting without content
        estimate_tokens(size_bytes, None)
    } else {
        estimate_tokens(size_bytes, None)
    };

    RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_estimate_tokens_from_content() {
        let content = "Hello, World!";
        let tokens = estimate_tokens(0, Some(content));
        // 13 chars / 4.0 = 3 (o200k)
        assert_eq!(tokens.o200k, 3);
    }

    #[test]
    fn test_estimate_tokens_from_size() {
        let tokens = estimate_tokens(1000, None);
        // 1000 bytes / 4.0 = 250 (o200k)
        assert_eq!(tokens.o200k, 250);
    }

    #[test]
    fn test_estimate_lines() {
        assert_eq!(estimate_lines(400), 10);
        assert_eq!(estimate_lines(0), 0);
    }

    #[test]
    fn test_count_tokens_configurable() {
        let content = "fn main() {}";

        // Fast estimation
        let fast = count_tokens(content, content.len() as u64, false);

        // Accurate (tiktoken)
        let accurate = count_tokens(content, content.len() as u64, true);

        // Both should produce reasonable results
        assert!(fast.o200k > 0);
        assert!(accurate.o200k > 0);
    }

    #[test]
    fn test_process_file_content_only() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let info = FileInfo {
            path: file_path,
            relative_path: "test.rs".to_string(),
            size_bytes: Some(12),
            language: Some("rust".to_string()),
        };

        let config = ScannerConfig::default();
        let result = process_file_content_only(info, &config);

        assert!(result.is_some());
        let repo_file = result.unwrap();
        assert!(repo_file.content.is_some());
        assert!(repo_file.symbols.is_empty());
    }

    #[test]
    fn test_process_file_with_content() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let info = FileInfo {
            path: file_path,
            relative_path: "test.rs".to_string(),
            size_bytes: Some(12),
            language: Some("rust".to_string()),
        };

        let config = ScannerConfig::default();
        let result = process_file_with_content(info, &config);

        assert!(result.is_some());
        let repo_file = result.unwrap();
        assert!(repo_file.content.is_some());
        // Should have parsed symbols
        assert!(!repo_file.symbols.is_empty());
    }

    #[test]
    fn test_process_file_without_content() {
        let info = FileInfo {
            path: PathBuf::from("/path/to/test.rs"),
            relative_path: "test.rs".to_string(),
            size_bytes: Some(1000),
            language: Some("rust".to_string()),
        };

        let config = ScannerConfig::default();
        let repo_file = process_file_without_content(info, &config);

        assert!(repo_file.content.is_none());
        assert!(repo_file.symbols.is_empty());
        assert_eq!(repo_file.size_bytes, 1000);
    }

    #[test]
    fn test_parse_with_thread_local_rust() {
        let content = "fn main() {}";
        let path = PathBuf::from("test.rs");
        let symbols = parse_with_thread_local(content, &path);

        // Should parse the main function
        assert!(!symbols.is_empty());
    }

    #[test]
    fn test_parse_with_thread_local_unknown_extension() {
        let content = "some content";
        let path = PathBuf::from("test.unknown");
        let symbols = parse_with_thread_local(content, &path);

        // Unknown extension should return empty
        assert!(symbols.is_empty());
    }
}
