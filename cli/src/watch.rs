//! Watch mode for automatic repository regeneration
//!
//! This module provides file watching capabilities to automatically regenerate
//! packed repository output when files change.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use std::time::{Duration, Instant};

use infiniloom_engine::{
    content_processing::truncate_base64,
    git::GitRepo,
    output::{OutputFormat, OutputFormatter},
    repomap::RepoMapGenerator,
    security::SecurityScanner,
    tokenizer::Tokenizer,
    types::{CompressionLevel, TokenizerModel},
    HeuristicCompressor, RepoCache, Repository,
};

use crate::commands::pack::PackConfig;
use crate::scanner;

/// Configuration for watch mode
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Repository path to watch
    pub repo_path: std::path::PathBuf,

    /// Output file path
    pub output_path: std::path::PathBuf,

    /// Pack configuration
    pub pack_config: PackConfig,

    /// Debounce duration (time to wait after last change before rebuilding)
    pub debounce_duration: Duration,

    /// Poll interval for file watcher
    pub poll_interval: Duration,
}

impl WatchConfig {
    /// Create a new watch configuration
    pub fn new(
        repo_path: std::path::PathBuf,
        output_path: std::path::PathBuf,
        pack_config: PackConfig,
    ) -> Self {
        Self {
            repo_path,
            output_path,
            pack_config,
            debounce_duration: Duration::from_millis(500),
            poll_interval: Duration::from_secs(1),
        }
    }
}

/// Run watch mode for automatic repository regeneration
///
/// This function sets up a file watcher that monitors the repository for changes
/// and automatically regenerates the packed output when files are modified.
///
/// # Arguments
///
/// * `config` - Watch mode configuration
///
/// # Errors
///
/// Returns error if:
/// - File watcher fails to initialize
/// - Directory cannot be watched
/// - File operations fail
pub fn run_watch_mode(config: WatchConfig) -> Result<()> {
    use notify::{Config as NotifyConfig, Event, PollWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    eprintln!();
    eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

    let (tx, rx) = channel();

    let mut watcher = PollWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        },
        NotifyConfig::default().with_poll_interval(config.poll_interval),
    )
    .context("Failed to create file watcher")?;

    watcher
        .watch(&config.repo_path, RecursiveMode::Recursive)
        .context("Failed to watch directory")?;

    let mut last_rebuild = Instant::now();
    let mut repo_cache: Option<RepoCache> = if config.pack_config.scan.incremental_cache {
        Some(RepoCache::new(config.repo_path.to_string_lossy().as_ref()))
    } else {
        None
    };

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => {
                if last_rebuild.elapsed() < config.debounce_duration {
                    continue;
                }

                eprintln!("{} Change detected, regenerating...", "🔄".yellow());
                let rebuild_start = Instant::now();

                match regenerate_output(&config, &mut repo_cache) {
                    Ok(stats) => {
                        eprintln!(
                            "{} Regenerated in {:?} ({} files, ~{} tokens)",
                            "✓".green(),
                            rebuild_start.elapsed(),
                            stats.file_count,
                            stats.token_count
                        );
                    },
                    Err(e) => {
                        eprintln!("{} Failed to regenerate: {}", "Error:".red(), e);
                    },
                }

                last_rebuild = Instant::now();
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Continue watching
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            },
        }
    }

    Ok(())
}

/// Statistics from regeneration
struct RegenerationStats {
    file_count: usize,
    token_count: u32,
}

/// Regenerate packed output from repository
fn regenerate_output(
    config: &WatchConfig,
    repo_cache: &mut Option<RepoCache>,
) -> Result<RegenerationStats> {
    // STEP 1: Fast scan without reading content (for filtering)
    let scan_config = scanner::ScanConfig {
        include_hidden: config.pack_config.scan.include_hidden,
        respect_gitignore: config.pack_config.scan.respect_gitignore,
        read_contents: false,
        max_file_size: 50 * 1024 * 1024, // 50MB
        skip_symbols: !config.pack_config.scan.enable_symbols,
    };

    let mut repo = if config.pack_config.scan.incremental_cache {
        let cache = repo_cache
            .get_or_insert_with(|| RepoCache::new(config.repo_path.to_string_lossy().as_ref()));
        scanner::scan_repository_with_cache(&config.repo_path, scan_config, cache)?
    } else {
        scanner::scan_repository(&config.repo_path, scan_config)?
    };

    // STEP 2: Apply filters
    apply_filters(&mut repo, &config.pack_config)?;

    // STEP 3: Read content for filtered files
    read_file_contents(&mut repo);

    // STEP 4: Apply transformations
    apply_transformations(&mut repo, &config.pack_config)?;

    // STEP 5: Security scan
    let security_issues = if config.pack_config.security.security_check
        || config.pack_config.security.redact_secrets
    {
        Some(scan_and_redact_secrets(&mut repo)?)
    } else {
        None
    };

    // STEP 6: Recalculate metadata
    recalculate_metadata(&mut repo);

    // STEP 7: Apply ranking and sorting
    apply_ranking(&mut repo, &config.pack_config)?;

    // STEP 8: Generate output
    let output = generate_formatted_output(&repo, &config.pack_config, security_issues.as_deref())?;

    // STEP 9: Write output file
    std::fs::write(&config.output_path, &output).context("Failed to write output file")?;

    // Save cache if enabled
    if let Some(cache) = repo_cache.as_mut() {
        let cache_path = config.repo_path.join(".infiniloom").join("cache.bin");
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        cache.save(&cache_path).ok(); // Ignore cache save errors
    }

    let token_count = repo.total_tokens(
        config
            .pack_config
            .output
            .model
            .unwrap_or(TokenizerModel::Claude),
    );

    Ok(RegenerationStats { file_count: repo.files.len(), token_count })
}

/// Apply filters to repository
fn apply_filters(repo: &mut Repository, config: &PackConfig) -> Result<()> {
    // Default ignores
    if config.scan.use_default_ignores {
        use infiniloom_engine::default_ignores::{
            matches_any, DEFAULT_IGNORES, DOC_IGNORES, TEST_IGNORES,
        };
        repo.files.retain(|f| {
            if matches_any(&f.relative_path, DEFAULT_IGNORES) {
                return false;
            }
            if !config.scan.include_tests && matches_any(&f.relative_path, TEST_IGNORES) {
                return false;
            }
            if !config.scan.include_docs && matches_any(&f.relative_path, DOC_IGNORES) {
                return false;
            }
            true
        });
    }

    // Apply centralized filtering with pattern caching (OnceLock-based)
    infiniloom_engine::filtering::apply_exclude_patterns(
        &mut repo.files,
        &config.scan.exclude_patterns,
        |f| &f.relative_path,
    );
    infiniloom_engine::filtering::apply_include_patterns(
        &mut repo.files,
        &config.scan.include_patterns,
        |f| &f.relative_path,
    );

    Ok(())
}

/// Read file contents in parallel
fn read_file_contents(repo: &mut Repository) {
    use rayon::prelude::*;
    repo.files.par_iter_mut().for_each(|file| {
        if let Ok(content) = std::fs::read_to_string(&file.path) {
            file.content = Some(content);
        }
    });
}

/// Apply content transformations (compression, comment removal, etc.)
fn apply_transformations(repo: &mut Repository, config: &PackConfig) -> Result<()> {
    let compression = config.output.compression.unwrap_or(CompressionLevel::None);

    // Determine transformation flags
    let should_remove_comments = config.scan.remove_comments
        || matches!(
            compression,
            CompressionLevel::Balanced | CompressionLevel::Aggressive | CompressionLevel::Extreme
        );

    let should_remove_empty = config.scan.remove_empty_lines
        || matches!(
            compression,
            CompressionLevel::Minimal
                | CompressionLevel::Balanced
                | CompressionLevel::Aggressive
                | CompressionLevel::Extreme
        );

    let semantic_compressor = if compression == CompressionLevel::Semantic {
        Some(HeuristicCompressor::new())
    } else {
        None
    };

    // Apply transformations
    for file in &mut repo.files {
        if let Some(ref mut content) = file.content {
            // Apply compression strategy
            match compression {
                CompressionLevel::Aggressive => {
                    if let Some(lang) = &file.language {
                        *content = crate::commands::pack::extract_signatures_only(
                            content,
                            lang,
                            &file.symbols,
                        );
                    }
                },
                CompressionLevel::Extreme => {
                    if let Some(lang) = &file.language {
                        *content = crate::commands::pack::extract_key_symbols_only(
                            content,
                            lang,
                            &file.symbols,
                        );
                    }
                },
                CompressionLevel::Focused => {
                    if let Some(lang) = &file.language {
                        *content = crate::commands::pack::extract_key_symbols_focused(
                            content,
                            lang,
                            &file.symbols,
                        );
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
                        *content = crate::commands::pack::remove_empty_lines_from_content(
                            content,
                            config.output.show_line_numbers,
                        );
                    }
                    if should_remove_comments {
                        if let Some(lang) = &file.language {
                            *content = crate::commands::pack::remove_comments_from_content(
                                content,
                                lang,
                                config.output.show_line_numbers,
                            );
                        }
                    }
                },
            }

            // Truncate base64 if enabled
            if config.scan.truncate_base64 {
                *content = truncate_base64(content);
            }
        }
    }

    // Exclude content if requested
    if config.scan.exclude_content {
        for file in &mut repo.files {
            file.content = None;
            file.token_count = infiniloom_engine::types::TokenCounts::default();
        }
    }

    Ok(())
}

/// Scan for secrets and redact them
fn scan_and_redact_secrets(
    repo: &mut Repository,
) -> Result<Vec<infiniloom_engine::security::SecretFinding>> {
    use rayon::prelude::*;

    let scanner = SecurityScanner::new();

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

    Ok(all_issues)
}

/// Recalculate repository metadata after transformations
fn recalculate_metadata(repo: &mut Repository) {
    use infiniloom_engine::types::{LanguageStats, TokenCounts};
    use std::collections::HashMap;

    // Update total files
    repo.metadata.total_files = repo.files.len() as u32;

    // Recalculate total lines
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

    // Recalculate token counts
    let tokenizer = Tokenizer::new();
    for file in &mut repo.files {
        if let Some(ref content) = file.content {
            let counts = tokenizer.count_all(content);
            file.token_count = TokenCounts {
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

    // Recalculate language statistics
    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new();
    for file in &repo.files {
        if let Some(ref lang) = file.language {
            let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1;
            let file_lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| file.size_bytes / 40);
            entry.1 += file_lines;
        }
    }

    let total_files = repo.metadata.total_files;
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
    repo.metadata.languages = languages;
}

/// Apply file ranking and sorting
fn apply_ranking(repo: &mut Repository, config: &PackConfig) -> Result<()> {
    // Sort by git changes if requested
    if config.git.sort_by_changes {
        if let Ok(git_repo) = GitRepo::open(&config.path) {
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
    } else if config.scan.full_mode {
        infiniloom_engine::rank_files(repo);
        infiniloom_engine::sort_files_by_importance(repo);
    } else {
        crate::commands::pack::rank_files_fast(repo);
    }

    // Truncate to top files if specified
    if config.scan.top_files > 0 && repo.files.len() > config.scan.top_files {
        repo.files.truncate(config.scan.top_files);
        recalculate_metadata(repo);
    }

    Ok(())
}

/// Generate formatted output
fn generate_formatted_output(
    repo: &Repository,
    config: &PackConfig,
    security_issues: Option<&[infiniloom_engine::security::SecretFinding]>,
) -> Result<String> {
    let format = config.output.format.unwrap_or(OutputFormat::Xml);
    let model = config.output.model.unwrap_or(TokenizerModel::Claude);
    let max_tokens = config.output.max_tokens;
    let map_budget = config.map_budget;

    // Generate repository map
    let map = RepoMapGenerator::builder()
        .token_budget(map_budget)
        .model(model)
        .build()
        .generate(repo);

    // Format output
    let formatter = OutputFormatter::by_format_with_all_options_and_model(
        format,
        config.output.show_line_numbers,
        config.output.show_file_summary,
        model,
    );
    let mut output = formatter.format(repo, &map);

    // Apply extras (header, instructions, token tree, security warnings)
    output = crate::commands::pack::apply_pack_extras(
        output,
        format,
        repo,
        model,
        config.output.header_text.as_deref(),
        None, // instruction_file content would be read here
        config.output.token_tree,
        security_issues,
        config.git.include_logs || config.git.include_diffs,
    )?;

    // Truncate if needed
    if max_tokens > 0 {
        let current_tokens = crate::commands::pack::estimate_tokens(&output, model);
        if current_tokens > max_tokens as usize {
            output = crate::commands::pack::truncate_to_tokens(&output, max_tokens as usize, model);
        }
    }

    Ok(output)
}
