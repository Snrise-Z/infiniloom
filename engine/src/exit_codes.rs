//! Semantic exit codes for CI/CD integration
//!
//! This module defines standardized exit codes that allow CI/CD pipelines
//! to programmatically handle different error conditions. Exit codes follow
//! Unix conventions (0 = success, non-zero = error) with specific meanings.
//!
//! # Exit Code Ranges
//!
//! | Range | Category |
//! |-------|----------|
//! | 0 | Success |
//! | 1-9 | General errors |
//! | 10-19 | Security issues |
//! | 20-29 | Configuration errors |
//! | 30-39 | I/O and resource errors |
//! | 40-49 | Validation errors |
//!
//! # CI/CD Usage
//!
//! ```bash
//! # In a CI pipeline
//! infiniloom embed /path/to/repo --security-check
//! exit_code=$?
//!
//! case $exit_code in
//!     0) echo "Success" ;;
//!     10) echo "Secrets detected - blocking PR" ;;
//!     11) echo "PII detected - review required" ;;
//!     12) echo "License violation - legal review needed" ;;
//!     *) echo "Error: $exit_code" ;;
//! esac
//! ```
//!
//! # GitHub Actions Integration
//!
//! ```yaml
//! - name: Security Scan
//!   id: scan
//!   run: infiniloom embed . --security-check
//!   continue-on-error: true
//!
//! - name: Block on Secrets
//!   if: steps.scan.outcome == 'failure' && steps.scan.exit-code == 10
//!   run: |
//!     echo "::error::Secrets detected in codebase"
//!     exit 1
//!
//! - name: Warn on PII
//!   if: steps.scan.exit-code == 11
//!   run: echo "::warning::PII detected - review recommended"
//! ```

use std::fmt;
use std::process::ExitCode as StdExitCode;

/// Semantic exit codes for CLI commands
///
/// These exit codes provide meaningful status information to CI/CD systems
/// and shell scripts. Each code represents a specific condition that can
/// be programmatically handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    // === Success (0) ===
    /// Operation completed successfully
    Success = 0,

    // === General Errors (1-9) ===
    /// Unspecified error
    GeneralError = 1,
    /// Command-line argument parsing error
    ArgumentError = 2,
    /// Internal error (bug)
    InternalError = 3,
    /// Operation interrupted by user (Ctrl+C)
    Interrupted = 4,
    /// Operation timed out
    Timeout = 5,

    // === Security Issues (10-19) ===
    /// Secrets/credentials detected in code
    SecretsDetected = 10,
    /// PII (Personally Identifiable Information) detected
    PiiDetected = 11,
    /// License compliance violation (GPL, AGPL, etc.)
    LicenseViolation = 12,
    /// Path traversal attack blocked
    PathTraversalBlocked = 13,
    /// Security scan failed
    SecurityScanFailed = 14,

    // === Configuration Errors (20-29) ===
    /// Configuration file not found
    ConfigNotFound = 20,
    /// Invalid configuration format
    ConfigInvalid = 21,
    /// Missing required configuration
    ConfigMissing = 22,
    /// Conflicting configuration options
    ConfigConflict = 23,
    /// Unsupported configuration version
    ConfigVersionError = 24,

    // === I/O and Resource Errors (30-39) ===
    /// File or directory not found
    NotFound = 30,
    /// Permission denied
    PermissionDenied = 31,
    /// Disk full or quota exceeded
    DiskFull = 32,
    /// Network error (e.g., remote clone failed)
    NetworkError = 33,
    /// Resource limit exceeded (too many files, etc.)
    ResourceLimitExceeded = 34,
    /// File too large
    FileTooLarge = 35,
    /// Binary file detected
    BinaryFileDetected = 36,

    // === Validation Errors (40-49) ===
    /// No files matched the include patterns
    NoFilesMatched = 40,
    /// No chunks generated (empty output)
    NoChunksGenerated = 41,
    /// Invalid pattern (glob/regex)
    InvalidPattern = 42,
    /// Unsupported language or file type
    UnsupportedLanguage = 43,
    /// Manifest integrity error
    ManifestCorrupted = 44,
    /// Token budget exceeded
    BudgetExceeded = 45,
}

impl ExitCode {
    /// Get the numeric exit code value
    pub fn code(&self) -> u8 {
        *self as u8
    }

    /// Get the exit code category
    pub fn category(&self) -> ExitCodeCategory {
        match self.code() {
            0 => ExitCodeCategory::Success,
            1..=9 => ExitCodeCategory::GeneralError,
            10..=19 => ExitCodeCategory::SecurityIssue,
            20..=29 => ExitCodeCategory::ConfigurationError,
            30..=39 => ExitCodeCategory::IoError,
            40..=49 => ExitCodeCategory::ValidationError,
            _ => ExitCodeCategory::GeneralError,
        }
    }

    /// Get a human-readable name for this exit code
    pub fn name(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::GeneralError => "GENERAL_ERROR",
            Self::ArgumentError => "ARGUMENT_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
            Self::Interrupted => "INTERRUPTED",
            Self::Timeout => "TIMEOUT",
            Self::SecretsDetected => "SECRETS_DETECTED",
            Self::PiiDetected => "PII_DETECTED",
            Self::LicenseViolation => "LICENSE_VIOLATION",
            Self::PathTraversalBlocked => "PATH_TRAVERSAL_BLOCKED",
            Self::SecurityScanFailed => "SECURITY_SCAN_FAILED",
            Self::ConfigNotFound => "CONFIG_NOT_FOUND",
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::ConfigMissing => "CONFIG_MISSING",
            Self::ConfigConflict => "CONFIG_CONFLICT",
            Self::ConfigVersionError => "CONFIG_VERSION_ERROR",
            Self::NotFound => "NOT_FOUND",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::DiskFull => "DISK_FULL",
            Self::NetworkError => "NETWORK_ERROR",
            Self::ResourceLimitExceeded => "RESOURCE_LIMIT_EXCEEDED",
            Self::FileTooLarge => "FILE_TOO_LARGE",
            Self::BinaryFileDetected => "BINARY_FILE_DETECTED",
            Self::NoFilesMatched => "NO_FILES_MATCHED",
            Self::NoChunksGenerated => "NO_CHUNKS_GENERATED",
            Self::InvalidPattern => "INVALID_PATTERN",
            Self::UnsupportedLanguage => "UNSUPPORTED_LANGUAGE",
            Self::ManifestCorrupted => "MANIFEST_CORRUPTED",
            Self::BudgetExceeded => "BUDGET_EXCEEDED",
        }
    }

    /// Get a description of this exit code
    pub fn description(&self) -> &'static str {
        match self {
            Self::Success => "Operation completed successfully",
            Self::GeneralError => "An unspecified error occurred",
            Self::ArgumentError => "Invalid command-line arguments",
            Self::InternalError => "Internal error (please report this bug)",
            Self::Interrupted => "Operation was interrupted by user",
            Self::Timeout => "Operation timed out",
            Self::SecretsDetected => "Secrets or credentials detected in code",
            Self::PiiDetected => "Personally identifiable information detected",
            Self::LicenseViolation => "License compliance violation detected",
            Self::PathTraversalBlocked => "Path traversal attack blocked",
            Self::SecurityScanFailed => "Security scan failed to complete",
            Self::ConfigNotFound => "Configuration file not found",
            Self::ConfigInvalid => "Invalid configuration file format",
            Self::ConfigMissing => "Required configuration is missing",
            Self::ConfigConflict => "Conflicting configuration options",
            Self::ConfigVersionError => "Unsupported configuration version",
            Self::NotFound => "File or directory not found",
            Self::PermissionDenied => "Permission denied",
            Self::DiskFull => "Disk full or quota exceeded",
            Self::NetworkError => "Network operation failed",
            Self::ResourceLimitExceeded => "Resource limit exceeded",
            Self::FileTooLarge => "File exceeds size limit",
            Self::BinaryFileDetected => "Binary file detected",
            Self::NoFilesMatched => "No files matched the specified patterns",
            Self::NoChunksGenerated => "No chunks were generated",
            Self::InvalidPattern => "Invalid glob or regex pattern",
            Self::UnsupportedLanguage => "Unsupported language or file type",
            Self::ManifestCorrupted => "Manifest file is corrupted",
            Self::BudgetExceeded => "Token budget exceeded",
        }
    }

    /// Check if this is a security-related exit code
    pub fn is_security_issue(&self) -> bool {
        matches!(self.category(), ExitCodeCategory::SecurityIssue)
    }

    /// Check if this exit code indicates success
    pub fn is_success(&self) -> bool {
        *self == Self::Success
    }

    /// Create an exit code from a numeric value
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::GeneralError,
            2 => Self::ArgumentError,
            3 => Self::InternalError,
            4 => Self::Interrupted,
            5 => Self::Timeout,
            10 => Self::SecretsDetected,
            11 => Self::PiiDetected,
            12 => Self::LicenseViolation,
            13 => Self::PathTraversalBlocked,
            14 => Self::SecurityScanFailed,
            20 => Self::ConfigNotFound,
            21 => Self::ConfigInvalid,
            22 => Self::ConfigMissing,
            23 => Self::ConfigConflict,
            24 => Self::ConfigVersionError,
            30 => Self::NotFound,
            31 => Self::PermissionDenied,
            32 => Self::DiskFull,
            33 => Self::NetworkError,
            34 => Self::ResourceLimitExceeded,
            35 => Self::FileTooLarge,
            36 => Self::BinaryFileDetected,
            40 => Self::NoFilesMatched,
            41 => Self::NoChunksGenerated,
            42 => Self::InvalidPattern,
            43 => Self::UnsupportedLanguage,
            44 => Self::ManifestCorrupted,
            45 => Self::BudgetExceeded,
            _ => Self::GeneralError,
        }
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): {}", self.name(), self.code(), self.description())
    }
}

impl From<ExitCode> for u8 {
    fn from(code: ExitCode) -> Self {
        code.code()
    }
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code.code() as i32
    }
}

impl From<ExitCode> for StdExitCode {
    fn from(code: ExitCode) -> Self {
        StdExitCode::from(code.code())
    }
}

/// Categories of exit codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCodeCategory {
    /// Success (0)
    Success,
    /// General errors (1-9)
    GeneralError,
    /// Security issues (10-19)
    SecurityIssue,
    /// Configuration errors (20-29)
    ConfigurationError,
    /// I/O and resource errors (30-39)
    IoError,
    /// Validation errors (40-49)
    ValidationError,
}

impl ExitCodeCategory {
    /// Get a human-readable name for this category
    pub fn name(&self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::GeneralError => "General Error",
            Self::SecurityIssue => "Security Issue",
            Self::ConfigurationError => "Configuration Error",
            Self::IoError => "I/O Error",
            Self::ValidationError => "Validation Error",
        }
    }
}

impl fmt::Display for ExitCodeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Result type that can be converted to an exit code
pub struct ExitResult {
    code: ExitCode,
    message: Option<String>,
}

impl ExitResult {
    /// Create a success result
    pub fn success() -> Self {
        Self { code: ExitCode::Success, message: None }
    }

    /// Create an error result
    pub fn error(code: ExitCode, message: impl Into<String>) -> Self {
        Self { code, message: Some(message.into()) }
    }

    /// Create from just an exit code
    pub fn from_code(code: ExitCode) -> Self {
        Self { code, message: None }
    }

    /// Get the exit code
    pub fn code(&self) -> ExitCode {
        self.code
    }

    /// Get the message, if any
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Check if this is a success result
    pub fn is_success(&self) -> bool {
        self.code.is_success()
    }

    /// Convert to a process exit code
    pub fn exit(self) -> ! {
        if let Some(ref msg) = self.message {
            if self.code.is_success() {
                println!("{}", msg);
            } else {
                eprintln!("Error: {}", msg);
            }
        }
        std::process::exit(self.code.code() as i32)
    }
}

impl From<ExitCode> for ExitResult {
    fn from(code: ExitCode) -> Self {
        Self::from_code(code)
    }
}

/// Helper trait to convert errors to exit codes
pub trait ToExitCode {
    /// Convert to an appropriate exit code
    fn to_exit_code(&self) -> ExitCode;
}

impl ToExitCode for std::io::Error {
    fn to_exit_code(&self) -> ExitCode {
        use std::io::ErrorKind;
        match self.kind() {
            ErrorKind::NotFound => ExitCode::NotFound,
            ErrorKind::PermissionDenied => ExitCode::PermissionDenied,
            ErrorKind::TimedOut => ExitCode::Timeout,
            ErrorKind::Interrupted => ExitCode::Interrupted,
            ErrorKind::WriteZero | ErrorKind::StorageFull => ExitCode::DiskFull,
            _ => ExitCode::GeneralError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_values() {
        assert_eq!(ExitCode::Success.code(), 0);
        assert_eq!(ExitCode::GeneralError.code(), 1);
        assert_eq!(ExitCode::SecretsDetected.code(), 10);
        assert_eq!(ExitCode::PiiDetected.code(), 11);
        assert_eq!(ExitCode::ConfigNotFound.code(), 20);
        assert_eq!(ExitCode::NotFound.code(), 30);
        assert_eq!(ExitCode::NoFilesMatched.code(), 40);
    }

    #[test]
    fn test_exit_code_categories() {
        assert_eq!(ExitCode::Success.category(), ExitCodeCategory::Success);
        assert_eq!(ExitCode::GeneralError.category(), ExitCodeCategory::GeneralError);
        assert_eq!(ExitCode::SecretsDetected.category(), ExitCodeCategory::SecurityIssue);
        assert_eq!(ExitCode::ConfigNotFound.category(), ExitCodeCategory::ConfigurationError);
        assert_eq!(ExitCode::NotFound.category(), ExitCodeCategory::IoError);
        assert_eq!(ExitCode::NoFilesMatched.category(), ExitCodeCategory::ValidationError);
    }

    #[test]
    fn test_is_security_issue() {
        assert!(ExitCode::SecretsDetected.is_security_issue());
        assert!(ExitCode::PiiDetected.is_security_issue());
        assert!(ExitCode::LicenseViolation.is_security_issue());
        assert!(!ExitCode::Success.is_security_issue());
        assert!(!ExitCode::NotFound.is_security_issue());
    }

    #[test]
    fn test_from_code() {
        assert_eq!(ExitCode::from_code(0), ExitCode::Success);
        assert_eq!(ExitCode::from_code(10), ExitCode::SecretsDetected);
        assert_eq!(ExitCode::from_code(255), ExitCode::GeneralError); // Unknown maps to general
    }

    #[test]
    fn test_display() {
        let code = ExitCode::SecretsDetected;
        let display = format!("{}", code);
        assert!(display.contains("SECRETS_DETECTED"));
        assert!(display.contains("10"));
    }

    #[test]
    fn test_exit_result() {
        let success = ExitResult::success();
        assert!(success.is_success());
        assert_eq!(success.code(), ExitCode::Success);

        let error = ExitResult::error(ExitCode::SecretsDetected, "Found API keys");
        assert!(!error.is_success());
        assert_eq!(error.code(), ExitCode::SecretsDetected);
        assert_eq!(error.message(), Some("Found API keys"));
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io::{Error, ErrorKind};

        let not_found = Error::new(ErrorKind::NotFound, "file not found");
        assert_eq!(not_found.to_exit_code(), ExitCode::NotFound);

        let permission = Error::new(ErrorKind::PermissionDenied, "access denied");
        assert_eq!(permission.to_exit_code(), ExitCode::PermissionDenied);
    }

    #[test]
    fn test_conversions() {
        let code = ExitCode::SecretsDetected;

        let u8_code: u8 = code.into();
        assert_eq!(u8_code, 10);

        let i32_code: i32 = code.into();
        assert_eq!(i32_code, 10);
    }
}
