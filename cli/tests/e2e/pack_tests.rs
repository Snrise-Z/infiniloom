//! Pack Command E2E Tests
//!
//! Tests user expectations for the `infiniloom pack` command.
//! Each test verifies: "As a user, I expect X to happen when I do Y."

use super::helpers::*;

/// E2E-PACK-001: Running `infiniloom pack .` produces valid output
#[test]
fn test_pack_produces_valid_output() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success(), "pack command should succeed");

    let stdout = stdout_str(&output);
    assert!(!stdout.is_empty(), "output should not be empty");
    // Default format is XML
    assert!(is_valid_xml(&stdout), "default output should be valid XML");
}

/// E2E-PACK-002: All source files are included in output
#[test]
fn test_pack_includes_all_source_files() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Check that our test files are included
    assert!(contains_file(&stdout, "main.rs"), "output should contain main.rs");
    assert!(contains_file(&stdout, "script.py"), "output should contain script.py");
}

/// E2E-PACK-003: Binary files are excluded automatically
#[test]
fn test_pack_excludes_binary_files() {
    let temp = create_temp_project_with_binary();
    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Source files should be included
    assert!(contains_file(&stdout, "source.rs"), "output should contain source.rs");
    // Binary files should be excluded
    assert!(
        not_contains(&stdout, "image.png"),
        "output should NOT contain image.png (binary file)"
    );
}

/// E2E-PACK-004: .gitignore is respected by default
#[test]
fn test_pack_respects_gitignore() {
    let temp = create_temp_project_with_gitignore();
    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Included file should be present
    assert!(contains_file(&stdout, "included.rs"), "output should contain included.rs");
    // Gitignored files should be excluded
    assert!(
        not_contains(&stdout, "ignored_file.txt"),
        "output should NOT contain ignored_file.txt"
    );
    assert!(not_contains(&stdout, "debug.log"), "output should NOT contain debug.log");
}

/// E2E-PACK-005: Hidden files excluded by default
#[test]
fn test_pack_excludes_hidden_files_by_default() {
    let temp = create_temp_project_with_hidden();
    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Visible files should be present
    assert!(contains_file(&stdout, "visible.rs"), "output should contain visible.rs");
    // Hidden files should be excluded by default
    assert!(not_contains(&stdout, ".hidden.rs"), "output should NOT contain .hidden.rs by default");
}

/// E2E-PACK-005b: Hidden files included with --hidden flag
#[test]
fn test_pack_includes_hidden_files_with_flag() {
    let temp = create_temp_project_with_hidden();
    let output = pack_directory(temp.path(), &["--hidden"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Both visible and hidden files should be present
    assert!(contains_file(&stdout, "visible.rs"), "output should contain visible.rs");
    assert!(
        contains_file(&stdout, ".hidden.rs"),
        "output should contain .hidden.rs with --hidden flag"
    );
}

/// E2E-PACK-006: Output can be written to a file
#[test]
fn test_pack_output_to_file() {
    let temp = create_temp_project();
    let output_file = temp.path().join("output.xml");

    let output = pack_directory(temp.path(), &["--output", output_file.to_str().unwrap()]);

    assert!(output.status.success(), "pack command should succeed");
    assert!(output_file.exists(), "output file should be created");

    let content = std::fs::read_to_string(&output_file).expect("should read output file");
    assert!(!content.is_empty(), "output file should not be empty");
    assert!(is_valid_xml(&content), "output file should contain valid XML");
}

/// E2E-PACK-007: Directory structure is shown in output
#[test]
fn test_pack_shows_directory_structure() {
    let rust_fixture = fixture_path("rust_project");
    if !rust_fixture.exists() {
        // Skip if fixture doesn't exist
        return;
    }

    let output = pack_directory(&rust_fixture, &[]);
    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Should show file paths that indicate structure
    assert!(
        contains_file(&stdout, "src/main.rs") || contains_file(&stdout, "main.rs"),
        "output should show file structure"
    );
}

/// E2E-PACK-008: Pack works on real multi-file Rust project
#[test]
fn test_pack_rust_fixture_project() {
    let rust_fixture = fixture_path("rust_project");
    if !rust_fixture.exists() {
        return; // Skip if fixture doesn't exist
    }

    let output = pack_directory(&rust_fixture, &[]);
    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Check for expected Rust project files
    assert!(contains_file(&stdout, "Cargo.toml"), "should include Cargo.toml");
    assert!(contains_file(&stdout, ".rs"), "should include Rust source files");
}

/// E2E-PACK-009: Pack works on Python project
#[test]
fn test_pack_python_fixture_project() {
    let py_fixture = fixture_path("python_project");
    if !py_fixture.exists() {
        return;
    }

    let output = pack_directory(&py_fixture, &[]);
    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(contains_file(&stdout, ".py"), "should include Python source files");
}

/// E2E-PACK-010: Pack works on TypeScript project
#[test]
fn test_pack_typescript_fixture_project() {
    let ts_fixture = fixture_path("typescript_project");
    if !ts_fixture.exists() {
        return;
    }

    let output = pack_directory(&ts_fixture, &[]);
    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(contains_file(&stdout, "package.json"), "should include package.json");
    assert!(contains_file(&stdout, ".ts"), "should include TypeScript source files");
}

/// E2E-PACK-011: Pack with include filter only includes matching files
#[test]
fn test_pack_with_include_filter() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--include", "*.rs"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Only .rs files should be included
    assert!(contains_file(&stdout, "main.rs"), "output should contain main.rs");
    assert!(
        not_contains(&stdout, "script.py"),
        "output should NOT contain script.py when filtering for *.rs"
    );
}

/// E2E-PACK-012: Pack with exclude filter removes matching files
#[test]
fn test_pack_with_exclude_filter() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--exclude", "*.py"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // .rs files should be included
    assert!(contains_file(&stdout, "main.rs"), "output should contain main.rs");
    // .py files should be excluded
    assert!(
        not_contains(&stdout, "script.py"),
        "output should NOT contain script.py when excluding *.py"
    );
}

/// E2E-PACK-013: Incremental cache creates repo cache file
#[test]
fn test_pack_creates_cache_file() {
    let temp = create_temp_project();
    let output_file = temp.path().join("cached.xml");

    let output =
        pack_directory(temp.path(), &["--cache", "--output", output_file.to_str().unwrap()]);

    assert!(output.status.success(), "pack --cache should succeed");
    assert!(output_file.exists(), "output file should be created");

    let cache_path = temp.path().join(".infiniloom/cache/repo.cache");
    assert!(cache_path.exists(), "cache file should be created at .infiniloom/cache/repo.cache");
}

/// E2E-PACK-014: Cache built without symbols is refreshed when symbols are requested
#[test]
fn test_cache_rescans_for_symbols() {
    let temp = create_temp_project();

    let initial = pack_directory(temp.path(), &["--cache", "--format", "json"]);
    assert!(initial.status.success(), "initial pack should succeed");

    let with_symbols = pack_directory(temp.path(), &["--cache", "--symbols", "--format", "json"]);
    assert!(with_symbols.status.success(), "pack --symbols should succeed");

    let stdout = stdout_str(&with_symbols);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    let files = value["repository"]["files"]
        .as_array()
        .expect("repository.files should be an array");
    let total_symbols: usize = files
        .iter()
        .map(|f| f["symbols"].as_array().map_or(0, |s| s.len()))
        .sum();

    assert!(total_symbols > 0, "expected symbols to be present after --symbols");
}

/// E2E-PACK-015: Cache removes deleted files after rescan
#[test]
fn test_cache_prunes_deleted_files() {
    let temp = create_temp_project();
    let extra_file = temp.path().join("extra.rs");
    std::fs::write(&extra_file, "fn extra() {}").expect("write extra.rs");

    let initial = pack_directory(temp.path(), &["--cache"]);
    assert!(initial.status.success(), "initial pack should succeed");

    std::fs::remove_file(&extra_file).expect("remove extra.rs");

    let updated = pack_directory(temp.path(), &["--cache"]);
    assert!(updated.status.success(), "pack after deletion should succeed");

    let cache_path = temp.path().join(".infiniloom/cache/repo.cache");
    let cache = infiniloom_engine::RepoCache::load(&cache_path).expect("load cache");

    assert!(!cache.files.contains_key("extra.rs"), "cache should not contain deleted files");
}

/// E2E-PACK-016: Cache detects content changes when size/mtime are unchanged
#[test]
fn test_cache_rescans_on_hash_change() {
    let temp = create_temp_project();
    let main_path = temp.path().join("main.rs");

    let initial = pack_directory(temp.path(), &["--cache", "--symbols", "--format", "json"]);
    assert!(initial.status.success(), "initial pack should succeed");

    let metadata = std::fs::metadata(&main_path).expect("read main.rs metadata");
    let original_mtime = filetime::FileTime::from_last_modification_time(&metadata);

    let original = std::fs::read_to_string(&main_path).expect("read main.rs");
    assert!(original.contains("add"), "expected test fixture to contain add()");

    let updated = original.replace("add", "sub");
    assert_eq!(original.len(), updated.len(), "replacement should preserve file size");
    std::fs::write(&main_path, updated).expect("write updated main.rs");
    filetime::set_file_mtime(&main_path, original_mtime).expect("reset mtime");

    let updated_output = pack_directory(temp.path(), &["--cache", "--symbols", "--format", "json"]);
    assert!(updated_output.status.success(), "pack after update should succeed");

    let stdout = stdout_str(&updated_output);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    let files = value["repository"]["files"]
        .as_array()
        .expect("repository.files should be an array");
    let main_file = files
        .iter()
        .find(|f| f["relative_path"] == "main.rs")
        .expect("main.rs should be present");
    let symbols = main_file["symbols"]
        .as_array()
        .expect("main.rs symbols should be an array");
    let symbol_names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();

    assert!(symbol_names.contains(&"sub"), "expected updated symbol name");
    assert!(!symbol_names.contains(&"add"), "expected old symbol name to be gone");
}
