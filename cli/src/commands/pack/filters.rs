//! File filtering logic for pack command
//!
//! This module provides thin wrappers around centralized filtering functions
//! for backward compatibility with pack command's specialized API.
//!
//! New code should use `infiniloom_engine::filtering` directly for better
//! performance and consistency across commands.

use infiniloom_engine::filtering;

/// Check if a glob pattern matches a file path
///
/// **Deprecated**: Use `infiniloom_engine::filtering::matches_include_pattern()` instead.
///
/// This function is retained for backward compatibility with existing pack command code.
/// Matches against both the full relative path and just the filename.
///
/// # Arguments
///
/// * `pattern` - Compiled glob pattern
/// * `relative_path` - Relative file path to match against
///
/// # Returns
///
/// Returns `true` if the pattern matches either the full path or just the filename.
#[deprecated(
    since = "0.4.12",
    note = "Use infiniloom_engine::filtering::matches_include_pattern instead"
)]
pub fn pattern_matches_file(pattern: &glob::Pattern, relative_path: &str) -> bool {
    // Delegate to centralized filtering
    pattern.matches(relative_path)
        || std::path::Path::new(relative_path)
            .file_name()
            .and_then(|f| f.to_str())
            .map(|f| pattern.matches(f))
            .unwrap_or(false)
}

/// Apply default ignores to a repository
///
/// Filters out files based on default ignore patterns, test patterns, and doc patterns.
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
/// * `use_default_ignores` - Whether to apply default ignore patterns
/// * `include_tests` - Whether to include test files
/// * `include_docs` - Whether to include documentation files
pub fn apply_default_ignores(
    repo: &mut infiniloom_engine::Repository,
    use_default_ignores: bool,
    include_tests: bool,
    include_docs: bool,
) {
    if !use_default_ignores {
        return;
    }

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

/// Filter repository files to stdin paths
///
/// Only keeps files that match provided stdin paths.
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
/// * `stdin_paths` - Optional list of paths from stdin
pub fn filter_stdin_paths(
    repo: &mut infiniloom_engine::Repository,
    stdin_paths: &Option<Vec<String>>,
) {
    if let Some(ref paths) = stdin_paths {
        repo.files.retain(|f| {
            paths
                .iter()
                .any(|p| f.relative_path == *p || f.relative_path.ends_with(p))
        });
    }
}

/// Apply include patterns to repository
///
/// **Thin wrapper**: Now delegates to centralized `infiniloom_engine::filtering` module.
///
/// Only keeps files that match at least one include pattern.
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
/// * `include_patterns` - List of include glob patterns
///
/// # Returns
///
/// Returns compiled glob patterns for reuse.
pub fn apply_include_patterns(
    repo: &mut infiniloom_engine::Repository,
    include_patterns: Vec<String>,
) -> Vec<glob::Pattern> {
    // Delegate to centralized filtering
    filtering::apply_include_patterns(&mut repo.files, &include_patterns, |f| &f.relative_path);

    // Compile and return patterns for backward compatibility
    filtering::compile_patterns(&include_patterns)
}

/// Apply exclude patterns to repository
///
/// **Thin wrapper**: Now delegates to centralized `infiniloom_engine::filtering` module.
///
/// Removes files that match any exclude pattern.
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
/// * `exclude_patterns` - List of exclude glob patterns
///
/// # Returns
///
/// Returns compiled glob patterns for reuse.
pub fn apply_exclude_patterns(
    repo: &mut infiniloom_engine::Repository,
    exclude_patterns: Vec<String>,
) -> Vec<glob::Pattern> {
    // Delegate to centralized filtering
    filtering::apply_exclude_patterns(&mut repo.files, &exclude_patterns, |f| &f.relative_path);

    // Compile and return patterns for backward compatibility
    filtering::compile_patterns(&exclude_patterns)
}
