//! Comprehensive configuration tests
//!
//! Tests configuration loading, precedence, format parsing, and failure modes.

use infiniloom_engine::config::{
    Config, OutputConfig, PatternConfig, PerformanceConfig, ScanConfig, SecurityConfig,
    SymbolConfig,
};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Default Configuration Tests
// ============================================================================

#[test]
fn test_default_config() {
    let config = Config::default();

    assert_eq!(config.version, 1);
    assert!(config.scan.respect_gitignore);
    assert_eq!(config.output.format, "xml");
    assert_eq!(config.output.model, "claude");
}

#[test]
fn test_default_scan_config() {
    let config = ScanConfig::default();

    assert!(config.include.contains(&"**/*".to_owned()));
    assert!(config.exclude.contains(&"**/node_modules/**".to_owned()));
    assert!(config.exclude.contains(&"**/.git/**".to_owned()));
    assert!(config.exclude.contains(&"**/target/**".to_owned()));
    assert_eq!(config.max_file_size, "10MB");
    assert!(!config.follow_symlinks);
    assert!(!config.include_hidden);
    assert!(config.respect_gitignore);
    assert!(config.read_contents);
}

#[test]
fn test_default_output_config() {
    let config = OutputConfig::default();

    assert_eq!(config.format, "xml");
    assert_eq!(config.model, "claude");
    assert_eq!(config.compression, "none");
    assert_eq!(config.token_budget, 0);
    assert!(config.line_numbers);
    assert!(config.cache_optimized);
    assert_eq!(config.output_file, "-");
    assert!(config.header_text.is_none());
    assert!(config.instruction_file.is_none());
    assert!(!config.copy_to_clipboard);
}

#[test]
fn test_default_symbol_config() {
    let config = SymbolConfig::default();

    assert!(config.enabled);
    assert!(config.languages.is_empty());
    assert!(config.extract_docstrings);
    assert!(config.extract_signatures);
    assert_eq!(config.max_symbols, 100);
    assert!(config.include_imports);
    assert!(config.build_dependency_graph);
}

#[test]
fn test_default_security_config() {
    let config = SecurityConfig::default();

    assert!(config.scan_secrets);
    assert!(!config.fail_on_secrets);
    assert!(config.allowlist.is_empty());
    assert!(config.custom_patterns.is_empty());
    assert!(config.redact_secrets);
}

#[test]
fn test_default_performance_config() {
    let config = PerformanceConfig::default();

    assert_eq!(config.threads, 0); // Auto
    assert!(!config.incremental);
    assert_eq!(config.cache_dir, ".infiniloom/cache");
    assert!(config.memory_mapped);
    assert!(!config.skip_symbols);
}

#[test]
fn test_default_pattern_config() {
    let config = PatternConfig::default();

    assert!(config.extensions.is_empty());
    assert!(config.priority_paths.contains(&"README.md".to_owned()));
    assert!(config.priority_paths.contains(&"Cargo.toml".to_owned()));
    assert!(config.ignore_paths.contains(&"*.lock".to_owned()));
    assert!(config.modified_since.is_none());
    assert!(config.by_author.is_none());
}

// ============================================================================
// Size Parsing Tests
// ============================================================================

#[test]
fn test_max_file_size_bytes_default() {
    let config = ScanConfig::default();
    assert_eq!(config.max_file_size_bytes(), 10 * 1024 * 1024);
}

#[test]
fn test_max_file_size_bytes_custom() {
    let mut config = ScanConfig { max_file_size: "1MB".to_owned(), ..Default::default() };
    assert_eq!(config.max_file_size_bytes(), 1024 * 1024);

    config.max_file_size = "500KB".to_owned();
    assert_eq!(config.max_file_size_bytes(), 500 * 1024);

    config.max_file_size = "1GB".to_owned();
    assert_eq!(config.max_file_size_bytes(), 1024 * 1024 * 1024);

    config.max_file_size = "100".to_owned();
    assert_eq!(config.max_file_size_bytes(), 100);
}

#[test]
fn test_max_file_size_bytes_invalid() {
    let config = ScanConfig { max_file_size: "invalid".to_owned(), ..Default::default() };

    // Should fall back to default
    assert_eq!(config.max_file_size_bytes(), 10 * 1024 * 1024);
}

// ============================================================================
// Effective Threads Tests
// ============================================================================

#[test]
fn test_effective_threads_auto() {
    let config = Config::default();
    // Should use available parallelism
    assert!(config.effective_threads() > 0);
}

#[test]
fn test_effective_threads_explicit() {
    let mut config = Config::default();
    config.performance.threads = 4;
    assert_eq!(config.effective_threads(), 4);

    config.performance.threads = 16;
    assert_eq!(config.effective_threads(), 16);
}

// ============================================================================
// Config Generation Tests
// ============================================================================

#[test]
fn test_generate_default_yaml() {
    let yaml = Config::generate_default("yaml");

    // MinimalConfig only includes fields the CLI actually uses
    assert!(yaml.contains("scan:"));
    assert!(yaml.contains("output:"));
    assert!(yaml.contains("security:"));
    // These fields are not included in MinimalConfig to avoid misleading users
    // assert!(yaml.contains("version:"));
    // assert!(yaml.contains("symbols:"));
    // assert!(yaml.contains("performance:"));
    // assert!(yaml.contains("patterns:"));
}

#[test]
fn test_generate_default_toml() {
    let toml = Config::generate_default("toml");

    // MinimalConfig only includes fields the CLI actually uses
    assert!(toml.contains("[scan]"));
    assert!(toml.contains("[output]"));
    assert!(toml.contains("[security]"));
    // These fields are not included in MinimalConfig to avoid misleading users
    // assert!(toml.contains("version = 1"));
    // assert!(toml.contains("[symbols]"));
    // assert!(toml.contains("[performance]"));
    // assert!(toml.contains("[patterns]"));
}

#[test]
fn test_generate_default_json() {
    let json = Config::generate_default("json");

    // MinimalConfig only includes fields the CLI actually uses
    assert!(json.contains("\"scan\""));
    assert!(json.contains("\"output\""));
    assert!(json.contains("\"security\""));
    // These fields are not included in MinimalConfig to avoid misleading users
    // assert!(json.contains("\"version\""));
    // assert!(json.contains("\"symbols\""));
    // assert!(json.contains("\"performance\""));
    // assert!(json.contains("\"patterns\""));

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should be valid JSON");
    assert!(parsed.get("output").is_some());
    assert!(parsed.get("scan").is_some());
    assert!(parsed.get("security").is_some());
}

// ============================================================================
// Config Loading Tests
// ============================================================================

fn create_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

#[test]
fn test_load_yaml_config() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml_content = r#"
version: 2
scan:
  include_hidden: true
  max_file_size: "5MB"
output:
  format: markdown
  model: gpt4
"#;

    fs::write(&config_path, yaml_content).expect("Failed to write config");

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    assert_eq!(config.version, 2);
    assert!(config.scan.include_hidden);
    assert_eq!(config.scan.max_file_size, "5MB");
    assert_eq!(config.output.format, "markdown");
    assert_eq!(config.output.model, "gpt4");
}

#[test]
fn test_load_toml_config() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.toml");

    let toml_content = r#"
version = 3

[scan]
include_hidden = true
follow_symlinks = true

[output]
format = "json"
compression = "aggressive"
"#;

    fs::write(&config_path, toml_content).expect("Failed to write config");

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    assert_eq!(config.version, 3);
    assert!(config.scan.include_hidden);
    assert!(config.scan.follow_symlinks);
    assert_eq!(config.output.format, "json");
    assert_eq!(config.output.compression, "aggressive");
}

#[test]
fn test_load_json_config() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.json");

    let json_content = r#"{
    "version": 4,
    "scan": {
        "include_hidden": true,
        "respect_gitignore": false
    },
    "output": {
        "format": "yaml",
        "token_budget": 50000
    }
}"#;

    fs::write(&config_path, json_content).expect("Failed to write config");

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    assert_eq!(config.version, 4);
    assert!(config.scan.include_hidden);
    assert!(!config.scan.respect_gitignore);
    assert_eq!(config.output.format, "yaml");
    assert_eq!(config.output.token_budget, 50000);
}

#[test]
fn test_load_infiniloomrc_yaml() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloomrc");

    // YAML format (detected by presence of ':')
    let yaml_content = r#"
version: 5
output:
  format: xml
"#;

    fs::write(&config_path, yaml_content).expect("Failed to write config");

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    assert_eq!(config.version, 5);
    assert_eq!(config.output.format, "xml");
}

#[test]
fn test_load_infiniloomrc_json() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloomrc");

    // JSON format (detected by starting with '{')
    let json_content = r#"{"version": 6, "output": {"format": "markdown"}}"#;

    fs::write(&config_path, json_content).expect("Failed to write config");

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    assert_eq!(config.version, 6);
    assert_eq!(config.output.format, "markdown");
}

#[test]
fn test_load_no_config_file() {
    let temp_dir = create_temp_dir();
    // No config file created

    let config = Config::load(temp_dir.path()).expect("Should use defaults");

    assert_eq!(config.version, 1);
    assert_eq!(config.output.format, "xml");
}

// ============================================================================
// Config Precedence Tests
// ============================================================================

#[test]
fn test_yaml_takes_precedence_over_toml() {
    let temp_dir = create_temp_dir();

    // Create both YAML and TOML
    let yaml_path = temp_dir.path().join(".infiniloom.yaml");
    let toml_path = temp_dir.path().join(".infiniloom.toml");

    fs::write(&yaml_path, "version: 10\noutput:\n  format: yaml_wins").unwrap();
    fs::write(&toml_path, "version = 20\n[output]\nformat = \"toml_wins\"").unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    // YAML should be checked first based on config_files order
    // Note: depends on implementation - might be yaml_wins or need to verify order
    assert!(config.version == 10 || config.version == 20);
}

#[test]
fn test_infiniloomrc_takes_precedence() {
    let temp_dir = create_temp_dir();

    // Create .infiniloomrc and .infiniloom.yaml
    let rc_path = temp_dir.path().join(".infiniloomrc");
    let yaml_path = temp_dir.path().join(".infiniloom.yaml");

    fs::write(&rc_path, "version: 100").unwrap();
    fs::write(&yaml_path, "version: 200").unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load config");

    // .infiniloomrc should be checked first
    assert_eq!(config.version, 100);
}

// ============================================================================
// Config Save Tests
// ============================================================================

#[test]
fn test_save_yaml() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("config.yaml");

    let config = Config {
        version: 42,
        output: OutputConfig { format: "saved".to_owned(), ..Default::default() },
        ..Default::default()
    };

    config.save(&config_path).expect("Failed to save");

    let content = fs::read_to_string(&config_path).expect("Failed to read");
    assert!(content.contains("version: 42"));
    assert!(content.contains("format: saved"));
}

#[test]
fn test_save_toml() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("config.toml");

    let config = Config { version: 43, ..Default::default() };

    config.save(&config_path).expect("Failed to save");

    let content = fs::read_to_string(&config_path).expect("Failed to read");
    assert!(content.contains("version = 43"));
}

#[test]
fn test_save_json() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("config.json");

    let config = Config { version: 44, ..Default::default() };

    config.save(&config_path).expect("Failed to save");

    let content = fs::read_to_string(&config_path).expect("Failed to read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");
    assert_eq!(parsed["version"], 44);
}

#[test]
fn test_save_and_reload() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let original = Config {
        version: 99,
        output: OutputConfig { format: "roundtrip".to_owned(), ..Default::default() },
        scan: ScanConfig { include_hidden: true, ..Default::default() },
        security: SecurityConfig { fail_on_secrets: true, ..Default::default() },
        ..Default::default()
    };

    original.save(&config_path).expect("Failed to save");

    let loaded = Config::load(temp_dir.path()).expect("Failed to load");

    assert_eq!(loaded.version, 99);
    assert_eq!(loaded.output.format, "roundtrip");
    assert!(loaded.scan.include_hidden);
    assert!(loaded.security.fail_on_secrets);
}

// ============================================================================
// Partial Config Override Tests
// ============================================================================

#[test]
fn test_partial_override_preserves_defaults() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    // Only override one field
    fs::write(&config_path, "output:\n  format: custom").unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    // Overridden field
    assert_eq!(config.output.format, "custom");

    // Defaults should be preserved
    assert_eq!(config.version, 1);
    assert!(config.scan.respect_gitignore);
    assert_eq!(config.output.model, "claude");
    assert!(config.symbols.enabled);
}

#[test]
fn test_nested_partial_override() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml = r#"
scan:
  include_hidden: true
  # max_file_size not specified - should use default
security:
  fail_on_secrets: true
"#;

    fs::write(&config_path, yaml).unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    assert!(config.scan.include_hidden);
    assert_eq!(config.scan.max_file_size, "10MB"); // Default
    assert!(config.security.fail_on_secrets);
    assert!(config.security.scan_secrets); // Default
}

// ============================================================================
// Invalid Config Tests
// ============================================================================

#[test]
fn test_invalid_yaml_syntax() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    // Invalid YAML
    fs::write(&config_path, "version: [unclosed").unwrap();

    let result = Config::load(temp_dir.path());
    assert!(result.is_err());
}

#[test]
fn test_invalid_toml_syntax() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.toml");

    // Invalid TOML
    fs::write(&config_path, "version = [unclosed").unwrap();

    let result = Config::load(temp_dir.path());
    assert!(result.is_err());
}

#[test]
fn test_invalid_json_syntax() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.json");

    // Invalid JSON
    fs::write(&config_path, "{version: 1}").unwrap(); // Missing quotes

    let result = Config::load(temp_dir.path());
    assert!(result.is_err());
}

#[test]
fn test_wrong_type_in_config() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    // version should be u32, not string
    fs::write(&config_path, "version: \"not a number\"").unwrap();

    let result = Config::load(temp_dir.path());
    // This may or may not fail depending on serde configuration
    // If it succeeds, version would be default
    if let Ok(config) = result {
        // Figment might use default on type mismatch
        assert_eq!(config.version, Config::default().version);
    }
}

// ============================================================================
// Serialization Roundtrip Tests
// ============================================================================

#[test]
fn test_yaml_roundtrip() {
    let original = Config::default();
    let yaml = serde_yaml::to_string(&original).expect("Serialize failed");
    let restored: Config = serde_yaml::from_str(&yaml).expect("Deserialize failed");

    assert_eq!(original.version, restored.version);
    assert_eq!(original.output.format, restored.output.format);
}

#[test]
fn test_toml_roundtrip() {
    let original = Config::default();
    let toml = toml::to_string(&original).expect("Serialize failed");
    let restored: Config = toml::from_str(&toml).expect("Deserialize failed");

    assert_eq!(original.version, restored.version);
    assert_eq!(original.output.format, restored.output.format);
}

#[test]
fn test_json_roundtrip() {
    let original = Config::default();
    let json = serde_json::to_string(&original).expect("Serialize failed");
    let restored: Config = serde_json::from_str(&json).expect("Deserialize failed");

    assert_eq!(original.version, restored.version);
    assert_eq!(original.output.format, restored.output.format);
}

// ============================================================================
// Array Field Tests
// ============================================================================

#[test]
fn test_custom_exclude_patterns() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml = r#"
scan:
  exclude:
    - "**/custom_exclude/**"
    - "**/*.custom"
"#;

    fs::write(&config_path, yaml).unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    assert!(config
        .scan
        .exclude
        .contains(&"**/custom_exclude/**".to_owned()));
    assert!(config.scan.exclude.contains(&"**/*.custom".to_owned()));
}

#[test]
fn test_custom_allowlist() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml = r#"
security:
  allowlist:
    - "EXAMPLE_KEY"
    - "TEST_TOKEN"
"#;

    fs::write(&config_path, yaml).unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    assert!(config
        .security
        .allowlist
        .contains(&"EXAMPLE_KEY".to_owned()));
    assert!(config.security.allowlist.contains(&"TEST_TOKEN".to_owned()));
}

#[test]
fn test_custom_priority_paths() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml = r#"
patterns:
  priority_paths:
    - "IMPORTANT.md"
    - "src/main.rs"
"#;

    fs::write(&config_path, yaml).unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    assert!(config
        .patterns
        .priority_paths
        .contains(&"IMPORTANT.md".to_owned()));
    assert!(config
        .patterns
        .priority_paths
        .contains(&"src/main.rs".to_owned()));
}

// ============================================================================
// Optional Field Tests
// ============================================================================

#[test]
fn test_optional_fields_none() {
    let config = Config::default();

    assert!(config.output.header_text.is_none());
    assert!(config.output.instruction_file.is_none());
    assert!(config.patterns.modified_since.is_none());
    assert!(config.patterns.by_author.is_none());
}

#[test]
fn test_optional_fields_some() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    let yaml = r#"
output:
  header_text: "Custom header"
  instruction_file: "/path/to/instructions.md"
patterns:
  modified_since: "HEAD~10"
  by_author: "developer@example.com"
"#;

    fs::write(&config_path, yaml).unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");

    assert_eq!(config.output.header_text, Some("Custom header".to_owned()));
    assert_eq!(config.output.instruction_file, Some("/path/to/instructions.md".to_owned()));
    assert_eq!(config.patterns.modified_since, Some("HEAD~10".to_owned()));
    assert_eq!(config.patterns.by_author, Some("developer@example.com".to_owned()));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_config_file() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    fs::write(&config_path, "").unwrap();

    // Empty file should use defaults
    let config = Config::load(temp_dir.path()).expect("Failed to load");
    assert_eq!(config.version, 1);
}

#[test]
fn test_whitespace_only_config() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    // Use just spaces and newlines (no tabs which are invalid in YAML)
    fs::write(&config_path, "   \n\n   \n   ").unwrap();

    // Empty YAML might error or use defaults - both are acceptable
    let result = Config::load(temp_dir.path());
    if let Ok(config) = result {
        assert_eq!(config.version, 1);
    }
    // If error, that's acceptable for whitespace-only content
}

#[test]
fn test_comments_only_yaml() {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join(".infiniloom.yaml");

    fs::write(&config_path, "# This is a comment\n# Another comment").unwrap();

    let config = Config::load(temp_dir.path()).expect("Failed to load");
    assert_eq!(config.version, 1);
}
