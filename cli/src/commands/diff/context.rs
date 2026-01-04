//! Context enrichment for diff command
//!
//! This module provides functions to enrich diff context with code snippets
//! and apply token budget constraints.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use infiniloom_engine::index::{ChangeType, ContextSnippet, ExpandedContext};
use infiniloom_engine::tokenizer::{TokenModel, Tokenizer};

use super::formatting::{line_contains_symbol_name, merge_snippet_ranges, SnippetRange};
use super::git_ops::read_file_from_git;

/// Enrich diff context with code snippets for changed and dependent files
pub(crate) fn enrich_diff_context(
    repo_path: &Path,
    changes: &[infiniloom_engine::index::DiffChange],
    base_ref: Option<&str>,
    context: &mut ExpandedContext,
    token_model: TokenModel,
) -> Result<()> {
    const CONTEXT_LINES: u32 = 3;
    let tokenizer = Tokenizer::new();
    let mut change_by_path: HashMap<String, &infiniloom_engine::index::DiffChange> = HashMap::new();
    for change in changes {
        change_by_path.insert(change.file_path.clone(), change);
        if let Some(old_path) = &change.old_path {
            change_by_path.insert(old_path.clone(), change);
        }
    }

    let mut changed_symbols_by_file: HashMap<
        String,
        Vec<&infiniloom_engine::index::ContextSymbol>,
    > = HashMap::new();
    for sym in &context.changed_symbols {
        changed_symbols_by_file
            .entry(sym.file_path.clone())
            .or_default()
            .push(sym);
    }

    let mut dependent_symbols_by_file: HashMap<
        String,
        Vec<&infiniloom_engine::index::ContextSymbol>,
    > = HashMap::new();
    for sym in &context.dependent_symbols {
        dependent_symbols_by_file
            .entry(sym.file_path.clone())
            .or_default()
            .push(sym);
    }

    let mut file_lines_cache: HashMap<String, Vec<String>> = HashMap::new();

    let changed_symbol_names: Vec<String> = context
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let mut process_file = |file: &mut infiniloom_engine::index::ContextFile, is_test: bool| {
        let lines = if let Some(lines) = file_lines_cache.get(&file.path) {
            lines.clone()
        } else {
            let full_path = repo_path.join(&file.path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    let lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();
                    file_lines_cache.insert(file.path.clone(), lines.clone());
                    lines
                },
                Err(_) => {
                    let fallback_content = if let Some(change) = change_by_path.get(&file.path) {
                        if let Some(ref_path) = base_ref {
                            if let Some(old_path) = &change.old_path {
                                read_file_from_git(repo_path, ref_path, old_path)
                                    .or_else(|| read_file_from_git(repo_path, ref_path, &file.path))
                            } else {
                                read_file_from_git(repo_path, ref_path, &file.path)
                            }
                        } else {
                            None
                        }
                    } else if let Some(ref_path) = base_ref {
                        read_file_from_git(repo_path, ref_path, &file.path)
                    } else {
                        None
                    };

                    if let Some(content) = fallback_content {
                        let lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();
                        file_lines_cache.insert(file.path.clone(), lines.clone());
                        lines
                    } else {
                        file.snippets = Vec::new();
                        file.tokens = file
                            .diff_content
                            .as_deref()
                            .map_or(0, |d| tokenizer.count(d, token_model));
                        return;
                    }
                },
            }
        };

        let total_lines = lines.len() as u32;
        if total_lines == 0 {
            return;
        }

        let mut ranges: Vec<SnippetRange> = Vec::new();

        for (start, end) in &file.relevant_sections {
            let start = start.saturating_sub(CONTEXT_LINES).max(1);
            let end = end.saturating_add(CONTEXT_LINES).min(total_lines);
            if start <= end {
                ranges.push(SnippetRange { start, end, reasons: vec!["diff hunk".to_owned()] });
            }
        }

        if let Some(symbols) = changed_symbols_by_file.get(&file.path) {
            for sym in symbols {
                let start = sym.start_line.max(1);
                let end = sym.end_line.max(start).min(total_lines);
                ranges.push(SnippetRange {
                    start,
                    end,
                    reasons: vec![format!("changed symbol: {}", sym.name)],
                });
            }
        }

        if let Some(symbols) = dependent_symbols_by_file.get(&file.path) {
            for sym in symbols {
                let start = sym.start_line.max(1);
                let end = sym.end_line.max(start).min(total_lines);
                ranges.push(SnippetRange {
                    start,
                    end,
                    reasons: vec![format!("dependent symbol: {}", sym.name)],
                });
            }
        }

        if is_test && !changed_symbol_names.is_empty() {
            for (idx, line) in lines.iter().enumerate() {
                let line_no = idx as u32 + 1;
                for name in &changed_symbol_names {
                    if line_contains_symbol_name(line, name) {
                        let start = line_no.saturating_sub(CONTEXT_LINES).max(1);
                        let end = line_no.saturating_add(CONTEXT_LINES).min(total_lines);
                        ranges.push(SnippetRange {
                            start,
                            end,
                            reasons: vec![format!("references changed symbol: {}", name)],
                        });
                    }
                }
            }
        }

        if ranges.is_empty() {
            if let Some(change) = change_by_path.get(&file.path) {
                if change.change_type == ChangeType::Deleted {
                    let end = total_lines.min(200);
                    ranges.push(SnippetRange {
                        start: 1,
                        end,
                        reasons: vec!["file removed".to_owned()],
                    });
                }
            }
        }

        let merged = merge_snippet_ranges(ranges);
        let mut snippets = Vec::new();
        let mut tokens = file
            .diff_content
            .as_deref()
            .map_or(0, |d| tokenizer.count(d, token_model));

        for range in merged {
            let start_idx = range.start.saturating_sub(1) as usize;
            let end_idx = range.end.saturating_sub(1) as usize;
            if start_idx >= lines.len() || end_idx >= lines.len() || start_idx > end_idx {
                continue;
            }
            let content = lines[start_idx..=end_idx].join("\n");
            tokens += tokenizer.count(&content, token_model);
            snippets.push(ContextSnippet {
                start_line: range.start,
                end_line: range.end,
                reason: range.reasons.join("; "),
                content,
            });
        }

        file.snippets = snippets;
        file.tokens = tokens;
    };

    for file in &mut context.changed_files {
        process_file(file, false);
    }

    for file in &mut context.dependent_files {
        process_file(file, false);
    }

    for file in &mut context.related_tests {
        process_file(file, true);
    }

    Ok(())
}

/// Apply token budget constraint to diff context
///
/// Prioritizes snippets by relevance score and keeps only those within budget.
/// Changed files are always included regardless of budget.
pub(crate) fn apply_diff_budget(
    context: &mut ExpandedContext,
    budget: u32,
    token_model: TokenModel,
) {
    use std::collections::HashSet;

    let tokenizer = Tokenizer::new();
    let mut running_tokens: u32 = context.changed_files.iter().map(|f| f.tokens).sum();

    if budget > 0 {
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        enum SnippetOwner {
            Dependent,
            Test,
        }

        struct SnippetCandidate {
            owner: SnippetOwner,
            file_index: usize,
            snippet_index: usize,
            tokens: u32,
            score: f32,
        }

        let snippet_score =
            |file: &infiniloom_engine::index::ContextFile, snippet: &ContextSnippet| -> f32 {
                let mut score = file.relevance_score;
                let reason = snippet.reason.as_str();
                if reason.contains("changed symbol") {
                    score += 0.3;
                } else if reason.contains("dependent symbol") {
                    score += 0.2;
                } else if reason.contains("diff hunk") {
                    score += 0.1;
                } else if reason.contains("file removed") {
                    score += 0.25;
                }
                score
            };

        let mut candidates: Vec<SnippetCandidate> = Vec::new();

        for (file_index, file) in context.dependent_files.iter().enumerate() {
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                let tokens = tokenizer.count(&snippet.content, token_model);
                candidates.push(SnippetCandidate {
                    owner: SnippetOwner::Dependent,
                    file_index,
                    snippet_index,
                    tokens,
                    score: snippet_score(file, snippet),
                });
            }
        }

        for (file_index, file) in context.related_tests.iter().enumerate() {
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                let tokens = tokenizer.count(&snippet.content, token_model);
                candidates.push(SnippetCandidate {
                    owner: SnippetOwner::Test,
                    file_index,
                    snippet_index,
                    tokens,
                    score: snippet_score(file, snippet),
                });
            }
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.tokens.cmp(&b.tokens))
        });

        let mut keep: HashSet<(SnippetOwner, usize, usize)> = HashSet::new();

        for candidate in candidates {
            if running_tokens.saturating_add(candidate.tokens) <= budget {
                running_tokens = running_tokens.saturating_add(candidate.tokens);
                keep.insert((candidate.owner, candidate.file_index, candidate.snippet_index));
            }
        }

        for (file_index, file) in context.dependent_files.iter_mut().enumerate() {
            let mut tokens: u32 = 0;
            let mut kept = Vec::new();
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                if keep.contains(&(SnippetOwner::Dependent, file_index, snippet_index)) {
                    tokens = tokens.saturating_add(tokenizer.count(&snippet.content, token_model));
                    kept.push(snippet.clone());
                }
            }
            file.snippets = kept;
            file.tokens = tokens;
        }

        for (file_index, file) in context.related_tests.iter_mut().enumerate() {
            let mut tokens: u32 = 0;
            let mut kept = Vec::new();
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                if keep.contains(&(SnippetOwner::Test, file_index, snippet_index)) {
                    tokens = tokens.saturating_add(tokenizer.count(&snippet.content, token_model));
                    kept.push(snippet.clone());
                }
            }
            file.snippets = kept;
            file.tokens = tokens;
        }

        context.dependent_files.retain(|f| !f.snippets.is_empty());
        context.related_tests.retain(|f| !f.snippets.is_empty());
    }

    let allowed_paths: HashSet<&str> = context
        .changed_files
        .iter()
        .map(|f| f.path.as_str())
        .chain(context.dependent_files.iter().map(|f| f.path.as_str()))
        .chain(context.related_tests.iter().map(|f| f.path.as_str()))
        .collect();

    context
        .dependent_symbols
        .retain(|sym| allowed_paths.contains(sym.file_path.as_str()));

    context.total_tokens = context
        .changed_files
        .iter()
        .chain(context.dependent_files.iter())
        .chain(context.related_tests.iter())
        .map(|f| f.tokens)
        .sum();
}
