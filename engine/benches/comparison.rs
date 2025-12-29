//! Benchmarks comparing Infiniloom performance with similar tools
//!
//! This benchmark suite measures:
//! - Repository scanning speed
//! - Symbol extraction performance
//! - Output generation time
//! - Memory usage patterns
//!
//! Run with: cargo bench
//!
//! For comparison with external tools (repomix, gitingest), use the
//! benchmark scripts in the scripts/ directory.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use infiniloom_engine::dependencies::DependencyGraph;
use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::{RepoFile, Repository, TokenCounts};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test repository with varying sizes
fn create_test_repo(num_files: usize, lines_per_file: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    // Create source directory
    fs::create_dir_all(base.join("src")).unwrap();

    // Generate Rust files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("//! Module {}\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
/// Function {} documentation
pub fn function_{}_{}(x: i32, y: i32) -> i32 {{
    let result = x + y;
    if result > 100 {{
        return result * 2;
    }}
    result
}}

"#,
                j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.rs", i)), content).unwrap();
    }

    // Generate Python files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("\"\"\"Module {} for Python code\"\"\"\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
def function_{}_{}(x: int, y: int) -> int:
    """Calculate result from x and y."""
    result = x + y
    if result > 100:
        return result * 2
    return result


class Class_{}_{}:
    """A sample class."""

    def __init__(self):
        self.value = 0

    def process(self):
        return self.value * 2

"#,
                i, j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.py", i)), content).unwrap();
    }

    // Generate JavaScript files
    for i in 0..num_files / 3 {
        let mut content = String::new();
        content.push_str(&format!("/**\n * Module {} for JavaScript code\n */\n\n", i));

        for j in 0..lines_per_file / 10 {
            content.push_str(&format!(
                r#"
/**
 * Function {}_{} documentation
 * @param {{{{number}}}} x - First number
 * @param {{{{number}}}} y - Second number
 * @returns {{{{number}}}} The result
 */
function function_{}_{}(x, y) {{{{
    const result = x + y;
    if (result > 100) {{{{
        return result * 2;
    }}}}
    return result;
}}}}

class Class_{}_{} {{{{
    constructor() {{{{
        this.value = 0;
    }}}}

    process() {{{{
        return this.value * 2;
    }}}}
}}}}

"#,
                i, j, i, j, i, j
            ));
        }

        fs::write(base.join(format!("src/module_{}.js", i)), content).unwrap();
    }

    // Create .gitignore
    fs::write(base.join(".gitignore"), "target/\nnode_modules/\n__pycache__/\n").unwrap();

    temp_dir
}

/// Benchmark file traversal speed
fn bench_file_traversal(c: &mut Criterion) {
    let sizes = [(10, "small"), (50, "medium"), (200, "large")];

    let mut group = c.benchmark_group("file_traversal");

    for (num_files, name) in sizes.iter() {
        let temp_dir = create_test_repo(*num_files, 100);
        let path = temp_dir.path().to_path_buf();

        group.throughput(Throughput::Elements(*num_files as u64));
        group.bench_with_input(BenchmarkId::new("walkdir", name), &path, |b, path| {
            b.iter(|| {
                let mut count = 0;
                for entry in walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        count += 1;
                    }
                }
                black_box(count)
            })
        });

        group.bench_with_input(BenchmarkId::new("ignore_crate", name), &path, |b, path| {
            b.iter(|| {
                let mut count = 0;
                for entry in ignore::WalkBuilder::new(path)
                    .hidden(false)
                    .git_ignore(true)
                    .build()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        count += 1;
                    }
                }
                black_box(count)
            })
        });
    }

    group.finish();
}

/// Benchmark file reading speed
fn bench_file_reading(c: &mut Criterion) {
    let temp_dir = create_test_repo(30, 500);
    let path = temp_dir.path().to_path_buf();

    // Collect all file paths
    let files: Vec<_> = ignore::WalkBuilder::new(&path)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();

    let mut group = c.benchmark_group("file_reading");

    // Benchmark sequential reading
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut total_bytes = 0;
            for file in &files {
                if let Ok(content) = fs::read_to_string(file) {
                    total_bytes += content.len();
                }
            }
            black_box(total_bytes)
        })
    });

    // Benchmark parallel reading with rayon
    group.bench_function("parallel_rayon", |b| {
        use rayon::prelude::*;
        b.iter(|| {
            let total_bytes: usize = files
                .par_iter()
                .filter_map(|file| fs::read_to_string(file).ok())
                .map(|content| content.len())
                .sum();
            black_box(total_bytes)
        })
    });

    group.finish();
}

/// Benchmark line counting methods
fn bench_line_counting(c: &mut Criterion) {
    // Create a large file for benchmarking
    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large.rs");

    let content: String = (0..10000)
        .map(|i| format!("fn function_{}() {{ /* code */ }}\n", i))
        .collect();
    fs::write(&large_file, &content).unwrap();

    let file_content = fs::read_to_string(&large_file).unwrap();

    let mut group = c.benchmark_group("line_counting");

    group.bench_function("lines_iterator", |b| b.iter(|| black_box(file_content.lines().count())));

    group.bench_function("matches_newline", |b| {
        b.iter(|| black_box(file_content.matches('\n').count() + 1))
    });

    group.bench_function("bytes_filter", |b| {
        b.iter(|| black_box(file_content.bytes().filter(|&b| b == b'\n').count() + 1))
    });

    group.finish();
}

/// Benchmark token estimation methods
fn bench_token_estimation(c: &mut Criterion) {
    // Sample code of varying sizes
    let small_code = "fn main() { println!(\"Hello\"); }";
    let medium_code: String = (0..100)
        .map(|i| format!("fn function_{}(x: i32) -> i32 {{ x * {} }}\n", i, i))
        .collect();
    let large_code: String = (0..1000)
        .map(|i| format!("fn function_{}(x: i32) -> i32 {{ x * {} }}\n", i, i))
        .collect();

    let mut group = c.benchmark_group("token_estimation");

    // Simple char-based estimation
    group.bench_function("char_ratio_small", |b| {
        b.iter(|| black_box((small_code.len() as f32 / 3.5) as u32))
    });

    group.bench_function("char_ratio_medium", |b| {
        b.iter(|| black_box((medium_code.len() as f32 / 3.5) as u32))
    });

    group.bench_function("char_ratio_large", |b| {
        b.iter(|| black_box((large_code.len() as f32 / 3.5) as u32))
    });

    // Word-based estimation (more accurate but slower)
    group.bench_function("word_based_small", |b| {
        b.iter(|| {
            let words = small_code.split_whitespace().count();
            let symbols = small_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.bench_function("word_based_medium", |b| {
        b.iter(|| {
            let words = medium_code.split_whitespace().count();
            let symbols = medium_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.bench_function("word_based_large", |b| {
        b.iter(|| {
            let words = large_code.split_whitespace().count();
            let symbols = large_code.chars().filter(|c| !c.is_alphanumeric()).count();
            black_box(words + symbols / 2)
        })
    });

    group.finish();
}

/// Benchmark output format generation
fn bench_output_generation(c: &mut Criterion) {
    // Simulated repository data
    let files: Vec<(String, String)> = (0..50)
        .map(|i| {
            let path = format!("src/module_{}.rs", i);
            let content: String = (0..20)
                .map(|j| format!("fn func_{}_{}_{}() {{ }}\n", i, j, j))
                .collect();
            (path, content)
        })
        .collect();

    let mut group = c.benchmark_group("output_generation");

    // XML generation
    group.bench_function("xml_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("<repository>\n");
            output.push_str("  <files>\n");
            for (path, content) in &files {
                output.push_str(&format!("    <file path=\"{}\">\n", path));
                output.push_str("      <content><![CDATA[");
                output.push_str(content);
                output.push_str("]]></content>\n");
                output.push_str("    </file>\n");
            }
            output.push_str("  </files>\n");
            output.push_str("</repository>");
            black_box(output)
        })
    });

    // Markdown generation
    group.bench_function("markdown_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("# Repository\n\n");
            for (path, content) in &files {
                output.push_str(&format!("## {}\n\n", path));
                output.push_str("```rust\n");
                output.push_str(content);
                output.push_str("```\n\n");
            }
            black_box(output)
        })
    });

    // JSON generation
    group.bench_function("json_format", |b| {
        b.iter(|| {
            let mut output = String::with_capacity(100_000);
            output.push_str("{\"files\":[");
            for (i, (path, content)) in files.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                output.push_str(&format!(
                    "{{\"path\":\"{}\",\"content\":{}}}",
                    path,
                    serde_json::to_string(content).unwrap()
                ));
            }
            output.push_str("]}");
            black_box(output)
        })
    });

    group.finish();
}

/// Benchmark language detection
fn bench_language_detection(c: &mut Criterion) {
    let extensions = [
        "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", "rb", "php",
        "swift", "kt", "scala", "cs", "fs", "ml", "hs", "clj", "ex", "erl", "lua", "r", "jl",
        "zig", "nim", "cr", "d", "ada", "pas", "f90", "cob", "pl", "tcl", "sh", "bash", "zsh",
    ];

    let mut group = c.benchmark_group("language_detection");

    group.bench_function("match_extension", |b| {
        b.iter(|| {
            for ext in &extensions {
                let lang = match *ext {
                    "rs" => "rust",
                    "py" | "pyw" => "python",
                    "js" | "jsx" | "mjs" => "javascript",
                    "ts" | "tsx" => "typescript",
                    "go" => "go",
                    "java" => "java",
                    "c" | "h" => "c",
                    "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                    _ => "unknown",
                };
                black_box(lang);
            }
        })
    });

    group.bench_function("hashmap_lookup", |b| {
        use std::collections::HashMap;
        let map: HashMap<&str, &str> = [
            ("rs", "rust"),
            ("py", "python"),
            ("pyw", "python"),
            ("js", "javascript"),
            ("jsx", "javascript"),
            ("mjs", "javascript"),
            ("ts", "typescript"),
            ("tsx", "typescript"),
            ("go", "go"),
            ("java", "java"),
            ("c", "c"),
            ("h", "c"),
            ("cpp", "cpp"),
            ("cc", "cpp"),
            ("cxx", "cpp"),
            ("hpp", "cpp"),
        ]
        .into_iter()
        .collect();

        b.iter(|| {
            for ext in &extensions {
                let lang = map.get(ext).copied().unwrap_or("unknown");
                black_box(lang);
            }
        })
    });

    group.finish();
}

/// Benchmark regex pattern matching for security scanning
fn bench_security_patterns(c: &mut Criterion) {
    use regex::Regex;

    let content = r#"
const AWS_KEY = "AKIAIOSFODNN7EXAMPLE";
const password = "mysecretpassword123";
const api_key = "sk-1234567890abcdef";
const token = "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
function connect() {
    const conn = "postgresql://user:pass@localhost/db";
}
"#
    .repeat(100);

    let mut group = c.benchmark_group("security_patterns");

    // Single pattern
    group.bench_function("single_regex", |b| {
        let re =
            Regex::new(r#"(?i)(password|secret|api_key|token)\s*[=:]\s*['"][^'"]+['"]"#).unwrap();
        b.iter(|| black_box(re.find_iter(&content).count()))
    });

    // Multiple patterns compiled separately
    group.bench_function("multiple_regex", |b| {
        let patterns = vec![
            Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            Regex::new(r#"(?i)password\s*[=:]\s*['"][^'"]+['"]"#).unwrap(),
            Regex::new(r#"(?i)api_key\s*[=:]\s*['"][^'"]+['"]"#).unwrap(),
            Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
        ];
        b.iter(|| {
            let mut count = 0;
            for re in &patterns {
                count += re.find_iter(&content).count();
            }
            black_box(count)
        })
    });

    // RegexSet for multiple patterns
    group.bench_function("regex_set", |b| {
        use regex::RegexSet;
        let set = RegexSet::new([
            r"AKIA[0-9A-Z]{16}",
            r#"(?i)password\s*[=:]\s*['"][^'"]+['"]"#,
            r#"(?i)api_key\s*[=:]\s*['"][^'"]+['"]"#,
            r"ghp_[a-zA-Z0-9]{36}",
        ])
        .unwrap();
        b.iter(|| black_box(set.matches(&content).iter().count()))
    });

    group.finish();
}

/// Benchmark string allocation strategies
fn bench_string_building(c: &mut Criterion) {
    let parts: Vec<String> = (0..1000)
        .map(|i| format!("Part {} of the content\n", i))
        .collect();

    let mut group = c.benchmark_group("string_building");

    // Push approach
    group.bench_function("push_string", |b| {
        b.iter(|| {
            let mut result = String::new();
            for part in &parts {
                result.push_str(part);
            }
            black_box(result)
        })
    });

    // Pre-allocated push
    group.bench_function("push_preallocated", |b| {
        b.iter(|| {
            let total_len: usize = parts.iter().map(|p| p.len()).sum();
            let mut result = String::with_capacity(total_len);
            for part in &parts {
                result.push_str(part);
            }
            black_box(result)
        })
    });

    // Collect approach
    group.bench_function("collect_join", |b| {
        b.iter(|| {
            let result: String = parts.iter().map(|s| s.as_str()).collect();
            black_box(result)
        })
    });

    // Collect with join
    group.bench_function("vec_join", |b| {
        b.iter(|| {
            let result = parts.join("");
            black_box(result)
        })
    });

    group.finish();
}

// ============================================================================
// Parser Benchmarks - Tree-sitter AST parsing
// ============================================================================

/// Generate Python code with the given number of functions
fn generate_python_code(num_functions: usize) -> String {
    let mut content = String::new();
    content.push_str("\"\"\"Generated Python module for benchmarking.\"\"\"\n\n");

    for i in 0..num_functions {
        content.push_str(&format!(
            "def function_{}(arg1, arg2):\n    \"\"\"Docstring for function {}.\"\"\"\n    result = arg1 + arg2\n    return result\n\n",
            i, i
        ));
    }
    content
}

/// Generate Rust code with the given number of functions
fn generate_rust_code(num_functions: usize) -> String {
    let mut content = String::new();
    content.push_str("//! Generated Rust module for benchmarking.\n\n");

    for i in 0..num_functions {
        content.push_str(&format!(
            "/// Documentation for function {}.\npub fn function_{}(arg1: i32, arg2: i32) -> i32 {{\n    let result = arg1 + arg2;\n    result\n}}\n\n",
            i, i
        ));
    }
    content
}

/// Generate JavaScript code with the given number of functions
fn generate_js_code(num_functions: usize) -> String {
    let mut content = String::new();
    content.push_str("/**\n * Generated JavaScript module for benchmarking.\n */\n\n");

    for i in 0..num_functions {
        content.push_str(&format!(
            "/**\n * Function {} description.\n * @param {{number}} arg1 First argument\n * @param {{number}} arg2 Second argument\n * @returns {{number}} The result\n */\nfunction function_{}(arg1, arg2) {{\n    const result = arg1 + arg2;\n    return result;\n}}\n\n",
            i, i
        ));
    }
    content
}

/// Benchmark Tree-sitter parsing across languages
fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    // Test sizes: 10, 50, 100 functions
    let sizes = [(10, "10_funcs"), (50, "50_funcs"), (100, "100_funcs")];

    for (num_funcs, size_name) in sizes.iter() {
        let python_code = generate_python_code(*num_funcs);
        let rust_code = generate_rust_code(*num_funcs);
        let js_code = generate_js_code(*num_funcs);

        // Python parsing
        group.bench_with_input(BenchmarkId::new("python", size_name), &python_code, |b, code| {
            b.iter(|| {
                let mut parser = Parser::new();
                let symbols = parser.parse(code, Language::Python).unwrap();
                black_box(symbols.len())
            })
        });

        // Rust parsing
        group.bench_with_input(BenchmarkId::new("rust", size_name), &rust_code, |b, code| {
            b.iter(|| {
                let mut parser = Parser::new();
                let symbols = parser.parse(code, Language::Rust).unwrap();
                black_box(symbols.len())
            })
        });

        // JavaScript parsing
        group.bench_with_input(BenchmarkId::new("javascript", size_name), &js_code, |b, code| {
            b.iter(|| {
                let mut parser = Parser::new();
                let symbols = parser.parse(code, Language::JavaScript).unwrap();
                black_box(symbols.len())
            })
        });
    }

    group.finish();
}

/// Benchmark parser reuse (single parser instance vs new each time)
fn bench_parser_reuse(c: &mut Criterion) {
    let python_code = generate_python_code(50);

    let mut group = c.benchmark_group("parser_reuse");

    // New parser each time
    group.bench_function("new_parser_each", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            let symbols = parser.parse(&python_code, Language::Python).unwrap();
            black_box(symbols.len())
        })
    });

    // Reuse single parser
    group.bench_function("reuse_parser", |b| {
        let mut parser = Parser::new();
        b.iter(|| {
            let symbols = parser.parse(&python_code, Language::Python).unwrap();
            black_box(symbols.len())
        })
    });

    group.finish();
}

// ============================================================================
// Dependency Graph Benchmarks
// ============================================================================

/// Create a RepoFile for dependency graph testing
fn create_repo_file(path: &str, language: &str, content: &str) -> RepoFile {
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

/// Benchmark dependency graph construction
fn bench_dependency_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_graph");

    // Small: 20 files
    let small_files: Vec<RepoFile> = (0..20)
        .map(|i| {
            let content = generate_python_code(5);
            create_repo_file(&format!("src/module_{}.py", i), "python", &content)
        })
        .collect();

    let mut small_repo = Repository::new("small", "/tmp/small");
    small_repo.files = small_files;

    group.bench_function("build_small_20_files", |b| {
        b.iter(|| {
            let graph = DependencyGraph::build(&small_repo);
            black_box(graph.stats())
        })
    });

    // Medium: 100 files
    let medium_files: Vec<RepoFile> = (0..100)
        .map(|i| {
            let content = generate_python_code(10);
            create_repo_file(&format!("src/module_{}.py", i), "python", &content)
        })
        .collect();

    let mut medium_repo = Repository::new("medium", "/tmp/medium");
    medium_repo.files = medium_files;

    group.bench_function("build_medium_100_files", |b| {
        b.iter(|| {
            let graph = DependencyGraph::build(&medium_repo);
            black_box(graph.stats())
        })
    });

    // Large: 300 files with mixed languages
    let large_files: Vec<RepoFile> = (0..300)
        .map(|i| {
            let (lang, ext, content) = match i % 3 {
                0 => ("python", "py", generate_python_code(8)),
                1 => ("rust", "rs", generate_rust_code(8)),
                _ => ("javascript", "js", generate_js_code(8)),
            };
            create_repo_file(&format!("src/module_{}.{}", i, ext), lang, &content)
        })
        .collect();

    let mut large_repo = Repository::new("large", "/tmp/large");
    large_repo.files = large_files;

    group.bench_function("build_large_300_files", |b| {
        b.iter(|| {
            let graph = DependencyGraph::build(&large_repo);
            black_box(graph.stats())
        })
    });

    group.finish();
}

/// Benchmark cycle detection in dependency graphs
fn bench_cycle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("cycle_detection");

    // Create repo with many files
    let files: Vec<RepoFile> = (0..100)
        .map(|i| {
            let content = generate_python_code(5);
            create_repo_file(&format!("src/module_{}.py", i), "python", &content)
        })
        .collect();

    let mut repo = Repository::new("cycle_test", "/tmp/cycle_test");
    repo.files = files;
    let graph = DependencyGraph::build(&repo);

    group.bench_function("detect_cycles_100_files", |b| {
        b.iter(|| {
            let cycles = graph.get_circular_deps();
            black_box(cycles.len())
        })
    });

    group.finish();
}

/// Benchmark PageRank computation
fn bench_pagerank(c: &mut Criterion) {
    let mut group = c.benchmark_group("pagerank");

    // Create repo with varying sizes
    for (num_files, name) in [(50, "50_files"), (200, "200_files")].iter() {
        let files: Vec<RepoFile> = (0..*num_files)
            .map(|i| {
                let content = generate_python_code(10);
                create_repo_file(&format!("src/module_{}.py", i), "python", &content)
            })
            .collect();

        let mut repo = Repository::new("pagerank_test", "/tmp/pagerank");
        repo.files = files;
        let graph = DependencyGraph::build(&repo);

        group.bench_function(*name, |b| {
            b.iter(|| {
                let top = graph.get_most_important(10);
                black_box(top.len())
            })
        });
    }

    group.finish();
}

// ============================================================================
// Parallel Processing Benchmarks
// ============================================================================

/// Benchmark parallel vs sequential parsing
fn bench_parallel_parsing(c: &mut Criterion) {
    use rayon::prelude::*;

    let codes: Vec<(String, Language)> = (0..50)
        .map(|i| {
            let (code, lang) = match i % 3 {
                0 => (generate_python_code(20), Language::Python),
                1 => (generate_rust_code(20), Language::Rust),
                _ => (generate_js_code(20), Language::JavaScript),
            };
            (code, lang)
        })
        .collect();

    let mut group = c.benchmark_group("parallel_parsing");

    // Sequential parsing
    group.bench_function("sequential_50_files", |b| {
        b.iter(|| {
            let mut total = 0;
            let mut parser = Parser::new();
            for (code, lang) in &codes {
                if let Ok(symbols) = parser.parse(code, *lang) {
                    total += symbols.len();
                }
            }
            black_box(total)
        })
    });

    // Parallel parsing with thread-local parsers
    group.bench_function("parallel_50_files", |b| {
        b.iter(|| {
            let total: usize = codes
                .par_iter()
                .map(|(code, lang)| {
                    let mut parser = Parser::new();
                    parser.parse(code, *lang).map(|s| s.len()).unwrap_or(0)
                })
                .sum();
            black_box(total)
        })
    });

    group.finish();
}

// ============================================================================
// Phase 1 Refactoring Benchmarks (Item 6)
// ============================================================================

/// Benchmark XML escaping function (Item 3 - Centralize XML/YAML Escaping)
fn bench_xml_escaping(c: &mut Criterion) {
    use infiniloom_engine::output::escaping::{escape_xml_attribute, escape_xml_text};

    // Various test strings with different characteristics
    let simple = "Hello, world!";
    let with_ampersand = "This & that & the other thing";
    let with_tags = "<div class=\"container\"><p>Text with <b>bold</b> & <i>italic</i></p></div>";
    let with_quotes = r#"attribute="value" and 'another' value"#;
    let mixed = r#"Complex: <tag attr="val & 'quoted'">content & more</tag>"#;
    let large = "<tag>".repeat(1000) + &"content & text".repeat(1000) + &"</tag>".repeat(1000);
    let no_escaping_needed = "JustPlainTextWithNoSpecialCharacters".repeat(100);

    let mut group = c.benchmark_group("xml_escaping");

    // escape_xml_text benchmarks
    group.bench_function("text_simple", |b| b.iter(|| black_box(escape_xml_text(simple))));

    group.bench_function("text_with_ampersand", |b| {
        b.iter(|| black_box(escape_xml_text(with_ampersand)))
    });

    group.bench_function("text_with_tags", |b| b.iter(|| black_box(escape_xml_text(with_tags))));

    group
        .bench_function("text_with_quotes", |b| b.iter(|| black_box(escape_xml_text(with_quotes))));

    group.bench_function("text_mixed", |b| b.iter(|| black_box(escape_xml_text(mixed))));

    group.bench_function("text_large", |b| b.iter(|| black_box(escape_xml_text(&large))));

    group.bench_function("text_no_escaping", |b| {
        b.iter(|| black_box(escape_xml_text(&no_escaping_needed)))
    });

    // escape_xml_attribute benchmarks (delegates to text escaping)
    group
        .bench_function("attribute_simple", |b| b.iter(|| black_box(escape_xml_attribute(simple))));

    group.bench_function("attribute_mixed", |b| b.iter(|| black_box(escape_xml_attribute(mixed))));

    group.finish();
}

/// Benchmark YAML escaping function (Item 3 - Centralize XML/YAML Escaping)
fn bench_yaml_escaping(c: &mut Criterion) {
    use infiniloom_engine::output::escaping::escape_yaml_string;

    // Various test strings
    let simple = "Simple string";
    let with_backslash = r"Path: C:\Users\test\file.txt";
    let with_quotes = r#"String with "double quotes" inside"#;
    let with_newlines = "Line 1\nLine 2\nLine 3\n";
    let with_tabs = "Col1\tCol2\tCol3\t";
    let mixed = r#"Complex: "quoted" with \backslash\ and newlines\n"#;
    let large = "Text\n".repeat(1000) + &r#""quoted" \"#.repeat(1000);
    let no_escaping_needed = "PlainTextNoEscaping".repeat(100);

    let mut group = c.benchmark_group("yaml_escaping");

    group.bench_function("simple", |b| b.iter(|| black_box(escape_yaml_string(simple))));

    group.bench_function("with_backslash", |b| {
        b.iter(|| black_box(escape_yaml_string(with_backslash)))
    });

    group.bench_function("with_quotes", |b| b.iter(|| black_box(escape_yaml_string(with_quotes))));

    group.bench_function("with_newlines", |b| {
        b.iter(|| black_box(escape_yaml_string(with_newlines)))
    });

    group.bench_function("with_tabs", |b| b.iter(|| black_box(escape_yaml_string(with_tabs))));

    group.bench_function("mixed", |b| b.iter(|| black_box(escape_yaml_string(mixed))));

    group.bench_function("large", |b| b.iter(|| black_box(escape_yaml_string(&large))));

    group.bench_function("no_escaping", |b| {
        b.iter(|| black_box(escape_yaml_string(&no_escaping_needed)))
    });

    group.finish();
}

/// Benchmark base64 truncation function (Item 4 - Centralize Base64 Truncation)
fn bench_base64_truncation(c: &mut Criterion) {
    use infiniloom_engine::content_processing::truncate_base64;

    // Test strings with different base64 patterns
    let no_base64 = "This is just plain text with no base64 content at all.".repeat(100);

    let data_uri_small = r#"<img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA" />"#;

    let data_uri_large = format!(r#"<img src="data:image/png;base64,{}" />"#, "A".repeat(1000));

    let long_base64 = format!(
        "Some text before {} and some text after",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".repeat(10)
    );

    let multiple_data_uris = format!(
        r#"<img src="data:image/png;base64,{}" />
           <img src="data:image/jpeg;base64,{}" />
           <img src="data:image/gif;base64,{}" />"#,
        "A".repeat(500),
        "B".repeat(500),
        "C".repeat(500)
    );

    let mixed_content = format!(
        "Normal text here. data:text/plain;base64,{} More text. {} End text.",
        "D".repeat(300),
        "EFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".repeat(5)
    );

    let very_large = format!(
        "Before {} Middle {} After",
        "data:image/png;base64,".to_string() + &"X".repeat(10000),
        "YZ0123456789+/".repeat(1000)
    );

    let mut group = c.benchmark_group("base64_truncation");

    group.bench_function("no_base64", |b| b.iter(|| black_box(truncate_base64(&no_base64))));

    group.bench_function("data_uri_small", |b| {
        b.iter(|| black_box(truncate_base64(data_uri_small)))
    });

    group.bench_function("data_uri_large", |b| {
        b.iter(|| black_box(truncate_base64(&data_uri_large)))
    });

    group.bench_function("long_base64", |b| b.iter(|| black_box(truncate_base64(&long_base64))));

    group.bench_function("multiple_data_uris", |b| {
        b.iter(|| black_box(truncate_base64(&multiple_data_uris)))
    });

    group
        .bench_function("mixed_content", |b| b.iter(|| black_box(truncate_base64(&mixed_content))));

    group.bench_function("very_large", |b| b.iter(|| black_box(truncate_base64(&very_large))));

    group.finish();
}

// Configure criterion
criterion_group!(
    benches,
    bench_file_traversal,
    bench_file_reading,
    bench_line_counting,
    bench_token_estimation,
    bench_output_generation,
    bench_language_detection,
    bench_security_patterns,
    bench_string_building,
    bench_parser,
    bench_parser_reuse,
    bench_dependency_graph,
    bench_cycle_detection,
    bench_pagerank,
    bench_parallel_parsing,
    // Phase 1 refactoring benchmarks
    bench_xml_escaping,
    bench_yaml_escaping,
    bench_base64_truncation,
);

criterion_main!(benches);
