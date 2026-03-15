//! Integration tests for the `infiniloom embed` CLI command.
//!
//! Tests cover: basic operation, output formats, flag handling, streaming mode,
//! error cases, output-to-file, manifest creation, and incremental (diff-only) mode.
//!
//! Run with: cargo test --test embed_test -p infiniloom

use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run `infiniloom` with the given arguments and return the raw `Output`.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args(args)
        .output()
        .expect("Failed to execute infiniloom binary")
}

/// Create a minimal temporary repo containing a single Rust source file
/// with enough code for the embed command to extract at least one chunk.
fn create_rust_repo() -> TempDir {
    let dir = TempDir::new().expect("Failed to create temp dir");

    fs::write(
        dir.path().join("lib.rs"),
        r#"/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiplies two numbers.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#,
    )
    .unwrap();

    dir
}

/// Create a temporary repo with Rust and Python source files.
fn create_multi_lang_repo() -> TempDir {
    let dir = TempDir::new().expect("Failed to create temp dir");

    fs::write(
        dir.path().join("math.rs"),
        r#"/// Subtracts b from a.
pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#,
    )
    .unwrap();

    fs::write(
        dir.path().join("utils.py"),
        r#"def greet(name: str) -> str:
    """Return a greeting."""
    return f"Hello, {name}!"
"#,
    )
    .unwrap();

    dir
}

/// Create a temporary git repo with an initial commit so that manifest/since
/// features work.
fn create_git_repo() -> TempDir {
    let dir = create_rust_repo();

    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    dir
}

/// Create a temp dir that contains a test file (to exercise --include-tests).
fn create_repo_with_tests() -> TempDir {
    let dir = TempDir::new().unwrap();

    fs::write(
        dir.path().join("lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 { a + b }
"#,
    )
    .unwrap();

    fs::create_dir_all(dir.path().join("tests")).unwrap();
    fs::write(
        dir.path().join("tests").join("test_lib.rs"),
        r#"#[test]
fn test_add() {
    assert_eq!(super::add(1, 2), 3);
}
"#,
    )
    .unwrap();

    dir
}

fn stdout_str(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr_str(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

/// Parse the entire stdout as JSON. Panics on invalid JSON.
fn parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).expect("stdout is not valid JSON")
}

/// Parse JSONL output: each line is a JSON object. Returns the vec of parsed values.
fn parse_jsonl(s: &str) -> Vec<serde_json::Value> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("invalid JSONL line: {e}\n{l}")))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests – basic operation
// ---------------------------------------------------------------------------

#[test]
fn test_embed_basic_jsonl_output() {
    let dir = create_rust_repo();
    let out = run(&["embed", dir.path().to_str().unwrap(), "--no-security-scan", "--quiet"]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let lines = parse_jsonl(&stdout_str(&out));
    assert!(lines.len() >= 3, "expect header + chunk(s) + summary, got {}", lines.len());

    // First line is always the header
    assert_eq!(lines[0]["type"], "header");

    // Last line is the summary
    let last = lines.last().unwrap();
    assert_eq!(last["type"], "summary");

    // At least one chunk line between header and summary
    let chunk_lines: Vec<_> = lines.iter().filter(|l| l["type"] == "chunk").collect();
    assert!(!chunk_lines.is_empty(), "should have at least one chunk");

    // Every chunk must have an id starting with "ec_"
    for c in &chunk_lines {
        let id = c["data"]["id"].as_str().unwrap();
        assert!(id.starts_with("ec_"), "chunk id should start with ec_, got: {id}");
    }
}

#[test]
fn test_embed_json_format() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));

    // Top-level keys
    assert!(json.get("version").is_some(), "missing version key");
    assert!(json.get("settings").is_some(), "missing settings key");
    assert!(json.get("chunks").is_some(), "missing chunks key");

    let chunks = json["chunks"].as_array().unwrap();
    assert!(!chunks.is_empty(), "should have at least one chunk");

    // Verify chunk structure
    let chunk = &chunks[0];
    assert!(chunk.get("id").is_some(), "chunk missing id");
    assert!(chunk.get("content").is_some(), "chunk missing content");
    assert!(chunk.get("tokens").is_some(), "chunk missing tokens");
    assert!(chunk.get("kind").is_some(), "chunk missing kind");
    assert!(chunk.get("source").is_some(), "chunk missing source");
}

// ---------------------------------------------------------------------------
// Tests – flags
// ---------------------------------------------------------------------------

#[test]
fn test_embed_max_tokens_flag() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--max-tokens",
        "5000",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(
        settings["max_tokens"].as_u64().unwrap(),
        5000,
        "max_tokens should be reflected in settings"
    );
}

#[test]
fn test_embed_min_tokens_flag() {
    let dir = create_rust_repo();

    // Use a very high min_tokens to filter out all chunks (they are small)
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--min-tokens",
        "999999",
        "--no-security-scan",
        "--quiet",
    ]);

    // The command may succeed with zero chunks or error with NoChunksGenerated;
    // either is acceptable depending on internal handling.
    let stdout = stdout_str(&out);
    if out.status.success() && !stdout.trim().is_empty() {
        let json = parse_json(&stdout);
        let chunks = json["chunks"].as_array().unwrap();
        assert_eq!(chunks.len(), 0, "high min_tokens should filter out all chunks");
    }
    // If it errors that is also fine -- it means NoChunksGenerated was raised.
}

#[test]
fn test_embed_no_security_scan_flag() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(
        settings["scan_secrets"].as_bool().unwrap(),
        false,
        "scan_secrets should be false with --no-security-scan"
    );
}

#[test]
fn test_embed_include_tests_flag() {
    let dir = create_repo_with_tests();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--include-tests",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(settings["include_tests"].as_bool().unwrap(), true, "include_tests should be true");
}

// ---------------------------------------------------------------------------
// Tests – output to file
// ---------------------------------------------------------------------------

#[test]
fn test_embed_output_to_file_jsonl() {
    let dir = create_rust_repo();
    let output_file = dir.path().join("out.jsonl");

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));
    assert!(output_file.exists(), "output file should be created");

    let content = fs::read_to_string(&output_file).unwrap();
    let lines = parse_jsonl(&content);
    assert!(lines.len() >= 3, "output file should contain header + chunks + summary");
    assert_eq!(lines[0]["type"], "header");
}

#[test]
fn test_embed_output_to_file_json() {
    let dir = create_rust_repo();
    let output_file = dir.path().join("out.json");

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));
    assert!(output_file.exists(), "output file should be created");

    let content = fs::read_to_string(&output_file).unwrap();
    let json = parse_json(&content);
    assert!(json.get("chunks").is_some(), "JSON output should have chunks key");
}

// ---------------------------------------------------------------------------
// Tests – manifest creation
// ---------------------------------------------------------------------------

#[test]
fn test_embed_creates_manifest() {
    let dir = create_git_repo();
    let manifest = dir.path().join(".infiniloom-embed.bin");

    // Ensure manifest does not exist yet
    assert!(!manifest.exists());

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "-o",
        dir.path().join("out.json").to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));
    assert!(manifest.exists(), "manifest file should be created after embed run");
}

// ---------------------------------------------------------------------------
// Tests – diff-only mode
// ---------------------------------------------------------------------------

#[test]
fn test_embed_diff_only_first_run() {
    let dir = create_git_repo();
    let output_file = dir.path().join("diff_out.json");

    // First run with --diff: since there is no manifest yet, all chunks are new
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--diff",
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let content = fs::read_to_string(&output_file).unwrap();
    let json = parse_json(&content);

    // On first run without a prior manifest, it should output the chunks
    // (no diff section, all treated as new).
    assert!(
        json.get("chunks").is_some() || json.get("diff").is_some(),
        "diff-only output should have chunks or diff key"
    );
}

#[test]
fn test_embed_diff_only_second_run_unchanged() {
    let dir = create_git_repo();

    // First full run to create manifest
    let out1 = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "-o",
        dir.path().join("run1.json").to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);
    assert!(out1.status.success(), "first embed failed: {}", stderr_str(&out1));

    // Second run with --diff (no code changes)
    let out2 = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--diff",
        "-o",
        dir.path().join("run2.json").to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);
    assert!(out2.status.success(), "second embed failed: {}", stderr_str(&out2));

    let content = fs::read_to_string(dir.path().join("run2.json")).unwrap();
    let json = parse_json(&content);

    // Should have a summary with diff info
    if let Some(summary) = json.get("summary") {
        // On an unchanged repo the diff should show zero added/modified/removed
        let added = summary["added"].as_u64().unwrap_or(0);
        let modified = summary["modified"].as_u64().unwrap_or(0);
        let removed = summary["removed"].as_u64().unwrap_or(0);
        assert_eq!(added, 0, "no new chunks expected");
        assert_eq!(modified, 0, "no modified chunks expected");
        assert_eq!(removed, 0, "no removed chunks expected");
    }
}

// ---------------------------------------------------------------------------
// Tests – streaming mode
// ---------------------------------------------------------------------------

#[test]
fn test_embed_streaming_jsonl() {
    let dir = create_rust_repo();
    let output_file = dir.path().join("stream.jsonl");

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--streaming",
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "streaming embed failed: {}", stderr_str(&out));
    assert!(output_file.exists(), "streaming output file should exist");

    let content = fs::read_to_string(&output_file).unwrap();
    let lines = parse_jsonl(&content);

    // Streaming JSONL has: header, chunk(s), footer
    assert!(lines.len() >= 2, "streaming should produce at least header + footer");

    let header = &lines[0];
    assert_eq!(header["type"], "header");
    assert_eq!(header["streaming"], true, "header should indicate streaming mode");

    let footer = lines.last().unwrap();
    assert_eq!(footer["type"], "footer");
}

#[test]
fn test_embed_streaming_rejects_json_format() {
    let dir = create_rust_repo();

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--streaming",
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(!out.status.success(), "streaming + json should fail");
    let err = stderr_str(&out);
    assert!(
        err.contains("Streaming mode only supports JSONL"),
        "error should mention JSONL requirement, got: {err}"
    );
}

#[test]
fn test_embed_streaming_rejects_diff_only() {
    let dir = create_rust_repo();

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--streaming",
        "--diff",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(!out.status.success(), "streaming + diff should fail");
}

// ---------------------------------------------------------------------------
// Tests – error cases
// ---------------------------------------------------------------------------

#[test]
fn test_embed_nonexistent_path() {
    let out = run(&["embed", "/tmp/__infiniloom_nonexistent_dir_1234__", "--quiet"]);

    assert!(!out.status.success(), "embed on non-existent path should fail");
}

#[test]
fn test_embed_generate_schema_pgvector() {
    let out = run(&["embed", "--generate-schema", "pgvector"]);

    assert!(out.status.success(), "generate-schema should succeed: {}", stderr_str(&out));
    let stdout = stdout_str(&out);
    assert!(stdout.contains("CREATE"), "pgvector schema should contain CREATE statements");
    assert!(stdout.contains("vector"), "pgvector schema should mention vector type");
}

#[test]
fn test_embed_generate_schema_unknown_type() {
    let out = run(&["embed", "--generate-schema", "unknown_db"]);

    assert!(!out.status.success(), "generate-schema with unknown type should fail");
    let err = stderr_str(&out);
    assert!(
        err.contains("Unsupported schema type"),
        "error should mention unsupported type, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Tests – multi-language repo
// ---------------------------------------------------------------------------

#[test]
fn test_embed_multi_language() {
    let dir = create_multi_lang_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let chunks = json["chunks"].as_array().unwrap();

    // We have a Rust function and a Python function - expect at least 2 chunks
    assert!(chunks.len() >= 2, "should have chunks from both languages, got {}", chunks.len());

    // Check that we see both languages
    let languages: Vec<&str> = chunks
        .iter()
        .filter_map(|c| c["source"]["language"].as_str())
        .collect();
    assert!(languages.iter().any(|l| *l == "Rust"), "should have Rust chunks");
    assert!(languages.iter().any(|l| *l == "Python"), "should have Python chunks");
}

// ---------------------------------------------------------------------------
// Tests – chunk content validation
// ---------------------------------------------------------------------------

#[test]
fn test_embed_chunk_content_has_expected_fields() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let chunks = json["chunks"].as_array().unwrap();
    assert!(!chunks.is_empty());

    for chunk in chunks {
        // Required fields
        let id = chunk["id"].as_str().unwrap();
        assert!(id.starts_with("ec_"), "id should start with ec_");

        assert!(chunk["tokens"].as_u64().unwrap() > 0, "tokens must be > 0");

        let source = &chunk["source"];
        assert!(source["file"].as_str().is_some(), "source.file required");
        assert!(source["language"].as_str().is_some(), "source.language required");

        // lines should be an array of two numbers
        let lines = source["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 2, "lines should be [start, end]");

        // content should be non-empty
        let content = chunk["content"].as_str().unwrap();
        assert!(!content.is_empty(), "content must not be empty");
    }
}

// ---------------------------------------------------------------------------
// Tests – determinism
// ---------------------------------------------------------------------------

#[test]
fn test_embed_deterministic_output() {
    let dir = create_rust_repo();

    let out1 = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);
    let out2 = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out1.status.success());
    assert!(out2.status.success());

    let json1 = parse_json(&stdout_str(&out1));
    let json2 = parse_json(&stdout_str(&out2));

    let chunks1 = json1["chunks"].as_array().unwrap();
    let chunks2 = json2["chunks"].as_array().unwrap();

    assert_eq!(chunks1.len(), chunks2.len(), "same input should produce same number of chunks");

    // Compare chunk IDs (which are content-addressable hashes)
    let ids1: Vec<&str> = chunks1.iter().map(|c| c["id"].as_str().unwrap()).collect();
    let ids2: Vec<&str> = chunks2.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids1, ids2, "chunk IDs should be deterministic");
}

// ---------------------------------------------------------------------------
// Tests – include/exclude patterns
// ---------------------------------------------------------------------------

#[test]
fn test_embed_include_pattern() {
    let dir = create_multi_lang_repo();

    // Include only .rs files
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "-i",
        "*.rs",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let chunks = json["chunks"].as_array().unwrap();

    // Should only have Rust chunks
    for chunk in chunks {
        let lang = chunk["source"]["language"].as_str().unwrap();
        assert_eq!(lang, "Rust", "include *.rs should only produce Rust chunks");
    }
}

#[test]
fn test_embed_exclude_pattern() {
    let dir = create_multi_lang_repo();

    // Exclude .py files
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "-e",
        "*.py",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let chunks = json["chunks"].as_array().unwrap();

    // Should only have Rust chunks (Python excluded)
    for chunk in chunks {
        let lang = chunk["source"]["language"].as_str().unwrap();
        assert_ne!(lang, "Python", "exclude *.py should filter out Python chunks");
    }
}

// ---------------------------------------------------------------------------
// Tests – verbose / json-stats
// ---------------------------------------------------------------------------

#[test]
fn test_embed_verbose_produces_stderr_output() {
    let dir = create_rust_repo();
    let output_file = dir.path().join("out.jsonl");

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "-v",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let err = stderr_str(&out);
    // Verbose mode should print statistics to stderr
    assert!(
        err.contains("Embedding Statistics") || err.contains("Total Chunks"),
        "verbose mode should print stats to stderr, got: {err}"
    );
}

#[test]
fn test_embed_json_stats() {
    let dir = create_rust_repo();
    let output_file = dir.path().join("out.jsonl");

    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "-o",
        output_file.to_str().unwrap(),
        "--no-security-scan",
        "-v",
        "--json-stats",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let err = stderr_str(&out);
    // json-stats outputs JSON to stderr, but there may be progress lines before it.
    // Find the last line that parses as JSON with a "total_chunks" key.
    let stats_line = err
        .lines()
        .rev()
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line.trim())
                .ok()
                .filter(|v| v.get("total_chunks").is_some())
        })
        .unwrap_or_else(|| {
            panic!("json-stats stderr should contain a JSON stats line, got: {err}")
        });
    assert!(stats_line.get("total_chunks").is_some(), "stats should contain total_chunks");
    assert!(stats_line.get("elapsed_ms").is_some(), "stats should contain elapsed_ms");
}

// ---------------------------------------------------------------------------
// Tests – no-imports / no-top-level
// ---------------------------------------------------------------------------

#[test]
fn test_embed_no_imports_flag() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-imports",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(
        settings["include_imports"].as_bool().unwrap(),
        false,
        "include_imports should be false with --no-imports"
    );
}

#[test]
fn test_embed_no_top_level_flag() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--no-top-level",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(
        settings["include_top_level"].as_bool().unwrap(),
        false,
        "include_top_level should be false with --no-top-level"
    );
}

// ---------------------------------------------------------------------------
// Tests – context-lines flag
// ---------------------------------------------------------------------------

#[test]
fn test_embed_context_lines_flag() {
    let dir = create_rust_repo();
    let out = run(&[
        "embed",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
        "--context-lines",
        "0",
        "--no-security-scan",
        "--quiet",
    ]);

    assert!(out.status.success(), "embed failed: {}", stderr_str(&out));

    let json = parse_json(&stdout_str(&out));
    let settings = &json["settings"];
    assert_eq!(
        settings["context_lines"].as_u64().unwrap(),
        0,
        "context_lines should reflect CLI flag"
    );
}

// ---------------------------------------------------------------------------
// Tests – empty repository
// ---------------------------------------------------------------------------

#[test]
fn test_embed_empty_directory() {
    let dir = TempDir::new().unwrap();

    let out = run(&["embed", dir.path().to_str().unwrap(), "--no-security-scan", "--quiet"]);

    // Empty directory should either fail with NoChunksGenerated or succeed with 0 chunks
    // Both are acceptable outcomes.
    if out.status.success() {
        let stdout = stdout_str(&out);
        if !stdout.trim().is_empty() {
            let lines = parse_jsonl(&stdout);
            let chunk_lines: Vec<_> = lines.iter().filter(|l| l["type"] == "chunk").collect();
            assert_eq!(chunk_lines.len(), 0, "empty dir should produce zero chunks");
        }
    }
    // If it failed, that is also acceptable (NoChunksGenerated error)
}
