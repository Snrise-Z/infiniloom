//! Diff context operations
//!
//! This module provides context-aware diff operations:
//! - Getting context for git diffs with symbol analysis
//! - Expanding context to include related symbols and tests
//! - Formatting diff context in multiple output formats

use crate::types::{
    ContextSymbolInfo, DiffContextOptions, DiffContextResult, DiffFileContext,
};
use crate::utils::format_file_status;
use crate::validation::validate_path;
use infiniloom_bindings_common::{parse_format, reconstruct_diff_from_hunks as common_reconstruct_diff_from_hunks};
use infiniloom_engine::{
    git::{
        ChangedFile, DiffHunk as EngineGitDiffHunk, FileStatus as EngineFileStatus,
        GitRepo as EngineGitRepo,
    },
    index::{ChangeType, ContextDepth, ContextExpander, DiffChange, IndexStorage},
    tokenizer::{TokenModel, Tokenizer},
    OutputFormat,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Get context-aware diff with surrounding symbols and dependencies
///
/// Unlike basic diffFiles, this provides semantic context around changes.
/// Requires an index (will build on-the-fly if not present).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch (use "" for unstaged changes)
/// * `to_ref` - Ending commit/branch (use "HEAD" for staged, "" for working tree)
/// * `options` - Optional context options
///
/// # Returns
/// Context-aware diff result with related symbols
///
/// # Example
/// ```javascript
/// const { getDiffContext } = require('infiniloom-node');
///
/// // Get context for last commit
/// const context = getDiffContext('./my-repo', 'HEAD~1', 'HEAD', {
///   depth: 2,
///   budget: 50000,
///   includeDiff: true
/// });
///
/// console.log(`Changed: ${context.changedFiles.length} files`);
/// console.log(`Related symbols: ${context.contextSymbols.length}`);
/// console.log(`Related tests: ${context.relatedTests.length}`);
/// ```
#[napi]
pub fn get_diff_context(
    path: String,
    from_ref: String,
    to_ref: String,
    options: Option<DiffContextOptions>,
) -> Result<DiffContextResult> {
    // Input validation
    validate_path(&path)?;
    // Note: from_ref and to_ref can be empty strings (means uncommitted changes)

    let opts = options.unwrap_or(DiffContextOptions {
        depth: None,
        budget: None,
        include_diff: None,
        format: None,
        model: None,
        exclude: None,
        include: None,
    });

    let path_buf = PathBuf::from(&path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e))
    })?;

    // Get changed files
    let changed: Vec<ChangedFile> = if from_ref.is_empty() && to_ref.is_empty() {
        // Uncommitted changes
        git_repo
            .status()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
            .iter()
            .map(|f| ChangedFile {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: f.status,
                additions: 0,
                deletions: 0,
            })
            .collect()
    } else {
        let from = if from_ref.is_empty() {
            "HEAD"
        } else {
            &from_ref
        };
        let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };
        git_repo
            .diff_files(from, to)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    };

    // Try to load existing index, or use lazy context builder
    let storage = IndexStorage::new(&path_buf);
    let include_diff = opts.include_diff.unwrap_or(false);

    // OPTIMIZATION: Get all hunks in one git call instead of per-file
    // This dramatically improves performance for diffs with many files
    let from = if from_ref.is_empty() {
        "HEAD"
    } else {
        &from_ref
    };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    let all_hunks: Vec<EngineGitDiffHunk> = if from_ref.is_empty() && to_ref.is_empty() {
        git_repo.uncommitted_hunks(None).unwrap_or_default()
    } else {
        git_repo.diff_hunks(from, to, None).unwrap_or_default()
    };

    // Group hunks by file path and extract line ranges
    let mut file_line_ranges: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut file_diff_contents: HashMap<String, String> = HashMap::new();

    // Build hunks-by-file map for efficient lookup
    let mut hunks_by_file: HashMap<&str, Vec<&EngineGitDiffHunk>> = HashMap::new();
    for hunk in &all_hunks {
        hunks_by_file.entry(&hunk.file).or_default().push(hunk);
    }

    // Process each changed file using pre-fetched hunks
    for file in &changed {
        if let Some(hunks) = hunks_by_file.get(file.path.as_str()) {
            // Extract line ranges from hunks
            let mut line_ranges = Vec::new();
            for hunk in hunks {
                if hunk.new_count > 0 {
                    line_ranges.push((hunk.new_start, hunk.new_start + hunk.new_count - 1));
                }
            }
            if !line_ranges.is_empty() {
                file_line_ranges.insert(file.path.clone(), line_ranges);
            }

            // Reconstruct diff content from hunks (avoids additional git call)
            let diff_content = common_reconstruct_diff_from_hunks(&all_hunks, &file.path);
            if !diff_content.is_empty() {
                file_diff_contents.insert(file.path.clone(), diff_content);
            }
        }
    }

    // Build file contexts
    let mut changed_files: Vec<DiffFileContext> = Vec::new();
    for file in &changed {
        let diff_content = if include_diff {
            file_diff_contents.get(&file.path).cloned()
        } else {
            None
        };

        changed_files.push(DiffFileContext {
            path: file.path.clone(),
            change_type: format_file_status(file.status),
            additions: file.additions,
            deletions: file.deletions,
            diff: diff_content,
            context_snippets: vec![],
        });
    }

    // Try to expand context if index exists
    let mut context_symbols: Vec<ContextSymbolInfo> = Vec::new();
    let mut related_tests: Vec<String> = Vec::new();
    let mut file_snippets: HashMap<String, Vec<String>> = HashMap::new();

    // Bug #1, #2, #3 fixes: Improved context expansion
    if let (Ok(index), Ok(graph)) = (storage.load_index(), storage.load_graph()) {
        let depth = match opts.depth.unwrap_or(2) {
            1 => ContextDepth::L1,
            2 => ContextDepth::L2,
            _ => ContextDepth::L3,
        };

        let expander = ContextExpander::new(&index, &graph);
        let changes: Vec<DiffChange> = changed
            .iter()
            .map(|f| {
                // Get line ranges from diff hunks
                let mut line_ranges = file_line_ranges.get(&f.path).cloned().unwrap_or_default();

                // Bug #1 fix: If no line ranges found but file exists in index, include all lines
                // This ensures we capture symbols even when hunk parsing fails or for new files
                if line_ranges.is_empty() {
                    if let Some(file_entry) = index.get_file(&f.path) {
                        // For added files, include all lines
                        if f.status == EngineFileStatus::Added {
                            line_ranges = vec![(1, file_entry.lines.max(1))];
                        } else if f.status != EngineFileStatus::Deleted {
                            // For modified files with no hunks, include all symbol ranges
                            let symbols = index.get_file_symbols(file_entry.id);
                            if symbols.is_empty() {
                                // No symbols found - include entire file
                                line_ranges = vec![(1, file_entry.lines.max(1))];
                            } else {
                                line_ranges = symbols
                                    .iter()
                                    .map(|s| (s.span.start_line, s.span.end_line))
                                    .collect();
                            }
                        }
                    } else {
                        // File not in index yet - include as entire file change
                        line_ranges = vec![(1, 10000)]; // Large range to capture all symbols
                    }
                }

                DiffChange {
                    file_path: f.path.clone(),
                    old_path: f.old_path.clone(),
                    line_ranges,
                    change_type: match f.status {
                        EngineFileStatus::Added => ChangeType::Added,
                        EngineFileStatus::Deleted => ChangeType::Deleted,
                        _ => ChangeType::Modified,
                    },
                    diff_content: file_diff_contents.get(&f.path).cloned(),
                }
            })
            .collect();

        let token_budget = opts.budget.unwrap_or(50000);
        let context = expander.expand(&changes, depth, token_budget);

        // Combine changed and dependent symbols
        context_symbols = context
            .changed_symbols
            .iter()
            .chain(context.dependent_symbols.iter())
            .map(|s| ContextSymbolInfo {
                name: s.name.clone(),
                kind: s.kind.clone(),
                file: s.file_path.clone(),
                line: s.start_line,
                reason: s.relevance_reason.clone(),
                signature: s.signature.clone(),
            })
            .collect();

        related_tests = context
            .related_tests
            .iter()
            .map(|f| f.path.clone())
            .collect();

        // Bug #2 fix: Always try direct test detection (expander may miss some tests)
        {
            let mut seen_tests: HashSet<String> = related_tests.iter().cloned().collect();

            // Helper to check if a file is a test file
            let is_test_file = |path: &str| -> bool {
                let path_lower = path.to_lowercase();
                path_lower.contains("test")
                    || path_lower.contains("spec")
                    || path_lower.contains("__tests__")
                    || path_lower.ends_with("_test.rs")
                    || path_lower.ends_with("_test.go")
                    || path_lower.ends_with("_test.py")
                    || path_lower.ends_with(".test.ts")
                    || path_lower.ends_with(".test.js")
                    || path_lower.ends_with(".spec.ts")
                    || path_lower.ends_with(".spec.js")
            };

            for changed_file in &changed {
                // Method 1: Find test files that import the changed file
                if let Some(file_entry) = index.get_file(&changed_file.path) {
                    let importers = graph.get_importers(file_entry.id.as_u32());
                    for importer_id in importers {
                        if let Some(importer_file) = index.get_file_by_id(importer_id) {
                            if is_test_file(&importer_file.path)
                                && seen_tests.insert(importer_file.path.clone())
                            {
                                related_tests.push(importer_file.path.clone());
                            }
                        }
                    }
                }

                // Method 2: Find test files by naming convention
                let path_lower = changed_file.path.to_lowercase();
                let base_name = std::path::Path::new(&path_lower)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if !base_name.is_empty() {
                    let test_patterns = [
                        format!("{}_test.", base_name),
                        format!("test_{}", base_name),
                        format!("{}.test.", base_name),
                        format!("{}.spec.", base_name),
                        format!("test/{}", base_name),
                        format!("tests/{}", base_name),
                        format!("__tests__/{}", base_name),
                    ];

                    for indexed_file in &index.files {
                        if is_test_file(&indexed_file.path) {
                            let file_lower = indexed_file.path.to_lowercase();
                            for pattern in &test_patterns {
                                if file_lower.contains(pattern)
                                    && seen_tests.insert(indexed_file.path.clone())
                                {
                                    related_tests.push(indexed_file.path.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Bug #3 fix: Generate snippets directly from changed files
        // The expander's snippets field is not populated, so we generate them here
        for cf in &changed_files {
            let full_path = path_buf.join(&cf.path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let lines: Vec<&str> = content.lines().collect();
                let mut snippets = Vec::new();

                // Get line ranges for this file
                if let Some(ranges) = file_line_ranges.get(&cf.path) {
                    // Generate snippet for each changed section with context
                    for (start, end) in ranges {
                        let context_lines = 3; // Lines of context before/after
                        let snippet_start = (*start as usize).saturating_sub(context_lines + 1);
                        let snippet_end = ((*end as usize) + context_lines).min(lines.len());

                        if snippet_start < lines.len() && snippet_start < snippet_end {
                            let snippet_content = lines[snippet_start..snippet_end].join("\n");
                            if !snippet_content.is_empty() {
                                snippets.push(format!(
                                    "// Lines {}-{}\n{}",
                                    snippet_start + 1,
                                    snippet_end,
                                    snippet_content
                                ));
                            }
                        }
                    }
                }

                if !snippets.is_empty() {
                    file_snippets.insert(cf.path.clone(), snippets);
                }
            }
        }
    }

    // Update changed_files with snippets from context expansion
    for file_ctx in &mut changed_files {
        if let Some(snippets) = file_snippets.remove(&file_ctx.path) {
            file_ctx.context_snippets = snippets;
        }
    }

    // Calculate tokens
    let tokenizer = Tokenizer::new();
    let total_content: String = changed_files
        .iter()
        .filter_map(|f| f.diff.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let total_tokens = tokenizer.count(&total_content, TokenModel::Claude);

    // Generate formatted output if format is specified (Bug #1 fix)
    let formatted_output = if let Some(ref format_str) = opts.format {
        let format = parse_format(Some(format_str))
            .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;
        Some(format_diff_context(&changed_files, &context_symbols, &related_tests, format))
    } else {
        None
    };

    Ok(DiffContextResult {
        changed_files,
        context_symbols,
        related_tests,
        formatted_output,
        total_tokens,
    })
}

/// Format diff context result into a specific output format (Bug #1 fix)
fn format_diff_context(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Xml => format_diff_context_xml(changed_files, context_symbols, related_tests),
        OutputFormat::Markdown => {
            format_diff_context_markdown(changed_files, context_symbols, related_tests)
        },
        OutputFormat::Json => {
            format_diff_context_json(changed_files, context_symbols, related_tests)
        },
        OutputFormat::Yaml => {
            format_diff_context_yaml(changed_files, context_symbols, related_tests)
        },
        _ => format_diff_context_plain(changed_files, context_symbols, related_tests),
    }
}

fn format_diff_context_xml(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("<diff-context>\n");

    // Changed files
    output.push_str("  <changed-files>\n");
    for file in changed_files {
        output.push_str(&format!(
            "    <file path=\"{}\" change=\"{}\" additions=\"{}\" deletions=\"{}\">\n",
            file.path, file.change_type, file.additions, file.deletions
        ));
        if !file.context_snippets.is_empty() {
            output.push_str("      <context>\n");
            for snippet in &file.context_snippets {
                output.push_str(&format!("        <snippet><![CDATA[{}]]></snippet>\n", snippet));
            }
            output.push_str("      </context>\n");
        }
        if let Some(ref diff) = file.diff {
            output.push_str(&format!("      <diff><![CDATA[{}]]></diff>\n", diff));
        }
        output.push_str("    </file>\n");
    }
    output.push_str("  </changed-files>\n");

    // Context symbols
    if !context_symbols.is_empty() {
        output.push_str("  <context-symbols>\n");
        for sym in context_symbols {
            output.push_str(&format!(
                "    <symbol name=\"{}\" kind=\"{}\" file=\"{}\" line=\"{}\" reason=\"{}\"",
                sym.name, sym.kind, sym.file, sym.line, sym.reason
            ));
            if let Some(ref sig) = sym.signature {
                output.push_str(&format!(" signature=\"{}\"", sig.replace('"', "&quot;")));
            }
            output.push_str("/>\n");
        }
        output.push_str("  </context-symbols>\n");
    }

    // Related tests
    if !related_tests.is_empty() {
        output.push_str("  <related-tests>\n");
        for test in related_tests {
            output.push_str(&format!("    <test>{}</test>\n", test));
        }
        output.push_str("  </related-tests>\n");
    }

    output.push_str("</diff-context>\n");
    output
}

fn format_diff_context_markdown(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("# Diff Context\n\n");

    // Changed files
    output.push_str("## Changed Files\n\n");
    for file in changed_files {
        output.push_str(&format!(
            "### {} ({}: +{} -{} )\n\n",
            file.path, file.change_type, file.additions, file.deletions
        ));
        if !file.context_snippets.is_empty() {
            output.push_str("**Context:**\n");
            for snippet in &file.context_snippets {
                output.push_str(&format!("```\n{}\n```\n\n", snippet));
            }
        }
        if let Some(ref diff) = file.diff {
            output.push_str("**Diff:**\n");
            output.push_str(&format!("```diff\n{}\n```\n\n", diff));
        }
    }

    // Context symbols
    if !context_symbols.is_empty() {
        output.push_str("## Related Symbols\n\n");
        output.push_str("| Name | Kind | File | Line | Reason |\n");
        output.push_str("|------|------|------|------|--------|\n");
        for sym in context_symbols {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                sym.name, sym.kind, sym.file, sym.line, sym.reason
            ));
        }
        output.push('\n');
    }

    // Related tests
    if !related_tests.is_empty() {
        output.push_str("## Related Tests\n\n");
        for test in related_tests {
            output.push_str(&format!("- {}\n", test));
        }
        output.push('\n');
    }

    output
}

fn format_diff_context_json(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
) -> String {
    // Create a JSON structure
    let json = serde_json::json!({
        "changedFiles": changed_files.iter().map(|f| {
            serde_json::json!({
                "path": f.path,
                "changeType": f.change_type,
                "additions": f.additions,
                "deletions": f.deletions,
                "contextSnippets": f.context_snippets,
                "diff": f.diff
            })
        }).collect::<Vec<_>>(),
        "contextSymbols": context_symbols.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "file": s.file,
                "line": s.line,
                "reason": s.reason,
                "signature": s.signature
            })
        }).collect::<Vec<_>>(),
        "relatedTests": related_tests
    });
    serde_json::to_string_pretty(&json).unwrap_or_default()
}

fn format_diff_context_yaml(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("---\n");
    output.push_str("diff_context:\n");

    // Changed files
    output.push_str("  changed_files:\n");
    for file in changed_files {
        output.push_str(&format!("    - path: {}\n", file.path));
        output.push_str(&format!("      change_type: {}\n", file.change_type));
        output.push_str(&format!("      additions: {}\n", file.additions));
        output.push_str(&format!("      deletions: {}\n", file.deletions));
        if !file.context_snippets.is_empty() {
            output.push_str("      context_snippets:\n");
            for snippet in &file.context_snippets {
                output.push_str(&format!(
                    "        - |\n          {}\n",
                    snippet.replace('\n', "\n          ")
                ));
            }
        }
        if let Some(ref diff) = file.diff {
            output.push_str(&format!(
                "      diff: |\n        {}\n",
                diff.replace('\n', "\n        ")
            ));
        }
    }

    // Context symbols
    if !context_symbols.is_empty() {
        output.push_str("  context_symbols:\n");
        for sym in context_symbols {
            output.push_str(&format!("    - name: {}\n", sym.name));
            output.push_str(&format!("      kind: {}\n", sym.kind));
            output.push_str(&format!("      file: {}\n", sym.file));
            output.push_str(&format!("      line: {}\n", sym.line));
            output.push_str(&format!("      reason: {}\n", sym.reason));
            if let Some(ref sig) = sym.signature {
                output.push_str(&format!("      signature: {}\n", sig));
            }
        }
    }

    // Related tests
    if !related_tests.is_empty() {
        output.push_str("  related_tests:\n");
        for test in related_tests {
            output.push_str(&format!("    - {}\n", test));
        }
    }

    output
}

fn format_diff_context_plain(
    changed_files: &[DiffFileContext],
    context_symbols: &[ContextSymbolInfo],
    related_tests: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("DIFF CONTEXT\n");
    output.push_str(&"=".repeat(60));
    output.push('\n');

    // Changed files
    output.push_str("\nCHANGED FILES:\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');
    for file in changed_files {
        output.push_str(&format!(
            "{} ({}: +{} -{})\n",
            file.path, file.change_type, file.additions, file.deletions
        ));
        if !file.context_snippets.is_empty() {
            output.push_str("  Context:\n");
            for snippet in &file.context_snippets {
                output.push_str(&format!("    {}\n", snippet.replace('\n', "\n    ")));
            }
        }
        if let Some(ref diff) = file.diff {
            output.push_str("  Diff:\n");
            output.push_str(&format!("    {}\n", diff.replace('\n', "\n    ")));
        }
    }

    // Context symbols
    if !context_symbols.is_empty() {
        output.push_str("\nRELATED SYMBOLS:\n");
        output.push_str(&"-".repeat(40));
        output.push('\n');
        for sym in context_symbols {
            output.push_str(&format!(
                "  {} ({}) in {}:{} [{}]\n",
                sym.name, sym.kind, sym.file, sym.line, sym.reason
            ));
        }
    }

    // Related tests
    if !related_tests.is_empty() {
        output.push_str("\nRELATED TESTS:\n");
        output.push_str(&"-".repeat(40));
        output.push('\n');
        for test in related_tests {
            output.push_str(&format!("  {}\n", test));
        }
    }

    output
}
