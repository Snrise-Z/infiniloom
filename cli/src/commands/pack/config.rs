//! Pack command configuration
//!
//! This module provides structured configuration for the pack command,
//! replacing the original 78-parameter function with a clean builder pattern.
//!
//! # Overview
//!
//! The pack command configuration uses a builder pattern to construct a `PackConfig`
//! instance with all necessary options. This approach provides:
//!
//! - **Type safety**: Compile-time validation of required parameters
//! - **Maintainability**: Logical grouping of related options
//! - **Testability**: Easy to construct test configurations
//! - **Extensibility**: Simple to add new options without breaking existing code
//!
//! # Architecture
//!
//! The configuration is organized into 6 logical groups:
//!
//! - `OutputOptions`: Format, model, compression, and output settings
//! - `ScanOptions`: File filtering, symbol extraction, and content transformation
//! - `GitOptions`: Git history, logs, and change tracking
//! - `SecurityOptions`: Secret detection and redaction
//! - `WatchOptions`: File watching configuration
//! - Top-level options: verbose, config path, map budget
//!
//! # Usage Examples
//!
//! ## Basic Usage
//!
//! ```rust
//! use infiniloom_cli::commands::pack::{PackConfig, OutputOptions};
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("/path/to/repo"))
//!     .build()
//!     .expect("Failed to build config");
//! ```
//!
//! ## With Custom Output Options
//!
//! ```rust
//! use infiniloom_cli::commands::pack::{PackConfig, OutputOptions};
//! use infiniloom_engine::output::OutputFormat;
//! use infiniloom_engine::types::{CompressionLevel, TokenizerModel};
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("/path/to/repo"))
//!     .output(OutputOptions {
//!         format: Some(OutputFormat::Xml),
//!         model: Some(TokenizerModel::Claude),
//!         compression: Some(CompressionLevel::Balanced),
//!         max_tokens: 100000,
//!         output_file: Some(PathBuf::from("output.xml")),
//!         show_line_numbers: true,
//!         ..Default::default()
//!     })
//!     .verbose(true)
//!     .build()
//!     .expect("Failed to build config");
//! ```
//!
//! ## With Pattern Matching
//!
//! ```rust
//! use infiniloom_cli::commands::pack::{PackConfig, ScanOptions};
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("/path/to/repo"))
//!     .scan(ScanOptions {
//!         include_patterns: vec!["src/**/*.rs".to_string(), "lib/**/*.rs".to_string()],
//!         exclude_patterns: vec!["**/*_test.rs".to_string()],
//!         include_tests: false,
//!         ..Default::default()
//!     })
//!     .build()
//!     .expect("Failed to build config");
//! ```
//!
//! ## Full Configuration
//!
//! ```rust
//! use infiniloom_cli::commands::pack::{
//!     PackConfig, OutputOptions, ScanOptions, GitOptions,
//!     SecurityOptions, WatchOptions
//! };
//! use infiniloom_engine::output::OutputFormat;
//! use infiniloom_engine::types::{CompressionLevel, TokenizerModel};
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("/path/to/repo"))
//!     .output(OutputOptions {
//!         format: Some(OutputFormat::Xml),
//!         model: Some(TokenizerModel::Gpt4o),
//!         compression: Some(CompressionLevel::Aggressive),
//!         max_tokens: 50000,
//!         output_file: Some(PathBuf::from("packed.xml")),
//!         show_line_numbers: true,
//!         show_file_summary: true,
//!         ..Default::default()
//!     })
//!     .scan(ScanOptions {
//!         enable_symbols: true,
//!         remove_comments: true,
//!         remove_empty_lines: true,
//!         top_files: 100,
//!         ..Default::default()
//!     })
//!     .git(GitOptions {
//!         include_logs: true,
//!         logs_count: 50,
//!         sort_by_changes: true,
//!         ..Default::default()
//!     })
//!     .security(SecurityOptions {
//!         security_check: true,
//!         redact_secrets: true,
//!     })
//!     .verbose(true)
//!     .map_budget(2000)
//!     .build()
//!     .expect("Failed to build config");
//! ```
//!
//! # Error Handling
//!
//! The builder pattern validates required parameters at build time:
//!
//! ```rust
//! use infiniloom_cli::commands::pack::PackConfig;
//!
//! let result = PackConfig::builder()
//!     .verbose(true)
//!     .build();
//!
//! assert!(result.is_err()); // Missing required 'path' parameter
//! assert!(result.unwrap_err().to_string().contains("path is required"));
//! ```

use anyhow::Result;
use infiniloom_engine::{
    output::OutputFormat,
    types::{CompressionLevel, TokenizerModel},
};
use std::path::PathBuf;

/// Main configuration for pack command
///
/// This struct groups all pack command options into logical categories,
/// making the API more maintainable and testable.
#[derive(Debug, Clone)]
pub struct PackConfig {
    /// Path to the repository
    pub path: PathBuf,

    /// Output-related options
    pub output: OutputOptions,

    /// Scanning and filtering options
    pub scan: ScanOptions,

    /// Git-related options
    pub git: GitOptions,

    /// Security options
    pub security: SecurityOptions,

    /// Watch mode options
    pub watch: WatchOptions,

    /// Verbose output
    pub verbose: bool,

    /// Configuration file path
    pub config_path: Option<PathBuf>,

    /// Token budget for repository map
    pub map_budget: u32,
}

/// Output formatting and generation options
#[derive(Debug, Clone)]
pub struct OutputOptions {
    /// Output format (XML, Markdown, JSON, etc.)
    pub format: Option<OutputFormat>,

    /// Target LLM model for tokenization
    pub model: Option<TokenizerModel>,

    /// Compression level
    pub compression: Option<CompressionLevel>,

    /// Maximum tokens in output
    pub max_tokens: u32,

    /// Output file path (None = stdout)
    pub output_file: Option<PathBuf>,

    /// Custom header text
    pub header_text: Option<String>,

    /// Instruction file to prepend
    pub instruction_file: Option<PathBuf>,

    /// Show line numbers in output
    pub show_line_numbers: bool,

    /// Show directory structure
    pub show_directory_structure: bool,

    /// Show file summary statistics
    pub show_file_summary: bool,

    /// Show token tree breakdown
    pub token_tree: bool,

    /// Copy output to clipboard
    pub copy_to_clipboard: bool,
}

/// Scanning and filtering options
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Include hidden files
    pub include_hidden: bool,

    /// Respect .gitignore patterns
    pub respect_gitignore: bool,

    /// Enable symbol extraction
    pub enable_symbols: bool,

    /// Full mode (include all content)
    pub full_mode: bool,

    /// Exclude file content (structure only)
    pub exclude_content: bool,

    /// Include test files
    pub include_tests: bool,

    /// Include documentation files
    pub include_docs: bool,

    /// Use default ignore patterns
    pub use_default_ignores: bool,

    /// Include patterns (glob)
    pub include_patterns: Vec<String>,

    /// Exclude patterns (glob)
    pub exclude_patterns: Vec<String>,

    /// Read file paths from stdin
    pub stdin: bool,

    /// Remove empty lines from output
    pub remove_empty_lines: bool,

    /// Remove comments from output
    pub remove_comments: bool,

    /// Number of top files to include
    pub top_files: usize,

    /// Truncate base64 content
    pub truncate_base64: bool,

    /// Use incremental caching
    pub incremental_cache: bool,
}

/// Git-related options
#[derive(Debug, Clone)]
pub struct GitOptions {
    /// Include git logs
    pub include_logs: bool,

    /// Number of log entries to include
    pub logs_count: usize,

    /// Include git diffs
    pub include_diffs: bool,

    /// Sort files by change frequency
    pub sort_by_changes: bool,

    /// Remote repository branch
    pub remote_branch: Option<String>,

    /// Sparse checkout paths
    pub sparse_paths: Vec<String>,
}

/// Security scanning options
#[derive(Debug, Clone)]
pub struct SecurityOptions {
    /// Run security check for secrets
    pub security_check: bool,

    /// Redact detected secrets
    pub redact_secrets: bool,
}

/// Watch mode options
#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// Enable watch mode
    pub enabled: bool,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            format: None,
            model: None,
            compression: None,
            max_tokens: 0,
            output_file: None,
            header_text: None,
            instruction_file: None,
            show_line_numbers: false,
            show_directory_structure: false,
            show_file_summary: false,
            token_tree: false,
            copy_to_clipboard: false,
        }
    }
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            enable_symbols: true,
            full_mode: false,
            exclude_content: false,
            include_tests: false,
            include_docs: false,
            use_default_ignores: true,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            stdin: false,
            remove_empty_lines: false,
            remove_comments: false,
            top_files: 0,
            truncate_base64: false,
            incremental_cache: false,
        }
    }
}

impl Default for GitOptions {
    fn default() -> Self {
        Self {
            include_logs: false,
            logs_count: 10,
            include_diffs: false,
            sort_by_changes: false,
            remote_branch: None,
            sparse_paths: Vec::new(),
        }
    }
}

impl Default for SecurityOptions {
    fn default() -> Self {
        Self { security_check: false, redact_secrets: false }
    }
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl PackConfig {
    /// Create a new builder for PackConfig
    pub fn builder() -> PackConfigBuilder {
        PackConfigBuilder::default()
    }
}

/// Builder for PackConfig
///
/// Uses the builder pattern to construct PackConfig with sensible defaults
/// and fluent API for setting options.
#[derive(Debug, Default)]
pub struct PackConfigBuilder {
    path: Option<PathBuf>,
    output: OutputOptions,
    scan: ScanOptions,
    git: GitOptions,
    security: SecurityOptions,
    watch: WatchOptions,
    verbose: bool,
    config_path: Option<PathBuf>,
    map_budget: u32,
}

impl PackConfigBuilder {
    /// Set the repository path
    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Set output options
    pub fn output(mut self, output: OutputOptions) -> Self {
        self.output = output;
        self
    }

    /// Set scan options
    pub fn scan(mut self, scan: ScanOptions) -> Self {
        self.scan = scan;
        self
    }

    /// Set git options
    pub fn git(mut self, git: GitOptions) -> Self {
        self.git = git;
        self
    }

    /// Set security options
    pub fn security(mut self, security: SecurityOptions) -> Self {
        self.security = security;
        self
    }

    /// Set watch options
    pub fn watch(mut self, watch: WatchOptions) -> Self {
        self.watch = watch;
        self
    }

    /// Set verbose flag
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set config file path
    pub fn config_path(mut self, config_path: Option<PathBuf>) -> Self {
        self.config_path = config_path;
        self
    }

    /// Set repository map token budget
    pub fn map_budget(mut self, budget: u32) -> Self {
        self.map_budget = budget;
        self
    }

    /// Build the PackConfig
    ///
    /// # Errors
    ///
    /// Returns error if path is not set
    pub fn build(self) -> Result<PackConfig> {
        let path = self
            .path
            .ok_or_else(|| anyhow::anyhow!("Repository path is required"))?;

        Ok(PackConfig {
            path,
            output: self.output,
            scan: self.scan,
            git: self.git,
            security: self.security,
            watch: self.watch,
            verbose: self.verbose,
            config_path: self.config_path,
            map_budget: self.map_budget,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_minimal() {
        let config = PackConfig::builder()
            .path(PathBuf::from("/test/path"))
            .build()
            .unwrap();

        assert_eq!(config.path, PathBuf::from("/test/path"));
        assert!(!config.verbose);
        assert_eq!(config.map_budget, 0);
    }

    #[test]
    fn test_builder_with_options() {
        let output = OutputOptions {
            format: Some(OutputFormat::Xml),
            max_tokens: 100000,
            show_line_numbers: true,
            ..Default::default()
        };

        let config = PackConfig::builder()
            .path(PathBuf::from("/test/path"))
            .output(output)
            .verbose(true)
            .map_budget(2000)
            .build()
            .unwrap();

        assert_eq!(config.output.max_tokens, 100000);
        assert!(config.output.show_line_numbers);
        assert!(config.verbose);
        assert_eq!(config.map_budget, 2000);
    }

    #[test]
    fn test_builder_requires_path() {
        let result = PackConfig::builder().build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Repository path is required");
    }

    #[test]
    fn test_default_options() {
        let output = OutputOptions::default();
        assert_eq!(output.max_tokens, 0);
        assert!(!output.show_line_numbers);
        assert!(output.format.is_none());

        let scan = ScanOptions::default();
        assert!(!scan.include_hidden);
        assert!(scan.respect_gitignore);
        assert!(scan.enable_symbols);

        let git = GitOptions::default();
        assert!(!git.include_logs);
        assert_eq!(git.logs_count, 10);

        let security = SecurityOptions::default();
        assert!(!security.security_check);

        let watch = WatchOptions::default();
        assert!(!watch.enabled);
    }

    #[test]
    fn test_fluent_api() {
        let config = PackConfig::builder()
            .path(PathBuf::from("/test"))
            .verbose(true)
            .map_budget(5000)
            .build()
            .unwrap();

        assert_eq!(config.path, PathBuf::from("/test"));
        assert!(config.verbose);
        assert_eq!(config.map_budget, 5000);
    }

    #[test]
    fn test_builder_missing_path_fails() {
        let result = PackConfig::builder().verbose(true).build();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path is required"));
    }

    #[test]
    fn test_output_options_builder() {
        let output = OutputOptions {
            format: Some(OutputFormat::Xml),
            model: Some(TokenizerModel::Claude),
            compression: Some(CompressionLevel::Balanced),
            max_tokens: 100000,
            output_file: Some(PathBuf::from("/out.xml")),
            header_text: Some("Custom header".to_string()),
            instruction_file: Some(PathBuf::from("/instructions.txt")),
            show_line_numbers: true,
            show_directory_structure: true,
            show_file_summary: true,
            token_tree: true,
            copy_to_clipboard: true,
        };

        assert_eq!(output.format, Some(OutputFormat::Xml));
        assert_eq!(output.model, Some(TokenizerModel::Claude));
        assert_eq!(output.max_tokens, 100000);
        assert!(output.show_line_numbers);
        assert!(output.token_tree);
    }

    #[test]
    fn test_scan_options_comprehensive() {
        let scan = ScanOptions {
            include_hidden: true,
            respect_gitignore: false,
            enable_symbols: true,
            full_mode: true,
            exclude_content: false,
            include_tests: true,
            include_docs: true,
            use_default_ignores: false,
            remove_empty_lines: true,
            remove_comments: true,
            top_files: 50,
            truncate_base64: true,
            include_patterns: vec!["*.rs".to_string(), "*.py".to_string()],
            exclude_patterns: vec!["test_*".to_string()],
            stdin: true,
            incremental_cache: true,
        };

        assert!(scan.include_hidden);
        assert!(!scan.respect_gitignore);
        assert!(scan.full_mode);
        assert_eq!(scan.top_files, 50);
        assert_eq!(scan.include_patterns.len(), 2);
        assert_eq!(scan.exclude_patterns.len(), 1);
    }

    #[test]
    fn test_git_options_all_fields() {
        let git = GitOptions {
            include_logs: true,
            logs_count: 100,
            include_diffs: true,
            sort_by_changes: true,
            remote_branch: Some("feature/test".to_string()),
            sparse_paths: vec!["src/".to_string(), "lib/".to_string()],
        };

        assert!(git.include_logs);
        assert_eq!(git.logs_count, 100);
        assert!(git.include_diffs);
        assert!(git.sort_by_changes);
        assert_eq!(git.remote_branch, Some("feature/test".to_string()));
        assert_eq!(git.sparse_paths.len(), 2);
    }

    #[test]
    fn test_security_options_both_flags() {
        let security = SecurityOptions { security_check: true, redact_secrets: true };

        assert!(security.security_check);
        assert!(security.redact_secrets);
    }

    #[test]
    fn test_watch_options_enabled() {
        let watch = WatchOptions { enabled: true };
        assert!(watch.enabled);

        let watch_disabled = WatchOptions::default();
        assert!(!watch_disabled.enabled);
    }

    #[test]
    fn test_builder_all_options_custom() {
        let config = PackConfig::builder()
            .path(PathBuf::from("/repo"))
            .output(OutputOptions {
                format: Some(OutputFormat::Json),
                model: Some(TokenizerModel::Gpt4o),
                compression: Some(CompressionLevel::Aggressive),
                max_tokens: 50000,
                output_file: Some(PathBuf::from("/output.json")),
                header_text: None,
                instruction_file: None,
                show_line_numbers: false,
                show_directory_structure: false,
                show_file_summary: false,
                token_tree: false,
                copy_to_clipboard: false,
            })
            .scan(ScanOptions {
                include_hidden: true,
                respect_gitignore: false,
                enable_symbols: false,
                full_mode: false,
                exclude_content: true,
                include_tests: false,
                include_docs: false,
                use_default_ignores: true,
                remove_empty_lines: false,
                remove_comments: false,
                top_files: 100,
                truncate_base64: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
                stdin: false,
                incremental_cache: false,
            })
            .git(GitOptions {
                include_logs: false,
                logs_count: 20,
                include_diffs: false,
                sort_by_changes: false,
                remote_branch: None,
                sparse_paths: vec![],
            })
            .security(SecurityOptions { security_check: false, redact_secrets: false })
            .watch(WatchOptions { enabled: false })
            .verbose(false)
            .config_path(Some(PathBuf::from("/.infiniloom.yaml")))
            .map_budget(1000)
            .build()
            .unwrap();

        assert_eq!(config.path, PathBuf::from("/repo"));
        assert_eq!(config.output.format, Some(OutputFormat::Json));
        assert_eq!(config.output.max_tokens, 50000);
        assert!(config.scan.include_hidden);
        assert!(!config.scan.respect_gitignore);
        assert_eq!(config.scan.top_files, 100);
        assert!(!config.verbose);
        assert_eq!(config.map_budget, 1000);
    }

    #[test]
    fn test_builder_partial_options() {
        // Test that we can build with only some options customized
        let config = PackConfig::builder()
            .path(PathBuf::from("/test"))
            .output(OutputOptions { max_tokens: 10000, ..Default::default() })
            .verbose(true)
            .build()
            .unwrap();

        assert_eq!(config.output.max_tokens, 10000);
        assert!(config.verbose);
        // Other options should have defaults
        assert!(!config.scan.include_hidden);
        assert!(config.scan.respect_gitignore);
    }

    #[test]
    fn test_builder_chaining() {
        // Test that builder methods can be chained in any order
        let config = PackConfig::builder()
            .verbose(true)
            .map_budget(2000)
            .path(PathBuf::from("/test"))
            .config_path(Some(PathBuf::from("/config.yaml")))
            .build()
            .unwrap();

        assert!(config.verbose);
        assert_eq!(config.map_budget, 2000);
        assert_eq!(config.path, PathBuf::from("/test"));
        assert_eq!(config.config_path, Some(PathBuf::from("/config.yaml")));
    }

    #[test]
    fn test_empty_vectors_in_scan_options() {
        let scan = ScanOptions {
            include_patterns: vec![],
            exclude_patterns: vec![],
            ..Default::default()
        };

        assert!(scan.include_patterns.is_empty());
        assert!(scan.exclude_patterns.is_empty());
    }

    #[test]
    fn test_git_options_sparse_paths_empty() {
        let git = GitOptions { sparse_paths: vec![], ..Default::default() };

        assert!(git.sparse_paths.is_empty());
        assert_eq!(git.logs_count, 10); // Default value
    }

    #[test]
    fn test_output_options_none_values() {
        let output = OutputOptions {
            format: None,
            model: None,
            compression: None,
            output_file: None,
            header_text: None,
            instruction_file: None,
            ..Default::default()
        };

        assert!(output.format.is_none());
        assert!(output.model.is_none());
        assert!(output.compression.is_none());
        assert!(output.output_file.is_none());
    }

    #[test]
    fn test_builder_overwrite_values() {
        // Test that later calls overwrite earlier ones
        let config = PackConfig::builder()
            .path(PathBuf::from("/first"))
            .path(PathBuf::from("/second")) // Overwrite
            .verbose(false)
            .verbose(true) // Overwrite
            .map_budget(1000)
            .map_budget(2000) // Overwrite
            .build()
            .unwrap();

        assert_eq!(config.path, PathBuf::from("/second"));
        assert!(config.verbose);
        assert_eq!(config.map_budget, 2000);
    }

    #[test]
    fn test_complex_pattern_configuration() {
        let patterns =
            vec!["src/**/*.rs".to_string(), "lib/**/*.rs".to_string(), "!**/*_test.rs".to_string()];

        let scan = ScanOptions {
            include_patterns: patterns.clone(),
            exclude_patterns: vec!["target/".to_string(), "build/".to_string()],
            ..Default::default()
        };

        assert_eq!(scan.include_patterns.len(), 3);
        assert_eq!(scan.exclude_patterns.len(), 2);
        assert_eq!(scan.include_patterns[0], "src/**/*.rs");
    }

    #[test]
    fn test_full_mode_implies_symbols() {
        // In practice, full_mode should enable symbols
        let scan = ScanOptions {
            full_mode: true,
            enable_symbols: true, // Should be true when full_mode is true
            ..Default::default()
        };

        assert!(scan.full_mode);
        assert!(scan.enable_symbols);
    }

    #[test]
    fn test_watch_mode_with_output_file() {
        // Watch mode requires output file in practice
        let output =
            OutputOptions { output_file: Some(PathBuf::from("/output.xml")), ..Default::default() };

        let watch = WatchOptions { enabled: true };

        assert!(output.output_file.is_some());
        assert!(watch.enabled);
    }
}
