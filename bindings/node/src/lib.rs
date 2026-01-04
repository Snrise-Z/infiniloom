#![deny(clippy::all)]

//! Infiniloom Node.js bindings
//!
//! This library provides high-performance Node.js bindings for the Infiniloom
//! repository context generator. It enables LLM-optimized codebase indexing,
//! symbol extraction, and diff analysis directly from Node.js/TypeScript.

// Module declarations
mod types;
mod validation;
mod utils;
mod security;
mod scan;
mod chunk;
mod pack;
mod git;
mod index;
mod call_graph;
mod symbols;
mod diff;
mod impact;
mod embed;
mod analysis;

// Re-export all public types
pub use types::*;

// Re-export public functions from modules
pub use call_graph::{
    find_symbol, find_symbol_async, find_symbol_filtered, find_symbol_filtered_async,
    get_call_graph, get_call_graph_async, get_callees, get_callees_async, get_callees_filtered,
    get_callees_filtered_async, get_callers, get_callers_async, get_callers_filtered,
    get_callers_filtered_async, get_references, get_references_async, get_references_filtered,
    get_references_filtered_async,
};
pub use chunk::chunk;
pub use diff::get_diff_context;
pub use git::{is_git_repo, GitRepo};
pub use impact::analyze_impact;
pub use index::{build_index, index_status};
pub use pack::pack;
pub use scan::scan;
pub use security::scan_security;
pub use symbols::{
    get_call_sites, get_call_sites_async, get_call_sites_with_context,
    get_call_sites_with_context_async, get_changed_symbols, get_changed_symbols_async,
    get_changed_symbols_filtered, get_changed_symbols_filtered_async, get_symbol_source,
    get_symbol_source_async, get_symbols_in_file, get_symbols_in_file_async, get_tests_for_file,
    get_tests_for_file_async, get_transitive_callers, get_transitive_callers_async,
};
pub use embed::{
    delete_embed_manifest, delete_embed_manifest_async, embed, embed_async, load_embed_manifest,
    load_embed_manifest_async,
};
pub use analysis::{
    extract_documentation, extract_documentation_async, detect_dead_code, detect_dead_code_async,
    detect_breaking_changes, detect_breaking_changes_async,
};

use infiniloom_bindings_common::{
    parse_compression, parse_format, parse_model, scan_repository as bindings_scan_repository,
    ScanConfig,
};
use infiniloom_engine::{
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    repomap::RepoMapGenerator,
    types::Repository,
    HeuristicCompressor, OutputFormatter, SecurityScanner, TokenizerModel,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::path::PathBuf;

// ============================================================================
// Package Version
// ============================================================================

/// Get the package version
///
/// # Returns
/// The version string of the infiniloom-node package
///
/// # Example
/// ```javascript
/// const { version } = require('infiniloom-node');
///
/// console.log(`infiniloom-node v${version()}`);
/// ```
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ============================================================================
// Async Wrapper Functions
// ============================================================================
// These functions wrap the synchronous versions in tokio::task::spawn_blocking
// to provide async APIs for Node.js applications that benefit from non-blocking I/O.

/// Async version of pack
///
/// Pack a repository into optimized LLM context asynchronously.
/// This is useful for Node.js applications that want to avoid blocking
/// the event loop during repository scanning.
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
/// const { packAsync } = require('infiniloom-node');
///
/// const context = await packAsync('./my-repo', {
///   format: 'xml',
///   model: 'claude',
///   compression: 'balanced'
/// });
/// ```
#[napi]
pub async fn pack_async(path: Option<String>, options: Option<types::PackOptions>) -> Result<String> {
    // Run synchronous pack in a blocking task
    tokio::task::spawn_blocking(move || pack(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of scan
///
/// Scan a repository and return statistics asynchronously.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `model` - Optional tokenizer model
///
/// # Returns
/// Repository statistics
///
/// # Example
/// ```javascript
/// const { scanAsync } = require('infiniloom-node');
///
/// const stats = await scanAsync('./my-repo', 'claude');
/// console.log(`Total tokens: ${stats.totalTokens}`);
/// ```
#[napi]
pub async fn scan_async(path: Option<String>, model: Option<String>) -> Result<types::ScanStats> {
    tokio::task::spawn_blocking(move || scan(path, model))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of buildIndex
///
/// Build a symbol index for fast diff context analysis asynchronously.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional index build options
///
/// # Returns
/// Index status
///
/// # Example
/// ```javascript
/// const { buildIndexAsync } = require('infiniloom-node');
///
/// const status = await buildIndexAsync('./my-repo', { force: false });
/// console.log(`Indexed ${status.totalFiles} files`);
/// ```
#[napi]
pub async fn build_index_async(
    path: Option<String>,
    options: Option<types::IndexOptions>,
) -> Result<types::IndexStatus> {
    tokio::task::spawn_blocking(move || build_index(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of chunk
///
/// Split a repository into semantic chunks asynchronously.
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
/// const { chunkAsync } = require('infiniloom-node');
///
/// const chunks = await chunkAsync('./my-repo', {
///   strategy: 'module',
///   maxTokens: 4000,
///   overlap: 500
/// });
/// ```
#[napi]
pub async fn chunk_async(
    path: Option<String>,
    options: Option<types::ChunkOptions>,
) -> Result<Vec<types::RepoChunk>> {
    tokio::task::spawn_blocking(move || chunk(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of analyzeImpact
///
/// Analyze the impact of file changes asynchronously.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `files` - List of changed file paths
/// * `options` - Optional impact analysis options
///
/// # Returns
/// Impact analysis result
///
/// # Example
/// ```javascript
/// const { analyzeImpactAsync } = require('infiniloom-node');
///
/// const impact = await analyzeImpactAsync('./my-repo', ['src/auth.rs'], {
///   depth: 2
/// });
/// ```
#[napi]
pub async fn analyze_impact_async(
    path: String,
    files: Vec<String>,
    options: Option<types::ImpactOptions>,
) -> Result<types::ImpactResult> {
    tokio::task::spawn_blocking(move || analyze_impact(path, files, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getDiffContext
///
/// Get context-aware diff with surrounding symbols and dependencies asynchronously.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch
/// * `to_ref` - Ending commit/branch
/// * `options` - Optional diff context options
///
/// # Returns
/// Diff context result
///
/// # Example
/// ```javascript
/// const { getDiffContextAsync } = require('infiniloom-node');
///
/// const context = await getDiffContextAsync('./my-repo', 'HEAD~1', 'HEAD', {
///   depth: 2,
///   budget: 50000
/// });
/// ```
#[napi]
pub async fn get_diff_context_async(
    path: String,
    from_ref: String,
    to_ref: String,
    options: Option<types::DiffContextOptions>,
) -> Result<types::DiffContextResult> {
    tokio::task::spawn_blocking(move || get_diff_context(path, from_ref, to_ref, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

// ============================================================================
// Additional Type Definitions
// ============================================================================

/// Options for generateMap
#[napi(object)]
pub struct GenerateMapOptions {
    /// Token budget for the map (default: 2000)
    pub budget: Option<u32>,
    /// Maximum number of symbols to include (default: 50)
    pub max_symbols: Option<u32>,
}

/// Options for semanticCompress
#[napi(object)]
pub struct SemanticCompressOptions {
    /// Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
    /// Note: Only affects output when built with "embeddings" feature.
    pub similarity_threshold: Option<f64>,
    /// Target size as ratio of original (0.0-1.0, default: 0.5)
    /// Lower values = more aggressive compression
    pub budget_ratio: Option<f64>,
    /// Minimum chunk size in characters (default: 100)
    pub min_chunk_size: Option<u32>,
    /// Maximum chunk size in characters (default: 2000)
    pub max_chunk_size: Option<u32>,
}

// ============================================================================
// Infiniloom Class (OOP API)
// ============================================================================

/// Object-oriented API for Infiniloom
///
/// Provides a stateful wrapper around the repository scanning and formatting functionality.
/// Alternative to the functional API for users who prefer an object-oriented style.
///
/// # Example
/// ```javascript
/// const { Infiniloom } = require('infiniloom-node');
///
/// const loom = new Infiniloom('./my-repo', 'claude');
/// const stats = loom.getStats();
/// const map = loom.generateMap({ budget: 3000, maxSymbols: 100 });
/// const output = loom.pack({ format: 'xml', compression: 'balanced' });
/// ```
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
    ///
    /// # Example
    /// ```javascript
    /// const loom = new Infiniloom('./my-repo', 'gpt4o');
    /// ```
    #[napi(constructor)]
    pub fn new(path: String, model: Option<String>) -> Result<Self> {
        // Validate path is not empty
        crate::validation::validate_path(&path)?;

        let tokenizer_model = parse_model(model.as_deref())
            .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;

        // Scan repository with default configuration
        let config = ScanConfig {
            read_contents: true,
            skip_symbols: false,
            ..Default::default()
        };
        let path_buf = PathBuf::from(&path);
        let mut repo = bindings_scan_repository(&path_buf, config)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        // Apply default filters
        repo.files
            .retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));
        repo.files
            .retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));

        // Prepare repository (rank and sort files)
        infiniloom_bindings_common::prepare_repository(&mut repo);

        Ok(Self { repo, model: tokenizer_model })
    }

    /// Get repository statistics
    ///
    /// Returns the same statistics as the `scan()` function.
    ///
    /// # Example
    /// ```javascript
    /// const loom = new Infiniloom('./my-repo');
    /// const stats = loom.getStats();
    /// console.log(`${stats.totalFiles} files, ${stats.totalTokens} tokens`);
    /// ```
    #[napi]
    pub fn get_stats(&self) -> types::ScanStats {
        // Calculate actual file and line counts
        let total_files = self.repo.files.len() as u32;
        let total_lines: u64 = self
            .repo
            .files
            .iter()
            .map(|f| f.content.as_ref().map(|c| c.lines().count() as u64).unwrap_or(0))
            .sum();

        // Calculate language stats
        let mut language_stats: std::collections::HashMap<String, (u32, u64)> =
            std::collections::HashMap::new();
        for file in &self.repo.files {
            if let Some(ref lang) = file.language {
                let lines = file
                    .content
                    .as_ref()
                    .map(|c| c.lines().count() as u64)
                    .unwrap_or(0);
                let entry = language_stats.entry(lang.clone()).or_insert((0, 0));
                entry.0 += 1; // files
                entry.1 += lines; // lines
            }
        }

        // Sort languages by percentage
        let mut languages: Vec<types::LanguageStat> = language_stats
            .into_iter()
            .map(|(lang, (files, lines))| {
                let percentage = if total_files > 0 {
                    (files as f64 / total_files as f64) * 100.0
                } else {
                    0.0
                };
                types::LanguageStat {
                    language: lang,
                    files,
                    lines: lines as u32,
                    percentage,
                }
            })
            .collect();
        languages.sort_by(|a, b| {
            b.percentage
                .partial_cmp(&a.percentage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Security scan
        let scanner = SecurityScanner::new();
        let mut total_findings = 0;
        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                total_findings += findings.len();
            }
        }

        let tokenizer = infiniloom_engine::tokenizer::Tokenizer::new();
        let total_tokens = self
            .repo
            .files
            .iter()
            .map(|f| {
                f.content
                    .as_ref()
                    .map(|c| tokenizer.count(c, self.model.into()))
                    .unwrap_or(0)
            })
            .sum();

        types::ScanStats {
            name: self.repo.name.clone(),
            total_files,
            total_lines: total_lines as u32,
            total_tokens,
            primary_language: languages.first().map(|l| l.language.clone()),
            languages,
            security_findings: total_findings as u32,
        }
    }

    /// Generate a repository map with ranked symbols
    ///
    /// # Arguments
    /// * `options` - Options object with budget (default: 2000) and maxSymbols (default: 50)
    ///
    /// # Example
    /// ```javascript
    /// const loom = new Infiniloom('./my-repo');
    /// const map = loom.generateMap({ budget: 3000, maxSymbols: 100 });
    /// ```
    #[napi]
    pub fn generate_map(&self, options: Option<GenerateMapOptions>) -> Result<String> {
        let opts = options.unwrap_or(GenerateMapOptions {
            budget: None,
            max_symbols: None,
        });
        let token_budget = opts.budget.unwrap_or(2000);
        let max_syms = opts.max_symbols.unwrap_or(50);

        let generator = RepoMapGenerator::builder()
            .token_budget(token_budget)
            .max_symbols(max_syms as usize)
            .model(self.model)
            .build();

        let map = generator.generate(&self.repo);

        serde_json::to_string_pretty(&map).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Pack repository with specific options
    ///
    /// Formats the repository using the same logic as the `pack()` function,
    /// but operates on the pre-scanned repository stored in this instance.
    ///
    /// # Arguments
    /// * `options` - Pack options (format, compression, etc.)
    ///
    /// # Example
    /// ```javascript
    /// const loom = new Infiniloom('./my-repo');
    /// const output = loom.pack({ format: 'xml', compression: 'balanced' });
    /// ```
    #[napi]
    pub fn pack(&self, options: Option<types::PackOptions>) -> Result<String> {
        // Default values for options
        let opts = options.unwrap_or(types::PackOptions {
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
        let format = parse_format(opts.format.as_deref())
            .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;
        let compression = parse_compression(opts.compression.as_deref())
            .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;
        let map_budget = opts.map_budget.unwrap_or(2000);
        let max_symbols = opts.max_symbols.unwrap_or(50);

        // Clone the repository for processing (avoid mutating internal state)
        let mut repo_copy = self.repo.clone();

        // Apply compression
        infiniloom_bindings_common::apply_compression(&mut repo_copy, compression);

        // Apply token budget if specified
        if let Some(budget) = opts.token_budget {
            if budget > 0 {
                infiniloom_bindings_common::apply_token_budget(
                    &mut repo_copy,
                    budget as u32,
                    self.model.into(),
                );
            }
        }

        // Redact secrets if requested
        if opts.redact_secrets.unwrap_or(false) {
            infiniloom_bindings_common::redact_secrets(&mut repo_copy);
        }

        // Generate repository map
        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols as usize)
            .model(self.model)
            .build();
        let map = generator.generate(&repo_copy);

        // Format output
        let formatter = OutputFormatter::by_format_with_model(format, self.model);
        let output = formatter.format(&repo_copy, &map);

        Ok(output)
    }

    /// Scan repository for security issues
    ///
    /// Scans the pre-loaded repository files for secrets and sensitive information.
    ///
    /// # Returns
    /// Array of security findings with file, line, severity, kind, and pattern
    ///
    /// # Example
    /// ```javascript
    /// const loom = new Infiniloom('./my-repo');
    /// const findings = loom.securityScan();
    /// for (const finding of findings) {
    ///   console.log(`${finding.severity}: ${finding.kind} in ${finding.file}:${finding.line}`);
    /// }
    /// ```
    #[napi]
    pub fn security_scan(&self) -> Result<Vec<types::SecurityFinding>> {
        let scanner = SecurityScanner::new();
        let mut findings = Vec::new();

        for file in &self.repo.files {
            if let Some(content) = &file.content {
                let file_findings = scanner.scan(content, &file.relative_path);
                for finding in file_findings {
                    findings.push(types::SecurityFinding {
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
}

// ============================================================================
// Semantic Compression (Standalone Function)
// ============================================================================

/// Compress text using semantic compression
///
/// Uses heuristic-based compression to reduce text size while preserving
/// semantic meaning. This is a simplified version that doesn't require embeddings.
///
/// # Arguments
/// * `text` - Text to compress
/// * `options` - Optional compression options
///
/// # Returns
/// Compressed text
///
/// # Example
/// ```javascript
/// const { semanticCompress } = require('infiniloom-node');
///
/// const compressed = semanticCompress(longText, {
///   budgetRatio: 0.5,
///   minChunkSize: 100,
///   maxChunkSize: 2000
/// });
/// ```
#[napi]
pub fn semantic_compress(
    text: Option<String>,
    options: Option<SemanticCompressOptions>,
) -> Result<String> {
    // Input validation
    let text = match text {
        None => {
            return Err(Error::new(
                Status::InvalidArg,
                "Text cannot be null or undefined".to_string(),
            ))
        },
        Some(t) if t.is_empty() => {
            return Err(Error::new(
                Status::InvalidArg,
                "Text cannot be empty".to_string(),
            ))
        },
        Some(t) => t,
    };

    let _opts = options.unwrap_or(SemanticCompressOptions {
        similarity_threshold: None,
        budget_ratio: None,
        min_chunk_size: None,
        max_chunk_size: None,
    });

    // Use heuristic compression
    let compressor = HeuristicCompressor::new();
    compressor
        .compress(&text)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Compression failed: {}", e)))
}
