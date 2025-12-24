#![deny(clippy::all)]

use infiniloom_engine::{
    count_symbol_references,
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    git::{
        GitRepo as EngineGitRepo, FileStatus as EngineFileStatus, ChangedFile,
        DiffHunk as EngineGitDiffHunk,
    },
    rank_files, sort_files_by_importance,
    CompressionLevel, OutputFormat, OutputFormatter, RepoMapGenerator, Repository, SecurityScanner,
    SemanticCompressor, SemanticConfig, TokenizerModel, Tokenizer, tokenizer::TokenModel, Symbol,
    SymbolKind, Visibility, HeuristicCompressor,
    // Index module for new APIs
    index::{
        IndexBuilder, IndexStorage, BuildOptions, ContextExpander, ContextDepth,
        DiffChange, ChangeType,
        // Call graph query API
        find_symbol as engine_find_symbol,
        get_callers_by_name, get_callees_by_name, get_references_by_name,
        get_call_graph as engine_get_call_graph,
        get_call_graph_filtered,
        SymbolInfo as EngineSymbolInfo,
        ReferenceInfo as EngineReferenceInfo,
        CallGraph as EngineCallGraph,
        CallGraphEdge as EngineCallGraphEdge,
        CallGraphStats as EngineCallGraphStats,
    },
    // Chunking module
    Chunker, ChunkStrategy,
};
use std::collections::HashSet;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;

mod scanner;
use scanner::{scan_repository as do_scan, ScanConfig};

/// Options for packing a repository
#[napi(object)]
pub struct PackOptions {
    /// Output format: "xml", "markdown", "json", "yaml", "toon", or "plain"
    pub format: Option<String>,
    /// Target model: "claude", "gpt-5.2", "gpt-5.1", "gpt-5", "o4-mini", "o3", "o1", "gpt-4o", "gpt-4", "gemini", "llama", "mistral", "deepseek", "qwen", "cohere", "grok"
    pub model: Option<String>,
    /// Compression level: "none", "minimal", "balanced", "aggressive", "extreme", "focused", "semantic"
    pub compression: Option<String>,
    /// Token budget for repository map
    pub map_budget: Option<u32>,
    /// Maximum number of symbols in map
    pub max_symbols: Option<u32>,
    /// Skip security scanning (fail on critical findings)
    pub skip_security: Option<bool>,
    /// Redact detected secrets in output (default: true)
    pub redact_secrets: Option<bool>,
    /// Skip symbol extraction for faster scanning
    pub skip_symbols: Option<bool>,
    /// Glob patterns to include (e.g., ["src/**/*.ts", "lib/**/*.js"])
    pub include: Option<Vec<String>>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Include test files (default: false)
    pub include_tests: Option<bool>,
    /// Minimum security severity to block on: "critical", "high", "medium", "low" (default: "critical")
    pub security_threshold: Option<String>,
    /// Token budget for total output (0 = no limit). Files are included by importance until budget is reached.
    pub token_budget: Option<u32>,
    /// Only include files changed in git (requires baseSha or uses uncommitted changes)
    pub changed_only: Option<bool>,
    /// Base SHA/ref for diff comparison (e.g., "main", "HEAD~5", commit hash)
    pub base_sha: Option<String>,
    /// Head SHA/ref for diff comparison (default: working tree or HEAD)
    pub head_sha: Option<String>,
    /// Include staged changes only (if changedOnly is true and no refs specified)
    pub staged_only: Option<bool>,
    /// Include related files (importers/dependencies of changed files)
    pub include_related: Option<bool>,
    /// Depth for related file traversal (1-3, default: 1)
    pub related_depth: Option<u32>,
}

/// Statistics from scanning a repository
#[napi(object)]
pub struct ScanStats {
    /// Repository name
    pub name: String,
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u32,
    /// Total tokens for target model
    pub total_tokens: u32,
    /// Primary language
    pub primary_language: Option<String>,
    /// Language breakdown
    pub languages: Vec<LanguageStat>,
    /// Number of security findings
    pub security_findings: u32,
}

/// Statistics for a single language
#[napi(object)]
pub struct LanguageStat {
    /// Language name
    pub language: String,
    /// Number of files
    pub files: u32,
    /// Total lines
    pub lines: u32,
    /// Percentage of codebase
    pub percentage: f64,
}

/// Options for scanning a repository
#[napi(object)]
pub struct ScanOptions {
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Glob patterns to include (e.g., ["src/**/*.ts", "lib/**/*.js"])
    pub include: Option<Vec<String>>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Include test files (default: false)
    pub include_tests: Option<bool>,
    /// Apply default ignores for dist/, node_modules/, etc. (default: true)
    pub apply_default_ignores: Option<bool>,
}

/// Pack a repository into optimized LLM context
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional packing options
///
/// # Returns
/// Formatted repository context as a string
///
/// # Example
/// ```javascript
/// const { pack } = require('infiniloom-node');
///
/// const context = pack('./my-repo', {
///   format: 'xml',
///   model: 'claude',
///   compression: 'balanced',
///   mapBudget: 2000
/// });
/// ```
#[napi]
pub fn pack(path: String, options: Option<PackOptions>) -> Result<String> {
    let opts = options.unwrap_or(PackOptions {
        format: None,
        model: None,
        compression: None,
        map_budget: None,
        max_symbols: None,
        skip_security: None,
        redact_secrets: None,
        skip_symbols: None,
        include: None,
        exclude: None,
        include_tests: None,
        security_threshold: None,
        token_budget: None,
        changed_only: None,
        base_sha: None,
        head_sha: None,
        staged_only: None,
        include_related: None,
        related_depth: None,
    });

    // Parse options
    let format = parse_format(opts.format.as_deref())?;
    let model = parse_model(opts.model.as_deref())?;
    let compression = parse_compression(opts.compression.as_deref())?;
    let map_budget = opts.map_budget.unwrap_or(2000);
    let max_symbols = opts.max_symbols.unwrap_or(50);
    let skip_security = opts.skip_security.unwrap_or(false);
    let redact_secrets = opts.redact_secrets.unwrap_or(true);
    let skip_symbols = opts.skip_symbols.unwrap_or(false);
    let include_tests = opts.include_tests.unwrap_or(false);
    let security_threshold = parse_security_threshold(opts.security_threshold.as_deref())?;
    let token_budget = opts.token_budget.unwrap_or(0); // 0 = no limit
    let changed_only = opts.changed_only.unwrap_or(false);
    let include_related = opts.include_related.unwrap_or(false);
    let related_depth = opts.related_depth.unwrap_or(1).clamp(1, 3);

    // Scan repository (with contents for packing)
    let mut repo = scan_repository_with_options(&path, model, true, skip_symbols)?;

    // Apply default ignores to filter out build outputs, dependencies, etc.
    repo.files.retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));

    // Apply test ignores unless include_tests is true
    if !include_tests {
        repo.files.retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    // Apply custom include patterns (if specified, only keep matching files)
    if let Some(ref include_patterns) = opts.include {
        let patterns: Vec<&str> = include_patterns.iter().map(|s| s.as_str()).collect();
        repo.files.retain(|f| matches_any_pattern(&f.relative_path, &patterns));
    }

    // Apply custom exclude patterns
    if let Some(ref exclude_patterns) = opts.exclude {
        let patterns: Vec<&str> = exclude_patterns.iter().map(|s| s.as_str()).collect();
        repo.files.retain(|f| !matches_any_pattern(&f.relative_path, &patterns));
    }

    // Filter to changed files only (if enabled)
    if changed_only {
        let path_buf = PathBuf::from(&path);
        if EngineGitRepo::is_git_repo(&path_buf) {
            let git_repo = EngineGitRepo::open(&path_buf)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e)))?;

            // Get changed file paths
            let changed_paths: HashSet<String> = if opts.staged_only.unwrap_or(false) {
                // Only staged changes - status() returns all changes
                // For staged-only, we'd need to parse status output more carefully
                // For now, we include all changed files
                git_repo.status()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else if let (Some(ref base), Some(ref head)) = (&opts.base_sha, &opts.head_sha) {
                // Diff between two refs
                git_repo.diff_files(base, head)
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else if let Some(ref base) = opts.base_sha {
                // Diff from base to HEAD
                git_repo.diff_files(base, "HEAD")
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else {
                // Uncommitted changes (default)
                git_repo.status()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            };

            // Filter repo files to only include changed files
            repo.files.retain(|f| changed_paths.contains(&f.relative_path));
        }
    }

    // Expand to include related files (if enabled)
    if include_related && !repo.files.is_empty() {
        let path_buf = PathBuf::from(&path);
        let storage = IndexStorage::new(&path_buf);

        // Try to load index for dependency information
        if let (Ok(index), Ok(graph)) = (storage.load_index(), storage.load_graph()) {
            let depth = match related_depth {
                1 => ContextDepth::L1,
                2 => ContextDepth::L2,
                _ => ContextDepth::L3,
            };

            let expander = ContextExpander::new(&index, &graph);

            // Get current file paths
            let changed_paths: Vec<String> = repo.files.iter().map(|f| f.relative_path.clone()).collect();

            // Convert to DiffChange for expander
            let changes: Vec<DiffChange> = changed_paths.iter().map(|p| DiffChange {
                file_path: p.clone(),
                old_path: None,
                line_ranges: vec![],
                change_type: ChangeType::Modified,
                diff_content: None,
            }).collect();

            // Expand context
            let context = expander.expand(&changes, depth, token_budget);

            // Collect related file paths
            let mut related_paths: HashSet<String> = HashSet::new();
            for f in &context.dependent_files {
                related_paths.insert(f.path.clone());
            }
            for f in &context.related_tests {
                related_paths.insert(f.path.clone());
            }

            // Re-scan to include related files that weren't in the original set
            if !related_paths.is_empty() {
                let full_repo = scan_repository_with_options(&path, model, true, skip_symbols)?;
                for file in full_repo.files {
                    if related_paths.contains(&file.relative_path) {
                        // Check if we already have this file
                        if !repo.files.iter().any(|f| f.relative_path == file.relative_path) {
                            repo.files.push(file);
                        }
                    }
                }
            }
        }
    }

    // Count cross-file symbol references (populates Symbol.references field)
    count_symbol_references(&mut repo);

    // Rank files by importance
    rank_files(&mut repo);
    sort_files_by_importance(&mut repo);

    // Security check and redaction
    let scanner = SecurityScanner::new();
    for file in &mut repo.files {
        if let Some(ref content) = file.content {
            // Check for findings at or above threshold
            if !skip_security {
                let findings = scanner.scan(content, &file.relative_path);
                if findings.iter().any(|f| severity_at_or_above(&f.severity, &security_threshold)) {
                    return Err(Error::new(
                        Status::GenericFailure,
                        format!(
                            "{:?} security issues found in {}. Use skip_security: true or adjust security_threshold to override.",
                            security_threshold,
                            file.relative_path
                        ),
                    ));
                }
            }

            // Redact secrets from content if enabled
            if redact_secrets {
                let redacted = scanner.redact_content(content, &file.relative_path);
                file.content = Some(redacted);
            }
        }
    }

    // Apply compression to file contents based on compression level
    match compression {
        CompressionLevel::None => {
            // No compression - keep content as-is
        }
        CompressionLevel::Minimal => {
            // Remove empty lines
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        }
        CompressionLevel::Balanced => {
            // Remove empty lines and comments (basic heuristic)
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| {
                            let trimmed = line.trim();
                            !trimmed.is_empty()
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with('#')
                                && !trimmed.starts_with("/*")
                                && !trimmed.starts_with('*')
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        }
        CompressionLevel::Aggressive | CompressionLevel::Extreme => {
            // Extract signatures only - keep function/class definitions
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    file.content = Some(signature_lines(content));
                }
            }
        }
        CompressionLevel::Focused => {
            // Key symbols with small surrounding context
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let focused = focused_symbol_context(content, &file.symbols);
                    file.content = Some(focused);
                }
            }
        }
        CompressionLevel::Semantic => {
            // Use heuristic-based semantic compression
            let compressor = HeuristicCompressor::new();
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    if let Ok(compressed) = compressor.compress(content) {
                        file.content = Some(compressed);
                    }
                }
            }
        }
    }

    // Apply token budget to limit output size (Bug #7 fix)
    // Files are already sorted by importance, so we keep top files until budget is reached
    if token_budget > 0 {
        let tokenizer = Tokenizer::new();
        let mut cumulative_tokens: u32 = 0;
        let mut files_to_keep = Vec::new();

        for file in repo.files {
            let file_tokens = file.content.as_ref()
                .map(|c| tokenizer.count(c, model))
                .unwrap_or(0);

            // Check if adding this file would exceed budget
            if cumulative_tokens + file_tokens <= token_budget {
                cumulative_tokens += file_tokens;
                files_to_keep.push(file);
            } else if files_to_keep.is_empty() {
                // Always include at least one file (the most important)
                files_to_keep.push(file);
                break;
            } else {
                // Budget exceeded, stop adding files
                break;
            }
        }

        repo.files = files_to_keep;
        // Update metadata
        repo.metadata.total_files = repo.files.len() as u32;
    }

    // Generate repository map using builder pattern
    let generator = RepoMapGenerator::builder()
        .token_budget(map_budget)
        .max_symbols(max_symbols as usize)
        .model(model)
        .build();
    let map = generator.generate(&repo);

    // Format output
    let formatter = OutputFormatter::by_format_with_model(format, model);
    let output = formatter.format(&repo, &map);

    Ok(output)
}

/// Scan a repository and return statistics
///
/// # Arguments
/// * `path` - Path to repository root
/// * `model` - Optional target model (default: "claude") - for backwards compatibility
///
/// # Returns
/// Repository statistics
///
/// # Example
/// ```javascript
/// const { scan } = require('infiniloom-node');
///
/// const stats = scan('./my-repo', 'claude');
/// console.log(`Total files: ${stats.totalFiles}`);
/// console.log(`Total tokens: ${stats.totalTokens}`);
/// ```
#[napi]
pub fn scan(path: String, model: Option<String>) -> Result<ScanStats> {
    // Call scan_with_options with default options for backwards compatibility
    scan_with_options(path, Some(ScanOptions {
        model,
        include: None,
        exclude: None,
        include_tests: None,
        apply_default_ignores: Some(true),
    }))
}

/// Scan a repository with full options
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Scan options
///
/// # Returns
/// Repository statistics
///
/// # Example
/// ```javascript
/// const { scanWithOptions } = require('infiniloom-node');
///
/// const stats = scanWithOptions('./my-repo', {
///   model: 'claude',
///   exclude: ['dist/**', '**/*.test.ts'],
///   applyDefaultIgnores: true
/// });
/// ```
#[napi]
pub fn scan_with_options(path: String, options: Option<ScanOptions>) -> Result<ScanStats> {
    let opts = options.unwrap_or(ScanOptions {
        model: None,
        include: None,
        exclude: None,
        include_tests: None,
        apply_default_ignores: Some(true),
    });

    let tokenizer_model = parse_model(opts.model.as_deref())?;
    let apply_default_ignores = opts.apply_default_ignores.unwrap_or(true);
    let include_tests = opts.include_tests.unwrap_or(false);

    let mut repo = scan_repository(&path, tokenizer_model, true)?;

    // Apply default ignores (Bug #2 fix)
    if apply_default_ignores {
        repo.files.retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));
    }

    // Apply test ignores unless include_tests is true
    if !include_tests {
        repo.files.retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    // Apply custom include patterns
    if let Some(ref include_patterns) = opts.include {
        let patterns: Vec<&str> = include_patterns.iter().map(|s| s.as_str()).collect();
        repo.files.retain(|f| matches_any_pattern(&f.relative_path, &patterns));
    }

    // Apply custom exclude patterns
    if let Some(ref exclude_patterns) = opts.exclude {
        let patterns: Vec<&str> = exclude_patterns.iter().map(|s| s.as_str()).collect();
        repo.files.retain(|f| !matches_any_pattern(&f.relative_path, &patterns));
    }

    // Recalculate metadata after filtering
    let total_files = repo.files.len() as u32;
    let total_lines: u64 = repo.files.iter()
        .map(|f| f.content.as_ref().map(|c| c.lines().count() as u64).unwrap_or(0))
        .sum();

    // Calculate language stats with actual line counts (Bug #9 fix)
    let mut language_stats: std::collections::HashMap<String, (u32, u64)> = std::collections::HashMap::new();
    for file in &repo.files {
        if let Some(ref lang) = file.language {
            let lines = file.content.as_ref().map(|c| c.lines().count() as u64).unwrap_or(0);
            let entry = language_stats.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1;  // files
            entry.1 += lines;  // lines
        }
    }

    // Sort languages by percentage (Bug #12 fix)
    let mut languages: Vec<LanguageStat> = language_stats
        .into_iter()
        .map(|(lang, (files, lines))| {
            let percentage = if total_files > 0 {
                (files as f64 / total_files as f64) * 100.0
            } else {
                0.0
            };
            LanguageStat {
                language: lang,
                files,
                lines: lines as u32,
                percentage,
            }
        })
        .collect();
    languages.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap_or(std::cmp::Ordering::Equal));

    // Security scan
    let scanner = SecurityScanner::new();
    let mut total_findings = 0;
    for file in &repo.files {
        if let Some(content) = &file.content {
            let findings = scanner.scan(content, &file.relative_path);
            total_findings += findings.len();
        }
    }

    Ok(ScanStats {
        name: repo.name.clone(),
        total_files,
        total_lines: total_lines as u32,
        total_tokens: repo.total_tokens(tokenizer_model),
        primary_language: languages.first().map(|l| l.language.clone()),
        languages,
        security_findings: total_findings as u32,
    })
}

/// Count tokens in text for a specific model
///
/// # Arguments
/// * `text` - Text to tokenize
/// * `model` - Optional model name (default: "claude")
///
/// # Returns
/// Token count (exact for OpenAI models via tiktoken, calibrated estimates for others)
///
/// # Example
/// ```javascript
/// const { countTokens } = require('infiniloom-node');
///
/// const count = countTokens('Hello, world!', 'claude');
/// console.log(`Tokens: ${count}`);
/// ```
#[napi]
pub fn count_tokens(text: String, model: Option<String>) -> Result<u32> {
    // Parse model string to TokenModel for accurate tokenization
    let model_str = model.as_deref().unwrap_or("claude");
    let token_model = match model_str.to_lowercase().as_str() {
        "claude" => TokenModel::Claude,
        // GPT-5.x series (latest)
        "gpt-5.2" | "gpt5.2" | "gpt52" => TokenModel::Gpt52,
        "gpt-5.2-pro" | "gpt52-pro" => TokenModel::Gpt52Pro,
        "gpt-5.1" | "gpt5.1" | "gpt51" => TokenModel::Gpt51,
        "gpt-5.1-mini" | "gpt51-mini" => TokenModel::Gpt51Mini,
        "gpt-5.1-codex" | "gpt51-codex" => TokenModel::Gpt51Codex,
        "gpt-5" | "gpt5" => TokenModel::Gpt5,
        "gpt-5-mini" | "gpt5-mini" => TokenModel::Gpt5Mini,
        "gpt-5-nano" | "gpt5-nano" => TokenModel::Gpt5Nano,
        // O-series reasoning models
        "o4-mini" => TokenModel::O4Mini,
        "o3" => TokenModel::O3,
        "o3-mini" => TokenModel::O3Mini,
        "o1" => TokenModel::O1,
        "o1-mini" => TokenModel::O1Mini,
        "o1-preview" => TokenModel::O1Preview,
        // GPT-4 series
        "gpt-4o" | "gpt4o" => TokenModel::Gpt4o,
        "gpt-4o-mini" | "gpt4o-mini" => TokenModel::Gpt4oMini,
        "gpt" | "gpt-4" | "gpt4" => TokenModel::Gpt4,
        "gpt-3.5-turbo" | "gpt35-turbo" | "gpt35turbo" => TokenModel::Gpt35Turbo,
        // Other vendors
        "gemini" => TokenModel::Gemini,
        "llama" => TokenModel::Llama,
        "codellama" => TokenModel::CodeLlama,
        "mistral" => TokenModel::Mistral,
        "deepseek" => TokenModel::DeepSeek,
        "qwen" => TokenModel::Qwen,
        "cohere" => TokenModel::Cohere,
        "grok" => TokenModel::Grok,
        _ => {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "Invalid model: {}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, codellama, mistral, deepseek, qwen, cohere, grok",
                    model_str
                ),
            ));
        }
    };

    // Use the engine's accurate tokenizer (tiktoken for OpenAI, calibrated estimates for others)
    let tokenizer = Tokenizer::new();
    Ok(tokenizer.count(&text, token_model))
}

/// Infiniloom class for advanced usage
#[napi]
pub struct Infiniloom {
    repo: Repository,
    model: TokenizerModel,
}

#[napi]
impl Infiniloom {
    /// Create a new Infiniloom instance
    ///
    /// # Arguments
    /// * `path` - Path to repository root
    /// * `model` - Optional model name (default: "claude")
    #[napi(constructor)]
    pub fn new(path: String, model: Option<String>) -> Result<Self> {
        let tokenizer_model = parse_model(model.as_deref())?;
        let mut repo = scan_repository(&path, tokenizer_model, true)?;

        // Apply default ignores to filter out build outputs, dependencies, test fixtures, etc.
        repo.files.retain(|f| {
            !matches_any(&f.relative_path, DEFAULT_IGNORES)
                && !matches_any(&f.relative_path, TEST_IGNORES)
        });

        // Count cross-file symbol references (populates Symbol.references field)
        count_symbol_references(&mut repo);

        // Rank files by importance
        rank_files(&mut repo);
        sort_files_by_importance(&mut repo);

        Ok(Self {
            repo,
            model: tokenizer_model,
        })
    }

    /// Get repository statistics (Bug #4 fix - consistent with scan() function)
    #[napi]
    pub fn get_stats(&self) -> ScanStats {
        // Calculate actual file and line counts from filtered files
        let total_files = self.repo.files.len() as u32;
        let total_lines: u64 = self.repo.files.iter()
            .map(|f| f.content.as_ref().map(|c| c.lines().count() as u64).unwrap_or(0))
            .sum();

        // Calculate language stats with actual line counts (Bug #9 fix)
        let mut language_stats: std::collections::HashMap<String, (u32, u64)> = std::collections::HashMap::new();
        for file in &self.repo.files {
            if let Some(ref lang) = file.language {
                let lines = file.content.as_ref().map(|c| c.lines().count() as u64).unwrap_or(0);
                let entry = language_stats.entry(lang.clone()).or_insert((0, 0));
                entry.0 += 1;  // files
                entry.1 += lines;  // lines
            }
        }

        // Sort languages by percentage (Bug #12 fix)
        let mut languages: Vec<LanguageStat> = language_stats
            .into_iter()
            .map(|(lang, (files, lines))| {
                let percentage = if total_files > 0 {
                    (files as f64 / total_files as f64) * 100.0
                } else {
                    0.0
                };
                LanguageStat {
                    language: lang,
                    files,
                    lines: lines as u32,
                    percentage,
                }
            })
            .collect();
        languages.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap_or(std::cmp::Ordering::Equal));

        // Security scan
        let scanner = SecurityScanner::new();
        let mut total_findings = 0;
        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                total_findings += findings.len();
            }
        }

        ScanStats {
            name: self.repo.name.clone(),
            total_files,
            total_lines: total_lines as u32,
            total_tokens: self.repo.total_tokens(self.model),
            primary_language: languages.first().map(|l| l.language.clone()),
            languages,
            security_findings: total_findings as u32,
        }
    }

    /// Generate a repository map
    ///
    /// # Arguments
    /// * `budget` - Token budget (default: 2000)
    /// * `max_symbols` - Maximum symbols (default: 50)
    #[napi]
    pub fn generate_map(&self, budget: Option<u32>, max_symbols: Option<u32>) -> Result<String> {
        let token_budget = budget.unwrap_or(2000);
        let max_syms = max_symbols.unwrap_or(50);

        let generator = RepoMapGenerator::builder()
            .token_budget(token_budget)
            .max_symbols(max_syms as usize)
            .model(self.model)
            .build();

        let map = generator.generate(&self.repo);

        serde_json::to_string_pretty(&map)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Pack repository with specific options
    #[napi]
    pub fn pack(&self, options: Option<PackOptions>) -> Result<String> {
        let opts = options.unwrap_or(PackOptions {
            format: None,
            model: None,
            compression: None,
            map_budget: None,
            max_symbols: None,
            skip_security: None,
            redact_secrets: None,
            skip_symbols: None,
            include: None,
            exclude: None,
            include_tests: None,
            security_threshold: None,
            token_budget: None,
            changed_only: None,
            base_sha: None,
            head_sha: None,
            staged_only: None,
            include_related: None,
            related_depth: None,
        });

        let format = parse_format(opts.format.as_deref())?;
        let compression = parse_compression(opts.compression.as_deref())?;
        let map_budget = opts.map_budget.unwrap_or(2000);
        let max_symbols = opts.max_symbols.unwrap_or(50);
        let redact_secrets = opts.redact_secrets.unwrap_or(true);
        let token_budget = opts.token_budget.unwrap_or(0); // 0 = no limit

        // Clone repo to apply transformations
        let mut repo = self.repo.clone();

        // Redact secrets from content if enabled
        if redact_secrets {
            let scanner = SecurityScanner::new();
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let redacted = scanner.redact_content(content, &file.relative_path);
                    file.content = Some(redacted);
                }
            }
        }

        // Apply compression to file contents
        match compression {
            CompressionLevel::None => {}
            CompressionLevel::Minimal => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let compressed: String = content
                            .lines()
                            .filter(|line| !line.trim().is_empty())
                            .collect::<Vec<_>>()
                            .join("\n");
                        file.content = Some(compressed);
                    }
                }
            }
            CompressionLevel::Balanced => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let compressed: String = content
                            .lines()
                            .filter(|line| {
                                let trimmed = line.trim();
                                !trimmed.is_empty()
                                    && !trimmed.starts_with("//")
                                    && !trimmed.starts_with('#')
                                    && !trimmed.starts_with("/*")
                                    && !trimmed.starts_with('*')
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        file.content = Some(compressed);
                    }
                }
            }
            CompressionLevel::Aggressive | CompressionLevel::Extreme => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        file.content = Some(signature_lines(content));
                    }
                }
            }
            CompressionLevel::Focused => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let focused = focused_symbol_context(content, &file.symbols);
                        file.content = Some(focused);
                    }
                }
            }
            CompressionLevel::Semantic => {
                let compressor = HeuristicCompressor::new();
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        if let Ok(compressed) = compressor.compress(content) {
                            file.content = Some(compressed);
                        }
                    }
                }
            }
        }

        // Apply token budget to limit output size (Bug #7 fix)
        if token_budget > 0 {
            let tokenizer = Tokenizer::new();
            let mut cumulative_tokens: u32 = 0;
            let mut files_to_keep = Vec::new();

            for file in repo.files {
                let file_tokens = file.content.as_ref()
                    .map(|c| tokenizer.count(c, self.model))
                    .unwrap_or(0);

                if cumulative_tokens + file_tokens <= token_budget {
                    cumulative_tokens += file_tokens;
                    files_to_keep.push(file);
                } else if files_to_keep.is_empty() {
                    files_to_keep.push(file);
                    break;
                } else {
                    break;
                }
            }

            repo.files = files_to_keep;
            repo.metadata.total_files = repo.files.len() as u32;
        }

        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols as usize)
            .model(self.model)
            .build();

        let map = generator.generate(&repo);
        let formatter = OutputFormatter::by_format_with_model(format, self.model);

        Ok(formatter.format(&repo, &map))
    }

    /// Check for security issues (Bug #8 fix - now returns structured findings)
    #[napi]
    pub fn security_scan(&self) -> Result<Vec<SecurityFinding>> {
        let scanner = SecurityScanner::new();
        let mut findings = Vec::new();

        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let file_findings = scanner.scan(content, &file.relative_path);
                for finding in file_findings {
                    findings.push(SecurityFinding {
                        file: finding.file.clone(),
                        line: finding.line,
                        severity: format!("{:?}", finding.severity),
                        kind: finding.kind.name().to_string(),
                        pattern: finding.pattern.clone(),
                    });
                }
            }
        }

        Ok(findings)
    }

    /// Check for security issues (legacy format, returns formatted strings)
    #[napi]
    pub fn security_scan_formatted(&self) -> Result<Vec<String>> {
        let scanner = SecurityScanner::new();
        let mut findings = Vec::new();

        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let file_findings = scanner.scan(content, &file.relative_path);
                for finding in file_findings {
                    findings.push(format!(
                        "{} in {} at line {}: {}",
                        finding.kind.name(),
                        finding.file,
                        finding.line,
                        finding.pattern
                    ));
                }
            }
        }

        Ok(findings)
    }
}

// Helper functions

fn parse_format(format: Option<&str>) -> Result<OutputFormat> {
    match format.unwrap_or("xml") {
        "xml" => Ok(OutputFormat::Xml),
        "markdown" | "md" => Ok(OutputFormat::Markdown),
        "json" => Ok(OutputFormat::Json),
        "yaml" | "yml" => Ok(OutputFormat::Yaml),
        "toon" => Ok(OutputFormat::Toon),
        "plain" | "text" | "txt" => Ok(OutputFormat::Plain),
        other => Err(Error::new(
            Status::InvalidArg,
            format!("Unknown format: {}. Use 'xml', 'markdown', 'json', 'yaml', 'toon', or 'plain'", other),
        )),
    }
}

fn parse_model(model: Option<&str>) -> Result<TokenizerModel> {
    match model.unwrap_or("claude").to_lowercase().as_str() {
        "claude" => Ok(TokenizerModel::Claude),
        // GPT-5.x series (latest)
        "gpt-5.2" | "gpt5.2" | "gpt52" => Ok(TokenizerModel::Gpt52),
        "gpt-5.2-pro" | "gpt52-pro" => Ok(TokenizerModel::Gpt52Pro),
        "gpt-5.1" | "gpt5.1" | "gpt51" => Ok(TokenizerModel::Gpt51),
        "gpt-5.1-mini" | "gpt51-mini" => Ok(TokenizerModel::Gpt51Mini),
        "gpt-5.1-codex" | "gpt51-codex" => Ok(TokenizerModel::Gpt51Codex),
        "gpt-5" | "gpt5" => Ok(TokenizerModel::Gpt5),
        "gpt-5-mini" | "gpt5-mini" => Ok(TokenizerModel::Gpt5Mini),
        "gpt-5-nano" | "gpt5-nano" => Ok(TokenizerModel::Gpt5Nano),
        // O-series reasoning models
        "o4-mini" => Ok(TokenizerModel::O4Mini),
        "o3" => Ok(TokenizerModel::O3),
        "o3-mini" => Ok(TokenizerModel::O3Mini),
        "o1" => Ok(TokenizerModel::O1),
        "o1-mini" => Ok(TokenizerModel::O1Mini),
        "o1-preview" => Ok(TokenizerModel::O1Preview),
        // GPT-4 series
        "gpt-4o" | "gpt4o" => Ok(TokenizerModel::Gpt4o),
        "gpt-4o-mini" | "gpt4o-mini" => Ok(TokenizerModel::Gpt4oMini),
        "gpt-4" | "gpt4" | "gpt" => Ok(TokenizerModel::Gpt4),
        "gpt-3.5-turbo" | "gpt35-turbo" => Ok(TokenizerModel::Gpt35Turbo),
        // Other vendors
        "gemini" => Ok(TokenizerModel::Gemini),
        "llama" => Ok(TokenizerModel::Llama),
        "codellama" => Ok(TokenizerModel::CodeLlama),
        "mistral" => Ok(TokenizerModel::Mistral),
        "deepseek" => Ok(TokenizerModel::DeepSeek),
        "qwen" => Ok(TokenizerModel::Qwen),
        "cohere" => Ok(TokenizerModel::Cohere),
        "grok" => Ok(TokenizerModel::Grok),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown model: {}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, mistral, deepseek, qwen, cohere, grok",
                other
            ),
        )),
    }
}

fn signature_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("export ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("type ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn focused_symbol_context(content: &str, symbols: &[Symbol]) -> String {
    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return signature_lines(content);
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
            ) && s.visibility != Visibility::Private
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

    let mut ranges = Vec::new();
    let mut fallback_snippets = Vec::new();

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
            ranges.push(SymbolRange {
                start,
                end,
                labels: vec![label],
            });
        } else if let Some(ref sig) = symbol.signature {
            fallback_snippets.push(format!("// {}\n{}", label, sig.trim()));
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return signature_lines(content);
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
        result.push_str(&format!(
            "// Focused symbols: {}\n",
            range.labels.join(", ")
        ));
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

fn parse_compression(compression: Option<&str>) -> Result<CompressionLevel> {
    match compression.unwrap_or("balanced") {
        "none" => Ok(CompressionLevel::None),
        "minimal" => Ok(CompressionLevel::Minimal),
        "balanced" => Ok(CompressionLevel::Balanced),
        "aggressive" => Ok(CompressionLevel::Aggressive),
        "extreme" => Ok(CompressionLevel::Extreme),
        "focused" => Ok(CompressionLevel::Focused),
        "semantic" => Ok(CompressionLevel::Semantic),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown compression: {}. Use 'none', 'minimal', 'balanced', 'aggressive', 'extreme', 'focused', or 'semantic'",
                other
            ),
        )),
    }
}

/// Parse security severity threshold (Bug #5 fix)
fn parse_security_threshold(threshold: Option<&str>) -> Result<infiniloom_engine::security::Severity> {
    use infiniloom_engine::security::Severity;
    match threshold.unwrap_or("critical").to_lowercase().as_str() {
        "critical" => Ok(Severity::Critical),
        "high" => Ok(Severity::High),
        "medium" => Ok(Severity::Medium),
        "low" => Ok(Severity::Low),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown security threshold: {}. Use 'critical', 'high', 'medium', or 'low'",
                other
            ),
        )),
    }
}

/// Check if a severity is at or above a threshold
fn severity_at_or_above(
    severity: &infiniloom_engine::security::Severity,
    threshold: &infiniloom_engine::security::Severity,
) -> bool {
    use infiniloom_engine::security::Severity;
    let severity_level = match severity {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    };
    let threshold_level = match threshold {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    };
    severity_level >= threshold_level
}

/// Check if a path matches any of the given glob patterns (Bug #3 fix)
fn matches_any_pattern(path: &str, patterns: &[&str]) -> bool {
    for pattern in patterns {
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
    }
    false
}

/// Compress text using semantic compression
///
/// Uses heuristic-based compression to reduce content while preserving meaning.
/// When built with the "embeddings" feature, uses neural networks for clustering.
///
/// # Arguments
/// * `text` - Text to compress
/// * `similarity_threshold` - Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
/// * `budget_ratio` - Target size as ratio of original (0.0-1.0, default: 0.5)
///
/// # Returns
/// Compressed text
///
/// # Example
/// ```javascript
/// const { semanticCompress } = require('infiniloom-node');
///
/// const compressed = semanticCompress(longText, 0.7, 0.3);
/// ```
#[napi]
pub fn semantic_compress(
    text: String,
    similarity_threshold: Option<f64>,
    budget_ratio: Option<f64>,
) -> Result<String> {
    let config = SemanticConfig {
        similarity_threshold: similarity_threshold.unwrap_or(0.7) as f32,
        budget_ratio: budget_ratio.unwrap_or(0.5) as f32,
        min_chunk_size: 100,
        max_chunk_size: 2000,
    };

    let compressor = SemanticCompressor::with_config(config);
    compressor.compress(&text).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Compression failed: {}", e))
    })
}

fn scan_repository(path: &str, model: TokenizerModel, read_contents: bool) -> Result<Repository> {
    scan_repository_with_options(path, model, read_contents, false)
}

fn scan_repository_with_options(
    path: &str,
    _model: TokenizerModel,
    read_contents: bool,
    skip_symbols: bool,
) -> Result<Repository> {
    let path_buf = PathBuf::from(path);

    if !path_buf.exists() {
        return Err(Error::new(
            Status::InvalidArg,
            format!("Path does not exist: {}", path),
        ));
    }

    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents,
        max_file_size: 50 * 1024 * 1024, // 50MB
        skip_symbols,
    };

    do_scan(&path_buf, config).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}

// ============================================================================
// Git Operations
// ============================================================================

/// Check if a path is a git repository
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// True if path is a git repository, false otherwise
///
/// # Example
/// ```javascript
/// const { isGitRepo } = require('infiniloom-node');
///
/// if (isGitRepo('./my-project')) {
///   console.log('This is a git repository');
/// }
/// ```
#[napi]
pub fn is_git_repo(path: String) -> bool {
    let path_buf = PathBuf::from(path);
    EngineGitRepo::is_git_repo(&path_buf)
}

/// File status information
#[napi(object)]
pub struct GitFileStatus {
    /// File path
    pub path: String,
    /// Old path (for renames)
    pub old_path: Option<String>,
    /// Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
    pub status: String,
}

/// Changed file with diff stats
#[napi(object)]
pub struct GitChangedFile {
    /// File path
    pub path: String,
    /// Old path (for renames)
    pub old_path: Option<String>,
    /// Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
    pub status: String,
    /// Number of lines added
    pub additions: u32,
    /// Number of lines deleted
    pub deletions: u32,
}

/// Commit information
#[napi(object)]
pub struct GitCommit {
    /// Full commit hash
    pub hash: String,
    /// Short commit hash (7 characters)
    pub short_hash: String,
    /// Author name
    pub author: String,
    /// Author email
    pub email: String,
    /// Commit date (ISO 8601 format)
    pub date: String,
    /// Commit message (first line)
    pub message: String,
}

/// Blame line information
#[napi(object)]
pub struct GitBlameLine {
    /// Commit hash that introduced the line
    pub commit: String,
    /// Author who wrote the line
    pub author: String,
    /// Date when line was written
    pub date: String,
    /// Line number (1-indexed)
    pub line_number: u32,
}

/// A single line change within a diff hunk
#[napi(object)]
pub struct GitDiffLine {
    /// Type of change: "add", "remove", or "context"
    pub change_type: String,
    /// Line number in the old file (null for additions)
    pub old_line: Option<u32>,
    /// Line number in the new file (null for deletions)
    pub new_line: Option<u32>,
    /// The actual line content (without +/- prefix)
    pub content: String,
}

/// A diff hunk representing a contiguous block of changes
#[napi(object)]
pub struct GitDiffHunk {
    /// Starting line in the old file
    pub old_start: u32,
    /// Number of lines in the old file section
    pub old_count: u32,
    /// Starting line in the new file
    pub new_start: u32,
    /// Number of lines in the new file section
    pub new_count: u32,
    /// Header line (e.g., "@@ -1,5 +1,7 @@ function name")
    pub header: String,
    /// Individual line changes within this hunk
    pub lines: Vec<GitDiffLine>,
}

/// Git repository wrapper for Node.js
///
/// Provides access to git operations like status, diff, log, and blame.
///
/// # Example
/// ```javascript
/// const { GitRepo } = require('infiniloom-node');
///
/// const repo = new GitRepo('./my-project');
/// console.log(`Branch: ${repo.currentBranch()}`);
/// console.log(`Commit: ${repo.currentCommit()}`);
///
/// for (const file of repo.status()) {
///   console.log(`${file.status}: ${file.path}`);
/// }
/// ```
#[napi]
pub struct GitRepo {
    inner: EngineGitRepo,
}

#[napi]
impl GitRepo {
    /// Open a git repository
    ///
    /// # Arguments
    /// * `path` - Path to the repository
    ///
    /// # Throws
    /// Error if path is not a git repository
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let path_buf = PathBuf::from(path);
        let inner = EngineGitRepo::open(&path_buf)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e)))?;
        Ok(GitRepo { inner })
    }

    /// Get the current branch name
    ///
    /// # Returns
    /// Current branch name (e.g., "main", "feature/xyz")
    #[napi]
    pub fn current_branch(&self) -> Result<String> {
        self.inner.current_branch()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get the current commit hash
    ///
    /// # Returns
    /// Full SHA-1 hash of HEAD commit
    #[napi]
    pub fn current_commit(&self) -> Result<String> {
        self.inner.current_commit()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get working tree status
    ///
    /// Returns both staged and unstaged changes.
    ///
    /// # Returns
    /// Array of file status objects
    #[napi]
    pub fn status(&self) -> Result<Vec<GitFileStatus>> {
        let files = self.inner.status()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(files.iter().map(|f| GitFileStatus {
            path: f.path.clone(),
            old_path: f.old_path.clone(),
            status: format_file_status(f.status),
        }).collect())
    }

    /// Get files changed between two commits
    ///
    /// # Arguments
    /// * `from_ref` - Starting commit/branch/tag
    /// * `to_ref` - Ending commit/branch/tag
    ///
    /// # Returns
    /// Array of changed files with diff stats
    #[napi]
    pub fn diff_files(&self, from_ref: String, to_ref: String) -> Result<Vec<GitChangedFile>> {
        let files = self.inner.diff_files(&from_ref, &to_ref)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(files.iter().map(|f| GitChangedFile {
            path: f.path.clone(),
            old_path: f.old_path.clone(),
            status: format_file_status(f.status),
            additions: f.additions,
            deletions: f.deletions,
        }).collect())
    }

    /// Get recent commits
    ///
    /// # Arguments
    /// * `count` - Maximum number of commits to return (default: 10)
    ///
    /// # Returns
    /// Array of commit objects
    #[napi]
    pub fn log(&self, count: Option<u32>) -> Result<Vec<GitCommit>> {
        let commits = self.inner.log(count.unwrap_or(10) as usize)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(commits.iter().map(|c| GitCommit {
            hash: c.hash.clone(),
            short_hash: c.short_hash.clone(),
            author: c.author.clone(),
            email: c.email.clone(),
            date: c.date.clone(),
            message: c.message.clone(),
        }).collect())
    }

    /// Get commits that modified a specific file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `count` - Maximum number of commits to return (default: 10)
    ///
    /// # Returns
    /// Array of commits that modified the file
    #[napi]
    pub fn file_log(&self, path: String, count: Option<u32>) -> Result<Vec<GitCommit>> {
        let commits = self.inner.file_log(&path, count.unwrap_or(10) as usize)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(commits.iter().map(|c| GitCommit {
            hash: c.hash.clone(),
            short_hash: c.short_hash.clone(),
            author: c.author.clone(),
            email: c.email.clone(),
            date: c.date.clone(),
            message: c.message.clone(),
        }).collect())
    }

    /// Get blame information for a file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Array of blame line objects
    #[napi]
    pub fn blame(&self, path: String) -> Result<Vec<GitBlameLine>> {
        let lines = self.inner.blame(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(lines.iter().map(|l| GitBlameLine {
            commit: l.commit.clone(),
            author: l.author.clone(),
            date: l.date.clone(),
            line_number: l.line_number,
        }).collect())
    }

    /// Get list of files tracked by git
    ///
    /// # Returns
    /// Array of file paths tracked by git
    #[napi]
    pub fn ls_files(&self) -> Result<Vec<String>> {
        self.inner.ls_files()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff content between two commits for a file
    ///
    /// # Arguments
    /// * `from_ref` - Starting commit/branch/tag
    /// * `to_ref` - Ending commit/branch/tag
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn diff_content(&self, from_ref: String, to_ref: String, path: String) -> Result<String> {
        self.inner.diff_content(&from_ref, &to_ref, &path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff content for uncommitted changes in a file
    ///
    /// Includes both staged and unstaged changes compared to HEAD.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn uncommitted_diff(&self, path: String) -> Result<String> {
        self.inner.uncommitted_diff(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff for all uncommitted changes
    ///
    /// Returns combined diff for all changed files.
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn all_uncommitted_diffs(&self) -> Result<String> {
        self.inner.all_uncommitted_diffs()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Check if a file has uncommitted changes
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// True if file has changes, false otherwise
    #[napi]
    pub fn has_changes(&self, path: String) -> Result<bool> {
        self.inner.has_changes(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get the last commit that modified a file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Commit information object
    #[napi]
    pub fn last_modified_commit(&self, path: String) -> Result<GitCommit> {
        let commit = self.inner.last_modified_commit(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(GitCommit {
            hash: commit.hash,
            short_hash: commit.short_hash,
            author: commit.author,
            email: commit.email,
            date: commit.date,
            message: commit.message,
        })
    }

    /// Get file change frequency in recent days
    ///
    /// Useful for determining file importance based on recent activity.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `days` - Number of days to look back (default: 30)
    ///
    /// # Returns
    /// Number of commits that modified the file in the period
    #[napi]
    pub fn file_change_frequency(&self, path: String, days: Option<u32>) -> Result<u32> {
        self.inner.file_change_frequency(&path, days.unwrap_or(30))
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get file content at a specific git ref (commit, branch, tag)
    ///
    /// Uses `git show <ref>:<path>` to retrieve file content at that revision.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `git_ref` - Git ref (commit hash, branch name, tag, HEAD~n, etc.)
    ///
    /// # Returns
    /// File content as string
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const oldVersion = repo.fileAtRef('src/main.ts', 'HEAD~5');
    /// const mainVersion = repo.fileAtRef('src/main.ts', 'main');
    /// ```
    #[napi]
    pub fn file_at_ref(&self, path: String, git_ref: String) -> Result<String> {
        self.inner.file_at_ref(&path, &git_ref)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Parse diff between two refs into structured hunks
    ///
    /// Returns detailed hunk information including line numbers for each change.
    /// Useful for PR review tools that need to post comments at specific lines.
    ///
    /// # Arguments
    /// * `from_ref` - Starting ref (e.g., "main", "HEAD~5", commit hash)
    /// * `to_ref` - Ending ref (e.g., "HEAD", "feature-branch")
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks with line-level information
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.diffHunks('main', 'HEAD', 'src/index.ts');
    /// for (const hunk of hunks) {
    ///   console.log(`Hunk at old:${hunk.oldStart} new:${hunk.newStart}`);
    ///   for (const line of hunk.lines) {
    ///     console.log(`${line.changeType}: ${line.content}`);
    ///   }
    /// }
    /// ```
    #[napi]
    pub fn diff_hunks(
        &self,
        from_ref: String,
        to_ref: String,
        path: Option<String>,
    ) -> Result<Vec<GitDiffHunk>> {
        let hunks = self.inner.diff_hunks(&from_ref, &to_ref, path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }

    /// Parse uncommitted changes (working tree vs HEAD) into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks for uncommitted changes
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.uncommittedHunks('src/index.ts');
    /// console.log(`${hunks.length} hunks with uncommitted changes`);
    /// ```
    #[napi]
    pub fn uncommitted_hunks(&self, path: Option<String>) -> Result<Vec<GitDiffHunk>> {
        let hunks = self.inner.uncommitted_hunks(path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }

    /// Parse staged changes into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks for staged changes only
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.stagedHunks('src/index.ts');
    /// console.log(`${hunks.length} hunks staged for commit`);
    /// ```
    #[napi]
    pub fn staged_hunks(&self, path: Option<String>) -> Result<Vec<GitDiffHunk>> {
        let hunks = self.inner.staged_hunks(path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }
}

/// Convert engine DiffHunk to JS GitDiffHunk
fn convert_hunk(hunk: EngineGitDiffHunk) -> GitDiffHunk {
    GitDiffHunk {
        old_start: hunk.old_start,
        old_count: hunk.old_count,
        new_start: hunk.new_start,
        new_count: hunk.new_count,
        header: hunk.header,
        lines: hunk.lines.into_iter().map(|l| GitDiffLine {
            change_type: l.change_type.as_str().to_owned(),
            old_line: l.old_line,
            new_line: l.new_line,
            content: l.content,
        }).collect(),
    }
}

/// Security finding information
#[napi(object)]
pub struct SecurityFinding {
    /// File where the finding was detected
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Severity level: "Critical", "High", "Medium", "Low", "Info"
    pub severity: String,
    /// Type of finding
    pub kind: String,
    /// Matched pattern
    pub pattern: String,
}

/// Scan a repository for security issues
///
/// # Arguments
/// * `path` - Path to repository root
///
/// # Returns
/// Array of security findings
///
/// # Example
/// ```javascript
/// const { scanSecurity } = require('infiniloom-node');
///
/// const findings = scanSecurity('./my-repo');
/// for (const finding of findings) {
///   console.log(`${finding.severity}: ${finding.kind} in ${finding.file}:${finding.line}`);
/// }
/// ```
#[napi]
pub fn scan_security(path: String) -> Result<Vec<SecurityFinding>> {
    let repo = scan_repository_with_options(&path, TokenizerModel::Claude, true, true)?;

    let scanner = SecurityScanner::new();
    let mut findings = Vec::new();

    for file in &repo.files {
        if let Some(content) = &file.content {
            let file_findings = scanner.scan(content, &file.relative_path);
            for finding in file_findings {
                findings.push(SecurityFinding {
                    file: finding.file.clone(),
                    line: finding.line,
                    severity: format!("{:?}", finding.severity),
                    kind: finding.kind.name().to_string(),
                    pattern: finding.pattern.clone(),
                });
            }
        }
    }

    Ok(findings)
}

/// Format FileStatus as string
fn format_file_status(status: EngineFileStatus) -> String {
    match status {
        EngineFileStatus::Added => "Added".to_string(),
        EngineFileStatus::Modified => "Modified".to_string(),
        EngineFileStatus::Deleted => "Deleted".to_string(),
        EngineFileStatus::Renamed => "Renamed".to_string(),
        EngineFileStatus::Copied => "Copied".to_string(),
        EngineFileStatus::Unknown => "Unknown".to_string(),
    }
}

/// Format Unix timestamp to ISO 8601 string
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    // Simple ISO 8601 format
    format!("{:?}", datetime)
}

// ============================================================================
// Index API - Build and query symbol indexes
// ============================================================================

/// Options for building an index
#[napi(object)]
pub struct IndexOptions {
    /// Force full rebuild even if index exists
    pub force: Option<bool>,
    /// Include test files in index
    pub include_tests: Option<bool>,
    /// Maximum file size to index (bytes)
    pub max_file_size: Option<u32>,
}

/// Index status information
#[napi(object)]
pub struct IndexStatus {
    /// Whether an index exists
    pub exists: bool,
    /// Number of files indexed
    pub file_count: u32,
    /// Number of symbols indexed
    pub symbol_count: u32,
    /// Last build timestamp (ISO 8601)
    pub last_built: Option<String>,
    /// Index version
    pub version: Option<String>,
}

/// Build or update the symbol index for a repository
///
/// The index enables fast diff-to-context lookups and impact analysis.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional index build options
///
/// # Returns
/// Index status after building
///
/// # Example
/// ```javascript
/// const { buildIndex } = require('infiniloom-node');
///
/// const status = buildIndex('./my-repo');
/// console.log(`Indexed ${status.symbolCount} symbols`);
///
/// // Force rebuild
/// const status2 = buildIndex('./my-repo', { force: true });
/// ```
#[napi]
pub fn build_index(path: String, options: Option<IndexOptions>) -> Result<IndexStatus> {
    let opts = options.unwrap_or(IndexOptions {
        force: None,
        include_tests: None,
        max_file_size: None,
    });

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    // Check if we need to rebuild
    let force = opts.force.unwrap_or(false);

    if !force {
        // Check if index exists and is valid
        if let Ok(meta) = storage.load_meta() {
            if let (Ok(index), Ok(_graph)) = (storage.load_index(), storage.load_graph()) {
                return Ok(IndexStatus {
                    exists: true,
                    file_count: index.files.len() as u32,
                    symbol_count: index.symbols.len() as u32,
                    last_built: Some(format_timestamp(meta.created_at)),
                    version: Some(format!("v{}", meta.version)),
                });
            }
        }
    }

    // Build new index
    let mut exclude_dirs = vec![
        "node_modules".to_string(),
        "target".to_string(),
        ".git".to_string(),
        "dist".to_string(),
        "build".to_string(),
    ];

    // Exclude test directories if not including tests
    if !opts.include_tests.unwrap_or(false) {
        exclude_dirs.extend(vec![
            "test".to_string(),
            "tests".to_string(),
            "__tests__".to_string(),
            "spec".to_string(),
        ]);
    }

    let build_opts = BuildOptions {
        max_file_size: opts.max_file_size.map(|s| s as u64).unwrap_or(10 * 1024 * 1024),
        exclude_dirs,
        ..Default::default()
    };

    let builder = IndexBuilder::new(&path_buf).with_options(build_opts);
    let (index, graph) = builder.build()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to build index: {}", e)))?;

    // Save index
    storage.save_all(&index, &graph)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to save index: {}", e)))?;

    let meta = storage.load_meta()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load meta: {}", e)))?;

    Ok(IndexStatus {
        exists: true,
        file_count: index.files.len() as u32,
        symbol_count: index.symbols.len() as u32,
        last_built: Some(format_timestamp(meta.created_at)),
        version: Some(format!("v{}", meta.version)),
    })
}

/// Get the status of an existing index
///
/// # Arguments
/// * `path` - Path to repository root
///
/// # Returns
/// Index status information
///
/// # Example
/// ```javascript
/// const { indexStatus } = require('infiniloom-node');
///
/// const status = indexStatus('./my-repo');
/// if (status.exists) {
///   console.log(`Index has ${status.symbolCount} symbols`);
/// } else {
///   console.log('No index found, run buildIndex first');
/// }
/// ```
#[napi]
pub fn index_status(path: String) -> Result<IndexStatus> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    match (storage.load_meta(), storage.load_index()) {
        (Ok(meta), Ok(index)) => Ok(IndexStatus {
            exists: true,
            file_count: index.files.len() as u32,
            symbol_count: index.symbols.len() as u32,
            last_built: Some(format_timestamp(meta.created_at)),
            version: Some(format!("v{}", meta.version)),
        }),
        _ => Ok(IndexStatus {
            exists: false,
            file_count: 0,
            symbol_count: 0,
            last_built: None,
            version: None,
        }),
    }
}

// ============================================================================
// Call Graph API - Query symbol relationships
// ============================================================================

/// Information about a symbol in the call graph
#[napi(object)]
pub struct SymbolInfo {
    /// Symbol ID
    pub id: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// File path containing the symbol
    pub file: String,
    /// Start line number
    pub line: u32,
    /// End line number
    pub end_line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Visibility (public, private, etc.)
    pub visibility: String,
}

impl From<EngineSymbolInfo> for SymbolInfo {
    fn from(s: EngineSymbolInfo) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            file: s.file,
            line: s.line,
            end_line: s.end_line,
            signature: s.signature,
            visibility: s.visibility,
        }
    }
}

/// A reference to a symbol with context
#[napi(object)]
pub struct ReferenceInfo {
    /// Symbol making the reference
    pub symbol: SymbolInfo,
    /// Reference kind (call, import, inherit, implement)
    pub kind: String,
}

impl From<EngineReferenceInfo> for ReferenceInfo {
    fn from(r: EngineReferenceInfo) -> Self {
        Self {
            symbol: r.symbol.into(),
            kind: r.kind,
        }
    }
}

/// An edge in the call graph
#[napi(object)]
pub struct CallGraphEdge {
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
    /// Caller symbol name
    pub caller: String,
    /// Callee symbol name
    pub callee: String,
    /// File containing the call site
    pub file: String,
    /// Line number of the call
    pub line: u32,
}

impl From<EngineCallGraphEdge> for CallGraphEdge {
    fn from(e: EngineCallGraphEdge) -> Self {
        Self {
            caller_id: e.caller_id,
            callee_id: e.callee_id,
            caller: e.caller,
            callee: e.callee,
            file: e.file,
            line: e.line,
        }
    }
}

/// Call graph statistics
#[napi(object)]
pub struct CallGraphStats {
    /// Total number of symbols
    pub total_symbols: u32,
    /// Total number of call edges
    pub total_calls: u32,
    /// Number of functions/methods
    pub functions: u32,
    /// Number of classes/structs
    pub classes: u32,
}

impl From<EngineCallGraphStats> for CallGraphStats {
    fn from(s: EngineCallGraphStats) -> Self {
        Self {
            total_symbols: s.total_symbols as u32,
            total_calls: s.total_calls as u32,
            functions: s.functions as u32,
            classes: s.classes as u32,
        }
    }
}

/// Complete call graph with nodes and edges
#[napi(object)]
pub struct CallGraph {
    /// All symbols (nodes)
    pub nodes: Vec<SymbolInfo>,
    /// Call relationships (edges)
    pub edges: Vec<CallGraphEdge>,
    /// Summary statistics
    pub stats: CallGraphStats,
}

impl From<EngineCallGraph> for CallGraph {
    fn from(g: EngineCallGraph) -> Self {
        Self {
            nodes: g.nodes.into_iter().map(Into::into).collect(),
            edges: g.edges.into_iter().map(Into::into).collect(),
            stats: g.stats.into(),
        }
    }
}

/// Options for call graph queries
#[napi(object)]
pub struct CallGraphOptions {
    /// Maximum number of nodes to return (default: unlimited)
    pub max_nodes: Option<u32>,
    /// Maximum number of edges to return (default: unlimited)
    pub max_edges: Option<u32>,
}

/// Find a symbol by name
///
/// Searches the index for all symbols matching the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `name` - Symbol name to search for
///
/// # Returns
/// Array of matching symbols
///
/// # Example
/// ```javascript
/// const { findSymbol, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const symbols = findSymbol('./my-repo', 'processRequest');
/// console.log(`Found ${symbols.length} symbols named processRequest`);
/// ```
#[napi]
pub fn find_symbol(path: String, name: String) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    let results = engine_find_symbol(&index, &name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all callers of a symbol
///
/// Returns symbols that call any symbol with the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callers for
///
/// # Returns
/// Array of symbols that call the target symbol
///
/// # Example
/// ```javascript
/// const { getCallers, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callers = getCallers('./my-repo', 'authenticate');
/// console.log(`authenticate is called by ${callers.length} functions`);
/// for (const c of callers) {
///   console.log(`  ${c.name} at ${c.file}:${c.line}`);
/// }
/// ```
#[napi]
pub fn get_callers(path: String, symbol_name: String) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_callers_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all callees of a symbol
///
/// Returns symbols that are called by any symbol with the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callees for
///
/// # Returns
/// Array of symbols that the target symbol calls
///
/// # Example
/// ```javascript
/// const { getCallees, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callees = getCallees('./my-repo', 'main');
/// console.log(`main calls ${callees.length} functions`);
/// for (const c of callees) {
///   console.log(`  ${c.name} at ${c.file}:${c.line}`);
/// }
/// ```
#[napi]
pub fn get_callees(path: String, symbol_name: String) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_callees_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all references to a symbol
///
/// Returns all locations where a symbol is referenced (calls, imports, inheritance).
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find references for
///
/// # Returns
/// Array of reference information including the referencing symbol and kind
///
/// # Example
/// ```javascript
/// const { getReferences, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const refs = getReferences('./my-repo', 'UserService');
/// console.log(`UserService is referenced ${refs.length} times`);
/// for (const r of refs) {
///   console.log(`  ${r.kind}: ${r.symbol.name} at ${r.symbol.file}:${r.symbol.line}`);
/// }
/// ```
#[napi]
pub fn get_references(path: String, symbol_name: String) -> Result<Vec<ReferenceInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_references_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get the complete call graph
///
/// Returns all symbols and their call relationships.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional filtering options
///
/// # Returns
/// Call graph with nodes (symbols), edges (calls), and statistics
///
/// # Example
/// ```javascript
/// const { getCallGraph, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const graph = getCallGraph('./my-repo');
/// console.log(`Call graph: ${graph.stats.totalSymbols} symbols, ${graph.stats.totalCalls} calls`);
///
/// // Find most called functions
/// const callCounts = new Map();
/// for (const edge of graph.edges) {
///   callCounts.set(edge.callee, (callCounts.get(edge.callee) || 0) + 1);
/// }
/// const sorted = [...callCounts.entries()].sort((a, b) => b[1] - a[1]);
/// console.log('Most called functions:', sorted.slice(0, 10));
/// ```
#[napi]
pub fn get_call_graph(path: String, options: Option<CallGraphOptions>) -> Result<CallGraph> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let result = if let Some(opts) = options {
        get_call_graph_filtered(
            &index,
            &graph,
            opts.max_nodes.map(|n| n as usize),
            opts.max_edges.map(|n| n as usize),
        )
    } else {
        engine_get_call_graph(&index, &graph)
    };

    Ok(result.into())
}

/// Async version of findSymbol
#[napi]
pub async fn find_symbol_async(path: String, name: String) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || find_symbol(path, name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallers
#[napi]
pub async fn get_callers_async(path: String, symbol_name: String) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callers(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallees
#[napi]
pub async fn get_callees_async(path: String, symbol_name: String) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callees(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getReferences
#[napi]
pub async fn get_references_async(path: String, symbol_name: String) -> Result<Vec<ReferenceInfo>> {
    tokio::task::spawn_blocking(move || get_references(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallGraph
#[napi]
pub async fn get_call_graph_async(path: String, options: Option<CallGraphOptions>) -> Result<CallGraph> {
    tokio::task::spawn_blocking(move || get_call_graph(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

// ============================================================================
// Chunk API - Split repositories into manageable pieces
// ============================================================================

/// Options for chunking a repository
#[napi(object)]
pub struct ChunkOptions {
    /// Chunking strategy: "fixed", "file", "module", "symbol", "semantic", "dependency"
    pub strategy: Option<String>,
    /// Maximum tokens per chunk (default: 8000)
    pub max_tokens: Option<u32>,
    /// Token overlap between chunks (default: 0)
    pub overlap: Option<u32>,
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Output format: "xml", "markdown", "json" (default: "xml")
    pub format: Option<String>,
    /// Sort chunks by priority (core modules first)
    pub priority_first: Option<bool>,
}

/// A chunk of repository content
#[napi(object)]
pub struct RepoChunk {
    /// Chunk index (0-based)
    pub index: u32,
    /// Total number of chunks
    pub total: u32,
    /// Primary focus/topic of this chunk
    pub focus: String,
    /// Estimated token count
    pub tokens: u32,
    /// Files included in this chunk
    pub files: Vec<String>,
    /// Formatted content of the chunk
    pub content: String,
}

/// Split a repository into chunks for incremental processing
///
/// Useful for processing large repositories that exceed LLM context limits.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional chunking options
///
/// # Returns
/// Array of repository chunks
///
/// # Example
/// ```javascript
/// const { chunk } = require('infiniloom-node');
///
/// const chunks = chunk('./large-repo', {
///   strategy: 'module',
///   maxTokens: 50000,
///   model: 'claude'
/// });
///
/// for (const c of chunks) {
///   console.log(`Chunk ${c.index}/${c.total}: ${c.focus} (${c.tokens} tokens)`);
///   // Process c.content with LLM
/// }
/// ```
#[napi]
pub fn chunk(path: String, options: Option<ChunkOptions>) -> Result<Vec<RepoChunk>> {
    let opts = options.unwrap_or(ChunkOptions {
        strategy: None,
        max_tokens: None,
        overlap: None,
        model: None,
        format: None,
        priority_first: None,
    });

    let strategy = match opts.strategy.as_deref().unwrap_or("module") {
        "fixed" => ChunkStrategy::Fixed { size: opts.max_tokens.unwrap_or(8000) },
        "file" => ChunkStrategy::File,
        "module" => ChunkStrategy::Module,
        "symbol" => ChunkStrategy::Symbol,
        "semantic" => ChunkStrategy::Semantic,
        "dependency" => ChunkStrategy::Dependency,
        other => return Err(Error::new(
            Status::InvalidArg,
            format!("Unknown chunk strategy: {}. Use 'fixed', 'file', 'module', 'symbol', 'semantic', or 'dependency'", other),
        )),
    };

    let max_tokens = opts.max_tokens.unwrap_or(8000);
    let overlap = opts.overlap.unwrap_or(0);
    let model = parse_model(opts.model.as_deref())?;
    let format = parse_format(opts.format.as_deref())?;
    let priority_first = opts.priority_first.unwrap_or(false);

    // Scan repository
    let needs_symbols = matches!(strategy, ChunkStrategy::Dependency | ChunkStrategy::Symbol);
    let mut repo = scan_repository_with_options(&path, model, true, !needs_symbols)?;

    // Apply default ignores
    repo.files.retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));
    repo.files.retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));

    // Create chunker
    let chunker = Chunker::new(strategy, max_tokens)
        .with_model(model)
        .with_overlap(overlap);

    let mut chunks = chunker.chunk(&repo);

    // Apply priority sorting if requested
    if priority_first && chunks.len() > 1 {
        let mut chunk_priorities: Vec<(usize, f64)> = chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                let avg_priority = if chunk.files.is_empty() {
                    0.0
                } else {
                    let total: f64 = chunk
                        .files
                        .iter()
                        .map(|f| file_priority_score(&f.path))
                        .sum();
                    total / chunk.files.len() as f64
                };
                (i, avg_priority)
            })
            .collect();

        chunk_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let original_chunks = std::mem::take(&mut chunks);
        for (idx, (orig_idx, _)) in chunk_priorities.iter().enumerate() {
            let mut chunk = original_chunks[*orig_idx].clone();
            chunk.index = idx;
            chunks.push(chunk);
        }

        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.total = total;
        }
    }

    // Format each chunk
    // Note: formatter and map_generator are available if we want to format chunks
    // For now, we return raw content and let the caller format
    let _ = format; // Mark format as used (could use for chunk formatting later)

    let result: Vec<RepoChunk> = chunks
        .iter()
        .map(|c| {
            // Format chunk content manually since ChunkFile doesn't match RepoFile
            let content = c.files
                .iter()
                .map(|f| format!("// {}\n{}", f.path, f.content))
                .collect::<Vec<_>>()
                .join("\n\n");

            RepoChunk {
                index: c.index as u32,
                total: c.total as u32,
                focus: c.focus.clone(),
                tokens: c.tokens,
                files: c.files.iter().map(|f| f.path.clone()).collect(),
                content,
            }
        })
        .collect();

    Ok(result)
}

/// Calculate priority score for a file path
fn file_priority_score(path: &str) -> f64 {
    let path_lower = path.to_lowercase();

    // Core source files
    if path_lower.contains("src/") || path_lower.contains("lib/") {
        if path_lower.contains("main") || path_lower.contains("index") || path_lower.contains("app") {
            return 1.0;
        }
        return 0.8;
    }

    // Config files
    if path_lower.ends_with(".json") || path_lower.ends_with(".yaml") || path_lower.ends_with(".toml") {
        return 0.6;
    }

    // Test files
    if path_lower.contains("test") || path_lower.contains("spec") {
        return 0.3;
    }

    // Docs
    if path_lower.contains("doc") || path_lower.ends_with(".md") {
        return 0.2;
    }

    0.5
}

// ============================================================================
// Impact API - Analyze change impact
// ============================================================================

/// Options for impact analysis
#[napi(object)]
pub struct ImpactOptions {
    /// Depth of dependency traversal (1-3, default: 2)
    pub depth: Option<u32>,
    /// Include test files in analysis
    pub include_tests: Option<bool>,
}

/// Symbol affected by a change
#[napi(object)]
pub struct AffectedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, etc.)
    pub kind: String,
    /// File containing the symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// How the symbol is affected: "direct", "caller", "callee", "dependent"
    pub impact_type: String,
}

/// Impact analysis result
#[napi(object)]
pub struct ImpactResult {
    /// Files directly changed
    pub changed_files: Vec<String>,
    /// Files that depend on changed files
    pub dependent_files: Vec<String>,
    /// Related test files
    pub test_files: Vec<String>,
    /// Symbols affected by the changes
    pub affected_symbols: Vec<AffectedSymbol>,
    /// Overall impact level: "low", "medium", "high", "critical"
    pub impact_level: String,
    /// Summary of the impact
    pub summary: String,
}

/// Analyze the impact of changes to files or symbols
///
/// Requires an index to be built first (use buildIndex).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `files` - Files to analyze (can be paths or globs)
/// * `options` - Optional analysis options
///
/// # Returns
/// Impact analysis result
///
/// # Example
/// ```javascript
/// const { buildIndex, analyzeImpact } = require('infiniloom-node');
///
/// // Build index first
/// buildIndex('./my-repo');
///
/// // Analyze impact of changes
/// const impact = analyzeImpact('./my-repo', ['src/auth.ts']);
/// console.log(`Impact level: ${impact.impactLevel}`);
/// console.log(`Affected files: ${impact.dependentFiles.length}`);
/// ```
#[napi]
pub fn analyze_impact(path: String, files: Vec<String>, options: Option<ImpactOptions>) -> Result<ImpactResult> {
    let opts = options.unwrap_or(ImpactOptions {
        depth: None,
        include_tests: None,
    });

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    // Load index
    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index (run buildIndex first): {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load dependency graph: {}", e)))?;

    // Create context expander
    let depth = match opts.depth.unwrap_or(2) {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expander = ContextExpander::new(&index, &graph);

    // Convert files to diff changes
    let changes: Vec<DiffChange> = files.iter().map(|f| DiffChange {
        file_path: f.clone(),
        old_path: None,
        line_ranges: vec![],
        change_type: ChangeType::Modified,
        diff_content: None,
    }).collect();

    // Expand context (returns directly, not Result)
    let token_budget = 50000; // Default budget
    let context = expander.expand(&changes, depth, token_budget);

    // Collect results
    let changed_files: Vec<String> = changes.iter().map(|c| c.file_path.clone()).collect();

    let dependent_files: Vec<String> = context.dependent_files
        .iter()
        .map(|f| f.path.clone())
        .collect();

    let test_files: Vec<String> = context.related_tests
        .iter()
        .map(|f| f.path.clone())
        .collect();

    // Combine changed and dependent symbols
    let affected_symbols: Vec<AffectedSymbol> = context.changed_symbols
        .iter()
        .map(|s| AffectedSymbol {
            name: s.name.clone(),
            kind: s.kind.clone(),
            file: s.file_path.clone(),
            line: s.start_line,
            impact_type: s.relevance_reason.clone(),
        })
        .chain(context.dependent_symbols.iter().map(|s| AffectedSymbol {
            name: s.name.clone(),
            kind: s.kind.clone(),
            file: s.file_path.clone(),
            line: s.start_line,
            impact_type: s.relevance_reason.clone(),
        }))
        .collect();

    // Determine impact level
    let impact_level = if dependent_files.len() > 20 || affected_symbols.len() > 50 {
        "critical"
    } else if dependent_files.len() > 10 || affected_symbols.len() > 20 {
        "high"
    } else if dependent_files.len() > 5 || affected_symbols.len() > 10 {
        "medium"
    } else {
        "low"
    }.to_string();

    let summary = format!(
        "{} files changed, {} dependents affected, {} symbols impacted, {} tests related",
        changed_files.len(),
        dependent_files.len(),
        affected_symbols.len(),
        test_files.len()
    );

    Ok(ImpactResult {
        changed_files,
        dependent_files,
        test_files,
        affected_symbols,
        impact_level,
        summary,
    })
}

// ============================================================================
// Diff Context API - Get context-aware diffs
// ============================================================================

/// Options for diff context
#[napi(object)]
pub struct DiffContextOptions {
    /// Depth of context expansion (1-3, default: 2)
    pub depth: Option<u32>,
    /// Token budget for context (default: 50000)
    pub budget: Option<u32>,
    /// Include the actual diff content (default: false)
    pub include_diff: Option<bool>,
    /// Output format: "xml", "markdown", "json" (default: "xml")
    pub format: Option<String>,
}

/// Context-aware diff result
#[napi(object)]
pub struct DiffContextResult {
    /// Changed files with context
    pub changed_files: Vec<DiffFileContext>,
    /// Related symbols and their context
    pub context_symbols: Vec<ContextSymbolInfo>,
    /// Related test files
    pub related_tests: Vec<String>,
    /// Formatted output (if format specified)
    pub formatted_output: Option<String>,
    /// Total token count
    pub total_tokens: u32,
}

/// A changed file with surrounding context
#[napi(object)]
pub struct DiffFileContext {
    /// File path
    pub path: String,
    /// Change type: "Added", "Modified", "Deleted", "Renamed"
    pub change_type: String,
    /// Lines added
    pub additions: u32,
    /// Lines deleted
    pub deletions: u32,
    /// Unified diff content (if include_diff is true)
    pub diff: Option<String>,
    /// Relevant code context around changes
    pub context_snippets: Vec<String>,
}

/// Symbol context information
#[napi(object)]
pub struct ContextSymbolInfo {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File containing symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// Why this symbol is included: "changed", "caller", "callee", "dependent"
    pub reason: String,
    /// Symbol signature/definition
    pub signature: Option<String>,
}

/// Get context-aware diff with surrounding symbols and dependencies
///
/// Unlike basic diffFiles, this provides semantic context around changes.
/// Requires an index (will build on-the-fly if not present).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch (use "" for unstaged changes)
/// * `to_ref` - Ending commit/branch (use "HEAD" for staged, "" for working tree)
/// * `options` - Optional context options
///
/// # Returns
/// Context-aware diff result with related symbols
///
/// # Example
/// ```javascript
/// const { getDiffContext } = require('infiniloom-node');
///
/// // Get context for last commit
/// const context = getDiffContext('./my-repo', 'HEAD~1', 'HEAD', {
///   depth: 2,
///   budget: 50000,
///   includeDiff: true
/// });
///
/// console.log(`Changed: ${context.changedFiles.length} files`);
/// console.log(`Related symbols: ${context.contextSymbols.length}`);
/// console.log(`Related tests: ${context.relatedTests.length}`);
/// ```
#[napi]
pub fn get_diff_context(
    path: String,
    from_ref: String,
    to_ref: String,
    options: Option<DiffContextOptions>,
) -> Result<DiffContextResult> {
    let opts = options.unwrap_or(DiffContextOptions {
        depth: None,
        budget: None,
        include_diff: None,
        format: None,
    });

    let path_buf = PathBuf::from(&path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e)))?;

    // Get changed files
    let changed: Vec<ChangedFile> = if from_ref.is_empty() && to_ref.is_empty() {
        // Uncommitted changes
        git_repo.status()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
            .iter()
            .map(|f| ChangedFile {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: f.status,
                additions: 0,
                deletions: 0,
            })
            .collect()
    } else {
        let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
        let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };
        git_repo.diff_files(from, to)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    };

    // Try to load existing index, or use lazy context builder
    let storage = IndexStorage::new(&path_buf);
    let include_diff = opts.include_diff.unwrap_or(false);

    // Build file contexts
    let mut changed_files: Vec<DiffFileContext> = Vec::new();
    for file in &changed {
        let diff_content = if include_diff {
            let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
            let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };
            git_repo.diff_content(from, to, &file.path).ok()
        } else {
            None
        };

        changed_files.push(DiffFileContext {
            path: file.path.clone(),
            change_type: format_file_status(file.status),
            additions: file.additions,
            deletions: file.deletions,
            diff: diff_content,
            context_snippets: vec![],
        });
    }

    // Try to expand context if index exists
    let mut context_symbols: Vec<ContextSymbolInfo> = Vec::new();
    let mut related_tests: Vec<String> = Vec::new();

    if let (Ok(index), Ok(graph)) = (storage.load_index(), storage.load_graph()) {
        let depth = match opts.depth.unwrap_or(2) {
            1 => ContextDepth::L1,
            2 => ContextDepth::L2,
            _ => ContextDepth::L3,
        };

        let expander = ContextExpander::new(&index, &graph);
        let changes: Vec<DiffChange> = changed.iter().map(|f| DiffChange {
            file_path: f.path.clone(),
            old_path: f.old_path.clone(),
            line_ranges: vec![],
            change_type: match f.status {
                EngineFileStatus::Added => ChangeType::Added,
                EngineFileStatus::Deleted => ChangeType::Deleted,
                _ => ChangeType::Modified,
            },
            diff_content: None,
        }).collect();

        let token_budget = opts.budget.unwrap_or(50000);
        let context = expander.expand(&changes, depth, token_budget);
        {
            // Combine changed and dependent symbols
            context_symbols = context.changed_symbols
                .iter()
                .chain(context.dependent_symbols.iter())
                .map(|s| ContextSymbolInfo {
                    name: s.name.clone(),
                    kind: s.kind.clone(),
                    file: s.file_path.clone(),
                    line: s.start_line,
                    reason: s.relevance_reason.clone(),
                    signature: s.signature.clone(),
                })
                .collect();

            related_tests = context.related_tests
                .iter()
                .map(|f| f.path.clone())
                .collect();
        }
    }

    // Calculate tokens
    let tokenizer = Tokenizer::new();
    let total_content: String = changed_files
        .iter()
        .filter_map(|f| f.diff.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let total_tokens = tokenizer.count(&total_content, TokenModel::Claude);

    Ok(DiffContextResult {
        changed_files,
        context_symbols,
        related_tests,
        formatted_output: None,
        total_tokens,
    })
}

// ============================================================================
// Async API - Async versions of key functions
// ============================================================================

/// Async version of pack
///
/// # Example
/// ```javascript
/// const { packAsync } = require('infiniloom-node');
///
/// const context = await packAsync('./my-repo', { format: 'xml' });
/// ```
#[napi]
pub async fn pack_async(path: String, options: Option<PackOptions>) -> Result<String> {
    // Run synchronous pack in a blocking task
    tokio::task::spawn_blocking(move || pack(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of scan
///
/// # Example
/// ```javascript
/// const { scanAsync } = require('infiniloom-node');
///
/// const stats = await scanAsync('./my-repo', 'claude');
/// ```
#[napi]
pub async fn scan_async(path: String, model: Option<String>) -> Result<ScanStats> {
    tokio::task::spawn_blocking(move || scan(path, model))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of buildIndex
///
/// # Example
/// ```javascript
/// const { buildIndexAsync } = require('infiniloom-node');
///
/// const status = await buildIndexAsync('./my-repo', { force: true });
/// ```
#[napi]
pub async fn build_index_async(path: String, options: Option<IndexOptions>) -> Result<IndexStatus> {
    tokio::task::spawn_blocking(move || build_index(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of chunk
///
/// # Example
/// ```javascript
/// const { chunkAsync } = require('infiniloom-node');
///
/// const chunks = await chunkAsync('./large-repo', { maxTokens: 50000 });
/// ```
#[napi]
pub async fn chunk_async(path: String, options: Option<ChunkOptions>) -> Result<Vec<RepoChunk>> {
    tokio::task::spawn_blocking(move || chunk(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of analyzeImpact
///
/// # Example
/// ```javascript
/// const { analyzeImpactAsync } = require('infiniloom-node');
///
/// const impact = await analyzeImpactAsync('./my-repo', ['src/auth.ts']);
/// ```
#[napi]
pub async fn analyze_impact_async(
    path: String,
    files: Vec<String>,
    options: Option<ImpactOptions>,
) -> Result<ImpactResult> {
    tokio::task::spawn_blocking(move || analyze_impact(path, files, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getDiffContext
///
/// # Example
/// ```javascript
/// const { getDiffContextAsync } = require('infiniloom-node');
///
/// const context = await getDiffContextAsync('./my-repo', 'HEAD~1', 'HEAD');
/// ```
#[napi]
pub async fn get_diff_context_async(
    path: String,
    from_ref: String,
    to_ref: String,
    options: Option<DiffContextOptions>,
) -> Result<DiffContextResult> {
    tokio::task::spawn_blocking(move || get_diff_context(path, from_ref, to_ref, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}
