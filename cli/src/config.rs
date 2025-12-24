//! Configuration file loading and parsing
//!
//! Supports .infiniloom.yaml, .infiniloom.toml, .infiniloom.json formats
//! with both flat (legacy) and nested (engine's Config) structures.

use colored::Colorize;
use std::path::Path;

/// Loaded configuration from file
#[derive(Default)]
pub struct LoadedConfig {
    /// Additional exclude patterns from config
    pub exclude_patterns: Vec<String>,
    /// Additional include patterns from config
    pub include_patterns: Vec<String>,
    /// Output format from config
    pub format: Option<String>,
    /// Target model from config
    pub model: Option<String>,
    /// Token budget from config
    pub token_budget: Option<u32>,
    /// Compression level from config
    pub compression: Option<String>,
    /// Include tests from config
    pub include_tests: Option<bool>,
    /// Include docs from config
    pub include_docs: Option<bool>,
    /// Security check from config
    pub security_check: Option<bool>,
    /// Fail if secrets are detected
    pub fail_on_secrets: Option<bool>,
    /// Security allowlist patterns
    pub security_allowlist: Vec<String>,
    /// Custom security patterns
    pub security_custom_patterns: Vec<String>,
    /// Redact secrets in output
    pub redact_secrets: Option<bool>,
    /// Show line numbers
    pub line_numbers: Option<bool>,
    /// Show directory structure
    pub show_directory_structure: Option<bool>,
    /// Show file summary
    pub show_file_summary: Option<bool>,
    /// Remove empty lines
    pub remove_empty_lines: Option<bool>,
    /// Remove comments
    pub remove_comments: Option<bool>,
    /// Include hidden files
    pub include_hidden: Option<bool>,
    /// Maximum file size in bytes
    pub max_file_size: Option<u64>,
}

/// Output section of config file (supports both flat and nested formats)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct OutputConfigSection {
    format: Option<String>,
    model: Option<String>,
    compression: Option<String>,
    #[serde(alias = "token_budget")]
    token_budget: Option<u32>,
    line_numbers: Option<bool>,
    show_directory_structure: Option<bool>,
    show_file_summary: Option<bool>,
    remove_empty_lines: Option<bool>,
    remove_comments: Option<bool>,
}

/// Budget section of config file (legacy flat format)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct BudgetConfig {
    #[serde(alias = "max_tokens", alias = "token_budget")]
    tokens: Option<u32>,
}

/// Scan section (from engine's Config structure)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ScanConfigSection {
    /// Include patterns (from engine's nested format)
    include: Vec<String>,
    /// Exclude patterns (from engine's nested format)
    exclude: Vec<String>,
    /// Include hidden files
    include_hidden: Option<bool>,
    /// Maximum file size (supports "10MB", "500KB", etc. or raw bytes)
    max_file_size: Option<String>,
}

/// Security section (from engine's Config structure)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct SecurityConfigSection {
    /// Enable secret scanning
    scan_secrets: Option<bool>,
    /// Fail with error if secrets are detected
    fail_on_secrets: Option<bool>,
    /// Patterns to allowlist (won't be flagged)
    #[serde(default)]
    allowlist: Vec<String>,
    /// Additional secret patterns (regex)
    #[serde(default)]
    custom_patterns: Vec<String>,
    /// Redact secrets in output
    redact_secrets: Option<bool>,
}

/// Partial config structure for serde deserialization
/// Supports multiple naming conventions used in config files:
/// - Flat format: include/exclude at top level
/// - Nested format: scan.include/scan.exclude (from engine's Config)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct PartialConfig {
    /// Exclude patterns (flat format - legacy)
    #[serde(alias = "ignore", alias = "excludes")]
    exclude: Vec<String>,
    /// Include patterns (flat format - legacy)
    #[serde(alias = "includes")]
    include: Vec<String>,
    /// Scan configuration (nested format from engine's Config)
    scan: ScanConfigSection,
    /// Output configuration
    output: OutputConfigSection,
    /// Budget configuration (legacy flat format)
    budget: BudgetConfig,
    /// Security configuration (nested format)
    security: SecurityConfigSection,
    /// Include tests (flat format)
    include_tests: Option<bool>,
    /// Include docs (flat format)
    include_docs: Option<bool>,
    /// Security check (flat format, legacy)
    security_check: Option<bool>,
}

/// Load config file (.infiniloom.yaml, .infiniloom.toml, .infiniloom.json)
pub fn load_config_file(
    config_path: Option<&std::path::PathBuf>,
    repo_path: &Path,
) -> LoadedConfig {
    let mut config = LoadedConfig::default();

    // Try to load specified config file OR look for default config files
    if let Some(path) = config_path {
        // Use specified config file
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                parse_config_content(&content, path, &mut config);
            }
        }
        // Note: Don't return early - still need to load .infiniloomignore below
    } else {
        // Look for default config files (including .infiniloomrc for engine compatibility)
        let config_files = [
            ".infiniloomrc",
            ".infiniloom.yaml",
            ".infiniloom.yml",
            ".infiniloom.toml",
            ".infiniloom.json",
        ];
        for name in config_files {
            let path = repo_path.join(name);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // For .infiniloomrc, detect format from content
                    let effective_path = if name == ".infiniloomrc" {
                        // Detect format: JSON starts with {, YAML has :, else TOML
                        let trimmed = content.trim_start();
                        if trimmed.starts_with('{') {
                            path.with_extension("json")
                        } else if content.contains(':') {
                            path.with_extension("yaml")
                        } else {
                            path.with_extension("toml")
                        }
                    } else {
                        path.clone()
                    };
                    parse_config_content(&content, &effective_path, &mut config);
                    break;
                }
            }
        }
    }

    // Always load .infiniloomignore patterns (regardless of config file source)
    let ignore_path = repo_path.join(".infiniloomignore");
    if ignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ignore_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    config.exclude_patterns.push(line.to_owned());
                }
            }
        }
    }

    config
}

/// Parse config content using proper serde deserialization
/// Supports both flat format (legacy) and nested format (from engine's Config)
fn parse_config_content(content: &str, path: &Path, config: &mut LoadedConfig) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Try to parse and capture errors for warning
    let (partial, parse_error): (Option<PartialConfig>, Option<String>) = match ext {
        "yaml" | "yml" => match serde_yaml::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("YAML parse error: {}", e))),
        },
        "json" => match serde_json::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("JSON parse error: {}", e))),
        },
        "toml" => match toml::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("TOML parse error: {}", e))),
        },
        _ => (None, Some(format!("Unknown config file extension: {}", ext))),
    };

    // Print warning if parsing failed
    if let Some(error) = parse_error {
        eprintln!(
            "{} Failed to parse config file '{}': {}",
            "warning:".yellow().bold(),
            path.display(),
            error
        );
        eprintln!("         Using default configuration.");
        return;
    }

    if let Some(parsed) = partial {
        // Merge exclude patterns from both flat and nested formats
        config.exclude_patterns.extend(parsed.exclude);
        config.exclude_patterns.extend(parsed.scan.exclude);

        // Merge include patterns from both flat and nested formats
        config.include_patterns.extend(parsed.include);
        config.include_patterns.extend(parsed.scan.include);

        // Merge output settings (config provides defaults, CLI overrides)
        if config.format.is_none() {
            config.format = parsed.output.format;
        }
        if config.model.is_none() {
            config.model = parsed.output.model;
        }
        if config.compression.is_none() {
            config.compression = parsed.output.compression;
        }
        // Token budget: check both output.token_budget and budget.tokens
        if config.token_budget.is_none() {
            config.token_budget = parsed.output.token_budget.or(parsed.budget.tokens);
        }
        if config.include_tests.is_none() {
            config.include_tests = parsed.include_tests;
        }
        if config.include_docs.is_none() {
            config.include_docs = parsed.include_docs;
        }
        // Security check: check both flat and nested formats
        if config.security_check.is_none() {
            config.security_check = parsed.security_check.or(parsed.security.scan_secrets);
        }
        // Security options from nested format
        if config.fail_on_secrets.is_none() {
            config.fail_on_secrets = parsed.security.fail_on_secrets;
        }
        if config.redact_secrets.is_none() {
            config.redact_secrets = parsed.security.redact_secrets;
        }
        if config.security_allowlist.is_empty() {
            config.security_allowlist = parsed.security.allowlist;
        }
        if config.security_custom_patterns.is_empty() {
            config.security_custom_patterns = parsed.security.custom_patterns;
        }
        // Additional output options
        if config.line_numbers.is_none() {
            config.line_numbers = parsed.output.line_numbers;
        }
        if config.show_directory_structure.is_none() {
            config.show_directory_structure = parsed.output.show_directory_structure;
        }
        if config.show_file_summary.is_none() {
            config.show_file_summary = parsed.output.show_file_summary;
        }
        if config.remove_empty_lines.is_none() {
            config.remove_empty_lines = parsed.output.remove_empty_lines;
        }
        if config.remove_comments.is_none() {
            config.remove_comments = parsed.output.remove_comments;
        }
        // Scan options from nested format
        if config.include_hidden.is_none() {
            config.include_hidden = parsed.scan.include_hidden;
        }
        if config.max_file_size.is_none() {
            if let Some(ref size_str) = parsed.scan.max_file_size {
                config.max_file_size = Some(parse_size_string(size_str));
            }
        }
    }
}

/// Parse a size string like "10MB", "500KB", "1GB" into bytes
pub fn parse_size_string(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    let (num_part, suffix) = if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024u64)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024 * 1024u64)
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024u64)
    } else if s.ends_with('B') {
        (&s[..s.len() - 1], 1u64)
    } else {
        (s.as_str(), 1u64)
    };
    num_part.trim().parse::<u64>().unwrap_or(50 * 1024 * 1024) * suffix
}
