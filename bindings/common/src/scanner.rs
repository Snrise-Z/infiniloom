//! Repository scanner for language bindings
//!
//! This is a pure Rust scanner similar to the CLI's scanner, adapted for bindings.
//! Used by both Python and Node.js bindings.

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::tokenizer::TokenCounts;
use infiniloom_engine::tokenizer::Tokenizer;
use infiniloom_engine::types::{LanguageStats, RepoFile, RepoMetadata, Repository};

// Thread-local parser for each rayon worker
// This avoids mutex contention by giving each thread its own parser
thread_local! {
    static THREAD_PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
    static THREAD_TOKENIZER: Tokenizer = Tokenizer::new();
}

/// Parse content using thread-local parser (lock-free)
fn parse_with_thread_local(content: &str, path: &Path) -> Vec<infiniloom_engine::types::Symbol> {
    THREAD_PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = Language::from_extension(ext) {
                parser.parse(content, lang).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    })
}

/// Count tokens using thread-local tokenizer (accurate via tiktoken)
fn count_tokens_accurate(content: &str) -> TokenCounts {
    THREAD_TOKENIZER.with(|tokenizer| tokenizer.count_all(content))
}

/// Configuration for repository scanning
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

/// Intermediate struct for collecting file info before parallel processing
struct FileInfo {
    path: std::path::PathBuf,
    relative_path: String,
}

/// Scan a repository and return a Repository struct
pub fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Collect file paths first (fast)
    let file_infos = collect_file_paths(path, &config)?;

    // Process files in parallel with thread-local parsers (lock-free)
    let files: Vec<RepoFile> = file_infos
        .into_par_iter()
        .filter_map(|file_info| process_file(file_info, &config).ok().flatten())
        .collect();

    // Calculate statistics
    let total_files = files.len() as u32;
    let mut total_lines: u64 = 0;
    let mut total_tokens = TokenCounts::default();
    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new(); // (files, lines)

    for file in &files {
        if let Some(ref content) = file.content {
            let lines = content.lines().count() as u64;
            total_lines += lines;

            // Accumulate token counts
            total_tokens.o200k += file.token_count.o200k;
            total_tokens.cl100k += file.token_count.cl100k;
            total_tokens.claude += file.token_count.claude;
            total_tokens.gemini += file.token_count.gemini;
            total_tokens.llama += file.token_count.llama;
            total_tokens.mistral += file.token_count.mistral;
            total_tokens.deepseek += file.token_count.deepseek;
            total_tokens.qwen += file.token_count.qwen;
            total_tokens.cohere += file.token_count.cohere;
            total_tokens.grok += file.token_count.grok;

            if let Some(ref lang) = file.language {
                let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
                entry.0 += 1; // files
                entry.1 += lines; // lines
            }
        }
    }

    // Build language stats
    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, (files, lines))| {
            let percentage = if total_files > 0 {
                (files as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files, lines, percentage }
        })
        .collect();

    let metadata =
        RepoMetadata { total_files, total_lines, total_tokens, languages, ..Default::default() };

    Ok(Repository { name: repo_name, path: path.to_path_buf(), files, metadata })
}

/// Collect file paths without reading contents
fn collect_file_paths(path: &Path, config: &ScanConfig) -> Result<Vec<FileInfo>> {
    let mut file_infos = Vec::new();

    let walker = WalkBuilder::new(path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        // Skip binary files by checking extension
        if is_likely_binary_extension(entry_path) {
            continue;
        }

        // Calculate relative path
        let relative_path = entry_path
            .strip_prefix(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry_path.to_string_lossy().to_string());

        file_infos.push(FileInfo { path: entry_path.to_path_buf(), relative_path });
    }

    Ok(file_infos)
}

/// Process a single file
fn process_file(file_info: FileInfo, config: &ScanConfig) -> Result<Option<RepoFile>> {
    // Check file size
    let metadata = std::fs::metadata(&file_info.path).context("Failed to get file metadata")?;
    if metadata.len() > config.max_file_size {
        return Ok(None);
    }

    // Detect language (use name() for lowercase consistency in APIs)
    let language = file_info
        .path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Language::from_extension)
        .map(|l| l.name().to_string());

    // Read content if requested
    let (content, token_count, symbols, size_bytes) = if config.read_contents {
        let content =
            std::fs::read_to_string(&file_info.path).context("Failed to read file content")?;

        // Skip binary files based on content
        if is_binary_content(&content) {
            return Ok(None);
        }

        let size = content.len() as u64;
        let tokens = count_tokens_accurate(&content);

        // Extract symbols unless skipped
        let symbols = if config.skip_symbols {
            Vec::new()
        } else {
            parse_with_thread_local(&content, &file_info.path)
        };

        (Some(content), tokens, symbols, size)
    } else {
        (None, TokenCounts::default(), Vec::new(), metadata.len())
    };

    Ok(Some(RepoFile {
        path: file_info.path,
        relative_path: file_info.relative_path,
        language,
        size_bytes,
        token_count,
        symbols,
        importance: 0.5, // Default importance, will be recalculated by rank_files
        content,
    }))
}

/// Check if a file is likely binary based on extension
fn is_likely_binary_extension(path: &Path) -> bool {
    let binary_extensions = [
        "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib", "pyc", "pyo", "class", "jar",
        "war", "ear", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "iso", "dmg", "img", "png",
        "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "mp3", "mp4", "avi", "mov", "wmv",
        "flv", "webm", "ogg", "wav", "flac", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
        "woff", "woff2", "ttf", "otf", "eot", "sqlite", "db", "lock", "wasm",
    ];

    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| binary_extensions.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Check if content appears to be binary
fn is_binary_content(content: &str) -> bool {
    // Check first 8KB for null bytes
    let check_size = content.len().min(8192);
    let sample = &content[..check_size];

    // If we find null bytes, it's likely binary
    if sample.contains('\0') {
        return true;
    }

    // Check for high ratio of non-printable characters
    let non_printable = sample
        .chars()
        .filter(|c| !c.is_ascii_graphic() && !c.is_whitespace())
        .count();

    let ratio = non_printable as f64 / check_size as f64;
    ratio > 0.3 // More than 30% non-printable = likely binary
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
}
