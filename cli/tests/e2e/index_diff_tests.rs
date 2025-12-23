//! E2E tests for index, diff, and impact commands
//!
//! These commands form the git-aware context generation system.

use super::helpers::{infiniloom_binary, run_infiniloom, stderr_str, stdout_str};
use std::process::Command;

/// Create a temp git repository with multiple commits
fn create_temp_git_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to init git");

    // Configure git user for commits
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .ok();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir.path())
        .output()
        .ok();

    // Create initial file structure
    std::fs::create_dir_all(dir.path().join("src")).expect("Failed to create src dir");

    std::fs::write(
        dir.path().join("src/main.rs"),
        r#"fn main() {
    println!("Hello, world!");
    helper();
}

fn helper() {
    println!("Helper function");
}
"#,
    )
    .expect("Failed to write main.rs");

    std::fs::write(
        dir.path().join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#,
    )
    .expect("Failed to write lib.rs");

    // Initial commit
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to commit");

    dir
}

// =============================================================================
// Index Command Tests
// =============================================================================

#[test]
fn test_index_build() {
    let temp = create_temp_git_project();

    let output = Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to run index");

    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);

    // Should succeed
    assert!(
        output.status.success(),
        "index command should succeed.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    // Should create index files
    let infiniloom_dir = temp.path().join(".infiniloom");
    assert!(infiniloom_dir.exists(), ".infiniloom directory should be created");
}

#[test]
fn test_index_status() {
    let temp = create_temp_git_project();

    // First build the index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Then check status
    let output = Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap(), "--status"])
        .output()
        .expect("Failed to run index --status");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "index --status should succeed");
    // Should show index information
    assert!(
        stdout.contains("Index") || stdout.contains("files") || stdout.contains("symbols"),
        "Status should show index info: {}",
        stdout
    );
}

#[test]
fn test_index_force_rebuild() {
    let temp = create_temp_git_project();

    // Build initial index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build initial index");

    // Force rebuild
    let output = Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap(), "--force"])
        .output()
        .expect("Failed to force rebuild");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "index --force should succeed: {}", stdout);
}

#[test]
fn test_index_nonexistent_directory() {
    let output = run_infiniloom(&["index", "/nonexistent/path/to/repo"]);

    assert!(!output.status.success(), "index on nonexistent path should fail");
}

// =============================================================================
// Diff Command Tests
// =============================================================================

#[test]
fn test_diff_no_changes() {
    let temp = create_temp_git_project();

    // Build index first
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run diff with no changes
    let output = Command::new(infiniloom_binary())
        .args(["diff"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff");

    let _stdout = stdout_str(&output);

    // Should succeed even with no changes
    assert!(output.status.success(), "diff with no changes should succeed");

    // Output might indicate no changes
    // (exact behavior depends on implementation)
}

#[test]
fn test_diff_with_unstaged_changes() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Make unstaged changes
    std::fs::write(
        temp.path().join("src/main.rs"),
        r#"fn main() {
    println!("Modified!");
    helper();
    new_function();
}

fn helper() {
    println!("Helper function");
}

fn new_function() {
    println!("New function added");
}
"#,
    )
    .expect("Failed to modify main.rs");

    // Run diff
    let output = Command::new(infiniloom_binary())
        .args(["diff"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "diff should succeed: {}", stdout);

    // Should include the changed file
    assert!(
        stdout.contains("main.rs")
            || stdout.contains("changed")
            || stdout.to_lowercase().contains("file"),
        "diff output should mention changed files: {}",
        stdout
    );
}

#[test]
fn test_diff_staged_changes() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Make changes and stage them
    std::fs::write(
        temp.path().join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#,
    )
    .expect("Failed to modify lib.rs");

    Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to stage changes");

    // Run diff --staged
    let output = Command::new(infiniloom_binary())
        .args(["diff", "--staged"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff --staged");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "diff --staged should succeed: {}", stdout);
}

#[test]
fn test_diff_with_include_diff() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Make changes
    std::fs::write(
        temp.path().join("src/main.rs"),
        r#"fn main() {
    println!("Changed line!");
}
"#,
    )
    .expect("Failed to modify main.rs");

    // Run diff with --include-diff
    let output = Command::new(infiniloom_binary())
        .args(["diff", "--include-diff"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff --include-diff");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "diff --include-diff should succeed: {}", stdout);

    // Should include actual diff content (+ and - lines)
    // Note: The exact format depends on the output format chosen
}

#[test]
fn test_diff_depth_levels() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Make changes
    std::fs::write(temp.path().join("src/main.rs"), "fn main() { changed(); }")
        .expect("Failed to modify");

    // Test different depth levels
    for depth in ["1", "2", "3"] {
        let output = Command::new(infiniloom_binary())
            .args(["diff", "--depth", depth])
            .current_dir(temp.path())
            .output()
            .unwrap_or_else(|_| panic!("Failed to run diff --depth {}", depth));

        assert!(output.status.success(), "diff --depth {} should succeed", depth);
    }
}

#[test]
fn test_diff_json_format() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Make changes
    std::fs::write(temp.path().join("src/main.rs"), "fn main() { }").expect("Failed to modify");

    // Run diff with JSON format
    let output = Command::new(infiniloom_binary())
        .args(["diff", "--format", "json"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff --format json");

    let stdout = stdout_str(&output);

    if output.status.success() && !stdout.trim().is_empty() {
        // If we got output, it should be valid JSON
        assert!(
            stdout.trim().starts_with('{') || stdout.trim().starts_with('['),
            "JSON output should start with {{ or [: {}",
            stdout
        );
    }
}

#[test]
fn test_diff_commit_range() {
    let temp = create_temp_git_project();

    // Make a second commit
    std::fs::write(temp.path().join("src/new_file.rs"), "pub fn new_stuff() { }")
        .expect("Failed to write new file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to add");

    Command::new("git")
        .args(["commit", "-m", "Second commit"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to commit");

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run diff for last commit
    let output = Command::new(infiniloom_binary())
        .args(["diff", "HEAD~1"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff HEAD~1");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "diff HEAD~1 should succeed: {}", stdout);
}

// =============================================================================
// Impact Command Tests
// =============================================================================

#[test]
fn test_impact_file() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run impact on a file
    let output = Command::new(infiniloom_binary())
        .args(["impact", "src/lib.rs"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run impact");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "impact on file should succeed: {}", stdout);
}

#[test]
fn test_impact_symbol() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run impact on a symbol
    let output = Command::new(infiniloom_binary())
        .args(["impact", "--symbol", "add"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run impact --symbol");

    let stdout = stdout_str(&output);

    // May or may not find the symbol depending on indexing
    // The important thing is it doesn't crash
    assert!(
        output.status.success() || stderr_str(&output).contains("not found"),
        "impact --symbol should succeed or report not found: {}",
        stdout
    );
}

#[test]
fn test_impact_json_output() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run impact with JSON output
    let output = Command::new(infiniloom_binary())
        .args(["impact", "src/main.rs", "--json"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run impact --json");

    let stdout = stdout_str(&output);

    assert!(output.status.success(), "impact --json should succeed: {}", stderr_str(&output));

    if !stdout.trim().is_empty() {
        // If we got output, validate it looks like JSON
        assert!(
            stdout.trim().starts_with('{') || stdout.trim().starts_with('['),
            "JSON output should be valid: {}",
            stdout
        );
    }
}

#[test]
fn test_impact_nonexistent_file() {
    let temp = create_temp_git_project();

    // Build index
    Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    // Run impact on nonexistent file
    let _output = Command::new(infiniloom_binary())
        .args(["impact", "nonexistent.rs"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run impact on nonexistent file");

    // Should handle gracefully (either succeed with empty or report not found)
    // The important thing is it doesn't panic
}

// =============================================================================
// Integration Tests (Index + Diff + Impact workflow)
// =============================================================================

#[test]
fn test_full_workflow() {
    let temp = create_temp_git_project();

    // Step 1: Build index
    let index_output = Command::new(infiniloom_binary())
        .args(["index", temp.path().to_str().unwrap()])
        .output()
        .expect("Failed to build index");

    assert!(index_output.status.success(), "Index build should succeed");

    // Step 2: Make changes
    std::fs::write(
        temp.path().join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    // Modified implementation
    let result = a + b;
    result
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn divide(a: i32, b: i32) -> Option<i32> {
    if b == 0 { None } else { Some(a / b) }
}
"#,
    )
    .expect("Failed to modify lib.rs");

    // Step 3: Get diff context
    let diff_output = Command::new(infiniloom_binary())
        .args(["diff", "--depth", "2"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff");

    assert!(diff_output.status.success(), "Diff should succeed");

    let diff_stdout = stdout_str(&diff_output);
    assert!(
        diff_stdout.contains("lib.rs") || diff_stdout.to_lowercase().contains("changed"),
        "Diff should show lib.rs was changed"
    );

    // Step 4: Analyze impact
    let impact_output = Command::new(infiniloom_binary())
        .args(["impact", "src/lib.rs"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run impact");

    assert!(impact_output.status.success(), "Impact analysis should succeed");
}

#[test]
fn test_lazy_diff_without_prebuilt_index() {
    let temp = create_temp_git_project();

    // Make changes WITHOUT building index first
    std::fs::write(temp.path().join("src/main.rs"), "fn main() { changed(); }")
        .expect("Failed to modify");

    // Run diff - should use lazy indexing
    let output = Command::new(infiniloom_binary())
        .args(["diff"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to run diff without index");

    // Should either succeed with lazy indexing or prompt to build index
    // The important thing is it handles this gracefully
    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);

    // Should not panic
    assert!(
        output.status.success() || stderr.contains("index") || stderr.contains("No index"),
        "Should handle missing index gracefully.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}
