//! Integration tests for incremental embed mode (--since)
//!
//! These tests verify the critical bug fix for Issue #200:
//! Files modified but generating no chunks should have their old chunks removed.

use std::fs;
use tempfile::TempDir;

#[test]
fn test_incremental_mode_removes_chunks_for_empty_files() {
    // This test verifies the fix for Issue #200:
    // When a file is modified but now generates no chunks (e.g., only comments),
    // its old chunks must be removed from the manifest to prevent data loss.

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Step 1: Create initial file with a function
    let file_path = repo_path.join("test.rs");
    fs::write(
        &file_path,
        r#"
/// Calculates the sum
pub fn calculate(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Step 2: Generate initial chunks with manifest
    let manifest_path = repo_path.join(".infiniloom-embed.bin");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args([
            "embed",
            repo_path.to_str().unwrap(),
            "--format",
            "json",
            "-o",
            repo_path.join("chunks1.json").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run infiniloom embed");

    assert!(
        output.status.success(),
        "Initial embed failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify manifest exists
    assert!(manifest_path.exists(), "Manifest should be created");

    // Verify at least one chunk was generated (for the calculate function)
    let chunks1: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path.join("chunks1.json")).unwrap()).unwrap();
    let initial_chunks = chunks1["chunks"].as_array().unwrap();
    assert!(initial_chunks.len() > 0, "Should have generated chunks for calculate function");

    // Find chunk ID for the calculate function
    let calculate_chunk_id = initial_chunks
        .iter()
        .find(|c| {
            c["source"]["symbol"]
                .as_str()
                .map(|s| s.contains("calculate"))
                .unwrap_or(false)
        })
        .and_then(|c| c["id"].as_str())
        .expect("Should have chunk for calculate function");

    println!("Initial chunk ID for calculate: {}", calculate_chunk_id);

    // Step 3: Modify file to only have comments (no symbols)
    fs::write(
        &file_path,
        r#"
// This file now only has comments
// All functions have been removed
// This should result in zero chunks
"#,
    )
    .unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Remove all functions, keep only comments"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Step 4: Run incremental embed (--since-manifest with --diff to get diff info)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args([
            "embed",
            repo_path.to_str().unwrap(),
            "--since-manifest",
            "--diff",
            "--format",
            "json",
            "-o",
            repo_path.join("chunks2.json").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run incremental embed");

    assert!(
        output.status.success(),
        "Incremental embed failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 5: Verify the old chunk was removed
    let chunks2: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path.join("chunks2.json")).unwrap()).unwrap();

    // Check if diff output shows removed chunk
    if let Some(diff) = chunks2.get("diff") {
        let removed = diff["removed"].as_array().unwrap();
        println!("Removed chunks: {:?}", removed);

        // The calculate chunk should be in the removed list
        let found_removed = removed
            .iter()
            .any(|c| c["id"].as_str().unwrap_or("") == calculate_chunk_id);

        assert!(
            found_removed,
            "The old calculate chunk should be marked as removed. \
             This is the bug fix for Issue #200: files modified to have no symbols \
             must have their old chunks removed."
        );
    } else {
        panic!("Diff output should be present for incremental mode");
    }

    // Step 6: Verify manifest is updated correctly
    // We can't run a full scan when there are no chunks (it will error).
    // Instead, verify the manifest file directly
    use infiniloom_engine::embedding::EmbedManifest;
    let manifest_path = repo_path.join(".infiniloom-embed.bin");

    if manifest_path.exists() {
        let manifest = EmbedManifest::load(&manifest_path).expect("Failed to load manifest");

        // Manifest should have zero chunks now (file has only comments)
        assert_eq!(
            manifest.chunks.len(),
            0,
            "After incremental update, manifest should have zero chunks \
             since the file now has only comments. Found {} chunks in manifest.",
            manifest.chunks.len()
        );

        println!("✓ Test passed: Incremental mode correctly removes chunks for empty files");
        println!("  Manifest has {} chunks (expected: 0)", manifest.chunks.len());
    } else {
        panic!("Manifest file should exist after incremental update");
    }
}

#[test]
fn test_incremental_mode_preserves_unchanged_files() {
    // Verify that files NOT in the git diff are preserved in the manifest

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Step 1: Create two files
    fs::write(repo_path.join("file1.rs"), r#"pub fn func1() { println!("one"); }"#).unwrap();

    fs::write(repo_path.join("file2.rs"), r#"pub fn func2() { println!("two"); }"#).unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Step 2: Generate initial chunks
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args([
            "embed",
            repo_path.to_str().unwrap(),
            "--format",
            "json",
            "-o",
            repo_path.join("chunks1.json").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run embed");

    assert!(output.status.success());

    let chunks1: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path.join("chunks1.json")).unwrap()).unwrap();
    let initial_count = chunks1["chunks"].as_array().unwrap().len();
    assert_eq!(initial_count, 2, "Should have 2 chunks initially");

    // Step 3: Modify only file1
    fs::write(
        repo_path.join("file1.rs"),
        r#"pub fn func1_modified() { println!("one modified"); }"#,
    )
    .unwrap();

    std::process::Command::new("git")
        .args(["add", "file1.rs"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Modify file1"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Step 4: Run incremental embed
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args([
            "embed",
            repo_path.to_str().unwrap(),
            "--since-manifest",
            "--format",
            "json",
            "-o",
            repo_path.join("chunks2.json").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run incremental embed");

    assert!(output.status.success());

    // Step 5: Full scan to verify manifest state
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_infiniloom"))
        .args([
            "embed",
            repo_path.to_str().unwrap(),
            "--format",
            "json",
            "-o",
            repo_path.join("chunks3.json").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run full embed");

    assert!(output.status.success());

    let chunks3: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo_path.join("chunks3.json")).unwrap()).unwrap();
    let final_chunks = chunks3["chunks"].as_array().unwrap();

    // Should still have 2 chunks (one from file1, one from unchanged file2)
    assert_eq!(final_chunks.len(), 2, "Should preserve chunk from unchanged file2");

    // Verify file2 chunk is unchanged
    let has_file2 = final_chunks.iter().any(|c| {
        c["source"]["file"]
            .as_str()
            .map(|s| s.contains("file2.rs"))
            .unwrap_or(false)
    });

    assert!(has_file2, "Should preserve file2 chunk in manifest");

    println!("✓ Test passed: Unchanged files are preserved in incremental mode");
}
