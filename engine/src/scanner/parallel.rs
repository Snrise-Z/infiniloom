//! Unified scanner implementation
//!
//! This module provides the main [`UnifiedScanner`] struct that handles
//! repository scanning with configurable features. It automatically chooses
//! between pipelined and simple parallel processing based on repository size.

use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use crate::types::{LanguageStats, RepoFile, RepoMetadata, Repository};

use super::pipelined::scan_files_pipelined;
use super::process::{
    estimate_lines, process_file_content_only, process_file_with_content,
    process_file_without_content,
};
use super::walk::collect_file_infos;
use super::{FileInfo, ScannerConfig, PIPELINE_THRESHOLD};

/// Unified scanner for repository scanning
///
/// This scanner combines the best features of both CLI and bindings scanners:
/// - Pipelined architecture for large repositories
/// - Configurable token counting (accurate vs fast)
/// - Memory-mapped I/O for large files
/// - Batching to prevent stack overflow
///
/// # Example
///
/// ```ignore
/// use infiniloom_engine::scanner::{UnifiedScanner, ScannerConfig};
///
/// let scanner = UnifiedScanner::new(ScannerConfig::default());
/// let repo = scanner.scan(Path::new("/path/to/repo"))?;
/// ```
#[derive(Debug, Clone)]
pub struct UnifiedScanner {
    config: ScannerConfig,
}

impl UnifiedScanner {
    /// Create a new scanner with the given configuration
    pub fn new(config: ScannerConfig) -> Self {
        Self { config }
    }

    /// Create a scanner with default (fast) settings
    pub fn fast() -> Self {
        Self::new(ScannerConfig::fast())
    }

    /// Create a scanner with accurate token counting
    pub fn accurate() -> Self {
        Self::new(ScannerConfig::accurate())
    }

    /// Scan a repository and return a Repository struct
    pub fn scan(&self, path: &Path) -> Result<Repository> {
        scan_repository(path, self.config.clone())
    }

    /// Get the current configuration
    pub fn config(&self) -> &ScannerConfig {
        &self.config
    }
}

impl Default for UnifiedScanner {
    fn default() -> Self {
        Self::new(ScannerConfig::default())
    }
}

/// Scan a repository and return a Repository struct
///
/// Uses parallel processing for improved performance on large repositories.
/// For large repos (>100 files), uses a pipelined architecture with channels
/// to overlap I/O with CPU-intensive parsing work.
///
/// # Arguments
/// * `path` - Path to the repository root
/// * `config` - Scanner configuration
///
/// # Returns
/// A Repository struct containing all scanned files and metadata
pub fn scan_repository(path: &Path, config: ScannerConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Phase 1: Collect file paths (fast, sequential walk with ignore filtering)
    let file_infos = collect_file_infos(&path, &config)?;

    // Phase 2: Process files
    let files = process_files(file_infos, &config)?;

    // Phase 3: Aggregate statistics
    let metadata = compute_metadata(&files);

    Ok(Repository { name: repo_name, path, files, metadata })
}

/// Process files using the appropriate strategy based on count and config
fn process_files(file_infos: Vec<FileInfo>, config: &ScannerConfig) -> Result<Vec<RepoFile>> {
    let file_count = file_infos.len();

    if !config.read_contents {
        // Sequential is fine when just collecting metadata (CPU bound, fast)
        return Ok(file_infos
            .into_iter()
            .map(|info| process_file_without_content(info, config))
            .collect());
    }

    if config.skip_symbols {
        // Without symbols, use batched parallel processing
        return Ok(process_files_batched(file_infos, config, |info, cfg| {
            process_file_content_only(info, cfg)
        }));
    }

    // With symbols: choose between pipelined and simple parallel
    if config.use_pipelining && file_count >= PIPELINE_THRESHOLD {
        // Large repo: use pipelined architecture
        scan_files_pipelined(file_infos, config)
    } else {
        // Small repo: use batched parallel with thread-local parsers
        Ok(process_files_batched(file_infos, config, |info, cfg| {
            process_file_with_content(info, cfg)
        }))
    }
}

/// Process files in batches to prevent stack overflow on large repos
///
/// Rayon's work-stealing can exhaust stack space with 75K+ files.
fn process_files_batched<F>(
    file_infos: Vec<FileInfo>,
    config: &ScannerConfig,
    processor: F,
) -> Vec<RepoFile>
where
    F: Fn(FileInfo, &ScannerConfig) -> Option<RepoFile> + Sync,
{
    let batch_size = config.batch_size;

    if file_infos.len() <= batch_size {
        // Small repo: process all at once
        file_infos
            .into_par_iter()
            .filter_map(|info| processor(info, config))
            .collect()
    } else {
        // Large repo: process in batches
        let mut all_files = Vec::with_capacity(file_infos.len());
        for chunk in file_infos.chunks(batch_size) {
            let batch_files: Vec<RepoFile> = chunk
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_par_iter()
                .filter_map(|info| processor(info, config))
                .collect();
            all_files.extend(batch_files);
        }
        all_files
    }
}

/// Compute metadata from processed files
fn compute_metadata(files: &[RepoFile]) -> RepoMetadata {
    let total_files = files.len() as u32;

    let total_lines: u64 = files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(f.size_bytes))
        })
        .sum();

    // Track both file counts and line counts per language
    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new();
    for file in files {
        if let Some(ref lang) = file.language {
            let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1; // file count
            let file_lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(file.size_bytes));
            entry.1 += file_lines; // line count
        }
    }

    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, (count, lines))| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    // Sort by file count descending so primary language is deterministic
    languages.sort_by(|a, b| b.files.cmp(&a.files));

    // Sum token counts from all files
    let total_tokens = crate::tokenizer::TokenCounts {
        o200k: files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
        mistral: files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: files.iter().map(|f| f.token_count.cohere).sum(),
        grok: files.iter().map(|f| f.token_count.grok).sum(),
    };

    RepoMetadata {
        total_files,
        total_lines,
        total_tokens,
        languages,
        framework: None,
        description: None,
        branch: None,
        commit: None,
        directory_structure: None,
        external_dependencies: Vec::new(),
        git_history: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_unified_scanner_default() {
        let scanner = UnifiedScanner::default();
        assert!(!scanner.config().accurate_tokens);
        assert!(scanner.config().use_mmap);
    }

    #[test]
    fn test_unified_scanner_fast() {
        let scanner = UnifiedScanner::fast();
        assert!(!scanner.config().accurate_tokens);
    }

    #[test]
    fn test_unified_scanner_accurate() {
        let scanner = UnifiedScanner::accurate();
        assert!(scanner.config().accurate_tokens);
    }

    #[test]
    fn test_scan_repository_empty() {
        let dir = tempdir().unwrap();
        let config = ScannerConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();
        assert_eq!(repo.files.len(), 0);
        assert_eq!(repo.metadata.total_files, 0);
    }

    #[test]
    fn test_scan_repository_single_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let config = ScannerConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert_eq!(repo.metadata.total_files, 1);
        assert!(repo.files[0].content.is_some());
    }

    #[test]
    fn test_scan_repository_multiple_languages() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("app.py"), "def main(): pass").unwrap();
        fs::write(dir.path().join("index.ts"), "const x = 1;").unwrap();

        let config = ScannerConfig::default();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 3);
        assert_eq!(repo.metadata.languages.len(), 3);
    }

    #[test]
    fn test_scan_repository_skip_symbols() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig { skip_symbols: true, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].symbols.is_empty());
    }

    #[test]
    fn test_scan_repository_no_content() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig { read_contents: false, ..Default::default() };
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].content.is_none());
    }

    #[test]
    fn test_scan_repository_accurate_tokens() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig::accurate();
        let repo = scan_repository(dir.path(), config).unwrap();

        assert_eq!(repo.files.len(), 1);
        // Token count should be populated
        assert!(repo.files[0].token_count.o200k > 0);
    }

    #[test]
    fn test_compute_metadata() {
        let files = vec![RepoFile {
            path: std::path::PathBuf::from("test.rs"),
            relative_path: "test.rs".to_string(),
            language: Some("rust".to_string()),
            size_bytes: 100,
            token_count: crate::tokenizer::TokenCounts {
                o200k: 25,
                cl100k: 27,
                claude: 28,
                gemini: 26,
                llama: 28,
                mistral: 28,
                deepseek: 28,
                qwen: 28,
                cohere: 27,
                grok: 28,
            },
            symbols: vec![],
            importance: 0.5,
            content: Some("fn main() {\n    println!(\"hello\");\n}".to_string()),
        }];

        let metadata = compute_metadata(&files);

        assert_eq!(metadata.total_files, 1);
        assert_eq!(metadata.total_lines, 3);
        assert_eq!(metadata.total_tokens.o200k, 25);
        assert_eq!(metadata.languages.len(), 1);
        assert_eq!(metadata.languages[0].language, "rust");
    }

    #[test]
    fn test_process_files_batched() {
        let dir = tempdir().unwrap();

        // Create 10 files
        for i in 0..10 {
            fs::write(dir.path().join(format!("test{}.rs", i)), "fn main() {}").unwrap();
        }

        let infos: Vec<FileInfo> = (0..10)
            .map(|i| FileInfo {
                path: dir.path().join(format!("test{}.rs", i)),
                relative_path: format!("test{}.rs", i),
                size_bytes: Some(12),
                language: Some("rust".to_string()),
            })
            .collect();

        let config = ScannerConfig {
            batch_size: 3, // Small batch for testing
            ..Default::default()
        };

        let files =
            process_files_batched(infos, &config, |info, cfg| process_file_content_only(info, cfg));

        assert_eq!(files.len(), 10);
    }
}
