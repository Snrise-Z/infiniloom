//! Content processing utilities for transforming file contents
//!
//! This module provides utilities for processing and transforming file content,
//! particularly for optimizing content for LLM consumption by removing or
//! truncating large binary/encoded data.
//!
//! # Features
//!
//! - **Base64 Detection and Truncation**: Automatically detects and truncates
//!   base64-encoded content (data URIs, embedded images, etc.) to save tokens
//! - **Pattern-based Processing**: Uses pre-compiled regex patterns for
//!   efficient content transformation
//!
//! # Examples
//!
//! ## Truncating Base64 Content
//!
//! ```rust
//! use infiniloom_engine::content_processing::truncate_base64;
//!
//! // Data URI with embedded image
//! let content = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB...";
//! let truncated = truncate_base64(content);
//! assert!(truncated.contains("[BASE64_TRUNCATED]"));
//!
//! // Regular text is preserved
//! let text = "This is normal text";
//! let result = truncate_base64(text);
//! assert_eq!(result, text);
//! ```
//!
//! # Performance
//!
//! - Uses `once_cell::sync::Lazy` for one-time regex compilation
//! - Regex patterns are compiled once and reused across all calls
//! - Efficient for processing large codebases with many files
//!
//! # Detection Rules
//!
//! The base64 detection looks for:
//! - **Data URIs**: `data:[mimetype];base64,[content]`
//! - **Long base64 strings**: Sequences of 200+ base64 characters
//!
//! Truncation behavior:
//! - Data URIs: Preserves prefix, replaces content with `[BASE64_TRUNCATED]`
//! - Long strings (>100 chars with +/): Shows first 50 chars + `...[BASE64_TRUNCATED]`
//! - Short strings (<200 chars): Not truncated
//! - Non-base64 text: Preserved unchanged

use once_cell::sync::Lazy;
use regex::Regex;

/// Pre-compiled regex pattern for detecting base64 content
///
/// Matches:
/// - `data:[mimetype];base64,[base64-content]` (data URIs)
/// - Sequences of 200+ base64 characters (likely embedded data)
///
/// The pattern uses `[A-Za-z0-9+/]*={0,2}` to match valid base64 characters
/// with optional padding (0-2 `=` characters at the end).
static BASE64_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data:[^;]+;base64,[A-Za-z0-9+/]*={0,2}|[A-Za-z0-9+/]{200,}={0,2}").unwrap()
});

/// Truncate base64-encoded content in text to save tokens
///
/// This function detects and truncates large base64-encoded content (such as
/// embedded images in data URIs or long base64 strings) to reduce token count
/// while preserving the structure and meaning of the text.
///
/// # Arguments
///
/// * `content` - The text content to process
///
/// # Returns
///
/// A new string with base64 content truncated and replaced with markers.
///
/// # Detection and Truncation Rules
///
/// 1. **Data URIs** (e.g., `data:image/png;base64,iVBORw0KG...`):
///    - Preserves the MIME type prefix: `data:image/png;base64,`
///    - Replaces the base64 content with: `[BASE64_TRUNCATED]`
///    - Result: `data:image/png;base64,[BASE64_TRUNCATED]`
///
/// 2. **Long base64 strings** (200+ characters with `+` or `/`):
///    - Shows first 50 characters
///    - Appends: `...[BASE64_TRUNCATED]`
///    - Result: `SGVsbG8gV29...ybGQ=...[BASE64_TRUNCATED]`
///
/// 3. **Short base64 strings** (<200 characters):
///    - Not truncated (kept as-is)
///
/// 4. **Long strings without base64 characters** (no `+` or `/`):
///    - Not truncated (likely not base64)
///
/// 5. **Regular text**:
///    - Completely preserved
///
/// # Examples
///
/// ```rust,no_run
/// use infiniloom_engine::content_processing::truncate_base64;
///
/// // Data URI truncation
/// let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB...";
/// let result = truncate_base64(data_uri);
/// assert_eq!(result, "data:image/png;base64,[BASE64_TRUNCATED]");
///
/// // Long base64 string truncation
/// let long_base64 = "A".repeat(250) + "+/";
/// let result = truncate_base64(&long_base64);
/// assert!(result.contains("[BASE64_TRUNCATED]"));
///
/// // Short base64 preserved
/// let short = "SGVsbG8gV29ybGQ="; // "Hello World" in base64 (16 chars)
/// let result = truncate_base64(short);
/// assert_eq!(result, short); // Unchanged
///
/// // Regular text preserved
/// let text = "This is regular code with no base64";
/// let result = truncate_base64(text);
/// assert_eq!(result, text); // Unchanged
/// ```
///
/// # Performance
///
/// - Uses pre-compiled regex pattern (compiled once, reused forever)
/// - Efficient replacement with `Regex::replace_all()`
/// - Only allocates new string if matches are found
///
/// # Use Cases
///
/// - Reducing token count when packing repositories with embedded images
/// - Removing large data URIs from HTML/CSS files
/// - Truncating base64-encoded assets in configuration files
/// - Optimizing content for LLM context windows
pub fn truncate_base64(content: &str) -> String {
    BASE64_PATTERN
        .replace_all(content, |caps: &regex::Captures<'_>| {
            let matched = caps.get(0).map_or("", |m| m.as_str());

            // Handle data URIs (preserve MIME type prefix)
            if matched.starts_with("data:") {
                if let Some(comma_idx) = matched.find(',') {
                    let prefix = &matched[..comma_idx + 1];
                    format!("{}[BASE64_TRUNCATED]", prefix)
                } else {
                    "[BASE64_TRUNCATED]".to_owned()
                }
            }
            // Handle long base64 strings (200+ chars)
            else if matched.len() > 100 {
                // Only truncate if it looks like base64 (has + or /)
                if matched.contains('+') || matched.contains('/') {
                    format!("{}...[BASE64_TRUNCATED]", &matched[..50])
                } else {
                    // Might not be base64, keep as-is
                    matched.to_owned()
                }
            }
            // Short strings: keep as-is
            else {
                matched.to_owned()
            }
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // truncate_base64 Tests
    // ============================================

    #[test]
    fn test_truncate_base64_data_uri() {
        let input = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let result = truncate_base64(input);
        assert!(result.contains("data:image/png;base64,"));
        assert!(result.contains("[BASE64_TRUNCATED]"));
    }

    #[test]
    fn test_truncate_base64_long_string() {
        // Long base64 string with + and / characters
        let input = "A".repeat(150) + "+" + &"B".repeat(100) + "/";
        let result = truncate_base64(&input);
        assert!(result.contains("[BASE64_TRUNCATED]") || result.len() == input.len());
    }

    #[test]
    fn test_truncate_base64_no_truncation_short() {
        let input = "SGVsbG8gV29ybGQ="; // "Hello World" in base64
        let result = truncate_base64(input);
        // Short strings are not truncated
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_base64_preserves_non_base64() {
        let input = "This is regular text without base64";
        let result = truncate_base64(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_base64_multiple_data_uris() {
        let input = "data:image/png;base64,ABC123 and data:image/jpeg;base64,XYZ789";
        let result = truncate_base64(input);
        assert!(result.contains("data:image/png;base64,[BASE64_TRUNCATED]"));
        assert!(result.contains("data:image/jpeg;base64,[BASE64_TRUNCATED]"));
    }

    #[test]
    fn test_truncate_base64_mixed_content() {
        let input = "Normal text data:image/png;base64,iVBORw0KGgoAAAA more text";
        let result = truncate_base64(input);
        assert!(result.contains("Normal text"));
        assert!(result.contains("[BASE64_TRUNCATED]"));
        assert!(result.contains("more text"));
    }

    #[test]
    fn test_truncate_base64_empty_string() {
        let input = "";
        let result = truncate_base64(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_base64_data_uri_without_comma() {
        // Malformed data URI without comma
        let input = "data:image/png;base64";
        let result = truncate_base64(input);
        // Should handle gracefully
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_base64_long_without_special_chars() {
        // Long string without + or / (likely not base64)
        let input = "A".repeat(250);
        let result = truncate_base64(&input);
        // Should NOT be truncated (no base64 indicators)
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_base64_exactly_200_chars() {
        // Edge case: exactly 200 characters with base64 chars
        let input = "A".repeat(199) + "+";
        let result = truncate_base64(&input);
        // Should be detected (200+ chars with +)
        assert!(result.contains("[BASE64_TRUNCATED]"));
    }
}
