//! Tests for query deduplication and call graph API
//!
//! These tests target issues like:
//! - Duplicate symbols returned from findSymbol (Issue #3)
//! - Empty getReferences results (Issue #4)
//! - Call graph edge correctness
//! - Symbol lookup edge cases

use infiniloom_engine::index::{
    query::{
        find_symbol, get_call_graph, get_call_graph_filtered, get_callees_by_name,
        get_callers_by_name, get_references_by_name,
    },
    types::{
        DepGraph, FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId,
        SymbolIndex, Visibility,
    },
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test index with potential duplicate scenarios
fn create_index_with_duplicates() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "dedup_test".to_owned();

    // Add test file
    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/module.ts".to_owned(),
        language: Language::TypeScript,
        content_hash: [0; 32],
        symbols: 0..4,
        imports: vec![],
        lines: 50,
        tokens: 250,
    });

    // Symbol that appears as both export and declaration (same line)
    // This simulates: export function chat() { ... }
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "chat".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(10, 0, 20, 0),
        signature: Some("export function chat()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Duplicate entry with same name and line (simulating export + declaration)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "chat".to_owned(),
        kind: IndexSymbolKind::Export,
        file_id: FileId::new(0),
        span: Span::new(10, 0, 20, 0), // Same line!
        signature: Some("export function chat".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Another symbol with same name in different file
    index.files.push(FileEntry {
        id: FileId::new(1),
        path: "src/other.ts".to_owned(),
        language: Language::TypeScript,
        content_hash: [0; 32],
        symbols: 2..3,
        imports: vec![],
        lines: 30,
        tokens: 150,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "chat".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(1),
        span: Span::new(5, 0, 15, 0), // Different line, different file
        signature: Some("function chat()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Unique symbol
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "sendMessage".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(25, 0, 35, 0),
        signature: Some("function sendMessage()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Rebuild lookups - this is critical for find_symbols to work
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    let graph = DepGraph::new();

    (index, graph)
}

/// Create index with complex call relationships
fn create_index_with_calls() -> (SymbolIndex, DepGraph) {
    let mut index = SymbolIndex::new();
    index.repo_name = "call_test".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "src/main.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..4,
        imports: vec![],
        lines: 100,
        tokens: 500,
    });

    // main -> process -> helper -> utility
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "main".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 10, 0),
        signature: Some("fn main()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "process".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 30, 0),
        signature: Some("fn process()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "helper".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(35, 0, 50, 0),
        signature: Some("fn helper()".to_owned()),
        parent: None,
        visibility: Visibility::Private,
        docstring: None,
    });

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(3),
        name: "utility".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(55, 0, 70, 0),
        signature: Some("fn utility()".to_owned()),
        parent: None,
        visibility: Visibility::Private,
        docstring: None,
    });

    // Rebuild lookups
    index.symbols_by_name.clear();
    for (i, sym) in index.symbols.iter().enumerate() {
        index
            .symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(i as u32);
    }

    // Build call graph: main -> process -> helper -> utility
    let mut graph = DepGraph::new();
    graph.add_call(0, 1); // main calls process
    graph.add_call(1, 2); // process calls helper
    graph.add_call(2, 3); // helper calls utility

    // Also add some references (imports)
    graph.add_symbol_ref(0, 1); // main references process

    (index, graph)
}

// ============================================================================
// Deduplication Tests (Issue #3)
// ============================================================================

#[test]
fn test_find_symbol_deduplicates_same_line() {
    let (index, _graph) = create_index_with_duplicates();

    // Find symbol "chat" - should deduplicate entries on same line
    let results = find_symbol(&index, "chat");

    // Should only return 2 unique symbols (one in module.ts, one in other.ts)
    // NOT 3 (which would include both export and declaration from module.ts)
    assert_eq!(
        results.len(),
        2,
        "Should deduplicate symbols on same file+line, got: {:?}",
        results
            .iter()
            .map(|s| (&s.file, s.line))
            .collect::<Vec<_>>()
    );

    // Verify both files are represented
    let files: Vec<&str> = results.iter().map(|s| s.file.as_str()).collect();
    assert!(files.contains(&"src/module.ts"));
    assert!(files.contains(&"src/other.ts"));
}

#[test]
fn test_find_symbol_keeps_different_lines() {
    let (index, _graph) = create_index_with_duplicates();

    // Symbols on different lines should NOT be deduplicated
    let chat_results = find_symbol(&index, "chat");

    // module.ts line 10 and other.ts line 5 are different, should both appear
    let lines: Vec<(String, u32)> = chat_results
        .iter()
        .map(|s| (s.file.clone(), s.line))
        .collect();

    assert!(lines.contains(&("src/module.ts".to_owned(), 10)));
    assert!(lines.contains(&("src/other.ts".to_owned(), 5)));
}

#[test]
fn test_find_symbol_unique_results() {
    let (index, _graph) = create_index_with_duplicates();

    // Find unique symbol
    let results = find_symbol(&index, "sendMessage");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "sendMessage");
    assert_eq!(results[0].file, "src/module.ts");
}

#[test]
fn test_find_symbol_nonexistent() {
    let (index, _graph) = create_index_with_duplicates();

    let results = find_symbol(&index, "nonexistent");

    assert!(results.is_empty());
}

#[test]
fn test_find_symbol_case_sensitive() {
    let (index, _graph) = create_index_with_duplicates();

    // Should be case sensitive
    let results_lower = find_symbol(&index, "chat");
    let results_upper = find_symbol(&index, "Chat");
    let results_mixed = find_symbol(&index, "CHAT");

    assert!(!results_lower.is_empty());
    assert!(results_upper.is_empty());
    assert!(results_mixed.is_empty());
}

// ============================================================================
// Caller/Callee Tests
// ============================================================================

#[test]
fn test_get_callers_direct() {
    let (index, graph) = create_index_with_calls();

    // process is called by main
    let callers = get_callers_by_name(&index, &graph, "process");

    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].name, "main");
}

#[test]
fn test_get_callers_none() {
    let (index, graph) = create_index_with_calls();

    // main has no callers
    let callers = get_callers_by_name(&index, &graph, "main");

    assert!(callers.is_empty());
}

#[test]
fn test_get_callees_direct() {
    let (index, graph) = create_index_with_calls();

    // main calls process
    let callees = get_callees_by_name(&index, &graph, "main");

    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].name, "process");
}

#[test]
fn test_get_callees_leaf() {
    let (index, graph) = create_index_with_calls();

    // utility calls nothing
    let callees = get_callees_by_name(&index, &graph, "utility");

    assert!(callees.is_empty());
}

#[test]
fn test_get_callers_deduplicates() {
    // Create scenario where same symbol is called multiple times
    let mut index = SymbolIndex::new();
    index.repo_name = "caller_dedup".to_owned();

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

    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "caller".to_owned(),
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
        name: "target".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 20, 0),
        signature: None,
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Same name, different location
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "target".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(25, 0, 30, 0),
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
    graph.add_call(0, 1); // caller -> target (id 1)
    graph.add_call(0, 2); // caller -> target (id 2) - both have same name

    // Getting callers of "target" should only return "caller" once
    let callers = get_callers_by_name(&index, &graph, "target");

    assert_eq!(callers.len(), 1, "Should deduplicate callers with same ID");
}

// ============================================================================
// References Tests (Issue #4)
// ============================================================================

#[test]
fn test_get_references_includes_callers() {
    let (index, graph) = create_index_with_calls();

    // process is called by main
    let refs = get_references_by_name(&index, &graph, "process");

    // Should include the caller
    assert!(!refs.is_empty(), "Should find references");

    let caller_names: Vec<&str> = refs.iter().map(|r| r.symbol.name.as_str()).collect();
    assert!(caller_names.contains(&"main"), "Should find main as a caller reference");
}

#[test]
fn test_get_references_has_file_and_line() {
    let (index, graph) = create_index_with_calls();

    let refs = get_references_by_name(&index, &graph, "process");

    // All references should have valid file and line
    for r in &refs {
        assert!(!r.symbol.file.is_empty(), "File should not be empty");
        assert!(r.symbol.line > 0, "Line should be positive");
    }
}

#[test]
fn test_get_references_includes_kind() {
    let (index, graph) = create_index_with_calls();

    let refs = get_references_by_name(&index, &graph, "process");

    // All references should have a kind
    for r in &refs {
        assert!(!r.kind.is_empty(), "Kind should not be empty");
        assert!(
            r.kind == "call" || r.kind == "reference",
            "Kind should be 'call' or 'reference', got: {}",
            r.kind
        );
    }
}

#[test]
fn test_get_references_nonexistent() {
    let (index, graph) = create_index_with_calls();

    let refs = get_references_by_name(&index, &graph, "nonexistent");

    assert!(refs.is_empty());
}

// ============================================================================
// Call Graph Tests
// ============================================================================

#[test]
fn test_get_call_graph_nodes() {
    let (index, graph) = create_index_with_calls();

    let call_graph = get_call_graph(&index, &graph);

    // Should have all 4 symbols as nodes
    assert_eq!(call_graph.nodes.len(), 4);

    let node_names: Vec<&str> = call_graph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(node_names.contains(&"main"));
    assert!(node_names.contains(&"process"));
    assert!(node_names.contains(&"helper"));
    assert!(node_names.contains(&"utility"));
}

#[test]
fn test_get_call_graph_edges() {
    let (index, graph) = create_index_with_calls();

    let call_graph = get_call_graph(&index, &graph);

    // Should have 3 edges: main->process, process->helper, helper->utility
    assert_eq!(call_graph.edges.len(), 3);

    // Verify edge relationships
    let edge_pairs: Vec<(&str, &str)> = call_graph
        .edges
        .iter()
        .map(|e| (e.caller.as_str(), e.callee.as_str()))
        .collect();

    assert!(edge_pairs.contains(&("main", "process")));
    assert!(edge_pairs.contains(&("process", "helper")));
    assert!(edge_pairs.contains(&("helper", "utility")));
}

#[test]
fn test_get_call_graph_stats() {
    let (index, graph) = create_index_with_calls();

    let call_graph = get_call_graph(&index, &graph);

    assert_eq!(call_graph.stats.total_symbols, 4);
    assert_eq!(call_graph.stats.total_calls, 3);
    assert_eq!(call_graph.stats.functions, 4); // All are functions
    assert_eq!(call_graph.stats.classes, 0);
}

#[test]
fn test_get_call_graph_filtered_nodes() {
    let (index, graph) = create_index_with_calls();

    // Limit to 2 nodes
    let call_graph = get_call_graph_filtered(&index, &graph, Some(2), None);

    assert_eq!(call_graph.nodes.len(), 2);
}

#[test]
fn test_get_call_graph_filtered_edges() {
    let (index, graph) = create_index_with_calls();

    // Limit to 1 edge
    let call_graph = get_call_graph_filtered(&index, &graph, None, Some(1));

    assert_eq!(call_graph.edges.len(), 1);
}

#[test]
fn test_get_call_graph_empty() {
    let index = SymbolIndex::new();
    let graph = DepGraph::new();

    let call_graph = get_call_graph(&index, &graph);

    assert!(call_graph.nodes.is_empty());
    assert!(call_graph.edges.is_empty());
    assert_eq!(call_graph.stats.total_symbols, 0);
    assert_eq!(call_graph.stats.total_calls, 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_symbol_with_empty_name() {
    let mut index = SymbolIndex::new();
    index.repo_name = "empty_name".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..1,
        imports: vec![],
        lines: 10,
        tokens: 50,
    });

    // Symbol with empty name (edge case)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 5, 0),
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

    // Should not panic
    let results = find_symbol(&index, "");
    assert_eq!(results.len(), 1);
}

#[test]
fn test_symbol_with_unicode_name() {
    let mut index = SymbolIndex::new();
    index.repo_name = "unicode_name".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.py".to_owned(),
        language: Language::Python,
        content_hash: [0; 32],
        symbols: 0..3,
        imports: vec![],
        lines: 30,
        tokens: 150,
    });

    // Chinese function name
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "处理数据".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 10, 0),
        signature: Some("def 处理数据()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Russian function name
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(1),
        name: "обработка".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(15, 0, 20, 0),
        signature: Some("def обработка()".to_owned()),
        parent: None,
        visibility: Visibility::Public,
        docstring: None,
    });

    // Emoji name (unlikely but possible)
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(2),
        name: "func_🎉".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(25, 0, 30, 0),
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

    // All should be findable
    let chinese = find_symbol(&index, "处理数据");
    let russian = find_symbol(&index, "обработка");
    let emoji = find_symbol(&index, "func_🎉");

    assert_eq!(chinese.len(), 1);
    assert_eq!(russian.len(), 1);
    assert_eq!(emoji.len(), 1);
}

#[test]
fn test_very_large_call_graph() {
    let mut index = SymbolIndex::new();
    index.repo_name = "large_graph".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..100,
        imports: vec![],
        lines: 1000,
        tokens: 5000,
    });

    // Create 100 symbols
    for i in 0..100 {
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(i),
            name: format!("func_{}", i),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(i * 10 + 1, 0, (i + 1) * 10, 0),
            signature: Some(format!("fn func_{}()", i)),
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

    // Create a dense call graph (each function calls the next 3)
    let mut graph = DepGraph::new();
    for i in 0..97 {
        graph.add_call(i, i + 1);
        graph.add_call(i, i + 2);
        graph.add_call(i, i + 3);
    }

    // Should handle large graph without panic
    let call_graph = get_call_graph(&index, &graph);

    assert_eq!(call_graph.nodes.len(), 100);
    assert_eq!(call_graph.edges.len(), 97 * 3); // 3 edges per function (except last 3)
}

#[test]
fn test_call_graph_with_self_reference() {
    let mut index = SymbolIndex::new();
    index.repo_name = "self_ref".to_owned();

    index.files.push(FileEntry {
        id: FileId::new(0),
        path: "test.rs".to_owned(),
        language: Language::Rust,
        content_hash: [0; 32],
        symbols: 0..1,
        imports: vec![],
        lines: 10,
        tokens: 50,
    });

    // Recursive function
    index.symbols.push(IndexSymbol {
        id: SymbolId::new(0),
        name: "recursive".to_owned(),
        kind: IndexSymbolKind::Function,
        file_id: FileId::new(0),
        span: Span::new(1, 0, 10, 0),
        signature: Some("fn recursive()".to_owned()),
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

    // Self-call
    let mut graph = DepGraph::new();
    graph.add_call(0, 0); // recursive calls itself

    let call_graph = get_call_graph(&index, &graph);

    assert_eq!(call_graph.nodes.len(), 1);
    assert_eq!(call_graph.edges.len(), 1);
    assert_eq!(call_graph.edges[0].caller, "recursive");
    assert_eq!(call_graph.edges[0].callee, "recursive");
}
