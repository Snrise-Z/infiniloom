//! Shared utilities for Infiniloom language bindings
//!
//! This crate provides common functionality used by both Python and Node.js bindings,
//! reducing code duplication and ensuring consistent behavior across bindings.

use infiniloom_engine::{
    security::Severity, CompressionLevel, OutputFormat, Symbol, SymbolKind, TokenizerModel,
    Visibility,
};
use thiserror::Error;

// Re-export scanner module for use by bindings
pub mod scanner;
pub use scanner::{matches_any_pattern, matches_pattern, scan_repository, ScanConfig};

// Re-export repository operations module
pub mod repo_ops;
pub use repo_ops::{
    apply_compression, apply_default_ignores, apply_token_budget, prepare_repository,
    redact_secrets,
};

// Re-export diff utilities module
pub mod diff_utils;
pub use diff_utils::{
    find_call_in_line, find_call_site_in_body, get_line_context, load_file_lines,
    reconstruct_diff_from_hunks, FileCache,
};

/// Errors that can occur when parsing binding options
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unknown format: {0}. Use 'xml', 'markdown', 'json', 'yaml', 'toon', or 'plain'")]
    InvalidFormat(String),

    #[error("Unknown model: {0}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, codellama, mistral, deepseek, qwen, cohere, grok")]
    InvalidModel(String),

    #[error("Unknown compression: {0}. Use 'none', 'minimal', 'balanced', 'aggressive', 'extreme', 'focused', or 'semantic'")]
    InvalidCompression(String),
}

// ============================================================================
// Parsing Utilities
// ============================================================================

/// Parse output format string to OutputFormat enum
///
/// # Arguments
/// * `format` - Optional format string. Defaults to "xml" if None.
///
/// # Returns
/// The corresponding OutputFormat or an error if invalid.
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::parse_format;
///
/// assert!(parse_format(Some("xml")).is_ok());
/// assert!(parse_format(Some("markdown")).is_ok());
/// assert!(parse_format(None).is_ok()); // defaults to XML
/// assert!(parse_format(Some("invalid")).is_err());
/// ```
pub fn parse_format(format: Option<&str>) -> Result<OutputFormat, ParseError> {
    match format.unwrap_or("xml").to_lowercase().as_str() {
        "xml" => Ok(OutputFormat::Xml),
        "markdown" | "md" => Ok(OutputFormat::Markdown),
        "json" => Ok(OutputFormat::Json),
        "yaml" | "yml" => Ok(OutputFormat::Yaml),
        "toon" => Ok(OutputFormat::Toon),
        "plain" | "text" | "txt" => Ok(OutputFormat::Plain),
        other => Err(ParseError::InvalidFormat(other.to_owned())),
    }
}

/// Parse tokenizer model string to TokenizerModel enum
///
/// Supports all 27+ models with various alias forms (e.g., "gpt-4o", "gpt4o").
///
/// # Arguments
/// * `model` - Optional model string. Defaults to "claude" if None.
///
/// # Returns
/// The corresponding TokenizerModel or an error if invalid.
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::parse_model;
///
/// assert!(parse_model(Some("claude")).is_ok());
/// assert!(parse_model(Some("gpt-4o")).is_ok());
/// assert!(parse_model(Some("gpt4o")).is_ok()); // alias
/// assert!(parse_model(None).is_ok()); // defaults to Claude
/// ```
pub fn parse_model(model: Option<&str>) -> Result<TokenizerModel, ParseError> {
    match model.unwrap_or("claude").to_lowercase().as_str() {
        "claude" => Ok(TokenizerModel::Claude),
        // GPT-5.x series (latest)
        "gpt-5.2" | "gpt5.2" | "gpt52" => Ok(TokenizerModel::Gpt52),
        "gpt-5.2-pro" | "gpt52-pro" => Ok(TokenizerModel::Gpt52Pro),
        "gpt-5.1" | "gpt5.1" | "gpt51" => Ok(TokenizerModel::Gpt51),
        "gpt-5.1-mini" | "gpt51-mini" => Ok(TokenizerModel::Gpt51Mini),
        "gpt-5.1-codex" | "gpt51-codex" => Ok(TokenizerModel::Gpt51Codex),
        "gpt-5" | "gpt5" => Ok(TokenizerModel::Gpt5),
        "gpt-5-mini" | "gpt5-mini" => Ok(TokenizerModel::Gpt5Mini),
        "gpt-5-nano" | "gpt5-nano" => Ok(TokenizerModel::Gpt5Nano),
        // O-series reasoning models
        "o4-mini" => Ok(TokenizerModel::O4Mini),
        "o3" => Ok(TokenizerModel::O3),
        "o3-mini" => Ok(TokenizerModel::O3Mini),
        "o1" => Ok(TokenizerModel::O1),
        "o1-mini" => Ok(TokenizerModel::O1Mini),
        "o1-preview" => Ok(TokenizerModel::O1Preview),
        // GPT-4 series
        "gpt-4o" | "gpt4o" => Ok(TokenizerModel::Gpt4o),
        "gpt-4o-mini" | "gpt4o-mini" => Ok(TokenizerModel::Gpt4oMini),
        "gpt" | "gpt-4" | "gpt4" => Ok(TokenizerModel::Gpt4),
        "gpt-3.5-turbo" | "gpt35-turbo" | "gpt35turbo" => Ok(TokenizerModel::Gpt35Turbo),
        // Other vendors
        "gemini" => Ok(TokenizerModel::Gemini),
        "llama" => Ok(TokenizerModel::Llama),
        "codellama" => Ok(TokenizerModel::CodeLlama),
        "mistral" => Ok(TokenizerModel::Mistral),
        "deepseek" => Ok(TokenizerModel::DeepSeek),
        "qwen" => Ok(TokenizerModel::Qwen),
        "cohere" => Ok(TokenizerModel::Cohere),
        "grok" => Ok(TokenizerModel::Grok),
        other => Err(ParseError::InvalidModel(other.to_owned())),
    }
}

/// Parse compression level string to CompressionLevel enum
///
/// # Arguments
/// * `compression` - Optional compression string. Defaults to "balanced" if None.
///
/// # Returns
/// The corresponding CompressionLevel or an error if invalid.
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::parse_compression;
///
/// assert!(parse_compression(Some("balanced")).is_ok());
/// assert!(parse_compression(Some("aggressive")).is_ok());
/// assert!(parse_compression(None).is_ok()); // defaults to balanced
/// ```
pub fn parse_compression(compression: Option<&str>) -> Result<CompressionLevel, ParseError> {
    match compression.unwrap_or("balanced").to_lowercase().as_str() {
        "none" => Ok(CompressionLevel::None),
        "minimal" => Ok(CompressionLevel::Minimal),
        "balanced" => Ok(CompressionLevel::Balanced),
        "aggressive" => Ok(CompressionLevel::Aggressive),
        "extreme" => Ok(CompressionLevel::Extreme),
        "focused" => Ok(CompressionLevel::Focused),
        "semantic" => Ok(CompressionLevel::Semantic),
        other => Err(ParseError::InvalidCompression(other.to_owned())),
    }
}

// ============================================================================
// Content Compression Utilities
// ============================================================================

/// Extract function/class signature lines from content
///
/// Filters content to only include lines that define functions, classes,
/// structs, traits, interfaces, etc. across multiple languages.
///
/// # Arguments
/// * `content` - Source code content
///
/// # Returns
/// String containing only signature lines joined by newlines
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::signature_lines;
///
/// let code = "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() {}";
/// let sigs = signature_lines(code);
/// assert!(sigs.contains("fn main()"));
/// assert!(sigs.contains("fn helper()"));
/// assert!(!sigs.contains("println"));
/// ```
pub fn signature_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("export ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("type ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract focused symbol context from content
///
/// For each public symbol, extracts a few lines of context around it.
/// This provides more context than `signature_lines` while still being
/// much smaller than the full file content.
///
/// # Arguments
/// * `content` - Source code content
/// * `symbols` - Parsed symbols from the file
///
/// # Returns
/// String with focused context around key symbols
pub fn focused_symbol_context(content: &str, symbols: &[Symbol]) -> String {
    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return signature_lines(content);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return String::new();
    }

    // Filter to key symbols (public functions, classes, etc.)
    let key_symbols: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Function
                    | SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::Interface
            ) && s.visibility != Visibility::Private
        })
        .collect();

    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        // Fallback: use any non-import symbols
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20)
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect()
    };

    #[derive(Clone)]
    struct SymbolRange {
        start: u32,
        end: u32,
        labels: Vec<String>,
    }

    let mut ranges = Vec::new();
    let mut fallback_snippets = Vec::new();

    for symbol in symbols_to_use {
        let label = format!("{}: {}", symbol.kind.name(), symbol.name);
        if symbol.start_line > 0
            && symbol.end_line >= symbol.start_line
            && symbol.start_line <= total_lines
        {
            let start = symbol.start_line.saturating_sub(CONTEXT_LINES).max(1);
            let end = symbol
                .end_line
                .max(symbol.start_line)
                .saturating_add(CONTEXT_LINES)
                .min(total_lines);
            ranges.push(SymbolRange { start, end, labels: vec![label] });
        } else if let Some(ref sig) = symbol.signature {
            fallback_snippets.push(format!("// {}\n{}", label, sig.trim()));
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return signature_lines(content);
    }

    // Sort and merge overlapping ranges
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SymbolRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                // Merge overlapping ranges
                last.end = last.end.max(range.end);
                for label in range.labels {
                    if !last.labels.contains(&label) {
                        last.labels.push(label);
                    }
                }
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    // Build result
    let mut result = String::new();
    for range in merged {
        result.push_str(&format!("// Focused symbols: {}\n", range.labels.join(", ")));
        let start_idx = range.start.saturating_sub(1) as usize;
        let end_idx = range.end.saturating_sub(1) as usize;
        if start_idx <= end_idx && end_idx < lines.len() {
            result.push_str(&lines[start_idx..=end_idx].join("\n"));
            result.push('\n');
        }
        result.push('\n');
    }

    if !fallback_snippets.is_empty() {
        result.push_str("// Additional signatures\n");
        for snippet in fallback_snippets {
            result.push_str(&snippet);
            result.push('\n');
        }
    }

    result
}

// ============================================================================
// File Utilities
// ============================================================================

/// Calculate priority score for a file path
///
/// Returns a score between 0.0 and 1.0 based on the file's likely importance:
/// - 1.0: Core entry points (main, index, app in src/)
/// - 0.8: Other source files (src/, lib/)
/// - 0.6: Config files (json, yaml, toml)
/// - 0.5: Default/unknown
/// - 0.3: Test files
/// - 0.2: Documentation
///
/// # Arguments
/// * `path` - File path to score
///
/// # Returns
/// Priority score between 0.0 and 1.0
pub fn file_priority_score(path: &str) -> f64 {
    let path_lower = path.to_lowercase();

    // Core source files
    if path_lower.contains("src/") || path_lower.contains("lib/") {
        if path_lower.contains("main") || path_lower.contains("index") || path_lower.contains("app")
        {
            return 1.0;
        }
        return 0.8;
    }

    // Config files
    if path_lower.ends_with(".json")
        || path_lower.ends_with(".yaml")
        || path_lower.ends_with(".toml")
    {
        return 0.6;
    }

    // Test files
    if path_lower.contains("test") || path_lower.contains("spec") {
        return 0.3;
    }

    // Docs
    if path_lower.contains("doc") || path_lower.ends_with(".md") {
        return 0.2;
    }

    0.5
}

/// Format git file status as a string
///
/// # Arguments
/// * `status` - File status from git operations
///
/// # Returns
/// Human-readable status string
pub fn format_file_status(status: infiniloom_engine::git::FileStatus) -> &'static str {
    use infiniloom_engine::git::FileStatus;
    match status {
        FileStatus::Added => "Added",
        FileStatus::Modified => "Modified",
        FileStatus::Deleted => "Deleted",
        FileStatus::Renamed => "Renamed",
        FileStatus::Copied => "Copied",
        FileStatus::Unknown => "Unknown",
    }
}

// ============================================================================
// Security Utilities
// ============================================================================

/// Error type for security threshold parsing
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Unknown security threshold: {0}. Use 'critical', 'high', 'medium', or 'low'")]
    InvalidThreshold(String),
}

/// Parse security severity threshold string
///
/// # Arguments
/// * `threshold` - Optional threshold string. Defaults to "critical" if None.
///
/// # Returns
/// The corresponding Severity or an error if invalid.
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::parse_security_threshold;
///
/// assert!(parse_security_threshold(Some("critical")).is_ok());
/// assert!(parse_security_threshold(Some("high")).is_ok());
/// assert!(parse_security_threshold(None).is_ok()); // defaults to critical
/// ```
pub fn parse_security_threshold(threshold: Option<&str>) -> Result<Severity, SecurityError> {
    match threshold.unwrap_or("critical").to_lowercase().as_str() {
        "critical" => Ok(Severity::Critical),
        "high" => Ok(Severity::High),
        "medium" => Ok(Severity::Medium),
        "low" => Ok(Severity::Low),
        other => Err(SecurityError::InvalidThreshold(other.to_owned())),
    }
}

/// Check if a severity is at or above a threshold
///
/// Returns true if the severity is as severe or more severe than the threshold.
///
/// # Arguments
/// * `severity` - The severity to check
/// * `threshold` - The minimum severity threshold
///
/// # Returns
/// True if severity >= threshold
///
/// # Examples
/// ```
/// use infiniloom_bindings_common::severity_at_or_above;
/// use infiniloom_engine::security::Severity;
///
/// assert!(severity_at_or_above(&Severity::Critical, &Severity::High));
/// assert!(severity_at_or_above(&Severity::High, &Severity::High));
/// assert!(!severity_at_or_above(&Severity::Low, &Severity::High));
/// ```
pub fn severity_at_or_above(severity: &Severity, threshold: &Severity) -> bool {
    let severity_level = match severity {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    };
    let threshold_level = match threshold {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    };
    severity_level >= threshold_level
}

// ============================================================================
// Time Utilities
// ============================================================================

/// Format Unix timestamp to ISO 8601 string
///
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
/// ISO 8601 formatted date string (e.g., "2024-01-01T00:00:00Z")
pub fn format_timestamp(timestamp: u64) -> String {
    // Convert Unix timestamp to ISO 8601 format
    // timestamp is seconds since 1970-01-01 00:00:00 UTC
    let secs_per_min = 60u64;
    let secs_per_hour = 3600u64;
    let secs_per_day = 86400u64;

    // Days since epoch
    let mut remaining = timestamp;
    let days = remaining / secs_per_day;
    remaining %= secs_per_day;

    // Time of day
    let hours = remaining / secs_per_hour;
    remaining %= secs_per_hour;
    let minutes = remaining / secs_per_min;
    let seconds = remaining % secs_per_min;

    // Calculate year, month, day from days since epoch
    // Using a simplified algorithm for Gregorian calendar
    let mut year = 1970i32;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Calculate month and day
    let leap = is_leap_year(year);
    let days_in_months: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for days_in_month in days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    let day = remaining_days as u32 + 1;

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

/// Check if a year is a leap year
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format() {
        assert!(matches!(parse_format(Some("xml")), Ok(OutputFormat::Xml)));
        assert!(matches!(parse_format(Some("markdown")), Ok(OutputFormat::Markdown)));
        assert!(matches!(parse_format(Some("md")), Ok(OutputFormat::Markdown)));
        assert!(matches!(parse_format(Some("json")), Ok(OutputFormat::Json)));
        assert!(matches!(parse_format(Some("yaml")), Ok(OutputFormat::Yaml)));
        assert!(matches!(parse_format(Some("yml")), Ok(OutputFormat::Yaml)));
        assert!(matches!(parse_format(Some("toon")), Ok(OutputFormat::Toon)));
        assert!(matches!(parse_format(Some("plain")), Ok(OutputFormat::Plain)));
        assert!(matches!(parse_format(None), Ok(OutputFormat::Xml))); // default
        assert!(parse_format(Some("invalid")).is_err());
    }

    #[test]
    fn test_parse_model() {
        assert!(matches!(parse_model(Some("claude")), Ok(TokenizerModel::Claude)));
        assert!(matches!(parse_model(Some("gpt-4o")), Ok(TokenizerModel::Gpt4o)));
        assert!(matches!(parse_model(Some("gpt4o")), Ok(TokenizerModel::Gpt4o)));
        assert!(matches!(parse_model(Some("gpt-5.2")), Ok(TokenizerModel::Gpt52)));
        assert!(matches!(parse_model(Some("o3")), Ok(TokenizerModel::O3)));
        assert!(matches!(parse_model(None), Ok(TokenizerModel::Claude))); // default
        assert!(parse_model(Some("invalid")).is_err());
    }

    #[test]
    fn test_parse_compression() {
        assert!(matches!(parse_compression(Some("none")), Ok(CompressionLevel::None)));
        assert!(matches!(parse_compression(Some("minimal")), Ok(CompressionLevel::Minimal)));
        assert!(matches!(parse_compression(Some("balanced")), Ok(CompressionLevel::Balanced)));
        assert!(matches!(parse_compression(Some("aggressive")), Ok(CompressionLevel::Aggressive)));
        assert!(matches!(parse_compression(Some("extreme")), Ok(CompressionLevel::Extreme)));
        assert!(matches!(parse_compression(Some("focused")), Ok(CompressionLevel::Focused)));
        assert!(matches!(parse_compression(Some("semantic")), Ok(CompressionLevel::Semantic)));
        assert!(matches!(parse_compression(None), Ok(CompressionLevel::Balanced))); // default
        assert!(parse_compression(Some("invalid")).is_err());
    }

    #[test]
    fn test_signature_lines() {
        let code = r#"
fn main() {
    println!("hello");
}

pub fn helper() -> i32 {
    42
}

class MyClass:
    def method(self):
        pass
"#;
        let sigs = signature_lines(code);
        assert!(sigs.contains("fn main()"));
        assert!(sigs.contains("pub fn helper()"));
        assert!(sigs.contains("class MyClass:"));
        assert!(sigs.contains("def method(self):"));
        assert!(!sigs.contains("println"));
        assert!(!sigs.contains("42"));
    }

    #[test]
    fn test_file_priority_score() {
        assert_eq!(file_priority_score("src/main.rs"), 1.0);
        assert_eq!(file_priority_score("src/index.ts"), 1.0);
        assert_eq!(file_priority_score("lib/app.py"), 1.0);
        assert_eq!(file_priority_score("src/utils.rs"), 0.8);
        assert_eq!(file_priority_score("config.json"), 0.6);
        assert_eq!(file_priority_score("tests/test_main.py"), 0.3);
        assert_eq!(file_priority_score("README.md"), 0.2);
        assert_eq!(file_priority_score("random.txt"), 0.5);
    }

    #[test]
    fn test_parse_security_threshold() {
        assert!(matches!(parse_security_threshold(Some("critical")), Ok(Severity::Critical)));
        assert!(matches!(parse_security_threshold(Some("high")), Ok(Severity::High)));
        assert!(matches!(parse_security_threshold(Some("medium")), Ok(Severity::Medium)));
        assert!(matches!(parse_security_threshold(Some("low")), Ok(Severity::Low)));
        assert!(matches!(parse_security_threshold(None), Ok(Severity::Critical))); // default
        assert!(parse_security_threshold(Some("invalid")).is_err());
    }

    #[test]
    fn test_severity_at_or_above() {
        assert!(severity_at_or_above(&Severity::Critical, &Severity::Critical));
        assert!(severity_at_or_above(&Severity::Critical, &Severity::High));
        assert!(severity_at_or_above(&Severity::Critical, &Severity::Low));
        assert!(severity_at_or_above(&Severity::High, &Severity::High));
        assert!(severity_at_or_above(&Severity::High, &Severity::Medium));
        assert!(!severity_at_or_above(&Severity::Low, &Severity::High));
        assert!(!severity_at_or_above(&Severity::Medium, &Severity::Critical));
    }

    #[test]
    fn test_format_timestamp() {
        // Unix epoch - 1970-01-01T00:00:00Z
        let ts = format_timestamp(0);
        assert_eq!(ts, "1970-01-01T00:00:00Z");

        // Test a recent timestamp (2024-01-01 00:00:00 UTC = 1704067200)
        let ts2 = format_timestamp(1704067200);
        assert_eq!(ts2, "2024-01-01T00:00:00Z");

        // Test with time components (2024-06-15 14:10:45 UTC)
        let ts3 = format_timestamp(1718460645);
        assert_eq!(ts3, "2024-06-15T14:10:45Z");

        // Different timestamps should produce different output
        assert_ne!(ts, ts2);
    }
}
