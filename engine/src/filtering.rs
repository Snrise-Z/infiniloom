//! Centralized file filtering logic
//!
//! This module provides unified pattern matching and filtering functionality
//! used across all commands (pack, diff, scan, map, chunk, index).
//!
//! # Key Features
//!
//! - **Glob pattern support**: `*.rs`, `src/**/*.ts`, `**/*.test.js`
//! - **Substring matching**: `node_modules`, `dist`, `target`
//! - **Path component matching**: Match against directory names
//! - **Generic API**: Works with any collection type
//!
//! # Usage Example
//!
//! ```no_run
//! use infiniloom_engine::filtering::{apply_exclude_patterns, apply_include_patterns};
//! use infiniloom_engine::types::RepoFile;
//!
//! let mut files: Vec<RepoFile> = vec![/* ... */];
//! let exclude = vec!["node_modules".to_string(), "*.min.js".to_string()];
//! let include = vec!["src/**/*.rs".to_string()];
//!
//! // Apply filters
//! apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);
//! apply_include_patterns(&mut files, &include, |f| &f.relative_path);
//! ```

use glob::Pattern;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Compiled pattern cache to avoid recompilation
static PATTERN_CACHE: OnceLock<std::sync::Mutex<HashMap<String, Option<Pattern>>>> =
    OnceLock::new();

/// Get or create the pattern cache
fn get_pattern_cache() -> &'static std::sync::Mutex<HashMap<String, Option<Pattern>>> {
    PATTERN_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Compile a glob pattern with caching
///
/// Returns `None` if the pattern is invalid.
fn compile_pattern(pattern: &str) -> Option<Pattern> {
    let cache = get_pattern_cache();
    let mut cache_guard = cache.lock().unwrap();

    if let Some(cached) = cache_guard.get(pattern) {
        return cached.clone();
    }

    let compiled = Pattern::new(pattern).ok();
    cache_guard.insert(pattern.to_string(), compiled.clone());
    compiled
}

/// Check if a path matches an exclude pattern
///
/// Exclude patterns support:
/// - Glob patterns: `*.min.js`, `src/**/*.test.ts`
/// - Path component matches: `tests`, `vendor`, `node_modules` (matches directory names)
/// - Prefix matches: `target` matches `target/debug/file.rs`
///
/// Note: Pattern "target" will match "target/file.rs" and "src/target/file.rs"
/// but NOT "src/target.rs" (where target is part of a filename).
///
/// # Arguments
///
/// * `path` - File path to check
/// * `pattern` - Exclude pattern
///
/// # Returns
///
/// Returns `true` if the path should be excluded.
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::filtering::matches_exclude_pattern;
///
/// assert!(matches_exclude_pattern("src/tests/foo.rs", "tests"));
/// assert!(matches_exclude_pattern("node_modules/lib.js", "node_modules"));
/// assert!(matches_exclude_pattern("dist/bundle.min.js", "*.min.js"));
/// ```
pub fn matches_exclude_pattern(path: &str, pattern: &str) -> bool {
    // Empty pattern should not match anything
    if pattern.is_empty() {
        return false;
    }

    // Try as glob pattern first if contains wildcard
    if pattern.contains('*') {
        if let Some(glob) = compile_pattern(pattern) {
            if glob.matches(path) {
                return true;
            }
        }
    }

    // Path component match (e.g., "tests" matches "src/tests/foo.rs")
    // This handles directory names like "node_modules", "target", "dist"
    if path.split('/').any(|part| part == pattern) {
        return true;
    }

    // Prefix match (e.g., "src/" matches "src/foo.rs")
    if path.starts_with(pattern) {
        return true;
    }

    false
}

/// Check if a path matches an include pattern
///
/// Include patterns support:
/// - Glob patterns: `*.rs`, `src/**/*.ts`, `**/*.test.js`
/// - Substring matches: `src`, `lib`
/// - Suffix matches: `.rs`, `.ts`
///
/// # Arguments
///
/// * `path` - File path to check
/// * `pattern` - Include pattern
///
/// # Returns
///
/// Returns `true` if the path should be included.
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::filtering::matches_include_pattern;
///
/// assert!(matches_include_pattern("src/main.rs", "*.rs"));
/// assert!(matches_include_pattern("src/lib.rs", "src"));
/// assert!(matches_include_pattern("foo.test.ts", "*.test.ts"));
/// ```
pub fn matches_include_pattern(path: &str, pattern: &str) -> bool {
    // Empty pattern should not match anything
    if pattern.is_empty() {
        return false;
    }

    // Try as glob pattern first if contains wildcard
    if pattern.contains('*') {
        if let Some(glob) = compile_pattern(pattern) {
            return glob.matches(path);
        }
    }

    // Substring match or suffix match
    path.contains(pattern) || path.ends_with(pattern)
}

/// Apply exclude patterns to a collection
///
/// Removes items whose paths match any exclude pattern.
/// Uses a generic `get_path` function to extract the path from each item.
///
/// # Arguments
///
/// * `items` - Mutable reference to collection to filter
/// * `patterns` - List of exclude patterns
/// * `get_path` - Function to extract path from an item
///
/// # Type Parameters
///
/// * `T` - Type of items in the collection
/// * `F` - Type of the path extraction function
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::filtering::apply_exclude_patterns;
/// use infiniloom_engine::types::RepoFile;
///
/// let mut files: Vec<RepoFile> = vec![/* ... */];
/// let exclude = vec!["node_modules".to_string(), "*.min.js".to_string()];
///
/// apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);
/// ```
pub fn apply_exclude_patterns<T, F>(items: &mut Vec<T>, patterns: &[String], get_path: F)
where
    F: Fn(&T) -> &str,
{
    if patterns.is_empty() {
        return;
    }

    items.retain(|item| {
        let path = get_path(item);
        !patterns
            .iter()
            .any(|pattern| matches_exclude_pattern(path, pattern))
    });
}

/// Apply include patterns to a collection
///
/// Keeps only items whose paths match at least one include pattern.
/// Uses a generic `get_path` function to extract the path from each item.
///
/// # Arguments
///
/// * `items` - Mutable reference to collection to filter
/// * `patterns` - List of include patterns
/// * `get_path` - Function to extract path from an item
///
/// # Type Parameters
///
/// * `T` - Type of items in the collection
/// * `F` - Type of the path extraction function
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::filtering::apply_include_patterns;
/// use infiniloom_engine::types::RepoFile;
///
/// let mut files: Vec<RepoFile> = vec![/* ... */];
/// let include = vec!["*.rs".to_string(), "*.ts".to_string()];
///
/// apply_include_patterns(&mut files, &include, |f| &f.relative_path);
/// ```
pub fn apply_include_patterns<T, F>(items: &mut Vec<T>, patterns: &[String], get_path: F)
where
    F: Fn(&T) -> &str,
{
    if patterns.is_empty() {
        return;
    }

    items.retain(|item| {
        let path = get_path(item);
        patterns
            .iter()
            .any(|pattern| matches_include_pattern(path, pattern))
    });
}

/// Compile patterns into glob::Pattern objects
///
/// Used by CLI commands that need pre-compiled patterns for repeated use.
///
/// # Arguments
///
/// * `patterns` - List of pattern strings
///
/// # Returns
///
/// Vector of successfully compiled glob patterns.
/// Invalid patterns are silently skipped.
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::filtering::compile_patterns;
///
/// let patterns = vec!["*.rs".to_string(), "src/**/*.ts".to_string()];
/// let compiled = compile_patterns(&patterns);
/// assert_eq!(compiled.len(), 2);
/// ```
pub fn compile_patterns(patterns: &[String]) -> Vec<Pattern> {
    patterns.iter().filter_map(|p| compile_pattern(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // Exclude Pattern Tests
    // =============================================================================

    #[test]
    fn test_exclude_glob_patterns() {
        assert!(matches_exclude_pattern("foo.min.js", "*.min.js"));
        assert!(matches_exclude_pattern("dist/bundle.min.js", "*.min.js"));
        assert!(!matches_exclude_pattern("foo.js", "*.min.js"));
    }

    #[test]
    fn test_exclude_glob_recursive() {
        assert!(matches_exclude_pattern("src/tests/foo.rs", "**/tests/**"));
        assert!(matches_exclude_pattern("tests/unit/bar.rs", "**/tests/**"));
        assert!(!matches_exclude_pattern("src/main.rs", "**/tests/**"));
    }

    #[test]
    fn test_exclude_substring_match() {
        assert!(matches_exclude_pattern("node_modules/foo/bar.js", "node_modules"));
        assert!(matches_exclude_pattern("dist/bundle.js", "dist"));
        assert!(!matches_exclude_pattern("src/index.ts", "dist"));
    }

    #[test]
    fn test_exclude_prefix_match() {
        assert!(matches_exclude_pattern("target/debug/main", "target"));
        assert!(matches_exclude_pattern("vendor/lib.js", "vendor"));
        assert!(!matches_exclude_pattern("src/target.rs", "target"));
    }

    #[test]
    fn test_exclude_component_match() {
        assert!(matches_exclude_pattern("src/tests/foo.rs", "tests"));
        assert!(matches_exclude_pattern("lib/vendor/bar.js", "vendor"));
        assert!(!matches_exclude_pattern("src/main.rs", "tests"));
    }

    // =============================================================================
    // Include Pattern Tests
    // =============================================================================

    #[test]
    fn test_include_glob_patterns() {
        assert!(matches_include_pattern("foo.rs", "*.rs"));
        assert!(matches_include_pattern("src/main.rs", "*.rs"));
        assert!(!matches_include_pattern("foo.py", "*.rs"));
    }

    #[test]
    fn test_include_glob_recursive() {
        assert!(matches_include_pattern("src/foo/bar.rs", "src/**/*.rs"));
        assert!(matches_include_pattern("src/main.rs", "src/**/*.rs"));
        assert!(!matches_include_pattern("tests/foo.rs", "src/**/*.rs"));
    }

    #[test]
    fn test_include_substring_match() {
        assert!(matches_include_pattern("src/main.rs", "src"));
        assert!(matches_include_pattern("lib/index.ts", "lib"));
        assert!(!matches_include_pattern("tests/foo.rs", "src"));
    }

    #[test]
    fn test_include_suffix_match() {
        assert!(matches_include_pattern("foo.test.ts", ".test.ts"));
        assert!(matches_include_pattern("bar.spec.js", ".spec.js"));
        assert!(!matches_include_pattern("foo.ts", ".test.ts"));
    }

    // =============================================================================
    // Generic Filtering Tests
    // =============================================================================

    #[derive(Debug, Clone)]
    struct TestFile {
        path: String,
    }

    #[test]
    fn test_apply_exclude_patterns_empty() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "node_modules/lib.js".to_string() },
        ];

        apply_exclude_patterns(&mut files, &[], |f| &f.path);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_apply_exclude_patterns_basic() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "node_modules/lib.js".to_string() },
            TestFile { path: "dist/bundle.js".to_string() },
        ];

        let exclude = vec!["node_modules".to_string(), "dist".to_string()];
        apply_exclude_patterns(&mut files, &exclude, |f| &f.path);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
    }

    #[test]
    fn test_apply_exclude_patterns_glob() {
        let mut files = vec![
            TestFile { path: "foo.js".to_string() },
            TestFile { path: "foo.min.js".to_string() },
            TestFile { path: "bar.js".to_string() },
        ];

        let exclude = vec!["*.min.js".to_string()];
        apply_exclude_patterns(&mut files, &exclude, |f| &f.path);

        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| !f.path.contains(".min.")));
    }

    #[test]
    fn test_apply_include_patterns_empty() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "src/lib.py".to_string() },
        ];

        apply_include_patterns(&mut files, &[], |f| &f.path);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_apply_include_patterns_basic() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "src/lib.py".to_string() },
            TestFile { path: "src/index.ts".to_string() },
        ];

        let include = vec!["*.rs".to_string(), "*.ts".to_string()];
        apply_include_patterns(&mut files, &include, |f| &f.path);

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.path.ends_with(".rs")));
        assert!(files.iter().any(|f| f.path.ends_with(".ts")));
    }

    #[test]
    fn test_apply_include_patterns_substring() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "tests/test.rs".to_string() },
            TestFile { path: "lib/index.ts".to_string() },
        ];

        let include = vec!["src".to_string()];
        apply_include_patterns(&mut files, &include, |f| &f.path);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
    }

    #[test]
    fn test_compile_patterns() {
        let patterns = vec!["*.rs".to_string(), "*.ts".to_string(), "src/**/*.js".to_string()];

        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 3);
    }

    #[test]
    fn test_compile_patterns_invalid() {
        let patterns = vec![
            "*.rs".to_string(),
            "[invalid".to_string(), // Invalid glob
            "*.ts".to_string(),
        ];

        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 2); // Invalid pattern skipped
    }

    // =============================================================================
    // Integration Tests
    // =============================================================================

    #[test]
    fn test_exclude_then_include() {
        let mut files = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "src/lib.rs".to_string() },
            TestFile { path: "src/main.test.rs".to_string() },
            TestFile { path: "node_modules/lib.js".to_string() },
        ];

        // First exclude test files and node_modules
        let exclude = vec!["node_modules".to_string(), "*.test.rs".to_string()];
        apply_exclude_patterns(&mut files, &exclude, |f| &f.path);
        assert_eq!(files.len(), 2);

        // Then include only Rust files
        let include = vec!["*.rs".to_string()];
        apply_include_patterns(&mut files, &include, |f| &f.path);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.path.ends_with(".rs")));
    }

    #[test]
    fn test_pattern_cache() {
        // First call compiles pattern
        let pattern1 = compile_pattern("*.rs");
        assert!(pattern1.is_some());

        // Second call uses cache
        let pattern2 = compile_pattern("*.rs");
        assert!(pattern2.is_some());

        // Patterns should be equal
        assert!(pattern1.unwrap().matches("foo.rs"));
        assert!(pattern2.unwrap().matches("foo.rs"));
    }
}
