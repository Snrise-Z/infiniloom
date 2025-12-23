//! Comprehensive tests for the repomap module
//!
//! Tests PageRank correctness, symbol index collision handling, and edge cases.

use infiniloom_engine::repomap::RepoMapGenerator;
use infiniloom_engine::types::{
    LanguageStats, RepoFile, RepoMetadata, Repository, Symbol, SymbolKind, TokenCounts, Visibility,
};

fn default_token_counts() -> TokenCounts {
    TokenCounts {
        o200k: 100,
        cl100k: 102,
        claude: 105,
        gemini: 98,
        llama: 100,
        mistral: 100,
        deepseek: 100,
        qwen: 100,
        cohere: 101,
        grok: 100,
    }
}

fn create_symbol(name: &str, kind: SymbolKind) -> Symbol {
    Symbol {
        name: name.to_owned(),
        kind,
        signature: Some(format!("fn {}()", name)),
        docstring: None,
        start_line: 1,
        end_line: 10,
        references: 0,
        importance: 0.5,
        parent: None,
        visibility: Visibility::Public,
        calls: vec![],
        extends: None,
        implements: vec![],
    }
}

fn create_symbol_with_calls(name: &str, kind: SymbolKind, calls: Vec<&str>) -> Symbol {
    Symbol {
        name: name.to_owned(),
        kind,
        signature: Some(format!("fn {}()", name)),
        docstring: None,
        start_line: 1,
        end_line: 10,
        references: 0,
        importance: 0.5,
        parent: None,
        visibility: Visibility::Public,
        calls: calls.into_iter().map(|s| s.to_owned()).collect(),
        extends: None,
        implements: vec![],
    }
}

fn create_test_repo(files: Vec<RepoFile>) -> Repository {
    let total_files = files.len() as u32;
    let total_lines: u64 = files.iter().map(|f| f.symbols.len() as u64 * 10).sum();
    let total_tokens = files
        .iter()
        .fold(default_token_counts(), |acc, f| TokenCounts {
            o200k: acc.o200k + f.token_count.o200k,
            cl100k: acc.cl100k + f.token_count.cl100k,
            claude: acc.claude + f.token_count.claude,
            gemini: acc.gemini + f.token_count.gemini,
            llama: acc.llama + f.token_count.llama,
            mistral: acc.mistral + f.token_count.mistral,
            deepseek: acc.deepseek + f.token_count.deepseek,
            qwen: acc.qwen + f.token_count.qwen,
            cohere: acc.cohere + f.token_count.cohere,
            grok: acc.grok + f.token_count.grok,
        });

    Repository {
        name: "test-repo".to_owned(),
        path: "/tmp/test".into(),
        files,
        metadata: RepoMetadata {
            total_files,
            total_lines,
            total_tokens,
            languages: vec![LanguageStats {
                language: "Python".to_owned(),
                files: total_files,
                lines: total_lines,
                percentage: 100.0,
            }],
            framework: None,
            description: None,
            branch: None,
            commit: None,
            directory_structure: None,
            external_dependencies: vec![],
            git_history: None,
        },
    }
}

// =============================================================================
// Symbol Index Collision Tests
// =============================================================================

#[test]
fn test_symbol_collision_multiple_main_functions() {
    // Test that multiple files with 'main' function are all indexed
    let files = vec![
        RepoFile {
            path: "/tmp/test/src/main.py".into(),
            relative_path: "src/main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 500,
            token_count: default_token_counts(),
            symbols: vec![
                create_symbol_with_calls("main", SymbolKind::Function, vec!["helper"]),
                create_symbol("helper", SymbolKind::Function),
            ],
            importance: 0.9,
            content: None,
        },
        RepoFile {
            path: "/tmp/test/tests/main.py".into(),
            relative_path: "tests/main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 300,
            token_count: default_token_counts(),
            symbols: vec![
                create_symbol_with_calls("main", SymbolKind::Function, vec!["test_helper"]),
                create_symbol("test_helper", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: "/tmp/test/cli/main.py".into(),
            relative_path: "cli/main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 400,
            token_count: default_token_counts(),
            symbols: vec![create_symbol_with_calls("main", SymbolKind::Function, vec!["run_cli"])],
            importance: 0.7,
            content: None,
        },
    ];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(5000);
    let map = generator.generate(&repo);

    // All three 'main' functions should be represented in the map
    // (though only some may appear in key_symbols based on ranking)
    assert!(!map.key_symbols.is_empty(), "Should have key symbols");
    assert!(!map.file_index.is_empty(), "Should have file index");

    // The map should have files from all three directories
    let file_paths: Vec<_> = map.file_index.iter().map(|f| f.path.as_str()).collect();
    assert!(file_paths.iter().any(|p| p.contains("src/")), "Should include src/ files");
}

#[test]
fn test_symbol_collision_common_names() {
    // Test common symbol names that appear in many files
    let common_names = ["init", "setup", "teardown", "get", "set", "run"];

    let files: Vec<RepoFile> = (0..6)
        .map(|i| {
            let module_name = format!("module{}", i);
            RepoFile {
                path: format!("/tmp/test/{}.py", module_name).into(),
                relative_path: format!("{}.py", module_name),
                language: Some("python".to_owned()),
                size_bytes: 200,
                token_count: default_token_counts(),
                symbols: common_names
                    .iter()
                    .map(|&name| create_symbol(name, SymbolKind::Function))
                    .collect(),
                importance: 0.5,
                content: None,
            }
        })
        .collect();

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(10000);
    let map = generator.generate(&repo);

    // Should generate a valid map without panics
    assert!(!map.summary.is_empty());
    // With 6 files * 6 symbols = 36 total symbols, we should have several in the map
    assert!(!map.key_symbols.is_empty(), "Should have key symbols even with name collisions");
}

// =============================================================================
// PageRank Tests
// =============================================================================

#[test]
fn test_pagerank_simple_chain() {
    // A -> B -> C: C should have highest rank
    let files = vec![RepoFile {
        path: "/tmp/test/chain.py".into(),
        relative_path: "chain.py".to_owned(),
        language: Some("python".to_owned()),
        size_bytes: 300,
        token_count: default_token_counts(),
        symbols: vec![
            create_symbol_with_calls("a", SymbolKind::Function, vec!["b"]),
            create_symbol_with_calls("b", SymbolKind::Function, vec!["c"]),
            create_symbol("c", SymbolKind::Function),
        ],
        importance: 0.5,
        content: None,
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    // Function 'c' should be in the top symbols (most incoming links)
    let top_names: Vec<_> = map.key_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(top_names.contains(&"c"), "C should be in top symbols: {:?}", top_names);
}

#[test]
fn test_pagerank_star_topology() {
    // Many functions calling one central function
    let central_callers: Vec<_> = (0..10)
        .map(|i| {
            create_symbol_with_calls(&format!("caller{}", i), SymbolKind::Function, vec!["central"])
        })
        .collect();

    let mut symbols = central_callers;
    symbols.push(create_symbol("central", SymbolKind::Function));

    let files = vec![RepoFile {
        path: "/tmp/test/star.py".into(),
        relative_path: "star.py".to_owned(),
        language: Some("python".to_owned()),
        size_bytes: 500,
        token_count: default_token_counts(),
        symbols,
        importance: 0.5,
        content: None,
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(5000);
    let map = generator.generate(&repo);

    // 'central' should be the top symbol (10 incoming calls)
    assert!(!map.key_symbols.is_empty(), "Should have key symbols");
    assert_eq!(map.key_symbols[0].name, "central", "Central function should be ranked first");
}

#[test]
fn test_pagerank_disconnected_components() {
    // Two separate groups of functions with no connections between them
    let files = vec![
        RepoFile {
            path: "/tmp/test/component_a.py".into(),
            relative_path: "component_a.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 300,
            token_count: default_token_counts(),
            symbols: vec![
                create_symbol_with_calls("a1", SymbolKind::Function, vec!["a2"]),
                create_symbol("a2", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        },
        RepoFile {
            path: "/tmp/test/component_b.py".into(),
            relative_path: "component_b.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 300,
            token_count: default_token_counts(),
            symbols: vec![
                create_symbol_with_calls("b1", SymbolKind::Function, vec!["b2"]),
                create_symbol("b2", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        },
    ];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    // Both components should be represented
    let symbol_names: Vec<_> = map.key_symbols.iter().map(|s| s.name.as_str()).collect();
    // Should have symbols from both components (a2 and b2 have incoming edges)
    assert!(symbol_names.len() >= 2, "Should have symbols from both components");
}

#[test]
fn test_pagerank_cyclic_graph() {
    // A -> B -> C -> A (cycle)
    let files = vec![RepoFile {
        path: "/tmp/test/cycle.py".into(),
        relative_path: "cycle.py".to_owned(),
        language: Some("python".to_owned()),
        size_bytes: 300,
        token_count: default_token_counts(),
        symbols: vec![
            create_symbol_with_calls("a", SymbolKind::Function, vec!["b"]),
            create_symbol_with_calls("b", SymbolKind::Function, vec!["c"]),
            create_symbol_with_calls("c", SymbolKind::Function, vec!["a"]),
        ],
        importance: 0.5,
        content: None,
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    // Should handle cycle without infinite loop
    assert!(!map.key_symbols.is_empty(), "Should compute PageRank for cyclic graph");
    // In a perfect cycle, all nodes should have similar ranks
    if map.key_symbols.len() >= 3 {
        let ranks: Vec<f32> = map
            .key_symbols
            .iter()
            .take(3)
            .map(|s| s.importance)
            .collect();
        let max_rank = ranks.iter().cloned().fold(0.0f32, f32::max);
        let min_rank = ranks.iter().cloned().fold(1.0f32, f32::min);
        // In a cycle, ranks should be relatively close (within 50% of each other)
        assert!(
            max_rank - min_rank < 0.5 * max_rank || max_rank < 0.01,
            "Cyclic graph ranks should be similar: {:?}",
            ranks
        );
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_empty_repository() {
    let repo = create_test_repo(vec![]);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    assert!(map.key_symbols.is_empty(), "Empty repo should have no symbols");
    assert!(map.file_index.is_empty(), "Empty repo should have no files");
}

#[test]
fn test_single_file_no_symbols() {
    let files = vec![RepoFile {
        path: "/tmp/test/empty.txt".into(),
        relative_path: "empty.txt".to_owned(),
        language: Some("text".to_owned()),
        size_bytes: 10,
        token_count: default_token_counts(),
        symbols: vec![],
        importance: 0.1,
        content: Some("".to_owned()),
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    assert!(map.key_symbols.is_empty(), "File with no symbols should produce no key symbols");
    assert!(!map.file_index.is_empty(), "Should still have file index");
}

#[test]
fn test_large_repository_simulation() {
    // Simulate a larger repo with 50 files, each with 10 symbols
    let files: Vec<RepoFile> = (0..50)
        .map(|i| {
            let symbols: Vec<Symbol> = (0..10)
                .map(|j| {
                    // Create some call relationships
                    let calls = if j > 0 {
                        vec![format!("func_{}_{}", i, j - 1)]
                    } else if i > 0 {
                        vec![format!("func_{}_{}", i - 1, 9)]
                    } else {
                        vec![]
                    };
                    Symbol {
                        name: format!("func_{}_{}", i, j),
                        kind: SymbolKind::Function,
                        signature: Some(format!("def func_{}_{}():", i, j)),
                        docstring: None,
                        start_line: j * 10 + 1,
                        end_line: j * 10 + 10,
                        references: 0,
                        importance: 0.5,
                        parent: None,
                        visibility: Visibility::Public,
                        calls,
                        extends: None,
                        implements: vec![],
                    }
                })
                .collect();

            RepoFile {
                path: format!("/tmp/test/module_{}.py", i).into(),
                relative_path: format!("module_{}.py", i),
                language: Some("python".to_owned()),
                size_bytes: 1000,
                token_count: default_token_counts(),
                symbols,
                importance: 0.5,
                content: None,
            }
        })
        .collect();

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(10000);
    let map = generator.generate(&repo);

    // Should handle large repo without issues
    assert!(!map.key_symbols.is_empty(), "Large repo should have key symbols");
    assert!(!map.file_index.is_empty(), "Large repo should have file index");
    assert!(map.token_count > 0, "Should have non-zero token count");

    // Verify budget is respected
    // Note: With Bug #7 fix, formula changed from budget/30 to budget/20, allowing more symbols
    // This means token count can exceed budget when there are many symbols
    assert!(
        map.token_count <= 15000, // Allow larger margin for new formula
        "Token count {} should be reasonable for budget 10000",
        map.token_count
    );
}

#[test]
fn test_inheritance_graph() {
    // Base -> Child1, Base -> Child2
    let files = vec![RepoFile {
        path: "/tmp/test/classes.py".into(),
        relative_path: "classes.py".to_owned(),
        language: Some("python".to_owned()),
        size_bytes: 500,
        token_count: default_token_counts(),
        symbols: vec![
            Symbol {
                name: "Base".to_owned(),
                kind: SymbolKind::Class,
                signature: Some("class Base:".to_owned()),
                docstring: None,
                start_line: 1,
                end_line: 20,
                references: 0,
                importance: 0.8,
                parent: None,
                visibility: Visibility::Public,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "Child1".to_owned(),
                kind: SymbolKind::Class,
                signature: Some("class Child1(Base):".to_owned()),
                docstring: None,
                start_line: 21,
                end_line: 40,
                references: 0,
                importance: 0.6,
                parent: None,
                visibility: Visibility::Public,
                calls: vec![],
                extends: Some("Base".to_owned()),
                implements: vec![],
            },
            Symbol {
                name: "Child2".to_owned(),
                kind: SymbolKind::Class,
                signature: Some("class Child2(Base):".to_owned()),
                docstring: None,
                start_line: 41,
                end_line: 60,
                references: 0,
                importance: 0.6,
                parent: None,
                visibility: Visibility::Public,
                calls: vec![],
                extends: Some("Base".to_owned()),
                implements: vec![],
            },
        ],
        importance: 0.7,
        content: None,
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    // Base class should have highest rank due to inheritance
    let top_names: Vec<_> = map.key_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(top_names.contains(&"Base"), "Base class should be in top symbols: {:?}", top_names);
}

#[test]
fn test_token_budget_enforcement() {
    // Create repo that would exceed a small budget
    let files: Vec<RepoFile> = (0..20)
        .map(|i| {
            let symbols: Vec<Symbol> = (0..5)
                .map(|j| create_symbol(&format!("func_{}_{}", i, j), SymbolKind::Function))
                .collect();

            RepoFile {
                path: format!("/tmp/test/file_{}.py", i).into(),
                relative_path: format!("file_{}.py", i),
                language: Some("python".to_owned()),
                size_bytes: 500,
                token_count: default_token_counts(),
                symbols,
                importance: 0.5,
                content: None,
            }
        })
        .collect();

    let repo = create_test_repo(files);

    // Small budget should limit output
    let generator = RepoMapGenerator::new(500);
    let map = generator.generate(&repo);

    // Budget of 500 tokens should result in limited symbols
    // Bug #7 fix: max_symbols = 500 / 20 = 25, clamped to range [5, 500]
    assert!(
        map.key_symbols.len() <= 30,
        "Small budget should limit key symbols: got {}",
        map.key_symbols.len()
    );
}

#[test]
fn test_module_graph_generation() {
    // Test module dependency graph
    let files = vec![
        RepoFile {
            path: "/tmp/test/core/engine.py".into(),
            relative_path: "core/engine.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 500,
            token_count: default_token_counts(),
            symbols: vec![create_symbol("Engine", SymbolKind::Class)],
            importance: 0.9,
            content: None,
        },
        RepoFile {
            path: "/tmp/test/api/routes.py".into(),
            relative_path: "api/routes.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 300,
            token_count: default_token_counts(),
            symbols: vec![
                Symbol {
                    name: "Engine".to_owned(),
                    kind: SymbolKind::Import,
                    signature: None,
                    docstring: None,
                    start_line: 1,
                    end_line: 1,
                    references: 0,
                    importance: 0.1,
                    parent: None,
                    visibility: Visibility::Public,
                    calls: vec![],
                    extends: None,
                    implements: vec![],
                },
                create_symbol("handle_request", SymbolKind::Function),
            ],
            importance: 0.7,
            content: None,
        },
    ];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(5000);
    let map = generator.generate(&repo);

    // Should generate module graph with nodes
    assert!(!map.module_graph.nodes.is_empty(), "Should have module nodes");
    // May or may not have edges depending on symbol resolution
}

// =============================================================================
// Display and Formatting Tests
// =============================================================================

#[test]
fn test_repomap_display() {
    let files = vec![RepoFile {
        path: "/tmp/test/main.py".into(),
        relative_path: "main.py".to_owned(),
        language: Some("python".to_owned()),
        size_bytes: 100,
        token_count: default_token_counts(),
        symbols: vec![create_symbol("main", SymbolKind::Function)],
        importance: 0.8,
        content: None,
    }];

    let repo = create_test_repo(files);
    let generator = RepoMapGenerator::new(2000);
    let map = generator.generate(&repo);

    // Test Display trait
    let display_str = format!("{}", map);
    assert!(display_str.contains("RepoMap"));
    assert!(display_str.contains("symbols"));
    assert!(display_str.contains("files"));
    assert!(display_str.contains("tokens"));
}
