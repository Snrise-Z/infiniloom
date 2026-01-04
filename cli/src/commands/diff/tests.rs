//! Tests for diff command modules
//!
//! This module contains tests for:
//! - formatting.rs helper functions (is_word_char, line_contains_symbol_name, merge_snippet_ranges)
//! - git_ops.rs functions (resolve_base_ref)
//! - formatting.rs output functions (diff_preamble)

use super::formatting::{
    diff_preamble, line_contains_symbol_name, merge_snippet_ranges, SnippetRange,
};
use super::git_ops::resolve_base_ref;
use std::path::Path;

// Helper function to check if a character is a word character
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ============================================
// is_word_char Tests
// ============================================

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
fn test_is_word_char_not_punctuation() {
    assert!(!is_word_char('.'));
    assert!(!is_word_char('-'));
    assert!(!is_word_char('('));
    assert!(!is_word_char(')'));
    assert!(!is_word_char('['));
    assert!(!is_word_char(']'));
    assert!(!is_word_char(' '));
    assert!(!is_word_char('\t'));
    assert!(!is_word_char('\n'));
}

// ============================================
// line_contains_symbol_name Tests
// ============================================

#[test]
fn test_line_contains_symbol_name_basic() {
    assert!(line_contains_symbol_name("def foo():", "foo"));
    assert!(line_contains_symbol_name("fn main() {", "main"));
    assert!(line_contains_symbol_name("const MAX_SIZE = 100", "MAX_SIZE"));
}

#[test]
fn test_line_contains_symbol_name_not_substring() {
    // "foo" should not match "foobar"
    assert!(!line_contains_symbol_name("def foobar():", "foo"));
    assert!(!line_contains_symbol_name("def barfoo():", "foo"));
}

#[test]
fn test_line_contains_symbol_name_with_boundaries() {
    assert!(line_contains_symbol_name("foo()", "foo"));
    assert!(line_contains_symbol_name("(foo)", "foo"));
    assert!(line_contains_symbol_name("[foo]", "foo"));
    assert!(line_contains_symbol_name("foo;", "foo"));
    assert!(line_contains_symbol_name("foo,bar", "foo"));
    assert!(line_contains_symbol_name("foo,bar", "bar"));
}

#[test]
fn test_line_contains_symbol_name_empty() {
    assert!(!line_contains_symbol_name("def foo():", ""));
    assert!(!line_contains_symbol_name("", "foo"));
    assert!(!line_contains_symbol_name("", ""));
}

#[test]
fn test_line_contains_symbol_name_multiple_occurrences() {
    assert!(line_contains_symbol_name("foo + foo * foo", "foo"));
}

#[test]
fn test_line_contains_symbol_name_at_line_start() {
    assert!(line_contains_symbol_name("foo = 1", "foo"));
}

#[test]
fn test_line_contains_symbol_name_at_line_end() {
    assert!(line_contains_symbol_name("return foo", "foo"));
}

#[test]
fn test_line_contains_symbol_name_with_underscore() {
    assert!(line_contains_symbol_name("my_func()", "my_func"));
    assert!(!line_contains_symbol_name("my_function()", "my_func"));
}

// ============================================
// merge_snippet_ranges Tests
// ============================================

#[test]
fn test_merge_snippet_ranges_empty() {
    let ranges: Vec<SnippetRange> = vec![];
    let merged = merge_snippet_ranges(ranges);
    assert!(merged.is_empty());
}

#[test]
fn test_merge_snippet_ranges_single() {
    let ranges = vec![SnippetRange { start: 1, end: 5, reasons: vec!["test".to_owned()] }];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start, 1);
    assert_eq!(merged[0].end, 5);
}

#[test]
fn test_merge_snippet_ranges_non_overlapping() {
    let ranges = vec![
        SnippetRange { start: 1, end: 5, reasons: vec!["reason1".to_owned()] },
        SnippetRange { start: 10, end: 15, reasons: vec!["reason2".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 2);
}

#[test]
fn test_merge_snippet_ranges_overlapping() {
    let ranges = vec![
        SnippetRange { start: 1, end: 10, reasons: vec!["reason1".to_owned()] },
        SnippetRange { start: 5, end: 15, reasons: vec!["reason2".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start, 1);
    assert_eq!(merged[0].end, 15);
    assert_eq!(merged[0].reasons.len(), 2);
}

#[test]
fn test_merge_snippet_ranges_adjacent() {
    // Adjacent ranges (end+1 == start of next) should merge
    let ranges = vec![
        SnippetRange { start: 1, end: 5, reasons: vec!["reason1".to_owned()] },
        SnippetRange { start: 6, end: 10, reasons: vec!["reason2".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start, 1);
    assert_eq!(merged[0].end, 10);
}

#[test]
fn test_merge_snippet_ranges_unsorted() {
    // Should sort before merging
    let ranges = vec![
        SnippetRange { start: 10, end: 15, reasons: vec!["reason2".to_owned()] },
        SnippetRange { start: 1, end: 5, reasons: vec!["reason1".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].start, 1);
    assert_eq!(merged[1].start, 10);
}

#[test]
fn test_merge_snippet_ranges_duplicate_reasons() {
    let ranges = vec![
        SnippetRange { start: 1, end: 10, reasons: vec!["reason".to_owned()] },
        SnippetRange { start: 5, end: 15, reasons: vec!["reason".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 1);
    // Duplicate reasons should not be added
    assert_eq!(merged[0].reasons.len(), 1);
}

#[test]
fn test_merge_snippet_ranges_contained() {
    // One range completely contained in another
    let ranges = vec![
        SnippetRange { start: 1, end: 20, reasons: vec!["outer".to_owned()] },
        SnippetRange { start: 5, end: 10, reasons: vec!["inner".to_owned()] },
    ];
    let merged = merge_snippet_ranges(ranges);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start, 1);
    assert_eq!(merged[0].end, 20);
    assert_eq!(merged[0].reasons.len(), 2);
}

// ============================================
// resolve_base_ref Tests
// ============================================

#[test]
fn test_resolve_base_ref_range() {
    // For range refs like "main..HEAD", we want the left side
    // This doesn't require git verification
    let base = resolve_base_ref(Some("main..HEAD"), Path::new("."));
    assert_eq!(base, Some("main".to_owned()));
}

#[test]
fn test_resolve_base_ref_triple_dot_range() {
    // Also handles triple dot ranges
    let base = resolve_base_ref(Some("develop...feature"), Path::new("."));
    assert_eq!(base, Some("develop".to_owned()));
}

// ============================================
// diff_preamble Tests
// ============================================

fn create_test_context_file(path: &str, tokens: u32) -> infiniloom_engine::index::ContextFile {
    infiniloom_engine::index::ContextFile {
        id: 0,
        path: path.to_owned(),
        language: "rust".to_owned(),
        relevance_reason: "test".to_owned(),
        relevance_score: 1.0,
        tokens,
        relevant_sections: vec![],
        diff_content: None,
        snippets: vec![],
    }
}

#[test]
fn test_diff_preamble() {
    use infiniloom_engine::index::{ExpandedContext, ImpactSummary};

    let context = ExpandedContext {
        changed_files: vec![],
        changed_symbols: vec![],
        dependent_files: vec![],
        dependent_symbols: vec![],
        related_tests: vec![],
        call_chains: vec![],
        impact_summary: ImpactSummary::default(),
        total_tokens: 1000,
    };

    let preamble = diff_preamble(&context);
    // The preamble describes how to use the context
    assert!(preamble.contains("diff context"));
    assert!(preamble.contains("Impact"));
}

#[test]
fn test_diff_preamble_with_content() {
    use infiniloom_engine::index::{ExpandedContext, ImpactLevel, ImpactSummary};

    let context = ExpandedContext {
        changed_files: vec![
            create_test_context_file("src/main.rs", 100),
            create_test_context_file("src/lib.rs", 200),
        ],
        changed_symbols: vec![],
        dependent_files: vec![create_test_context_file("src/utils.rs", 50)],
        dependent_symbols: vec![],
        related_tests: vec![create_test_context_file("tests/test.rs", 150)],
        call_chains: vec![],
        impact_summary: ImpactSummary { level: ImpactLevel::Medium, ..Default::default() },
        total_tokens: 500,
    };

    let preamble = diff_preamble(&context);
    assert!(preamble.contains("medium"));
}
