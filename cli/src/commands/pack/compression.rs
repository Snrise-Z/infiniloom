//! Content compression and transformation functions for pack command
//!
//! This module provides thin wrappers around the centralized content transformation
//! functions from `infiniloom_engine::content_transformation`.
//!
//! All transformation logic has been centralized in the engine for reusability.
//! This module maintains backward compatibility by providing the same function
//! signatures that were previously implemented here.
//!
//! # Functions
//!
//! - `remove_empty_lines_from_content` - Wrapper for `content_transformation::remove_empty_lines`
//! - `remove_comments_from_content` - Wrapper for `content_transformation::remove_comments`
//! - `extract_signatures_only` - Wrapper for `content_transformation::extract_signatures`
//! - `extract_key_symbols_only` - Wrapper for `content_transformation::extract_key_symbols`
//! - `extract_key_symbols_focused` - Wrapper for `content_transformation::extract_key_symbols_with_context`

use infiniloom_engine::content_transformation;
use infiniloom_engine::Symbol;

/// Remove empty lines from content
///
/// Thin wrapper around `content_transformation::remove_empty_lines`.
/// Maintains backward compatibility with existing pack command code.
///
/// Handles both content with embedded line numbers (format: "123:line content")
/// and regular content without line numbers.
///
/// # Arguments
///
/// * `content` - Content to process
/// * `preserve_line_numbers` - Whether to preserve line numbers in output
///
/// # Returns
///
/// Content with empty lines removed, optionally with line numbers preserved.
pub(crate) fn remove_empty_lines_from_content(
    content: &str,
    preserve_line_numbers: bool,
) -> String {
    content_transformation::remove_empty_lines(content, preserve_line_numbers)
}

/// Remove comments from content
///
/// Thin wrapper around `content_transformation::remove_comments`.
/// Maintains backward compatibility with existing pack command code.
///
/// Removes both line comments and block comments based on language syntax.
/// Handles embedded line numbers and preserves line numbers if requested.
///
/// Supports 13+ languages including:
/// - Python, Ruby, Shell (# comments)
/// - JavaScript, TypeScript, Rust, Go, C/C++, Java (// and /* */ comments)
/// - HTML/XML (<!-- --> comments)
/// - CSS/SCSS (/* */ comments)
/// - SQL (-- and /* */ comments)
/// - Lua (-- and --[[ ]] comments)
///
/// # Arguments
///
/// * `content` - Content to process
/// * `language` - Programming language (determines comment syntax)
/// * `preserve_line_numbers` - Whether to preserve line numbers in output
///
/// # Returns
///
/// Content with comments removed.
pub(crate) fn remove_comments_from_content(
    content: &str,
    language: &str,
    preserve_line_numbers: bool,
) -> String {
    content_transformation::remove_comments(content, language, preserve_line_numbers)
}

/// Extract only function/class signatures from content
///
/// Thin wrapper around `content_transformation::extract_signatures`.
/// Maintains backward compatibility with existing pack command code.
///
/// Uses symbol information if available, falls back to heuristics.
/// Extracts declarations without function bodies for maximum compression.
///
/// # Arguments
///
/// * `content` - Source code content
/// * `language` - Programming language
/// * `symbols` - Extracted symbols (from AST parsing)
///
/// # Returns
///
/// Content with only signatures (no function bodies).
pub(crate) fn extract_signatures_only(content: &str, language: &str, symbols: &[Symbol]) -> String {
    content_transformation::extract_signatures(content, language, symbols)
}

/// Extract only key public symbols (functions, classes, structs, etc.)
///
/// Thin wrapper around `content_transformation::extract_key_symbols`.
/// Maintains backward compatibility with existing pack command code.
///
/// Filters for important, public symbols and includes just their signatures.
/// Prioritizes:
/// - Public functions, classes, structs, traits, enums, interfaces
/// - Up to 30 key symbols
/// - Falls back to first 20 non-import symbols if no public symbols found
///
/// # Arguments
///
/// * `content` - Source code content
/// * `language` - Programming language
/// * `symbols` - Extracted symbols (from AST parsing)
///
/// # Returns
///
/// Content with only key public symbols (up to 30).
pub(crate) fn extract_key_symbols_only(
    content: &str,
    language: &str,
    symbols: &[Symbol],
) -> String {
    content_transformation::extract_key_symbols(content, language, symbols)
}

/// Extract key symbols with focused context (a few lines around each symbol)
///
/// Thin wrapper around `content_transformation::extract_key_symbols_with_context`.
/// Maintains backward compatibility with existing pack command code.
///
/// Provides more context than signatures-only, but less than full content.
/// Merges overlapping ranges for efficiency.
///
/// # Arguments
///
/// * `content` - Source code content
/// * `language` - Programming language
/// * `symbols` - Extracted symbols (from AST parsing)
///
/// # Returns
///
/// Content with key symbols and 2 lines of context before/after each.
pub(crate) fn extract_key_symbols_focused(
    content: &str,
    language: &str,
    symbols: &[Symbol],
) -> String {
    content_transformation::extract_key_symbols_with_context(content, language, symbols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrapper_remove_empty_lines() {
        let content = "line1\n\nline2\n";
        let result = remove_empty_lines_from_content(content, false);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_wrapper_remove_comments() {
        let content = "// Comment\nfn main() {}\n";
        let result = remove_comments_from_content(content, "rust", false);
        assert!(!result.contains("// Comment"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_wrapper_extract_signatures() {
        let content = "fn foo() {\n    body\n}\n";
        let result = extract_signatures_only(content, "rust", &[]);
        // Should fall back to heuristic
        assert!(result.contains("fn foo()"));
    }

    #[test]
    fn test_wrapper_extract_key_symbols() {
        let content = "fn foo() {}\nfn bar() {}\n";
        let result = extract_key_symbols_only(content, "rust", &[]);
        assert!(result.contains("fn foo()"));
    }

    #[test]
    fn test_wrapper_extract_key_symbols_focused() {
        let content = "fn foo() {\n    body\n}\n";
        let result = extract_key_symbols_focused(content, "rust", &[]);
        assert!(result.contains("fn foo()"));
    }
}
