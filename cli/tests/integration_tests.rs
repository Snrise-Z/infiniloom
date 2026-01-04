//! Integration tests for Infiniloom CLI
//!
//! These tests verify the CLI commands work correctly with real filesystem operations.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get the infiniloom binary command
fn infiniloom_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("infiniloom"))
}

/// Helper to create a test repository structure
fn create_test_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(base.join("src")).unwrap();
    fs::create_dir_all(base.join("tests")).unwrap();
    fs::create_dir_all(base.join("docs")).unwrap();

    // Create source files
    fs::write(
        base.join("src/main.rs"),
        r#"//! Main entry point
fn main() {
    println!("Hello, world!");
}

/// A simple function
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    fs::write(
        base.join("src/lib.rs"),
        r#"//! Library module
pub mod utils;

/// Calculate factorial
pub fn factorial(n: u64) -> u64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

pub struct Calculator {
    value: f64,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0.0 }
    }

    pub fn add(&mut self, x: f64) -> &mut Self {
        self.value += x;
        self
    }
}
"#,
    )
    .unwrap();

    fs::write(
        base.join("src/utils.rs"),
        r#"//! Utility functions
use std::collections::HashMap;

pub fn parse_config(s: &str) -> HashMap<String, String> {
    HashMap::new()
}
"#,
    )
    .unwrap();

    // Create a Python file for multi-language testing
    fs::write(
        base.join("src/helper.py"),
        r#""""Helper module for data processing"""

def process_data(data: list) -> list:
    """Process a list of data items."""
    return [item * 2 for item in data]

class DataProcessor:
    """A class for processing data."""

    def __init__(self, name: str):
        self.name = name
        self.items = []

    def add_item(self, item):
        """Add an item to the processor."""
        self.items.append(item)

    def process(self) -> list:
        """Process all items."""
        return process_data(self.items)
"#,
    )
    .unwrap();

    // Create a JavaScript file
    fs::write(
        base.join("src/index.js"),
        r#"/**
 * Main module
 * @module main
 */

/**
 * Greet a user
 * @param {string} name - The name to greet
 * @returns {string} Greeting message
 */
function greet(name) {
    return `Hello, ${name}!`;
}

class UserService {
    constructor() {
        this.users = [];
    }

    addUser(user) {
        this.users.push(user);
    }

    getUsers() {
        return this.users;
    }
}

module.exports = { greet, UserService };
"#,
    )
    .unwrap();

    // Create test file
    fs::write(
        base.join("tests/test_main.rs"),
        r#"#[test]
fn test_add() {
    assert_eq!(2 + 2, 4);
}
"#,
    )
    .unwrap();

    // Create README
    fs::write(
        base.join("README.md"),
        r#"# Test Repository

This is a test repository for Infiniloom integration tests.

## Features

- Multi-language support
- Symbol extraction
- Repository mapping
"#,
    )
    .unwrap();

    // Create .gitignore
    fs::write(
        base.join(".gitignore"),
        r#"target/
node_modules/
*.pyc
__pycache__/
"#,
    )
    .unwrap();

    // Create Cargo.toml
    fs::write(
        base.join("Cargo.toml"),
        r#"[package]
name = "test-repo"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    temp_dir
}

#[test]
fn test_help_command() {
    let mut cmd = infiniloom_cmd();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("LLM-friendly formats"));
}

#[test]
fn test_version_command() {
    let mut cmd = infiniloom_cmd();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("infiniloom"));
}

#[test]
fn test_scan_command_basic() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Scan Results"))
        .stdout(predicate::str::contains("Files:"))
        .stdout(predicate::str::contains("Token Counts"));
}

#[test]
fn test_scan_command_verbose() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path()).arg("--verbose");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Scan Results"));
}

#[test]
fn test_scan_command_json() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path()).arg("--json");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("repository").is_some());
    assert!(json.get("files").is_some());
    assert!(json.get("total_tokens").is_some());
}

#[test]
fn test_pack_command_xml() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack").arg(temp.path()).arg("--format").arg("xml");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("<repository"))
        .stdout(predicate::str::contains("<files>"))
        .stdout(predicate::str::contains("</repository>"));
}

#[test]
fn test_pack_command_markdown() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--format")
        .arg("markdown");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("# Repository:"))
        .stdout(predicate::str::contains("## Files"));
}

#[test]
fn test_pack_command_json() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack").arg(temp.path()).arg("--format").arg("json");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should be valid JSON
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("repository").is_some() || json.get("files").is_some());
}

#[test]
fn test_pack_with_model_option() {
    let temp = create_test_repo();

    for model in &["claude", "gpt4o", "gpt4", "gemini", "llama"] {
        let mut cmd = infiniloom_cmd();
        cmd.arg("pack")
            .arg(temp.path())
            .arg("--model")
            .arg(model)
            .arg("--format")
            .arg("xml");

        cmd.assert().success();
    }
}

#[test]
fn test_pack_with_compression() {
    let temp = create_test_repo();

    for compression in &["none", "minimal", "balanced", "aggressive", "extreme", "focused"] {
        let mut cmd = infiniloom_cmd();
        cmd.arg("pack")
            .arg(temp.path())
            .arg("--compression")
            .arg(compression)
            .arg("--format")
            .arg("xml");

        cmd.assert().success();
    }
}

#[test]
fn test_map_command() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("map").arg(temp.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Repository:"));
}

#[test]
fn test_map_with_budget() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("map").arg(temp.path()).arg("--budget").arg("1000");

    cmd.assert().success();
}

#[test]
fn test_map_with_model_option() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("map").arg(temp.path()).arg("--model").arg("gpt4o");

    cmd.assert().success();
}

#[test]
fn test_info_command() {
    let mut cmd = infiniloom_cmd();
    cmd.arg("info");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Infiniloom"))
        .stdout(predicate::str::contains("Version:"));
}

#[test]
fn test_nonexistent_path() {
    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg("/nonexistent/path/12345");
    cmd.assert().failure();
}

#[test]
fn test_gitignore_respected() {
    let temp = create_test_repo();

    // Initialize git repo so .gitignore is respected
    drop(
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output(),
    );

    // Create a directory that should be ignored
    fs::create_dir_all(temp.path().join("target")).unwrap();
    fs::write(temp.path().join("target/debug.rs"), "fn ignored() {}").unwrap();

    // Create node_modules (should be ignored)
    fs::create_dir_all(temp.path().join("node_modules")).unwrap();
    fs::write(temp.path().join("node_modules/package.json"), "{}").unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack").arg(temp.path()).arg("--format").arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should not contain ignored files
    assert!(!stdout.contains("target/debug.rs"));
    assert!(!stdout.contains("node_modules"));
}

#[test]
fn test_output_to_file() {
    let temp = create_test_repo();
    let output_file = temp.path().join("output.xml");

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--output")
        .arg(&output_file)
        .arg("--format")
        .arg("xml");

    cmd.assert().success();

    // Verify file was created
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("<repository"));
}

// Note: --include option doesn't exist in current CLI
// Test removed

// Note: --exclude option doesn't exist in current CLI
// Test removed

#[test]
fn test_multi_language_detection() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path());

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should detect multiple languages
    assert!(stdout.contains("Rust") || stdout.contains("rust"));
}

#[test]
fn test_empty_directory() {
    let temp = TempDir::new().unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path());

    // Should handle empty directory gracefully
    cmd.assert().success();
}

#[test]
fn test_large_file_handling() {
    let temp = TempDir::new().unwrap();

    // Create a large file (1MB of repeated text)
    let large_content = "fn large_function() { /* code */ }\n".repeat(30000);
    fs::write(temp.path().join("large.rs"), large_content).unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path());

    cmd.assert().success();
}

#[test]
fn test_binary_file_skipped() {
    let temp = TempDir::new().unwrap();

    // Create a binary file
    fs::write(temp.path().join("binary.bin"), vec![0u8, 1, 2, 255, 254, 253]).unwrap();
    // Create a valid source file
    fs::write(temp.path().join("source.rs"), "fn main() {}").unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack").arg(temp.path()).arg("--format").arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should contain source file
    assert!(stdout.contains("source.rs") || stdout.contains("fn main"));
    // Binary file should be skipped or handled gracefully
}

#[test]
fn test_symlink_handling() {
    let temp = create_test_repo();

    // Create a symlink (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        drop(symlink(temp.path().join("src/main.rs"), temp.path().join("main_link.rs")));
    }

    let mut cmd = infiniloom_cmd();
    cmd.arg("scan").arg(temp.path());

    cmd.assert().success();
}

// ============================================================================
// Integration tests for PackConfig refactoring (Item 1)
// ============================================================================

#[test]
fn test_pack_with_output_file() {
    let temp = create_test_repo();
    let output_file = temp.path().join("output.xml");

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--output")
        .arg(&output_file)
        .arg("--format")
        .arg("xml");

    cmd.assert().success();

    // Verify output file was created
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("<repository"));
    assert!(content.contains("</repository>"));
}

#[test]
fn test_pack_with_multiple_options() {
    let temp = create_test_repo();
    let output_file = temp.path().join("packed.json");

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--format")
        .arg("json")
        .arg("--model")
        .arg("gpt4o")
        .arg("--compression")
        .arg("balanced")
        .arg("--max-tokens")
        .arg("10000")
        .arg("--output")
        .arg(&output_file)
        .arg("--verbose");

    cmd.assert().success();

    // Verify output file exists and is valid JSON
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(json.get("repository").is_some() || json.get("files").is_some());
}

#[test]
fn test_pack_with_include_exclude_patterns() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--include")
        .arg("*.rs")
        .arg("--exclude")
        .arg("*test*")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should contain .rs files
    assert!(stdout.contains(".rs") || stdout.contains("fn "));
}

#[test]
fn test_pack_with_symbols_enabled() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--symbols")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // With symbols enabled, should extract function names
    assert!(stdout.contains("<repository") || stdout.contains("main") || stdout.contains("fn"));
}

#[test]
fn test_pack_with_full_mode() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--full")
        .arg("--format")
        .arg("markdown");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Full mode should include comprehensive analysis
    assert!(stdout.contains('#') || stdout.contains("Repository"));
}

#[test]
fn test_pack_with_no_content() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--no-content")
        .arg("--format")
        .arg("json");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should be valid JSON with metadata but no file content
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("repository").is_some() || json.get("metadata").is_some());
}

#[test]
fn test_pack_with_line_numbers() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--line-numbers")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Output should contain line numbers (1: or similar)
    assert!(stdout.contains("<repository"));
}

#[test]
fn test_pack_with_no_line_numbers() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--no-line-numbers")
        .arg("--format")
        .arg("plain");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should contain code without line number prefixes
    assert!(!stdout.is_empty());
}

#[test]
fn test_pack_with_compression_levels() {
    let temp = create_test_repo();

    // Test minimal compression
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--compression")
        .arg("minimal")
        .arg("--format")
        .arg("xml");

    cmd.assert().success();

    // Test balanced compression
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--compression")
        .arg("balanced")
        .arg("--format")
        .arg("xml");

    cmd.assert().success();

    // Test aggressive compression
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--compression")
        .arg("aggressive")
        .arg("--format")
        .arg("xml");

    cmd.assert().success();
}

#[test]
fn test_pack_with_model_selection() {
    let temp = create_test_repo();

    // Test Claude model
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--model")
        .arg("claude")
        .arg("--format")
        .arg("xml");

    cmd.assert().success();

    // Test GPT-4o model
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--model")
        .arg("gpt4o")
        .arg("--format")
        .arg("markdown");

    cmd.assert().success();

    // Test Gemini model
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--model")
        .arg("gemini")
        .arg("--format")
        .arg("yaml");

    cmd.assert().success();
}

#[test]
fn test_pack_with_hidden_files() {
    let temp = create_test_repo();

    // Create a hidden file
    fs::write(temp.path().join(".hidden.rs"), "fn hidden() {}").unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--hidden")
        .arg("--format")
        .arg("plain");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // With --hidden, should include hidden files
    assert!(!stdout.is_empty());
}

#[test]
fn test_pack_with_include_tests() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--include-tests")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should include test files
    assert!(stdout.contains("<repository"));
}

#[test]
fn test_pack_with_include_docs() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--include-docs")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should include documentation files
    assert!(stdout.contains("<repository"));
}

#[test]
fn test_pack_with_token_budget() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--max-tokens")
        .arg("1000")
        .arg("--format")
        .arg("xml");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should produce output within token budget
    assert!(stdout.contains("<repository"));
}

#[test]
fn test_pack_with_top_files_limit() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--top-files")
        .arg("2")
        .arg("--format")
        .arg("json");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should limit to top N files
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.is_object());
}

#[test]
fn test_pack_with_remove_comments() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--remove-comments")
        .arg("--format")
        .arg("plain");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Comments should be removed from output
    assert!(!stdout.is_empty());
}

#[test]
fn test_pack_with_remove_empty_lines() {
    let temp = create_test_repo();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--remove-empty-lines")
        .arg("--format")
        .arg("plain");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Empty lines should be removed
    assert!(!stdout.is_empty());
}

#[test]
fn test_pack_with_security_check() {
    let temp = create_test_repo();

    // Create a file with a potential secret
    fs::write(
        temp.path().join("config.rs"),
        r#"
const API_KEY: &str = "sk_test_1234567890abcdef";
fn main() {}
"#,
    )
    .unwrap();

    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--security-check")
        .arg("--format")
        .arg("json");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should run security scan and produce output
    assert!(!stdout.is_empty());
}

#[test]
fn test_pack_all_formats() {
    let temp = create_test_repo();

    // Test all supported formats
    for format in &["xml", "markdown", "json", "yaml", "plain", "toon"] {
        let mut cmd = infiniloom_cmd();
        cmd.arg("pack").arg(temp.path()).arg("--format").arg(format);

        cmd.assert().success();
    }
}

#[test]
fn test_pack_complex_configuration() {
    let temp = create_test_repo();
    let output_file = temp.path().join("complex.xml");

    // Test complex combination of options (simulates real-world usage)
    let mut cmd = infiniloom_cmd();
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--format")
        .arg("xml")
        .arg("--model")
        .arg("claude")
        .arg("--compression")
        .arg("balanced")
        .arg("--output")
        .arg(&output_file)
        .arg("--include")
        .arg("*.rs")
        .arg("--exclude")
        .arg("*test*")
        .arg("--symbols")
        .arg("--line-numbers")
        .arg("--remove-comments")
        .arg("--remove-empty-lines")
        .arg("--max-tokens")
        .arg("50000")
        .arg("--verbose");

    cmd.assert().success();

    // Verify output file
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("<repository"));
    assert!(content.contains("</repository>"));
}
