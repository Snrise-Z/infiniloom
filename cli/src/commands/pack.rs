//! Pack command handler
//!
//! Packs a repository into LLM-friendly format with intelligent compression,
//! symbol ranking, and model-specific optimizations.

use anyhow::{Context, Result};
use colored::Colorize;
use humansize::{format_size, BINARY};
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::time::Instant;

use infiniloom_engine::{
    git::GitRepo,
    output::{OutputFormat, OutputFormatter},
    remote::RemoteRepo,
    repomap::RepoMapGenerator,
    security::SecurityScanner,
    tokenizer::{TokenModel, Tokenizer},
    types::{CompressionLevel, TokenizerModel},
};

use crate::config::load_config_file;
use crate::scanner;

/// Pre-compiled regex for base64 content detection
static BASE64_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data:[^;]+;base64,[A-Za-z0-9+/]*={0,2}|[A-Za-z0-9+/]{200,}={0,2}").unwrap()
});

/// Pack a repository into LLM-friendly format
#[allow(clippy::too_many_arguments)]
pub fn cmd_pack(
    path: PathBuf,
    cli_format: Option<OutputFormat>,
    cli_model: Option<TokenizerModel>,
    cli_compression: Option<CompressionLevel>,
    max_tokens: u32,
    output: Option<PathBuf>,
    include_hidden: bool,
    respect_gitignore: bool,
    enable_symbols: bool,
    full_mode: bool,
    exclude_content: bool,
    include_tests: bool,
    include_docs: bool,
    use_default_ignores: bool,
    verbose: bool,
    header_text: Option<String>,
    instruction_file: Option<PathBuf>,
    copy_to_clipboard: bool,
    token_tree: bool,
    show_directory_structure: bool,
    show_file_summary: bool,
    remove_empty_lines: bool,
    remove_comments: bool,
    top_files: usize,
    include_logs: bool,
    logs_count: usize,
    include_diffs: bool,
    sort_by_changes: bool,
    stdin: bool,
    truncate_base64: bool,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    security_check: bool,
    remote_branch: Option<String>,
    sparse_paths: Vec<String>,
    show_line_numbers: bool,
    redact_secrets: bool,
    config_path: Option<PathBuf>,
    watch_mode: bool,
    incremental_cache: bool,
    map_budget: u32,
) -> Result<()> {
    let start = Instant::now();

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
        (path.clone(), None)
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

    // Scan repository
    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: true,
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
        scanner::scan_repository_with_cache(&repo_path, config, cache)
            .context("Failed to scan repository")?
    } else {
        scanner::scan_repository(&repo_path, config).context("Failed to scan repository")?
    };

    // Update cache
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

    // Apply include/exclude patterns
    let all_include_patterns: Vec<String> = include_patterns
        .into_iter()
        .chain(loaded_config.include_patterns)
        .collect();
    let compiled_include_patterns: Vec<glob::Pattern> = all_include_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    if !compiled_include_patterns.is_empty() {
        repo.files.retain(|f| {
            compiled_include_patterns
                .iter()
                .any(|p| pattern_matches_file(p, &f.relative_path))
        });
    }

    let all_exclude_patterns: Vec<String> = exclude_patterns
        .into_iter()
        .chain(loaded_config.exclude_patterns)
        .collect();
    let compiled_exclude_patterns: Vec<glob::Pattern> = all_exclude_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    if !compiled_exclude_patterns.is_empty() {
        repo.files.retain(|f| {
            !compiled_exclude_patterns
                .iter()
                .any(|p| pattern_matches_file(p, &f.relative_path))
        });
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

    let semantic_compressor = if compression == CompressionLevel::Semantic {
        Some(infiniloom_engine::HeuristicCompressor::new())
    } else {
        None
    };

    for file in &mut repo.files {
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
            if truncate_base64 {
                *content = truncate_base64_content(content);
            }
        }
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
        scanner.add_custom_patterns(&security_custom_patterns);

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

    // Recompute token counts after transformations
    {
        let tokenizer = Tokenizer::new();
        for file in &mut repo.files {
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
        }
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
            use clipboard::{ClipboardContext, ClipboardProvider};
            if let Ok(mut ctx) = ClipboardContext::new() {
                let _ = ctx.set_contents(output_text.clone());
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
        run_watch_mode(
            &repo_path,
            &output,
            format,
            model,
            compression,
            include_hidden,
            respect_gitignore,
            enable_symbols,
            full_mode,
            exclude_content,
            include_tests,
            include_docs,
            use_default_ignores,
            max_file_size,
            show_line_numbers,
            show_directory_structure,
            show_file_summary,
            remove_empty_lines,
            remove_comments,
            top_files,
            sort_by_changes,
            truncate_base64,
            &compiled_include_patterns,
            &compiled_exclude_patterns,
            &stdin_paths,
            security_check,
            redact_secrets,
            fail_on_secrets,
            &security_allowlist,
            &security_custom_patterns,
            include_logs,
            logs_count,
            include_diffs,
            max_tokens,
            map_budget,
            header_text.as_deref(),
            &instruction_file,
            token_tree,
            incremental_cache,
            &mut repo_cache,
            &cache_path,
        )?;
    }

    Ok(())
}

// ============================================================================
// Helper functions
// ============================================================================

fn pattern_matches_file(pattern: &glob::Pattern, relative_path: &str) -> bool {
    if pattern.matches(relative_path) {
        return true;
    }
    if let Some(filename) = Path::new(relative_path).file_name() {
        if let Some(filename_str) = filename.to_str() {
            if pattern.matches(filename_str) {
                return true;
            }
        }
    }
    false
}

fn truncate_base64_content(content: &str) -> String {
    BASE64_PATTERN
        .replace_all(content, |caps: &regex::Captures<'_>| {
            let matched = caps.get(0).map_or("", |m| m.as_str());
            if matched.starts_with("data:") {
                if let Some(comma_idx) = matched.find(',') {
                    let prefix = &matched[..comma_idx + 1];
                    format!("{}[BASE64_TRUNCATED]", prefix)
                } else {
                    "[BASE64_TRUNCATED]".to_owned()
                }
            } else if matched.len() > 100 {
                if matched.contains('+') || matched.contains('/') {
                    format!("{}...[BASE64_TRUNCATED]", &matched[..50])
                } else {
                    matched.to_owned()
                }
            } else {
                matched.to_owned()
            }
        })
        .to_string()
}

fn remove_empty_lines_from_content(content: &str, preserve_line_numbers: bool) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    if has_embedded_nums {
        if preserve_line_numbers {
            content
                .lines()
                .filter(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        !rest.trim().is_empty()
                    } else {
                        !line.trim().is_empty()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
                .lines()
                .filter_map(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        if !rest.trim().is_empty() {
                            Some(rest.to_owned())
                        } else {
                            None
                        }
                    } else if !line.trim().is_empty() {
                        Some(line.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if preserve_line_numbers {
        content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| format!("{}:{}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn is_inside_string(text: &str) -> bool {
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_backslash = false;

    for c in text.chars() {
        if prev_backslash {
            prev_backslash = false;
            continue;
        }
        match c {
            '\\' => prev_backslash = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            _ => {},
        }
    }

    in_double || in_single
}

fn remove_comments_from_content(
    content: &str,
    language: &str,
    preserve_line_numbers: bool,
) -> String {
    let (line_comment, block_start, block_end) = match language.to_lowercase().as_str() {
        "python" | "ruby" | "shell" | "bash" | "sh" | "yaml" | "yml" => ("#", "", ""),
        "javascript" | "typescript" | "java" | "c" | "cpp" | "c++" | "rust" | "go" | "swift"
        | "kotlin" | "scala" => ("//", "/*", "*/"),
        "html" | "xml" => ("", "<!--", "-->"),
        "css" | "scss" | "sass" => ("", "/*", "*/"),
        "sql" => ("--", "/*", "*/"),
        "lua" => ("--", "--[[", "]]"),
        _ => ("//", "/*", "*/"),
    };

    let format_line = |line_num: u32, content: &str| -> String {
        if preserve_line_numbers {
            format!("{}:{}\n", line_num, content)
        } else {
            format!("{}\n", content)
        }
    };

    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    let mut result = String::new();
    let mut in_block_comment = false;

    for (line_num, raw_line) in content.lines().enumerate() {
        let (original_line_num, line) = if has_embedded_nums {
            if let Some((num_str, rest)) = raw_line.split_once(':') {
                if let Ok(n) = num_str.parse::<u32>() {
                    (n, rest)
                } else {
                    (line_num as u32 + 1, raw_line)
                }
            } else {
                (line_num as u32 + 1, raw_line)
            }
        } else {
            (line_num as u32 + 1, raw_line)
        };

        let trimmed = line.trim();

        if !block_start.is_empty() && !block_end.is_empty() {
            if in_block_comment {
                if let Some(idx) = line.find(block_end) {
                    in_block_comment = false;
                    let after_block = &line[idx + block_end.len()..];
                    if !after_block.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, after_block));
                    }
                }
                continue;
            }

            if let Some(idx) = line.find(block_start) {
                if let Some(end_idx) = line[idx + block_start.len()..].find(block_end) {
                    let before = &line[..idx];
                    let after = &line[idx + block_start.len() + end_idx + block_end.len()..];
                    let combined = format!("{}{}", before.trim_end(), after);
                    if !combined.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, &combined));
                    }
                    continue;
                } else {
                    in_block_comment = true;
                    let before = &line[..idx];
                    if !before.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, before.trim_end()));
                    }
                    continue;
                }
            }
        }

        if !line_comment.is_empty() && trimmed.starts_with(line_comment) {
            continue;
        }

        if !line_comment.is_empty() {
            if let Some(idx) = line.find(line_comment) {
                let before = &line[..idx];
                if !is_inside_string(before) {
                    let cleaned = before.trim_end();
                    if !cleaned.is_empty() {
                        result.push_str(&format_line(original_line_num, cleaned));
                    }
                    continue;
                }
            }
        }

        result.push_str(&format_line(original_line_num, line));
    }

    result
}

fn extract_signatures_only(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut included_lines: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for symbol in symbols {
        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            let line_idx = (symbol.start_line - 1) as usize;
            if !included_lines.contains(&symbol.start_line) {
                result.push_str(lines[line_idx]);
                result.push('\n');
                included_lines.insert(symbol.start_line);
            }
        }

        if let Some(ref doc) = symbol.docstring {
            if !doc.is_empty() {
                result.push_str("  // ");
                result.push_str(doc);
                result.push('\n');
            }
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

fn extract_signatures_heuristic(content: &str, language: &str) -> String {
    let mut result = String::new();
    let signature_patterns: &[&str] = match language.to_lowercase().as_str() {
        "python" => &["def ", "class ", "async def "],
        "javascript" | "typescript" | "jsx" | "tsx" => {
            &["function ", "class ", "const ", "let ", "export ", "async "]
        },
        "rust" => &["fn ", "pub fn ", "struct ", "enum ", "trait ", "impl ", "type ", "const "],
        "go" => &["func ", "type ", "const ", "var "],
        "java" | "kotlin" => {
            &["public ", "private ", "protected ", "class ", "interface ", "enum "]
        },
        "c" | "cpp" | "c++" => &["void ", "int ", "char ", "bool ", "class ", "struct ", "enum "],
        _ => &["def ", "fn ", "func ", "function ", "class ", "struct "],
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if signature_patterns.iter().any(|p| trimmed.starts_with(p)) {
            result.push_str(line);
            result.push('\n');
        }
    }

    if result.is_empty() {
        content.lines().take(50).collect::<Vec<_>>().join("\n")
    } else {
        result
    }
}

fn extract_key_symbols_only(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    use infiniloom_engine::SymbolKind;

    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();

    let key_symbols: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Function
                    | SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::Interface
            ) && s.visibility != infiniloom_engine::Visibility::Private
        })
        .collect();

    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20)
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect()
    };

    for symbol in symbols_to_use {
        result.push_str(&format!("// {}: {}\n", symbol.kind.name(), symbol.name));

        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            let line_idx = (symbol.start_line - 1) as usize;
            result.push_str(lines[line_idx]);
            result.push('\n');
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

fn extract_key_symbols_focused(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    use infiniloom_engine::SymbolKind;

    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return String::new();
    }

    let key_symbols: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Function
                    | SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::Interface
            ) && s.visibility != infiniloom_engine::Visibility::Private
        })
        .collect();

    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20)
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect()
    };

    #[derive(Clone)]
    struct SymbolRange {
        start: u32,
        end: u32,
        labels: Vec<String>,
    }

    let mut ranges: Vec<SymbolRange> = Vec::new();
    let mut fallback_snippets: Vec<String> = Vec::new();

    for symbol in symbols_to_use {
        let label = format!("{}: {}", symbol.kind.name(), symbol.name);
        if symbol.start_line > 0
            && symbol.end_line >= symbol.start_line
            && symbol.start_line <= total_lines
        {
            let start = symbol.start_line.saturating_sub(CONTEXT_LINES).max(1);
            let end = symbol
                .end_line
                .max(symbol.start_line)
                .saturating_add(CONTEXT_LINES)
                .min(total_lines);
            ranges.push(SymbolRange { start, end, labels: vec![label] });
        } else if let Some(ref sig) = symbol.signature {
            let snippet = format!("// {}\n{}", label, sig.trim());
            fallback_snippets.push(snippet);
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SymbolRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                for label in range.labels {
                    if !last.labels.contains(&label) {
                        last.labels.push(label);
                    }
                }
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    let mut result = String::new();
    for range in merged {
        let header = format!("// Focused symbols: {}\n", range.labels.join(", "));
        result.push_str(&header);

        let start_idx = range.start.saturating_sub(1) as usize;
        let end_idx = range.end.saturating_sub(1) as usize;
        if start_idx <= end_idx && end_idx < lines.len() {
            result.push_str(&lines[start_idx..=end_idx].join("\n"));
            result.push('\n');
        }
        result.push('\n');
    }

    if !fallback_snippets.is_empty() {
        result.push_str("// Additional signatures\n");
        for snippet in fallback_snippets {
            result.push_str(&snippet);
            result.push('\n');
        }
    }

    result
}

fn to_token_model(model: TokenizerModel) -> TokenModel {
    match model {
        TokenizerModel::Gpt52 => TokenModel::Gpt52,
        TokenizerModel::Gpt52Pro => TokenModel::Gpt52Pro,
        TokenizerModel::Gpt51 => TokenModel::Gpt51,
        TokenizerModel::Gpt51Mini => TokenModel::Gpt51Mini,
        TokenizerModel::Gpt51Codex => TokenModel::Gpt51Codex,
        TokenizerModel::Gpt5 => TokenModel::Gpt5,
        TokenizerModel::Gpt5Mini => TokenModel::Gpt5Mini,
        TokenizerModel::Gpt5Nano => TokenModel::Gpt5Nano,
        TokenizerModel::O4Mini => TokenModel::O4Mini,
        TokenizerModel::O3 => TokenModel::O3,
        TokenizerModel::O3Mini => TokenModel::O3Mini,
        TokenizerModel::O1 => TokenModel::O1,
        TokenizerModel::O1Mini => TokenModel::O1Mini,
        TokenizerModel::O1Preview => TokenModel::O1Preview,
        TokenizerModel::Gpt4o => TokenModel::Gpt4o,
        TokenizerModel::Gpt4oMini => TokenModel::Gpt4oMini,
        TokenizerModel::Gpt4 => TokenModel::Gpt4,
        TokenizerModel::Gpt35Turbo => TokenModel::Gpt35Turbo,
        TokenizerModel::Claude => TokenModel::Claude,
        TokenizerModel::Gemini => TokenModel::Gemini,
        TokenizerModel::Llama => TokenModel::Llama,
        TokenizerModel::CodeLlama => TokenModel::CodeLlama,
        TokenizerModel::Mistral => TokenModel::Mistral,
        TokenizerModel::DeepSeek => TokenModel::DeepSeek,
        TokenizerModel::Qwen => TokenModel::Qwen,
        TokenizerModel::Cohere => TokenModel::Cohere,
        TokenizerModel::Grok => TokenModel::Grok,
    }
}

fn estimate_tokens(text: &str, model: TokenizerModel) -> usize {
    let tokenizer = Tokenizer::new();
    tokenizer.count(text, to_token_model(model)) as usize
}

fn truncate_to_tokens(text: &str, max_tokens: usize, model: TokenizerModel) -> String {
    let tokenizer = Tokenizer::new();
    let token_model = to_token_model(model);
    let current = tokenizer.count(text, token_model) as usize;

    if current <= max_tokens {
        return text.to_owned();
    }

    let truncated = tokenizer.truncate_to_budget(text, token_model, max_tokens as u32);

    let markers = ["</file>", "```\n\n", "----------------------------------------\n", "\n---\n"];
    let mut best_end = truncated.len();

    for marker in markers {
        if let Some(pos) = truncated.rfind(marker) {
            let end_pos = pos + marker.len();
            if end_pos > truncated.len() / 2 {
                best_end = end_pos;
                break;
            }
        }
    }

    let mut result = truncated[..best_end].to_string();
    result.push_str("\n\n<!-- Output truncated to fit token limit -->\n");
    result
}

fn rank_files_fast(repo: &mut infiniloom_engine::Repository) {
    repo.files.sort_by_key(|f| {
        let path = &f.relative_path;
        let mut score: i32 = 1000;

        let entry_point_patterns = [
            "main.rs",
            "main.go",
            "main.py",
            "main.ts",
            "main.js",
            "main.c",
            "main.cpp",
            "index.ts",
            "index.js",
            "index.tsx",
            "index.jsx",
            "index.py",
            "app.py",
            "app.ts",
            "app.js",
            "app.tsx",
            "app.jsx",
            "app.go",
            "server.py",
            "server.ts",
            "server.js",
            "server.go",
            "mod.rs",
            "lib.rs",
            "lib.py",
            "__main__.py",
            "__init__.py",
        ];
        if entry_point_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 5000;
        }

        let config_patterns = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tsconfig.json",
            "webpack.config.js",
            "vite.config.ts",
            ".eslintrc",
            "Makefile",
            "CMakeLists.txt",
            "docker-compose.yml",
            "Dockerfile",
        ];
        if config_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 3000;
        }

        if path.contains("/src/") || path.starts_with("src/") {
            score -= 1000;
        }
        if path.contains("/lib/") || path.contains("/core/") {
            score -= 800;
        }
        if path.contains("/api/") || path.contains("/handlers/") || path.contains("/routes/") {
            score -= 600;
        }

        if path.contains("/test") || path.contains("_test.") || path.contains(".test.") {
            score += 2000;
        }
        if path.contains("/examples/") || path.contains("/docs/") || path.ends_with(".md") {
            score += 1500;
        }
        if path.contains("/vendor/") || path.contains("/node_modules/") {
            score += 3000;
        }

        score
    });
}

fn recalculate_metadata(repo: &mut infiniloom_engine::types::Repository) {
    use infiniloom_engine::types::{LanguageStats, TokenCounts};
    use std::collections::HashMap;

    repo.metadata.total_files = repo.files.len() as u32;

    repo.metadata.total_lines = repo
        .files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| f.size_bytes / 40)
        })
        .sum();

    repo.metadata.total_tokens = TokenCounts {
        o200k: repo.files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: repo.files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: repo.files.iter().map(|f| f.token_count.claude).sum(),
        gemini: repo.files.iter().map(|f| f.token_count.gemini).sum(),
        llama: repo.files.iter().map(|f| f.token_count.llama).sum(),
        mistral: repo.files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: repo.files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: repo.files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: repo.files.iter().map(|f| f.token_count.cohere).sum(),
        grok: repo.files.iter().map(|f| f.token_count.grok).sum(),
    };

    let mut language_counts: HashMap<String, u32> = HashMap::new();
    let mut language_lines: HashMap<String, u64> = HashMap::new();

    for file in &repo.files {
        if let Some(ref lang) = file.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
            let lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| file.size_bytes / 40);
            *language_lines.entry(lang.clone()).or_insert(0) += lines;
        }
    }

    let total_files = repo.metadata.total_files;
    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let lines = language_lines.get(&lang).copied().unwrap_or(0);
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    languages.sort_by(|a, b| b.files.cmp(&a.files));
    repo.metadata.languages = languages;

    let mut paths: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    paths.sort();
    repo.metadata.directory_structure = Some(paths.join("\n"));
}

fn update_repo_cache(
    cache: &mut infiniloom_engine::RepoCache,
    repo: &infiniloom_engine::Repository,
    symbols_extracted: bool,
) {
    use infiniloom_engine::incremental::hash_content;

    for file in &repo.files {
        let mtime = std::fs::metadata(&file.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let content_hash = file
            .content
            .as_ref()
            .map(|c| hash_content(c.as_bytes()))
            .unwrap_or(0);

        let cached = cache.get(&file.relative_path);
        let changed = cached.map_or(true, |_| {
            if content_hash != 0 {
                cache.needs_rescan_with_hash(
                    &file.relative_path,
                    mtime,
                    file.size_bytes,
                    content_hash,
                )
            } else {
                cache.needs_rescan(&file.relative_path, mtime, file.size_bytes)
            }
        });

        let symbols_extracted_for_file = if symbols_extracted {
            true
        } else if !changed {
            cached.map(|c| c.symbols_extracted).unwrap_or(false)
        } else {
            false
        };

        cache.update_file(infiniloom_engine::CachedFile {
            path: file.relative_path.clone(),
            mtime,
            size: file.size_bytes,
            hash: content_hash,
            tokens: infiniloom_engine::AccurateTokenCounts {
                o200k: file.token_count.o200k,
                cl100k: file.token_count.cl100k,
                claude: file.token_count.claude,
                gemini: file.token_count.gemini,
                llama: file.token_count.llama,
                mistral: file.token_count.mistral,
                deepseek: file.token_count.deepseek,
                qwen: file.token_count.qwen,
                cohere: file.token_count.cohere,
                grok: file.token_count.grok,
            },
            symbols: file
                .symbols
                .iter()
                .map(infiniloom_engine::CachedSymbol::from)
                .collect(),
            symbols_extracted: symbols_extracted_for_file,
            language: file.language.clone(),
            lines: file
                .content
                .as_ref()
                .map(|c| c.lines().count())
                .unwrap_or(0),
        });
    }

    let current_files: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    for deleted in cache.find_deleted_files(&current_files) {
        cache.remove_file(&deleted);
    }

    cache.recalculate_totals();
}

fn budget_token_model_for(model: TokenizerModel) -> TokenModel {
    match model {
        TokenizerModel::Claude => TokenModel::Claude,
        TokenizerModel::Gpt52
        | TokenizerModel::Gpt52Pro
        | TokenizerModel::Gpt51
        | TokenizerModel::Gpt51Mini
        | TokenizerModel::Gpt51Codex
        | TokenizerModel::Gpt5
        | TokenizerModel::Gpt5Mini
        | TokenizerModel::Gpt5Nano
        | TokenizerModel::O4Mini
        | TokenizerModel::O3
        | TokenizerModel::O3Mini
        | TokenizerModel::O1
        | TokenizerModel::O1Mini
        | TokenizerModel::O1Preview
        | TokenizerModel::Gpt4o
        | TokenizerModel::Gpt4oMini => TokenModel::Gpt4o,
        TokenizerModel::Gpt4 | TokenizerModel::Gpt35Turbo => TokenModel::Gpt4,
        TokenizerModel::Gemini => TokenModel::Gemini,
        TokenizerModel::Llama | TokenizerModel::CodeLlama => TokenModel::Llama,
        TokenizerModel::Mistral => TokenModel::Mistral,
        TokenizerModel::DeepSeek => TokenModel::DeepSeek,
        TokenizerModel::Qwen => TokenModel::Qwen,
        TokenizerModel::Cohere => TokenModel::Cohere,
        TokenizerModel::Grok => TokenModel::Grok,
    }
}

fn enforce_budget(
    repo: &mut infiniloom_engine::Repository,
    max_tokens: u32,
    model: TokenizerModel,
) -> Option<infiniloom_engine::budget::EnforcementResult> {
    if max_tokens == 0 {
        return None;
    }

    use infiniloom_engine::budget::{BudgetConfig, BudgetEnforcer, TruncationStrategy};
    use infiniloom_engine::TokenCount;

    let config = BudgetConfig {
        budget: TokenCount::new(max_tokens),
        model: budget_token_model_for(model),
        strategy: TruncationStrategy::Line,
        overhead_reserve: TokenCount::new(2000),
    };
    let enforcer = BudgetEnforcer::new(config);
    let result = enforcer.enforce(repo);

    recalculate_metadata(repo);

    Some(result)
}

fn read_instruction_file(instruction_file: &Option<PathBuf>) -> Result<Option<String>> {
    let path = match instruction_file {
        Some(path) => path,
        None => return Ok(None),
    };
    let instructions = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read instruction file: {}", path.display()))?;
    Ok(Some(instructions))
}

// ============================================================================
// Output formatting helpers
// ============================================================================

#[derive(serde::Serialize)]
struct TokenTreeEntry {
    path: String,
    tokens: u32,
}

#[derive(serde::Serialize)]
struct SecurityIssueEntry {
    file: String,
    line: u32,
    kind: String,
    severity: String,
}

fn token_tree_entries(
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
) -> Vec<TokenTreeEntry> {
    repo.files
        .iter()
        .map(|file| TokenTreeEntry {
            path: file.relative_path.clone(),
            tokens: file.token_count.get(model),
        })
        .collect()
}

fn security_issue_entries(
    issues: &[infiniloom_engine::security::SecretFinding],
) -> Vec<SecurityIssueEntry> {
    issues
        .iter()
        .map(|issue| SecurityIssueEntry {
            file: issue.file.clone(),
            line: issue.line,
            kind: issue.kind.name().to_owned(),
            severity: format!("{:?}", issue.severity),
        })
        .collect()
}

fn escape_xml_text(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn escape_yaml_string(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn append_yaml_block(output: &mut String, key: &str, value: &str) {
    output.push_str(&format!("\n{}: |\n", key));
    for line in value.lines() {
        output.push_str(&format!("  {}\n", line));
    }
}

fn append_git_context_markdown(
    output: &mut String,
    history: &infiniloom_engine::types::GitHistory,
) {
    if !history.commits.is_empty() {
        output.push_str("\n\n## Recent Commits\n\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "- **{}** {} - {}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\n\n## Uncommitted Changes\n\n");
        for file in &history.changed_files {
            output.push_str(&format!("- [{}] {}\n", file.status, file.path));
        }
    }
}

fn append_git_context_plain(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str("\n\nRECENT COMMITS\n");
        output.push_str("--------------\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "{} {} - {}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\n\nUNCOMMITTED CHANGES\n");
        output.push_str("-------------------\n");
        for file in &history.changed_files {
            output.push_str(&format!("[{}] {}\n", file.status, file.path));
        }
    }
}

fn append_git_context_toon(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str(&format!(
            "\n\nrecent_commits[{}]{{hash,message,author}}:\n",
            history.commits.len()
        ));
        for commit in &history.commits {
            output.push_str(&format!(
                "  {},{},{}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str(&format!(
            "\n\nuncommitted_changes[{}]{{status,path}}:\n",
            history.changed_files.len()
        ));
        for file in &history.changed_files {
            output.push_str(&format!("  {},{}\n", file.status, file.path));
        }
    }
}

fn append_git_context_yaml(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str("\nrecent_commits:\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "  - hash: {}\n    message: {}\n    author: {}\n",
                escape_yaml_string(&commit.short_hash),
                escape_yaml_string(&commit.message),
                escape_yaml_string(&commit.author)
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\nuncommitted_changes:\n");
        for file in &history.changed_files {
            output.push_str(&format!(
                "  - status: {}\n    path: {}\n",
                escape_yaml_string(&file.status),
                escape_yaml_string(&file.path)
            ));
        }
    }
}

fn apply_pack_extras(
    output_text: String,
    format: OutputFormat,
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
    header_text: Option<&str>,
    instructions: Option<&str>,
    token_tree: bool,
    security_issues: Option<&[infiniloom_engine::security::SecretFinding]>,
    include_git_context: bool,
) -> Result<String> {
    let token_tree_entries = if token_tree {
        Some(token_tree_entries(repo, model))
    } else {
        None
    };
    let security_entries = security_issues.map(security_issue_entries);
    let git_history = if include_git_context {
        repo.metadata.git_history.as_ref()
    } else {
        None
    };

    match format {
        OutputFormat::Json => {
            let mut root: serde_json::Value =
                serde_json::from_str(&output_text).context("Failed to parse JSON output")?;
            let obj = root
                .as_object_mut()
                .context("JSON output is not an object")?;

            if let Some(header) = header_text {
                obj.insert("header_text".to_owned(), serde_json::Value::String(header.to_owned()));
            }
            if let Some(instructions) = instructions {
                obj.insert(
                    "instructions".to_owned(),
                    serde_json::Value::String(instructions.to_owned()),
                );
            }
            if let Some(entries) = token_tree_entries {
                obj.insert(
                    "token_tree".to_owned(),
                    serde_json::json!({ "model": model.name(), "files": entries }),
                );
            }
            if let Some(entries) = security_entries {
                obj.insert(
                    "security_scan".to_owned(),
                    serde_json::json!({ "issues_found": entries.len(), "issues": entries }),
                );
            }

            serde_json::to_string_pretty(&root)
                .context("Failed to serialize JSON output with extras")
        },
        OutputFormat::Yaml => {
            let mut output = output_text;
            if !output.ends_with('\n') {
                output.push('\n');
            }

            if let Some(header) = header_text {
                append_yaml_block(&mut output, "header_text", header);
            }
            if let Some(instructions) = instructions {
                append_yaml_block(&mut output, "instructions", instructions);
            }
            if let Some(entries) = token_tree_entries {
                output.push_str("\ntoken_tree:\n");
                output.push_str(&format!("  model: {}\n", escape_yaml_string(model.name())));
                output.push_str("  files:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - path: {}\n      tokens: {}\n",
                        escape_yaml_string(&entry.path),
                        entry.tokens
                    ));
                }
            }
            if let Some(entries) = security_entries {
                output.push_str("\nsecurity_scan:\n");
                output.push_str(&format!("  issues_found: {}\n", entries.len()));
                output.push_str("  issues:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - file: {}\n      line: {}\n      kind: {}\n      severity: {}\n",
                        escape_yaml_string(&entry.file),
                        entry.line,
                        escape_yaml_string(&entry.kind),
                        escape_yaml_string(&entry.severity)
                    ));
                }
            }
            if let Some(history) = git_history {
                append_git_context_yaml(&mut output, history);
            }

            Ok(output)
        },
        OutputFormat::Xml => {
            let mut extras = String::new();
            if header_text.is_some()
                || instructions.is_some()
                || token_tree_entries.is_some()
                || security_entries.is_some()
            {
                extras.push_str("  <extras>\n");
                if let Some(header) = header_text {
                    extras.push_str(&format!(
                        "    <header_text>{}</header_text>\n",
                        escape_xml_text(header)
                    ));
                }
                if let Some(instructions) = instructions {
                    extras.push_str(&format!(
                        "    <instructions>{}</instructions>\n",
                        escape_xml_text(instructions)
                    ));
                }
                if let Some(entries) = token_tree_entries {
                    extras.push_str(&format!(
                        "    <token_tree model=\"{}\">\n",
                        escape_xml_text(model.name())
                    ));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <file path=\"{}\" tokens=\"{}\"/>\n",
                            escape_xml_text(&entry.path),
                            entry.tokens
                        ));
                    }
                    extras.push_str("    </token_tree>\n");
                }
                if let Some(entries) = security_entries {
                    extras.push_str(&format!("    <security_scan issues=\"{}\">\n", entries.len()));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <issue file=\"{}\" line=\"{}\" kind=\"{}\" severity=\"{}\"/>\n",
                            escape_xml_text(&entry.file),
                            entry.line,
                            escape_xml_text(&entry.kind),
                            escape_xml_text(&entry.severity)
                        ));
                    }
                    extras.push_str("    </security_scan>\n");
                }
                extras.push_str("  </extras>\n");
            }

            if extras.is_empty() {
                return Ok(output_text);
            }

            if let Some(pos) = output_text.rfind("</repository>") {
                let mut output = String::with_capacity(output_text.len() + extras.len() + 2);
                output.push_str(&output_text[..pos]);
                output.push('\n');
                output.push_str(&extras);
                output.push_str(&output_text[pos..]);
                Ok(output)
            } else {
                Ok(format!("{}\n{}", output_text, extras))
            }
        },
        OutputFormat::Markdown => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_markdown(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\n## Security Scan Results\n\n");
                output.push_str(&format!("Found {} potential security issues.\n\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!(
                    "\n\n## Token Tree\n\n| File | Tokens ({}) |\n|------|--------|\n",
                    model.name()
                ));
                for entry in entries {
                    output.push_str(&format!("| {} | {} |\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\n## Instructions\n\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Plain => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_plain(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\nSECURITY SCAN RESULTS\n");
                output.push_str("----------------------\n");
                output.push_str(&format!("Found {} potential security issues.\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\nTOKEN TREE ({})\n", model.name()));
                output.push_str("----------------------\n");
                for entry in entries {
                    output.push_str(&format!("- {}: {}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\nINSTRUCTIONS\n");
                output.push_str("------------\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Toon => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str("header_text: |\n");
                for line in header.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
                output.push('\n');
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_toon(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str(&format!(
                    "\n\nsecurity_scan[{}]{{severity,kind,file,line}}:\n",
                    entries.len()
                ));
                for entry in entries {
                    output.push_str(&format!(
                        "  {},{},{},{}\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\ntoken_tree_model: {}\n", model.name()));
                output.push_str(&format!("token_tree[{}]{{path,tokens}}:\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!("  {},{}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\ninstructions: |\n");
                for line in instructions.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }

            Ok(output)
        },
    }
}

// ============================================================================
// Watch mode implementation
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn run_watch_mode(
    repo_path: &Path,
    output: &Option<PathBuf>,
    format: OutputFormat,
    model: TokenizerModel,
    compression: CompressionLevel,
    include_hidden: bool,
    respect_gitignore: bool,
    enable_symbols: bool,
    full_mode: bool,
    exclude_content: bool,
    include_tests: bool,
    include_docs: bool,
    use_default_ignores: bool,
    max_file_size: u64,
    show_line_numbers: bool,
    show_directory_structure: bool,
    show_file_summary: bool,
    remove_empty_lines: bool,
    remove_comments: bool,
    top_files: usize,
    sort_by_changes: bool,
    truncate_base64: bool,
    compiled_include_patterns: &[glob::Pattern],
    compiled_exclude_patterns: &[glob::Pattern],
    stdin_paths: &Option<Vec<String>>,
    security_check: bool,
    redact_secrets: bool,
    fail_on_secrets: bool,
    security_allowlist: &[String],
    security_custom_patterns: &[String],
    include_logs: bool,
    logs_count: usize,
    include_diffs: bool,
    max_tokens: u32,
    map_budget: u32,
    header_text: Option<&str>,
    instruction_file: &Option<PathBuf>,
    token_tree: bool,
    incremental_cache: bool,
    repo_cache: &mut Option<infiniloom_engine::RepoCache>,
    cache_path: &Path,
) -> Result<()> {
    if output.is_none() {
        eprintln!("{} Watch mode requires --output to be specified", "Error:".red().bold());
        std::process::exit(1);
    }

    let output_path = output.as_ref().unwrap().clone();
    eprintln!();
    eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

    use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    let (tx, rx) = channel();

    let mut watcher = PollWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    )
    .context("Failed to create file watcher")?;

    watcher
        .watch(repo_path, RecursiveMode::Recursive)
        .context("Failed to watch directory")?;

    let debounce_duration = Duration::from_millis(500);
    let mut last_rebuild = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => {
                if last_rebuild.elapsed() < debounce_duration {
                    continue;
                }

                eprintln!("{} Change detected, regenerating...", "🔄".yellow());

                let rebuild_start = Instant::now();

                let scan_config = scanner::ScanConfig {
                    include_hidden,
                    respect_gitignore,
                    read_contents: true,
                    max_file_size,
                    skip_symbols: !enable_symbols,
                };

                let scan_result = if incremental_cache {
                    let cache = repo_cache.get_or_insert_with(|| {
                        infiniloom_engine::RepoCache::new(repo_path.to_string_lossy().as_ref())
                    });
                    scanner::scan_repository_with_cache(repo_path, scan_config, cache)
                } else {
                    scanner::scan_repository(repo_path, scan_config)
                };

                if let Ok(mut new_repo) = scan_result {
                    // Apply all the same transformations as initial pack
                    if incremental_cache {
                        if let Some(cache) = repo_cache.as_mut() {
                            update_repo_cache(cache, &new_repo, enable_symbols);
                            if let Err(e) = cache.save(cache_path) {
                                eprintln!("{} Failed to save cache: {}", "⚠".yellow(), e);
                            }
                        }
                    }

                    if use_default_ignores {
                        use infiniloom_engine::default_ignores::{
                            matches_any, DEFAULT_IGNORES, DOC_IGNORES, TEST_IGNORES,
                        };
                        new_repo.files.retain(|f| {
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

                    if let Some(ref paths) = stdin_paths {
                        new_repo.files.retain(|f| {
                            paths
                                .iter()
                                .any(|p| f.relative_path == *p || f.relative_path.ends_with(p))
                        });
                    }

                    if !compiled_include_patterns.is_empty() {
                        new_repo.files.retain(|f| {
                            compiled_include_patterns
                                .iter()
                                .any(|p| pattern_matches_file(p, &f.relative_path))
                        });
                    }

                    if !compiled_exclude_patterns.is_empty() {
                        new_repo.files.retain(|f| {
                            !compiled_exclude_patterns
                                .iter()
                                .any(|p| pattern_matches_file(p, &f.relative_path))
                        });
                    }

                    if exclude_content {
                        for file in &mut new_repo.files {
                            file.content = None;
                            file.token_count = infiniloom_engine::types::TokenCounts::default();
                        }
                    }

                    recalculate_metadata(&mut new_repo);

                    if sort_by_changes {
                        if let Ok(git_repo) = GitRepo::open(repo_path) {
                            let mut file_changes: Vec<(String, u32)> = new_repo
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
                            new_repo.files.sort_by_key(|f| {
                                order_map
                                    .get(&f.relative_path)
                                    .copied()
                                    .unwrap_or(usize::MAX)
                            });
                        }
                    } else if full_mode {
                        infiniloom_engine::rank_files(&mut new_repo);
                        infiniloom_engine::sort_files_by_importance(&mut new_repo);
                    } else {
                        rank_files_fast(&mut new_repo);
                    }

                    if top_files > 0 && new_repo.files.len() > top_files {
                        new_repo.files.truncate(top_files);
                        recalculate_metadata(&mut new_repo);
                    }

                    // Apply content transformations
                    let should_remove_comments = remove_comments
                        || matches!(
                            compression,
                            CompressionLevel::Balanced
                                | CompressionLevel::Aggressive
                                | CompressionLevel::Extreme
                        );
                    let should_remove_empty = remove_empty_lines
                        || matches!(
                            compression,
                            CompressionLevel::Minimal
                                | CompressionLevel::Balanced
                                | CompressionLevel::Aggressive
                                | CompressionLevel::Extreme
                        );

                    let watch_semantic_compressor = if compression == CompressionLevel::Semantic {
                        Some(infiniloom_engine::HeuristicCompressor::new())
                    } else {
                        None
                    };

                    for file in &mut new_repo.files {
                        if let Some(ref mut content) = file.content {
                            match compression {
                                CompressionLevel::Aggressive => {
                                    if let Some(lang) = &file.language {
                                        *content =
                                            extract_signatures_only(content, lang, &file.symbols);
                                    }
                                },
                                CompressionLevel::Extreme => {
                                    if let Some(lang) = &file.language {
                                        *content =
                                            extract_key_symbols_only(content, lang, &file.symbols);
                                    }
                                },
                                CompressionLevel::Focused => {
                                    if let Some(lang) = &file.language {
                                        *content = extract_key_symbols_focused(
                                            content,
                                            lang,
                                            &file.symbols,
                                        );
                                    }
                                },
                                CompressionLevel::Semantic => {
                                    if let Some(ref compressor) = watch_semantic_compressor {
                                        if let Ok(compressed) = compressor.compress(content) {
                                            *content = compressed;
                                        }
                                    }
                                },
                                _ => {
                                    if should_remove_empty {
                                        *content = remove_empty_lines_from_content(
                                            content,
                                            show_line_numbers,
                                        );
                                    }
                                    if should_remove_comments {
                                        if let Some(lang) = &file.language {
                                            *content = remove_comments_from_content(
                                                content,
                                                lang,
                                                show_line_numbers,
                                            );
                                        }
                                    }
                                },
                            }
                            if truncate_base64 {
                                *content = truncate_base64_content(content);
                            }
                        }
                    }

                    // Security scan
                    let watch_security_issues = if security_check || redact_secrets {
                        use rayon::prelude::*;
                        let mut scanner = SecurityScanner::new();
                        for pattern in security_allowlist {
                            scanner.allowlist(pattern);
                        }
                        scanner.add_custom_patterns(security_custom_patterns);
                        let all_issues: Vec<_> = new_repo
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

                    if fail_on_secrets {
                        if let Some(ref issues) = watch_security_issues {
                            if !issues.is_empty() {
                                eprintln!(
                                    "\n{} Secrets detected with fail_on_secrets enabled",
                                    "Error:".red().bold()
                                );
                                eprintln!("Watch mode stopping due to fail_on_secrets policy.");
                                break;
                            }
                        }
                    }

                    // Recompute token counts
                    {
                        let tokenizer = Tokenizer::new();
                        for file in &mut new_repo.files {
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
                        }
                        recalculate_metadata(&mut new_repo);
                    }

                    // Git history
                    if include_logs || include_diffs {
                        if let Ok(git_repo) = GitRepo::open(repo_path) {
                            use infiniloom_engine::types::{
                                GitChangedFile, GitCommitInfo, GitHistory,
                            };

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

                            new_repo.metadata.git_history = Some(git_history);
                        }
                    }

                    let _budget_result = enforce_budget(&mut new_repo, max_tokens, model);

                    if !show_directory_structure {
                        new_repo.metadata.directory_structure = None;
                    }

                    let new_map = RepoMapGenerator::builder()
                        .token_budget(map_budget)
                        .model(model)
                        .build()
                        .generate(&new_repo);

                    let new_formatter = OutputFormatter::by_format_with_all_options_and_model(
                        format,
                        show_line_numbers,
                        show_file_summary,
                        model,
                    );
                    let new_output = new_formatter.format(&new_repo, &new_map);

                    let instructions_text = match read_instruction_file(instruction_file) {
                        Ok(text) => text,
                        Err(err) => {
                            eprintln!("{} {}", "Error:".red(), err);
                            None
                        },
                    };

                    let mut new_output = match apply_pack_extras(
                        new_output,
                        format,
                        &new_repo,
                        model,
                        header_text,
                        instructions_text.as_deref(),
                        token_tree,
                        if security_check {
                            watch_security_issues.as_deref()
                        } else {
                            None
                        },
                        include_logs || include_diffs,
                    ) {
                        Ok(output) => output,
                        Err(err) => {
                            eprintln!("{} {}", "Error:".red(), err);
                            continue;
                        },
                    };

                    if max_tokens > 0 {
                        let current_tokens = estimate_tokens(&new_output, model);
                        if current_tokens > max_tokens as usize {
                            new_output =
                                truncate_to_tokens(&new_output, max_tokens as usize, model);
                        }
                    }

                    if let Err(e) = std::fs::write(&output_path, &new_output) {
                        eprintln!("{} Failed to write output: {}", "Error:".red(), e);
                    } else {
                        eprintln!(
                            "{} Regenerated in {:?} ({} files, ~{} tokens)",
                            "✓".green(),
                            rebuild_start.elapsed(),
                            new_repo.files.len(),
                            new_repo.total_tokens(model)
                        );
                    }
                }

                last_rebuild = Instant::now();
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Just keep watching
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            },
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // pattern_matches_file Tests
    // ============================================

    #[test]
    fn test_pattern_matches_file_exact_extension() {
        let pattern = glob::Pattern::new("*.rs").unwrap();
        assert!(pattern_matches_file(&pattern, "main.rs"));
        assert!(pattern_matches_file(&pattern, "src/lib.rs"));
        assert!(!pattern_matches_file(&pattern, "main.py"));
    }

    #[test]
    fn test_pattern_matches_file_directory_glob() {
        let pattern = glob::Pattern::new("src/**/*.rs").unwrap();
        assert!(pattern_matches_file(&pattern, "src/main.rs"));
        assert!(pattern_matches_file(&pattern, "src/utils/helper.rs"));
        assert!(!pattern_matches_file(&pattern, "tests/test.rs"));
    }

    #[test]
    fn test_pattern_matches_file_filename_only() {
        let pattern = glob::Pattern::new("Cargo.toml").unwrap();
        assert!(pattern_matches_file(&pattern, "Cargo.toml"));
        assert!(pattern_matches_file(&pattern, "subdir/Cargo.toml"));
    }

    #[test]
    fn test_pattern_matches_file_no_match() {
        let pattern = glob::Pattern::new("*.txt").unwrap();
        assert!(!pattern_matches_file(&pattern, "main.rs"));
        assert!(!pattern_matches_file(&pattern, "src/lib.py"));
    }

    // ============================================
    // truncate_base64_content Tests
    // ============================================

    #[test]
    fn test_truncate_base64_data_uri() {
        let input = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let result = truncate_base64_content(input);
        assert!(result.contains("data:image/png;base64,"));
        assert!(result.contains("[BASE64_TRUNCATED]"));
    }

    #[test]
    fn test_truncate_base64_long_string() {
        // Long base64 string with + and / characters
        let input = "A".repeat(150) + "+" + &"B".repeat(100) + "/";
        let result = truncate_base64_content(&input);
        assert!(result.contains("[BASE64_TRUNCATED]") || result.len() == input.len());
    }

    #[test]
    fn test_truncate_base64_no_truncation_short() {
        let input = "SGVsbG8gV29ybGQ="; // "Hello World" in base64
        let result = truncate_base64_content(input);
        // Short strings are not truncated
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_base64_preserves_non_base64() {
        let input = "This is regular text without base64";
        let result = truncate_base64_content(input);
        assert_eq!(result, input);
    }

    // ============================================
    // is_inside_string Tests
    // ============================================

    #[test]
    fn test_is_inside_string_double_quotes_open() {
        assert!(is_inside_string("\"hello"));
    }

    #[test]
    fn test_is_inside_string_double_quotes_closed() {
        assert!(!is_inside_string("\"hello\""));
    }

    #[test]
    fn test_is_inside_string_single_quotes_open() {
        assert!(is_inside_string("'hello"));
    }

    #[test]
    fn test_is_inside_string_single_quotes_closed() {
        assert!(!is_inside_string("'hello'"));
    }

    #[test]
    fn test_is_inside_string_escaped_quote() {
        assert!(is_inside_string("\"hello\\\"")); // Ends with escaped quote, still open
    }

    #[test]
    fn test_is_inside_string_escaped_then_close() {
        assert!(!is_inside_string("\"hello\\\"world\"")); // Escaped quote then closing
    }

    #[test]
    fn test_is_inside_string_nested_quotes() {
        assert!(!is_inside_string("\"it's a test\"")); // Single inside double
        assert!(!is_inside_string("'he said \"hi\"'")); // Double inside single
    }

    #[test]
    fn test_is_inside_string_empty() {
        assert!(!is_inside_string(""));
    }

    #[test]
    fn test_is_inside_string_no_quotes() {
        assert!(!is_inside_string("hello world"));
    }

    // ============================================
    // remove_empty_lines_from_content Tests
    // ============================================

    #[test]
    fn test_remove_empty_lines_basic() {
        let input = "line1\n\nline2\n\n\nline3";
        let result = remove_empty_lines_from_content(input, false);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_remove_empty_lines_preserve_numbers() {
        // Line numbers are 1-indexed from original positions
        // line1 at index 0 -> "1:line1", line2 at index 2 -> "3:line2", line3 at index 4 -> "5:line3"
        let input = "line1\n\nline2\n\nline3";
        let result = remove_empty_lines_from_content(input, true);
        assert!(result.contains("1:line1"));
        assert!(result.contains("3:line2")); // Original line 3 (index 2)
        assert!(result.contains("5:line3")); // Original line 5 (index 4)
    }

    #[test]
    fn test_remove_empty_lines_with_embedded_numbers() {
        let input = "1:code here\n2:\n3:more code";
        let result = remove_empty_lines_from_content(input, true);
        assert!(result.contains("1:code here"));
        assert!(result.contains("3:more code"));
        assert!(!result.contains("2:"));
    }

    #[test]
    fn test_remove_empty_lines_whitespace_only() {
        let input = "line1\n   \nline2\n\t\nline3";
        let result = remove_empty_lines_from_content(input, false);
        assert_eq!(result, "line1\nline2\nline3");
    }

    // ============================================
    // remove_comments_from_content Tests
    // ============================================

    #[test]
    fn test_remove_comments_python() {
        let input = "# comment\ncode = 1\n# another comment\nmore_code = 2";
        let result = remove_comments_from_content(input, "python", false);
        assert!(result.contains("code = 1"));
        assert!(result.contains("more_code = 2"));
        assert!(!result.contains("# comment"));
    }

    #[test]
    fn test_remove_comments_rust_line() {
        let input = "// comment\nlet x = 1;\n// another\nlet y = 2;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(result.contains("let y = 2;"));
        assert!(!result.contains("// comment"));
    }

    #[test]
    fn test_remove_comments_rust_block() {
        let input = "/* block comment */\nlet x = 1;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(!result.contains("block comment"));
    }

    #[test]
    fn test_remove_comments_javascript() {
        let input = "// single\nconst x = 1;\n/* multi\nline */\nconst y = 2;";
        let result = remove_comments_from_content(input, "javascript", false);
        assert!(result.contains("const x = 1;"));
        assert!(result.contains("const y = 2;"));
        assert!(!result.contains("single"));
        assert!(!result.contains("multi"));
    }

    #[test]
    fn test_remove_comments_html() {
        let input = "<!-- comment -->\n<div>content</div>";
        let result = remove_comments_from_content(input, "html", false);
        assert!(result.contains("<div>content</div>"));
        assert!(!result.contains("comment"));
    }

    #[test]
    fn test_remove_comments_sql() {
        let input = "-- comment\nSELECT * FROM table;\n/* block */\nUPDATE table;";
        let result = remove_comments_from_content(input, "sql", false);
        assert!(result.contains("SELECT * FROM table;"));
        assert!(result.contains("UPDATE table;"));
        assert!(!result.contains("-- comment"));
    }

    #[test]
    fn test_remove_comments_preserves_string_comments() {
        let input = "let x = \"// not a comment\";\nlet y = 1;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("\"// not a comment\""));
    }

    #[test]
    fn test_remove_comments_inline() {
        let input = "let x = 1; // inline comment";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(!result.contains("inline comment"));
    }

    // ============================================
    // escape_xml_text Tests
    // ============================================

    #[test]
    fn test_escape_xml_ampersand() {
        assert_eq!(escape_xml_text("foo & bar"), "foo &amp; bar");
    }

    #[test]
    fn test_escape_xml_less_than() {
        assert_eq!(escape_xml_text("a < b"), "a &lt; b");
    }

    #[test]
    fn test_escape_xml_greater_than() {
        assert_eq!(escape_xml_text("a > b"), "a &gt; b");
    }

    #[test]
    fn test_escape_xml_quotes() {
        assert_eq!(escape_xml_text("say \"hello\""), "say &quot;hello&quot;");
        assert_eq!(escape_xml_text("it's"), "it&apos;s");
    }

    #[test]
    fn test_escape_xml_multiple() {
        assert_eq!(escape_xml_text("<tag attr=\"val\">"), "&lt;tag attr=&quot;val&quot;&gt;");
    }

    #[test]
    fn test_escape_xml_no_escaping() {
        assert_eq!(escape_xml_text("hello world"), "hello world");
        assert_eq!(escape_xml_text(""), "");
    }

    // ============================================
    // escape_yaml_string Tests
    // ============================================

    #[test]
    fn test_escape_yaml_basic() {
        assert_eq!(escape_yaml_string("hello"), "\"hello\"");
    }

    #[test]
    fn test_escape_yaml_with_quotes() {
        assert_eq!(escape_yaml_string("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn test_escape_yaml_with_backslash() {
        assert_eq!(escape_yaml_string("path\\to\\file"), "\"path\\\\to\\\\file\"");
    }

    #[test]
    fn test_escape_yaml_empty() {
        assert_eq!(escape_yaml_string(""), "\"\"");
    }

    // ============================================
    // to_token_model Tests
    // ============================================

    #[test]
    fn test_to_token_model_claude() {
        let result = to_token_model(TokenizerModel::Claude);
        assert_eq!(result, TokenModel::Claude);
    }

    #[test]
    fn test_to_token_model_gpt4o() {
        let result = to_token_model(TokenizerModel::Gpt4o);
        assert_eq!(result, TokenModel::Gpt4o);
    }

    #[test]
    fn test_to_token_model_gpt4() {
        let result = to_token_model(TokenizerModel::Gpt4);
        assert_eq!(result, TokenModel::Gpt4);
    }

    #[test]
    fn test_to_token_model_gemini() {
        let result = to_token_model(TokenizerModel::Gemini);
        assert_eq!(result, TokenModel::Gemini);
    }

    #[test]
    fn test_to_token_model_all_variants() {
        // Test all model conversions work without panic
        let models = [
            TokenizerModel::Claude,
            TokenizerModel::Gpt4o,
            TokenizerModel::Gpt4oMini,
            TokenizerModel::Gpt4,
            TokenizerModel::Gpt35Turbo,
            TokenizerModel::Gemini,
            TokenizerModel::Llama,
            TokenizerModel::CodeLlama,
            TokenizerModel::Mistral,
            TokenizerModel::DeepSeek,
            TokenizerModel::Qwen,
            TokenizerModel::Cohere,
            TokenizerModel::Grok,
            TokenizerModel::O1,
            TokenizerModel::O1Mini,
            TokenizerModel::O1Preview,
            TokenizerModel::O3,
            TokenizerModel::O3Mini,
            TokenizerModel::O4Mini,
            TokenizerModel::Gpt5,
            TokenizerModel::Gpt5Mini,
            TokenizerModel::Gpt5Nano,
            TokenizerModel::Gpt51,
            TokenizerModel::Gpt51Mini,
            TokenizerModel::Gpt51Codex,
            TokenizerModel::Gpt52,
            TokenizerModel::Gpt52Pro,
        ];

        for tokenizer_model in models {
            let _ = to_token_model(tokenizer_model);
        }
    }

    // ============================================
    // budget_token_model_for Tests
    // ============================================

    #[test]
    fn test_budget_token_model_claude() {
        let result = budget_token_model_for(TokenizerModel::Claude);
        assert_eq!(result, TokenModel::Claude);
    }

    #[test]
    fn test_budget_token_model_gpt5_variants() {
        // All GPT-5 variants map to Gpt4o for budget
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt52), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt51), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt5), TokenModel::Gpt4o);
    }

    #[test]
    fn test_budget_token_model_o_series() {
        // O-series models map to Gpt4o for budget
        assert_eq!(budget_token_model_for(TokenizerModel::O4Mini), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::O3), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::O1), TokenModel::Gpt4o);
    }

    #[test]
    fn test_budget_token_model_legacy_gpt4() {
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt4), TokenModel::Gpt4);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt35Turbo), TokenModel::Gpt4);
    }

    #[test]
    fn test_budget_token_model_other_vendors() {
        assert_eq!(budget_token_model_for(TokenizerModel::Gemini), TokenModel::Gemini);
        assert_eq!(budget_token_model_for(TokenizerModel::Llama), TokenModel::Llama);
        assert_eq!(budget_token_model_for(TokenizerModel::Mistral), TokenModel::Mistral);
        assert_eq!(budget_token_model_for(TokenizerModel::DeepSeek), TokenModel::DeepSeek);
        assert_eq!(budget_token_model_for(TokenizerModel::Qwen), TokenModel::Qwen);
        assert_eq!(budget_token_model_for(TokenizerModel::Cohere), TokenModel::Cohere);
        assert_eq!(budget_token_model_for(TokenizerModel::Grok), TokenModel::Grok);
    }

    // ============================================
    // estimate_tokens Tests
    // ============================================

    #[test]
    fn test_estimate_tokens_basic() {
        let text = "Hello, world!";
        let tokens = estimate_tokens(text, TokenizerModel::Claude);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = estimate_tokens("", TokenizerModel::Claude);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        let text = "This is a longer piece of text that should have more tokens.";
        let short_text = "Hi";
        let long_tokens = estimate_tokens(text, TokenizerModel::Claude);
        let short_tokens = estimate_tokens(short_text, TokenizerModel::Claude);
        assert!(long_tokens > short_tokens);
    }

    // ============================================
    // truncate_to_tokens Tests
    // ============================================

    #[test]
    fn test_truncate_to_tokens_no_truncation() {
        let text = "Hello, world!";
        let result = truncate_to_tokens(text, 1000, TokenizerModel::Claude);
        assert_eq!(result, text);
    }

    #[test]
    fn test_truncate_to_tokens_truncates() {
        let text = "This is some text. ".repeat(100);
        let result = truncate_to_tokens(&text, 50, TokenizerModel::Claude);
        assert!(result.len() < text.len());
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_truncate_to_tokens_empty() {
        let result = truncate_to_tokens("", 100, TokenizerModel::Claude);
        assert_eq!(result, "");
    }

    // ============================================
    // extract_signatures_heuristic Tests
    // ============================================

    #[test]
    fn test_extract_signatures_rust() {
        let content =
            "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() {\n    // do something\n}";
        let result = extract_signatures_heuristic(content, "rust");
        assert!(result.contains("fn main()"));
        assert!(result.contains("fn helper()"));
    }

    #[test]
    fn test_extract_signatures_python() {
        let content = "def main():\n    print('hello')\n\ndef helper():\n    pass\n\nclass MyClass:\n    pass";
        let result = extract_signatures_heuristic(content, "python");
        assert!(result.contains("def main()"));
        assert!(result.contains("def helper()"));
        assert!(result.contains("class MyClass"));
    }

    #[test]
    fn test_extract_signatures_javascript() {
        let content = "function main() {\n    console.log('hi');\n}\n\nconst helper = () => {};\n\nclass MyClass {}";
        let result = extract_signatures_heuristic(content, "javascript");
        assert!(result.contains("function main()"));
        assert!(result.contains("class MyClass"));
    }

    #[test]
    fn test_extract_signatures_typescript() {
        // TypeScript patterns: function, class, const, let, export, async
        let content = "function main(): void {\n}\n\nclass Config {\n    name: string;\n}\n\nconst result = { ok: true };";
        let result = extract_signatures_heuristic(content, "typescript");
        assert!(result.contains("function main()"));
        assert!(result.contains("class Config"));
        assert!(result.contains("const result"));
    }

    #[test]
    fn test_extract_signatures_go() {
        let content = "func main() {\n}\n\ntype Config struct {\n    Name string\n}";
        let result = extract_signatures_heuristic(content, "go");
        assert!(result.contains("func main()"));
        assert!(result.contains("type Config struct"));
    }

    #[test]
    fn test_extract_signatures_empty() {
        let result = extract_signatures_heuristic("", "rust");
        assert!(result.is_empty());
    }

    // ============================================
    // TokenTreeEntry Tests
    // ============================================

    #[test]
    fn test_token_tree_entry_serialization() {
        let entry = TokenTreeEntry { path: "src/main.rs".to_string(), tokens: 1000 };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("1000"));
    }

    // ============================================
    // SecurityIssueEntry Tests
    // ============================================

    #[test]
    fn test_security_issue_entry_serialization() {
        let entry = SecurityIssueEntry {
            file: "config.py".to_string(),
            line: 42,
            kind: "API_KEY".to_string(),
            severity: "High".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("config.py"));
        assert!(json.contains("42"));
        assert!(json.contains("API_KEY"));
        assert!(json.contains("High"));
    }

    // ============================================
    // append_yaml_block Tests
    // ============================================

    #[test]
    fn test_append_yaml_block_single_line() {
        let mut output = String::new();
        append_yaml_block(&mut output, "header", "Hello World");
        assert!(output.contains("\nheader: |\n"));
        assert!(output.contains("  Hello World\n"));
    }

    #[test]
    fn test_append_yaml_block_multi_line() {
        let mut output = String::new();
        append_yaml_block(&mut output, "description", "Line 1\nLine 2\nLine 3");
        assert!(output.contains("\ndescription: |\n"));
        assert!(output.contains("  Line 1\n"));
        assert!(output.contains("  Line 2\n"));
        assert!(output.contains("  Line 3\n"));
    }

    // ============================================
    // Integration Tests
    // ============================================

    #[test]
    fn test_remove_comments_then_empty_lines() {
        let input = "// comment\nlet x = 1;\n\n// another\nlet y = 2;\n\n";
        let without_comments = remove_comments_from_content(input, "rust", false);
        let result = remove_empty_lines_from_content(&without_comments, false);
        assert!(result.contains("let x = 1;"));
        assert!(result.contains("let y = 2;"));
        // Should not have empty lines or comments
        assert!(!result.contains("//"));
    }

    #[test]
    fn test_escape_chain() {
        // Test escaping special chars for XML then YAML
        let input = "foo & <bar> \"test\"";
        let xml_escaped = escape_xml_text(input);
        assert!(xml_escaped.contains("&amp;"));
        assert!(xml_escaped.contains("&lt;"));
        assert!(xml_escaped.contains("&quot;"));

        let yaml_escaped = escape_yaml_string(input);
        assert!(yaml_escaped.starts_with('"'));
        assert!(yaml_escaped.ends_with('"'));
        assert!(yaml_escaped.contains("\\\""));
    }
}
