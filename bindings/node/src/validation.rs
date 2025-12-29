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
