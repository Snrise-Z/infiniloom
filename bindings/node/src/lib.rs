#![deny(clippy::all)]

// Import from infiniloom-bindings-common
use infiniloom_bindings_common::{
    parse_format, parse_model, parse_compression,
    file_priority_score,
    format_file_status as common_format_file_status,
    // Scanner from common crate
    scan_repository as do_scan, ScanConfig, matches_any_pattern,
    // Security utilities
    parse_security_threshold as common_parse_security_threshold,
    severity_at_or_above,
    // Time utilities
    format_timestamp,
    // Repository operations
    apply_compression, apply_default_ignores, prepare_repository, apply_token_budget,
};

use infiniloom_engine::{
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    git::{
        GitRepo as EngineGitRepo, FileStatus as EngineFileStatus, ChangedFile,
        DiffHunk as EngineGitDiffHunk,
    },
    security::Severity,
    CompressionLevel, OutputFormat, OutputFormatter, RepoMapGenerator, Repository, SecurityScanner,
    SemanticCompressor, SemanticConfig, TokenizerModel, Tokenizer, tokenizer::TokenModel,
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
    let format = napi_parse_format(opts.format.as_deref())?;
    let model = napi_parse_model(opts.model.as_deref())?;
    let compression = napi_parse_compression(opts.compression.as_deref())?;
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

    // Prepare repository (count references, rank files, sort by importance)
    prepare_repository(&mut repo);

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

    // Apply compression to file contents
    apply_compression(&mut repo, compression);

    // Apply token budget to limit output size (Bug #7 fix)
    // Files are already sorted by importance, so we keep top files until budget is reached
    if token_budget > 0 {
        apply_token_budget(&mut repo, token_budget, model);
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

    let tokenizer_model = napi_parse_model(opts.model.as_deref())?;
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
    let token_model = napi_parse_model(model.as_deref())?;
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
        let tokenizer_model = napi_parse_model(model.as_deref())?;
        let mut repo = scan_repository(&path, tokenizer_model, true)?;

        // Apply default ignores to filter out build outputs, dependencies, test fixtures, etc.
        repo.files.retain(|f| {
            !matches_any(&f.relative_path, DEFAULT_IGNORES)
                && !matches_any(&f.relative_path, TEST_IGNORES)
        });

        // Prepare repository (count references, rank files, sort by importance)
        prepare_repository(&mut repo);

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

        let format = napi_parse_format(opts.format.as_deref())?;
        let compression = napi_parse_compression(opts.compression.as_deref())?;
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
        apply_compression(&mut repo, compression);

        // Apply token budget to limit output size (Bug #7 fix)
        if token_budget > 0 {
            apply_token_budget(&mut repo, token_budget, self.model);
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

fn napi_parse_format(format: Option<&str>) -> Result<OutputFormat> {
    parse_format(format).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

fn napi_parse_model(model: Option<&str>) -> Result<TokenizerModel> {
    parse_model(model).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

fn napi_parse_compression(compression: Option<&str>) -> Result<CompressionLevel> {
    parse_compression(compression).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

/// Parse security severity threshold (Bug #5 fix)
fn parse_security_threshold(threshold: Option<&str>) -> Result<Severity> {
    common_parse_security_threshold(threshold).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
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
    common_format_file_status(status).to_string()
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
    // Convenience fields for easier access (mirrors symbol fields)
    /// File path containing the reference (convenience field, same as symbol.file)
    pub file: String,
    /// Line number of the reference (convenience field, same as symbol.line)
    pub line: u32,
}

impl From<EngineReferenceInfo> for ReferenceInfo {
    fn from(r: EngineReferenceInfo) -> Self {
        let symbol: SymbolInfo = r.symbol.into();
        let file = symbol.file.clone();
        let line = symbol.line;
        Self {
            symbol,
            kind: r.kind,
            file,
            line,
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
    let model = napi_parse_model(opts.model.as_deref())?;
    let format = napi_parse_format(opts.format.as_deref())?;
    let priority_first = opts.priority_first.unwrap_or(false);

    // Scan repository
    let needs_symbols = matches!(strategy, ChunkStrategy::Dependency | ChunkStrategy::Symbol);
    let mut repo = scan_repository_with_options(&path, model, true, !needs_symbols)?;

    // Apply default ignores
    apply_default_ignores(&mut repo);

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

    // Convert files to diff changes, getting line ranges for all symbols in each file
    // Bug #4 fix: Ensure line_ranges are never empty so symbols are always found
    let changes: Vec<DiffChange> = files.iter().map(|f| {
        // Get all symbol line ranges from this file
        let line_ranges = if let Some(file_entry) = index.get_file(f) {
            // Include all lines where symbols are defined
            let symbols = index.get_file_symbols(file_entry.id);
            if symbols.is_empty() {
                // If no symbols, assume entire file is changed
                vec![(1, file_entry.lines.max(1))]
            } else {
                symbols.iter()
                    .map(|s| (s.span.start_line, s.span.end_line))
                    .collect()
            }
        } else {
            // File not in index - use a large range to capture potential symbols
            vec![(1, 10000)]
        };

        DiffChange {
            file_path: f.clone(),
            old_path: None,
            line_ranges,
            change_type: ChangeType::Modified,
            diff_content: None,
        }
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

    let mut test_files: Vec<String> = context.related_tests
        .iter()
        .map(|f| f.path.clone())
        .collect();

    // Bug #4 fix: If no related tests found via expander, try direct test detection
    if test_files.is_empty() {
        let mut seen_tests: HashSet<String> = HashSet::new();

        // Helper to check if a file is a test file
        let is_test_file = |path: &str| -> bool {
            let path_lower = path.to_lowercase();
            path_lower.contains("test")
                || path_lower.contains("spec")
                || path_lower.contains("__tests__")
                || path_lower.ends_with("_test.rs")
                || path_lower.ends_with("_test.go")
                || path_lower.ends_with("_test.py")
                || path_lower.ends_with(".test.ts")
                || path_lower.ends_with(".test.js")
                || path_lower.ends_with(".spec.ts")
                || path_lower.ends_with(".spec.js")
        };

        for changed_path in &files {
            // Method 1: Find test files that import the changed file
            if let Some(file_entry) = index.get_file(changed_path) {
                let importers = graph.get_importers(file_entry.id.as_u32());
                for importer_id in importers {
                    if let Some(importer_file) = index.get_file_by_id(importer_id) {
                        if is_test_file(&importer_file.path) && seen_tests.insert(importer_file.path.clone()) {
                            test_files.push(importer_file.path.clone());
                        }
                    }
                }
            }

            // Method 2: Find test files by naming convention
            let path_lower = changed_path.to_lowercase();
            let base_name = std::path::Path::new(&path_lower)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            if !base_name.is_empty() {
                let test_patterns = [
                    format!("{}_test.", base_name),
                    format!("test_{}", base_name),
                    format!("{}.test.", base_name),
                    format!("{}.spec.", base_name),
                    format!("test/{}", base_name),
                    format!("tests/{}", base_name),
                    format!("__tests__/{}", base_name),
                ];

                for indexed_file in &index.files {
                    if is_test_file(&indexed_file.path) {
                        let file_lower = indexed_file.path.to_lowercase();
                        for pattern in &test_patterns {
                            if file_lower.contains(pattern) && seen_tests.insert(indexed_file.path.clone()) {
                                test_files.push(indexed_file.path.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

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

    // Parse diff hunks to get line ranges for each file
    let mut file_line_ranges: std::collections::HashMap<String, Vec<(u32, u32)>> = std::collections::HashMap::new();
    let mut file_diff_contents: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    for file in &changed {
        // Get diff hunks for this file
        let hunks = if from_ref.is_empty() && to_ref.is_empty() {
            git_repo.uncommitted_hunks(Some(&file.path)).unwrap_or_default()
        } else {
            git_repo.diff_hunks(from, to, Some(&file.path)).unwrap_or_default()
        };

        // Extract line ranges from hunks (new lines that were added/modified)
        let mut line_ranges = Vec::new();
        for hunk in &hunks {
            // The new_start/new_count gives us the range of lines in the new version
            if hunk.new_count > 0 {
                line_ranges.push((hunk.new_start, hunk.new_start + hunk.new_count - 1));
            }
        }

        if !line_ranges.is_empty() {
            file_line_ranges.insert(file.path.clone(), line_ranges);
        }

        // Get diff content if requested or for context expansion
        let diff_content = git_repo.diff_content(from, to, &file.path).ok();
        if let Some(ref content) = diff_content {
            file_diff_contents.insert(file.path.clone(), content.clone());
        }
    }

    // Build file contexts
    let mut changed_files: Vec<DiffFileContext> = Vec::new();
    for file in &changed {
        let diff_content = if include_diff {
            file_diff_contents.get(&file.path).cloned()
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
    let mut file_snippets: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    // Bug #1, #2, #3 fixes: Improved context expansion
    if let (Ok(index), Ok(graph)) = (storage.load_index(), storage.load_graph()) {
        let depth = match opts.depth.unwrap_or(2) {
            1 => ContextDepth::L1,
            2 => ContextDepth::L2,
            _ => ContextDepth::L3,
        };

        let expander = ContextExpander::new(&index, &graph);
        let changes: Vec<DiffChange> = changed.iter().map(|f| {
            // Get line ranges from diff hunks
            let mut line_ranges = file_line_ranges.get(&f.path).cloned().unwrap_or_default();

            // Bug #1 fix: If no line ranges found but file exists in index, include all lines
            // This ensures we capture symbols even when hunk parsing fails or for new files
            if line_ranges.is_empty() {
                if let Some(file_entry) = index.get_file(&f.path) {
                    // For added files, include all lines
                    if f.status == EngineFileStatus::Added {
                        line_ranges = vec![(1, file_entry.lines.max(1))];
                    } else if f.status != EngineFileStatus::Deleted {
                        // For modified files with no hunks, include all symbol ranges
                        let symbols = index.get_file_symbols(file_entry.id);
                        if symbols.is_empty() {
                            // No symbols found - include entire file
                            line_ranges = vec![(1, file_entry.lines.max(1))];
                        } else {
                            line_ranges = symbols.iter()
                                .map(|s| (s.span.start_line, s.span.end_line))
                                .collect();
                        }
                    }
                } else {
                    // File not in index yet - include as entire file change
                    line_ranges = vec![(1, 10000)]; // Large range to capture all symbols
                }
            }

            DiffChange {
                file_path: f.path.clone(),
                old_path: f.old_path.clone(),
                line_ranges,
                change_type: match f.status {
                    EngineFileStatus::Added => ChangeType::Added,
                    EngineFileStatus::Deleted => ChangeType::Deleted,
                    _ => ChangeType::Modified,
                },
                diff_content: file_diff_contents.get(&f.path).cloned(),
            }
        }).collect();

        let token_budget = opts.budget.unwrap_or(50000);
        let context = expander.expand(&changes, depth, token_budget);

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

        // Bug #2 fix: Always try direct test detection (expander may miss some tests)
        {
            let mut seen_tests: HashSet<String> = related_tests.iter().cloned().collect();

            // Helper to check if a file is a test file
            let is_test_file = |path: &str| -> bool {
                let path_lower = path.to_lowercase();
                path_lower.contains("test")
                    || path_lower.contains("spec")
                    || path_lower.contains("__tests__")
                    || path_lower.ends_with("_test.rs")
                    || path_lower.ends_with("_test.go")
                    || path_lower.ends_with("_test.py")
                    || path_lower.ends_with(".test.ts")
                    || path_lower.ends_with(".test.js")
                    || path_lower.ends_with(".spec.ts")
                    || path_lower.ends_with(".spec.js")
            };

            for changed_file in &changed {
                // Method 1: Find test files that import the changed file
                if let Some(file_entry) = index.get_file(&changed_file.path) {
                    let importers = graph.get_importers(file_entry.id.as_u32());
                    for importer_id in importers {
                        if let Some(importer_file) = index.get_file_by_id(importer_id) {
                            if is_test_file(&importer_file.path) && seen_tests.insert(importer_file.path.clone()) {
                                related_tests.push(importer_file.path.clone());
                            }
                        }
                    }
                }

                // Method 2: Find test files by naming convention
                let path_lower = changed_file.path.to_lowercase();
                let base_name = std::path::Path::new(&path_lower)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if !base_name.is_empty() {
                    let test_patterns = [
                        format!("{}_test.", base_name),
                        format!("test_{}", base_name),
                        format!("{}.test.", base_name),
                        format!("{}.spec.", base_name),
                        format!("test/{}", base_name),
                        format!("tests/{}", base_name),
                        format!("__tests__/{}", base_name),
                    ];

                    for indexed_file in &index.files {
                        if is_test_file(&indexed_file.path) {
                            let file_lower = indexed_file.path.to_lowercase();
                            for pattern in &test_patterns {
                                if file_lower.contains(pattern) && seen_tests.insert(indexed_file.path.clone()) {
                                    related_tests.push(indexed_file.path.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Bug #3 fix: Generate snippets directly from changed files
        // The expander's snippets field is not populated, so we generate them here
        for cf in &changed_files {
            let full_path = path_buf.join(&cf.path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let lines: Vec<&str> = content.lines().collect();
                let mut snippets = Vec::new();

                // Get line ranges for this file
                if let Some(ranges) = file_line_ranges.get(&cf.path) {
                    // Generate snippet for each changed section with context
                    for (start, end) in ranges {
                        let context_lines = 3; // Lines of context before/after
                        let snippet_start = (*start as usize).saturating_sub(context_lines + 1);
                        let snippet_end = ((*end as usize) + context_lines).min(lines.len());

                        if snippet_start < lines.len() && snippet_start < snippet_end {
                            let snippet_content = lines[snippet_start..snippet_end].join("\n");
                            if !snippet_content.is_empty() {
                                snippets.push(format!(
                                    "// Lines {}-{}\n{}",
                                    snippet_start + 1, snippet_end,
                                    snippet_content
                                ));
                            }
                        }
                    }
                }

                if !snippets.is_empty() {
                    file_snippets.insert(cf.path.clone(), snippets);
                }
            }
        }
    }

    // Update changed_files with snippets from context expansion
    for file_ctx in &mut changed_files {
        if let Some(snippets) = file_snippets.remove(&file_ctx.path) {
            file_ctx.context_snippets = snippets;
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

// ============================================================================
// New High-Priority Features for PR Review
// ============================================================================

/// Options for filtering symbols
#[napi(object)]
pub struct SymbolFilter {
    /// Filter by symbol kind: "function", "class", "method", etc.
    pub kind: Option<String>,
    /// Filter by visibility: "public", "private", "protected"
    pub visibility: Option<String>,
}

/// A call site where a symbol is called
#[napi(object)]
pub struct CallSite {
    /// Name of the calling function/method
    pub caller: String,
    /// Name of the function/method being called
    pub callee: String,
    /// File containing the call
    pub file: String,
    /// Line number of the call (1-indexed)
    pub line: u32,
    /// Column number of the call (0-indexed, if available)
    pub column: Option<u32>,
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
}

/// Get all symbols in a specific file
///
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `file_path` - Relative path to the file within the repository
/// * `filter` - Optional filter for symbol kind/visibility
///
/// # Returns
/// Array of symbols defined in the file
///
/// # Example
/// ```javascript
/// const { getSymbolsInFile, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const symbols = getSymbolsInFile('./my-repo', 'src/auth.ts');
/// console.log(`Found ${symbols.length} symbols in auth.ts`);
/// for (const s of symbols) {
///   console.log(`  ${s.kind}: ${s.name} at line ${s.line}`);
/// }
/// ```
#[napi]
pub fn get_symbols_in_file(
    path: String,
    file_path: String,
    filter: Option<SymbolFilter>,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Get file entry
    let file = index.get_file(&file_path)
        .ok_or_else(|| Error::new(Status::GenericFailure, format!("File not found in index: {}", file_path)))?;

    // Get all symbols in this file
    let symbols = index.get_file_symbols(file.id);

    // Filter and convert to SymbolInfo
    let mut results: Vec<SymbolInfo> = symbols
        .iter()
        .filter(|s| {
            if let Some(ref f) = filter {
                // Filter by kind
                if let Some(ref kind) = f.kind {
                    if s.kind.name() != kind.as_str() {
                        return false;
                    }
                }
                // Filter by visibility
                if let Some(ref vis) = f.visibility {
                    let sym_vis = match s.visibility {
                        infiniloom_engine::index::Visibility::Public => "public",
                        infiniloom_engine::index::Visibility::Private => "private",
                        infiniloom_engine::index::Visibility::Protected => "protected",
                        infiniloom_engine::index::Visibility::Internal => "internal",
                    };
                    if sym_vis != vis.as_str() {
                        return false;
                    }
                }
            }
            true
        })
        .map(|sym| {
            use infiniloom_engine::index::query::SymbolInfo as EngineSymbolInfo;
            EngineSymbolInfo::from_index_symbol(sym, &index).into()
        })
        .collect();

    // Sort by line number
    results.sort_by_key(|s| s.line);
    Ok(results)
}

/// Get the source code of a symbol
///
/// Reads the file and extracts the source code for the specified symbol.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to get source for
/// * `file_path` - Optional file path to disambiguate when multiple symbols have the same name
///
/// # Returns
/// Source code of the symbol (or the first matching symbol if multiple exist)
///
/// # Example
/// ```javascript
/// const { getSymbolSource, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const source = getSymbolSource('./my-repo', 'authenticate', 'src/auth.ts');
/// console.log(source);
/// ```
#[napi]
pub fn get_symbol_source(
    path: String,
    symbol_name: String,
    file_path: Option<String>,
) -> Result<String> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Find the symbol
    let symbols = index.find_symbols(&symbol_name);
    if symbols.is_empty() {
        return Err(Error::new(Status::GenericFailure, format!("Symbol not found: {}", symbol_name)));
    }

    // Filter by file path if specified
    let symbol = if let Some(ref fp) = file_path {
        symbols.iter().find(|s| {
            index.get_file_by_id(s.file_id.as_u32())
                .is_some_and(|f| f.path == *fp)
        }).or_else(|| symbols.first())
    } else {
        symbols.first()
    };

    let symbol = symbol.ok_or_else(|| Error::new(Status::GenericFailure, format!("Symbol not found: {}", symbol_name)))?;

    // Get file path
    let file = index.get_file_by_id(symbol.file_id.as_u32())
        .ok_or_else(|| Error::new(Status::GenericFailure, "File not found in index"))?;

    // Read file content
    let full_path = path_buf.join(&file.path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to read file: {}", e)))?;

    // Extract the symbol source (lines are 1-indexed)
    let lines: Vec<&str> = content.lines().collect();
    let start = (symbol.span.start_line as usize).saturating_sub(1);
    let end = (symbol.span.end_line as usize).min(lines.len());

    if start >= lines.len() {
        return Err(Error::new(Status::GenericFailure, "Symbol line numbers out of range"));
    }

    let source = lines[start..end].join("\n");
    Ok(source)
}

/// Get symbols that were changed in a diff
///
/// Parses the diff between two refs and identifies which symbols were modified.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch (e.g., "main", "HEAD~1")
/// * `to_ref` - Ending commit/branch (e.g., "HEAD", "feature-branch")
///
/// # Returns
/// Array of symbols that were modified in the diff
///
/// # Example
/// ```javascript
/// const { getChangedSymbols, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const changed = getChangedSymbols('./my-repo', 'main', 'HEAD');
/// console.log(`${changed.length} symbols were modified`);
/// for (const s of changed) {
///   console.log(`  ${s.kind}: ${s.name} in ${s.file}`);
/// }
/// ```
#[napi]
pub fn get_changed_symbols(
    path: String,
    from_ref: String,
    to_ref: String,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e)))?;

    // Load index
    let storage = IndexStorage::new(&path_buf);
    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Get changed files
    let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    let changed_files = git_repo.diff_files(from, to)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let mut changed_symbols: Vec<SymbolInfo> = Vec::new();
    let mut seen_ids: HashSet<u32> = HashSet::new();

    for file in changed_files {
        // Get diff hunks to find changed lines
        let hunks = git_repo.diff_hunks(from, to, Some(&file.path)).unwrap_or_default();

        // Get file from index
        let file_entry = match index.get_file(&file.path) {
            Some(f) => f,
            None => continue, // File might be new or not indexed
        };

        // Find symbols that overlap with changed lines
        for hunk in hunks {
            if hunk.new_count == 0 {
                continue;
            }

            let start_line = hunk.new_start;
            let end_line = hunk.new_start + hunk.new_count;

            // Find symbols that overlap with this hunk
            for sym in index.get_file_symbols(file_entry.id) {
                // Check if symbol overlaps with changed lines
                let sym_overlaps = sym.span.start_line <= end_line && sym.span.end_line >= start_line;

                if sym_overlaps && !seen_ids.contains(&sym.id.as_u32()) {
                    seen_ids.insert(sym.id.as_u32());
                    use infiniloom_engine::index::query::SymbolInfo as EngineSymbolInfo;
                    changed_symbols.push(EngineSymbolInfo::from_index_symbol(sym, &index).into());
                }
            }
        }
    }

    // Sort by file and line
    changed_symbols.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(changed_symbols)
}

/// Get test files related to a source file
///
/// Finds test files that:
/// 1. Import the specified file
/// 2. Match common test naming conventions (e.g., foo.ts -> foo.test.ts, test_foo.py)
///
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `file_path` - Relative path to the source file
///
/// # Returns
/// Array of test file paths related to the source file
///
/// # Example
/// ```javascript
/// const { getTestsForFile, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const tests = getTestsForFile('./my-repo', 'src/auth.ts');
/// console.log(`Found ${tests.length} test files for auth.ts`);
/// for (const t of tests) {
///   console.log(`  ${t}`);
/// }
/// ```
#[napi]
pub fn get_tests_for_file(
    path: String,
    file_path: String,
) -> Result<Vec<String>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    // Get file entry
    let file = index.get_file(&file_path);
    let file_id = file.map(|f| f.id.as_u32());

    let mut test_files: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Helper to check if a file is a test file
    let is_test_file = |path: &str| -> bool {
        let path_lower = path.to_lowercase();
        path_lower.contains("test")
            || path_lower.contains("spec")
            || path_lower.contains("__tests__")
            || path_lower.ends_with("_test.rs")
            || path_lower.ends_with("_test.go")
            || path_lower.ends_with("_test.py")
            || path_lower.ends_with(".test.ts")
            || path_lower.ends_with(".test.js")
            || path_lower.ends_with(".spec.ts")
            || path_lower.ends_with(".spec.js")
    };

    // Method 1: Find test files that import this file
    if let Some(fid) = file_id {
        let importers = graph.get_importers(fid);
        for importer_id in importers {
            if let Some(importer_file) = index.get_file_by_id(importer_id) {
                if is_test_file(&importer_file.path) && seen.insert(importer_file.path.clone()) {
                    test_files.push(importer_file.path.clone());
                }
            }
        }
    }

    // Method 2: Find test files by naming convention
    let path_lower = file_path.to_lowercase();
    let base_name = std::path::Path::new(&path_lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if !base_name.is_empty() {
        let test_patterns = [
            format!("{}_test.", base_name),
            format!("test_{}", base_name),
            format!("{}.test.", base_name),
            format!("{}.spec.", base_name),
            format!("test/{}", base_name),
            format!("tests/{}", base_name),
            format!("__tests__/{}", base_name),
        ];

        for indexed_file in &index.files {
            if is_test_file(&indexed_file.path) {
                let file_lower = indexed_file.path.to_lowercase();
                for pattern in &test_patterns {
                    if file_lower.contains(pattern) && seen.insert(indexed_file.path.clone()) {
                        test_files.push(indexed_file.path.clone());
                        break;
                    }
                }
            }
        }
    }

    Ok(test_files)
}

/// Get call sites where a symbol is called
///
/// Returns the locations where a function/method is called, with exact line numbers.
/// This is useful for PR review tools that need to post inline comments.
///
/// The function scans the caller's body to find the actual line where the callee is called,
/// rather than just returning the caller's definition line.
///
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find call sites for
///
/// # Returns
/// Array of call sites with caller information and line numbers
///
/// # Example
/// ```javascript
/// const { getCallSites, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callSites = getCallSites('./my-repo', 'authenticate');
/// console.log(`authenticate is called from ${callSites.length} locations`);
/// for (const site of callSites) {
///   console.log(`  ${site.caller} in ${site.file}:${site.line}`);
/// }
/// ```
#[napi]
pub fn get_call_sites(path: String, symbol_name: String) -> Result<Vec<CallSite>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let mut call_sites: Vec<CallSite> = Vec::new();
    // Bug #5 fix: Track seen call sites to prevent duplicates
    let mut seen_sites: HashSet<(String, u32, u32, u32)> = HashSet::new(); // (file, line, caller_id, callee_id)

    // Cache file contents to avoid re-reading
    let mut file_cache: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    // Find all symbols with this name
    for sym in index.find_symbols(&symbol_name) {
        let callee_id = sym.id.as_u32();

        // Get all callers of this symbol
        for caller_id in graph.get_callers(callee_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                let file_path = index
                    .get_file_by_id(caller_sym.file_id.as_u32())
                    .map(|f| f.path.clone())
                    .unwrap_or_else(|| "<unknown>".to_owned());

                // Try to find exact call site by scanning the caller's body
                let (call_line, call_col) = find_call_site_in_body(
                    &path_buf,
                    &file_path,
                    caller_sym.span.start_line,
                    caller_sym.span.end_line,
                    &symbol_name,
                    &mut file_cache,
                );

                // Bug #5 fix: Deduplicate call sites
                let site_key = (file_path.clone(), call_line, caller_id, callee_id);
                if seen_sites.insert(site_key) {
                    call_sites.push(CallSite {
                        caller: caller_sym.name.clone(),
                        callee: sym.name.clone(),
                        file: file_path,
                        line: call_line,
                        column: call_col,
                        caller_id,
                        callee_id,
                    });
                }
            }
        }
    }

    // Sort by file and line
    call_sites.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(call_sites)
}

/// Helper function to find the actual call site within a caller's body
fn find_call_site_in_body(
    repo_root: &PathBuf,
    file_path: &str,
    start_line: u32,
    end_line: u32,
    callee_name: &str,
    file_cache: &mut std::collections::HashMap<String, Vec<String>>,
) -> (u32, Option<u32>) {
    // Try to load file content
    let lines = if let Some(cached) = file_cache.get(file_path) {
        cached.clone()
    } else {
        let full_path = repo_root.join(file_path);
        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                file_cache.insert(file_path.to_string(), lines.clone());
                lines
            }
            Err(_) => return (start_line, None), // Fall back to definition line
        }
    };

    // Search for the callee name within the caller's body
    // Skip the first line (function signature) and look for actual calls
    let search_start = (start_line as usize).saturating_sub(1);
    let search_end = (end_line as usize).min(lines.len());

    for line_idx in search_start..search_end {
        let line = &lines[line_idx];

        // Look for the callee name followed by ( to indicate a call
        // This is a heuristic - not perfect but covers most cases
        if let Some(col) = find_call_in_line(line, callee_name) {
            return ((line_idx + 1) as u32, Some(col as u32));
        }
    }

    // Fall back to the caller's start line if we can't find the exact call
    (start_line, None)
}

/// Find a function call in a line of code
/// Returns the column position if found
fn find_call_in_line(line: &str, callee_name: &str) -> Option<usize> {
    // Pattern: identifier followed by ( but not preceded by "def ", "fn ", "function ", etc.
    let mut search_pos = 0;

    while let Some(pos) = line[search_pos..].find(callee_name) {
        let abs_pos = search_pos + pos;

        // Check if this is actually a call (followed by parenthesis)
        let after_name = abs_pos + callee_name.len();
        if after_name < line.len() {
            let rest = &line[after_name..];
            let next_non_ws = rest.trim_start();
            if next_non_ws.starts_with('(') {
                // Check it's not a definition (preceded by def/fn/function/etc.)
                let before = &line[..abs_pos];
                let before_trimmed = before.trim_end();

                // Skip if this is a function definition
                let is_definition = before_trimmed.ends_with("def ")
                    || before_trimmed.ends_with("fn ")
                    || before_trimmed.ends_with("function ")
                    || before_trimmed.ends_with("func ")
                    || before_trimmed.ends_with("async def ")
                    || before_trimmed.ends_with("pub fn ")
                    || before_trimmed.ends_with("async fn ");

                if !is_definition {
                    // Also verify it's a standalone identifier (not part of a larger word)
                    let is_word_boundary_before = abs_pos == 0
                        || !line.chars().nth(abs_pos - 1).is_some_and(|c| c.is_alphanumeric() || c == '_');
                    let is_word_boundary_after = !callee_name.chars().next_back().is_some_and(|_| {
                        line.chars().nth(after_name).is_some_and(|c| c.is_alphanumeric() || c == '_')
                    });

                    if is_word_boundary_before && is_word_boundary_after {
                        return Some(abs_pos);
                    }
                }
            }
        }

        search_pos = abs_pos + 1;
    }

    None
}

/// Async version of getSymbolsInFile
#[napi]
pub async fn get_symbols_in_file_async(
    path: String,
    file_path: String,
    filter: Option<SymbolFilter>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_symbols_in_file(path, file_path, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getSymbolSource
#[napi]
pub async fn get_symbol_source_async(
    path: String,
    symbol_name: String,
    file_path: Option<String>,
) -> Result<String> {
    tokio::task::spawn_blocking(move || get_symbol_source(path, symbol_name, file_path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getChangedSymbols
#[napi]
pub async fn get_changed_symbols_async(
    path: String,
    from_ref: String,
    to_ref: String,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_changed_symbols(path, from_ref, to_ref))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getTestsForFile
#[napi]
pub async fn get_tests_for_file_async(
    path: String,
    file_path: String,
) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || get_tests_for_file(path, file_path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallSites
#[napi]
pub async fn get_call_sites_async(
    path: String,
    symbol_name: String,
) -> Result<Vec<CallSite>> {
    tokio::task::spawn_blocking(move || get_call_sites(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

// ============================================================================
// New Features (v0.4.5)
// ============================================================================

/// Options for filtering changed symbols (Feature #6)
#[napi(object)]
pub struct ChangedSymbolsFilter {
    /// Filter by symbol kinds: "function", "method", "class", etc.
    /// If specified, only symbols of these kinds are returned.
    pub kinds: Option<Vec<String>>,
    /// Exclude specific kinds (e.g., exclude "import" to skip import statements)
    pub exclude_kinds: Option<Vec<String>>,
}

/// A symbol with change type information (Feature #7)
#[napi(object)]
pub struct ChangedSymbolInfo {
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
    /// Change type: "added", "modified", or "deleted"
    pub change_type: String,
}

/// Get symbols that were changed in a diff with filtering and change type (Features #6 & #7)
///
/// Enhanced version of getChangedSymbols that supports filtering by symbol kind
/// and returns change type (added, modified, deleted) for each symbol.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch (e.g., "main", "HEAD~1")
/// * `to_ref` - Ending commit/branch (e.g., "HEAD", "feature-branch")
/// * `filter` - Optional filter for symbol kinds
///
/// # Returns
/// Array of symbols with change type that were modified in the diff
///
/// # Example
/// ```javascript
/// const { getChangedSymbolsFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const changed = getChangedSymbolsFiltered('./my-repo', 'main', 'HEAD', {
///   kinds: ['function', 'method'],  // Only functions and methods
///   excludeKinds: ['import']         // Skip import statements
/// });
/// for (const s of changed) {
///   console.log(`${s.changeType}: ${s.kind} ${s.name} in ${s.file}`);
/// }
/// ```
#[napi]
pub fn get_changed_symbols_filtered(
    path: String,
    from_ref: String,
    to_ref: String,
    filter: Option<ChangedSymbolsFilter>,
) -> Result<Vec<ChangedSymbolInfo>> {
    let path_buf = PathBuf::from(&path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e)))?;

    // Load index
    let storage = IndexStorage::new(&path_buf);
    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Get changed files
    let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    let changed_files = git_repo.diff_files(from, to)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let mut changed_symbols: Vec<ChangedSymbolInfo> = Vec::new();
    let mut seen_ids: HashSet<u32> = HashSet::new();

    // Get filter options
    let kinds: Option<HashSet<String>> = filter.as_ref()
        .and_then(|f| f.kinds.as_ref())
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect());
    let exclude_kinds: Option<HashSet<String>> = filter.as_ref()
        .and_then(|f| f.exclude_kinds.as_ref())
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect());

    for file in changed_files {
        // Determine file-level change type
        let file_change_type = match file.status {
            EngineFileStatus::Added => "added",
            EngineFileStatus::Deleted => "deleted",
            _ => "modified",
        };

        // Get diff hunks to find changed lines
        let hunks = git_repo.diff_hunks(from, to, Some(&file.path)).unwrap_or_default();

        // Get file from index
        let file_entry = match index.get_file(&file.path) {
            Some(f) => f,
            None => continue, // File might be new or not indexed
        };

        // For added/deleted files, all symbols get that change type
        if file.status == EngineFileStatus::Added || file.status == EngineFileStatus::Deleted {
            for sym in index.get_file_symbols(file_entry.id) {
                let kind_name = sym.kind.name().to_lowercase();

                // Apply filters
                if let Some(ref allowed_kinds) = kinds {
                    if !allowed_kinds.contains(&kind_name) {
                        continue;
                    }
                }
                if let Some(ref excluded) = exclude_kinds {
                    if excluded.contains(&kind_name) {
                        continue;
                    }
                }

                if !seen_ids.contains(&sym.id.as_u32()) {
                    seen_ids.insert(sym.id.as_u32());
                    changed_symbols.push(ChangedSymbolInfo {
                        id: sym.id.as_u32(),
                        name: sym.name.clone(),
                        kind: kind_name,
                        file: file.path.clone(),
                        line: sym.span.start_line,
                        end_line: sym.span.end_line,
                        signature: sym.signature.clone(),
                        visibility: format!("{:?}", sym.visibility).to_lowercase(),
                        change_type: file_change_type.to_string(),
                    });
                }
            }
            continue;
        }

        // For modified files, find symbols that overlap with changed lines
        for hunk in hunks {
            if hunk.new_count == 0 {
                continue;
            }

            let start_line = hunk.new_start;
            let end_line = hunk.new_start + hunk.new_count;

            // Find symbols that overlap with this hunk
            for sym in index.get_file_symbols(file_entry.id) {
                // Check if symbol overlaps with changed lines
                let sym_overlaps = sym.span.start_line <= end_line && sym.span.end_line >= start_line;

                if sym_overlaps && !seen_ids.contains(&sym.id.as_u32()) {
                    let kind_name = sym.kind.name().to_lowercase();

                    // Apply filters
                    if let Some(ref allowed_kinds) = kinds {
                        if !allowed_kinds.contains(&kind_name) {
                            continue;
                        }
                    }
                    if let Some(ref excluded) = exclude_kinds {
                        if excluded.contains(&kind_name) {
                            continue;
                        }
                    }

                    seen_ids.insert(sym.id.as_u32());
                    changed_symbols.push(ChangedSymbolInfo {
                        id: sym.id.as_u32(),
                        name: sym.name.clone(),
                        kind: kind_name,
                        file: file.path.clone(),
                        line: sym.span.start_line,
                        end_line: sym.span.end_line,
                        signature: sym.signature.clone(),
                        visibility: format!("{:?}", sym.visibility).to_lowercase(),
                        change_type: "modified".to_string(),
                    });
                }
            }
        }
    }

    // Sort by file and line
    changed_symbols.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(changed_symbols)
}

/// A caller in the transitive call chain (Feature #8)
#[napi(object)]
pub struct TransitiveCallerInfo {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Depth from the target symbol (1 = direct caller, 2 = caller of caller, etc.)
    pub depth: u32,
    /// Call path from this caller to the target (e.g., ["main", "process", "validate", "target"])
    pub call_path: Vec<String>,
}

/// Options for transitive callers query
#[napi(object)]
pub struct TransitiveCallersOptions {
    /// Maximum depth to traverse (default: 3)
    pub max_depth: Option<u32>,
    /// Maximum number of results (default: 100)
    pub max_results: Option<u32>,
}

/// Get all functions that eventually call a symbol (Feature #8)
///
/// Traverses the call graph to find all direct and indirect callers
/// of the specified symbol, up to a maximum depth.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callers for
/// * `options` - Optional query options
///
/// # Returns
/// Array of callers with their depth and call path
///
/// # Example
/// ```javascript
/// const { getTransitiveCallers, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callers = getTransitiveCallers('./my-repo', 'validateInput', { maxDepth: 3 });
/// for (const c of callers) {
///   console.log(`Depth ${c.depth}: ${c.name} -> ${c.callPath.join(' -> ')}`);
/// }
/// ```
#[napi]
pub fn get_transitive_callers(
    path: String,
    symbol_name: String,
    options: Option<TransitiveCallersOptions>,
) -> Result<Vec<TransitiveCallerInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let max_depth = options.as_ref().and_then(|o| o.max_depth).unwrap_or(3);
    let max_results = options.as_ref().and_then(|o| o.max_results).unwrap_or(100) as usize;

    let mut results: Vec<TransitiveCallerInfo> = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();

    // Find all symbols with this name (there might be multiple)
    let target_symbols: Vec<_> = index.find_symbols(&symbol_name);
    if target_symbols.is_empty() {
        return Ok(vec![]);
    }

    // BFS to find all callers up to max_depth
    // Queue contains: (symbol_id, current_depth, call_path)
    let mut queue: std::collections::VecDeque<(u32, u32, Vec<String>)> = std::collections::VecDeque::new();

    // Initialize with target symbols
    for target in &target_symbols {
        visited.insert(target.id.as_u32());
        queue.push_back((target.id.as_u32(), 0, vec![target.name.clone()]));
    }

    while let Some((current_id, current_depth, call_path)) = queue.pop_front() {
        if results.len() >= max_results {
            break;
        }

        // Get direct callers
        for caller_id in graph.get_callers(current_id) {
            if visited.insert(caller_id) {
                if let Some(caller) = index.get_symbol(caller_id) {
                    let mut new_path = call_path.clone();
                    new_path.insert(0, caller.name.clone());

                    let file_path = index
                        .get_file_by_id(caller.file_id.as_u32())
                        .map(|f| f.path.clone())
                        .unwrap_or_else(|| "<unknown>".to_string());

                    results.push(TransitiveCallerInfo {
                        name: caller.name.clone(),
                        kind: caller.kind.name().to_string(),
                        file: file_path,
                        line: caller.span.start_line,
                        depth: current_depth + 1,
                        call_path: new_path.clone(),
                    });

                    // Continue traversal if not at max depth
                    if current_depth + 1 < max_depth {
                        queue.push_back((caller_id, current_depth + 1, new_path));
                    }
                }
            }
        }
    }

    // Sort by depth then by name
    results.sort_by(|a, b| (a.depth, &a.name).cmp(&(b.depth, &b.name)));
    Ok(results)
}

/// A call site with surrounding context (Feature #9)
#[napi(object)]
pub struct CallSiteWithContext {
    /// Name of the calling function/method
    pub caller: String,
    /// Name of the function/method being called
    pub callee: String,
    /// File containing the call
    pub file: String,
    /// Line number of the call (1-indexed)
    pub line: u32,
    /// Column number of the call (0-indexed, if available)
    pub column: Option<u32>,
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
    /// Code context around the call site (configurable number of lines)
    pub context: Option<String>,
    /// Start line of context
    pub context_start_line: Option<u32>,
    /// End line of context
    pub context_end_line: Option<u32>,
}

/// Options for call sites with context
#[napi(object)]
pub struct CallSitesContextOptions {
    /// Number of lines of context before the call (default: 3)
    pub lines_before: Option<u32>,
    /// Number of lines of context after the call (default: 3)
    pub lines_after: Option<u32>,
}

/// Get call sites with surrounding code context (Feature #9)
///
/// Enhanced version of getCallSites that includes the surrounding code
/// for each call site, useful for AI-powered code review.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find call sites for
/// * `options` - Optional context options
///
/// # Returns
/// Array of call sites with code context
///
/// # Example
/// ```javascript
/// const { getCallSitesWithContext, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const sites = getCallSitesWithContext('./my-repo', 'authenticate', {
///   linesBefore: 5,
///   linesAfter: 5
/// });
/// for (const site of sites) {
///   console.log(`Call in ${site.file}:${site.line}`);
///   console.log(site.context);
/// }
/// ```
#[napi]
pub fn get_call_sites_with_context(
    path: String,
    symbol_name: String,
    options: Option<CallSitesContextOptions>,
) -> Result<Vec<CallSiteWithContext>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage.load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage.load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let lines_before = options.as_ref().and_then(|o| o.lines_before).unwrap_or(3) as usize;
    let lines_after = options.as_ref().and_then(|o| o.lines_after).unwrap_or(3) as usize;

    let mut call_sites: Vec<CallSiteWithContext> = Vec::new();
    let mut seen_sites: HashSet<(String, u32, u32, u32)> = HashSet::new();
    let mut file_cache: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    // Find all symbols with this name
    for sym in index.find_symbols(&symbol_name) {
        let callee_id = sym.id.as_u32();

        // Get all callers of this symbol
        for caller_id in graph.get_callers(callee_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                let file_path = index
                    .get_file_by_id(caller_sym.file_id.as_u32())
                    .map(|f| f.path.clone())
                    .unwrap_or_else(|| "<unknown>".to_owned());

                // Try to find exact call site by scanning the caller's body
                let (call_line, call_col) = find_call_site_in_body(
                    &path_buf,
                    &file_path,
                    caller_sym.span.start_line,
                    caller_sym.span.end_line,
                    &symbol_name,
                    &mut file_cache,
                );

                // Deduplicate
                let site_key = (file_path.clone(), call_line, caller_id, callee_id);
                if !seen_sites.insert(site_key) {
                    continue;
                }

                // Get context
                let (context, context_start, context_end) = get_line_context(
                    &path_buf,
                    &file_path,
                    call_line,
                    lines_before,
                    lines_after,
                    &mut file_cache,
                );

                call_sites.push(CallSiteWithContext {
                    caller: caller_sym.name.clone(),
                    callee: sym.name.clone(),
                    file: file_path,
                    line: call_line,
                    column: call_col,
                    caller_id,
                    callee_id,
                    context,
                    context_start_line: context_start,
                    context_end_line: context_end,
                });
            }
        }
    }

    // Sort by file and line
    call_sites.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(call_sites)
}

/// Helper function to get code context around a line
fn get_line_context(
    repo_root: &PathBuf,
    file_path: &str,
    line: u32,
    lines_before: usize,
    lines_after: usize,
    file_cache: &mut std::collections::HashMap<String, Vec<String>>,
) -> (Option<String>, Option<u32>, Option<u32>) {
    // Load file content
    let lines = if let Some(cached) = file_cache.get(file_path) {
        cached.clone()
    } else {
        let full_path = repo_root.join(file_path);
        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                file_cache.insert(file_path.to_string(), lines.clone());
                lines
            }
            Err(_) => return (None, None, None),
        }
    };

    if lines.is_empty() {
        return (None, None, None);
    }

    let line_idx = (line as usize).saturating_sub(1);
    let start_idx = line_idx.saturating_sub(lines_before);
    let end_idx = (line_idx + lines_after + 1).min(lines.len());

    if start_idx >= lines.len() {
        return (None, None, None);
    }

    let context_lines: Vec<String> = lines[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let line_num = start_idx + i + 1;
            let marker = if line_num == line as usize { ">" } else { " " };
            format!("{}{:4} | {}", marker, line_num, l)
        })
        .collect();

    (
        Some(context_lines.join("\n")),
        Some((start_idx + 1) as u32),
        Some(end_idx as u32),
    )
}

/// Async version of getChangedSymbolsFiltered
#[napi]
pub async fn get_changed_symbols_filtered_async(
    path: String,
    from_ref: String,
    to_ref: String,
    filter: Option<ChangedSymbolsFilter>,
) -> Result<Vec<ChangedSymbolInfo>> {
    tokio::task::spawn_blocking(move || get_changed_symbols_filtered(path, from_ref, to_ref, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getTransitiveCallers
#[napi]
pub async fn get_transitive_callers_async(
    path: String,
    symbol_name: String,
    options: Option<TransitiveCallersOptions>,
) -> Result<Vec<TransitiveCallerInfo>> {
    tokio::task::spawn_blocking(move || get_transitive_callers(path, symbol_name, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallSitesWithContext
#[napi]
pub async fn get_call_sites_with_context_async(
    path: String,
    symbol_name: String,
    options: Option<CallSitesContextOptions>,
) -> Result<Vec<CallSiteWithContext>> {
    tokio::task::spawn_blocking(move || get_call_sites_with_context(path, symbol_name, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}
