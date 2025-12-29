//! Impact analysis operations
//!
//! This module provides functionality for analyzing the impact of code changes:
//! - Identifying affected symbols and dependencies
//! - Finding related test files
//! - Determining impact severity level

use crate::types::{AffectedSymbol, ImpactOptions, ImpactResult};
use crate::validation::{validate_file_path, validate_path};
use infiniloom_engine::index::{ChangeType, ContextDepth, ContextExpander, DiffChange, IndexStorage};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::HashSet;
use std::path::PathBuf;

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
pub fn analyze_impact(
    path: String,
    files: Vec<String>,
    options: Option<ImpactOptions>,
) -> Result<ImpactResult> {
    // Input validation
    validate_path(&path)?;
    if files.is_empty() {
        return Err(Error::new(Status::InvalidArg, "Files array cannot be empty".to_string()));
    }
    // Validate each file path
    for f in &files {
        validate_file_path(f)?;
    }

    let opts = options.unwrap_or(ImpactOptions {
        depth: None,
        include_tests: None,
        model: None,
        exclude: None,
        include: None,
    });

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    // Load index
    let index = storage.load_index().map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to load index (run buildIndex first): {}", e),
        )
    })?;
    let graph = storage.load_graph().map_err(|e| {
        Error::new(Status::GenericFailure, format!("Failed to load dependency graph: {}", e))
    })?;

    // Create context expander
    let depth = match opts.depth.unwrap_or(2) {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expander = ContextExpander::new(&index, &graph);

    // Convert files to diff changes, getting line ranges for all symbols in each file
    // Bug #4 fix: Ensure line_ranges are never empty so symbols are always found
    let changes: Vec<DiffChange> = files
        .iter()
        .map(|f| {
            // Get all symbol line ranges from this file
            let line_ranges = if let Some(file_entry) = index.get_file(f) {
                // Include all lines where symbols are defined
                let symbols = index.get_file_symbols(file_entry.id);
                if symbols.is_empty() {
                    // If no symbols, assume entire file is changed
                    vec![(1, file_entry.lines.max(1))]
                } else {
                    symbols
                        .iter()
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
        })
        .collect();

    // Expand context (returns directly, not Result)
    let token_budget = 50000; // Default budget
    let context = expander.expand(&changes, depth, token_budget);

    // Collect results
    let changed_files: Vec<String> = changes.iter().map(|c| c.file_path.clone()).collect();

    let dependent_files: Vec<String> = context
        .dependent_files
        .iter()
        .map(|f| f.path.clone())
        .collect();

    let mut test_files: Vec<String> = context
        .related_tests
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
                        if is_test_file(&importer_file.path)
                            && seen_tests.insert(importer_file.path.clone())
                        {
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
                            if file_lower.contains(pattern)
                                && seen_tests.insert(indexed_file.path.clone())
                            {
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
    let affected_symbols: Vec<AffectedSymbol> = context
        .changed_symbols
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
    }
    .to_string();

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
