//! Test helper utilities for E2E tests

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the test fixtures directory
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

/// Path to a specific fixture
pub fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// Get the path to the infiniloom binary using assert_cmd's binary finder
pub fn infiniloom_binary() -> PathBuf {
    // Use assert_cmd's cargo_bin! macro which properly handles test builds
    assert_cmd::cargo::cargo_bin!("infiniloom").to_path_buf()
}

/// Run infiniloom with given arguments
pub fn run_infiniloom(args: &[&str]) -> Output {
    Command::new(infiniloom_binary())
        .args(args)
        .output()
        .expect("Failed to execute infiniloom")
}

/// Run infiniloom pack on a directory
pub fn pack_directory(dir: &Path, extra_args: &[&str]) -> Output {
    let mut args = vec!["pack", dir.to_str().unwrap()];
    args.extend(extra_args);
    run_infiniloom(&args)
}

/// Run infiniloom scan on a directory
pub fn scan_directory(dir: &Path, extra_args: &[&str]) -> Output {
    let mut args = vec!["scan", dir.to_str().unwrap()];
    args.extend(extra_args);
    run_infiniloom(&args)
}

/// Get stdout as string
pub fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Get stderr as string
pub fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Check if output is valid XML
pub fn is_valid_xml(content: &str) -> bool {
    // Simple check: starts with <?xml or <
    content.trim().starts_with("<?xml") || content.trim().starts_with('<')
}

/// Check if output is valid JSON
pub fn is_valid_json(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content).is_ok()
}

/// Check if output is valid YAML
pub fn is_valid_yaml(content: &str) -> bool {
    serde_yaml::from_str::<serde_yaml::Value>(content).is_ok()
}

/// Check if output contains a file path
pub fn contains_file(content: &str, filename: &str) -> bool {
    content.contains(filename)
}

/// Check if output does NOT contain a pattern
pub fn not_contains(content: &str, pattern: &str) -> bool {
    !content.contains(pattern)
}

/// Count occurrences of a pattern
pub fn count_occurrences(content: &str, pattern: &str) -> usize {
    content.matches(pattern).count()
}

/// Create a temporary directory with test files
pub fn create_temp_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    // Create a simple Rust file
    std::fs::write(
        dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .expect("Failed to write main.rs");

    // Create a Python file
    std::fs::write(
        dir.path().join("script.py"),
        r#"
def greet(name):
    """Greet a person."""
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(greet("World"))
"#,
    )
    .expect("Failed to write script.py");

    dir
}

/// Create a temp project with secrets for security testing
pub fn create_temp_project_with_secrets() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    std::fs::write(
        dir.path().join("config.py"),
        r#"
# Configuration with secrets
AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE"
AWS_SECRET_ACCESS_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
API_KEY = "sk_live_FakeTestKey0000000000000"
"#,
    )
    .expect("Failed to write config.py");

    dir
}

/// Create a temp project with a .gitignore
pub fn create_temp_project_with_gitignore() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    // Create .gitignore
    std::fs::write(dir.path().join(".gitignore"), "ignored_file.txt\n*.log\n")
        .expect("Failed to write .gitignore");

    // Create files
    std::fs::write(dir.path().join("included.rs"), "fn main() {}")
        .expect("Failed to write included.rs");
    std::fs::write(dir.path().join("ignored_file.txt"), "This should be ignored")
        .expect("Failed to write ignored_file.txt");
    std::fs::write(dir.path().join("debug.log"), "Log content").expect("Failed to write debug.log");

    // Initialize git repo for gitignore to work
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .ok();

    dir
}

/// Create a temp project with hidden files
pub fn create_temp_project_with_hidden() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    std::fs::write(dir.path().join("visible.rs"), "fn main() {}")
        .expect("Failed to write visible.rs");
    std::fs::write(dir.path().join(".hidden.rs"), "fn hidden() {}")
        .expect("Failed to write .hidden.rs");

    dir
}

/// Create a temp project with binary files
pub fn create_temp_project_with_binary() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    std::fs::write(dir.path().join("source.rs"), "fn main() {}")
        .expect("Failed to write source.rs");

    // Create a fake binary file
    std::fs::write(dir.path().join("image.png"), [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        .expect("Failed to write image.png");

    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures_dir_exists() {
        assert!(fixtures_dir().exists());
    }

    #[test]
    fn test_rust_fixture_exists() {
        assert!(fixture_path("rust_project").exists());
    }
}
