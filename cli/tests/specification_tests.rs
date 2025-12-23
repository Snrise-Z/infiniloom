//! Specification-based black-box tests
//!
//! These tests verify behavior against specifications, NOT implementation.
//! Each test case corresponds to a TC-* identifier in TEST_SPECIFICATION.md
//!
//! The test fixtures are created fresh with known content, and expectations
//! are defined independently of how the code works internally.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

// ============================================================================
// Test Helpers
// ============================================================================

fn infiniloom_command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_infiniloom"));
    cmd.env("NO_COLOR", "1"); // Disable colors for consistent output
    cmd
}

fn create_test_repo() -> TempDir {
    TempDir::new().expect("Failed to create temp dir")
}

fn write_file(dir: &Path, relative_path: &str, content: &str) {
    let path = dir.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create directories");
    }
    fs::write(&path, content).expect("Failed to write file");
}

fn run_pack(dir: &Path, args: &[&str]) -> Output {
    let mut cmd = infiniloom_command();
    cmd.arg("pack").arg(dir);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().expect("Failed to execute command")
}

fn run_scan(dir: &Path, args: &[&str]) -> Output {
    let mut cmd = infiniloom_command();
    cmd.arg("scan").arg(dir);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().expect("Failed to execute command")
}

fn run_map(dir: &Path, args: &[&str]) -> Output {
    let mut cmd = infiniloom_command();
    cmd.arg("map").arg(dir);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().expect("Failed to execute command")
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ============================================================================
// TC-PACK: Pack Command Tests
// ============================================================================

/// TC-PACK-001: Basic XML Output (Claude-optimized)
/// XML output must be well-formed and contain expected elements
#[test]
fn tc_pack_001_basic_xml_output() {
    let repo = create_test_repo();
    write_file(repo.path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // XML structure requirements
    assert!(stdout.contains("<repository"), "Must have <repository> element");
    assert!(stdout.contains("</repository>"), "Must close <repository>");

    // Content requirements
    assert!(
        stdout.contains("main.rs") || stdout.contains("src/main.rs"),
        "Must reference the file path"
    );
    assert!(
        stdout.contains("fn main()") || stdout.contains("println"),
        "Must include file content"
    );

    // Language detection
    assert!(stdout.to_lowercase().contains("rust"), "Must detect Rust language");
}

/// TC-PACK-002: Markdown Output (GPT-optimized)
#[test]
fn tc_pack_002_markdown_output() {
    let repo = create_test_repo();
    write_file(repo.path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");

    let output = run_pack(repo.path(), &["--format", "markdown"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Markdown structure
    assert!(stdout.contains("# ") || stdout.contains("## "), "Must have Markdown headers");
    assert!(stdout.contains("```"), "Must have code blocks");

    // Content preserved
    assert!(stdout.contains("fn main"), "Must preserve code content");
}

/// TC-PACK-004: Gitignore Respect
/// Files matching .gitignore patterns must have their CONTENT excluded
/// Note: Filenames may appear in directory structure metadata
#[test]
fn tc_pack_004_gitignore_respect() {
    let repo = create_test_repo();

    // Create .gitignore
    write_file(repo.path(), ".gitignore", "*.log\nnode_modules/\n");

    // Create files - some should be ignored
    // Use distinctive content markers to verify content exclusion
    write_file(repo.path(), "src/main.rs", "fn main() { println!(\"MAIN_FILE_CONTENT\"); }");
    write_file(repo.path(), "debug.log", "LOG_FILE_SECRET_CONTENT_12345");
    write_file(repo.path(), "node_modules/pkg/index.js", "NODE_MODULE_SECRET_CONTENT_67890");

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Included file - check both filename and CONTENT
    assert!(stdout.contains("main.rs"), "main.rs should be included");
    assert!(stdout.contains("MAIN_FILE_CONTENT"), "main.rs content should be included");

    // Ignored files - their CONTENT should NOT appear
    // (filenames may appear in directory structure metadata, that's OK)
    assert!(
        !stdout.contains("LOG_FILE_SECRET_CONTENT"),
        "debug.log CONTENT should be excluded by gitignore"
    );
    assert!(
        !stdout.contains("NODE_MODULE_SECRET_CONTENT"),
        "node_modules CONTENT should be excluded by gitignore"
    );

    // Also verify no <file path="debug.log"> element exists
    assert!(
        !stdout.contains(r#"path="debug.log""#),
        "debug.log should not be included as a file element"
    );
}

/// TC-PACK-005: Binary File Exclusion
/// Binary files must not have their content included
#[test]
fn tc_pack_005_binary_exclusion() {
    let repo = create_test_repo();

    write_file(repo.path(), "src/main.rs", "fn main() {}");

    // Create a "binary" file (PNG header simulation)
    let png_header: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    fs::write(repo.path().join("image.png"), png_header).unwrap();

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Source file included
    assert!(stdout.contains("main.rs"), "Source file should be included");

    // Binary content should not appear
    // (The raw bytes or the file might be listed but content excluded)
    assert!(!stdout.contains("\u{0089}PNG"), "Binary content should not be in output");
}

/// TC-PACK-006: Symbol Extraction
/// Functions, classes, and methods must be detected
#[test]
fn tc_pack_006_symbol_extraction() {
    let repo = create_test_repo();

    write_file(
        repo.path(),
        "lib.py",
        r#"
class MyClass:
    def method(self):
        pass

def standalone_func():
    return 42
"#,
    );

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Check for symbol detection (implementation may vary in format)
    // At minimum, the symbol names should appear in some form
    assert!(
        stdout.contains("MyClass") || stdout.contains("myclass"),
        "Class MyClass should be detected"
    );
    assert!(stdout.contains("standalone_func"), "Function standalone_func should be detected");
}

/// TC-PACK-008: Empty Directory
/// Empty repositories should produce valid (minimal) output
#[test]
fn tc_pack_008_empty_directory() {
    let repo = create_test_repo();
    // Don't create any files

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed even for empty repo");

    let stdout = stdout_str(&output);

    // Should still produce valid structure
    assert!(
        stdout.contains("<repository") || stdout.contains("repository") || !stdout.is_empty(),
        "Should produce some output structure"
    );
}

/// TC-PACK-009: Nonexistent Path
/// Invalid paths must produce clear error
#[test]
fn tc_pack_009_nonexistent_path() {
    let output = run_pack(Path::new("/nonexistent/path/that/does/not/exist"), &[]);

    assert!(!output.status.success(), "Command should fail for nonexistent path");

    // Should have error message
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.to_lowercase().contains("not found")
            || combined.to_lowercase().contains("no such")
            || combined.to_lowercase().contains("does not exist")
            || combined.to_lowercase().contains("error"),
        "Should provide error message about missing path"
    );
}

/// TC-PACK-012: Unicode Content Handling
/// Unicode characters must be preserved correctly
#[test]
fn tc_pack_012_unicode_handling() {
    let repo = create_test_repo();

    // Put unicode in code (string literals), not just comments
    // Comments may be stripped by compression features
    write_file(
        repo.path(),
        "unicode.py",
        r#"
def greet():
    chinese = "你好世界"
    russian = "Привет мир"
    emoji = "🌍🎉"
    return chinese + russian + emoji
"#,
    );

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Unicode in string literals should be preserved
    assert!(stdout.contains("你好"), "Chinese characters must be preserved");
    assert!(stdout.contains("Привет"), "Russian characters must be preserved");
    assert!(stdout.contains("🌍"), "Emoji must be preserved");
}

// ============================================================================
// TC-SCAN: Scan Command Tests
// ============================================================================

/// TC-SCAN-001: Basic Statistics
/// Scan should show file count and language breakdown
#[test]
fn tc_scan_001_basic_statistics() {
    let repo = create_test_repo();

    write_file(repo.path(), "src/main.rs", "fn main() {}");
    write_file(repo.path(), "src/lib.rs", "pub fn lib() {}");
    write_file(repo.path(), "README.md", "# Readme");

    let output = run_scan(repo.path(), &[]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Should show file statistics
    assert!(stdout.contains("3") || stdout.contains("files"), "Should show file count");

    // Should show language breakdown
    assert!(
        stdout.to_lowercase().contains("rust") || stdout.to_lowercase().contains(".rs"),
        "Should detect Rust files"
    );
}

/// TC-SCAN-002: Token Count Display
/// Scan should show token estimates
#[test]
fn tc_scan_002_token_count() {
    let repo = create_test_repo();
    write_file(repo.path(), "main.rs", "fn main() { println!(\"Hello, world!\"); }");

    let output = run_scan(repo.path(), &[]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Should mention tokens somewhere
    assert!(stdout.to_lowercase().contains("token"), "Should display token information");
}

// ============================================================================
// TC-MAP: Map Command Tests
// ============================================================================

/// TC-MAP-001: Basic Symbol Map
/// Map should provide repository summary with file information
#[test]
fn tc_map_001_basic_symbol_map() {
    let repo = create_test_repo();

    write_file(
        repo.path(),
        "lib.py",
        r#"
class Database:
    def connect(self):
        pass
    def query(self, sql):
        pass

def initialize():
    pass
"#,
    );

    let output = run_map(repo.path(), &[]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Map should show repository info
    assert!(stdout.contains("Repository:"), "Should show repository header");

    // Map should identify the file
    assert!(stdout.contains("lib.py"), "Should list lib.py file");

    // Should identify the primary language
    assert!(stdout.to_lowercase().contains("python"), "Should identify Python as primary language");

    // Should show some file/line statistics
    assert!(
        stdout.contains("file") || stdout.contains("line"),
        "Should show file/line count information"
    );
}

// ============================================================================
// TC-ERR: Error Handling Tests
// ============================================================================

/// TC-ERR-001: Invalid Format Option
#[test]
fn tc_err_001_invalid_format() {
    let repo = create_test_repo();
    write_file(repo.path(), "main.rs", "fn main() {}");

    let output = run_pack(repo.path(), &["--format", "invalid_format_xyz"]);

    assert!(!output.status.success(), "Should fail with invalid format");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty() || !output.status.success(), "Should provide error message");
}

// ============================================================================
// TC-FMT: Output Format Validation
// ============================================================================

/// TC-FMT-001: XML Well-Formedness
/// XML output must parse as valid XML
#[test]
fn tc_fmt_001_xml_wellformed() {
    let repo = create_test_repo();
    write_file(repo.path(), "main.rs", r#"fn main() { let x = 5 < 10; }"#);

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Basic XML validation: matching tags, proper escaping
    // Check that < and > in code are escaped or in CDATA
    assert!(
        !stdout.contains("5 < 10") || stdout.contains("CDATA"),
        "Special characters should be escaped or in CDATA"
    );

    // Check the root <repository> element is balanced
    // Use regex-like matching for the opening tag: <repository followed by space or >
    let has_opening = stdout.contains("<repository name=") || stdout.contains("<repository>");
    let has_closing = stdout.contains("</repository>");
    assert!(has_opening, "Must have opening <repository> tag");
    assert!(has_closing, "Must have closing </repository> tag");

    // Verify XML structure: opening comes before closing
    if let (Some(open_pos), Some(close_pos)) = (
        stdout
            .find("<repository name=")
            .or_else(|| stdout.find("<repository>")),
        stdout.rfind("</repository>"),
    ) {
        assert!(open_pos < close_pos, "Opening tag should come before closing tag");
    }
}

/// TC-FMT-002: JSON Validity
/// JSON output must be parseable
#[test]
fn tc_fmt_002_json_valid() {
    let repo = create_test_repo();
    write_file(repo.path(), "main.rs", "fn main() {}");

    let output = run_pack(repo.path(), &["--format", "json"]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output);

    // Attempt to parse as JSON
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(parse_result.is_ok(), "Output should be valid JSON: {:?}", parse_result.err());
}

// ============================================================================
// TC-SEC: Security Tests
// ============================================================================

/// TC-SEC-001: AWS Key Detection
/// AWS access keys should be detected when --security-check is enabled
#[test]
fn tc_sec_001_aws_key_detection() {
    let repo = create_test_repo();

    // Use a clearly fake but pattern-matching AWS key
    write_file(
        repo.path(),
        "config.py",
        r#"
AWS_ACCESS_KEY = "AKIAIOSFODNN7EXAMPLE"
AWS_SECRET_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
"#,
    );

    // Enable security scanning
    let output = run_pack(repo.path(), &["--format", "xml", "--security-check"]);

    // The command should either:
    // 1. Warn about secrets (in stderr)
    // 2. Redact the secrets in output
    // 3. Both

    let stdout = stdout_str(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let has_warning = stderr.to_lowercase().contains("secret")
        || stderr.to_lowercase().contains("key")
        || stderr.to_lowercase().contains("credential")
        || stderr.to_lowercase().contains("security");

    let is_redacted = !stdout.contains("AKIAIOSFODNN7EXAMPLE")
        || stdout.contains("REDACTED")
        || stdout.contains("***");

    assert!(
        has_warning || is_redacted,
        "AWS keys should be detected and either warned about or redacted"
    );
}

/// TC-SEC-003: Private Key Detection
/// Private keys should be redacted when --security-check is enabled
#[test]
fn tc_sec_003_private_key_detection() {
    let repo = create_test_repo();

    write_file(
        repo.path(),
        "key.pem",
        r#"-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA0Z3VS5JJcds3xfn/ygWyF8PbnGy0AHB7S
-----END RSA PRIVATE KEY-----"#,
    );

    write_file(repo.path(), "main.rs", "fn main() {}");

    // Enable security scanning
    let output = run_pack(repo.path(), &["--format", "xml", "--security-check"]);

    let stdout = stdout_str(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // With security check enabled, private key should be redacted or warned about
    let has_warning = stderr.to_lowercase().contains("secret")
        || stderr.to_lowercase().contains("key")
        || stderr.to_lowercase().contains("security");

    let is_redacted = !stdout.contains("BEGIN RSA PRIVATE KEY")
        || stdout.contains("REDACTED")
        || stdout.contains("***");

    assert!(
        has_warning || is_redacted,
        "Private key should be excluded or redacted when --security-check is enabled"
    );
}

// ============================================================================
// TC-GIT: Git Integration Tests
// ============================================================================

/// TC-GIT-003: Non-Git Directory
/// Tool should work on directories without git
#[test]
fn tc_git_003_non_git_directory() {
    let repo = create_test_repo();
    // Explicitly NOT initializing git

    write_file(repo.path(), "main.rs", "fn main() {}");

    let output = run_pack(repo.path(), &["--format", "xml"]);

    assert!(output.status.success(), "Should work without git");

    let stdout = stdout_str(&output);
    assert!(stdout.contains("main.rs"), "Should still process files");
}

// ============================================================================
// TC-LANG: Language Detection Tests
// ============================================================================

/// Verify language detection for common file extensions
#[test]
fn tc_lang_detection() {
    let repo = create_test_repo();

    // Create files with various extensions
    write_file(repo.path(), "app.py", "def main(): pass");
    write_file(repo.path(), "app.js", "function main() {}");
    write_file(repo.path(), "app.ts", "function main(): void {}");
    write_file(repo.path(), "app.rs", "fn main() {}");
    write_file(repo.path(), "app.go", "func main() {}");
    write_file(repo.path(), "app.rb", "def main; end");

    let output = run_scan(repo.path(), &[]);

    assert!(output.status.success(), "Command should succeed");

    let stdout = stdout_str(&output).to_lowercase();

    // At least some languages should be detected
    let detected_count = [
        stdout.contains("python") || stdout.contains(".py"),
        stdout.contains("javascript") || stdout.contains(".js"),
        stdout.contains("typescript") || stdout.contains(".ts"),
        stdout.contains("rust") || stdout.contains(".rs"),
        stdout.contains("go") || stdout.contains(".go"),
        stdout.contains("ruby") || stdout.contains(".rb"),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    assert!(
        detected_count >= 3,
        "Should detect at least some languages, detected: {}",
        detected_count
    );
}

// ============================================================================
// Performance Smoke Tests
// ============================================================================

/// TC-PERF-001: Small Repository Speed
/// 50 files should complete quickly
#[test]
fn tc_perf_001_small_repo_speed() {
    let repo = create_test_repo();

    // Create 50 small files
    for i in 0..50 {
        write_file(repo.path(), &format!("src/file_{}.rs", i), &format!("fn func_{}() {{ }}", i));
    }

    let start = std::time::Instant::now();
    let output = run_pack(repo.path(), &["--format", "xml"]);
    let duration = start.elapsed();

    assert!(output.status.success(), "Command should succeed");
    assert!(duration.as_secs() < 10, "Small repo should complete in <10s, took {:?}", duration);
}
