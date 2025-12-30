//! Input validation helpers for NAPI bindings
//!
//! These functions validate JavaScript/TypeScript inputs to ensure they meet
//! requirements before passing to the Rust engine.

use napi::{Error, Result, Status};

/// Validate path is not empty (accepts Option to handle null/undefined gracefully)
pub fn validate_path_option(path: Option<&str>) -> Result<String> {
    match path {
        None => Err(Error::new(Status::InvalidArg, "Path cannot be null or undefined".to_string())),
        Some(p) if p.trim().is_empty() => {
            Err(Error::new(Status::InvalidArg, "Path cannot be empty".to_string()))
        },
        Some(p) => Ok(p.to_string()),
    }
}

/// Validate path is not empty
pub fn validate_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        return Err(Error::new(Status::InvalidArg, "Path cannot be empty".to_string()));
    }
    Ok(())
}

/// Validate symbol name is not empty (accepts Option to handle null/undefined gracefully)
pub fn validate_symbol_name_option(name: Option<&str>) -> Result<String> {
    match name {
        None => Err(Error::new(
            Status::InvalidArg,
            "Symbol name cannot be null or undefined".to_string(),
        )),
        Some(n) if n.trim().is_empty() => {
            Err(Error::new(Status::InvalidArg, "Symbol name cannot be empty".to_string()))
        },
        Some(n) => Ok(n.to_string()),
    }
}

/// Validate file path is not empty
pub fn validate_file_path(file_path: &str) -> Result<()> {
    if file_path.trim().is_empty() {
        return Err(Error::new(Status::InvalidArg, "File path cannot be empty".to_string()));
    }
    Ok(())
}

/// Validate token budget is non-negative
pub fn validate_token_budget(budget: Option<i64>) -> Result<u32> {
    match budget {
        None => Ok(0), // No limit when not specified
        Some(b) if b < 0 => {
            Err(Error::new(Status::InvalidArg, format!("Token budget cannot be negative: {}", b)))
        }
        Some(0) => {
            // Bug fix: tokenBudget=0 is ambiguous - reject with clear guidance
            Err(Error::new(
                Status::InvalidArg,
                "tokenBudget cannot be 0. Omit the parameter for no limit, or use a value >= 1000 for meaningful output.".to_string(),
            ))
        }
        Some(b) if b > 0 && b < 1000 => {
            // Bug fix: Very small budgets are impractical (file + formatting overhead)
            Err(Error::new(
                Status::InvalidArg,
                format!("tokenBudget {} is too small. Minimum is 1000 tokens for meaningful output.", b),
            ))
        }
        Some(b) => Ok(b as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // validate_path_option tests
    // ============================================================================

    #[test]
    fn test_validate_path_option_valid() {
        let result = validate_path_option(Some("/valid/path"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/valid/path");
    }

    #[test]
    fn test_validate_path_option_with_spaces() {
        let result = validate_path_option(Some("/path/with spaces/file.rs"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/path/with spaces/file.rs");
    }

    #[test]
    fn test_validate_path_option_none() {
        let result = validate_path_option(None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("null or undefined"));
    }

    #[test]
    fn test_validate_path_option_empty() {
        let result = validate_path_option(Some(""));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_path_option_whitespace() {
        let result = validate_path_option(Some("   "));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_path_option_whitespace_around_valid() {
        // Whitespace around valid path should be preserved (paths can have trailing spaces)
        let result = validate_path_option(Some("  /valid/path  "));
        // This is technically valid since the path is not empty after trim check
        // but the original path (with spaces) is returned
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "  /valid/path  ");
    }

    // ============================================================================
    // validate_path tests
    // ============================================================================

    #[test]
    fn test_validate_path_valid() {
        let result = validate_path("/valid/path");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_empty() {
        let result = validate_path("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_path_whitespace_only() {
        let result = validate_path("   \t\n  ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_with_special_chars() {
        let result = validate_path("/path/with-special_chars/[test].rs");
        assert!(result.is_ok());
    }

    // ============================================================================
    // validate_symbol_name_option tests
    // ============================================================================

    #[test]
    fn test_validate_symbol_name_option_valid() {
        let result = validate_symbol_name_option(Some("MyFunction"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "MyFunction");
    }

    #[test]
    fn test_validate_symbol_name_option_with_underscore() {
        let result = validate_symbol_name_option(Some("_private_function"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "_private_function");
    }

    #[test]
    fn test_validate_symbol_name_option_none() {
        let result = validate_symbol_name_option(None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("null or undefined"));
    }

    #[test]
    fn test_validate_symbol_name_option_empty() {
        let result = validate_symbol_name_option(Some(""));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_symbol_name_option_whitespace() {
        let result = validate_symbol_name_option(Some("   "));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_symbol_name_option_qualified() {
        let result = validate_symbol_name_option(Some("module::function"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "module::function");
    }

    // ============================================================================
    // validate_file_path tests
    // ============================================================================

    #[test]
    fn test_validate_file_path_valid() {
        let result = validate_file_path("src/main.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_path_empty() {
        let result = validate_file_path("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_file_path_whitespace() {
        let result = validate_file_path("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_path_relative() {
        let result = validate_file_path("../parent/file.rs");
        assert!(result.is_ok());
    }

    // ============================================================================
    // validate_token_budget tests
    // ============================================================================

    #[test]
    fn test_validate_token_budget_none() {
        let result = validate_token_budget(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // No limit
    }

    #[test]
    fn test_validate_token_budget_valid() {
        let result = validate_token_budget(Some(10000));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10000);
    }

    #[test]
    fn test_validate_token_budget_minimum() {
        let result = validate_token_budget(Some(1000));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1000);
    }

    #[test]
    fn test_validate_token_budget_negative() {
        let result = validate_token_budget(Some(-100));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("negative"));
        assert!(err.to_string().contains("-100"));
    }

    #[test]
    fn test_validate_token_budget_zero() {
        let result = validate_token_budget(Some(0));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cannot be 0"));
        assert!(err.to_string().contains("Omit"));
    }

    #[test]
    fn test_validate_token_budget_too_small() {
        let result = validate_token_budget(Some(500));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too small"));
        assert!(err.to_string().contains("1000"));
    }

    #[test]
    fn test_validate_token_budget_boundary_999() {
        let result = validate_token_budget(Some(999));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("too small"));
    }

    #[test]
    fn test_validate_token_budget_large() {
        let result = validate_token_budget(Some(1_000_000));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1_000_000);
    }

    #[test]
    fn test_validate_token_budget_i32_max() {
        let result = validate_token_budget(Some(i32::MAX as i64));
        assert!(result.is_ok());
    }

    // ============================================================================
    // Error message quality tests
    // ============================================================================

    #[test]
    fn test_error_messages_are_helpful() {
        // All error messages should be descriptive and guide the user
        let path_err = validate_path_option(None).unwrap_err();
        assert!(path_err.to_string().len() > 10); // Not just "error"

        let symbol_err = validate_symbol_name_option(Some("")).unwrap_err();
        assert!(symbol_err.to_string().contains("empty"));

        let budget_err = validate_token_budget(Some(-1)).unwrap_err();
        assert!(budget_err.to_string().contains("-1")); // Shows the actual value
    }
}
