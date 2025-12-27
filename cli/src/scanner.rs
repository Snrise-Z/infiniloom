//! Repository scanner for Infiniloom CLI
//!
//! This module wraps the unified scanner from the engine with CLI-specific features:
//! - Incremental caching (skip unchanged files)
//! - Git branch/commit detection
//! - Directory structure generation
//! - External dependency extraction
//!
//! Performance notes:
//! - Uses pipelined architecture for large repos (>100 files)
//! - Memory-mapped I/O for files >= 1MB
//! - Thread-local parsers for lock-free parallel parsing
//! - Use --skip-symbols for 80x speedup on large repos

use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use infiniloom_engine::dependencies::DependencyGraph;
use infiniloom_engine::scanner::{
    collect_file_infos, scan_files_pipelined, smart_read_file_with_options, FileInfo,
    ScannerConfig, PIPELINE_THRESHOLD,
};
use infiniloom_engine::types::{LanguageStats, RepoFile, RepoMetadata, Repository, TokenCounts};
use infiniloom_engine::RepoCache;

/// Configuration for repository scanning (CLI-specific)
///
/// This wraps the engine's ScannerConfig with additional defaults appropriate for CLI use.
pub(crate) struct ScanConfig {
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Read file contents
    pub read_contents: bool,
    /// Maximum file size to include (bytes)
    pub max_file_size: u64,
    /// Skip symbol extraction (faster for large repos)
    pub skip_symbols: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: false,
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
            // CLI uses fast estimation by default
            accurate_tokens: false,
            use_mmap: true,
            use_pipelining: true,
            ..Default::default()
        }
    }
}

/// Scan a repository with cache support, skipping unchanged files
///
/// This function checks each file against the cache and only re-processes files
/// that have changed (different mtime or size). Unchanged files use cached data.
pub(crate) fn scan_repository_with_cache(
    path: &Path,
    config: ScanConfig,
    cache: &RepoCache,
) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;
    let scanner_config: ScannerConfig = config.into();

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Phase 1: Collect file paths
    let file_infos = collect_file_infos(&path, &scanner_config)?;

    // Phase 2: Partition files into cached (unchanged) and needs-rescan
    let mut cached_files: Vec<RepoFile> = Vec::new();
    let mut files_to_scan: Vec<FileInfo> = Vec::new();

    for info in file_infos {
        let size_bytes = info.size_bytes.unwrap_or(0);

        // Get file mtime
        let mtime = std::fs::metadata(&info.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let cached = cache.get(&info.relative_path);
        let mut needs_rescan = cache.needs_rescan(&info.relative_path, mtime, size_bytes);

        if !needs_rescan {
            if let Some(cached) = cached {
                // If symbols are required but were never extracted, force rescan
                if !scanner_config.skip_symbols && !cached.symbols_extracted {
                    needs_rescan = true;
                }

                let mut content = None;
                // Check content hash for cache validation when hash exists
                // This is needed even when read_contents=false (filter-first pattern)
                // because hash-based invalidation detects changes when mtime/size unchanged
                if !needs_rescan && cached.hash != 0 {
                    if let Some(content_str) = smart_read_file_with_options(
                        &info.path,
                        cached.size,
                        scanner_config.use_mmap,
                    ) {
                        let content_hash =
                            infiniloom_engine::incremental::hash_content(content_str.as_bytes());
                        if cache.needs_rescan_with_hash(
                            &info.relative_path,
                            mtime,
                            size_bytes,
                            content_hash,
                        ) {
                            needs_rescan = true;
                        } else if scanner_config.read_contents {
                            // Keep content if we were going to read it anyway
                            content = Some(content_str);
                        }
                    }
                } else if !needs_rescan && scanner_config.read_contents {
                    content = smart_read_file_with_options(
                        &info.path,
                        cached.size,
                        scanner_config.use_mmap,
                    );
                }

                if needs_rescan {
                    files_to_scan.push(info);
                } else {
                    cached_files.push(RepoFile {
                        path: info.path,
                        relative_path: info.relative_path,
                        language: cached.language.clone(),
                        size_bytes: cached.size,
                        token_count: TokenCounts {
                            o200k: cached.tokens.o200k,
                            cl100k: cached.tokens.cl100k,
                            claude: cached.tokens.claude,
                            gemini: cached.tokens.gemini,
                            llama: cached.tokens.llama,
                            mistral: cached.tokens.mistral,
                            deepseek: cached.tokens.deepseek,
                            qwen: cached.tokens.qwen,
                            cohere: cached.tokens.cohere,
                            grok: cached.tokens.grok,
                        },
                        symbols: cached.symbols.iter().map(|s| s.into()).collect(),
                        importance: 0.5,
                        content,
                    });
                }
            } else {
                files_to_scan.push(info);
            }
        } else {
            files_to_scan.push(info);
        }
    }

    // Phase 3: Process only changed files using engine's pipelined scanner
    // Note: Even when read_contents=false (filter-first pattern), we need to read and parse
    // files that need rescanning when symbols are required, otherwise symbols won't be extracted
    let scanned_files = if !files_to_scan.is_empty() {
        if scanner_config.read_contents || !scanner_config.skip_symbols {
            // Need content for token counting or symbol extraction
            scan_files_pipelined(files_to_scan, &scanner_config)?
        } else {
            // Just metadata, no content or symbols needed
            process_files_without_content(files_to_scan, &scanner_config)
        }
    } else {
        Vec::new()
    };

    // Merge cached and scanned files
    let mut files = scanned_files;
    files.extend(cached_files);

    // Phase 4: Add CLI-specific metadata
    let metadata = build_cli_metadata(&path, &files);

    Ok(Repository { name: repo_name, path, files, metadata })
}

/// Scan a repository and return a Repository struct
///
/// Uses the unified scanner from engine with CLI-specific metadata added.
pub(crate) fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;
    let scanner_config: ScannerConfig = config.into();

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Phase 1: Collect file paths
    let file_infos = collect_file_infos(&path, &scanner_config)?;

    // Phase 2: Process files
    let files = if scanner_config.read_contents {
        if scanner_config.skip_symbols || file_infos.len() < PIPELINE_THRESHOLD {
            // Simple parallel processing for small repos or skip-symbols mode
            process_files_parallel(file_infos, &scanner_config)
        } else {
            // Pipelined processing for large repos
            scan_files_pipelined(file_infos, &scanner_config)?
        }
    } else {
        process_files_without_content(file_infos, &scanner_config)
    };

    // Phase 3: Add CLI-specific metadata
    let metadata = build_cli_metadata(&path, &files);

    Ok(Repository { name: repo_name, path, files, metadata })
}

/// Process files in parallel (for small repos or skip-symbols mode)
fn process_files_parallel(file_infos: Vec<FileInfo>, config: &ScannerConfig) -> Vec<RepoFile> {
    use infiniloom_engine::scanner::{process_file_content_only, process_file_with_content};
    use rayon::prelude::*;

    if config.skip_symbols {
        file_infos
            .into_par_iter()
            .filter_map(|info| process_file_content_only(info, config))
            .collect()
    } else {
        file_infos
            .into_par_iter()
            .filter_map(|info| process_file_with_content(info, config))
            .collect()
    }
}

/// Process files without reading content
fn process_files_without_content(
    file_infos: Vec<FileInfo>,
    config: &ScannerConfig,
) -> Vec<RepoFile> {
    use infiniloom_engine::scanner::process_file_without_content;

    file_infos
        .into_iter()
        .map(|info| process_file_without_content(info, config))
        .collect()
}

/// Build CLI-specific metadata (git info, dependencies, directory structure)
fn build_cli_metadata(path: &Path, files: &[RepoFile]) -> RepoMetadata {
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

    // Track language statistics
    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new();
    for file in files {
        if let Some(ref lang) = file.language {
            let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1;
            let file_lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(file.size_bytes));
            entry.1 += file_lines;
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
    languages.sort_by(|a, b| b.files.cmp(&a.files));

    // Sum token counts
    let total_tokens = TokenCounts {
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

    // CLI-specific: Git info
    let branch = detect_git_branch(path);
    let commit = detect_git_commit(path);

    // CLI-specific: Directory structure
    let directory_structure = generate_directory_structure(files);

    // CLI-specific: External dependencies
    let temp_repo = Repository {
        name: String::new(),
        path: path.to_path_buf(),
        files: files.to_vec(),
        metadata: RepoMetadata::default(),
    };
    let dep_graph = DependencyGraph::build(&temp_repo);
    let mut external_dependencies: Vec<String> =
        dep_graph.get_external_deps().iter().cloned().collect();
    external_dependencies.sort();

    RepoMetadata {
        total_files,
        total_lines,
        total_tokens,
        languages,
        framework: None,
        description: None,
        branch,
        commit,
        directory_structure: Some(directory_structure),
        external_dependencies,
        git_history: None,
    }
}

/// Estimate lines from file size
fn estimate_lines(size_bytes: u64) -> u64 {
    // Average ~40 characters per line
    size_bytes / 40
}

/// Detect current git branch
fn detect_git_branch(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: refs/heads/") {
        Some(
            content
                .trim_start_matches("ref: refs/heads/")
                .trim()
                .to_owned(),
        )
    } else {
        // Detached HEAD - safely take first 7 characters
        Some(content.trim().chars().take(7).collect())
    }
}

/// Detect current git commit
fn detect_git_commit(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: ") {
        // Follow ref
        let ref_path = content.trim_start_matches("ref: ").trim();
        let full_path = path.join(".git").join(ref_path);
        std::fs::read_to_string(full_path)
            .ok()
            .map(|s| s.trim().chars().take(7).collect())
    } else {
        // Detached HEAD
        Some(content.trim().chars().take(7).collect())
    }
}

/// Generate a tree-like directory structure from file paths
fn generate_directory_structure(files: &[RepoFile]) -> String {
    // Collect all unique directory paths
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let mut file_set: BTreeSet<&str> = BTreeSet::new();

    for file in files {
        file_set.insert(&file.relative_path);

        let mut current = file.relative_path.as_str();
        while let Some(idx) = current.rfind('/') {
            current = &current[..idx];
            if !current.is_empty() {
                dirs.insert(current.to_owned());
            }
        }
    }

    // Build tree structure
    let mut output = String::new();
    let mut printed: BTreeSet<String> = BTreeSet::new();

    let mut all_paths: Vec<(&str, bool)> = Vec::new();
    for dir in &dirs {
        all_paths.push((dir, true));
    }
    for file in files {
        all_paths.push((&file.relative_path, false));
    }
    all_paths.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.0.split('/').collect();
        let b_parts: Vec<&str> = b.0.split('/').collect();
        a_parts.cmp(&b_parts)
    });

    for (path, is_dir) in all_paths {
        let parts: Vec<&str> = path.split('/').collect();
        let depth = parts.len() - 1;

        let mut parent_path = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i < parts.len() - 1 {
                if !parent_path.is_empty() {
                    parent_path.push('/');
                }
                parent_path.push_str(part);

                if !printed.contains(&parent_path) {
                    let indent = "  ".repeat(i);
                    output.push_str(&format!("{}{}/\n", indent, part));
                    printed.insert(parent_path.clone());
                }
            }
        }

        if !is_dir {
            let name = parts.last().unwrap_or(&"");
            let indent = "  ".repeat(depth);
            output.push_str(&format!("{}{}\n", indent, name));
        }
    }

    // Limit size for very large repos
    if output.len() > 50000 {
        let truncated: String = output.chars().take(49000).collect();
        format!("{}...\n[Directory structure truncated - {} files total]", truncated, files.len())
    } else {
        output
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scan_config_default() {
        let config = ScanConfig::default();
        assert!(!config.include_hidden);
        assert!(config.respect_gitignore);
        assert!(!config.read_contents);
        assert_eq!(config.max_file_size, 50 * 1024 * 1024);
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
        // CLI defaults
        assert!(!scanner_config.accurate_tokens);
        assert!(scanner_config.use_mmap);
    }

    #[test]
    fn test_estimate_lines() {
        assert_eq!(estimate_lines(400), 10);
        assert_eq!(estimate_lines(0), 0);
    }

    #[test]
    fn test_detect_file_language() {
        use infiniloom_engine::parser::detect_file_language;
        assert_eq!(detect_file_language(&PathBuf::from("test.py")), Some("python".to_string()));
        assert_eq!(detect_file_language(&PathBuf::from("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_file_language(&PathBuf::from("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_file_language(&PathBuf::from("test")), None);
    }

    #[test]
    fn test_is_binary_extension() {
        use infiniloom_engine::scanner::is_binary_extension;
        assert!(is_binary_extension(&PathBuf::from("test.exe")));
        assert!(is_binary_extension(&PathBuf::from("test.png")));
        assert!(!is_binary_extension(&PathBuf::from("test.rs")));
        assert!(!is_binary_extension(&PathBuf::from("test.py")));
    }
}
