//! Actionable error types for the embedding system
//!
//! All errors include:
//! - Clear description of what went wrong
//! - Actionable fix suggestions
//! - Context for debugging
//!
//! # Security
//!
//! Error messages use sanitized paths that strip the user's home directory
//! to prevent leaking sensitive filesystem information.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Sanitize a path for display in error messages
///
/// Removes the user's home directory prefix to prevent leaking sensitive paths.
/// Example: `/Users/john/code/project/src/foo.rs` → `~/code/project/src/foo.rs`
pub fn sanitize_path(path: &Path) -> String {
    // Try HOME environment variable (Unix/macOS)
    if let Ok(home) = std::env::var("HOME") {
        let home_path = Path::new(&home);
        if let Ok(relative) = path.strip_prefix(home_path) {
            return format!("~/{}", relative.display());
        }
    }
    // Try USERPROFILE for Windows
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home_path = Path::new(&home);
        if let Ok(relative) = path.strip_prefix(home_path) {
            return format!("~/{}", relative.display());
        }
    }
    // If we can't get home dir or path isn't under it, use as-is
    path.display().to_string()
}

/// Sanitize a PathBuf for display in error messages
pub fn sanitize_pathbuf(path: &PathBuf) -> String {
    sanitize_path(path.as_path())
}

/// A wrapper around PathBuf that sanitizes paths when displayed
#[derive(Debug, Clone)]
pub struct SafePath(pub PathBuf);

impl std::fmt::Display for SafePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", sanitize_path(&self.0))
    }
}

/// Actionable error types with helpful messages
#[derive(Debug, Error)]
pub enum EmbedError {
    // === User Errors (Actionable) ===
    #[error(
        "Invalid settings: {field} - {reason}\n\nFix: Check your --{field} argument or config file"
    )]
    InvalidSettings { field: String, reason: String },

    #[error("Manifest version {found} is newer than supported version {max_supported}\n\nFix: Upgrade infiniloom to latest version, or delete manifest and rebuild:\n  rm .infiniloom-embed.bin && infiniloom embed")]
    ManifestVersionTooNew { found: u32, max_supported: u32 },

    #[error("Manifest corrupted or tampered\n  Path: {path}\n  Expected checksum: {expected}\n  Actual checksum: {actual}\n\nFix: Delete manifest and rebuild:\n  rm {path} && infiniloom embed", path = path.display())]
    ManifestCorrupted { path: PathBuf, expected: String, actual: String },

    #[error("Settings changed since last run\n\nPrevious: {previous}\nCurrent:  {current}\n\nImpact: All chunk IDs may change\n\nFix: Run with --full to rebuild, or restore original settings")]
    SettingsChanged { previous: String, current: String },

    #[error("No code chunks found\n\nPossible causes:\n  - Include patterns too restrictive: {include_patterns}\n  - Exclude patterns too broad: {exclude_patterns}\n  - No supported languages in repository\n\nFix: Check -i/--include and -e/--exclude patterns")]
    NoChunksGenerated { include_patterns: String, exclude_patterns: String },

    #[error("Secrets detected in {count} chunks\n\nFiles with secrets:\n{files}\n\nFix: Either:\n  1. Remove secrets from code\n  2. Use --redact-secrets to mask them\n  3. Use --no-scan-secrets to skip scanning (not recommended)")]
    SecretsDetected { count: usize, files: String },

    #[error("Invalid glob pattern: '{pattern}'\n  Error: {reason}\n\nFix: Check -i/--include or -e/--exclude pattern syntax.\n  Examples: '*.rs', 'src/**/*.ts', '!tests/*'")]
    InvalidPattern { pattern: String, reason: String },

    #[error("Hash collision detected!\n  Chunk ID: {id}\n  Hash 1: {hash1}\n  Hash 2: {hash2}\n\nThis is extremely rare. Please report at https://github.com/infiniloom/issues")]
    HashCollision { id: String, hash1: String, hash2: String },

    // === Resource Limit Errors ===
    #[error("File too large: {path} ({size} bytes, max: {max})\n\nFix: Exclude large files with -e/--exclude pattern, or increase --max-file-size", path = path.display())]
    FileTooLarge { path: PathBuf, size: u64, max: u64 },

    #[error("Line too long in file: {path} ({length} chars, max: {max})\n\nThis is likely a minified file.\n\nFix: Exclude minified files with -e/--exclude pattern (e.g., '*.min.js'), or increase --max-line-length", path = path.display())]
    LineTooLong { path: PathBuf, length: usize, max: usize },

    #[error(
        "Too many chunks generated ({count}, max: {max})\n\nFix: Use more restrictive include patterns, or increase --max-chunks limit"
    )]
    TooManyChunks { count: usize, max: usize },

    #[error("Too many files to process ({count}, max: {max})\n\nFix: Use more restrictive include patterns, or increase --max-files limit")]
    TooManyFiles { count: usize, max: usize },

    #[error("Recursion limit exceeded while parsing\n  Depth: {depth}, Max: {max}\n  Context: {context}\n\nFix: File may have unusual nesting. Exclude it with -e pattern")]
    RecursionLimitExceeded { depth: u32, max: u32, context: String },

    #[error("Path traversal detected\n  Path: {path}\n  Repo root: {repo_root}\n\nFix: Remove symlinks pointing outside repository, or use --no-follow-symlinks", path = path.display(), repo_root = repo_root.display())]
    PathTraversal { path: PathBuf, repo_root: PathBuf },

    // === System Errors ===
    #[error("I/O error: {path}\n  {source}", path = path.display())]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Parse error in {file} at line {line}\n  {message}\n\nFix: Fix syntax error or exclude file with -e pattern")]
    ParseError { file: String, line: u32, message: String },

    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },

    #[error("Deserialization error: {reason}\n\nFix: Manifest may be corrupted. Delete and rebuild:\n  rm .infiniloom-embed.bin && infiniloom embed")]
    DeserializationError { reason: String },

    #[error("Unsupported algorithm version {found} (max supported: {max_supported})\n\nFix: Upgrade infiniloom or regenerate with current version")]
    UnsupportedAlgorithmVersion { found: u32, max_supported: u32 },

    #[error("Multiple files failed to process:\n{errors}\n\nFix: Address individual errors above")]
    MultipleErrors { errors: String },

    #[error("Not a directory: {path}", path = path.display())]
    NotADirectory { path: PathBuf },

    #[error("Too many errors encountered ({count}, max: {max})\n\nFix: Address individual errors, or increase error tolerance")]
    TooManyErrors { count: usize, max: usize },
}

impl EmbedError {
    /// Format multiple file errors into a single error
    pub fn from_file_errors(errors: Vec<(PathBuf, EmbedError)>) -> Self {
        let formatted = errors
            .iter()
            .map(|(path, err)| format!("  {}: {}", path.display(), err))
            .collect::<Vec<_>>()
            .join("\n");
        Self::MultipleErrors { errors: formatted }
    }

    /// Check if this error is critical (should stop processing)
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            EmbedError::TooManyChunks { .. }
                | EmbedError::TooManyFiles { .. }
                | EmbedError::PathTraversal { .. }
                | EmbedError::HashCollision { .. }
                | EmbedError::SecretsDetected { .. }
                | EmbedError::ManifestCorrupted { .. }
                | EmbedError::InvalidPattern { .. }
                | EmbedError::InvalidSettings { .. }
        )
    }

    /// Check if this error can be recovered from by skipping the file
    pub fn is_skippable(&self) -> bool {
        matches!(
            self,
            EmbedError::FileTooLarge { .. }
                | EmbedError::LineTooLong { .. }
                | EmbedError::ParseError { .. }
                | EmbedError::IoError { .. }
                | EmbedError::RecursionLimitExceeded { .. }
        )
    }

    /// Get the semantic exit code for this error
    ///
    /// Exit codes follow POSIX conventions and are designed for shell scripting:
    ///
    /// | Code | Category | Description |
    /// |------|----------|-------------|
    /// | 0 | Success | No error |
    /// | 1 | User Error | Invalid settings, patterns, or arguments |
    /// | 2 | Input Error | No chunks generated, no data to process |
    /// | 3 | Security | Secrets detected (use --redact-secrets or --no-scan-secrets) |
    /// | 4 | Security | Path traversal attempt blocked |
    /// | 10 | Manifest | Version mismatch, corruption, or settings changed |
    /// | 11 | Resource | Too many chunks/files, recursion limit |
    /// | 12 | System | I/O errors, serialization failures |
    /// | 13 | Internal | Hash collision (extremely rare, report as bug) |
    /// | 14 | Parse | Source code parse errors (skippable) |
    /// | 15 | Multiple | Multiple errors encountered |
    ///
    /// # Shell Script Example
    ///
    /// ```bash
    /// infiniloom embed /path/to/repo
    /// case $? in
    ///     0) echo "Success" ;;
    ///     1) echo "Invalid settings - check arguments" ;;
    ///     2) echo "No code found - check include/exclude patterns" ;;
    ///     3) echo "Secrets detected - use --redact-secrets" ;;
    ///     4) echo "Security violation - path traversal blocked" ;;
    ///     10) echo "Manifest issue - delete .infiniloom-embed.bin and retry" ;;
    ///     11) echo "Resource limit - use more restrictive patterns" ;;
    ///     12) echo "System error - check disk space and permissions" ;;
    ///     13) echo "Internal error - please report this bug" ;;
    ///     14) echo "Parse errors - some files skipped" ;;
    ///     15) echo "Multiple errors - see above for details" ;;
    /// esac
    /// ```
    pub fn exit_code(&self) -> i32 {
        match self {
            // User errors (invalid configuration): 1
            EmbedError::InvalidSettings { .. } | EmbedError::InvalidPattern { .. } => 1,

            // Input errors (no data): 2
            EmbedError::NoChunksGenerated { .. } | EmbedError::NotADirectory { .. } => 2,

            // Security - secrets detected: 3
            EmbedError::SecretsDetected { .. } => 3,

            // Security - path traversal: 4
            EmbedError::PathTraversal { .. } => 4,

            // Manifest errors: 10
            EmbedError::ManifestVersionTooNew { .. }
            | EmbedError::ManifestCorrupted { .. }
            | EmbedError::SettingsChanged { .. }
            | EmbedError::UnsupportedAlgorithmVersion { .. } => 10,

            // Resource limit errors: 11
            EmbedError::TooManyChunks { .. }
            | EmbedError::TooManyFiles { .. }
            | EmbedError::TooManyErrors { .. }
            | EmbedError::RecursionLimitExceeded { .. }
            | EmbedError::FileTooLarge { .. }
            | EmbedError::LineTooLong { .. } => 11,

            // System errors (I/O, serialization): 12
            EmbedError::IoError { .. }
            | EmbedError::SerializationError { .. }
            | EmbedError::DeserializationError { .. } => 12,

            // Internal errors (hash collision - extremely rare): 13
            EmbedError::HashCollision { .. } => 13,

            // Parse errors: 14
            EmbedError::ParseError { .. } => 14,

            // Multiple errors: 15
            EmbedError::MultipleErrors { .. } => 15,
        }
    }

    /// Get a short error code string for programmatic use
    ///
    /// Useful for JSON output or logging systems.
    pub fn error_code(&self) -> &'static str {
        match self {
            EmbedError::InvalidSettings { .. } => "E001_INVALID_SETTINGS",
            EmbedError::InvalidPattern { .. } => "E002_INVALID_PATTERN",
            EmbedError::NoChunksGenerated { .. } => "E003_NO_CHUNKS",
            EmbedError::NotADirectory { .. } => "E004_NOT_DIRECTORY",
            EmbedError::SecretsDetected { .. } => "E005_SECRETS_DETECTED",
            EmbedError::PathTraversal { .. } => "E006_PATH_TRAVERSAL",
            EmbedError::ManifestVersionTooNew { .. } => "E010_MANIFEST_VERSION",
            EmbedError::ManifestCorrupted { .. } => "E011_MANIFEST_CORRUPTED",
            EmbedError::SettingsChanged { .. } => "E012_SETTINGS_CHANGED",
            EmbedError::UnsupportedAlgorithmVersion { .. } => "E013_ALGORITHM_VERSION",
            EmbedError::TooManyChunks { .. } => "E020_TOO_MANY_CHUNKS",
            EmbedError::TooManyFiles { .. } => "E021_TOO_MANY_FILES",
            EmbedError::TooManyErrors { .. } => "E022_TOO_MANY_ERRORS",
            EmbedError::RecursionLimitExceeded { .. } => "E023_RECURSION_LIMIT",
            EmbedError::FileTooLarge { .. } => "E024_FILE_TOO_LARGE",
            EmbedError::LineTooLong { .. } => "E025_LINE_TOO_LONG",
            EmbedError::IoError { .. } => "E030_IO_ERROR",
            EmbedError::SerializationError { .. } => "E031_SERIALIZATION",
            EmbedError::DeserializationError { .. } => "E032_DESERIALIZATION",
            EmbedError::HashCollision { .. } => "E040_HASH_COLLISION",
            EmbedError::ParseError { .. } => "E050_PARSE_ERROR",
            EmbedError::MultipleErrors { .. } => "E099_MULTIPLE_ERRORS",
        }
    }
}

impl Clone for EmbedError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidSettings { field, reason } => {
                Self::InvalidSettings { field: field.clone(), reason: reason.clone() }
            },
            Self::ManifestVersionTooNew { found, max_supported } => {
                Self::ManifestVersionTooNew { found: *found, max_supported: *max_supported }
            },
            Self::ManifestCorrupted { path, expected, actual } => Self::ManifestCorrupted {
                path: path.clone(),
                expected: expected.clone(),
                actual: actual.clone(),
            },
            Self::SettingsChanged { previous, current } => {
                Self::SettingsChanged { previous: previous.clone(), current: current.clone() }
            },
            Self::NoChunksGenerated { include_patterns, exclude_patterns } => {
                Self::NoChunksGenerated {
                    include_patterns: include_patterns.clone(),
                    exclude_patterns: exclude_patterns.clone(),
                }
            },
            Self::SecretsDetected { count, files } => {
                Self::SecretsDetected { count: *count, files: files.clone() }
            },
            Self::HashCollision { id, hash1, hash2 } => {
                Self::HashCollision { id: id.clone(), hash1: hash1.clone(), hash2: hash2.clone() }
            },
            Self::FileTooLarge { path, size, max } => {
                Self::FileTooLarge { path: path.clone(), size: *size, max: *max }
            },
            Self::LineTooLong { path, length, max } => {
                Self::LineTooLong { path: path.clone(), length: *length, max: *max }
            },
            Self::TooManyChunks { count, max } => Self::TooManyChunks { count: *count, max: *max },
            Self::TooManyFiles { count, max } => Self::TooManyFiles { count: *count, max: *max },
            Self::RecursionLimitExceeded { depth, max, context } => {
                Self::RecursionLimitExceeded { depth: *depth, max: *max, context: context.clone() }
            },
            Self::PathTraversal { path, repo_root } => {
                Self::PathTraversal { path: path.clone(), repo_root: repo_root.clone() }
            },
            Self::IoError { path, source } => Self::IoError {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::ParseError { file, line, message } => {
                Self::ParseError { file: file.clone(), line: *line, message: message.clone() }
            },
            Self::SerializationError { reason } => {
                Self::SerializationError { reason: reason.clone() }
            },
            Self::DeserializationError { reason } => {
                Self::DeserializationError { reason: reason.clone() }
            },
            Self::UnsupportedAlgorithmVersion { found, max_supported } => {
                Self::UnsupportedAlgorithmVersion { found: *found, max_supported: *max_supported }
            },
            Self::MultipleErrors { errors } => Self::MultipleErrors { errors: errors.clone() },
            Self::NotADirectory { path } => Self::NotADirectory { path: path.clone() },
            Self::InvalidPattern { pattern, reason } => {
                Self::InvalidPattern { pattern: pattern.clone(), reason: reason.clone() }
            },
            Self::TooManyErrors { count, max } => Self::TooManyErrors { count: *count, max: *max },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EmbedError::InvalidSettings {
            field: "max_tokens".to_owned(),
            reason: "exceeds limit of 100000".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("max_tokens"));
        assert!(msg.contains("Fix:"));
    }

    #[test]
    fn test_from_file_errors() {
        let errors = vec![
            (
                PathBuf::from("src/foo.rs"),
                EmbedError::FileTooLarge {
                    path: PathBuf::from("src/foo.rs"),
                    size: 20_000_000,
                    max: 10_000_000,
                },
            ),
            (
                PathBuf::from("src/bar.rs"),
                EmbedError::ParseError {
                    file: "src/bar.rs".to_owned(),
                    line: 42,
                    message: "unexpected token".to_owned(),
                },
            ),
        ];

        let combined = EmbedError::from_file_errors(errors);
        let msg = combined.to_string();
        assert!(msg.contains("src/foo.rs"));
        assert!(msg.contains("src/bar.rs"));
    }

    #[test]
    fn test_is_critical() {
        assert!(EmbedError::TooManyChunks { count: 100, max: 50 }.is_critical());
        assert!(EmbedError::PathTraversal {
            path: PathBuf::from("/etc/passwd"),
            repo_root: PathBuf::from("/home/user/repo"),
        }
        .is_critical());
        assert!(!EmbedError::FileTooLarge { path: PathBuf::from("big.bin"), size: 100, max: 50 }
            .is_critical());
    }

    #[test]
    fn test_is_skippable() {
        assert!(EmbedError::FileTooLarge { path: PathBuf::from("big.bin"), size: 100, max: 50 }
            .is_skippable());
        assert!(EmbedError::ParseError {
            file: "bad.rs".to_owned(),
            line: 1,
            message: "syntax error".to_owned(),
        }
        .is_skippable());
        assert!(!EmbedError::TooManyChunks { count: 100, max: 50 }.is_skippable());
    }

    #[test]
    fn test_error_clone() {
        let err = EmbedError::HashCollision {
            id: "ec_123".to_owned(),
            hash1: "abc".to_owned(),
            hash2: "def".to_owned(),
        };
        let cloned = err;
        assert!(matches!(cloned, EmbedError::HashCollision { .. }));
    }

    #[test]
    fn test_exit_codes() {
        // User errors: 1
        assert_eq!(
            EmbedError::InvalidSettings {
                field: "max_tokens".to_owned(),
                reason: "too high".to_owned()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            EmbedError::InvalidPattern {
                pattern: "**[".to_owned(),
                reason: "unclosed bracket".to_owned()
            }
            .exit_code(),
            1
        );

        // Input errors: 2
        assert_eq!(
            EmbedError::NoChunksGenerated {
                include_patterns: "*.xyz".to_owned(),
                exclude_patterns: "".to_owned()
            }
            .exit_code(),
            2
        );
        assert_eq!(
            EmbedError::NotADirectory { path: PathBuf::from("/tmp/file.txt") }.exit_code(),
            2
        );

        // Security - secrets: 3
        assert_eq!(
            EmbedError::SecretsDetected { count: 5, files: "config.py".to_owned() }.exit_code(),
            3
        );

        // Security - path traversal: 4
        assert_eq!(
            EmbedError::PathTraversal {
                path: PathBuf::from("../../../etc/passwd"),
                repo_root: PathBuf::from("/repo")
            }
            .exit_code(),
            4
        );

        // Manifest errors: 10
        assert_eq!(
            EmbedError::ManifestVersionTooNew { found: 99, max_supported: 2 }.exit_code(),
            10
        );
        assert_eq!(
            EmbedError::ManifestCorrupted {
                path: PathBuf::from(".infiniloom-embed.bin"),
                expected: "abc".to_owned(),
                actual: "def".to_owned()
            }
            .exit_code(),
            10
        );

        // Resource limits: 11
        assert_eq!(EmbedError::TooManyChunks { count: 100000, max: 50000 }.exit_code(), 11);
        assert_eq!(EmbedError::TooManyFiles { count: 10000, max: 5000 }.exit_code(), 11);
        assert_eq!(
            EmbedError::FileTooLarge {
                path: PathBuf::from("big.bin"),
                size: 100_000_000,
                max: 10_000_000
            }
            .exit_code(),
            11
        );

        // System errors: 12
        assert_eq!(
            EmbedError::IoError {
                path: PathBuf::from("/tmp"),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found")
            }
            .exit_code(),
            12
        );
        assert_eq!(EmbedError::SerializationError { reason: "failed".to_owned() }.exit_code(), 12);

        // Internal errors: 13
        assert_eq!(
            EmbedError::HashCollision {
                id: "ec_123".to_owned(),
                hash1: "abc".to_owned(),
                hash2: "def".to_owned()
            }
            .exit_code(),
            13
        );

        // Parse errors: 14
        assert_eq!(
            EmbedError::ParseError {
                file: "bad.rs".to_owned(),
                line: 42,
                message: "syntax error".to_owned()
            }
            .exit_code(),
            14
        );

        // Multiple errors: 15
        assert_eq!(
            EmbedError::MultipleErrors { errors: "error1\nerror2".to_owned() }.exit_code(),
            15
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            EmbedError::InvalidSettings { field: "x".to_owned(), reason: "y".to_owned() }
                .error_code(),
            "E001_INVALID_SETTINGS"
        );
        assert_eq!(
            EmbedError::SecretsDetected { count: 1, files: "f".to_owned() }.error_code(),
            "E005_SECRETS_DETECTED"
        );
        assert_eq!(
            EmbedError::HashCollision {
                id: "i".to_owned(),
                hash1: "a".to_owned(),
                hash2: "b".to_owned()
            }
            .error_code(),
            "E040_HASH_COLLISION"
        );
    }
}
