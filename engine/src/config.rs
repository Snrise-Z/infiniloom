//! Configuration file support for Infiniloom
//!
//! Supports `.infiniloomrc`, `.infiniloom.yaml`, `.infiniloom.toml`, and `.infiniloom.json`
//! with environment variable override support.

use figment::{
    providers::{Env, Format, Json, Serialized, Toml, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Configuration version (for future compatibility)
    pub version: u32,

    /// Scanning options
    pub scan: ScanConfig,

    /// Output options
    pub output: OutputConfig,

    /// Symbol extraction options
    pub symbols: SymbolConfig,

    /// Security scanning options
    pub security: SecurityConfig,

    /// Performance options
    pub performance: PerformanceConfig,

    /// Include/exclude patterns
    pub patterns: PatternConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            scan: ScanConfig::default(),
            output: OutputConfig::default(),
            symbols: SymbolConfig::default(),
            security: SecurityConfig::default(),
            performance: PerformanceConfig::default(),
            patterns: PatternConfig::default(),
        }
    }
}

/// Scanning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    /// Include patterns (glob syntax)
    pub include: Vec<String>,

    /// Exclude patterns (glob syntax)
    pub exclude: Vec<String>,

    /// Maximum file size to include (in bytes, supports "100KB", "1MB" etc)
    pub max_file_size: String,

    /// Follow symbolic links
    pub follow_symlinks: bool,

    /// Include hidden files (starting with .)
    pub include_hidden: bool,

    /// Respect .gitignore files
    pub respect_gitignore: bool,

    /// Read file contents (false = metadata only)
    pub read_contents: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            include: vec!["**/*".to_owned()],
            exclude: vec![
                "**/node_modules/**".to_owned(),
                "**/.git/**".to_owned(),
                "**/target/**".to_owned(),
                "**/__pycache__/**".to_owned(),
                "**/dist/**".to_owned(),
                "**/build/**".to_owned(),
                "**/.venv/**".to_owned(),
                "**/venv/**".to_owned(),
                "**/*.min.js".to_owned(),
                "**/*.min.css".to_owned(),
            ],
            max_file_size: "10MB".to_owned(),
            follow_symlinks: false,
            include_hidden: false,
            respect_gitignore: true,
            read_contents: true,
        }
    }
}

impl ScanConfig {
    /// Parse max_file_size string to bytes
    pub fn max_file_size_bytes(&self) -> u64 {
        parse_size(&self.max_file_size).unwrap_or(10 * 1024 * 1024)
    }
}

/// Output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    /// Output format: xml, markdown, json, yaml
    pub format: String,

    /// Target LLM model: claude, gpt4o, gpt4, gemini, llama
    pub model: String,

    /// Compression level: none, minimal, balanced, aggressive, extreme
    pub compression: String,

    /// Maximum token budget (0 = unlimited)
    pub token_budget: u32,

    /// Include line numbers in code output
    pub line_numbers: bool,

    /// Optimize output for prompt caching (Claude)
    pub cache_optimized: bool,

    /// Output file path (- for stdout)
    pub output_file: String,

    /// Custom header text to include at the top of output
    pub header_text: Option<String>,

    /// Path to file containing custom instructions to include
    pub instruction_file: Option<String>,

    /// Copy output to clipboard after generation
    pub copy_to_clipboard: bool,

    /// Show token count tree/breakdown in output
    pub show_token_tree: bool,

    /// Include directory structure in output
    pub show_directory_structure: bool,

    /// Include file summary section
    pub show_file_summary: bool,

    /// Remove empty lines from code
    pub remove_empty_lines: bool,

    /// Remove comments from code
    pub remove_comments: bool,

    /// Number of top files to show in summary (0 = all)
    pub top_files_length: usize,

    /// Include empty directories in structure
    pub include_empty_directories: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: "xml".to_owned(),
            model: "claude".to_owned(),
            compression: "none".to_owned(),
            token_budget: 0,
            line_numbers: true,
            cache_optimized: true,
            output_file: "-".to_owned(),
            header_text: None,
            instruction_file: None,
            copy_to_clipboard: false,
            show_token_tree: false,
            show_directory_structure: true,
            show_file_summary: true,
            remove_empty_lines: false,
            remove_comments: false,
            top_files_length: 0,
            include_empty_directories: false,
        }
    }
}

/// Symbol extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolConfig {
    /// Enable symbol extraction
    pub enabled: bool,

    /// Languages to parse (empty = all supported)
    pub languages: Vec<String>,

    /// Extract docstrings/documentation
    pub extract_docstrings: bool,

    /// Extract function/method signatures
    pub extract_signatures: bool,

    /// Maximum number of symbols to include in repomap
    pub max_symbols: usize,

    /// Include imports in symbol graph
    pub include_imports: bool,

    /// Build dependency graph
    pub build_dependency_graph: bool,
}

impl Default for SymbolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: vec![],
            extract_docstrings: true,
            extract_signatures: true,
            max_symbols: 100,
            include_imports: true,
            build_dependency_graph: true,
        }
    }
}

/// Security scanning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Enable secret scanning
    pub scan_secrets: bool,

    /// Fail on secrets detected
    pub fail_on_secrets: bool,

    /// Patterns to allowlist (won't be flagged)
    pub allowlist: Vec<String>,

    /// Additional secret patterns (regex)
    pub custom_patterns: Vec<String>,

    /// Redact secrets in output
    pub redact_secrets: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            scan_secrets: true,
            fail_on_secrets: false,
            allowlist: vec![],
            custom_patterns: vec![],
            redact_secrets: true,
        }
    }
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    /// Number of threads (0 = auto)
    pub threads: usize,

    /// Enable incremental mode (cache results)
    pub incremental: bool,

    /// Cache directory
    pub cache_dir: String,

    /// Use memory-mapped I/O for large files
    pub memory_mapped: bool,

    /// Skip symbol extraction for faster scanning
    pub skip_symbols: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            threads: 0, // auto
            incremental: false,
            cache_dir: ".infiniloom/cache".to_owned(),
            memory_mapped: true,
            skip_symbols: false,
        }
    }
}

/// Pattern configuration for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternConfig {
    /// File extensions to include (empty = all)
    pub extensions: Vec<String>,

    /// Paths to always include (high priority)
    pub priority_paths: Vec<String>,

    /// Paths to always exclude (even if matched by include)
    pub ignore_paths: Vec<String>,

    /// Only include files modified after this git ref
    pub modified_since: Option<String>,

    /// Only include files by specific author
    pub by_author: Option<String>,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            extensions: vec![],
            priority_paths: vec![
                "README.md".to_owned(),
                "package.json".to_owned(),
                "Cargo.toml".to_owned(),
                "pyproject.toml".to_owned(),
            ],
            ignore_paths: vec!["*.lock".to_owned(), "*.sum".to_owned()],
            modified_since: None,
            by_author: None,
        }
    }
}

impl Config {
    /// Load configuration from default locations
    #[allow(clippy::result_large_err)]
    pub fn load(repo_path: &Path) -> Result<Self, ConfigError> {
        Self::load_with_profile(repo_path, None)
    }

    /// Load configuration with optional profile override
    #[allow(clippy::result_large_err)]
    pub fn load_with_profile(repo_path: &Path, profile: Option<&str>) -> Result<Self, ConfigError> {
        let mut figment = Figment::new().merge(Serialized::defaults(Config::default()));

        // Try loading from various config file locations
        let config_files = [
            repo_path.join(".infiniloomrc"),
            repo_path.join(".infiniloom.yaml"),
            repo_path.join(".infiniloom.yml"),
            repo_path.join(".infiniloom.toml"),
            repo_path.join(".infiniloom.json"),
            repo_path.join("infiniloom.yaml"),
            repo_path.join("infiniloom.toml"),
            repo_path.join("infiniloom.json"),
        ];

        for config_file in &config_files {
            if config_file.exists() {
                figment = match config_file.extension().and_then(|e| e.to_str()) {
                    Some("yaml") | Some("yml") => figment.merge(Yaml::file(config_file)),
                    Some("toml") => figment.merge(Toml::file(config_file)),
                    Some("json") => figment.merge(Json::file(config_file)),
                    None => {
                        // .infiniloomrc - try YAML first, then TOML
                        if let Ok(content) = std::fs::read_to_string(config_file) {
                            if content.trim_start().starts_with('{') {
                                figment.merge(Json::file(config_file))
                            } else if content.contains(':') {
                                figment.merge(Yaml::file(config_file))
                            } else {
                                figment.merge(Toml::file(config_file))
                            }
                        } else {
                            figment
                        }
                    },
                    _ => figment,
                };
                break; // Use first found config file
            }
        }

        // Check home directory for global config
        if let Some(home) = dirs::home_dir() {
            let global_config = home.join(".config/infiniloom/config.yaml");
            if global_config.exists() {
                figment = figment.merge(Yaml::file(global_config));
            }
        }

        // Environment variable overrides (INFINILOOM_*)
        figment = figment.merge(Env::prefixed("INFINILOOM_").split("__"));

        // Apply profile if specified
        if let Some(profile_name) = profile {
            figment = figment.select(profile_name);
        }

        figment.extract().map_err(ConfigError::ParseError)
    }

    /// Save configuration to a file
    #[allow(clippy::result_large_err)]
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = match path.extension().and_then(|e| e.to_str()) {
            Some("json") => serde_json::to_string_pretty(self)
                .map_err(|e| ConfigError::SerializeError(e.to_string()))?,
            Some("toml") => toml::to_string_pretty(self)
                .map_err(|e| ConfigError::SerializeError(e.to_string()))?,
            _ => serde_yaml::to_string(self)
                .map_err(|e| ConfigError::SerializeError(e.to_string()))?,
        };

        std::fs::write(path, content).map_err(ConfigError::IoError)
    }

    /// Generate a default configuration file
    /// Only includes fields that the CLI actually reads to avoid misleading users
    pub fn generate_default(format: &str) -> String {
        // Use a minimal config with only fields the CLI actually uses
        // This avoids confusion from fields that appear in config but are silently ignored
        let minimal = MinimalConfig::default();
        match format {
            "json" => serde_json::to_string_pretty(&minimal).unwrap_or_default(),
            "toml" => toml::to_string_pretty(&minimal).unwrap_or_default(),
            _ => serde_yaml::to_string(&minimal).unwrap_or_default(),
        }
    }
}

/// Minimal configuration with only fields the CLI actually uses
/// This prevents user confusion from config fields that are silently ignored
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinimalConfig {
    /// Output settings
    output: MinimalOutputConfig,
    /// Scan settings
    scan: MinimalScanConfig,
    /// Security settings
    security: MinimalSecurityConfig,
    /// Include test files
    #[serde(skip_serializing_if = "is_false")]
    include_tests: bool,
    /// Include documentation files
    #[serde(skip_serializing_if = "is_false")]
    include_docs: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinimalOutputConfig {
    /// Output format: xml, markdown, json
    format: String,
    /// Target model: claude, gpt4o, gpt4, gemini, llama
    model: String,
    /// Compression: none, minimal, balanced, aggressive, extreme, semantic
    compression: String,
    /// Token budget (0 = unlimited)
    token_budget: u32,
    /// Show line numbers in output
    line_numbers: bool,
    /// Include directory structure
    show_directory_structure: bool,
    /// Include file summary
    show_file_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinimalScanConfig {
    /// Include glob patterns
    include: Vec<String>,
    /// Exclude glob patterns
    exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinimalSecurityConfig {
    /// Enable secret scanning
    scan_secrets: bool,
    /// Fail if secrets detected
    fail_on_secrets: bool,
    /// Redact secrets in output
    redact_secrets: bool,
    /// Patterns to allowlist
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowlist: Vec<String>,
    /// Custom secret patterns (regex)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    custom_patterns: Vec<String>,
}

impl Default for MinimalConfig {
    fn default() -> Self {
        Self {
            output: MinimalOutputConfig {
                format: "xml".to_owned(),
                model: "claude".to_owned(),
                compression: "balanced".to_owned(),
                token_budget: 0,
                line_numbers: true,
                show_directory_structure: true,
                show_file_summary: true,
            },
            scan: MinimalScanConfig {
                include: vec![],
                exclude: vec![
                    "**/node_modules/**".to_owned(),
                    "**/.git/**".to_owned(),
                    "**/target/**".to_owned(),
                    "**/__pycache__/**".to_owned(),
                    "**/dist/**".to_owned(),
                    "**/build/**".to_owned(),
                ],
            },
            security: MinimalSecurityConfig {
                scan_secrets: true,
                fail_on_secrets: false,
                redact_secrets: true,
                allowlist: vec![],
                custom_patterns: vec![],
            },
            include_tests: false,
            include_docs: false,
        }
    }
}

impl Config {
    /// Get the effective number of threads
    pub fn effective_threads(&self) -> usize {
        if self.performance.threads == 0 {
            std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4)
        } else {
            self.performance.threads
        }
    }
}

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration parse error: {0}")]
    ParseError(#[from] figment::Error),
    #[error("Configuration I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Configuration serialize error: {0}")]
    SerializeError(String),
}

/// Parse a size string like "100KB", "1MB", "500" into bytes
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    // Try parsing as plain number first
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }

    // Parse with suffix
    let (num_str, multiplier) = if s.ends_with("KB") || s.ends_with("K") {
        (s.trim_end_matches("KB").trim_end_matches('K'), 1024u64)
    } else if s.ends_with("MB") || s.ends_with("M") {
        (s.trim_end_matches("MB").trim_end_matches('M'), 1024 * 1024)
    } else if s.ends_with("GB") || s.ends_with("G") {
        (s.trim_end_matches("GB").trim_end_matches('G'), 1024 * 1024 * 1024)
    } else if s.ends_with('B') {
        (s.trim_end_matches('B'), 1)
    } else {
        return None;
    };

    num_str.trim().parse::<u64>().ok().map(|n| n * multiplier)
}

// Provide dirs crate functionality inline if not available
mod dirs {
    use std::path::PathBuf;

    pub(super) fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert!(config.scan.respect_gitignore);
        assert_eq!(config.output.format, "xml");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100"), Some(100));
        assert_eq!(parse_size("100B"), Some(100));
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("1K"), Some(1024));
        assert_eq!(parse_size("10MB"), Some(10 * 1024 * 1024));
        assert_eq!(parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("invalid"), None);
    }

    #[test]
    fn test_generate_default_yaml() {
        let yaml = Config::generate_default("yaml");
        // MinimalConfig contains output and scan sections
        assert!(yaml.contains("output:"));
        assert!(yaml.contains("scan:"));
        assert!(yaml.contains("format:"));
    }

    #[test]
    fn test_generate_default_toml() {
        let toml = Config::generate_default("toml");
        // MinimalConfig contains output and scan sections
        assert!(toml.contains("[output]"));
        assert!(toml.contains("[scan]"));
    }

    #[test]
    fn test_generate_default_json() {
        let json = Config::generate_default("json");
        // MinimalConfig contains output and scan sections
        assert!(json.contains("\"output\""));
        assert!(json.contains("\"scan\""));
    }

    #[test]
    fn test_effective_threads() {
        let mut config = Config::default();
        config.performance.threads = 0;
        assert!(config.effective_threads() > 0);

        config.performance.threads = 8;
        assert_eq!(config.effective_threads(), 8);
    }
}
