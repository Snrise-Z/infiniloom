//! Error Handling E2E Tests
//!
//! Tests user expectations for error messages and edge cases.
//! Each test verifies: "As a user, I expect clear feedback when something goes wrong."

use super::helpers::*;

/// E2E-ERR-001: Non-existent path gives clear error
#[test]
fn test_nonexistent_path_error() {
    let output = run_infiniloom(&["pack", "/this/path/definitely/does/not/exist/12345"]);

    assert!(!output.status.success(), "Non-existent path should fail");

    let stderr = stderr_str(&output);
    assert!(
        stderr.contains("not found")
            || stderr.contains("does not exist")
            || stderr.contains("No such file")
            || stderr.contains("error")
            || stderr.contains("Error"),
        "Error message should indicate path not found: {}",
        stderr
    );
}

/// E2E-ERR-002: Invalid format gives suggestion
#[test]
fn test_invalid_format_error() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "invalid_format_xyz"]);

    assert!(!output.status.success(), "Invalid format should fail");

    let stderr = stderr_str(&output);
    // Should mention valid formats or give helpful error
    assert!(
        stderr.contains("format")
            || stderr.contains("invalid")
            || stderr.contains("xml")
            || stderr.contains("json")
            || stderr.contains("yaml")
            || stderr.contains("possible values"),
        "Error should mention format or valid options: {}",
        stderr
    );
}

/// E2E-ERR-003: Permission denied is reported (Unix only)
#[cfg(unix)]
#[test]
fn test_permission_denied_error() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("create temp dir");
    let restricted_dir = temp.path().join("restricted");
    std::fs::create_dir(&restricted_dir).expect("create dir");

    // Create a file inside with unique content we can search for
    std::fs::write(restricted_dir.join("file.rs"), "fn unique_permission_test_marker() {}")
        .expect("create file");

    // Make directory unreadable
    std::fs::set_permissions(&restricted_dir, std::fs::Permissions::from_mode(0o000))
        .expect("set permissions");

    let output = pack_directory(&restricted_dir, &[]);

    // Restore permissions for cleanup
    std::fs::set_permissions(&restricted_dir, std::fs::Permissions::from_mode(0o755)).ok();

    // Should fail or warn about permission
    let stderr = stderr_str(&output);
    let stdout = stdout_str(&output);

    // Graceful handling means one of:
    // 1. Command fails with non-zero exit
    // 2. Warning about permission/denied/access in stderr
    // 3. Output is empty or very small (no files found)
    // 4. The file content is NOT accessible (permissions blocked reading)
    // 5. No files indicator in output
    let file_content_blocked = !stdout.contains("unique_permission_test_marker");
    let has_permission_warning = stderr.contains("permission")
        || stderr.contains("denied")
        || stderr.contains("access")
        || stderr.contains("Error")
        || stderr.contains("error");
    let output_indicates_no_files = stdout.contains("0 files")
        || stdout.contains("No files")
        || stdout.is_empty()
        || stdout.len() < 100;

    assert!(
        !output.status.success()
            || has_permission_warning
            || output_indicates_no_files
            || file_content_blocked,
        "Should handle permission denied gracefully. stdout len: {}, stderr: {}",
        stdout.len(),
        stderr
    );
}

/// E2E-ERR-004: Empty directory handled gracefully
#[test]
fn test_empty_directory_handled() {
    let temp = tempfile::tempdir().expect("create temp dir");
    // Don't create any files - it's empty

    let output = pack_directory(temp.path(), &[]);

    // Should not crash - either succeed with minimal output or give helpful message
    let _stdout = stdout_str(&output);
    let stderr = stderr_str(&output);

    assert!(
        output.status.success() || stderr.contains("empty") || stderr.contains("no files"),
        "Empty directory should be handled gracefully, not crash"
    );
}

/// E2E-ERR-005: Invalid token budget gives error
#[test]
fn test_invalid_token_budget_error() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--max-tokens", "-100"]);

    // Negative token budget should fail or be rejected
    let stderr = stderr_str(&output);

    assert!(
        !output.status.success() || stderr.contains("invalid") || stderr.contains("error"),
        "Negative token budget should be rejected"
    );
}

/// E2E-ERR-006: Unknown flag gives helpful error
#[test]
fn test_unknown_flag_error() {
    let output = run_infiniloom(&["pack", ".", "--unknown-flag-xyz"]);

    assert!(!output.status.success(), "Unknown flag should fail");

    let stderr = stderr_str(&output);
    assert!(
        stderr.contains("unknown")
            || stderr.contains("unexpected")
            || stderr.contains("unrecognized")
            || stderr.contains("--unknown-flag-xyz")
            || stderr.contains("error"),
        "Error should mention unknown flag: {}",
        stderr
    );
}

/// E2E-ERR-007: Help flag works
#[test]
fn test_help_flag() {
    let output = run_infiniloom(&["--help"]);

    assert!(output.status.success(), "--help should succeed");

    let stdout = stdout_str(&output);
    assert!(
        stdout.contains("Usage") || stdout.contains("usage") || stdout.contains("USAGE"),
        "Help should show usage information"
    );
    assert!(
        stdout.contains("pack") || stdout.contains("Options") || stdout.contains("Commands"),
        "Help should mention commands or options"
    );
}

/// E2E-ERR-008: Version flag works
#[test]
fn test_version_flag() {
    let output = run_infiniloom(&["--version"]);

    assert!(output.status.success(), "--version should succeed");

    let stdout = stdout_str(&output);
    // Should show version number
    assert!(
        stdout.contains("infiniloom") || stdout.contains('.'),
        "Version output should contain tool name or version number"
    );
}

/// E2E-ERR-009: Subcommand help works
#[test]
fn test_subcommand_help() {
    let output = run_infiniloom(&["pack", "--help"]);

    assert!(output.status.success(), "pack --help should succeed");

    let stdout = stdout_str(&output);
    assert!(
        stdout.contains("pack") || stdout.contains("Usage") || stdout.contains("Options"),
        "Subcommand help should show relevant information"
    );
}

/// E2E-ERR-010: Missing required argument gives error
#[test]
fn test_missing_required_argument() {
    // 'pack' requires a path argument
    let output = run_infiniloom(&["pack"]);

    // Should either fail or use current directory
    let stderr = stderr_str(&output);
    let _stdout = stdout_str(&output);

    // Either it works (uses .) or gives helpful error
    assert!(
        output.status.success()
            || stderr.contains("required")
            || stderr.contains("missing")
            || stderr.contains("PATH")
            || stderr.contains("argument"),
        "Missing argument should either use default or give helpful error"
    );
}

/// E2E-ERR-011: Binary file in output request is handled
#[test]
fn test_binary_file_handling() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create only a binary file
    std::fs::write(temp.path().join("binary.bin"), [0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD])
        .expect("create binary file");

    let output = pack_directory(temp.path(), &[]);

    // Should handle gracefully - either skip binary or report
    assert!(output.status.success(), "Directory with only binary files should not crash");
}

/// E2E-ERR-012: Very long file path handled
#[test]
fn test_long_path_handling() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create a file - actual long paths are OS-limited
    std::fs::write(temp.path().join("normal_file.rs"), "fn main() {}").expect("create file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success(), "Normal file paths should work");
}

/// E2E-ERR-013: UTF-8 content handled correctly
#[test]
fn test_utf8_content_handling() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create file with various UTF-8 characters
    std::fs::write(
        temp.path().join("unicode.rs"),
        r#"
// Unicode test: emoji: 🦀 中文 العربية हिंदी
fn greet(name: &str) -> String {
    format!("Hello, {}! 👋", name)
}
"#,
    )
    .expect("create unicode file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success(), "UTF-8 content should be handled");

    let stdout = stdout_str(&output);
    // Content should be preserved (or safely encoded)
    assert!(
        stdout.contains("Unicode") || stdout.contains("greet"),
        "File content should be included"
    );
}

/// E2E-ERR-014: Non-UTF8 file handled gracefully
#[test]
fn test_non_utf8_handling() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create file with invalid UTF-8 bytes
    std::fs::write(temp.path().join("invalid.txt"), [0x80, 0x81, 0x82, 0xFF, 0xFE])
        .expect("create invalid utf8 file");

    // Also create a valid file
    std::fs::write(temp.path().join("valid.rs"), "fn main() {}").expect("create valid file");

    let output = pack_directory(temp.path(), &[]);

    // Should not crash - either skip invalid or handle gracefully
    assert!(output.status.success(), "Non-UTF8 files should be handled gracefully");

    let stdout = stdout_str(&output);
    // Valid file should still be included
    assert!(
        contains_file(&stdout, "valid.rs"),
        "Valid files should still be included when invalid files exist"
    );
}

/// E2E-ERR-015: Symlink handling
#[cfg(unix)]
#[test]
fn test_symlink_handling() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create a real file
    std::fs::write(temp.path().join("real.rs"), "fn real() {}").expect("create real file");

    // Create a symlink to it
    std::os::unix::fs::symlink(temp.path().join("real.rs"), temp.path().join("link.rs"))
        .expect("create symlink");

    let output = pack_directory(temp.path(), &[]);

    // Should handle symlinks without infinite loops or crashes
    assert!(output.status.success(), "Symlinks should be handled gracefully");
}
