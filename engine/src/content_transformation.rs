//! Content transformation utilities for code compression and optimization
//!
//! This module provides functions for transforming source code content through
//! various techniques to optimize for LLM context windows:
//!
//! - **Empty line removal**: Strip unnecessary whitespace while preserving structure
//! - **Comment removal**: Language-aware removal of comments (13+ languages supported)
//! - **Signature extraction**: Extract only function/class declarations without bodies
//! - **Symbol-focused extraction**: Extract key symbols with optional context
//!
//! # Features
//!
//! - Line number preservation support (format: "123:line content")
//! - Language-specific syntax handling
//! - Symbol-aware extraction using AST information
//! - Heuristic fallbacks when symbol info unavailable
//!
//! # Examples
//!
//! ```no_run
//! use infiniloom_engine::content_transformation;
//!
//! // Remove empty lines
//! let code = "fn main() {\n\n    println!(\"hello\");\n\n}\n";
//! let compact = content_transformation::remove_empty_lines(code, false);
//!
//! // Remove comments
//! let code_with_comments = "// Comment\nfn main() {}\n";
//! let clean = content_transformation::remove_comments(code_with_comments, "rust", false);
//!
//! // Extract signatures only
//! let signatures = content_transformation::extract_signatures(code, "rust", &[]);
//! ```

use crate::{Symbol, SymbolKind, Visibility};
use std::collections::HashSet;

/// Remove empty lines from content
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
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::content_transformation::remove_empty_lines;
///
/// let code = "fn main() {\n\n    println!(\"hello\");\n}\n";
/// let compact = remove_empty_lines(code, false);
/// assert_eq!(compact, "fn main() {\n    println!(\"hello\");\n}");
/// ```
pub fn remove_empty_lines(content: &str, preserve_line_numbers: bool) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    if has_embedded_nums {
        if preserve_line_numbers {
            content
                .lines()
                .filter(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        !rest.trim().is_empty()
                    } else {
                        !line.trim().is_empty()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
                .lines()
                .filter_map(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        if !rest.trim().is_empty() {
                            Some(rest.to_owned())
                        } else {
                            None
                        }
                    } else if !line.trim().is_empty() {
                        Some(line.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if preserve_line_numbers {
        content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| format!("{}:{}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Check if the given text ends inside a string literal
///
/// Used to detect if a comment marker is inside a string and should not be removed.
/// Tracks both single and double quote states with escape handling.
///
/// # Arguments
///
/// * `text` - Text to analyze (typically the part before a comment marker)
///
/// # Returns
///
/// `true` if the text ends with an open string literal (unclosed quote)
fn is_inside_string(text: &str) -> bool {
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_backslash = false;

    for c in text.chars() {
        if prev_backslash {
            prev_backslash = false;
            continue;
        }
        match c {
            '\\' => prev_backslash = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            _ => {},
        }
    }

    let result = in_double || in_single;
    #[cfg(test)]
    eprintln!(
        "is_inside_string({:?}) = {} (in_double={}, in_single={})",
        text, result, in_double, in_single
    );
    result
}

/// Remove comments from content
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
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::content_transformation::remove_comments;
///
/// let code = "// Comment\nfn main() {\n    println!(\"hello\"); // inline\n}\n";
/// let clean = remove_comments(code, "rust", false);
/// assert!(clean.contains("fn main()"));
/// assert!(!clean.contains("// Comment"));
/// ```
pub fn remove_comments(content: &str, language: &str, preserve_line_numbers: bool) -> String {
    let (line_comment, block_start, block_end) = match language.to_lowercase().as_str() {
        "python" | "ruby" | "shell" | "bash" | "sh" | "yaml" | "yml" => ("#", "", ""),
        "javascript" | "typescript" | "java" | "c" | "cpp" | "c++" | "rust" | "go" | "swift"
        | "kotlin" | "scala" => ("//", "/*", "*/"),
        "html" | "xml" => ("", "<!--", "-->"),
        "css" | "scss" | "sass" => ("", "/*", "*/"),
        "sql" => ("--", "/*", "*/"),
        "lua" => ("--", "--[[", "]]"),
        _ => ("//", "/*", "*/"),
    };

    let format_line = |line_num: u32, content: &str| -> String {
        if preserve_line_numbers {
            format!("{}:{}\n", line_num, content)
        } else {
            format!("{}\n", content)
        }
    };

    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    let mut result = String::new();
    let mut in_block_comment = false;

    for (line_num, raw_line) in content.lines().enumerate() {
        let (original_line_num, line) = if has_embedded_nums {
            if let Some((num_str, rest)) = raw_line.split_once(':') {
                if let Ok(n) = num_str.parse::<u32>() {
                    (n, rest)
                } else {
                    (line_num as u32 + 1, raw_line)
                }
            } else {
                (line_num as u32 + 1, raw_line)
            }
        } else {
            (line_num as u32 + 1, raw_line)
        };

        let trimmed = line.trim();

        // Handle block comments
        if !block_start.is_empty() && !block_end.is_empty() {
            if in_block_comment {
                if let Some(idx) = line.find(block_end) {
                    in_block_comment = false;
                    let after_block = &line[idx + block_end.len()..];
                    if !after_block.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, after_block));
                    }
                }
                continue;
            }

            // Check for block comment start
            if let Some(idx) = line.find(block_start) {
                // Check if block ends on same line
                if let Some(end_idx) = line[idx + block_start.len()..].find(block_end) {
                    let before = &line[..idx];
                    let after = &line[idx + block_start.len() + end_idx + block_end.len()..];
                    let combined = format!("{}{}", before, after);
                    if !combined.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, &combined));
                    }
                    continue;
                } else {
                    in_block_comment = true;
                    let before = &line[..idx];
                    if !before.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, before.trim_end()));
                    }
                    continue;
                }
            }
        }

        // Skip line comments
        if !line_comment.is_empty() && trimmed.starts_with(line_comment) {
            continue;
        }

        // Handle inline line comments
        // Find the first occurrence of the comment marker that is NOT inside a string
        if !line_comment.is_empty() {
            let mut found_comment_idx = None;
            let mut search_start = 0;

            while let Some(idx) = line[search_start..].find(line_comment) {
                let absolute_idx = search_start + idx;
                let before = &line[..absolute_idx];
                if !is_inside_string(before) {
                    found_comment_idx = Some(absolute_idx);
                    break;
                }
                search_start = absolute_idx + line_comment.len();
            }

            if let Some(idx) = found_comment_idx {
                let cleaned = line[..idx].trim_end();
                if !cleaned.is_empty() {
                    result.push_str(&format_line(original_line_num, cleaned));
                }
                continue;
            }
        }

        result.push_str(&format_line(original_line_num, line));
    }

    result
}

/// Extract only function/class signatures from content
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
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::content_transformation::extract_signatures;
///
/// let code = "fn main() {\n    println!(\"hello\");\n}\n";
/// let sigs = extract_signatures(code, "rust", &[]);
/// // Returns: "fn main() {\n"
/// ```
pub fn extract_signatures(content: &str, language: &str, symbols: &[Symbol]) -> String {
    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut included_lines: HashSet<u32> = HashSet::new();

    for symbol in symbols {
        // Use pre-extracted signature if available
        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            // Fall back to extracting first line
            let line_idx = (symbol.start_line - 1) as usize;
            if !included_lines.contains(&symbol.start_line) {
                result.push_str(lines[line_idx]);
                result.push('\n');
                included_lines.insert(symbol.start_line);
            }
        }

        // Include docstring if available
        if let Some(ref doc) = symbol.docstring {
            if !doc.is_empty() {
                result.push_str("  // ");
                result.push_str(doc);
                result.push('\n');
            }
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

/// Heuristic extraction of signatures when no symbol info available
///
/// Looks for lines starting with language-specific keywords (def, fn, class, etc.).
/// Falls back to first 50 lines if no signatures found.
///
/// # Arguments
///
/// * `content` - Source code content
/// * `language` - Programming language
///
/// # Returns
///
/// Lines that look like function/class declarations.
pub fn extract_signatures_heuristic(content: &str, language: &str) -> String {
    let mut result = String::new();
    let signature_patterns: &[&str] = match language.to_lowercase().as_str() {
        "python" => &["def ", "class ", "async def "],
        "javascript" | "typescript" | "jsx" | "tsx" => {
            &["function ", "class ", "const ", "let ", "export ", "async "]
        },
        "rust" => &["fn ", "pub fn ", "struct ", "enum ", "trait ", "impl ", "type ", "const "],
        "go" => &["func ", "type ", "const ", "var "],
        "java" | "kotlin" => {
            &["public ", "private ", "protected ", "class ", "interface ", "enum "]
        },
        "c" | "cpp" | "c++" => &["void ", "int ", "char ", "bool ", "class ", "struct ", "enum "],
        _ => &["def ", "fn ", "func ", "function ", "class ", "struct "],
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if signature_patterns.iter().any(|p| trimmed.starts_with(p)) {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Fallback: first 50 lines if no signatures found
    if result.is_empty() {
        content.lines().take(50).collect::<Vec<_>>().join("\n")
    } else {
        result
    }
}

/// Extract only key public symbols (functions, classes, structs, etc.)
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
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::content_transformation::extract_key_symbols;
///
/// let code = "pub fn main() {}\npub fn helper() {}\nfn private() {}\n";
/// let key = extract_key_symbols(code, "rust", &[]);
/// // Returns only public functions
/// ```
pub fn extract_key_symbols(content: &str, language: &str, symbols: &[Symbol]) -> String {
    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();

    // Filter for key symbol types that are not private
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

    // Use key symbols if available, otherwise use first 20 non-import symbols
    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20)
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect()
    };

    for symbol in symbols_to_use {
        result.push_str(&format!("// {}: {}\n", symbol.kind.name(), symbol.name));

        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            let line_idx = (symbol.start_line - 1) as usize;
            result.push_str(lines[line_idx]);
            result.push('\n');
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

/// Extract key symbols with focused context (a few lines around each symbol)
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
///
/// # Examples
///
/// ```no_run
/// use infiniloom_engine::content_transformation::extract_key_symbols_with_context;
///
/// let code = "// Setup\npub fn main() {\n    println!(\"hello\");\n}\n// Cleanup\n";
/// let focused = extract_key_symbols_with_context(code, "rust", &[]);
/// // Returns function with 2 lines before and after
/// ```
pub fn extract_key_symbols_with_context(
    content: &str,
    language: &str,
    symbols: &[Symbol],
) -> String {
    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return String::new();
    }

    // Filter for key symbol types
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

    let mut ranges: Vec<SymbolRange> = Vec::new();
    let mut fallback_snippets: Vec<String> = Vec::new();

    // Build ranges with context
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
            let snippet = format!("// {}\n{}", label, sig.trim());
            fallback_snippets.push(snippet);
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    // Sort and merge overlapping ranges
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SymbolRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
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

    // Build output with merged ranges
    let mut result = String::new();
    for range in merged {
        let header = format!("// Focused symbols: {}\n", range.labels.join(", "));
        result.push_str(&header);

        let start_idx = range.start.saturating_sub(1) as usize;
        let end_idx = range.end.saturating_sub(1) as usize;
        if start_idx <= end_idx && end_idx < lines.len() {
            result.push_str(&lines[start_idx..=end_idx].join("\n"));
            result.push('\n');
        }
        result.push('\n');
    }

    // Add fallback snippets
    if !fallback_snippets.is_empty() {
        result.push_str("// Additional signatures\n");
        for snippet in fallback_snippets {
            result.push_str(&snippet);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_empty_lines_basic() {
        let content = "line1\n\nline2\n\n\nline3\n";
        let result = remove_empty_lines(content, false);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_remove_empty_lines_with_line_numbers() {
        let content = "1:line1\n2:\n3:line2\n";
        let result = remove_empty_lines(content, true);
        assert_eq!(result, "1:line1\n3:line2");
    }

    #[test]
    fn test_remove_empty_lines_preserve_numbers() {
        let content = "line1\n\nline2\n";
        let result = remove_empty_lines(content, true);
        assert_eq!(result, "1:line1\n3:line2");
    }

    #[test]
    fn test_is_inside_string_double_quotes() {
        assert!(is_inside_string("let x = \"hello"));
        assert!(!is_inside_string("let x = \"hello\""));
    }

    #[test]
    fn test_is_inside_string_single_quotes() {
        assert!(is_inside_string("let x = 'hello"));
        assert!(!is_inside_string("let x = 'hello'"));
    }

    #[test]
    fn test_is_inside_string_escaped() {
        assert!(!is_inside_string("let x = \"hello\\\"\""));
    }

    #[test]
    fn test_remove_comments_python() {
        let content = "# Comment\ndef main():\n    pass  # inline\n";
        let result = remove_comments(content, "python", false);
        assert!(!result.contains("# Comment"));
        assert!(result.contains("def main()"));
        assert!(result.contains("pass"));
    }

    #[test]
    fn test_remove_comments_rust() {
        let content = "// Comment\nfn main() {\n    println!(\"hello\"); // inline\n}\n";
        let result = remove_comments(content, "rust", false);
        assert!(!result.contains("// Comment"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_remove_comments_block() {
        let content = "/* Block\ncomment */\nfn main() {}\n";
        let result = remove_comments(content, "rust", false);
        assert!(!result.contains("/* Block"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_remove_comments_inline_block() {
        let content = "fn /* comment */ main() {}\n";
        let result = remove_comments(content, "rust", false);
        assert_eq!(result.trim(), "fn  main() {}");
    }

    #[test]
    fn test_extract_signatures_heuristic_python() {
        let content = "def foo():\n    pass\nclass Bar:\n    pass\n";
        let result = extract_signatures_heuristic(content, "python");
        assert!(result.contains("def foo()"));
        assert!(result.contains("class Bar"));
        assert!(!result.contains("pass"));
    }

    #[test]
    fn test_extract_signatures_heuristic_rust() {
        let content = "fn foo() {\n    1 + 1\n}\nstruct Bar {}\n";
        let result = extract_signatures_heuristic(content, "rust");
        assert!(result.contains("fn foo()"));
        assert!(result.contains("struct Bar"));
    }

    #[test]
    fn test_extract_signatures_heuristic_fallback() {
        let content = "some random text\nwith no signatures\n";
        let result = extract_signatures_heuristic(content, "rust");
        assert!(result.contains("some random text"));
    }

    // Additional tests for comment removal across multiple languages
    #[test]
    fn test_remove_comments_javascript() {
        let content = "// Comment\nfunction foo() {\n  console.log('test'); // inline\n}\n";
        let result = remove_comments(content, "javascript", false);
        assert!(!result.contains("// Comment"));
        assert!(!result.contains("// inline"));
        assert!(result.contains("function foo()"));
    }

    #[test]
    fn test_remove_comments_html() {
        let content = "<!-- Comment -->\n<div>Hello</div>\n<!-- Another -->\n";
        let result = remove_comments(content, "html", false);
        assert!(!result.contains("<!-- Comment -->"));
        assert!(!result.contains("<!-- Another -->"));
        assert!(result.contains("<div>Hello</div>"));
    }

    #[test]
    fn test_remove_comments_sql() {
        let content = "-- Comment\nSELECT * FROM users; -- inline\n/* Block comment */\n";
        let result = remove_comments(content, "sql", false);
        assert!(!result.contains("-- Comment"));
        assert!(!result.contains("-- inline"));
        assert!(!result.contains("/* Block comment */"));
        assert!(result.contains("SELECT * FROM users;"));
    }

    #[test]
    fn test_remove_comments_lua() {
        let content = "-- Comment\nlocal x = 5\n--[[ Block\ncomment ]]--\n";
        let result = remove_comments(content, "lua", false);
        assert!(!result.contains("-- Comment"));
        assert!(!result.contains("--[["));
        assert!(result.contains("local x = 5"));
    }

    #[test]
    fn test_remove_comments_with_line_numbers() {
        let content = "1:// Comment\n2:fn main() {\n3:    // inline\n4:}\n";
        let result = remove_comments(content, "rust", true);
        assert!(!result.contains("// Comment"));
        assert!(!result.contains("// inline"));
        assert!(result.contains("2:fn main()"));
    }

    #[test]
    fn test_remove_comments_preserves_strings() {
        let content = "let url = \"http://example.com\"; // real comment\n";
        let result = remove_comments(content, "rust", false);
        eprintln!("Input:  {:?}", content);
        eprintln!("Output: {:?}", result);
        eprintln!("Contains comment: {}", result.contains("// real comment"));
        assert!(result.contains("http://example.com"));
        assert!(!result.contains("// real comment"));
    }

    // Edge cases for empty line removal
    #[test]
    fn test_remove_empty_lines_all_empty() {
        let content = "\n\n\n\n";
        let result = remove_empty_lines(content, false);
        assert_eq!(result, "");
    }

    #[test]
    fn test_remove_empty_lines_whitespace_only() {
        let content = "line1\n  \t  \nline2\n";
        let result = remove_empty_lines(content, false);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_remove_empty_lines_with_embedded_numbers_no_preserve() {
        let content = "1:line1\n2:\n3:  \t  \n4:line2\n";
        let result = remove_empty_lines(content, false);
        assert_eq!(result, "line1\nline2");
    }

    // Tests for signature extraction with symbols
    #[test]
    fn test_extract_signatures_with_symbols() {
        let content = "fn foo() {\n    body\n}\nfn bar() {\n    body\n}\n";
        let symbols = vec![
            Symbol {
                name: "foo".to_string(),
                kind: SymbolKind::Function,
                start_line: 1,
                end_line: 3,
                signature: Some("fn foo()".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "bar".to_string(),
                kind: SymbolKind::Function,
                start_line: 4,
                end_line: 6,
                signature: Some("fn bar()".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
        ];
        let result = extract_signatures(content, "rust", &symbols);
        assert!(result.contains("fn foo()"));
        assert!(result.contains("fn bar()"));
        assert!(!result.contains("body"));
    }

    #[test]
    fn test_extract_signatures_with_docstrings() {
        let content = "fn foo() {\n    body\n}\n";
        let symbols = vec![Symbol {
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            start_line: 1,
            end_line: 3,
            signature: Some("fn foo()".to_string()),
            docstring: Some("Does something important".to_string()),
            visibility: Visibility::Public,
            references: 0,
            importance: 0.0,
            parent: None,
            calls: vec![],
            extends: None,
            implements: vec![],
        }];
        let result = extract_signatures(content, "rust", &symbols);
        assert!(result.contains("fn foo()"));
        assert!(result.contains("Does something important"));
    }

    // Tests for key symbol extraction
    #[test]
    fn test_extract_key_symbols_public_only() {
        let content = "pub fn foo() {}\nfn bar() {}\npub struct Baz {}\n";
        let symbols = vec![
            Symbol {
                name: "foo".to_string(),
                kind: SymbolKind::Function,
                start_line: 1,
                end_line: 1,
                signature: Some("pub fn foo()".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "bar".to_string(),
                kind: SymbolKind::Function,
                start_line: 2,
                end_line: 2,
                signature: Some("fn bar()".to_string()),
                docstring: None,
                visibility: Visibility::Private,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "Baz".to_string(),
                kind: SymbolKind::Struct,
                start_line: 3,
                end_line: 3,
                signature: Some("pub struct Baz".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
        ];
        let result = extract_key_symbols(content, "rust", &symbols);
        assert!(result.contains("foo"));
        assert!(!result.contains("bar")); // Private should be excluded
        assert!(result.contains("Baz"));
    }

    #[test]
    fn test_extract_key_symbols_fallback() {
        let content = "fn foo() {}\nfn bar() {}\n";
        let result = extract_key_symbols(content, "rust", &[]);
        // Should fall back to heuristic
        assert!(result.contains("fn foo()"));
        assert!(result.contains("fn bar()"));
    }

    #[test]
    fn test_extract_key_symbols_with_context() {
        let content = "// Setup\npub fn foo() {\n    body\n}\n// Cleanup\n";
        let symbols = vec![Symbol {
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            start_line: 2,
            end_line: 4,
            signature: Some("pub fn foo()".to_string()),
            docstring: None,
            visibility: Visibility::Public,
            references: 0,
            importance: 0.0,
            parent: None,
            calls: vec![],
            extends: None,
            implements: vec![],
        }];
        let result = extract_key_symbols_with_context(content, "rust", &symbols);
        assert!(result.contains("// Setup")); // Context before
        assert!(result.contains("pub fn foo()"));
        assert!(result.contains("// Cleanup")); // Context after
    }

    #[test]
    fn test_extract_key_symbols_with_context_merges_overlapping() {
        let content = "fn foo() {}\nfn bar() {}\nfn baz() {}\n";
        let symbols = vec![
            Symbol {
                name: "foo".to_string(),
                kind: SymbolKind::Function,
                start_line: 1,
                end_line: 1,
                signature: Some("fn foo()".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "bar".to_string(),
                kind: SymbolKind::Function,
                start_line: 2,
                end_line: 2,
                signature: Some("fn bar()".to_string()),
                docstring: None,
                visibility: Visibility::Public,
                references: 0,
                importance: 0.0,
                parent: None,
                calls: vec![],
                extends: None,
                implements: vec![],
            },
        ];
        let result = extract_key_symbols_with_context(content, "rust", &symbols);
        // Should merge overlapping context ranges
        assert!(result.contains("fn foo()"));
        assert!(result.contains("fn bar()"));
    }

    // Edge cases
    #[test]
    fn test_empty_content() {
        assert_eq!(remove_empty_lines("", false), "");
        assert_eq!(remove_comments("", "rust", false), "");
        assert_eq!(extract_signatures("", "rust", &[]), "");
    }

    #[test]
    fn test_unicode_handling() {
        let content = "// Комментарий\nfn main() {\n    println!(\"Привет\"); // 내용\n}\n";
        let result = remove_comments(content, "rust", false);
        assert!(!result.contains("Комментарий"));
        assert!(!result.contains("내용"));
        assert!(result.contains("Привет")); // Unicode in string preserved
    }

    #[test]
    fn test_extract_signatures_javascript() {
        let content = "function foo() {}\nconst bar = () => {}\nclass Baz {}\n";
        let result = extract_signatures_heuristic(content, "javascript");
        assert!(result.contains("function foo()"));
        assert!(result.contains("const bar"));
        assert!(result.contains("class Baz"));
    }

    #[test]
    fn test_extract_signatures_go() {
        let content = "func Foo() {}\ntype Bar struct {}\nconst Baz = 42\n";
        let result = extract_signatures_heuristic(content, "go");
        assert!(result.contains("func Foo()"));
        assert!(result.contains("type Bar"));
        assert!(result.contains("const Baz"));
    }
}
