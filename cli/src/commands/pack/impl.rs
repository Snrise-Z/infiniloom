//! Pack command handler
//!
//! Packs a repository into LLM-friendly format with intelligent compression,
//! symbol ranking, and model-specific optimizations.

use anyhow::{Context, Result};
use colored::Colorize;
use humansize::{format_size, BINARY};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, BufRead};
use std::time::Instant;

use infiniloom_engine::{
    git::GitRepo,
    output::{OutputFormat, OutputFormatter},
    remote::RemoteRepo,
    repomap::RepoMapGenerator,
    security::SecurityScanner,
    tokenizer::Tokenizer,
    types::{CompressionLevel, TokenizerModel},
};

use crate::config::load_config_file;
use crate::scanner;

use super::{
    // Output formatting functions (from output module)
    apply_pack_extras,
    // Token budget functions (from budget module)
    enforce_budget,
    estimate_tokens,
    // Content transformation functions (from compression module)
    extract_key_symbols_focused,
    extract_key_symbols_only,
    extract_signatures_only,
    rank_files_fast,
    read_instruction_file,
    recalculate_metadata,
    remove_comments_from_content,
    remove_empty_lines_from_content,
    truncate_to_tokens,
    update_repo_cache,
    // Configuration type
    PackConfig,
};

/// Pack a repository into LLM-friendly format
///
/// This function implements the core pack command functionality, transforming a codebase
/// into an LLM-optimized format with intelligent compression, symbol ranking, and
/// model-specific optimizations.
///
/// # Arguments
///
/// * `config` - A `PackConfig` instance containing all pack options. Use
///   `PackConfig::builder()` to construct the configuration with a fluent API.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - The repository path doesn't exist
/// - File reading fails
/// - Output file cannot be written
/// - Security check fails (when `fail_on_secrets` is enabled)
///
/// # Examples
///
/// Basic usage with minimal configuration:
///
/// ```no_run
/// use infiniloom_cli::commands::pack::{PackConfig, cmd_pack};
/// use std::path::PathBuf;
///
/// let config = PackConfig::builder()
///     .path(PathBuf::from("/path/to/repo"))
///     .build()
///     .expect("Failed to build config");
///
/// cmd_pack(config).expect("Pack failed");
/// ```
///
/// Advanced usage with custom options:
///
/// ```no_run
/// use infiniloom_cli::commands::pack::{
///     PackConfig, OutputOptions, ScanOptions, cmd_pack
/// };
/// use infiniloom_engine::output::OutputFormat;
/// use infiniloom_engine::types::{CompressionLevel, TokenizerModel};
/// use std::path::PathBuf;
///
/// let config = PackConfig::builder()
///     .path(PathBuf::from("/path/to/repo"))
///     .output(OutputOptions {
///         format: Some(OutputFormat::Xml),
///         model: Some(TokenizerModel::Claude),
///         compression: Some(CompressionLevel::Balanced),
///         max_tokens: 100000,
///         output_file: Some(PathBuf::from("output.xml")),
///         show_line_numbers: true,
///         ..Default::default()
///     })
///     .scan(ScanOptions {
///         enable_symbols: true,
///         remove_comments: true,
///         include_patterns: vec!["src/**/*.rs".to_string()],
///         ..Default::default()
///     })
///     .verbose(true)
///     .build()
///     .expect("Failed to build config");
///
/// cmd_pack(config).expect("Pack failed");
/// ```
///
/// # Implementation Details
///
/// The function performs the following steps:
///
/// 1. **Repository Scanning**: Walks the directory tree, respects .gitignore
/// 2. **Symbol Extraction**: Uses Tree-sitter to parse code and extract symbols
/// 3. **File Filtering**: Applies include/exclude patterns, default ignores
/// 4. **Content Transformation**: Removes comments, empty lines, truncates base64
/// 5. **Security Scanning**: Detects and optionally redacts secrets
/// 6. **Symbol Ranking**: Uses PageRank to identify important symbols
/// 7. **Token Budget Enforcement**: Truncates content to fit model limits
/// 8. **Output Generation**: Formats output for target LLM model
/// 9. **Watch Mode**: Optionally watches for file changes and regenerates
///
/// # Performance Characteristics
///
/// - **Parallel processing**: Uses Rayon for concurrent file parsing
/// - **Incremental caching**: Reuses cached symbols for unchanged files
/// - **Thread-local parsers**: Eliminates mutex contention during parsing
/// - **Memory-efficient**: Streams large files, avoids loading entire repo in memory
///
/// # Error Scenarios
///
/// Common error scenarios and their causes:
///
/// - **"Failed to scan repository"**: Invalid path or permission issues
/// - **"Secrets detected with fail_on_secrets enabled"**: Security check failure
/// - **"Failed to write output file"**: Output path not writable
/// - **"Failed to clone repository"**: Remote URL invalid or network error
///
/// # Migration from 78-Parameter Function
///
/// Before (deprecated):
/// ```ignore
/// cmd_pack(path, format, model, compression, max_tokens, output, ... 73 more args)
/// ```
///
/// After (current):
/// ```no_run
/// use infiniloom_cli::commands::pack::{PackConfig, OutputOptions};
/// use std::path::PathBuf;
///
/// let config = PackConfig::builder()
///     .path(PathBuf::from("/path/to/repo"))
///     .output(OutputOptions {
///         max_tokens: 100000,
///         ..Default::default()
///     })
///     .build()
///     .expect("Failed to build config");
///
/// infiniloom_cli::commands::pack::cmd_pack(config).expect("Pack failed");
/// ```
pub(crate) fn cmd_pack(config: PackConfig) -> Result<()> {
    let start = Instant::now();

    // Extract frequently used values from config for convenience
    let path = config.path.clone();
    let cli_format = config.output.format;
    let cli_model = config.output.model;
    let cli_compression = config.output.compression;
    let max_tokens = config.output.max_tokens;
    let output = config.output.output_file.clone();
    let include_hidden = config.scan.include_hidden;
    let respect_gitignore = config.scan.respect_gitignore;
    let enable_symbols = config.scan.enable_symbols;
    let full_mode = config.scan.full_mode;
    let exclude_content = config.scan.exclude_content;
    let include_tests = config.scan.include_tests;
    let include_docs = config.scan.include_docs;
    let use_default_ignores = config.scan.use_default_ignores;
    let verbose = config.verbose;
    let header_text = config.output.header_text.clone();
    let instruction_file = config.output.instruction_file.clone();
    let copy_to_clipboard = config.output.copy_to_clipboard;
    let token_tree = config.output.token_tree;
    let show_directory_structure = config.output.show_directory_structure;
    let show_file_summary = config.output.show_file_summary;
    let remove_empty_lines = config.scan.remove_empty_lines;
    let remove_comments = config.scan.remove_comments;
    let top_files = config.scan.top_files;
    let include_logs = config.git.include_logs;
    let logs_count = config.git.logs_count;
    let include_diffs = config.git.include_diffs;
    let sort_by_changes = config.git.sort_by_changes;
    let stdin = config.scan.stdin;
    let should_truncate_base64 = config.scan.truncate_base64;
    let include_patterns = config.scan.include_patterns.clone();
    let exclude_patterns = config.scan.exclude_patterns.clone();
    let security_check = config.security.security_check;
    let remote_branch = config.git.remote_branch.clone();
    let sparse_paths = config.git.sparse_paths.clone();
    let show_line_numbers = config.output.show_line_numbers;
    let redact_secrets = config.security.redact_secrets;
    let config_path = config.config_path.clone();
    let watch_mode = config.watch.enabled;
    let incremental_cache = config.scan.incremental_cache;
    let map_budget = config.map_budget;

    // Handle stdin mode - read file paths from stdin
    let stdin_paths: Option<Vec<String>> = if stdin {
        let stdin_handle = io::stdin();
        let paths: Vec<String> = stdin_handle
            .lock()
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    } else {
        None
    };

    if verbose {
        eprintln!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
        eprintln!();
    }

    // Create progress bar
    let pb = if verbose {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("Scanning repository...");
        Some(pb)
    } else {
        None
    };

    // Handle remote URL - clone if needed
    let (repo_path, _temp_dir) = if RemoteRepo::is_remote_url(path.to_string_lossy().as_ref()) {
        if let Some(pb) = &pb {
            pb.set_message("Cloning remote repository...");
        }
        let mut remote = RemoteRepo::parse(path.to_string_lossy().as_ref())
            .map_err(|e| anyhow::anyhow!("Invalid remote URL: {}", e))?;

        if let Some(ref branch) = remote_branch {
            remote.branch = Some(branch.clone());
        }

        if verbose {
            let branch_info = remote.branch.as_deref().unwrap_or("default");
            if sparse_paths.is_empty() {
                eprintln!(
                    "  Cloning {} from {:?} (branch: {})...",
                    remote.name, remote.provider, branch_info
                );
            } else {
                eprintln!(
                    "  Sparse cloning {} from {:?} (branch: {}, paths: {:?})...",
                    remote.name, remote.provider, branch_info, sparse_paths
                );
            }
        }

        let (cloned_path, temp_dir) = if !sparse_paths.is_empty() {
            let temp_dir = tempfile::TempDir::with_prefix("infiniloom-sparse-")
                .map_err(|e| anyhow::anyhow!("Failed to create temp dir: {}", e))?;
            let paths_refs: Vec<&str> = sparse_paths.iter().map(|s| s.as_str()).collect();
            let cloned = remote
                .sparse_clone(&paths_refs, Some(temp_dir.path()))
                .map_err(|e| anyhow::anyhow!("Failed to sparse clone repository: {}", e))?;
            (cloned, temp_dir)
        } else {
            remote
                .clone_with_cleanup()
                .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?
        };

        let uses_history_features = include_logs || sort_by_changes || include_diffs;
        if uses_history_features {
            eprintln!(
                "{} Remote repositories are cloned with --depth 1 (shallow clone).",
                "Warning:".yellow().bold()
            );
            eprintln!("         History-dependent options may return incomplete results.");
            eprintln!();
        }

        (cloned_path, Some(temp_dir))
    } else {
        (path, None)
    };

    // Load config file
    let loaded_config = load_config_file(config_path.as_ref(), &repo_path);

    // Apply config defaults
    let mut include_tests = include_tests;
    let mut include_docs = include_docs;
    let mut security_check = security_check;
    let mut show_line_numbers = show_line_numbers;
    let mut show_directory_structure = show_directory_structure;
    let mut show_file_summary = show_file_summary;
    let mut remove_empty_lines = remove_empty_lines;
    let mut remove_comments = remove_comments;
    let mut max_tokens = max_tokens;
    let mut include_hidden = include_hidden;
    let mut max_file_size = 50 * 1024 * 1024u64;

    // Apply config file values
    if !include_tests {
        include_tests = loaded_config.include_tests.unwrap_or(false);
    }
    if !include_docs {
        include_docs = loaded_config.include_docs.unwrap_or(false);
    }
    if !security_check {
        security_check = loaded_config.security_check.unwrap_or(false);
    }
    if !include_hidden {
        include_hidden = loaded_config.include_hidden.unwrap_or(false);
    }
    if let Some(size) = loaded_config.max_file_size {
        max_file_size = size;
    }
    let fail_on_secrets = loaded_config.fail_on_secrets.unwrap_or(false);
    let security_allowlist = loaded_config.security_allowlist.clone();
    let security_custom_patterns = loaded_config.security_custom_patterns.clone();
    let redact_secrets = if redact_secrets {
        true
    } else {
        loaded_config.redact_secrets.unwrap_or(false)
    };
    if let Some(ln) = loaded_config.line_numbers {
        if show_line_numbers {
            show_line_numbers = ln;
        }
    }
    if let Some(ds) = loaded_config.show_directory_structure {
        if show_directory_structure {
            show_directory_structure = ds;
        }
    }
    if let Some(fs) = loaded_config.show_file_summary {
        if show_file_summary {
            show_file_summary = fs;
        }
    }
    if !remove_empty_lines {
        remove_empty_lines = loaded_config.remove_empty_lines.unwrap_or(false);
    }
    if !remove_comments {
        remove_comments = loaded_config.remove_comments.unwrap_or(false);
    }
    if max_tokens == 0 {
        if let Some(budget) = loaded_config.token_budget {
            max_tokens = budget;
        }
    }

    // Resolve format/model/compression
    let format: OutputFormat = if let Some(f) = cli_format {
        f
    } else if let Some(ref fmt_str) = loaded_config.format {
        match fmt_str.to_lowercase().as_str() {
            "markdown" | "md" => OutputFormat::Markdown,
            "json" => OutputFormat::Json,
            "yaml" | "yml" => OutputFormat::Yaml,
            "plain" | "text" | "txt" => OutputFormat::Plain,
            "toon" => OutputFormat::Toon,
            _ => OutputFormat::Xml,
        }
    } else {
        OutputFormat::Xml
    };

    let model: TokenizerModel = if let Some(m) = cli_model {
        m
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let compression: CompressionLevel = if let Some(c) = cli_compression {
        c
    } else if let Some(ref comp_str) = loaded_config.compression {
        CompressionLevel::from_str(comp_str).unwrap_or(CompressionLevel::Balanced)
    } else {
        CompressionLevel::Balanced
    };

    // STEP 1: Fast file list without reading content (for filtering)
    let scan_config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: false, // Don't read content yet - filter first!
        max_file_size,
        skip_symbols: !enable_symbols,
    };

    // Load cache if incremental caching is enabled
    let cache_path = infiniloom_engine::RepoCache::default_cache_path(&repo_path);
    let mut repo_cache = if incremental_cache {
        if let Some(pb) = &pb {
            pb.set_message("Loading cache...");
        }
        infiniloom_engine::RepoCache::load(&cache_path).ok()
    } else {
        None
    };

    let mut repo = if let Some(ref cache) = repo_cache {
        if let Some(pb) = &pb {
            pb.set_message("Scanning with incremental cache...");
        }
        scanner::scan_repository_with_cache(&repo_path, scan_config, cache)
            .context("Failed to scan repository")?
    } else {
        scanner::scan_repository(&repo_path, scan_config).context("Failed to scan repository")?
    };

    // Apply default ignores
    if use_default_ignores {
        use infiniloom_engine::default_ignores::{
            matches_any, DEFAULT_IGNORES, DOC_IGNORES, TEST_IGNORES,
        };

        repo.files.retain(|f| {
            if matches_any(&f.relative_path, DEFAULT_IGNORES) {
                return false;
            }
            if !include_tests && matches_any(&f.relative_path, TEST_IGNORES) {
                return false;
            }
            if !include_docs && matches_any(&f.relative_path, DOC_IGNORES) {
                return false;
            }
            true
        });
    }

    // Filter to stdin paths if provided
    if let Some(ref paths) = stdin_paths {
        repo.files.retain(|f| {
            paths
                .iter()
                .any(|p| f.relative_path == *p || f.relative_path.ends_with(p))
        });
    }

    // Apply include/exclude patterns using centralized filtering with pattern caching
    let all_include_patterns: Vec<String> = include_patterns
        .into_iter()
        .chain(loaded_config.include_patterns)
        .collect();
    let all_exclude_patterns: Vec<String> = exclude_patterns
        .into_iter()
        .chain(loaded_config.exclude_patterns)
        .collect();

    // Apply centralized filtering (uses OnceLock pattern cache for performance)
    infiniloom_engine::filtering::apply_exclude_patterns(
        &mut repo.files,
        &all_exclude_patterns,
        |f| &f.relative_path,
    );
    infiniloom_engine::filtering::apply_include_patterns(
        &mut repo.files,
        &all_include_patterns,
        |f| &f.relative_path,
    );

    // STEP 2: Now read content only for filtered files (much faster!)
    {
        use rayon::prelude::*;
        repo.files.par_iter_mut().for_each(|file| {
            if let Ok(content) = std::fs::read_to_string(&file.path) {
                file.content = Some(content);
            }
        });
    }

    // Update cache AFTER content is read so hash is computed correctly
    if incremental_cache {
        let cache = repo_cache.get_or_insert_with(|| {
            infiniloom_engine::RepoCache::new(repo_path.to_string_lossy().as_ref())
        });
        update_repo_cache(cache, &repo, enable_symbols);
        if let Err(e) = cache.save(&cache_path) {
            if verbose {
                eprintln!("{} Failed to save cache: {}", "⚠".yellow(), e);
            }
        }
    }

    // Strip file contents if --no-content
    if exclude_content {
        for file in &mut repo.files {
            file.content = None;
            file.token_count = infiniloom_engine::types::TokenCounts::default();
        }
    }

    recalculate_metadata(&mut repo);

    if let Some(pb) = &pb {
        pb.set_message(format!("Found {} files", repo.files.len()));
    }

    // Sort/rank files
    if sort_by_changes {
        if let Ok(git_repo) = GitRepo::open(&repo_path) {
            let mut file_changes: Vec<(String, u32)> = repo
                .files
                .iter()
                .map(|f| {
                    let freq = git_repo
                        .file_change_frequency(&f.relative_path, 90)
                        .unwrap_or(0);
                    (f.relative_path.clone(), freq)
                })
                .collect();

            file_changes.sort_by(|a, b| b.1.cmp(&a.1));

            let order_map: std::collections::HashMap<String, usize> = file_changes
                .iter()
                .enumerate()
                .map(|(i, (path, _))| (path.clone(), i))
                .collect();

            repo.files.sort_by_key(|f| {
                order_map
                    .get(&f.relative_path)
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }
    } else if full_mode {
        infiniloom_engine::rank_files(&mut repo);
        infiniloom_engine::sort_files_by_importance(&mut repo);
    } else {
        rank_files_fast(&mut repo);
    }

    // Limit to top N files
    if top_files > 0 && repo.files.len() > top_files {
        repo.files.truncate(top_files);
        recalculate_metadata(&mut repo);
    }

    // Apply content transformations
    let should_remove_comments = remove_comments
        || matches!(
            compression,
            CompressionLevel::Balanced | CompressionLevel::Aggressive | CompressionLevel::Extreme
        );
    let should_remove_empty = remove_empty_lines
        || matches!(
            compression,
            CompressionLevel::Minimal
                | CompressionLevel::Balanced
                | CompressionLevel::Aggressive
                | CompressionLevel::Extreme
        );

    // Apply content transformations in parallel
    {
        use rayon::prelude::*;

        let semantic_compressor = if compression == CompressionLevel::Semantic {
            Some(infiniloom_engine::HeuristicCompressor::new())
        } else {
            None
        };

        repo.files.par_iter_mut().for_each(|file| {
            if let Some(ref mut content) = file.content {
                match compression {
                    CompressionLevel::Aggressive => {
                        if let Some(lang) = &file.language {
                            *content = extract_signatures_only(content, lang, &file.symbols);
                        }
                    },
                    CompressionLevel::Extreme => {
                        if let Some(lang) = &file.language {
                            *content = extract_key_symbols_only(content, lang, &file.symbols);
                        }
                    },
                    CompressionLevel::Focused => {
                        if let Some(lang) = &file.language {
                            *content = extract_key_symbols_focused(content, lang, &file.symbols);
                        }
                    },
                    CompressionLevel::Semantic => {
                        if let Some(ref compressor) = semantic_compressor {
                            if let Ok(compressed) = compressor.compress(content) {
                                *content = compressed;
                            }
                        }
                    },
                    _ => {
                        if should_remove_empty {
                            *content = remove_empty_lines_from_content(content, show_line_numbers);
                        }
                        if should_remove_comments {
                            if let Some(lang) = &file.language {
                                *content =
                                    remove_comments_from_content(content, lang, show_line_numbers);
                            }
                        }
                    },
                }
                if should_truncate_base64 {
                    *content = infiniloom_engine::content_processing::truncate_base64(content);
                }
            }
        });
    }

    // Security scan
    let security_issues = if security_check || redact_secrets {
        if let Some(pb) = &pb {
            pb.set_message("Scanning for security issues...");
        }
        use rayon::prelude::*;

        let mut scanner = SecurityScanner::new();
        for pattern in &security_allowlist {
            scanner.allowlist(pattern);
        }
        if let Err(e) = scanner.add_custom_patterns(&security_custom_patterns) {
            eprintln!("⚠ Warning: Invalid custom security pattern: {}", e);
        }

        let all_issues: Vec<_> = repo
            .files
            .par_iter_mut()
            .filter_map(|file| {
                if let Some(content) = &file.content {
                    let (redacted_content, file_issues) =
                        scanner.scan_and_redact(content, &file.relative_path);
                    file.content = Some(redacted_content);
                    if file_issues.is_empty() {
                        None
                    } else {
                        Some(file_issues)
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        Some(all_issues)
    } else {
        None
    };

    // Check fail_on_secrets
    if fail_on_secrets {
        if let Some(ref issues) = security_issues {
            if !issues.is_empty() {
                if let Some(pb) = &pb {
                    pb.finish_and_clear();
                }
                eprintln!(
                    "{}",
                    format!("❌ Security check failed: found {} potential secrets", issues.len())
                        .red()
                );
                for issue in issues.iter().take(10) {
                    eprintln!(
                        "  • {} (line {}): {} [{:?}]",
                        issue.file.yellow(),
                        issue.line,
                        issue.kind.name(),
                        issue.severity
                    );
                }
                if issues.len() > 10 {
                    eprintln!("  ... and {} more", issues.len() - 10);
                }
                anyhow::bail!("Secrets detected with fail_on_secrets enabled");
            }
        }
    }

    // Recompute token counts after transformations (parallelized for performance)
    {
        use rayon::prelude::*;
        let tokenizer = Tokenizer::new();
        repo.files.par_iter_mut().for_each(|file| {
            if let Some(ref content) = file.content {
                let counts = tokenizer.count_all(content);
                file.token_count = infiniloom_engine::types::TokenCounts {
                    o200k: counts.o200k,
                    cl100k: counts.cl100k,
                    claude: counts.claude,
                    gemini: counts.gemini,
                    llama: counts.llama,
                    mistral: counts.mistral,
                    deepseek: counts.deepseek,
                    qwen: counts.qwen,
                    cohere: counts.cohere,
                    grok: counts.grok,
                };
            }
        });
        recalculate_metadata(&mut repo);
    }

    // Populate git history
    if include_logs || include_diffs {
        if let Ok(git_repo) = GitRepo::open(&repo_path) {
            use infiniloom_engine::types::{GitChangedFile, GitCommitInfo, GitHistory};

            let mut git_history = GitHistory::default();

            if include_logs {
                if let Ok(commits) = git_repo.log(logs_count) {
                    git_history.commits = commits
                        .iter()
                        .map(|c| GitCommitInfo {
                            hash: c.hash.clone(),
                            short_hash: c.short_hash.clone(),
                            author: c.author.clone(),
                            date: c.date.clone(),
                            message: c.message.clone(),
                        })
                        .collect();
                }
            }

            if include_diffs {
                if let Ok(changed_files) = git_repo.status() {
                    git_history.changed_files = changed_files
                        .iter()
                        .map(|f| {
                            let status = match f.status {
                                infiniloom_engine::git::FileStatus::Added => "A",
                                infiniloom_engine::git::FileStatus::Modified => "M",
                                infiniloom_engine::git::FileStatus::Deleted => "D",
                                infiniloom_engine::git::FileStatus::Renamed => "R",
                                infiniloom_engine::git::FileStatus::Copied => "C",
                                infiniloom_engine::git::FileStatus::Unknown => "?",
                            };
                            let diff_content = git_repo
                                .uncommitted_diff(&f.path)
                                .ok()
                                .filter(|d| !d.is_empty());
                            GitChangedFile {
                                path: f.path.clone(),
                                status: status.to_owned(),
                                diff_content,
                            }
                        })
                        .collect();
                }
            }

            repo.metadata.git_history = Some(git_history);
        }
    }

    // Enforce token budget
    if let Some(result) = enforce_budget(&mut repo, max_tokens, model) {
        if verbose && (result.truncated_files > 0 || result.excluded_files > 0) {
            if let Some(pb) = &pb {
                pb.set_message(format!(
                    "Budget enforced: {} truncated, {} excluded",
                    result.truncated_files, result.excluded_files
                ));
            }
        }
    }

    if !show_directory_structure {
        repo.metadata.directory_structure = None;
    }

    // Generate repo map
    let map = RepoMapGenerator::builder()
        .token_budget(map_budget)
        .model(model)
        .build()
        .generate(&repo);

    if let Some(pb) = &pb {
        pb.set_message("Generating output...");
    }

    let instructions_text = read_instruction_file(&instruction_file)?;

    let formatter = OutputFormatter::by_format_with_all_options_and_model(
        format,
        show_line_numbers,
        show_file_summary,
        model,
    );
    let output_text = formatter.format(&repo, &map);
    let mut output_text = apply_pack_extras(
        output_text,
        format,
        &repo,
        model,
        header_text.as_deref(),
        instructions_text.as_deref(),
        token_tree,
        if security_check {
            security_issues.as_deref()
        } else {
            None
        },
        include_logs || include_diffs,
    )?;

    // Enforce max tokens limit
    if max_tokens > 0 {
        let current_tokens = estimate_tokens(&output_text, model);
        if current_tokens > max_tokens as usize {
            output_text = truncate_to_tokens(&output_text, max_tokens as usize, model);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    // Copy to clipboard if requested
    if copy_to_clipboard {
        #[cfg(feature = "clipboard")]
        {
            use arboard::Clipboard;
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set_text(output_text.clone());
                if verbose {
                    eprintln!("{} Copied to clipboard", "✓".green());
                }
            }
        }
        #[cfg(not(feature = "clipboard"))]
        {
            eprintln!(
                "{} Clipboard support not enabled. Build with --features clipboard",
                "⚠".yellow()
            );
        }
    }

    // Write output
    if let Some(ref output_path) = output {
        std::fs::write(output_path, &output_text).context("Failed to write output file")?;

        if verbose {
            let elapsed = start.elapsed();
            let total_lines: usize = repo
                .files
                .iter()
                .filter_map(|f| f.content.as_ref())
                .map(|c| c.lines().count())
                .sum();

            eprintln!();
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("{} Output written to: {}", "✓".green(), output_path.display());
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("  {} {} files", "📁".dimmed(), repo.files.len());
            eprintln!("  {} {} lines", "📄".dimmed(), total_lines);
            eprintln!("  {} {}", "📦".dimmed(), format_size(output_text.len() as u64, BINARY));
            eprintln!(
                "  {} ~{} tokens ({})",
                "🔢".dimmed(),
                repo.total_tokens(model),
                model.name()
            );
            eprintln!("  {} {:?}", "⏱️ ".dimmed(), elapsed);
            eprintln!();
        }
    } else {
        print!("{}", output_text);
    }

    // Handle watch mode
    if watch_mode {
        if output.is_none() {
            eprintln!("{} Watch mode requires --output to be specified", "Error:".red().bold());
            std::process::exit(1);
        }

        let watch_config = crate::watch::WatchConfig::new(repo_path, output.unwrap(), config);
        crate::watch::run_watch_mode(watch_config)?;
    }

    Ok(())
}
