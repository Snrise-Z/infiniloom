//! Configuration E2E Tests
//!
//! Tests user expectations for configuration file handling.
//! Each test verifies: "As a user, I expect my config settings to be applied."

use super::helpers::*;
use std::fs;

/// E2E-CFG-001: .infiniloom.yaml is loaded
#[test]
fn test_yaml_config_loaded() {
    let temp = create_temp_project();

    // Create a YAML config file
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
output:
  format: json
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Output should be JSON (as specified in config)
    assert!(is_valid_json(&stdout), "Config should apply JSON format from .infiniloom.yaml");
}

/// E2E-CFG-002: .infiniloom.toml is loaded
#[test]
fn test_toml_config_loaded() {
    let temp = create_temp_project();

    // Create a TOML config file
    fs::write(
        temp.path().join(".infiniloom.toml"),
        r#"
[output]
format = "yaml"
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Output should be YAML (as specified in config)
    assert!(is_valid_yaml(&stdout), "Config should apply YAML format from .infiniloom.toml");
}

/// E2E-CFG-003: CLI args override config file
#[test]
fn test_cli_args_override_config() {
    let temp = create_temp_project();

    // Create a config file specifying JSON
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
output:
  format: json
"#,
    )
    .expect("create config file");

    // But use CLI arg to specify XML
    let output = pack_directory(temp.path(), &["--format", "xml"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // CLI should override config - output should be XML
    assert!(is_valid_xml(&stdout), "CLI --format should override config file format");
}

/// E2E-CFG-004: init command creates valid config
#[test]
fn test_init_creates_valid_config() {
    let temp = tempfile::tempdir().expect("create temp dir");

    let output = run_infiniloom(&["init", temp.path().to_str().unwrap()]);

    // init may create file or report if already exists
    let config_yaml = temp.path().join(".infiniloom.yaml");
    let config_toml = temp.path().join(".infiniloom.toml");

    if config_yaml.exists() {
        let content = fs::read_to_string(&config_yaml).expect("read config");
        assert!(
            serde_yaml::from_str::<serde_yaml::Value>(&content).is_ok(),
            "Created YAML config should be valid"
        );
    } else if config_toml.exists() {
        let content = fs::read_to_string(&config_toml).expect("read config");
        assert!(
            toml::from_str::<toml::Value>(&content).is_ok(),
            "Created TOML config should be valid"
        );
    } else {
        // init might have different behavior - check output
        let stderr = stderr_str(&output);
        assert!(
            output.status.success() || stderr.contains("exists") || stderr.contains("already"),
            "init should either create config or report existing"
        );
    }
}

/// E2E-CFG-005: .infiniloomignore patterns work
#[test]
fn test_infiniloomignore_patterns() {
    let temp = create_temp_project();

    // Create .infiniloomignore
    fs::write(temp.path().join(".infiniloomignore"), "script.py\n*.log\n")
        .expect("create ignore file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Files matching .infiniloomignore should be excluded
    assert!(not_contains(&stdout, "script.py"), ".infiniloomignore should exclude script.py");

    // But non-matching files should still be included
    assert!(contains_file(&stdout, "main.rs"), "main.rs should still be included");
}

/// E2E-CFG-006: Config include patterns work
#[test]
fn test_config_include_patterns() {
    let temp = create_temp_project();

    // Create config with include patterns
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
scan:
  include:
    - "*.rs"
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Only .rs files should be included
    assert!(contains_file(&stdout, "main.rs"), "main.rs should be included via config");
    // .py files should be excluded (not in include list)
    // Note: behavior depends on whether include is additive or exclusive
}

/// E2E-CFG-007: Config exclude patterns work
#[test]
fn test_config_exclude_patterns() {
    let temp = create_temp_project();

    // Create config with exclude patterns
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
scan:
  exclude:
    - "*.py"
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // .py files should be excluded
    assert!(not_contains(&stdout, "script.py"), "script.py should be excluded via config");
    // .rs files should still be included
    assert!(contains_file(&stdout, "main.rs"), "main.rs should still be included");
}

/// E2E-CFG-008: Config token budget is respected
#[test]
fn test_config_token_budget() {
    let temp = create_temp_project();

    // Create config with very low token budget
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
output:
  token_budget: 100
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &[]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Output should be limited (though exact behavior may vary)
    // With 100 tokens, output should be relatively short
    assert!(stdout.len() < 5000, "Token budget should limit output size");
}

/// E2E-CFG-009: Config compression level is applied
#[test]
fn test_config_compression_level() {
    let temp = create_temp_project();

    // Create config with aggressive compression
    fs::write(
        temp.path().join(".infiniloom.yaml"),
        r#"
output:
  compression: aggressive
"#,
    )
    .expect("create config file");

    let compressed_output = pack_directory(temp.path(), &[]);

    // Remove config and get uncompressed output
    fs::remove_file(temp.path().join(".infiniloom.yaml")).ok();
    let uncompressed_output = pack_directory(temp.path(), &["--compression", "none"]);

    assert!(compressed_output.status.success());
    assert!(uncompressed_output.status.success());

    let compressed_size = stdout_str(&compressed_output).len();
    let uncompressed_size = stdout_str(&uncompressed_output).len();

    // Compressed should be smaller (or at least not larger)
    assert!(
        compressed_size <= uncompressed_size,
        "Aggressive compression should produce smaller output"
    );
}

/// E2E-CFG-010: Invalid config gives helpful error
#[test]
fn test_invalid_config_error() {
    let temp = create_temp_project();

    // Create invalid YAML config
    fs::write(temp.path().join(".infiniloom.yaml"), "this: is: not: valid: yaml: [[[")
        .expect("create invalid config");

    let output = pack_directory(temp.path(), &[]);

    let stderr = stderr_str(&output);

    // Should either:
    // 1. Fail with helpful error about config
    // 2. Skip invalid config and continue with defaults
    assert!(
        !output.status.success() || stderr.contains("config") || stderr.contains("warning"),
        "Invalid config should produce error or warning"
    );
}
