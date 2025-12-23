//! Unified error types for Infiniloom
//!
//! This module provides a top-level error type that wraps all domain-specific
//! errors for convenient error handling across the library.

use thiserror::Error;

use crate::config::ConfigError;
use crate::git::GitError;
use crate::incremental::CacheError;
use crate::parser::ParserError;
use crate::remote::RemoteError;
use crate::semantic::SemanticError;

/// Unified error type for all Infiniloom operations
#[derive(Debug, Error)]
pub enum InfiniloomError {
    /// Parser-related errors
    #[error("Parse error: {0}")]
    Parser(#[from] ParserError),

    /// Git operation errors
    #[error("Git error: {0}")]
    Git(#[from] GitError),

    /// Remote repository errors
    #[error("Remote error: {0}")]
    Remote(#[from] RemoteError),

    /// Configuration errors
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    /// Cache/incremental scanning errors
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),

    /// Semantic analysis errors
    #[error("Semantic error: {0}")]
    Semantic(#[from] SemanticError),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Security scan found issues
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

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Operation not supported
    #[error("Operation not supported: {0}")]
    NotSupported(String),
}

/// Convenience type alias for Results using InfiniloomError
pub type Result<T> = std::result::Result<T, InfiniloomError>;

impl InfiniloomError {
    /// Create a security issues error
    pub fn security_issues(count: usize, critical: usize) -> Self {
        Self::SecurityIssues { count, critical }
    }

    /// Create a budget exceeded error
    pub fn budget_exceeded(used: u32, budget: u32) -> Self {
        Self::BudgetExceeded { used, budget }
    }

    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not supported error
    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::NotSupported(msg.into())
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::SecurityIssues { .. } | Self::BudgetExceeded { .. } | Self::InvalidInput(_)
        )
    }

    /// Check if this is a critical error
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Parser(_) | Self::Git(_) | Self::Io(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = InfiniloomError::security_issues(5, 2);
        assert_eq!(err.to_string(), "Security scan found 5 issues (2 critical)");

        let err = InfiniloomError::budget_exceeded(150000, 100000);
        assert_eq!(err.to_string(), "Token budget exceeded: 150000 tokens used, 100000 allowed");
    }

    #[test]
    fn test_error_classification() {
        let err = InfiniloomError::security_issues(1, 0);
        assert!(err.is_recoverable());
        assert!(!err.is_critical());

        let err = InfiniloomError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(!err.is_recoverable());
        assert!(err.is_critical());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: InfiniloomError = io_err.into();
        assert!(matches!(err, InfiniloomError::Io(_)));
    }
}
