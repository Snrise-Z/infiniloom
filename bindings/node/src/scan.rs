//! Repository scanning operations
//!
//! This module provides functions for scanning repositories and counting tokens.

use crate::types::{LanguageStat, ScanOptions, ScanStats};
use crate::utils::{napi_parse_model, read_contents_parallel, scan_repository};
use crate::validation::{validate_path, validate_path_option};
use infiniloom_bindings_common::matches_any_pattern;
use infiniloom_engine::{
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    SecurityScanner, Tokenizer,
};
use napi::Result;
use napi_derive::napi;

/// Scan a repository and return statistics
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
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
pub fn scan(path: Option<String>, model: Option<String>) -> Result<ScanStats> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;

    // Call scan_with_options with default options for backwards compatibility
    scan_with_options(
        path,
        Some(ScanOptions {
            model,
            include: None,
            exclude: None,
            include_tests: None,
            apply_default_ignores: Some(true),
        }),
    )
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
    // Input validation
    validate_path(&path)?;

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

    // STEP 1: Fast file list without reading content (filter-first optimization)
    let mut repo = scan_repository(&path, false)?;

    // STEP 2: Apply all filters BEFORE reading content
    // Apply default ignores (Bug #2 fix)
    if apply_default_ignores {
        repo.files
            .retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));
    }

    // Apply test ignores unless include_tests is true
    if !include_tests {
        repo.files
            .retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    // Apply custom include patterns
    if let Some(ref include_patterns) = opts.include {
        let patterns: Vec<&str> = include_patterns.iter().map(|s| s.as_str()).collect();
        repo.files
            .retain(|f| matches_any_pattern(&f.relative_path, &patterns));
    }

    // Apply custom exclude patterns
    if let Some(ref exclude_patterns) = opts.exclude {
        let patterns: Vec<&str> = exclude_patterns.iter().map(|s| s.as_str()).collect();
        repo.files
            .retain(|f| !matches_any_pattern(&f.relative_path, &patterns));
    }

    // STEP 3: Read content only for filtered files (much faster!)
    read_contents_parallel(&mut repo);

    // Recalculate metadata after filtering
    let total_files = repo.files.len() as u32;
    let total_lines: u64 = repo
        .files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or(0)
        })
        .sum();

    // Calculate language stats with actual line counts (Bug #9 fix)
    let mut language_stats: std::collections::HashMap<String, (u32, u64)> =
        std::collections::HashMap::new();
    for file in &repo.files {
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

    // Sort languages by percentage (Bug #12 fix)
    let mut languages: Vec<LanguageStat> = language_stats
        .into_iter()
        .map(|(lang, (files, lines))| {
            let percentage = if total_files > 0 {
                (files as f64 / total_files as f64) * 100.0
            } else {
                0.0
            };
            LanguageStat { language: lang, files, lines: lines as u32, percentage }
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
/// * `text` - Text to tokenize (null/undefined returns 0)
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
pub fn count_tokens(text: Option<String>, model: Option<String>) -> Result<u32> {
    // Handle null/undefined/empty text gracefully (return 0 tokens)
    let text = match text {
        None => return Ok(0),
        Some(t) if t.is_empty() => return Ok(0),
        Some(t) => t,
    };

    let token_model = napi_parse_model(model.as_deref())?;
    let tokenizer = Tokenizer::new();
    Ok(tokenizer.count(&text, token_model))
}
