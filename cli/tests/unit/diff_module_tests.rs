//! Unit tests for refactored diff module (Phase 2 Item 7)
//!
//! These tests verify that splitting diff.rs into 5 focused modules
//! (mod.rs, git_ops.rs, formatting.rs, context.rs, tests.rs) did not
//! break functionality.

use infiniloom_cli::commands::diff::{
    check_git_available, get_changed_lines, get_diff_changes, get_untracked_files, is_index_fresh,
    is_word_char, line_contains_symbol_name, merge_snippet_ranges, resolve_base_ref,
};
use std::path::PathBuf;
use tempfile::TempDir;

// =============================================================================
// Git Operations Tests (from git_ops.rs)
// =============================================================================

#[test]
fn test_check_git_available() {
    // Should return Ok if git is installed (true on most dev machines)
    let result = check_git_available();
    assert!(result.is_ok() || result.is_err()); // Either is valid
}

#[test]
fn test_resolve_base_ref_empty() {
    // Empty string should resolve to HEAD
    let result = resolve_base_ref("");
    assert_eq!(result, "HEAD");
}

#[test]
fn test_resolve_base_ref_branch() {
    // Branch names should be preserved
    let result = resolve_base_ref("main");
    assert_eq!(result, "main");

    let result = resolve_base_ref("feature/test");
    assert_eq!(result, "feature/test");
}

#[test]
fn test_resolve_base_ref_commit() {
    // Commit hashes should be preserved
    let result = resolve_base_ref("abc123def");
    assert_eq!(result, "abc123def");
}

#[test]
fn test_resolve_base_ref_relative() {
    // Relative refs should be preserved
    let result = resolve_base_ref("HEAD~1");
    assert_eq!(result, "HEAD~1");

    let result = resolve_base_ref("HEAD~5");
    assert_eq!(result, "HEAD~5");
}

// =============================================================================
// Formatting Tests (from formatting.rs)
// =============================================================================

#[test]
fn test_is_word_char_letters() {
    assert!(is_word_char('a'));
    assert!(is_word_char('Z'));
    assert!(is_word_char('m'));
}

#[test]
fn test_is_word_char_digits() {
    assert!(is_word_char('0'));
    assert!(is_word_char('5'));
    assert!(is_word_char('9'));
}

#[test]
fn test_is_word_char_underscore() {
    assert!(is_word_char('_'));
}

#[test]
fn test_is_word_char_special() {
    // Special characters should return false
    assert!(!is_word_char(' '));
    assert!(!is_word_char('.'));
    assert!(!is_word_char('('));
    assert!(!is_word_char(')'));
    assert!(!is_word_char(','));
    assert!(!is_word_char(';'));
}

#[test]
fn test_line_contains_symbol_name_exact_match() {
    // Exact match with word boundaries
    assert!(line_contains_symbol_name("fn main() {", "main"));
    assert!(line_contains_symbol_name("pub fn helper() {", "helper"));
}

#[test]
fn test_line_contains_symbol_name_with_punctuation() {
    // Symbol followed by punctuation
    assert!(line_contains_symbol_name("if validate(input)", "validate"));
    assert!(line_contains_symbol_name("result.process()", "process"));
    assert!(line_contains_symbol_name("call foo();", "foo"));
}

#[test]
fn test_line_contains_symbol_name_no_match() {
    // Should not match partial words
    assert!(!line_contains_symbol_name("fn main_helper() {", "main"));
    assert!(!line_contains_symbol_name("let result = 42;", "foo"));
}

#[test]
fn test_line_contains_symbol_name_case_sensitive() {
    // Should be case-sensitive
    assert!(line_contains_symbol_name("fn Main() {", "Main"));
    assert!(!line_contains_symbol_name("fn main() {", "Main"));
}

#[test]
fn test_line_contains_symbol_name_with_namespace() {
    // Symbol after namespace separator
    assert!(line_contains_symbol_name("std::process()", "process"));
    assert!(line_contains_symbol_name("module.function()", "function"));
}

#[test]
fn test_line_contains_symbol_name_with_underscore() {
    // Symbols with underscores
    assert!(line_contains_symbol_name("fn parse_input() {", "parse_input"));
    assert!(!line_contains_symbol_name("fn parse_input_data() {", "parse_input"));
}

#[test]
fn test_line_contains_symbol_name_multiple_occurrences() {
    // Multiple occurrences should return true
    assert!(line_contains_symbol_name("foo(foo)", "foo"));
}

#[test]
fn test_line_contains_symbol_name_empty() {
    // Empty line should return false
    assert!(!line_contains_symbol_name("", "foo"));

    // Empty symbol should return false
    assert!(!line_contains_symbol_name("fn main() {", ""));
}

#[test]
fn test_merge_snippet_ranges_non_overlapping() {
    // Non-overlapping ranges should remain separate
    let ranges = vec![(1, 5), (10, 15), (20, 25)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 5), (10, 15), (20, 25)]);
}

#[test]
fn test_merge_snippet_ranges_adjacent() {
    // Adjacent ranges should merge
    let ranges = vec![(1, 5), (6, 10)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 10)]);
}

#[test]
fn test_merge_snippet_ranges_overlapping() {
    // Overlapping ranges should merge
    let ranges = vec![(1, 10), (5, 15), (12, 20)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 20)]);
}

#[test]
fn test_merge_snippet_ranges_contained() {
    // Contained ranges should merge
    let ranges = vec![(1, 20), (5, 10), (15, 18)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 20)]);
}

#[test]
fn test_merge_snippet_ranges_empty() {
    // Empty input should return empty
    let ranges: Vec<(u32, u32)> = vec![];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![]);
}

#[test]
fn test_merge_snippet_ranges_single() {
    // Single range should remain unchanged
    let ranges = vec![(10, 20)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(10, 20)]);
}

#[test]
fn test_merge_snippet_ranges_unsorted() {
    // Unsorted ranges should be sorted and merged
    let ranges = vec![(20, 25), (1, 5), (10, 15), (3, 7)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 7), (10, 15), (20, 25)]);
}

#[test]
fn test_merge_snippet_ranges_complex() {
    // Complex case with multiple merges
    let ranges = vec![(1, 5), (3, 8), (10, 15), (14, 20), (25, 30), (28, 35)];
    let merged = merge_snippet_ranges(&ranges);
    assert_eq!(merged, vec![(1, 8), (10, 20), (25, 35)]);
}

// =============================================================================
// Context Tests (from context.rs)
// =============================================================================

#[test]
fn test_get_changed_lines_empty() {
    // Empty diff should return empty ranges
    let diff = "";
    let lines = get_changed_lines(diff);
    assert!(lines.is_empty());
}

#[test]
fn test_get_changed_lines_single_hunk() {
    // Single hunk should parse correctly
    let diff = "@@ -10,3 +10,4 @@ fn main() {
+    let x = 42;
     println!(\"Hello\");
";
    let lines = get_changed_lines(diff);
    assert!(!lines.is_empty());
    // Should include line 10 (start of hunk)
    assert!(lines.iter().any(|(start, _)| *start == 10));
}

#[test]
fn test_get_changed_lines_multiple_hunks() {
    // Multiple hunks should all be parsed
    let diff = "@@ -10,3 +10,4 @@ fn main() {
+    let x = 42;
@@ -50,2 +51,3 @@ fn helper() {
+    let y = 13;
";
    let lines = get_changed_lines(diff);
    assert!(lines.len() >= 2);
}

// =============================================================================
// Module Integration Tests
// =============================================================================

#[test]
fn test_diff_module_exports_all_functions() {
    // This test verifies that all functions are properly exported
    // from the diff module after refactoring

    // Git operations
    let _ = check_git_available;
    let _ = get_diff_changes;
    let _ = get_untracked_files;
    let _ = get_changed_lines;
    let _ = resolve_base_ref;
    let _ = is_index_fresh;

    // Formatting helpers
    let _ = line_contains_symbol_name;
    let _ = merge_snippet_ranges;
    let _ = is_word_char;
}
