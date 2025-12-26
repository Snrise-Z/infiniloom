//! Shared diff and code context utilities for language bindings
//!
//! This module provides common functions used by both Python and Node.js bindings
//! for working with git diffs and extracting code context.

use infiniloom_engine::git::DiffHunk;
use std::collections::HashMap;
use std::path::Path;

/// Reconstruct unified diff content from hunks for a specific file.
/// This avoids making additional git subprocess calls.
///
/// # Arguments
/// * `hunks` - Slice of diff hunks from git operations
/// * `file_path` - Path to the file to reconstruct diff for
///
/// # Returns
/// Unified diff format string for the specified file
pub fn reconstruct_diff_from_hunks(hunks: &[DiffHunk], file_path: &str) -> String {
    let file_hunks: Vec<_> = hunks.iter().filter(|h| h.file == file_path).collect();
    if file_hunks.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str(&format!("diff --git a/{} b/{}\n", file_path, file_path));
    output.push_str(&format!("--- a/{}\n", file_path));
    output.push_str(&format!("+++ b/{}\n", file_path));

    for hunk in file_hunks {
        output.push_str(&hunk.header);
        output.push('\n');
        for line in &hunk.lines {
            let prefix = match line.change_type.as_str() {
                "add" => "+",
                "remove" => "-",
                _ => " ",
            };
            output.push_str(prefix);
            output.push_str(&line.content);
            output.push('\n');
        }
    }

    output
}

/// A cache for file content to avoid repeated disk reads.
pub type FileCache = HashMap<String, Vec<String>>;

/// Load file content into a line-by-line vector, using cache if available.
///
/// # Arguments
/// * `repo_root` - Repository root path
/// * `file_path` - Relative file path within repository
/// * `cache` - Mutable reference to file cache
///
/// # Returns
/// Option containing vector of lines, or None if file couldn't be read
pub fn load_file_lines<P: AsRef<Path>>(
    repo_root: P,
    file_path: &str,
    cache: &mut FileCache,
) -> Option<Vec<String>> {
    if let Some(cached) = cache.get(file_path) {
        return Some(cached.clone());
    }

    let full_path = repo_root.as_ref().join(file_path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let lines: Vec<String> = content.lines().map(String::from).collect();
            cache.insert(file_path.to_string(), lines.clone());
            Some(lines)
        },
        Err(_) => None,
    }
}

/// Get code context around a specific line with line numbers and markers.
///
/// # Arguments
/// * `repo_root` - Repository root path
/// * `file_path` - Relative file path within repository
/// * `line` - Target line number (1-indexed)
/// * `lines_before` - Number of context lines before target
/// * `lines_after` - Number of context lines after target
/// * `file_cache` - Mutable reference to file cache
///
/// # Returns
/// Tuple of (formatted context string, start line, end line) or (None, None, None) on error
pub fn get_line_context<P: AsRef<Path>>(
    repo_root: P,
    file_path: &str,
    line: u32,
    lines_before: usize,
    lines_after: usize,
    file_cache: &mut FileCache,
) -> (Option<String>, Option<u32>, Option<u32>) {
    let lines = match load_file_lines(repo_root, file_path, file_cache) {
        Some(l) => l,
        None => return (None, None, None),
    };

    if lines.is_empty() {
        return (None, None, None);
    }

    let line_idx = (line as usize).saturating_sub(1);
    let start_idx = line_idx.saturating_sub(lines_before);
    let end_idx = (line_idx + lines_after + 1).min(lines.len());

    if start_idx >= lines.len() {
        return (None, None, None);
    }

    let context_lines: Vec<String> = lines[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let line_num = start_idx + i + 1;
            let marker = if line_num == line as usize { ">" } else { " " };
            format!("{}{:4} | {}", marker, line_num, l)
        })
        .collect();

    (Some(context_lines.join("\n")), Some((start_idx + 1) as u32), Some(end_idx as u32))
}

/// Find a function call within a range of lines.
///
/// Searches for occurrences of `callee_name` followed by `(` (with optional whitespace),
/// while avoiding matches that are function definitions.
///
/// # Arguments
/// * `repo_root` - Repository root path
/// * `file_path` - Relative file path within repository
/// * `start_line` - Start line of search range (1-indexed)
/// * `end_line` - End line of search range (1-indexed)
/// * `callee_name` - Name of the function being called
/// * `file_cache` - Mutable reference to file cache
///
/// # Returns
/// Tuple of (line number, optional column) where call was found,
/// or (start_line, None) if not found
pub fn find_call_site_in_body<P: AsRef<Path>>(
    repo_root: P,
    file_path: &str,
    start_line: u32,
    end_line: u32,
    callee_name: &str,
    file_cache: &mut FileCache,
) -> (u32, Option<u32>) {
    let lines = match load_file_lines(repo_root, file_path, file_cache) {
        Some(l) => l,
        None => return (start_line, None),
    };

    let search_start = (start_line as usize).saturating_sub(1);
    let search_end = (end_line as usize).min(lines.len());

    for (i, line) in lines
        .iter()
        .enumerate()
        .skip(search_start)
        .take(search_end - search_start)
    {
        if let Some(col) = find_call_in_line(line, callee_name) {
            return ((i + 1) as u32, Some(col as u32));
        }
    }

    (start_line, None)
}

/// Find a function call in a single line of code.
///
/// Returns the column position if a call to `callee_name` is found.
/// Excludes function definitions (preceded by def, fn, function, etc.).
///
/// # Arguments
/// * `line` - Single line of code
/// * `callee_name` - Name of function being called
///
/// # Returns
/// Optional column position where call starts
pub fn find_call_in_line(line: &str, callee_name: &str) -> Option<usize> {
    let mut search_pos = 0;

    while let Some(pos) = line[search_pos..].find(callee_name) {
        let abs_pos = search_pos + pos;

        // Check if this is actually a call (followed by parenthesis)
        let after_name = abs_pos + callee_name.len();
        if after_name < line.len() {
            let rest = &line[after_name..];
            let next_non_ws = rest.trim_start();
            if next_non_ws.starts_with('(') {
                // Check it's not a definition (preceded by def/fn/function/etc.)
                let before = &line[..abs_pos];
                let before_trimmed = before.trim_end();

                // Skip if this is a function definition
                // Note: We check without trailing space since before_trimmed has been trimmed
                let is_definition = before_trimmed.ends_with("def")
                    || before_trimmed.ends_with("fn")
                    || before_trimmed.ends_with("function")
                    || before_trimmed.ends_with("func")
                    || before_trimmed == "async def"
                    || before_trimmed.ends_with(" def")  // async def, static def, etc.
                    || before_trimmed == "pub fn"
                    || before_trimmed.ends_with(" fn")   // pub fn, async fn, etc.
                    || before_trimmed == "async fn";

                if !is_definition {
                    // Verify it's a standalone identifier (not part of a larger word)
                    let is_word_boundary_before = abs_pos == 0
                        || !line
                            .chars()
                            .nth(abs_pos - 1)
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                    let is_word_boundary_after =
                        !callee_name.chars().next_back().is_some_and(|_| {
                            line.chars()
                                .nth(after_name)
                                .is_some_and(|c| c.is_alphanumeric() || c == '_')
                        });

                    if is_word_boundary_before && is_word_boundary_after {
                        return Some(abs_pos);
                    }
                }
            }
        }

        search_pos = abs_pos + 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconstruct_diff_empty() {
        let result = reconstruct_diff_from_hunks(&[], "test.rs");
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_call_in_line() {
        // Simple function call
        assert_eq!(find_call_in_line("foo()", "foo"), Some(0));
        assert_eq!(find_call_in_line("  foo()", "foo"), Some(2));
        assert_eq!(find_call_in_line("bar.foo()", "foo"), Some(4));

        // Function call with whitespace before paren
        assert_eq!(find_call_in_line("foo ()", "foo"), Some(0));

        // Not a call - definition
        assert_eq!(find_call_in_line("fn foo()", "foo"), None);
        assert_eq!(find_call_in_line("def foo():", "foo"), None);
        assert_eq!(find_call_in_line("pub fn foo()", "foo"), None);
        assert_eq!(find_call_in_line("async fn foo()", "foo"), None);

        // Not a call - part of larger word
        assert_eq!(find_call_in_line("foobar()", "foo"), None);
        assert_eq!(find_call_in_line("_foo()", "foo"), None);

        // Multiple occurrences - find the call, not the definition
        assert!(find_call_in_line("fn foo() { foo(); }", "foo").is_some());
    }
}
