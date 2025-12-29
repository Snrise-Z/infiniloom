//! Repository packing operations
//!
//! This module provides functionality for packing repositories into optimized
//! LLM context formats. It handles scanning, filtering, security checks, compression,
//! and formatting.

use crate::types::PackOptions;
use crate::utils::{napi_parse_compression, napi_parse_format, napi_parse_model, parse_security_threshold, read_contents_and_symbols_parallel, scan_repository_with_options};
use crate::validation::validate_path_option;
use infiniloom_bindings_common::{
    apply_compression, apply_token_budget, matches_any_pattern, prepare_repository,
    severity_at_or_above,
};
use infiniloom_engine::{
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    git::GitRepo as EngineGitRepo,
    index::{ChangeType, ContextDepth, ContextExpander, DiffChange, IndexStorage},
    OutputFormatter, RepoMapGenerator, SecurityScanner,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::HashSet;
use std::path::PathBuf;

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
pub fn pack(path: Option<String>, options: Option<PackOptions>) -> Result<String> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;

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
    let token_budget = crate::validation::validate_token_budget(opts.token_budget)?;
    let changed_only = opts.changed_only.unwrap_or(false);
    let include_related = opts.include_related.unwrap_or(false);
    let related_depth = opts.related_depth.unwrap_or(1).clamp(1, 3);

    // STEP 1: Fast file list without reading content (filter-first optimization)
    let mut repo = scan_repository_with_options(&path, false, true)?;

    // STEP 2: Apply all filters BEFORE reading content
    // Apply default ignores to filter out build outputs, dependencies, etc.
    repo.files
        .retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));

    // Apply test ignores unless include_tests is true
    if !include_tests {
        repo.files
            .retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    // Apply custom include patterns (if specified, only keep matching files)
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

    // STEP 3: Read content and optionally extract symbols for filtered files
    read_contents_and_symbols_parallel(&mut repo, !skip_symbols);

    // Filter to changed files only (if enabled)
    if changed_only {
        let path_buf = PathBuf::from(&path);
        if EngineGitRepo::is_git_repo(&path_buf) {
            let git_repo = EngineGitRepo::open(&path_buf).map_err(|e| {
                Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e))
            })?;

            // Get changed file paths
            let changed_paths: HashSet<String> = if opts.staged_only.unwrap_or(false) {
                // Only staged changes - status() returns all changes
                // For staged-only, we'd need to parse status output more carefully
                // For now, we include all changed files
                git_repo
                    .status()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else if let (Some(ref base), Some(ref head)) = (&opts.base_sha, &opts.head_sha) {
                // Diff between two refs
                git_repo
                    .diff_files(base, head)
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else if let Some(ref base) = opts.base_sha {
                // Diff from base to HEAD
                git_repo
                    .diff_files(base, "HEAD")
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            } else {
                // Uncommitted changes (default)
                git_repo
                    .status()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .into_iter()
                    .map(|f| f.path)
                    .collect()
            };

            // Filter repo files to only include changed files
            repo.files
                .retain(|f| changed_paths.contains(&f.relative_path));
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
            let changed_paths: Vec<String> =
                repo.files.iter().map(|f| f.relative_path.clone()).collect();

            // Convert to DiffChange for expander
            let changes: Vec<DiffChange> = changed_paths
                .iter()
                .map(|p| DiffChange {
                    file_path: p.clone(),
                    old_path: None,
                    line_ranges: vec![],
                    change_type: ChangeType::Modified,
                    diff_content: None,
                })
                .collect();

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
                let full_repo = scan_repository_with_options(&path, true, skip_symbols)?;
                for file in full_repo.files {
                    if related_paths.contains(&file.relative_path) {
                        // Check if we already have this file
                        if !repo
                            .files
                            .iter()
                            .any(|f| f.relative_path == file.relative_path)
                        {
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
                if findings
                    .iter()
                    .any(|f| severity_at_or_above(&f.severity, &security_threshold))
                {
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
