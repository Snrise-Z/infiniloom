//! Content normalization for deterministic, cross-platform hashing
//!
//! This module ensures that the same code produces the same hash regardless of:
//! - Operating system (Windows CRLF, Unix LF, old Mac CR)
//! - Unicode representation (macOS NFD vs Linux NFC)
//! - Trailing whitespace differences
//! - Leading/trailing blank lines
//!
//! # Normalization Steps
//!
//! 1. **Unicode NFC normalization**: Converts decomposed characters (NFD) to composed form (NFC)
//!    - Example: "é" (e + combining accent) becomes "é" (single character)
//!    - Critical for macOS which often produces NFD in file names and content
//!
//! 2. **Line ending normalization**: CRLF (Windows) and CR (old Mac) → LF (Unix)
//!    - Ensures cross-platform consistency
//!
//! 3. **Trailing whitespace removal**: Strips spaces/tabs from line ends
//!    - Editors often differ in trailing whitespace handling
//!
//! 4. **Blank line trimming**: Removes leading and trailing blank lines
//!    - Keeps internal blank lines (they're semantically meaningful)
//!
//! 5. **Indentation preservation**: Internal indentation is kept intact
//!    - Critical for Python and other indentation-sensitive languages

use unicode_normalization::UnicodeNormalization;

/// Normalize content for deterministic, cross-platform hashing
///
/// # Guarantees
///
/// - Identical output on Windows, Linux, macOS
/// - Same code with different line endings produces same output
/// - Unicode-safe: NFD and NFC representations produce same output
/// - Preserves semantic structure (internal indentation, blank lines)
///
/// # Example
///
/// ```
/// use infiniloom_engine::embedding::normalize_for_hash;
///
/// let unix = "fn foo() {\n    bar();\n}";
/// let windows = "fn foo() {\r\n    bar();\r\n}";
///
/// assert_eq!(normalize_for_hash(unix), normalize_for_hash(windows));
/// ```
pub fn normalize_for_hash(content: &str) -> String {
    // Step 1: Unicode NFC normalization
    // This ensures "café" (NFD: e + combining accent) equals "café" (NFC: single char)
    let unicode_normalized: String = content.nfc().collect();

    // Step 2: Normalize line endings (optimize for common case - no \r)
    let line_normalized = if unicode_normalized.contains('\r') {
        unicode_normalized.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        unicode_normalized
    };

    // Step 3: Process lines - trim trailing whitespace only
    let lines: Vec<&str> = line_normalized
        .lines()
        .map(|line| line.trim_end()) // Remove trailing whitespace only
        .collect();

    // Step 4: Remove leading blank lines
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);

    // Step 5: Remove trailing blank lines
    let end = lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(0, |i| i + 1);

    // Handle empty content
    if start >= end {
        return String::new();
    }

    // Join the trimmed lines with LF
    lines[start..end].join("\n")
}

/// Fast check if content needs normalization
///
/// Returns `true` if the content might produce different hashes without normalization.
/// This is a quick heuristic check - it may return `true` for some content that
/// wouldn't actually change after normalization.
///
/// Use this for early-exit optimization when processing many files.
#[inline]
pub fn needs_normalization(content: &str) -> bool {
    // Check for carriage returns (Windows line endings or old Mac)
    if content.contains('\r') {
        return true;
    }

    // Check for potential Unicode that needs normalization
    // Any byte > 127 could be multi-byte UTF-8 that might need NFC
    if content.bytes().any(|b| b > 127) {
        return true;
    }

    // Check for trailing whitespace on any line
    for line in content.lines() {
        if line != line.trim_end() {
            return true;
        }
    }

    // Check for trailing newline (normalize_for_hash removes it)
    // Note: .lines() doesn't give us trailing empty lines, so check directly
    if content.ends_with('\n') {
        return true;
    }

    // Check for leading blank lines
    if content.starts_with('\n') {
        return true;
    }

    // Check for leading/trailing blank lines via .lines()
    let lines: Vec<&str> = content.lines().collect();
    if !lines.is_empty() {
        if lines.first().is_some_and(|l| l.is_empty()) {
            return true;
        }
        if lines.last().is_some_and(|l| l.is_empty()) {
            return true;
        }
    }

    false
}

/// Normalize a single line (without line ending changes)
///
/// Useful for processing content line by line.
#[inline]
pub(super) fn normalize_line(line: &str) -> String {
    line.nfc().collect::<String>().trim_end().to_owned()
}

/// Check if content is already normalized
///
/// Returns `true` if `normalize_for_hash(content) == content`.
/// More expensive than `needs_normalization` but more accurate.
pub(super) fn is_normalized(content: &str) -> bool {
    normalize_for_hash(content) == content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_nfc() {
        // NFD: e + combining acute accent
        let nfd = "cafe\u{0301}";
        // NFC: single character é
        let nfc = "caf\u{00E9}";

        assert_eq!(normalize_for_hash(nfd), normalize_for_hash(nfc));
    }

    #[test]
    fn test_cross_platform_line_endings() {
        let unix = "fn foo() {\n    bar();\n}";
        let windows = "fn foo() {\r\n    bar();\r\n}";
        let mac_classic = "fn foo() {\r    bar();\r}";
        let trailing_ws = "fn foo() {   \n    bar();   \n}";

        let normalized = normalize_for_hash(unix);
        assert_eq!(normalize_for_hash(windows), normalized);
        assert_eq!(normalize_for_hash(mac_classic), normalized);
        assert_eq!(normalize_for_hash(trailing_ws), normalized);
    }

    #[test]
    fn test_preserves_indentation() {
        let python = "def foo():\n    if True:\n        return 1";
        let normalized = normalize_for_hash(python);

        assert!(normalized.contains("    if True:"));
        assert!(normalized.contains("        return"));
    }

    #[test]
    fn test_removes_leading_blank_lines() {
        let with_leading = "\n\n\nfn foo() {}";
        let without = "fn foo() {}";

        assert_eq!(normalize_for_hash(with_leading), normalize_for_hash(without));
    }

    #[test]
    fn test_removes_trailing_blank_lines() {
        let with_trailing = "fn foo() {}\n\n\n";
        let without = "fn foo() {}";

        assert_eq!(normalize_for_hash(with_trailing), normalize_for_hash(without));
    }

    #[test]
    fn test_preserves_internal_blank_lines() {
        let code = "fn foo() {\n    let x = 1;\n\n    let y = 2;\n}";
        let normalized = normalize_for_hash(code);

        // Internal blank line should be preserved
        assert!(normalized.contains("\n\n"));
    }

    #[test]
    fn test_empty_content() {
        assert_eq!(normalize_for_hash(""), "");
        assert_eq!(normalize_for_hash("\n\n\n"), "");
        assert_eq!(normalize_for_hash("   \n   \n   "), "");
    }

    #[test]
    fn test_single_line() {
        let line = "fn foo() {}";
        assert_eq!(normalize_for_hash(line), line);
    }

    #[test]
    fn test_single_line_with_trailing_whitespace() {
        let with_ws = "fn foo() {}   ";
        let without = "fn foo() {}";
        assert_eq!(normalize_for_hash(with_ws), without);
    }

    #[test]
    fn test_needs_normalization() {
        // Needs normalization
        assert!(needs_normalization("fn foo() {\r\n}"));
        assert!(needs_normalization("café")); // Has non-ASCII
        assert!(needs_normalization("foo   \nbar"));
        assert!(needs_normalization("\nfoo"));
        assert!(needs_normalization("foo\n"));

        // Does NOT need normalization (already normalized)
        assert!(!needs_normalization("fn foo() {\n    bar();\n}"));
    }

    #[test]
    fn test_is_normalized() {
        assert!(is_normalized("fn foo() {\n    bar();\n}"));
        assert!(!is_normalized("fn foo() {\r\n    bar();\r\n}"));
        assert!(!is_normalized("fn foo() {}   "));
    }

    #[test]
    fn test_normalize_line() {
        assert_eq!(normalize_line("foo   "), "foo");
        assert_eq!(normalize_line("  foo  "), "  foo");
        assert_eq!(normalize_line("cafe\u{0301}"), "caf\u{00E9}");
    }

    #[test]
    fn test_mixed_line_endings() {
        // Mix of CRLF, LF, and CR
        let mixed = "line1\r\nline2\nline3\rline4";
        let normalized = normalize_for_hash(mixed);

        assert!(!normalized.contains('\r'));
        assert!(normalized.contains("line1\nline2\nline3\nline4"));
    }

    #[test]
    fn test_tabs_preserved() {
        let with_tabs = "fn foo() {\n\tbar();\n}";
        let normalized = normalize_for_hash(with_tabs);

        // Tabs should be preserved (they're indentation)
        assert!(normalized.contains('\t'));
    }

    #[test]
    fn test_unicode_identifiers() {
        // Some languages allow Unicode identifiers
        let code = "let αβγ = 42;";
        let normalized = normalize_for_hash(code);
        assert!(normalized.contains("αβγ"));
    }

    #[test]
    fn test_emoji_preserved() {
        // Some code has emoji in comments/strings
        let code = "// 🎉 Success!\nfn celebrate() {}";
        let normalized = normalize_for_hash(code);
        assert!(normalized.contains("🎉"));
    }

    #[test]
    fn test_deterministic_multiple_calls() {
        let content = "fn foo() {\r\n    bar();   \r\n}";

        let result1 = normalize_for_hash(content);
        let result2 = normalize_for_hash(content);
        let result3 = normalize_for_hash(content);

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn test_idempotent() {
        let content = "fn foo() {\r\n    bar();   \r\n}";
        let once = normalize_for_hash(content);
        let twice = normalize_for_hash(&once);
        let thrice = normalize_for_hash(&twice);

        assert_eq!(once, twice);
        assert_eq!(twice, thrice);
    }
}
