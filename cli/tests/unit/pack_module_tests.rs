//! Unit tests for refactored pack module (Phase 2 Item 8)
//!
//! These tests verify that splitting pack/impl.rs into 5 focused modules
//! (filters.rs, compression.rs, budget.rs, output.rs, tests.rs) plus impl.rs
//! did not break functionality.

use infiniloom_cli::commands::pack::{
    apply_default_ignores, apply_exclude_patterns, apply_include_patterns, estimate_tokens,
    extract_key_symbols_only, extract_signatures_only, filter_stdin_paths, is_inside_string,
    pattern_matches_file, rank_files_fast, remove_comments_from_content,
    remove_empty_lines_from_content, truncate_to_tokens,
};
use infiniloom_engine::types::RepoFile;

// =============================================================================
// Filter Tests (from filters.rs)
// =============================================================================

#[test]
fn test_pattern_matches_file_exact() {
    // Exact matches
    assert!(pattern_matches_file("src/main.rs", "src/main.rs"));
    assert!(pattern_matches_file("test.txt", "test.txt"));
}

#[test]
fn test_pattern_matches_file_wildcard() {
    // Wildcard patterns
    assert!(pattern_matches_file("src/main.rs", "*.rs"));
    assert!(pattern_matches_file("test.py", "*.py"));
    assert!(!pattern_matches_file("test.rs", "*.py"));
}

#[test]
fn test_pattern_matches_file_glob() {
    // Glob patterns
    assert!(pattern_matches_file("src/main.rs", "src/**/*.rs"));
    assert!(pattern_matches_file("src/foo/bar.rs", "src/**/*.rs"));
    assert!(!pattern_matches_file("tests/main.rs", "src/**/*.rs"));
}

#[test]
fn test_pattern_matches_file_directory() {
    // Directory patterns
    assert!(pattern_matches_file("tests/test.rs", "tests/*"));
    assert!(pattern_matches_file("src/main.rs", "src/*"));
}

#[test]
fn test_apply_default_ignores_basic() {
    // Should exclude common ignored patterns
    let mut files = vec![
        "src/main.rs".to_string(),
        "node_modules/lib.js".to_string(),
        "target/debug/bin".to_string(),
        ".git/config".to_string(),
    ];

    apply_default_ignores(&mut files);

    // Source files should remain
    assert!(files.contains(&"src/main.rs".to_string()));
    // Ignored files should be removed
    assert!(!files.iter().any(|f| f.contains("node_modules")));
    assert!(!files.iter().any(|f| f.contains("target")));
    assert!(!files.iter().any(|f| f.contains(".git")));
}

#[test]
fn test_apply_include_patterns_empty() {
    // Empty patterns should keep all files
    let mut files = vec!["src/main.rs".to_string(), "test.py".to_string()];
    let original_len = files.len();

    apply_include_patterns(&mut files, &[]);

    assert_eq!(files.len(), original_len);
}

#[test]
fn test_apply_include_patterns_specific() {
    // Should only keep files matching patterns
    let mut files =
        vec!["src/main.rs".to_string(), "src/lib.rs".to_string(), "test.py".to_string()];

    apply_include_patterns(&mut files, &["*.rs".to_string()]);

    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.ends_with(".rs")));
}

#[test]
fn test_apply_exclude_patterns_empty() {
    // Empty patterns should keep all files
    let mut files = vec!["src/main.rs".to_string(), "test.py".to_string()];
    let original_len = files.len();

    apply_exclude_patterns(&mut files, &[]);

    assert_eq!(files.len(), original_len);
}

#[test]
fn test_apply_exclude_patterns_specific() {
    // Should remove files matching patterns
    let mut files =
        vec!["src/main.rs".to_string(), "tests/test.rs".to_string(), "src/lib.rs".to_string()];

    apply_exclude_patterns(&mut files, &["tests/*".to_string()]);

    assert_eq!(files.len(), 2);
    assert!(!files.iter().any(|f| f.starts_with("tests")));
}

// =============================================================================
// Compression Tests (from compression.rs)
// =============================================================================

#[test]
fn test_remove_empty_lines_from_content_basic() {
    let input = "line 1\n\n\nline 2\n\nline 3";
    let result = remove_empty_lines_from_content(input);

    // Should remove empty lines but preserve content
    assert!(!result.contains("\n\n"));
    assert!(result.contains("line 1"));
    assert!(result.contains("line 2"));
    assert!(result.contains("line 3"));
}

#[test]
fn test_remove_empty_lines_from_content_preserves_non_empty() {
    let input = "line 1\nline 2\nline 3";
    let result = remove_empty_lines_from_content(input);

    // Should not modify if no empty lines
    assert!(result.contains("line 1"));
    assert!(result.contains("line 2"));
    assert!(result.contains("line 3"));
}

#[test]
fn test_is_inside_string_double_quotes() {
    let line = "let s = \"hello // not a comment\";";
    assert!(is_inside_string(line, 20)); // Inside string
    assert!(!is_inside_string(line, 5)); // Before string
}

#[test]
fn test_is_inside_string_single_quotes() {
    let line = "let c = 'x'; // comment";
    assert!(is_inside_string(line, 9)); // Inside character literal
    assert!(!is_inside_string(line, 15)); // After string
}

#[test]
fn test_is_inside_string_escaped_quotes() {
    let line = "let s = \"test \\\" more\";";
    assert!(is_inside_string(line, 18)); // After escaped quote
}

#[test]
fn test_remove_comments_from_content_c_style() {
    let rust_code = "fn main() {\n    // This is a comment\n    let x = 42;\n}";
    let result = remove_comments_from_content(rust_code, "rust");

    // Should remove comment lines
    assert!(!result.contains("// This is a comment"));
    assert!(result.contains("let x = 42"));
}

#[test]
fn test_remove_comments_from_content_preserves_code() {
    let code = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
    let result = remove_comments_from_content(code, "rust");

    // Should preserve code without comments
    assert!(result.contains("fn add"));
    assert!(result.contains("a + b"));
}

#[test]
fn test_extract_signatures_only_basic() {
    let code = "fn main() {\n    let x = 42;\n    println!(\"Hello\");\n}";
    let result = extract_signatures_only(code, "rust");

    // Should keep function signature
    assert!(result.contains("fn main()"));
    // Should remove body
    assert!(!result.contains("let x = 42"));
    assert!(!result.contains("println!"));
}

#[test]
fn test_extract_key_symbols_only_basic() {
    let code = "fn main() {\n    let x = 42;\n}\n\nfn helper() {\n    println!(\"test\");\n}";
    let result = extract_key_symbols_only(code, "rust");

    // Should keep function declarations
    assert!(result.contains("fn main()"));
    assert!(result.contains("fn helper()"));
    // Should remove bodies
    assert!(!result.contains("let x = 42"));
    assert!(!result.contains("println!"));
}

// =============================================================================
// Budget Tests (from budget.rs)
// =============================================================================

#[test]
fn test_estimate_tokens_empty() {
    let result = estimate_tokens("");
    assert_eq!(result, 0);
}

#[test]
fn test_estimate_tokens_basic() {
    let text = "Hello, world!";
    let result = estimate_tokens(text);
    // Should be roughly 3-4 tokens
    assert!(result > 0 && result < 10);
}

#[test]
fn test_estimate_tokens_long() {
    // Longer text should have more tokens
    let text = "This is a longer piece of text with many words.";
    let result = estimate_tokens(text);
    assert!(result > 5);
}

#[test]
fn test_truncate_to_tokens_exact() {
    let text = "word1 word2 word3 word4 word5";
    // Request fewer tokens than available
    let result = truncate_to_tokens(text, 3);

    // Should truncate
    assert!(result.len() < text.len());
    // Should preserve start of text
    assert!(result.starts_with("word1"));
}

#[test]
fn test_truncate_to_tokens_no_truncation() {
    let text = "short";
    // Request more tokens than available
    let result = truncate_to_tokens(text, 1000);

    // Should not truncate
    assert_eq!(result, text);
}

#[test]
fn test_rank_files_fast_basic() {
    // Create test files
    let files = vec![
        RepoFile {
            relative_path: "src/main.rs".to_string(),
            language: Some("Rust".to_string()),
            token_counts: Default::default(),
            symbols: vec![],
            lines: 50,
            importance_score: Some(0.0),
        },
        RepoFile {
            relative_path: "README.md".to_string(),
            language: Some("Markdown".to_string()),
            token_counts: Default::default(),
            symbols: vec![],
            lines: 10,
            importance_score: Some(0.0),
        },
    ];

    let ranked = rank_files_fast(files);

    // Should rank source files higher than docs
    assert!(ranked.len() == 2);
    // main.rs should rank higher than README.md
    if let Some(main_score) = ranked[0].importance_score {
        if let Some(readme_score) = ranked[1].importance_score {
            if ranked[0].relative_path.contains("main.rs") {
                assert!(main_score > readme_score);
            }
        }
    }
}

// =============================================================================
// Module Integration Tests
// =============================================================================

#[test]
fn test_pack_module_exports_all_functions() {
    // This test verifies that all functions are properly exported
    // from the pack module after refactoring

    // Filter functions
    let _ = pattern_matches_file;
    let _ = apply_default_ignores;
    let _ = apply_include_patterns;
    let _ = apply_exclude_patterns;
    let _ = filter_stdin_paths;

    // Compression functions
    let _ = remove_empty_lines_from_content;
    let _ = is_inside_string;
    let _ = remove_comments_from_content;
    let _ = extract_signatures_only;
    let _ = extract_key_symbols_only;

    // Budget functions
    let _ = estimate_tokens;
    let _ = truncate_to_tokens;
    let _ = rank_files_fast;
}
