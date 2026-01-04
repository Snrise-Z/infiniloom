//! Integration tests for Phase 3 Item 11: Centralized Filtering
//!
//! These tests verify that the centralized filtering module works correctly
//! when integrated with all commands (diff, scan, map, chunk, pack).

use infiniloom_engine::filtering::{
    apply_exclude_patterns, apply_include_patterns, matches_exclude_pattern,
    matches_include_pattern,
};
use infiniloom_engine::types::RepoFile;
use std::path::PathBuf;

// =============================================================================
// Helper Functions
// =============================================================================

fn create_test_files() -> Vec<RepoFile> {
    vec![
        RepoFile {
            path: PathBuf::from("/repo/src/main.rs"),
            relative_path: "src/main.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/src/lib.rs"),
            relative_path: "src/lib.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 200,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/tests/test_main.rs"),
            relative_path: "tests/test_main.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 150,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/node_modules/lib.js"),
            relative_path: "node_modules/lib.js".to_owned(),
            language: Some("JavaScript".to_owned()),
            size_bytes: 500,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/dist/bundle.min.js"),
            relative_path: "dist/bundle.min.js".to_owned(),
            language: Some("JavaScript".to_owned()),
            size_bytes: 1000,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
    ]
}

// =============================================================================
// Pattern Matching Tests
// =============================================================================

#[test]
fn test_exclude_pattern_node_modules() {
    assert!(matches_exclude_pattern("node_modules/foo/bar.js", "node_modules"));
    assert!(matches_exclude_pattern("node_modules/lib.js", "node_modules"));
}

#[test]
fn test_exclude_pattern_dist() {
    assert!(matches_exclude_pattern("dist/bundle.js", "dist"));
    assert!(matches_exclude_pattern("dist/output/main.js", "dist"));
}

#[test]
fn test_exclude_pattern_glob() {
    assert!(matches_exclude_pattern("foo.min.js", "*.min.js"));
    assert!(matches_exclude_pattern("dist/bundle.min.js", "*.min.js"));
    assert!(!matches_exclude_pattern("foo.js", "*.min.js"));
}

#[test]
fn test_include_pattern_rust_files() {
    assert!(matches_include_pattern("src/main.rs", "*.rs"));
    assert!(matches_include_pattern("tests/test.rs", "*.rs"));
    assert!(!matches_include_pattern("src/main.js", "*.rs"));
}

#[test]
fn test_include_pattern_src_directory() {
    assert!(matches_include_pattern("src/main.rs", "src"));
    assert!(matches_include_pattern("src/lib/utils.rs", "src"));
    assert!(!matches_include_pattern("tests/test.rs", "src"));
}

// =============================================================================
// Apply Filters Tests
// =============================================================================

#[test]
fn test_apply_exclude_removes_node_modules() {
    let mut files = create_test_files();
    let exclude = vec!["node_modules".to_owned()];

    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    assert_eq!(files.len(), 4);
    assert!(!files
        .iter()
        .any(|f| f.relative_path.contains("node_modules")));
}

#[test]
fn test_apply_exclude_removes_dist() {
    let mut files = create_test_files();
    let exclude = vec!["dist".to_owned()];

    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    assert_eq!(files.len(), 4);
    assert!(!files.iter().any(|f| f.relative_path.contains("dist")));
}

#[test]
fn test_apply_exclude_multiple_patterns() {
    let mut files = create_test_files();
    let exclude = vec!["node_modules".to_owned(), "dist".to_owned()];

    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    assert_eq!(files.len(), 3);
    assert!(!files
        .iter()
        .any(|f| f.relative_path.contains("node_modules")));
    assert!(!files.iter().any(|f| f.relative_path.contains("dist")));
}

#[test]
fn test_apply_exclude_tests_directory() {
    let mut files = create_test_files();
    let exclude = vec!["tests".to_owned()];

    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    assert_eq!(files.len(), 4);
    assert!(!files.iter().any(|f| f.relative_path.contains("tests/")));
}

#[test]
fn test_apply_include_only_rust_files() {
    let mut files = create_test_files();
    let include = vec!["*.rs".to_owned()];

    apply_include_patterns(&mut files, &include, |f| &f.relative_path);

    assert_eq!(files.len(), 3);
    assert!(files.iter().all(|f| f.relative_path.ends_with(".rs")));
}

#[test]
fn test_apply_include_only_src_directory() {
    let mut files = create_test_files();
    let include = vec!["src".to_owned()];

    apply_include_patterns(&mut files, &include, |f| &f.relative_path);

    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.relative_path.starts_with("src/")));
}

#[test]
fn test_apply_include_multiple_patterns() {
    let mut files = create_test_files();
    let include = vec!["*.rs".to_owned(), "*.js".to_owned()];

    apply_include_patterns(&mut files, &include, |f| &f.relative_path);

    assert_eq!(files.len(), 5);
    assert!(files
        .iter()
        .all(|f| f.relative_path.ends_with(".rs") || f.relative_path.ends_with(".js")));
}

// =============================================================================
// Combined Filter Tests (Exclude + Include)
// =============================================================================

#[test]
fn test_exclude_then_include() {
    let mut files = create_test_files();

    // First exclude build outputs and dependencies
    let exclude = vec!["node_modules".to_owned(), "dist".to_owned()];
    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);
    assert_eq!(files.len(), 3);

    // Then include only Rust files
    let include = vec!["*.rs".to_owned()];
    apply_include_patterns(&mut files, &include, |f| &f.relative_path);
    assert_eq!(files.len(), 3);
    assert!(files.iter().all(|f| f.relative_path.ends_with(".rs")));
}

#[test]
fn test_exclude_tests_then_include_src() {
    let mut files = create_test_files();

    // Exclude test files
    let exclude = vec!["tests".to_owned()];
    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);
    assert_eq!(files.len(), 4);

    // Include only src directory
    let include = vec!["src".to_owned()];
    apply_include_patterns(&mut files, &include, |f| &f.relative_path);
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.relative_path.starts_with("src/")));
}

// =============================================================================
// Empty Pattern Tests
// =============================================================================

#[test]
fn test_apply_exclude_empty_patterns() {
    let mut files = create_test_files();
    let original_len = files.len();

    apply_exclude_patterns(&mut files, &[], |f| &f.relative_path);

    assert_eq!(files.len(), original_len);
}

#[test]
fn test_apply_include_empty_patterns() {
    let mut files = create_test_files();
    let original_len = files.len();

    apply_include_patterns(&mut files, &[], |f| &f.relative_path);

    assert_eq!(files.len(), original_len);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_exclude_pattern_with_slash() {
    let mut files = create_test_files();
    let exclude = vec!["node_modules/".to_owned()];

    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    // Should still exclude node_modules files (substring match)
    assert!(!files
        .iter()
        .any(|f| f.relative_path.contains("node_modules")));
}

#[test]
fn test_include_pattern_case_sensitive() {
    let mut files = vec![
        RepoFile {
            path: PathBuf::from("/repo/Main.rs"),
            relative_path: "Main.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/main.rs"),
            relative_path: "main.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
    ];

    let include = vec!["*.rs".to_owned()];
    apply_include_patterns(&mut files, &include, |f| &f.relative_path);

    // Both should match (glob is case-insensitive for extensions)
    assert_eq!(files.len(), 2);
}

#[test]
fn test_exclude_pattern_component_match_deep() {
    let mut files = vec![
        RepoFile {
            path: PathBuf::from("/repo/src/tests/foo.rs"),
            relative_path: "src/tests/foo.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: PathBuf::from("/repo/src/main.rs"),
            relative_path: "src/main.rs".to_owned(),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        },
    ];

    let exclude = vec!["tests".to_owned()];
    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    // Should exclude files with "tests" as a path component
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].relative_path, "src/main.rs");
}

// =============================================================================
// Performance Tests
// =============================================================================

#[test]
fn test_large_file_list_filtering() {
    // Create 1000 test files
    let mut files: Vec<RepoFile> = (0..1000)
        .map(|i| RepoFile {
            path: PathBuf::from(format!("/repo/src/file{}.rs", i)),
            relative_path: format!("src/file{}.rs", i),
            language: Some("Rust".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        })
        .collect();

    // Add some node_modules files
    for i in 0..100 {
        files.push(RepoFile {
            path: PathBuf::from(format!("/repo/node_modules/lib{}.js", i)),
            relative_path: format!("node_modules/lib{}.js", i),
            language: Some("JavaScript".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });
    }

    let exclude = vec!["node_modules".to_owned()];
    apply_exclude_patterns(&mut files, &exclude, |f| &f.relative_path);

    // Should have filtered out all node_modules files
    assert_eq!(files.len(), 1000);
    assert!(files
        .iter()
        .all(|f| !f.relative_path.contains("node_modules")));
}
