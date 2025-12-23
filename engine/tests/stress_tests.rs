//! Stress tests for Infiniloom engine.
//!
//! These tests verify the engine handles large codebases correctly.
//! They are marked #[ignore] and run separately with `cargo test --ignored`.
//!
//! Run with: cargo test -p infiniloom-engine --test stress_tests -- --ignored

use infiniloom_engine::dependencies::DependencyGraph;
use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::{RepoFile, Repository, TokenCounts};
use std::path::PathBuf;
use std::time::Instant;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a RepoFile with the given parameters
fn create_file(path: &str, language: &str, content: &str) -> RepoFile {
    RepoFile {
        path: PathBuf::from(format!("/tmp/test/{}", path)),
        relative_path: path.to_string(),
        language: Some(language.to_string()),
        symbols: Vec::new(),
        token_count: TokenCounts::default(),
        importance: 0.0,
        content: Some(content.to_string()),
        size_bytes: content.len() as u64,
    }
}

/// Generate Python content with the given number of functions
fn generate_python_content(num_functions: usize, imports: &[&str]) -> String {
    let mut content = String::new();

    // Add imports
    for import in imports {
        content.push_str(&format!("import {}\n", import));
    }
    content.push('\n');

    // Add functions
    for i in 0..num_functions {
        content.push_str(&format!(
            "def function_{}(arg1, arg2):\n    \"\"\"Docstring for function {}.\"\"\"\n    result = arg1 + arg2\n    return result\n\n",
            i, i
        ));
    }

    content
}

/// Generate JavaScript content with the given number of functions
fn generate_js_content(num_functions: usize, imports: &[&str]) -> String {
    let mut content = String::new();

    // Add imports
    for import in imports {
        content.push_str(&format!("const {} = require('{}');\n", import.replace('-', "_"), import));
    }
    content.push('\n');

    // Add functions
    for i in 0..num_functions {
        content.push_str(&format!(
            "/**\n * Function {} description.\n * @param {{number}} arg1 First argument\n * @param {{number}} arg2 Second argument\n * @returns {{number}} The result\n */\nfunction function_{}(arg1, arg2) {{\n    const result = arg1 + arg2;\n    return result;\n}}\n\n",
            i, i
        ));
    }

    content
}

/// Generate Rust content with the given number of functions
fn generate_rust_content(num_functions: usize, uses: &[&str]) -> String {
    let mut content = String::new();

    // Add uses
    for use_stmt in uses {
        content.push_str(&format!("use {};\n", use_stmt));
    }
    content.push('\n');

    // Add functions
    for i in 0..num_functions {
        content.push_str(&format!(
            "/// Documentation for function {}.\npub fn function_{}(arg1: i32, arg2: i32) -> i32 {{\n    let result = arg1 + arg2;\n    result\n}}\n\n",
            i, i
        ));
    }

    content
}

// ============================================================================
// Large File Count Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_1000_files() {
    let start = Instant::now();

    // Generate 1000 files with ~20 functions each
    let mut files = Vec::new();

    for i in 0..1000 {
        let content = generate_python_content(20, &[]);
        let file = create_file(&format!("src/module_{}.py", i), "python", &content);
        files.push(file);
    }

    let mut repo = Repository::new("stress_test", "/tmp/stress_test");
    repo.files = files;

    // Build dependency graph
    let graph = DependencyGraph::build(&repo);

    let stats = graph.stats();
    let elapsed = start.elapsed();

    println!("Stress test 1000 files:");
    println!("  Total files: {}", stats.total_files);
    println!("  Total edges: {}", stats.total_edges);
    println!("  Time elapsed: {:?}", elapsed);

    assert_eq!(stats.total_files, 1000);
    assert!(elapsed.as_secs() < 30, "Should complete within 30 seconds");
}

#[test]
#[ignore]
fn stress_test_500_files_mixed_languages() {
    let start = Instant::now();

    let mut files = Vec::new();

    // 200 Python files
    for i in 0..200 {
        let content = generate_python_content(15, &[]);
        files.push(create_file(&format!("src/py/module_{}.py", i), "python", &content));
    }

    // 150 JavaScript files
    for i in 0..150 {
        let content = generate_js_content(15, &[]);
        files.push(create_file(&format!("src/js/module_{}.js", i), "javascript", &content));
    }

    // 100 Rust files
    for i in 0..100 {
        let content = generate_rust_content(15, &[]);
        files.push(create_file(&format!("src/rs/module_{}.rs", i), "rust", &content));
    }

    // 50 TypeScript files
    for i in 0..50 {
        let content = generate_js_content(15, &[]);
        files.push(create_file(&format!("src/ts/module_{}.ts", i), "typescript", &content));
    }

    let mut repo = Repository::new("mixed_lang_stress", "/tmp/mixed_stress");
    repo.files = files;

    let graph = DependencyGraph::build(&repo);

    let stats = graph.stats();
    let elapsed = start.elapsed();

    println!("Stress test 500 mixed language files:");
    println!("  Total files: {}", stats.total_files);
    println!("  Time elapsed: {:?}", elapsed);

    assert_eq!(stats.total_files, 500);
    assert!(elapsed.as_secs() < 20, "Should complete within 20 seconds");
}

// ============================================================================
// Large File Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_large_file_10k_lines() {
    let start = Instant::now();

    // Generate a file with ~10,000 lines (500 functions * ~20 lines each)
    let content = generate_python_content(500, &[]);

    let mut parser = Parser::new();
    let symbols = parser.parse(&content, Language::Python).unwrap();

    let elapsed = start.elapsed();

    println!("Stress test large file (10K lines):");
    println!("  Content size: {} bytes", content.len());
    println!("  Symbols found: {}", symbols.len());
    println!("  Parse time: {:?}", elapsed);

    assert!(symbols.len() >= 500, "Should find at least 500 functions");
    assert!(elapsed.as_millis() < 5000, "Should parse within 5 seconds");
}

#[test]
#[ignore]
fn stress_test_very_large_file_50k_lines() {
    let start = Instant::now();

    // Generate a file with ~50,000 lines (2500 functions * ~20 lines each)
    let content = generate_python_content(2500, &[]);

    let mut parser = Parser::new();
    let symbols = parser.parse(&content, Language::Python).unwrap();

    let elapsed = start.elapsed();

    println!("Stress test very large file (50K lines):");
    println!("  Content size: {} bytes", content.len());
    println!("  Symbols found: {}", symbols.len());
    println!("  Parse time: {:?}", elapsed);

    assert!(symbols.len() >= 2500, "Should find at least 2500 functions");
    assert!(elapsed.as_secs() < 30, "Should parse within 30 seconds");
}

// ============================================================================
// Deep Import Chain Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_deep_import_chain_100() {
    let start = Instant::now();

    // Create a chain of 100 files: a0 -> a1 -> a2 -> ... -> a99
    let mut files = Vec::new();

    for i in 0..100 {
        let import = if i < 99 {
            format!("const next = require('./a{}');\n", i + 1)
        } else {
            String::new()
        };

        let content = format!("{}function func_{}() {{ return {}; }}\n", import, i, i);
        files.push(create_file(&format!("src/a{}.js", i), "javascript", &content));
    }

    let mut repo = Repository::new("deep_chain", "/tmp/deep_chain");
    repo.files = files;

    let graph = DependencyGraph::build(&repo);

    let stats = graph.stats();
    let elapsed = start.elapsed();

    println!("Stress test deep chain (100 files):");
    println!("  Total files: {}", stats.total_files);
    println!("  Circular deps: {}", stats.circular_dep_groups);
    println!("  Time elapsed: {:?}", elapsed);

    assert_eq!(stats.total_files, 100);
    assert_eq!(stats.circular_dep_groups, 0, "Linear chain should have no cycles");
    assert!(elapsed.as_millis() < 5000, "Should complete within 5 seconds");
}

#[test]
#[ignore]
fn stress_test_wide_imports_200() {
    let start = Instant::now();

    // Create 201 files where the main file imports all 200 others
    let mut files = Vec::new();

    // Create 200 utility files
    for i in 0..200 {
        let content = format!(
            "function util_{}() {{ return {}; }}\nmodule.exports = {{ util_{} }};\n",
            i, i, i
        );
        files.push(create_file(&format!("src/util_{}.js", i), "javascript", &content));
    }

    // Create main file that imports all utilities
    let mut main_content = String::new();
    for i in 0..200 {
        main_content.push_str(&format!("const util_{} = require('./util_{}');\n", i, i));
    }
    main_content.push_str("\nfunction main() {\n    console.log('Main function');\n}\n");
    files.push(create_file("src/main.js", "javascript", &main_content));

    let mut repo = Repository::new("wide_imports", "/tmp/wide_imports");
    repo.files = files;

    let graph = DependencyGraph::build(&repo);

    let stats = graph.stats();
    let elapsed = start.elapsed();

    println!("Stress test wide imports (200 imports):");
    println!("  Total files: {}", stats.total_files);
    println!("  External deps: {}", stats.external_deps);
    println!("  Time elapsed: {:?}", elapsed);

    assert_eq!(stats.total_files, 201);
    assert!(elapsed.as_secs() < 10, "Should complete within 10 seconds");
}

// ============================================================================
// Parallel Parsing Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_parallel_parsing() {
    use std::thread;

    let start = Instant::now();

    // Generate test content
    let contents: Vec<String> = (0..100).map(|_| generate_python_content(50, &[])).collect();

    // Parse in parallel threads
    let handles: Vec<_> = contents
        .into_iter()
        .map(|content| {
            thread::spawn(move || {
                let mut parser = Parser::new();
                let result = parser.parse(&content, Language::Python);
                match result {
                    Ok(symbols) => symbols.len(),
                    Err(_) => 0,
                }
            })
        })
        .collect();

    // Collect results
    let total_symbols: usize = handles.into_iter().filter_map(|h| h.join().ok()).sum();

    let elapsed = start.elapsed();

    println!("Stress test parallel parsing (100 threads):");
    println!("  Total symbols: {}", total_symbols);
    println!("  Time elapsed: {:?}", elapsed);

    assert!(total_symbols >= 5000, "Should parse at least 5000 symbols total");
    assert!(elapsed.as_secs() < 30, "Should complete within 30 seconds");
}

// ============================================================================
// Memory Pressure Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_memory_many_symbols() {
    let start = Instant::now();

    // Generate file with many small functions (high symbol count)
    let mut content = String::new();
    for i in 0..5000 {
        content.push_str(&format!("def f{}(): pass\n", i));
    }

    let mut parser = Parser::new();
    let symbols = parser.parse(&content, Language::Python).unwrap();

    let elapsed = start.elapsed();

    println!("Stress test memory (5000 symbols):");
    println!("  Symbols found: {}", symbols.len());
    println!("  Time elapsed: {:?}", elapsed);

    assert!(symbols.len() >= 5000, "Should find at least 5000 functions");
    assert!(elapsed.as_secs() < 10, "Should complete within 10 seconds");
}

#[test]
#[ignore]
fn stress_test_deeply_nested_code() {
    let start = Instant::now();

    // Generate Python code with deep nesting
    let mut content = String::new();

    // Create deeply nested classes and methods
    for depth in 0..20 {
        let indent = "    ".repeat(depth);
        content.push_str(&format!("{}class Level{}:\n", indent, depth));
        content.push_str(&format!("{}    def method_{}(self):\n", indent, depth));
        content.push_str(&format!("{}        pass\n", indent));
    }

    let mut parser = Parser::new();
    let result = parser.parse(&content, Language::Python);

    let elapsed = start.elapsed();

    println!("Stress test deeply nested code:");
    println!("  Parse result: {:?}", result.is_ok());
    println!("  Time elapsed: {:?}", elapsed);

    // Should handle without panic
    assert!(result.is_ok() || result.is_err()); // Just ensure no panic
}

// ============================================================================
// Edge Case Stress Tests
// ============================================================================

#[test]
#[ignore]
fn stress_test_unicode_content() {
    let start = Instant::now();

    // Generate content with Unicode characters
    let content = r#"
# Unicode test file with various characters
# 日本語コメント
# Комментарий на русском
# תגובה בעברית

def greet_世界():
    """Say hello in multiple languages."""
    messages = [
        "Hello World",
        "こんにちは世界",
        "Привет мир",
        "שלום עולם",
        "مرحبا بالعالم",
    ]
    return messages

def calculate_émojis():
    """Function with émojis in name."""
    return "🎉🎊🎁"

class UnicodeClass_αβγ:
    """Class with Greek letters."""

    def method_δεζ(self):
        """Method with more Greek letters."""
        return "δεζ"
"#;

    let mut parser = Parser::new();
    let symbols = parser.parse(content, Language::Python).unwrap();

    let elapsed = start.elapsed();

    println!("Stress test Unicode content:");
    println!("  Symbols found: {}", symbols.len());
    println!("  Time elapsed: {:?}", elapsed);

    assert!(symbols.len() >= 3, "Should find at least 3 symbols");
}

#[test]
#[ignore]
fn stress_test_empty_and_minimal_files() {
    let start = Instant::now();

    let mut files = Vec::new();

    // Mix of empty, minimal, and normal files
    for i in 0..100 {
        let content = match i % 4 {
            0 => String::new(),                       // Empty
            1 => "# Comment only\n".to_string(),      // Comment only
            2 => "x = 1\n".to_string(),               // Minimal
            _ => format!("def func_{}(): pass\n", i), // Normal
        };
        files.push(create_file(&format!("src/file_{}.py", i), "python", &content));
    }

    let mut repo = Repository::new("mixed_files", "/tmp/mixed_files");
    repo.files = files;

    let graph = DependencyGraph::build(&repo);

    let stats = graph.stats();
    let elapsed = start.elapsed();

    println!("Stress test empty/minimal files:");
    println!("  Total files: {}", stats.total_files);
    println!("  Time elapsed: {:?}", elapsed);

    assert_eq!(stats.total_files, 100);
}

// ============================================================================
// Benchmark Helpers
// ============================================================================

/// Run a benchmark and report results
fn run_benchmark<F>(name: &str, iterations: usize, f: F)
where
    F: Fn() -> usize,
{
    let start = Instant::now();
    let mut total = 0;

    for _ in 0..iterations {
        total += f();
    }

    let elapsed = start.elapsed();
    let per_iteration = elapsed / iterations as u32;

    println!(
        "{}: {} iterations in {:?} ({:?}/iter, total: {})",
        name, iterations, elapsed, per_iteration, total
    );
}

#[test]
#[ignore]
fn benchmark_parser_throughput() {
    let content = generate_python_content(100, &[]);

    run_benchmark("Python parser (100 funcs)", 100, || {
        let mut parser = Parser::new();
        parser
            .parse(&content, Language::Python)
            .map(|s| s.len())
            .unwrap_or(0)
    });

    let js_content = generate_js_content(100, &[]);

    run_benchmark("JavaScript parser (100 funcs)", 100, || {
        let mut parser = Parser::new();
        parser
            .parse(&js_content, Language::JavaScript)
            .map(|s| s.len())
            .unwrap_or(0)
    });

    let rust_content = generate_rust_content(100, &[]);

    run_benchmark("Rust parser (100 funcs)", 100, || {
        let mut parser = Parser::new();
        parser
            .parse(&rust_content, Language::Rust)
            .map(|s| s.len())
            .unwrap_or(0)
    });
}
