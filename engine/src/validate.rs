//! Path validation utilities for safe file output
//!
//! Provides validation functions to prevent path traversal attacks and other
//! unsafe file operations when writing output files. This is especially important
//! when paths originate from untrusted input (e.g., agent-invoked file output).

use std::path::{Path, PathBuf};

use crate::error::InfiniloomError;

/// Validates that an output path is safe for writing.
///
/// This function rejects paths that could escape the current working directory
/// through path traversal, absolute paths, or other unsafe patterns.
///
/// # Checks performed
///
/// 1. Rejects empty paths
/// 2. Rejects paths containing control characters (bytes < 0x20 except tab)
/// 3. Rejects absolute paths (e.g., `/etc/passwd`, `C:\Windows`)
/// 4. Rejects paths containing `..` components
/// 5. Resolves the path relative to the current working directory
/// 6. Ensures the resolved path is contained within the current working directory
///
/// # Returns
///
/// The resolved `PathBuf` on success, or `InfiniloomError::InvalidInput` on failure.
///
/// # Examples
///
/// ```rust,no_run
/// use infiniloom_engine::validate::validate_safe_output_path;
///
/// // Safe paths
/// let path = validate_safe_output_path("output.xml").unwrap();
/// let path = validate_safe_output_path("subdir/output.json").unwrap();
///
/// // Rejected paths
/// assert!(validate_safe_output_path("/etc/passwd").is_err());
/// assert!(validate_safe_output_path("../../../etc/passwd").is_err());
/// assert!(validate_safe_output_path("").is_err());
/// ```
#[allow(clippy::result_large_err)]
pub fn validate_safe_output_path(path: &str) -> Result<PathBuf, InfiniloomError> {
    // 1. Reject empty paths
    if path.is_empty() {
        return Err(InfiniloomError::invalid_input("Output path must not be empty"));
    }

    // 2. Reject control characters (bytes < 0x20 except tab 0x09)
    if path.bytes().any(|b| b < 0x20 && b != b'\t') {
        return Err(InfiniloomError::invalid_input("Output path contains control characters"));
    }

    let p = Path::new(path);

    // 3. Reject absolute paths
    if p.is_absolute() {
        return Err(InfiniloomError::invalid_input(format!(
            "Output path must be relative, got absolute path: {path}"
        )));
    }

    // 4. Reject paths containing ".." components
    for component in p.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(InfiniloomError::invalid_input(format!(
                "Output path must not contain '..': {path}"
            )));
        }
    }

    // 5. Resolve relative to CWD
    let cwd = std::env::current_dir().map_err(|e| {
        InfiniloomError::invalid_input(format!("Failed to determine current directory: {e}"))
    })?;

    let resolved = cwd.join(p);

    // 6. Canonicalize if the path (or its parent) exists, otherwise normalize manually.
    //    We need to handle the case where the file doesn't exist yet but the parent does.
    let canonical = if resolved.exists() {
        resolved.canonicalize().map_err(|e| {
            InfiniloomError::invalid_input(format!("Failed to resolve output path: {e}"))
        })?
    } else if let Some(parent) = resolved.parent() {
        if parent.exists() {
            let canonical_parent = parent.canonicalize().map_err(|e| {
                InfiniloomError::invalid_input(format!("Failed to resolve parent directory: {e}"))
            })?;
            let file_name = resolved
                .file_name()
                .expect("path has a file name after joining with CWD");
            canonical_parent.join(file_name)
        } else {
            // Parent doesn't exist - normalize by stripping redundant separators.
            // Since we already rejected ".." components, the path is safe.
            resolved
        }
    } else {
        resolved
    };

    // 7. Ensure resolved path starts with CWD
    // Always canonicalize CWD to handle symlinks consistently.
    let canonical_cwd = cwd.canonicalize().map_err(|e| {
        InfiniloomError::invalid_input(format!("Failed to resolve current directory: {e}"))
    })?;

    if !canonical.starts_with(&canonical_cwd) {
        return Err(InfiniloomError::invalid_input(format!(
            "Output path resolves outside the current directory: {path}"
        )));
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_filename() {
        let result = validate_safe_output_path("output.xml");
        assert!(result.is_ok());
        let path = result.unwrap();
        // Should end with the filename
        assert!(path.ends_with("output.xml"));
        // Should be absolute (resolved against CWD)
        assert!(path.is_absolute());
    }

    #[test]
    fn test_subdirectory_path() {
        let result = validate_safe_output_path("subdir/output.json");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("subdir/output.json"));
    }

    #[test]
    fn test_reject_empty_path() {
        let result = validate_safe_output_path("");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"), "Error should mention empty: {err}");
    }

    #[test]
    fn test_reject_absolute_path() {
        let result = validate_safe_output_path("/etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("absolute"), "Error should mention absolute: {err}");
    }

    #[test]
    fn test_reject_parent_traversal() {
        let result = validate_safe_output_path("../../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".."), "Error should mention '..': {err}");
    }

    #[test]
    fn test_reject_sneaky_traversal() {
        let result = validate_safe_output_path("foo/../../bar");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".."), "Error should mention '..': {err}");
    }

    #[test]
    fn test_reject_null_byte() {
        let result = validate_safe_output_path("output\0.xml");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("control"), "Error should mention control characters: {err}");
    }

    #[test]
    fn test_reject_control_characters() {
        // Test various control characters
        for byte in 0u8..0x20 {
            if byte == b'\t' {
                continue; // Tab is allowed
            }
            let path = format!("output{}file.xml", byte as char);
            let result = validate_safe_output_path(&path);
            assert!(result.is_err(), "Should reject control char 0x{byte:02x} in path");
        }
    }

    #[test]
    fn test_tab_is_allowed() {
        // Tab should not trigger the control character check.
        // It may still fail for other reasons (invalid path on some OS),
        // but it should NOT fail with "control characters" error.
        let result = validate_safe_output_path("out\tput.xml");
        match result {
            Ok(_) => {}, // Fine on systems that allow tabs in paths
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    !msg.contains("control"),
                    "Tab should not be rejected as control char: {msg}"
                );
            },
        }
    }

    #[test]
    fn test_nested_subdirectory() {
        let result = validate_safe_output_path("a/b/c/output.xml");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("a/b/c/output.xml"));
    }

    #[test]
    fn test_dotfile_is_allowed() {
        let result = validate_safe_output_path(".hidden_output.xml");
        assert!(result.is_ok());
    }

    #[test]
    fn test_current_dir_prefix_is_allowed() {
        let result = validate_safe_output_path("./output.xml");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("output.xml"));
    }
}
