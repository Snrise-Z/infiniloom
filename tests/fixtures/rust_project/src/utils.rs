//! Utility functions

/// Format a string with a prefix
pub fn format_with_prefix(prefix: &str, value: &str) -> String {
    format!("{}: {}", prefix, value)
}

/// Check if a string is empty or whitespace
pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}
