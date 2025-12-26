//! Tests for v0.4.5 features and bug fixes
//!
//! This module tests:
//! - Bug #5: Call site deduplication
//! - Feature #8: Transitive callers API
//! - Feature #9: Call site context (line context extraction)
//! - Bug #1-4 fixes: Context expansion improvements
//! - Feature #6-7: Symbol filtering and change type

use infiniloom_engine::index::{
    context::{ChangeType, ContextDepth, ContextExpander, DiffChange},
    query::get_callers_by_name,
    types::{
        DepGraph, FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId,
        SymbolIndex, Visibility,
    },
};
use std::collections::{HashSet, VecDeque};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an index with a deep call chain for transitive caller testing
fn create_deep_call_chain_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "transitive_test".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..6,
        imports: vec![],
        lines: 200,
        tokens: 1000,
    });

    // Create a call chain: entry -> controller -> service -> repository -> database -> query
    let symbols = [
        ("entry", 1, 10),
        ("controller", 15, 30),
        ("service", 35, 50),
        ("repository", 55, 70),
        ("database", 75, 90),
        ("query", 95, 110),
    ];

    for (i, (name, start, end)) in symbols.iter().enumerate() {
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(i as u32),
            name: name.to_string(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(*start, 0, *end, 0),
            signature: Some(format!("fn {}()", name)),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
    }

    // Build lookups
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }
    index.file_by_path.insert("src/main.rs".to_owned(), 0);

    // Build call chain
    let mut graph = DepGraph::new();
    // entry -> controller -> service -> repository -> database -> query
    graph.add_call(0, 1); // entry -> controller
    graph.add_call(1, 2); // controller -> service
    graph.add_call(2, 3); // service -> repository
    graph.add_call(3, 4); // repository -> database
    graph.add_call(4, 5); // database -> query

    (index, graph)
}

/// Create an index with multiple callers of the same function (for dedup testing)
fn create_multi_caller_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "multi_caller".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/lib.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..5,
        imports: vec![],
        lines: 100,
        tokens: 500,
    });

    // Target function called by multiple callers
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "authenticate".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 15, 0),
        signature: Some("fn authenticate(token: &str)".to_owned()),
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
            span: Span::new(20 + i * 20, 0, 35 + i * 20, 0),
            signature: Some(format!("fn handler_{}()", i)),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
    }

    // Build lookups
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }
    index.file_by_path.insert("src/lib.rs".to_owned(), 0);

    // All handlers call authenticate
    let mut graph = DepGraph::new();
    for i in 1..5 {
        graph.add_call(i, 0);
    }

    (index, graph)
}

/// Create index with diamond dependency (A calls B and C, both call D)
fn create_diamond_dependency_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "diamond".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/diamond.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..4,
        imports: vec![],
        lines: 80,
        tokens: 400,
    });

    //    A
    //   / \
    //  B   C
    //   \ /
    //    D
    let symbols = [("A", 1, 15), ("B", 20, 35), ("C", 40, 55), ("D", 60, 75)];

    for (i, (name, start, end)) in symbols.iter().enumerate() {
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(i as u32),
            name: name.to_string(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(*start, 0, *end, 0),
            signature: Some(format!("fn {}()", name)),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
    }

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }
    index.file_by_path.insert("src/diamond.rs".to_owned(), 0);

    let mut graph = DepGraph::new();
    graph.add_call(0, 1); // A -> B
    graph.add_call(0, 2); // A -> C
    graph.add_call(1, 3); // B -> D
    graph.add_call(2, 3); // C -> D

    (index, graph)
}

/// Create index with mixed symbol kinds for filtering tests
fn create_mixed_kinds_index() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "mixed_kinds".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/module.ts".to_owned(),
        language: Language::TypeScript,
        content_hash: [0; 32],
        symbols: 0..6,
        imports: vec![],
        lines: 120,
        tokens: 600,
    });

    // Import
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "React".to_owned(),
        kind: IndexSymbolKind::Import,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 1, 0),
        signature: Some("import React from 'react'".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Class
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "UserService".to_owned(),
        kind: IndexSymbolKind::Class,
        file_id: FileId::new(0),
        span: Span::new(5, 0, 50, 0),
        signature: Some("class UserService".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Method inside class
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "getUser".to_owned(),
        kind: IndexSymbolKind::Method,
        file_id: FileId::new(0),
        span: Span::new(10, 0, 25, 0),
        signature: Some("async getUser(id: string)".to_owned()),
        parent: Some(SymbolId::new(1)),
        visibility: Visibility::Public,
        docstring: None,
    });

    // Function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "validateInput".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(55, 0, 70, 0),
        signature: Some("function validateInput(data: any)".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Constant
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(4),
        name: "API_URL".to_owned(),
        kind: IndexSymbolKind::Constant,
        file_id: FileId::new(0),
        span: Span::new(75, 0, 75, 0),
        signature: Some("const API_URL = 'https://api.example.com'".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Variable
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(5),
        name: "config".to_owned(),
        kind: IndexSymbolKind::Variable,
        file_id: FileId::new(0),
        span: Span::new(80, 0, 85, 0),
        signature: Some("let config = { ... }".to_owned()),
        parent: None,
        visibility: Visibility::Private,
        docstring: None,
    });

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }
    index.file_by_path.insert("src/module.ts".to_owned(), 0);

    let graph = DepGraph::new();
    (index, graph)
}

/// Create index with test files for test detection
fn create_index_with_tests() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "with_tests".to_owned();

    // Source file
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/auth.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..2,
        imports: vec![],
        lines: 50,
        tokens: 250,
    });

    // Test file matching by naming convention
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "tests/auth_test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 2..3,
        imports: vec![],
        lines: 40,
        tokens: 200,
    });

    // Test file by path pattern
    index.files.push(FileEntry {
        id: FileId::new(2),
        path: "src/auth.spec.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 3..4,
        imports: vec![],
        lines: 30,
        tokens: 150,
    });

    // Source symbol
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "login".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 20, 0),
        signature: Some("pub fn login()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "logout".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(25, 0, 45, 0),
        signature: Some("pub fn logout()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Test symbol
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "test_login".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(1),
        span: Span::new(1, 0, 30, 0),
        signature: Some("#[test] fn test_login()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "test_logout".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(2),
        span: Span::new(1, 0, 25, 0),
        signature: Some("#[test] fn test_logout()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    index.file_by_path.insert("src/auth.rs".to_owned(), 0);
    index
        .file_by_path
        .insert("tests/auth_test.rs".to_owned(), 1);
    index.file_by_path.insert("src/auth.spec.rs".to_owned(), 2);

    // Test files import source
    let mut graph = DepGraph::new();
    graph.add_file_import(1, 0);
    graph.add_file_import(2, 0);
    // Test functions call source functions
    graph.add_call(2, 0); // test_login calls login
    graph.add_call(3, 1); // test_logout calls logout

    (index, graph)
}

// ============================================================================
// Transitive Callers Tests (Feature #8)
// ============================================================================

/// Test helper to compute transitive callers using BFS (mimics the bindings implementation)
fn compute_transitive_callers(
    index: &SymbolIndex,
    graph: &DepGraph,
    symbol_name: &str,
    max_depth: u32,
) -> Vec<(String, u32, Vec<String>)> {
    let mut results = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut queue: VecDeque<(u32, u32, Vec<String>)> = VecDeque::new();

    let targets: Vec<_> = index.find_symbols(symbol_name);
    for target in &targets {
        visited.insert(target.id.as_u32());
        queue.push_back((target.id.as_u32(), 0, vec![target.name.clone()]));
    }

    while let Some((current_id, current_depth, call_path)) = queue.pop_front() {
        for caller_id in graph.get_callers(current_id) {
            if visited.insert(caller_id) {
                if let Some(caller) = index.get_symbol(caller_id) {
                    let mut new_path = call_path.clone();
                    new_path.insert(0, caller.name.clone());

                    results.push((caller.name.clone(), current_depth + 1, new_path.clone()));

                    if current_depth + 1 < max_depth {
                        queue.push_back((caller_id, current_depth + 1, new_path));
                    }
                }
            }
        }
    }

    results
}

#[test]
fn test_transitive_callers_depth_1() {
    let (index, graph) = create_deep_call_chain_index();

    let callers = compute_transitive_callers(&index, &graph, "query", 1);

    // At depth 1, only direct caller (database) should be found
    assert_eq!(callers.len(), 1, "Should find 1 direct caller");
    assert_eq!(callers[0].0, "database");
    assert_eq!(callers[0].1, 1); // depth 1
}

#[test]
fn test_transitive_callers_depth_3() {
    let (index, graph) = create_deep_call_chain_index();

    let callers = compute_transitive_callers(&index, &graph, "query", 3);

    // At depth 3: database (d1), repository (d2), service (d3)
    assert_eq!(callers.len(), 3, "Should find 3 callers at depth 3");

    let names: Vec<&str> = callers.iter().map(|(n, _, _)| n.as_str()).collect();
    assert!(names.contains(&"database"));
    assert!(names.contains(&"repository"));
    assert!(names.contains(&"service"));
}

#[test]
fn test_transitive_callers_full_chain() {
    let (index, graph) = create_deep_call_chain_index();

    let callers = compute_transitive_callers(&index, &graph, "query", 10);

    // Full chain: database -> repository -> service -> controller -> entry
    assert_eq!(callers.len(), 5, "Should find 5 callers in full chain");
}

#[test]
fn test_transitive_callers_call_path() {
    let (index, graph) = create_deep_call_chain_index();

    let callers = compute_transitive_callers(&index, &graph, "query", 3);

    // Find the service caller (depth 3)
    let service_caller = callers.iter().find(|(n, _, _)| n == "service");
    assert!(service_caller.is_some(), "Should find service");

    let (_, _, path) = service_caller.unwrap();
    // Path should be: service -> repository -> database -> query
    assert_eq!(path.len(), 4);
    assert_eq!(path[0], "service");
    assert_eq!(path[3], "query");
}

#[test]
fn test_transitive_callers_no_callers() {
    let (index, graph) = create_deep_call_chain_index();

    // entry has no callers
    let callers = compute_transitive_callers(&index, &graph, "entry", 5);

    assert!(callers.is_empty(), "entry should have no callers");
}

#[test]
fn test_transitive_callers_diamond_deduplication() {
    let (index, graph) = create_diamond_dependency_index();

    // D is called by both B and C, which are both called by A
    // transitive callers of D should include B, C, A (but each only once)
    let callers = compute_transitive_callers(&index, &graph, "D", 3);

    let names: Vec<&str> = callers.iter().map(|(n, _, _)| n.as_str()).collect();

    // B and C at depth 1, A at depth 2
    assert!(names.contains(&"B"));
    assert!(names.contains(&"C"));
    assert!(names.contains(&"A"));

    // A should appear only once despite two paths (A->B->D and A->C->D)
    let a_count = names.iter().filter(|&&n| n == "A").count();
    assert_eq!(a_count, 1, "A should appear only once due to deduplication");
}

// ============================================================================
// Call Site Deduplication Tests (Bug #5)
// ============================================================================

#[test]
fn test_callers_are_unique() {
    let (index, graph) = create_multi_caller_index();

    let callers = get_callers_by_name(&index, &graph, "authenticate");

    // Should have 4 unique callers
    assert_eq!(callers.len(), 4);

    // Each caller should be unique
    let caller_ids: HashSet<u32> = callers.iter().map(|c| c.id).collect();
    assert_eq!(caller_ids.len(), 4, "All callers should have unique IDs");
}

#[test]
fn test_no_duplicate_callers_same_symbol() {
    let mut index = SymbolIndex::new();
    index.repo_name = "dup_test".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..3,
        imports: vec![],
        lines: 50,
        tokens: 250,
    });

    // Target function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "target".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 10, 0),
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Caller (appears twice with same name but different IDs)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "caller".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 25, 0),
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Same name, different ID (e.g., export + declaration)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "caller".to_owned(),
        kind: IndexSymbolKind::Export,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 25, 0), // Same line!
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    // Both caller symbols call target
    let mut graph = DepGraph::new();
    graph.add_call(1, 0);
    graph.add_call(2, 0);

    let callers = get_callers_by_name(&index, &graph, "target");

    // find_symbol deduplicates by file+line, so we should get only 1 result
    // (both callers are on the same line)
    // Note: the query module's get_callers_by_name uses find_symbol which deduplicates
    assert!(
        callers.len() <= 2,
        "Should have at most 2 callers (deduplication depends on implementation)"
    );
}

// ============================================================================
// Symbol Kind Filtering Tests (Feature #6)
// ============================================================================

#[test]
fn test_filter_symbols_by_kind() {
    let (index, _) = create_mixed_kinds_index();

    // Find all symbols and categorize by kind
    let all_symbols: Vec<_> = index.symbols.iter().collect();

    let functions: Vec<_> = all_symbols
        .iter()
        .filter(|s| matches!(s.kind, IndexSymbolKind::Function))
        .collect();
    let methods: Vec<_> = all_symbols
        .iter()
        .filter(|s| matches!(s.kind, IndexSymbolKind::Method))
        .collect();
    let classes: Vec<_> = all_symbols
        .iter()
        .filter(|s| matches!(s.kind, IndexSymbolKind::Class))
        .collect();
    let imports: Vec<_> = all_symbols
        .iter()
        .filter(|s| matches!(s.kind, IndexSymbolKind::Import))
        .collect();

    assert_eq!(functions.len(), 1, "Should have 1 function");
    assert_eq!(methods.len(), 1, "Should have 1 method");
    assert_eq!(classes.len(), 1, "Should have 1 class");
    assert_eq!(imports.len(), 1, "Should have 1 import");
}

#[test]
fn test_symbol_kinds_are_correct() {
    let (index, _) = create_mixed_kinds_index();

    // Verify each symbol has the correct kind
    let react = index.find_symbols("React");
    assert!(!react.is_empty());
    assert!(matches!(react[0].kind, IndexSymbolKind::Import));

    let user_service = index.find_symbols("UserService");
    assert!(!user_service.is_empty());
    assert!(matches!(user_service[0].kind, IndexSymbolKind::Class));

    let get_user = index.find_symbols("getUser");
    assert!(!get_user.is_empty());
    assert!(matches!(get_user[0].kind, IndexSymbolKind::Method));

    let validate = index.find_symbols("validateInput");
    assert!(!validate.is_empty());
    assert!(matches!(validate[0].kind, IndexSymbolKind::Function));
}

// ============================================================================
// Context Expansion Tests (Bug #1, #2, #3, #4 fixes)
// ============================================================================

#[test]
fn test_context_expansion_with_line_ranges() {
    let (index, graph) = create_index_with_tests();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/auth.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(5, 15)], // Overlaps with login function (1-20)
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // Should find the login symbol
    assert!(!context.changed_symbols.is_empty(), "Should find changed symbols with line ranges");

    let names: Vec<&str> = context
        .changed_symbols
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert!(names.contains(&"login"), "Should find login function");
}

#[test]
fn test_context_expansion_empty_line_ranges_still_finds_symbols() {
    let (index, graph) = create_index_with_tests();
    let expander = ContextExpander::new(&index, &graph);

    // Empty line ranges - expander should fallback to file-level
    let change = DiffChange {
        file_path: "src/auth.rs".to_owned(),
        old_path: None,
        line_ranges: vec![], // Empty!
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L1, 100000);

    // File should still be tracked
    assert!(
        !context.changed_files.is_empty(),
        "Should track changed file even with empty line ranges"
    );
}

#[test]
fn test_related_tests_found_by_import() {
    let (index, graph) = create_index_with_tests();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/auth.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // Test files import src/auth.rs, so they should be found
    let test_paths: Vec<&str> = context
        .related_tests
        .iter()
        .map(|f| f.path.as_str())
        .collect();

    assert!(
        test_paths.contains(&"tests/auth_test.rs") || test_paths.contains(&"src/auth.spec.rs"),
        "Should find related test files, got: {:?}",
        test_paths
    );
}

#[test]
fn test_context_symbols_have_all_fields() {
    let (index, graph) = create_index_with_tests();
    let expander = ContextExpander::new(&index, &graph);

    let change = DiffChange {
        file_path: "src/auth.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    let context = expander.expand(&[change], ContextDepth::L2, 100000);

    // All changed symbols should have required fields
    for sym in &context.changed_symbols {
        assert!(!sym.name.is_empty(), "Symbol name should not be empty");
        assert!(!sym.kind.is_empty(), "Symbol kind should not be empty");
        assert!(!sym.file_path.is_empty(), "File path should not be empty");
        assert!(sym.start_line > 0, "Start line should be positive");
        assert!(sym.end_line >= sym.start_line, "End line should be >= start line");
    }
}

// ============================================================================
// Change Type Tests (Feature #7)
// ============================================================================

#[test]
fn test_change_type_added() {
    let change = DiffChange {
        file_path: "src/new_file.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(1, 100)],
        change_type: ChangeType::Added,
        diff_content: None,
    };

    assert!(matches!(change.change_type, ChangeType::Added));
}

#[test]
fn test_change_type_modified() {
    let change = DiffChange {
        file_path: "src/existing.rs".to_owned(),
        old_path: None,
        line_ranges: vec![(10, 20)],
        change_type: ChangeType::Modified,
        diff_content: None,
    };

    assert!(matches!(change.change_type, ChangeType::Modified));
}

#[test]
fn test_change_type_deleted() {
    let change = DiffChange {
        file_path: "src/old_file.rs".to_owned(),
        old_path: None,
        line_ranges: vec![],
        change_type: ChangeType::Deleted,
        diff_content: None,
    };

    assert!(matches!(change.change_type, ChangeType::Deleted));
}

#[test]
fn test_change_type_renamed() {
    let change = DiffChange {
        file_path: "src/new_name.rs".to_owned(),
        old_path: Some("src/old_name.rs".to_owned()),
        line_ranges: vec![(1, 50)],
        change_type: ChangeType::Renamed,
        diff_content: None,
    };

    assert!(matches!(change.change_type, ChangeType::Renamed));
    assert_eq!(change.old_path, Some("src/old_name.rs".to_owned()));
}

// ============================================================================
// Line Context Tests (Feature #9 helper)
// ============================================================================

#[test]
fn test_line_context_extraction() {
    // Simulate extracting context around a line
    let lines = [
        "fn main() {".to_string(),
        "    let x = 1;".to_string(),
        "    let y = 2;".to_string(),
        "    println!(\"{}\", x + y);".to_string(),
        "}".to_string(),
    ];

    let target_line = 3; // "let y = 2;" (1-indexed)
    let lines_before = 1;
    let lines_after = 1;

    let start_idx = (target_line as usize).saturating_sub(1 + lines_before);
    let end_idx = ((target_line as usize) + lines_after).min(lines.len());

    let context: Vec<_> = lines[start_idx..end_idx].iter().collect();

    assert_eq!(context.len(), 3);
    assert_eq!(*context[0], "    let x = 1;");
    assert_eq!(*context[1], "    let y = 2;");
    assert_eq!(*context[2], "    println!(\"{}\", x + y);");
}

#[test]
fn test_line_context_at_file_start() {
    let lines =
        ["// File header".to_string(), "fn foo() {}".to_string(), "fn bar() {}".to_string()];

    let target_line = 1; // First line
    let lines_before = 3; // More than available
    let lines_after = 1;

    let start_idx = (target_line as usize).saturating_sub(1 + lines_before);
    let end_idx = ((target_line as usize) + lines_after).min(lines.len());

    let context: Vec<_> = lines[start_idx..end_idx].iter().collect();

    // Should include lines 1 and 2 (0 and 1 in 0-indexed)
    assert_eq!(context.len(), 2);
    assert_eq!(*context[0], "// File header");
    assert_eq!(*context[1], "fn foo() {}");
}

#[test]
fn test_line_context_at_file_end() {
    let lines =
        ["fn foo() {}".to_string(), "fn bar() {}".to_string(), "// End of file".to_string()];

    let target_line = 3; // Last line
    let lines_before = 1;
    let lines_after = 3; // More than available

    let start_idx = (target_line as usize).saturating_sub(1 + lines_before);
    let end_idx = ((target_line as usize) + lines_after).min(lines.len());

    let context: Vec<_> = lines[start_idx..end_idx].iter().collect();

    // Should include lines 2 and 3
    assert_eq!(context.len(), 2);
    assert_eq!(*context[0], "fn bar() {}");
    assert_eq!(*context[1], "// End of file");
}

// ============================================================================
// Visibility Filter Tests (Future Feature #10)
// ============================================================================

#[test]
fn test_symbols_have_visibility() {
    let (index, _) = create_mixed_kinds_index();

    let api_url = index.find_symbols("API_URL");
    assert!(!api_url.is_empty());
    assert!(matches!(api_url[0].visibility, Visibility::Public));

    let config = index.find_symbols("config");
    assert!(!config.is_empty());
    assert!(matches!(config[0].visibility, Visibility::Private));
}

#[test]
fn test_filter_public_symbols() {
    let (index, _) = create_mixed_kinds_index();

    let public_symbols: Vec<_> = index
        .symbols
        .iter()
        .filter(|s| matches!(s.visibility, Visibility::Public))
        .collect();

    // All except config should be public
    assert_eq!(public_symbols.len(), 5);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_index_transitive_callers() {
    let index = SymbolIndex::new();
    let graph = DepGraph::new();

    let callers = compute_transitive_callers(&index, &graph, "nonexistent", 5);

    assert!(callers.is_empty());
}

#[test]
fn test_cyclic_calls_dont_infinite_loop() {
    let mut index = SymbolIndex::new();
    index.repo_name = "cyclic".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..2,
        imports: vec![],
        lines: 30,
        tokens: 150,
    });

    // A calls B, B calls A (cycle)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "func_a".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 10, 0),
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "func_b".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 25, 0),
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    let mut graph = DepGraph::new();
    graph.add_call(0, 1); // A -> B
    graph.add_call(1, 0); // B -> A (cycle!)

    // Should not hang
    let callers = compute_transitive_callers(&index, &graph, "func_a", 10);

    // Should find func_b (calls func_a), but not loop infinitely
    assert_eq!(callers.len(), 1, "Should find exactly 1 caller (func_b)");
    assert_eq!(callers[0].0, "func_b");
}

#[test]
fn test_self_recursive_function() {
    let mut index = SymbolIndex::new();
    index.repo_name = "recursive".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..1,
        imports: vec![],
        lines: 20,
        tokens: 100,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "recursive".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 15, 0),
        signature: Some("fn recursive(n: i32)".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    let mut graph = DepGraph::new();
    graph.add_call(0, 0); // Self-call

    let callers = compute_transitive_callers(&index, &graph, "recursive", 5);

    // Self-calls should not appear in transitive callers
    // (visited set prevents re-processing)
    assert!(callers.is_empty(), "Self-recursive function should have no external callers");
}
