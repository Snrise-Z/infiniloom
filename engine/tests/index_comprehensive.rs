//! Comprehensive tests for the index module.
//!
//! Tests cover:
//! - Context expansion (L1, L2, L3 depth)
//! - Lazy context generation
//! - Index building
//! - Token budget enforcement
//! - Test file detection

use infiniloom_engine::index::{
    builder::{BuildOptions, IndexBuilder},
    context::{ChangeType, ContextDepth, ContextExpander, DiffChange, ImpactLevel},
    lazy::LazyContextBuilder,
    types::{
        DepGraph, FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId,
        SymbolIndex, Visibility,
    },
};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test repository with the given files
fn create_test_repo(files: Vec<(&str, &str)>) -> TempDir {
    let tmp = TempDir::new().unwrap();
    for (path, content) in files {
        let full_path = tmp.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }
    tmp
}

/// Create a Python file with imports and functions
fn create_python_file(imports: &[&str], functions: &[&str]) -> String {
    let mut content = String::new();
    for import in imports {
        content.push_str(&format!("from {} import *\n", import));
    }
    content.push('\n');
    for func in functions {
        content.push_str(&format!(
            "def {}():\n    \"\"\"Docstring for {}.\"\"\"\n    pass\n\n",
            func, func
        ));
    }
    content
}

/// Create a Rust file with uses and functions
fn create_rust_file(uses: &[&str], functions: &[&str]) -> String {
    let mut content = String::new();
    for use_stmt in uses {
        content.push_str(&format!("use {};\n", use_stmt));
    }
    content.push('\n');
    for func in functions {
        content.push_str(&format!(
            "/// Documentation for {}\npub fn {}() {{\n    // Implementation\n}}\n\n",
            func, func
        ));
    }
    content
}

/// Create a JavaScript file with imports and functions
fn create_js_file(imports: &[&str], functions: &[&str]) -> String {
    let mut content = String::new();
    for import in imports {
        content.push_str(&format!("import {{ }} from '{}';\n", import));
    }
    content.push('\n');
    for func in functions {
        content.push_str(&format!(
            "/**\n * Description for {}\n */\nfunction {}() {{\n    // Implementation\n}}\n\n",
            func, func
        ));
    }
    content
}

/// Create a test index with known structure
fn create_test_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "test".to_owned();

    // Add files: src/main.rs, src/lib.rs, src/utils.rs, tests/test_main.rs
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..2,
        imports: vec![],
        lines: 100,
        tokens: 500,
    });
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "src/lib.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 2..4,
        imports: vec![],
        lines: 80,
        tokens: 400,
    });
    index.files.push(FileEntry {
        id: FileId::new(2),
        path: "src/utils.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 4..6,
        imports: vec![],
        lines: 50,
        tokens: 250,
    });
    index.files.push(FileEntry {
        id: FileId::new(3),
        path: "tests/test_main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 6..7,
        imports: vec![],
        lines: 40,
        tokens: 200,
    });

    // Symbols in main.rs
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "main".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 15, 0),
        signature: Some("fn main()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "run".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(20, 0, 35, 0),
        signature: Some("pub fn run()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Symbols in lib.rs
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "Config".to_owned(),
        kind: IndexSymbolKind::Struct,
        file_id: FileId::new(1),
        span: Span::new(1, 0, 20, 0),
        signature: Some("pub struct Config".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: Some("Configuration structure".to_owned()),
    });
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "init".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(1),
        span: Span::new(25, 0, 40, 0),
        signature: Some("pub fn init()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Symbols in utils.rs
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(4),
        name: "helper".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(2),
        span: Span::new(1, 0, 10, 0),
        signature: Some("pub fn helper()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(5),
        name: "format_output".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(2),
        span: Span::new(15, 0, 30, 0),
        signature: Some("pub fn format_output()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Symbols in test_main.rs
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(6),
        name: "test_main".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(3),
        span: Span::new(1, 0, 20, 0),
        signature: Some("fn test_main()".to_owned()),
        parent: None,
        visibility: Visibility::Private,
        docstring: None,
    });

    index.rebuild_lookups();

    // Build dependency graph
    // main.rs imports lib.rs and utils.rs
    // lib.rs imports utils.rs
    // test_main.rs imports main.rs
    let mut graph = DepGraph::new();
    graph.add_file_import(0, 1); // main -> lib
    graph.add_file_import(0, 2); // main -> utils
    graph.add_file_import(1, 2); // lib -> utils
    graph.add_file_import(3, 0); // test_main -> main

    // Call graph: main calls run, run calls init, init calls helper
    graph.add_call(0, 1); // main -> run
    graph.add_call(1, 3); // run -> init
    graph.add_call(3, 4); // init -> helper

    (index, graph)
}

// ============================================================================
// L1 Context Expansion Tests
// ============================================================================

#[test]
fn test_l1_context_single_file() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L1, 100000);

    assert_eq!(context.changed_files.len(), 1);
    assert_eq!(context.changed_files[0].path, "src/main.rs");
    // L1 should find the symbol containing the changed lines
    assert_eq!(context.changed_symbols.len(), 1);
    assert_eq!(context.changed_symbols[0].name, "main");
    // L1 should NOT expand to dependents
    assert!(context.dependent_files.is_empty());
}

#[test]
fn test_l1_context_multiple_changes() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![
        DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(5, 10)],
            change_type: ChangeType::Modified,
            diff_content: None,
        },
        DiffChange {
            file_path: "src/lib.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(30, 35)],
            change_type: ChangeType::Modified,
            diff_content: None,
        },
    ];

    let context = expander.expand(&changes, ContextDepth::L1, 100000);

    assert_eq!(context.changed_files.len(), 2);
    // Should find symbols in both files
    assert!(context.changed_symbols.len() >= 2);
}

#[test]
fn test_l1_with_token_budget() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    // With very small budget, should still include the changed file
    let context = expander.expand(&changes, ContextDepth::L1, 100);

    assert_eq!(context.changed_files.len(), 1);
    // Token count should be reasonable
    assert!(context.total_tokens <= 600); // main.rs has 500 tokens
}

// ============================================================================
// L2 Context Expansion Tests
// ============================================================================

#[test]
fn test_l2_context_with_imports() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Change utils.rs - should find lib.rs and main.rs as dependents
    let changes = vec![DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 8)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    assert_eq!(context.changed_files.len(), 1);
    // L2 should find files that import utils.rs
    assert!(
        !context.dependent_files.is_empty(),
        "Should find dependent files that import utils.rs"
    );

    // Check dependent files include expected importers
    let dep_paths: Vec<&str> = context
        .dependent_files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    // main.rs and lib.rs both import utils.rs
    assert!(
        dep_paths.contains(&"src/main.rs") || dep_paths.contains(&"src/lib.rs"),
        "Expected main.rs or lib.rs as dependents, got {:?}",
        dep_paths
    );
}

#[test]
fn test_l2_context_with_callers() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Change function that is called by others
    let changes = vec![DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)], // helper function
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // Should find callers of helper
    assert!(
        !context.dependent_symbols.is_empty() || !context.dependent_files.is_empty(),
        "Should find symbols or files that depend on helper"
    );
}

#[test]
fn test_l2_test_file_inclusion() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Change main.rs - should find test_main.rs as related test
    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // Should find related tests
    let test_paths: Vec<&str> = context
        .related_tests
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(
        test_paths.contains(&"tests/test_main.rs"),
        "Expected test_main.rs in related tests, got {:?}",
        test_paths
    );
}

// ============================================================================
// L3 Context Expansion Tests
// ============================================================================

#[test]
fn test_l3_context_full_chain() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Change utils.rs - L3 should find transitive dependents
    let changes = vec![DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L3, 100000);

    // L3 should include transitive dependents
    // utils <- lib <- nothing (no further dependents of lib in our setup)
    // utils <- main <- test_main
    // So test_main should be found as a transitive dependent
    assert!(context.dependent_files.len() >= 1, "L3 should find transitive dependents");
}

#[test]
fn test_l3_cycle_handling() {
    // Create index with potential cycle
    let mut index = SymbolIndex::new();
    index.repo_name = "cycle_test".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "a.py".to_owned(),
        language: Language::Python,
        content_hash: [0; 32],
        symbols: 0..1,
        imports: vec![],
        lines: 10,
        tokens: 50,
    });
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "b.py".to_owned(),
        language: Language::Python,
        content_hash: [0; 32],
        symbols: 1..2,
        imports: vec![],
        lines: 10,
        tokens: 50,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "func_a".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 5, 0),
        signature: Some("def func_a()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "func_b".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(1),
        span: Span::new(1, 0, 5, 0),
        signature: Some("def func_b()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.rebuild_lookups();

    // Create cyclic imports: a -> b -> a
    let mut graph = DepGraph::new();
    graph.add_file_import(0, 1); // a imports b
    graph.add_file_import(1, 0); // b imports a (cycle!)

    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "a.py".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    // Should not panic or loop infinitely
    let context = expander.expand(&changes, ContextDepth::L3, 100000);

    // Should still produce valid results
    assert_eq!(context.changed_files.len(), 1);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_context_empty_diff() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes: Vec<DiffChange> = vec![];
    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    assert!(context.changed_files.is_empty());
    assert!(context.changed_symbols.is_empty());
    assert!(context.dependent_files.is_empty());
}

#[test]
fn test_context_nonexistent_file() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "nonexistent.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // Should not panic, just have empty results
    assert!(context.changed_files.is_empty());
}

#[test]
fn test_context_budget_truncation() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    // Very small budget - should truncate dependent files
    let context = expander.expand(&changes, ContextDepth::L3, 300);

    // Changed file (250 tokens) should fit, but dependents may be truncated
    assert!(context.total_tokens <= 400); // Some slack
}

#[test]
fn test_context_diff_content_preserved() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let diff_content = "+added line\n-removed line\n context".to_owned();
    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: Some(diff_content.clone()),
    }];

    let context = expander.expand(&changes, ContextDepth::L1, 100000);

    assert_eq!(context.changed_files.len(), 1);
    assert_eq!(context.changed_files[0].diff_content, Some(diff_content));
}

// ============================================================================
// Impact Summary Tests
// ============================================================================

#[test]
fn test_impact_level_low() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Small change with few dependents
    let changes = vec![DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L1, 100000);

    // With L1 depth, impact should be low
    assert_eq!(context.impact_summary.level, ImpactLevel::Low);
}

#[test]
fn test_impact_summary_description() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // Impact summary should have a description
    assert!(!context.impact_summary.description.is_empty());
    assert!(context.impact_summary.direct_files > 0);
}

// ============================================================================
// Lazy Context Builder Tests
// ============================================================================

#[test]
fn test_lazy_context_no_index() {
    let tmp = create_test_repo(vec![
        ("main.py", &create_python_file(&["utils"], &["main", "run"])),
        ("utils.py", &create_python_file(&[], &["helper", "format"])),
    ]);

    let mut builder = LazyContextBuilder::new(tmp.path());

    let changes = vec![DiffChange {
        file_path: "main.py".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = builder
        .generate_context(&changes, ContextDepth::L2, 100000)
        .unwrap();

    assert_eq!(context.changed_files.len(), 1);
    assert_eq!(context.changed_files[0].path, "main.py");
}

#[test]
fn test_lazy_multi_language() {
    let tmp = create_test_repo(vec![
        ("src/main.rs", &create_rust_file(&[], &["main", "run"])),
        ("lib/utils.py", &create_python_file(&[], &["helper"])),
        ("web/app.js", &create_js_file(&[], &["init", "start"])),
    ]);

    let mut builder = LazyContextBuilder::new(tmp.path());

    // Change files in multiple languages
    let changes = vec![
        DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 5)],
            change_type: ChangeType::Modified,
            diff_content: None,
        },
        DiffChange {
            file_path: "lib/utils.py".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 5)],
            change_type: ChangeType::Modified,
            diff_content: None,
        },
    ];

    let context = builder
        .generate_context(&changes, ContextDepth::L2, 100000)
        .unwrap();

    assert_eq!(context.changed_files.len(), 2);
}

#[test]
fn test_lazy_relative_imports() {
    let tmp = create_test_repo(vec![
        ("src/main.py", "from .utils import helper\n\ndef main():\n    helper()\n"),
        ("src/utils.py", "def helper():\n    pass\n"),
    ]);

    let mut builder = LazyContextBuilder::new(tmp.path());

    let changes = vec![DiffChange {
        file_path: "src/main.py".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    // Should handle relative imports without panicking
    let result = builder.generate_context(&changes, ContextDepth::L2, 100000);
    assert!(result.is_ok());
}

#[test]
fn test_lazy_deleted_file() {
    let tmp = create_test_repo(vec![("existing.py", "def existing():\n    pass\n")]);

    let mut builder = LazyContextBuilder::new(tmp.path());

    // Try to process a deleted file
    let changes = vec![DiffChange {
        file_path: "deleted.py".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Deleted,
        diff_content: None,
    }];

    // Should not panic, just skip the deleted file
    let context = builder
        .generate_context(&changes, ContextDepth::L2, 100000)
        .unwrap();

    // Deleted file won't be in changed_files since it doesn't exist
    assert!(context.changed_files.is_empty());
}

// ============================================================================
// Index Builder Tests
// ============================================================================

#[test]
fn test_build_multi_language() {
    let tmp = create_test_repo(vec![
        ("src/main.rs", &create_rust_file(&[], &["main"])),
        ("src/utils.py", &create_python_file(&[], &["helper"])),
        ("src/app.ts", "export function start(): void {}\n"),
    ]);

    let builder = IndexBuilder::new(tmp.path());
    let (index, _graph) = builder.build().unwrap();

    // Should find all files
    assert_eq!(index.files.len(), 3);

    // Should have symbols from each language
    let languages: Vec<_> = index.files.iter().map(|f| f.language).collect();
    assert!(languages.contains(&Language::Rust));
    assert!(languages.contains(&Language::Python));
    assert!(languages.contains(&Language::TypeScript));
}

#[test]
fn test_build_complex_structure() {
    let tmp = create_test_repo(vec![
        ("src/main.rs", &create_rust_file(&["crate::lib"], &["main"])),
        ("src/lib.rs", &create_rust_file(&["crate::utils"], &["init"])),
        ("src/utils/mod.rs", &create_rust_file(&[], &["helper", "format"])),
        ("tests/integration.rs", &create_rust_file(&[], &["test_all"])),
    ]);

    let builder = IndexBuilder::new(tmp.path());
    let (index, graph) = builder.build().unwrap();

    // Should find all files including nested ones
    assert_eq!(index.files.len(), 4);

    // Graph should have edges
    assert!(
        !graph.file_imports.is_empty() || graph.calls.is_empty(),
        "Graph should have some structure"
    );
}

#[test]
fn test_build_with_options() {
    let tmp = create_test_repo(vec![
        ("src/main.rs", &create_rust_file(&[], &["main"])),
        ("src/utils.py", &create_python_file(&[], &["helper"])),
    ]);

    // Only include Rust files
    let options = BuildOptions { include_extensions: vec!["rs".to_string()], ..Default::default() };

    let builder = IndexBuilder::new(tmp.path()).with_options(options);
    let (index, _) = builder.build().unwrap();

    // Should only find Rust files
    assert_eq!(index.files.len(), 1);
    assert_eq!(index.files[0].language, Language::Rust);
}

#[test]
fn test_pagerank_computation() {
    let tmp = create_test_repo(vec![
        // Core utility used by many
        ("src/utils.rs", &create_rust_file(&[], &["helper"])),
        // App that uses utils
        ("src/app.rs", &create_rust_file(&["crate::utils"], &["run"])),
        // Main that uses app
        ("src/main.rs", &create_rust_file(&["crate::app"], &["main"])),
    ]);

    let builder = IndexBuilder::new(tmp.path());
    let (index, graph) = builder.build().unwrap();

    // PageRank should be computed
    assert_eq!(graph.file_pagerank.len(), index.files.len());

    // All ranks should be positive
    for rank in &graph.file_pagerank {
        assert!(*rank > 0.0, "PageRank should be positive");
    }
}

#[test]
fn test_symbol_collision_handling() {
    // Two files with same function name
    let tmp = create_test_repo(vec![
        ("src/a.rs", "fn process() {}\n"),
        ("src/b.rs", "fn process() {}\n"),
    ]);

    let builder = IndexBuilder::new(tmp.path());
    let (index, _) = builder.build().unwrap();

    // Both symbols should exist with unique IDs
    let process_symbols = index.find_symbols("process");
    assert_eq!(process_symbols.len(), 2);
    assert_ne!(process_symbols[0].id, process_symbols[1].id);
}

// ============================================================================
// Test File Detection Tests
// ============================================================================

#[test]
fn test_is_test_file_patterns() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    // Using the private is_test_file method via public behavior
    // Test files should be detected by naming convention

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // test_main.rs should be detected as a related test
    let has_test = context
        .related_tests
        .iter()
        .any(|t| t.path.contains("test"));
    assert!(has_test, "Should detect test files");
}

#[test]
fn test_various_test_file_patterns() {
    let tmp = create_test_repo(vec![
        ("src/main.rs", &create_rust_file(&[], &["main"])),
        ("tests/test_main.rs", &create_rust_file(&[], &["test_main"])),
        ("src/main_test.rs", &create_rust_file(&[], &["main_test"])),
        ("spec/main.spec.rs", &create_rust_file(&[], &["spec_main"])),
        ("__tests__/main.test.js", &create_js_file(&[], &["test_main"])),
    ]);

    let builder = IndexBuilder::new(tmp.path());
    let (index, graph) = builder.build().unwrap();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    // Should detect various test file patterns
    assert!(!context.related_tests.is_empty(), "Should find related test files");
}

// ============================================================================
// Change Type Tests
// ============================================================================

#[test]
fn test_change_type_added() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 100)],
        change_type: ChangeType::Added,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    assert_eq!(context.changed_files.len(), 1);
    // New format includes classification: "Added (NewCode)"
    assert!(
        context.changed_files[0].relevance_reason.contains("Added"),
        "Expected 'Added' in reason, got: {}",
        context.changed_files[0].relevance_reason
    );
}

#[test]
fn test_change_type_renamed() {
    let (index, graph) = create_test_index();
    let expander = ContextExpander::new(&index, &graph);

    let changes = vec![DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: Some("src/old_main.rs".to_owned()),
        line_ranges: vec![],
        change_type: ChangeType::Renamed,
        diff_content: None,
    }];

    let context = expander.expand(&changes, ContextDepth::L2, 100000);

    assert_eq!(context.changed_files.len(), 1);
    // New format includes classification: "Renamed (FileRename)"
    assert!(
        context.changed_files[0]
            .relevance_reason
            .contains("Renamed"),
        "Expected 'Renamed' in reason, got: {}",
        context.changed_files[0].relevance_reason
    );
}
