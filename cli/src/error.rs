//! Unified error types for Infiniloom CLI
//!
//! This module provides CLI-specific error types that extend the engine's
//! error handling with command-specific error variants.

use thiserror::Error;

/// CLI-specific error type
#[derive(Debug, Error)]
pub(crate) enum CliError {
    /// Engine errors (wrapped from infiniloom_engine::InfiniloomError)
    #[error("Engine error: {0}")]
    Engine(#[from] infiniloom_engine::InfiniloomError),

    /// Embed errors (wrapped from infiniloom_engine::embedding::EmbedError)
    #[error("{0}")]
    Embed(#[from] infiniloom_engine::embedding::EmbedError),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid command arguments
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Missing required argument
    #[error("Missing required argument: {0}")]
    MissingArgument(String),

    /// Invalid path provided
    #[error("Invalid path: {path}: {reason}")]
    InvalidPath {
        /// The invalid path
        path: String,
        /// Reason why the path is invalid
        reason: String,
    },

    /// Path does not exist
    #[error("Path not found: {0}")]
    PathNotFound(String),

    /// Not a git repository
    #[error("Not a git repository: {0}")]
    NotGitRepo(String),

    /// Git command not available
    #[error("Git command not found. Please install git and ensure it's in your PATH")]
    GitNotAvailable,

    /// Index not found
    #[error("No index found at {path}. Run 'infiniloom index' first to build the index")]
    IndexNotFound {
        /// Repository path
        path: String,
    },

    /// Index is stale
    #[error("Index is stale. Run 'infiniloom index --force' to rebuild")]
    IndexStale,

    /// No changes detected
    #[error("No changes detected in the repository")]
    NoChanges,

    /// Invalid output format
    #[error(
        "Invalid output format: {0}. Supported formats: xml, json, markdown, yaml, toon, plain"
    )]
    InvalidFormat(String),

    /// Invalid model
    #[error("Invalid model: {0}. See 'infiniloom info' for supported models")]
    InvalidModel(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Security scan failed
    #[error("Security scan found {count} issues ({critical} critical)")]
    SecurityIssues {
        /// Total number of issues
        count: usize,
        /// Number of critical issues
        critical: usize,
    },

    /// Token budget exceeded
    #[error("Token budget exceeded: {used} tokens used, {budget} allowed")]
    BudgetExceeded {
        /// Tokens used
        used: u32,
        /// Budget limit
        budget: u32,
    },

    /// Command execution failed
    #[error("Command failed: {command}: {reason}")]
    CommandFailed {
        /// Command that failed
        command: String,
        /// Reason for failure
        reason: String,
    },

    /// Feature not available
    #[error("Feature not available: {feature}. {hint}")]
    FeatureUnavailable {
        /// Feature name
        feature: String,
        /// Hint for user
        hint: String,
    },

    /// Generic error with context
    #[error("{0}")]
    Other(String),
}

/// Convenience type alias for Results using CliError
pub(crate) type Result<T> = std::result::Result<T, CliError>;

impl CliError {
    /// Create an invalid argument error
    pub(crate) fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }

    /// Create a missing argument error
    pub(crate) fn missing_argument(name: impl Into<String>) -> Self {
        Self::MissingArgument(name.into())
    }

    /// Create an invalid path error
    pub(crate) fn invalid_path(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidPath { path: path.into(), reason: reason.into() }
    }

    /// Create a path not found error
    pub(crate) fn path_not_found(path: impl Into<String>) -> Self {
        Self::PathNotFound(path.into())
    }

    /// Create a not git repo error
    pub(crate) fn not_git_repo(path: impl Into<String>) -> Self {
        Self::NotGitRepo(path.into())
    }

    /// Create an index not found error
    pub(crate) fn index_not_found(path: impl Into<String>) -> Self {
        Self::IndexNotFound { path: path.into() }
    }

    /// Create a security issues error
    pub(crate) fn security_issues(count: usize, critical: usize) -> Self {
        Self::SecurityIssues { count, critical }
    }

    /// Create a budget exceeded error
    pub(crate) fn budget_exceeded(used: u32, budget: u32) -> Self {
        Self::BudgetExceeded { used, budget }
    }

    /// Create a command failed error
    pub(crate) fn command_failed(command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::CommandFailed { command: command.into(), reason: reason.into() }
    }

    /// Create a feature unavailable error
    pub(crate) fn feature_unavailable(feature: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::FeatureUnavailable { feature: feature.into(), hint: hint.into() }
    }

    /// Create a generic error
    pub(crate) fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Check if this is a user error (invalid input, missing args, etc.)
    ///
    /// User errors are typically fixable by the user without code changes.
    pub(crate) fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidArgument(_)
                | Self::MissingArgument(_)
                | Self::InvalidPath { .. }
                | Self::PathNotFound(_)
                | Self::NotGitRepo(_)
                | Self::InvalidFormat(_)
                | Self::InvalidModel(_)
                | Self::Config(_)
                | Self::NoChanges
                | Self::IndexNotFound { .. }
                | Self::IndexStale
        )
    }

    /// Check if this is an internal error (system issues, bugs, etc.)
    ///
    /// Internal errors typically require code fixes or system-level changes.
    pub(crate) fn is_internal_error(&self) -> bool {
        matches!(
            self,
            Self::Engine(_) | Self::Io(_) | Self::GitNotAvailable | Self::CommandFailed { .. }
        )
    }

    /// Check if this is a recoverable error
    ///
    /// Recoverable errors allow the program to continue with alternative paths.
    pub(crate) fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::SecurityIssues { .. }
                | Self::BudgetExceeded { .. }
                | Self::NoChanges
                | Self::IndexStale
                | Self::FeatureUnavailable { .. }
        )
    }

    /// Check if this is a critical error
    ///
    /// Critical errors typically require immediate attention and program termination.
    pub(crate) fn is_critical(&self) -> bool {
        matches!(self, Self::Engine(_) | Self::Io(_) | Self::GitNotAvailable)
    }

    /// Get the exit code for this error
    ///
    /// Different error types map to different exit codes for shell scripting.
    ///
    /// # Exit Code Ranges
    ///
    /// | Range | Category |
    /// |-------|----------|
    /// | 1-9 | General CLI errors |
    /// | 10-19 | Engine/system errors |
    /// | 20-99 | Embed command errors (see EmbedError::exit_code for details) |
    ///
    /// # Embed Command Exit Codes
    ///
    /// The embed command uses semantic exit codes (20-99 range) for shell scripting:
    ///
    /// | Code | Category | Description |
    /// |------|----------|-------------|
    /// | 21 | User Error | Invalid settings, patterns, or arguments |
    /// | 22 | Input Error | No chunks generated, no data to process |
    /// | 23 | Security | Secrets detected (use --redact-secrets) |
    /// | 24 | Security | Path traversal attempt blocked |
    /// | 30 | Manifest | Version mismatch, corruption |
    /// | 31 | Resource | Too many chunks/files, recursion limit |
    /// | 32 | System | I/O errors, serialization failures |
    /// | 33 | Internal | Hash collision (extremely rare) |
    /// | 34 | Parse | Source code parse errors |
    /// | 35 | Multiple | Multiple errors encountered |
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            // Embed errors: use semantic exit codes in 20-99 range
            // Add 20 to avoid conflicts with general CLI exit codes
            Self::Embed(e) => 20 + e.exit_code(),

            // User errors: exit code 1
            Self::InvalidArgument(_)
            | Self::MissingArgument(_)
            | Self::InvalidPath { .. }
            | Self::PathNotFound(_)
            | Self::InvalidFormat(_)
            | Self::InvalidModel(_)
            | Self::Config(_) => 1,

            // Git errors: exit code 2
            Self::NotGitRepo(_) | Self::GitNotAvailable | Self::NoChanges => 2,

            // Index errors: exit code 3
            Self::IndexNotFound { .. } | Self::IndexStale => 3,

            // Security errors: exit code 4
            Self::SecurityIssues { .. } => 4,

            // Budget errors: exit code 5
            Self::BudgetExceeded { .. } => 5,

            // System/internal errors: exit code 10
            Self::Engine(_) | Self::Io(_) | Self::CommandFailed { .. } => 10,

            // Feature unavailable: exit code 6
            Self::FeatureUnavailable { .. } => 6,

            // Generic errors: exit code 1
            Self::Other(_) => 1,
        }
    }
}

// Allow conversion from anyhow::Error for gradual migration
impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Error Creation Tests
    // ============================================

    #[test]
    fn test_invalid_argument_error() {
        let err = CliError::invalid_argument("invalid value");
        assert_eq!(err.to_string(), "Invalid argument: invalid value");
    }

    #[test]
    fn test_missing_argument_error() {
        let err = CliError::missing_argument("path");
        assert_eq!(err.to_string(), "Missing required argument: path");
    }

    #[test]
    fn test_invalid_path_error() {
        let err = CliError::invalid_path("/invalid", "not a directory");
        assert_eq!(err.to_string(), "Invalid path: /invalid: not a directory");
    }

    #[test]
    fn test_path_not_found_error() {
        let err = CliError::path_not_found("/nonexistent");
        assert_eq!(err.to_string(), "Path not found: /nonexistent");
    }

    #[test]
    fn test_not_git_repo_error() {
        let err = CliError::not_git_repo("/tmp");
        assert_eq!(err.to_string(), "Not a git repository: /tmp");
    }

    #[test]
    fn test_index_not_found_error() {
        let err = CliError::index_not_found("/repo");
        assert_eq!(
            err.to_string(),
            "No index found at /repo. Run 'infiniloom index' first to build the index"
        );
    }

    #[test]
    fn test_security_issues_error() {
        let err = CliError::security_issues(10, 3);
        assert_eq!(err.to_string(), "Security scan found 10 issues (3 critical)");
    }

    #[test]
    fn test_budget_exceeded_error() {
        let err = CliError::budget_exceeded(150000, 100000);
        assert_eq!(err.to_string(), "Token budget exceeded: 150000 tokens used, 100000 allowed");
    }

    #[test]
    fn test_command_failed_error() {
        let err = CliError::command_failed("git", "command not found");
        assert_eq!(err.to_string(), "Command failed: git: command not found");
    }

    #[test]
    fn test_feature_unavailable_error() {
        let err = CliError::feature_unavailable(
            "embeddings",
            "Install with: cargo install infiniloom --features embeddings",
        );
        assert_eq!(
            err.to_string(),
            "Feature not available: embeddings. Install with: cargo install infiniloom --features embeddings"
        );
    }

    // ============================================
    // Error Classification Tests
    // ============================================

    #[test]
    fn test_is_user_error() {
        assert!(CliError::invalid_argument("test").is_user_error());
        assert!(CliError::missing_argument("path").is_user_error());
        assert!(CliError::path_not_found("/test").is_user_error());
        assert!(CliError::InvalidFormat("badformat".to_owned()).is_user_error());
        assert!(!CliError::GitNotAvailable.is_user_error());
    }

    #[test]
    fn test_is_internal_error() {
        assert!(CliError::GitNotAvailable.is_internal_error());
        assert!(CliError::command_failed("git", "failed").is_internal_error());
        assert!(!CliError::invalid_argument("test").is_internal_error());
    }

    #[test]
    fn test_is_recoverable() {
        assert!(CliError::security_issues(1, 0).is_recoverable());
        assert!(CliError::budget_exceeded(100, 50).is_recoverable());
        assert!(CliError::NoChanges.is_recoverable());
        assert!(!CliError::GitNotAvailable.is_recoverable());
    }

    #[test]
    fn test_is_critical() {
        assert!(CliError::GitNotAvailable.is_critical());
        assert!(!CliError::invalid_argument("test").is_critical());
        assert!(!CliError::NoChanges.is_critical());
    }

    // ============================================
    // Exit Code Tests
    // ============================================

    #[test]
    fn test_exit_codes() {
        // User errors: exit code 1
        assert_eq!(CliError::invalid_argument("test").exit_code(), 1);
        assert_eq!(CliError::missing_argument("path").exit_code(), 1);
        assert_eq!(CliError::InvalidFormat("bad".to_owned()).exit_code(), 1);

        // Git errors: exit code 2
        assert_eq!(CliError::not_git_repo("/tmp").exit_code(), 2);
        assert_eq!(CliError::GitNotAvailable.exit_code(), 2);
        assert_eq!(CliError::NoChanges.exit_code(), 2);

        // Index errors: exit code 3
        assert_eq!(CliError::index_not_found("/repo").exit_code(), 3);
        assert_eq!(CliError::IndexStale.exit_code(), 3);

        // Security errors: exit code 4
        assert_eq!(CliError::security_issues(1, 0).exit_code(), 4);

        // Budget errors: exit code 5
        assert_eq!(CliError::budget_exceeded(100, 50).exit_code(), 5);

        // Feature unavailable: exit code 6
        assert_eq!(CliError::feature_unavailable("test", "hint").exit_code(), 6);

        // System errors: exit code 10
        assert_eq!(CliError::command_failed("git", "failed").exit_code(), 10);
    }

    // ============================================
    // Conversion Tests
    // ============================================

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("test error");
        let cli_err: CliError = anyhow_err.into();
        assert!(matches!(cli_err, CliError::Other(_)));
        assert_eq!(cli_err.to_string(), "test error");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let cli_err: CliError = io_err.into();
        assert!(matches!(cli_err, CliError::Io(_)));
    }

    #[test]
    fn test_embed_error_exit_codes() {
        use infiniloom_engine::embedding::EmbedError;
        use std::path::PathBuf;

        // User error: 21 (20 + 1)
        let err: CliError = EmbedError::InvalidSettings {
            field: "max_tokens".to_owned(),
            reason: "too high".to_owned(),
        }
        .into();
        assert_eq!(err.exit_code(), 21);

        // Input error: 22 (20 + 2)
        let err: CliError = EmbedError::NoChunksGenerated {
            include_patterns: "*.xyz".to_owned(),
            exclude_patterns: "".to_owned(),
        }
        .into();
        assert_eq!(err.exit_code(), 22);

        // Security - secrets: 23 (20 + 3)
        let err: CliError =
            EmbedError::SecretsDetected { count: 5, files: "config.py".to_owned() }.into();
        assert_eq!(err.exit_code(), 23);

        // Security - path traversal: 24 (20 + 4)
        let err: CliError = EmbedError::PathTraversal {
            path: PathBuf::from("../../../etc/passwd"),
            repo_root: PathBuf::from("/repo"),
        }
        .into();
        assert_eq!(err.exit_code(), 24);

        // Manifest error: 30 (20 + 10)
        let err: CliError =
            EmbedError::ManifestVersionTooNew { found: 99, max_supported: 2 }.into();
        assert_eq!(err.exit_code(), 30);

        // Resource limit: 31 (20 + 11)
        let err: CliError = EmbedError::TooManyChunks { count: 100000, max: 50000 }.into();
        assert_eq!(err.exit_code(), 31);

        // System error: 32 (20 + 12)
        let err: CliError = EmbedError::IoError {
            path: PathBuf::from("/tmp"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        }
        .into();
        assert_eq!(err.exit_code(), 32);

        // Internal error: 33 (20 + 13)
        let err: CliError = EmbedError::HashCollision {
            id: "ec_123".to_owned(),
            hash1: "abc".to_owned(),
            hash2: "def".to_owned(),
        }
        .into();
        assert_eq!(err.exit_code(), 33);

        // Parse error: 34 (20 + 14)
        let err: CliError = EmbedError::ParseError {
            file: "bad.rs".to_owned(),
            line: 42,
            message: "syntax error".to_owned(),
        }
        .into();
        assert_eq!(err.exit_code(), 34);

        // Multiple errors: 35 (20 + 15)
        let err: CliError =
            EmbedError::MultipleErrors { errors: "error1\nerror2".to_owned() }.into();
        assert_eq!(err.exit_code(), 35);
    }
}
