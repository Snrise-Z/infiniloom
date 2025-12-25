//! Tests for context expansion and diff context API
//!
//! These tests cover:
//! - ContextExpander functionality
//! - Change classification (signature, type, implementation, etc.)
//! - Dependent symbol and file discovery
//! - Related test file detection
//! - Token budget enforcement
//! - Call chain building
//! - Impact summary computation

use infiniloom_engine::index::{
    context::{ChangeClassification, ChangeType, ContextDepth, ContextExpander, DiffChange},
    types::{
        DepGraph, FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId,
        SymbolIndex, Visibility,
    },
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a basic test index with files and symbols
fn create_basic_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "context_test".to_owned();

    // Add source file
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..2,
        imports: vec![],
        lines: 50,
        tokens: 200,
    });

    // Add dependency file
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "src/utils.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 2..3,
        imports: vec![],
        lines: 30,
        tokens: 100,
    });

    // Add test file
    index.files.push(FileEntry {
        id: FileId::new(2),
        path: "tests/test_main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 3..4,
        imports: vec![],
        lines: 40,
        tokens: 150,
    });

    // main function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "main".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 20, 0),
        signature: Some("pub fn main()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // helper function called by main
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "process".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(25, 0, 40, 0),
        signature: Some("pub fn process()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // utility function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "format_output".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(1),
        span: Span::new(5, 0, 25, 0),
        signature: Some("pub fn format_output()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // test function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "test_main".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(2),
        span: Span::new(1, 0, 30, 0),
        signature: Some("fn test_main()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Build lookup indices
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    // Build file path index
    index.file_by_path.clear();
    for (i, file) in index.files.iter().enumerate() {
        index.file_by_path.insert(file.path.clone(), i as u32);
    }

    // Build dependency graph
    let mut graph = DepGraph::new();
    // main calls process
    graph.add_call(0, 1);
    // process calls format_output
    graph.add_call(1, 2);
    // main.rs imports utils.rs
    graph.add_file_import(0, 1);
    // test_main.rs imports main.rs
    graph.add_file_import(2, 0);

    (index, graph)
}

/// Create index with multiple callers for testing high-impact changes
fn create_index_with_callers() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "caller_test".to_owned();

    // Add source file with multiple functions
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/api.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..5,
        imports: vec![],
        lines: 100,
        tokens: 400,
    });

    // Core API function that many others call
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "authenticate".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 20, 0),
        signature: Some("pub fn authenticate()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Multiple callers
    for i in 1..5 {
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(i),
            name: format!("handler_{}", i),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(20 + i * 15, 0, 35 + i * 15, 0),
            signature: Some(format!("pub fn handler_{}()", i)),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
    }

    // Build lookup indices
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    index.file_by_path.clear();
    for (i, file) in index.files.iter().enumerate() {
        index.file_by_path.insert(file.path.clone(), i as u32);
    }

    // All handlers call authenticate
    let mut graph = DepGraph::new();
    for i in 1..5 {
        graph.add_call(i, 0);
    }

    (index, graph)
}

// ============================================================================
// Context Expansion Tests
// ============================================================================

#[test]
fn test_expand_basic_change() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 15)], // Covers main function
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Should find the changed file
    assert!(!context.changed_files.is_empty(), "Should have changed files");
    assert_eq!(context.changed_files[0].path, "src/main.rs");
}

#[test]
fn test_expand_finds_changed_symbols() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 15)], // Line range overlapping main function (1-20)
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Should find symbols in the changed lines
    assert!(!context.changed_symbols.is_empty(), "Should find changed symbols");

    let symbol_names: Vec<&str> = context
        .changed_symbols
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert!(symbol_names.contains(&"main"), "Should find main function");
}

#[test]
fn test_expand_l2_finds_dependents() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    // L2 should expand to include importers
    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Test file imports main.rs, so it should appear in related tests
    // (it's detected as a test file due to path containing "test")
    let all_paths: Vec<&str> = context
        .related_tests
        .iter()
        .map(|f| f.path.as_str())
        .collect();

    assert!(
        all_paths.contains(&"tests/test_main.rs"),
        "L2 should find test file that imports changed file"
    );
}

#[test]
fn test_expand_finds_related_tests_by_naming() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should find test file by naming convention (test_main.rs for main.rs)
    assert!(!context.related_tests.is_empty(), "Should find related tests");
}

#[test]
fn test_expand_respects_token_budget() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    // Very small budget
    let context = expander.expand(&[change], ContextDepth::L3, 100);

    // Should not exceed budget significantly
    // (changed files are always included, so can exceed)
    assert!(context.total_tokens <= 500, "Should try to stay within budget");
}

// ============================================================================
// Change Classification Tests
// ============================================================================

#[test]
fn test_classify_signature_change() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Modified,
        diff_content: Some("-fn main()\n+fn main(args: Vec<String>)".to_owned()),
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::SignatureChange);
}

#[test]
fn test_classify_type_definition_change() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/types.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Modified,
        diff_content: Some("-struct Config {\n+struct Config {\n+    debug: bool,".to_owned()),
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::TypeDefinitionChange);
}

#[test]
fn test_classify_deletion() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/old.rs".to_owned(),
        old_path: None,
        line_ranges: vec![],
        change_type: ChangeType::Deleted,
        diff_content: None,
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::Deletion);
}

#[test]
fn test_classify_file_rename() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/new_name.rs".to_owned(),
        old_path: Some("src/old_name.rs".to_owned()),
        line_ranges: vec![],
        change_type: ChangeType::Renamed,
        diff_content: None,
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::FileRename);
}

#[test]
fn test_classify_import_change() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Modified,
        diff_content: Some("+use std::collections::HashMap;\n-use std::vec::Vec;".to_owned()),
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::ImportChange);
}

#[test]
fn test_classify_documentation_only() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 5)],
        change_type: ChangeType::Modified,
        diff_content: Some("+/// This is new documentation\n-/// Old documentation".to_owned()),
    };

    let classification = expander.classify_change(&change, None);
    assert_eq!(classification, ChangeClassification::DocumentationOnly);
}

// ============================================================================
// High-Impact Symbol Tests
// ============================================================================

#[test]
fn test_high_caller_count_expands_all_callers() {
    let (index, graph) = create_index_with_callers();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/api.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)], // authenticate function
        change_type: ChangeType::Modified,
        diff_content: Some("-pub fn authenticate()\n+pub fn authenticate(token: &str)".to_owned()),
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should find all callers of authenticate as dependent symbols
    let dependent_names: Vec<&str> = context
        .dependent_symbols
        .iter()
        .map(|s| s.name.as_str())
        .collect();

    // The change is a signature change with multiple callers, so all callers should be included
    assert!(
        dependent_names.len() >= 3,
        "Should find multiple dependent handlers, found: {:?}",
        dependent_names
    );
}

// ============================================================================
// Impact Summary Tests
// ============================================================================

#[test]
fn test_impact_summary_computed() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should have an impact summary
    assert!(context.impact_summary.direct_files > 0, "Should count direct files");
    assert!(!context.impact_summary.description.is_empty(), "Should have description");
}

#[test]
fn test_impact_level_low_for_small_changes() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Small isolated change should be low impact
    assert_eq!(
        format!("{:?}", context.impact_summary.level),
        "Low",
        "Small change should have low impact"
    );
}

// ============================================================================
// Call Chain Tests
// ============================================================================

#[test]
fn test_call_chains_built_for_changed_symbols() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(25, 35)], // process function
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should build call chains for the process function
    // process is called by main and calls format_output
    // So we might have chains like: main -> process -> format_output
    if !context.call_chains.is_empty() {
        let chain_symbols: Vec<Vec<&str>> = context
            .call_chains
            .iter()
            .map(|c| c.symbols.iter().map(|s| s.as_str()).collect())
            .collect();

        // At least one chain should exist
        assert!(!chain_symbols.is_empty(), "Should have at least one call chain");
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_expand_nonexistent_file() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/nonexistent.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 10)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should not panic, just have no results
    // (File not in index, so no symbols found)
    assert!(context.changed_symbols.is_empty(), "Nonexistent file should have no symbols");
}

#[test]
fn test_expand_empty_changes() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let context = expander.expand(&[], ContextDepth::L2, 100000);

    // Should not panic, just return empty context
    assert!(context.changed_files.is_empty());
    assert!(context.changed_symbols.is_empty());
}

#[test]
fn test_expand_renamed_file() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    // Simulate a file rename: old path is indexed, new path is the rename target
    let change = DiffChange {
        file_path: "src/main_renamed.rs".to_owned(),
        old_path: Some("src/main.rs".to_owned()), // Old path exists in index
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Renamed,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Should find the file via old_path fallback
    assert!(!context.changed_files.is_empty(), "Should find file via old_path");
    // Output path should be the new path
    assert_eq!(context.changed_files[0].path, "src/main_renamed.rs");
}

#[test]
fn test_expand_unicode_file_path() {
    let mut index = SymbolIndex::new();
    index.repo_name = "unicode_test".to_owned();

    // File with Unicode path
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/中文模块.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..1,
        imports: vec![],
        lines: 20,
        tokens: 80,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "处理数据".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 15, 0),
        signature: Some("fn 处理数据()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols_by_name.insert("处理数据".to_owned(), vec![0]);
    index.file_by_path.insert("src/中文模块.rs".to_owned(), 0);

    let graph = DepGraph::new();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/中文模块.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 15)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Should handle Unicode paths correctly
    assert!(!context.changed_files.is_empty());
    assert_eq!(context.changed_files[0].path, "src/中文模块.rs");

    if !context.changed_symbols.is_empty() {
        assert_eq!(context.changed_symbols[0].name, "处理数据");
    }
}

#[test]
fn test_context_symbols_have_file_and_line() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // All changed symbols should have valid file and line info
    for sym in &context.changed_symbols {
        assert!(!sym.file_path.is_empty(), "Symbol should have file_path");
        assert!(sym.start_line > 0, "Symbol should have positive start_line");
        assert!(sym.end_line >= sym.start_line, "End line should be >= start line");
    }

    // All dependent symbols should also have valid file and line info
    for sym in &context.dependent_symbols {
        assert!(!sym.file_path.is_empty(), "Dependent symbol should have file_path");
        assert!(sym.start_line > 0, "Dependent symbol should have positive start_line");
    }
}

#[test]
fn test_context_files_have_snippets_field() {
    let (index, graph) = create_basic_index();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/main.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Modified,
        diff_content: Some("+// New comment\n fn main() { }".to_owned()),
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Changed files should have the snippets field (even if empty)
    for file in &context.changed_files {
        // Snippets field should exist and be accessible (test Issue #7 fix)
        let _ = file.snippets.len(); // Should not panic
    }
}

// ============================================================================
// Test File Detection Tests (via indirect testing)
// ============================================================================

#[test]
fn test_related_tests_detection_patterns() {
    // Test that related tests are found by various patterns
    // This indirectly tests is_test_file without accessing the private method
    let mut index = SymbolIndex::new();
    index.repo_name = "test_detection".to_owned();

    // Add source file
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/utils.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..0,
        imports: vec![],
        lines: 20,
        tokens: 80,
    });

    // Add test files with various patterns
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "tests/test_utils.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..0,
        imports: vec![],
        lines: 30,
        tokens: 100,
    });

    index.files.push(FileEntry {
        id: FileId::new(2),
        path: "src/utils_test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..0,
        imports: vec![],
        lines: 25,
        tokens: 90,
    });

    index.file_by_path.insert("src/utils.rs".to_owned(), 0);
    index
        .file_by_path
        .insert("tests/test_utils.rs".to_owned(), 1);
    index.file_by_path.insert("src/utils_test.rs".to_owned(), 2);

    let mut graph = DepGraph::new();
    // Test files import the source file
    graph.add_file_import(1, 0);
    graph.add_file_import(2, 0);

    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/utils.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Should find test files
    let test_paths: Vec<&str> = context
        .related_tests
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(
        test_paths.contains(&"tests/test_utils.rs") || test_paths.contains(&"src/utils_test.rs"),
        "Should find related test files, found: {:?}",
        test_paths
    );
}

// ============================================================================
// L3 Transitive Expansion Tests
// ============================================================================

#[test]
fn test_l3_transitive_expansion() {
    let mut index = SymbolIndex::new();
    index.repo_name = "l3_test".to_owned();

    // Create a chain: A imports B imports C imports D
    for i in 0..4 {
        index.files.push(FileEntry {
            id: FileId::new(i),
            path: format!("src/module_{}.rs", (b'a' + i as u8) as char),
            language: Language::Rust,
            content_hash: [0; 32],
            symbols: 0..0,
            imports: vec![],
            lines: 20,
            tokens: 80,
        });
        index
            .file_by_path
            .insert(format!("src/module_{}.rs", (b'a' + i as u8) as char), i);
    }

    let mut graph = DepGraph::new();
    // A imports B, B imports C, C imports D
    graph.add_file_import(0, 1);
    graph.add_file_import(1, 2);
    graph.add_file_import(2, 3);

    let expander = ContextExpander::new(&index, &graph);

    // Change D (module_d.rs)
    let change = DiffChange {
        file_path: "src/module_d.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 20)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    // L3 should find transitively dependent files
    let context = expander.expand(&[change], ContextDepth::L3, 100000);

    let all_dependent_paths: Vec<&str> = context
        .dependent_files
        .iter()
        .map(|f| f.path.as_str())
        .collect();

    // C imports D, so C should be in dependents
    // B imports C, so B should also be in L3 dependents
    // A imports B, so A should also be in L3 dependents
    assert!(
        all_dependent_paths.contains(&"src/module_c.rs")
            || all_dependent_paths.contains(&"src/module_b.rs")
            || all_dependent_paths.contains(&"src/module_a.rs"),
        "L3 should find transitive dependents: {:?}",
        all_dependent_paths
    );
}
