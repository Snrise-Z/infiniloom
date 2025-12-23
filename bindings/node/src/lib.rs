#![deny(clippy::all)]

use infiniloom_engine::{
    git::{GitRepo as EngineGitRepo, FileStatus as EngineFileStatus},
    CompressionLevel, OutputFormat, OutputFormatter, RepoMapGenerator, Repository, SecurityScanner,
    SemanticCompressor, SemanticConfig, TokenizerModel, Tokenizer, tokenizer::TokenModel, Symbol,
    SymbolKind, Visibility, HeuristicCompressor,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;

mod scanner;
use scanner::{scan_repository as do_scan, ScanConfig};

/// Options for packing a repository
#[napi(object)]
pub struct PackOptions {
    /// Output format: "xml", "markdown", "json", or "yaml"
    pub format: Option<String>,
    /// Target model: "claude", "gpt-4o", "gpt-4", "gemini", or "llama"
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
/// const { pack } = require('@infiniloom/node');
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

    // Scan repository (with contents for packing)
    let mut repo = scan_repository_with_options(&path, model, true, skip_symbols)?;

    // Security check and redaction
    let scanner = SecurityScanner::new();
    for file in &mut repo.files {
        if let Some(ref content) = file.content {
            // Check for critical findings
            if !skip_security {
                let findings = scanner.scan(content, &file.relative_path);
                if findings.iter().any(|f| {
                    matches!(
                        f.severity,
                        infiniloom_engine::security::Severity::Critical
                    )
                }) {
                    return Err(Error::new(
                        Status::GenericFailure,
                        format!(
                            "Critical security issues found in {}. Use skip_security: true to override.",
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
/// * `model` - Optional target model (default: "claude")
///
/// # Returns
/// Repository statistics
///
/// # Example
/// ```javascript
/// const { scan } = require('@infiniloom/node');
///
/// const stats = scan('./my-repo', 'claude');
/// console.log(`Total files: ${stats.totalFiles}`);
/// console.log(`Total tokens: ${stats.totalTokens}`);
/// ```
#[napi]
pub fn scan(path: String, model: Option<String>) -> Result<ScanStats> {
    let tokenizer_model = parse_model(model.as_deref())?;
    let repo = scan_repository(&path, tokenizer_model, false)?;

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
        total_files: repo.metadata.total_files,
        total_lines: repo.metadata.total_lines as u32,
        total_tokens: repo.total_tokens(tokenizer_model),
        primary_language: repo
            .metadata
            .languages
            .first()
            .map(|l| l.language.clone()),
        languages: repo
            .metadata
            .languages
            .iter()
            .map(|l| LanguageStat {
                language: l.language.clone(),
                files: l.files,
                lines: l.lines as u32,
                percentage: l.percentage as f64,
            })
            .collect(),
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
/// const { countTokens } = require('@infiniloom/node');
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
        "gpt" | "gpt-4" | "gpt4" => TokenModel::Gpt4,
        "gpt-4o" | "gpt4o" => TokenModel::Gpt4o,
        "gpt-4o-mini" | "gpt4o-mini" => TokenModel::Gpt4oMini,
        "gpt-3.5-turbo" | "gpt35-turbo" | "gpt35turbo" => TokenModel::Gpt35Turbo,
        "o1" => TokenModel::O1,
        "o1-mini" => TokenModel::O1Mini,
        "o3" => TokenModel::O3,
        "o4-mini" => TokenModel::O4Mini,
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
                    "Invalid model: {}. Supported: claude, gpt, gpt-4, gpt-4o, gpt-4o-mini, gpt-3.5-turbo, o1, o1-mini, o3, o4-mini, gemini, llama, codellama, mistral, deepseek, qwen, cohere, grok",
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
        let repo = scan_repository(&path, tokenizer_model, true)?;

        Ok(Self {
            repo,
            model: tokenizer_model,
        })
    }

    /// Get repository statistics
    #[napi]
    pub fn get_stats(&self) -> ScanStats {
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
            total_files: self.repo.metadata.total_files,
            total_lines: self.repo.metadata.total_lines as u32,
            total_tokens: self.repo.total_tokens(self.model),
            primary_language: self
                .repo
                .metadata
                .languages
                .first()
                .map(|l| l.language.clone()),
            languages: self
                .repo
                .metadata
                .languages
                .iter()
                .map(|l| LanguageStat {
                    language: l.language.clone(),
                    files: l.files,
                    lines: l.lines as u32,
                    percentage: l.percentage as f64,
                })
                .collect(),
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
        });

        let format = parse_format(opts.format.as_deref())?;
        let compression = parse_compression(opts.compression.as_deref())?;
        let map_budget = opts.map_budget.unwrap_or(2000);
        let max_symbols = opts.max_symbols.unwrap_or(50);
        let redact_secrets = opts.redact_secrets.unwrap_or(true);

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

        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols as usize)
            .model(self.model)
            .build();

        let map = generator.generate(&repo);
        let formatter = OutputFormatter::by_format_with_model(format, self.model);

        Ok(formatter.format(&repo, &map))
    }

    /// Check for security issues
    #[napi]
    pub fn security_scan(&self) -> Result<Vec<String>> {
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
        other => Err(Error::new(
            Status::InvalidArg,
            format!("Unknown format: {}. Use 'xml', 'markdown', 'json', or 'yaml'", other),
        )),
    }
}

fn parse_model(model: Option<&str>) -> Result<TokenizerModel> {
    match model.unwrap_or("claude") {
        "claude" => Ok(TokenizerModel::Claude),
        "gpt-4o" | "gpt4o" => Ok(TokenizerModel::Gpt4o),
        "gpt-4" | "gpt4" => Ok(TokenizerModel::Gpt4),
        "gemini" => Ok(TokenizerModel::Gemini),
        "llama" => Ok(TokenizerModel::Llama),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown model: {}. Use 'claude', 'gpt-4o', 'gpt-4', 'gemini', or 'llama'",
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
/// const { semanticCompress } = require('@infiniloom/node');
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
/// const { isGitRepo } = require('@infiniloom/node');
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

/// Git repository wrapper for Node.js
///
/// Provides access to git operations like status, diff, log, and blame.
///
/// # Example
/// ```javascript
/// const { GitRepo } = require('@infiniloom/node');
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
