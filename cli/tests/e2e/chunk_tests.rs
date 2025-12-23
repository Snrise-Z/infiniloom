//! Chunk Command E2E Tests
//!
//! Tests user expectations for the `infiniloom chunk` command.

use super::helpers::*;

/// E2E-CHUNK-001: Chunk command outputs chunk headers and content
#[test]
fn test_chunk_outputs_to_stdout() {
    let temp = create_temp_project();

    let output = run_infiniloom(&["chunk", temp.path().to_str().unwrap()]);
    assert!(output.status.success(), "chunk command should succeed");

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Chunk 1/"), "chunk output should include chunk header");
    assert!(
        stdout.contains("main.rs") || stdout.contains("script.py"),
        "chunk output should include file content"
    );
}

/// E2E-CHUNK-002: Chunk command writes chunk files to output directory
#[test]
fn test_chunk_writes_output_dir() {
    let temp = create_temp_project();
    let output_dir = temp.path().join("chunks");

    let output = run_infiniloom(&[
        "chunk",
        temp.path().to_str().unwrap(),
        "--output",
        output_dir.to_str().unwrap(),
        "--format",
        "xml",
    ]);

    assert!(output.status.success(), "chunk --output should succeed");
    assert!(output_dir.exists(), "output directory should be created");

    let mut files = std::fs::read_dir(&output_dir)
        .expect("should read chunk output directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();

    assert!(
        files.iter().any(|p| {
            p.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("chunk_") && name.ends_with(".xml"))
                .unwrap_or(false)
        }),
        "chunk output directory should contain chunk_*.xml files"
    );
}

/// E2E-CHUNK-003: Chunk command outputs valid JSON when requested
#[test]
fn test_chunk_json_output_valid() {
    let temp = create_temp_project();

    let output = run_infiniloom(&["chunk", temp.path().to_str().unwrap(), "--format", "json"]);

    assert!(output.status.success(), "chunk --format json should succeed");

    let stdout = stdout_str(&output);
    assert!(is_valid_json(&stdout), "chunk JSON output should be valid JSON");
}

/// E2E-CHUNK-004: Chunk command outputs valid YAML when requested
#[test]
fn test_chunk_yaml_output_valid() {
    let temp = create_temp_project();

    let output = run_infiniloom(&["chunk", temp.path().to_str().unwrap(), "--format", "yaml"]);

    assert!(output.status.success(), "chunk --format yaml should succeed");

    let stdout = stdout_str(&output);
    assert!(is_valid_yaml(&stdout), "chunk YAML output should be valid YAML");
}

/// E2E-CHUNK-005: Symbol strategy works with chunk command
#[test]
fn test_chunk_symbol_strategy() {
    let temp = create_temp_project();

    let output = run_infiniloom(&[
        "chunk",
        temp.path().to_str().unwrap(),
        "--strategy",
        "symbol",
        "--max-tokens",
        "500",
    ]);

    assert!(output.status.success(), "chunk --strategy symbol should succeed");
    let stdout = stdout_str(&output);
    assert!(stdout.contains("Chunk 1/"), "symbol chunk output should include chunk header");
}
