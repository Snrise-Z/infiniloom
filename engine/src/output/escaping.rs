//! Text escaping utilities for XML, YAML, and other output formats
//!
//! This module provides functions for escaping special characters in text
//! to ensure valid output in various formats (XML, YAML, etc.).
//!
//! # Overview
//!
//! Escaping is necessary when embedding user-provided text in structured
//! formats to prevent:
//! - Malformed output (broken XML/YAML syntax)
//! - Injection vulnerabilities
//! - Parsing errors
//!
//! # Usage
//!
//! ```rust
//! use infiniloom_engine::output::escaping::{escape_xml_text, escape_yaml_string};
//!
//! // XML escaping
//! let safe_xml = escape_xml_text("Foo & Bar <tag>");
//! assert_eq!(safe_xml, "Foo &amp; Bar &lt;tag&gt;");
//!
//! // YAML escaping
//! let safe_yaml = escape_yaml_string("Path: \"C:\\temp\\file.txt\"");
//! assert_eq!(safe_yaml, "\"Path: \\\"C:\\\\temp\\\\file.txt\\\"\"");
//! ```
//!
//! # Functions
//!
//! - [`escape_xml_text`]: Escape text content for XML/HTML (escapes &, <, >, ", ')
//! - [`escape_xml_attribute`]: Escape attribute values for XML/HTML (alias for escape_xml_text)
//! - [`escape_yaml_string`]: Escape and quote strings for YAML output
//!
//! # Implementation Notes
//!
//! ## XML Escaping Strategy
//!
//! We escape all five XML entities (&, <, >, ", ') for maximum compatibility:
//! - `&` → `&amp;` (required in all contexts)
//! - `<` → `&lt;` (required in text content)
//! - `>` → `&gt;` (required after `]]`)
//! - `"` → `&quot;` (required in attributes)
//! - `'` → `&apos;` (required in attributes)
//!
//! While some escapes are only required in specific contexts (e.g., quotes
//! in attributes), we escape them everywhere for simplicity and safety.
//!
//! ## YAML Escaping Strategy
//!
//! For YAML, we:
//! 1. Escape backslashes (`\` → `\\`)
//! 2. Escape quotes (`"` → `\"`)
//! 3. Wrap in double quotes
//!
//! This ensures the string is valid YAML and preserves all characters.
//!
//! # Performance
//!
//! All functions pre-allocate output strings with the same capacity as input
//! for efficiency. If escaping is needed, the string will grow dynamically.
//!
//! # Refactoring History
//!
//! This module was extracted from `cli/src/commands/pack/impl.rs` and
//! `cli/src/commands/diff.rs` in Phase 1, Item 3 of the refactoring effort.
//! Previously, these functions were duplicated across multiple files,
//! leading to ~30 lines of code duplication.

/// Escape text content for XML/HTML output
///
/// Replaces special XML characters with their entity equivalents:
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
/// - `'` → `&apos;`
///
/// # Arguments
///
/// * `input` - The text to escape
///
/// # Returns
///
/// A new string with all XML special characters replaced
///
/// # Examples
///
/// ```
/// use infiniloom_engine::output::escaping::escape_xml_text;
///
/// let escaped = escape_xml_text("Foo & Bar <tag>");
/// assert_eq!(escaped, "Foo &amp; Bar &lt;tag&gt;");
///
/// let escaped = escape_xml_text("if (a < b && c > d)");
/// assert_eq!(escaped, "if (a &lt; b &amp;&amp; c &gt; d)");
/// ```
///
/// # Performance
///
/// Pre-allocates output string with same capacity as input. If no escaping
/// is needed, this is optimal. If escaping is needed, the string grows
/// dynamically.
pub fn escape_xml_text(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Escape attribute values for XML/HTML output
///
/// Currently an alias for [`escape_xml_text`], as we use the same escaping
/// rules for both text content and attributes for simplicity and safety.
///
/// # Arguments
///
/// * `input` - The attribute value to escape
///
/// # Returns
///
/// A new string with all XML special characters replaced
///
/// # Examples
///
/// ```
/// use infiniloom_engine::output::escaping::escape_xml_attribute;
///
/// let escaped = escape_xml_attribute("value with \"quotes\"");
/// assert_eq!(escaped, "value with &quot;quotes&quot;");
/// ```
///
/// # Note
///
/// This is functionally identical to [`escape_xml_text`]. The separate
/// function exists for semantic clarity - callers can explicitly indicate
/// whether they're escaping text content or attribute values.
#[inline]
pub fn escape_xml_attribute(input: &str) -> String {
    escape_xml_text(input)
}

/// Escape and quote a string for YAML output
///
/// This function:
/// 1. Escapes backslashes: `\` → `\\`
/// 2. Escapes double quotes: `"` → `\"`
/// 3. Wraps the result in double quotes
///
/// The result is a properly escaped and quoted YAML string value.
///
/// # Arguments
///
/// * `input` - The string to escape
///
/// # Returns
///
/// A YAML-safe double-quoted string
///
/// # Examples
///
/// ```
/// use infiniloom_engine::output::escaping::escape_yaml_string;
///
/// // Basic escaping
/// let escaped = escape_yaml_string("Hello World");
/// assert_eq!(escaped, "\"Hello World\"");
///
/// // Backslash escaping
/// let escaped = escape_yaml_string("C:\\temp\\file.txt");
/// assert_eq!(escaped, "\"C:\\\\temp\\\\file.txt\"");
///
/// // Quote escaping
/// let escaped = escape_yaml_string("He said \"hello\"");
/// assert_eq!(escaped, "\"He said \\\"hello\\\"\"");
///
/// // Combined escaping
/// let escaped = escape_yaml_string("Path: \"C:\\temp\"");
/// assert_eq!(escaped, "\"Path: \\\"C:\\\\temp\\\"\"");
/// ```
///
/// # YAML Compatibility
///
/// The output is compatible with YAML 1.2 and will be correctly parsed
/// by standard YAML parsers (serde_yaml, PyYAML, etc.).
pub fn escape_yaml_string(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // escape_xml_text Tests
    // ============================================

    #[test]
    fn test_escape_xml_text_ampersand() {
        assert_eq!(escape_xml_text("foo & bar"), "foo &amp; bar");
        assert_eq!(escape_xml_text("&&&"), "&amp;&amp;&amp;");
    }

    #[test]
    fn test_escape_xml_text_less_than() {
        assert_eq!(escape_xml_text("a < b"), "a &lt; b");
        assert_eq!(escape_xml_text("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_escape_xml_text_greater_than() {
        assert_eq!(escape_xml_text("a > b"), "a &gt; b");
    }

    #[test]
    fn test_escape_xml_text_quotes() {
        assert_eq!(escape_xml_text("say \"hello\""), "say &quot;hello&quot;");
        assert_eq!(escape_xml_text("it's"), "it&apos;s");
    }

    #[test]
    fn test_escape_xml_text_multiple() {
        assert_eq!(
            escape_xml_text("<tag attr=\"val\" & more>"),
            "&lt;tag attr=&quot;val&quot; &amp; more&gt;"
        );
    }

    #[test]
    fn test_escape_xml_text_no_escaping_needed() {
        assert_eq!(escape_xml_text("hello world"), "hello world");
        assert_eq!(escape_xml_text(""), "");
        assert_eq!(escape_xml_text("123 abc XYZ"), "123 abc XYZ");
    }

    #[test]
    fn test_escape_xml_text_code_snippet() {
        let code = "if (a < b && c > d) { return \"ok\"; }";
        let escaped = escape_xml_text(code);
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&quot;"));
    }

    #[test]
    fn test_escape_xml_text_unicode() {
        // Unicode should pass through unchanged
        assert_eq!(escape_xml_text("Hello 世界"), "Hello 世界");
        assert_eq!(escape_xml_text("Emoji: 🚀"), "Emoji: 🚀");
    }

    // ============================================
    // escape_xml_attribute Tests
    // ============================================

    #[test]
    fn test_escape_xml_attr_delegates_to_text() {
        // escape_xml_attribute should behave identically to escape_xml_text
        assert_eq!(escape_xml_attribute("foo & bar"), escape_xml_text("foo & bar"));
        assert_eq!(escape_xml_attribute("<tag>"), escape_xml_text("<tag>"));
    }

    #[test]
    fn test_escape_xml_attr_quotes() {
        assert_eq!(escape_xml_attribute("value with \"quotes\""), "value with &quot;quotes&quot;");
    }

    // ============================================
    // escape_yaml_string Tests
    // ============================================

    #[test]
    fn test_escape_yaml_string_no_escaping() {
        assert_eq!(escape_yaml_string("hello"), "\"hello\"");
        assert_eq!(escape_yaml_string("Hello World"), "\"Hello World\"");
    }

    #[test]
    fn test_escape_yaml_string_backslash() {
        assert_eq!(escape_yaml_string("C:\\temp"), "\"C:\\\\temp\"");
        assert_eq!(escape_yaml_string("C:\\temp\\file.txt"), "\"C:\\\\temp\\\\file.txt\"");
        assert_eq!(escape_yaml_string("\\n\\t"), "\"\\\\n\\\\t\"");
    }

    #[test]
    fn test_escape_yaml_string_quotes() {
        assert_eq!(escape_yaml_string("He said \"hello\""), "\"He said \\\"hello\\\"\"");
        assert_eq!(escape_yaml_string("\"quoted\""), "\"\\\"quoted\\\"\"");
    }

    #[test]
    fn test_escape_yaml_string_combined() {
        assert_eq!(escape_yaml_string("Path: \"C:\\temp\""), "\"Path: \\\"C:\\\\temp\\\"\"");
        assert_eq!(
            escape_yaml_string("\"C:\\Program Files\\app\\\""),
            "\"\\\"C:\\\\Program Files\\\\app\\\\\\\"\""
        );
    }

    #[test]
    fn test_escape_yaml_string_empty() {
        assert_eq!(escape_yaml_string(""), "\"\"");
    }

    #[test]
    fn test_escape_yaml_string_unicode() {
        // Unicode should pass through unchanged (YAML supports UTF-8)
        assert_eq!(escape_yaml_string("Hello 世界"), "\"Hello 世界\"");
        assert_eq!(escape_yaml_string("Emoji: 🚀"), "\"Emoji: 🚀\"");
    }

    #[test]
    fn test_escape_yaml_string_special_yaml_chars() {
        // These characters have special meaning in YAML, but are safe in quoted strings
        assert_eq!(escape_yaml_string("key: value"), "\"key: value\"");
        assert_eq!(escape_yaml_string("- item"), "\"- item\"");
        assert_eq!(escape_yaml_string("# comment"), "\"# comment\"");
    }
}
