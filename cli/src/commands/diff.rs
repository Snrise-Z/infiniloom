//! Diff command handler
//!
//! Gets context for git diffs including changed files, dependents, and tests.

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use infiniloom_engine::git::GitRepo;
use infiniloom_engine::index::{
    BuildOptions, ChangeType, ContextDepth, ContextExpander, ContextSnippet, DiffChange,
    IndexBuilder, IndexStorage, LazyContextBuilder,
};
use infiniloom_engine::output::OutputFormat;
use infiniloom_engine::tokenizer::{TokenModel, Tokenizer};
use infiniloom_engine::types::TokenizerModel;

use crate::config::load_config_file;

/// Type alias for file history map (file path -> list of commits)
type FileHistory = HashMap<String, Vec<infiniloom_engine::git::Commit>>;

/// Get context for a diff
#[allow(clippy::too_many_arguments)]
pub fn cmd_diff(
    mut path: PathBuf,
    mut reference: Option<String>,
    staged: bool,
    depth: u8,
    budget: u32,
    format: OutputFormat,
    output: Option<PathBuf>,
    include_diff: bool,
    cli_model: Option<TokenizerModel>,
    include_history: bool,
    history_count: usize,
    verbose: bool,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
) -> Result<()> {
    // Check git is available
    check_git_available()?;

    if reference.is_none() && !path.exists() {
        reference = Some(path.to_string_lossy().to_string());
        path = PathBuf::from(".");
    }

    let storage = IndexStorage::new(&path);
    let loaded_config = load_config_file(None, &path);

    let model: TokenizerModel = if let Some(m) = cli_model {
        m
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let token_model = to_token_model(model);
    let base_ref = resolve_base_ref(reference.as_deref(), &path);

    // Always load diff content for accurate change classification
    let mut changes = get_diff_changes(&path, reference.as_deref(), staged, true)?;

    // Apply exclude patterns
    if !exclude.is_empty() {
        changes.retain(|c| {
            !exclude.iter().any(|pattern| {
                c.file_path.contains(pattern)
                    || c.file_path.starts_with(pattern)
                    || c.file_path.split('/').any(|part| part == pattern)
            })
        });
    }

    // Apply include patterns (only keep matching files)
    if !include_patterns.is_empty() {
        changes.retain(|c| {
            include_patterns.iter().any(|pattern| {
                if pattern.contains('*') {
                    glob::Pattern::new(pattern).is_ok_and(|p| p.matches(&c.file_path))
                } else {
                    c.file_path.contains(pattern) || c.file_path.ends_with(pattern)
                }
            })
        });
    }

    // Exclude test files unless include_tests is true
    if !include_tests {
        use infiniloom_engine::default_ignores::{matches_any, TEST_IGNORES};
        changes.retain(|c| !matches_any(&c.file_path, TEST_IGNORES));
    }

    if changes.is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    if verbose {
        eprintln!("{} Analyzing {} changed files...", "→".cyan(), changes.len());
    }

    // Convert depth
    let context_depth = match depth {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expand_budget = u32::MAX / 2;

    // Try to use pre-built index, fall back to in-memory rebuild or lazy indexing
    let mut context = if storage.exists() {
        let mut use_prebuilt = true;
        if let Ok(meta) = storage.load_meta() {
            if !is_index_fresh(&path, &meta) {
                use_prebuilt = false;
            }
        }

        if use_prebuilt {
            let (index, graph) = storage.load_all().context("Failed to load index")?;
            let expander = ContextExpander::new(&index, &graph);
            expander.expand(&changes, context_depth, expand_budget)
        } else {
            eprintln!(
                "{} Index is stale; rebuilding in memory for accurate context...",
                "→".yellow()
            );
            let builder = IndexBuilder::new(&path)
                .with_options(BuildOptions { respect_gitignore: true, ..Default::default() });
            match builder.build() {
                Ok((index, graph)) => {
                    let expander = ContextExpander::new(&index, &graph);
                    expander.expand(&changes, context_depth, expand_budget)
                },
                Err(e) => {
                    eprintln!(
                        "{} Index rebuild failed ({}). Falling back to lazy indexing...",
                        "⚠".yellow(),
                        e
                    );
                    let mut builder = LazyContextBuilder::new(&path);
                    builder
                        .generate_context(&changes, context_depth, expand_budget)
                        .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
                },
            }
        }
    } else {
        // Lazy path: build minimal index on-the-fly
        eprintln!("{} No pre-built index found, using lazy indexing...", "→".yellow());
        let mut builder = LazyContextBuilder::new(&path);
        builder
            .generate_context(&changes, context_depth, expand_budget)
            .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
    };

    if !include_diff {
        for file in &mut context.changed_files {
            file.diff_content = None;
        }
    }

    enrich_diff_context(&path, &changes, base_ref.as_deref(), &mut context, token_model)?;
    apply_diff_budget(&mut context, budget, token_model);

    // Fetch commit history for changed files if requested
    let file_history: FileHistory = if include_history && history_count > 0 {
        let mut history_map = HashMap::new();
        if let Ok(repo) = GitRepo::open(&path) {
            for file in &context.changed_files {
                if let Ok(commits) = repo.file_log(&file.path, history_count) {
                    if !commits.is_empty() {
                        history_map.insert(file.path.clone(), commits);
                    }
                }
            }
        }
        history_map
    } else {
        HashMap::new()
    };

    // Format output
    let output_text = format_diff_context(&context, format, &file_history);

    // Write output
    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        println!("Context written to {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    // Print summary
    eprintln!();
    eprintln!(
        "{} Impact: {} ({} files, {} symbols, {} tests)",
        "→".cyan(),
        context.impact_summary.level.name(),
        context.impact_summary.direct_files + context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    );
    eprintln!("  Tokens: ~{}", context.total_tokens);

    Ok(())
}

/// Convert TokenizerModel to the tokenizer's TokenModel
fn to_token_model(model: TokenizerModel) -> TokenModel {
    match model {
        // OpenAI o200k_base models
        TokenizerModel::Gpt52 => TokenModel::Gpt52,
        TokenizerModel::Gpt52Pro => TokenModel::Gpt52Pro,
        TokenizerModel::Gpt51 => TokenModel::Gpt51,
        TokenizerModel::Gpt51Mini => TokenModel::Gpt51Mini,
        TokenizerModel::Gpt51Codex => TokenModel::Gpt51Codex,
        TokenizerModel::Gpt5 => TokenModel::Gpt5,
        TokenizerModel::Gpt5Mini => TokenModel::Gpt5Mini,
        TokenizerModel::Gpt5Nano => TokenModel::Gpt5Nano,
        TokenizerModel::O4Mini => TokenModel::O4Mini,
        TokenizerModel::O3 => TokenModel::O3,
        TokenizerModel::O3Mini => TokenModel::O3Mini,
        TokenizerModel::O1 => TokenModel::O1,
        TokenizerModel::O1Mini => TokenModel::O1Mini,
        TokenizerModel::O1Preview => TokenModel::O1Preview,
        TokenizerModel::Gpt4o => TokenModel::Gpt4o,
        TokenizerModel::Gpt4oMini => TokenModel::Gpt4oMini,
        // OpenAI cl100k_base models (legacy)
        TokenizerModel::Gpt4 => TokenModel::Gpt4,
        TokenizerModel::Gpt35Turbo => TokenModel::Gpt35Turbo,
        // Other vendors
        TokenizerModel::Claude => TokenModel::Claude,
        TokenizerModel::Gemini => TokenModel::Gemini,
        TokenizerModel::Llama => TokenModel::Llama,
        TokenizerModel::CodeLlama => TokenModel::CodeLlama,
        TokenizerModel::Mistral => TokenModel::Mistral,
        TokenizerModel::DeepSeek => TokenModel::DeepSeek,
        TokenizerModel::Qwen => TokenModel::Qwen,
        TokenizerModel::Cohere => TokenModel::Cohere,
        TokenizerModel::Grok => TokenModel::Grok,
    }
}

/// Check if git is available on the system
fn check_git_available() -> Result<()> {
    use std::process::Command;

    let output = Command::new("git")
        .arg("--version")
        .output()
        .context("Git is not installed or not found in PATH. Please install git and ensure it's available in your PATH.")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git is installed but returned an error. Please check your git installation."
        );
    }

    Ok(())
}

/// Get diff changes from git
fn get_diff_changes(
    repo_path: &PathBuf,
    reference: Option<&str>,
    staged: bool,
    include_diff_content: bool,
) -> Result<Vec<DiffChange>> {
    use std::process::Command;

    let mut changes = Vec::new();

    // Build git diff command
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--name-status");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    let output = cmd.output().context("Failed to run git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let status = parts[0];

            // For renames (R###), git outputs: R###\told_path\tnew_path
            let (file_path, old_path) = if status.starts_with('R') && parts.len() >= 3 {
                // Rename: parts[1] is old path, parts[2] is new path
                (parts[2].to_owned(), Some(parts[1].to_owned()))
            } else {
                (parts[1].to_owned(), None)
            };

            let change_type = match status.chars().next() {
                Some('A') => ChangeType::Added,
                Some('M') => ChangeType::Modified,
                Some('D') => ChangeType::Deleted,
                Some('R') => ChangeType::Renamed,
                _ => ChangeType::Modified,
            };

            // For modified files, get the actual changed lines
            // For renames, use the new path
            let line_ranges = match change_type {
                ChangeType::Modified => {
                    get_changed_lines(repo_path, &file_path, reference, staged)?
                },
                ChangeType::Added | ChangeType::Renamed => {
                    // Get actual line count to avoid iterating 4+ billion lines
                    let full_path = repo_path.join(&file_path);
                    let line_count = std::fs::read_to_string(&full_path)
                        .map(|content| content.lines().count() as u32)
                        .unwrap_or(1)
                        .max(1);
                    vec![(1, line_count)]
                },
                ChangeType::Deleted => {
                    // Deleted files have no lines to iterate - use empty range
                    vec![]
                },
            };

            // Optionally get the raw diff content
            // For renames, we need to check both old and new paths for diff content
            let diff_content = if include_diff_content {
                get_diff_content(repo_path, &file_path, reference, staged).ok()
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path,
                line_ranges,
                change_type,
                diff_content,
            });
        }
    }

    // Also include untracked files when looking at working tree changes
    // (not staged and no reference specified)
    if !staged && reference.is_none() {
        let untracked = get_untracked_files(repo_path)?;
        for file_path in untracked {
            // Get line count for untracked files
            let full_path = repo_path.join(&file_path);
            let line_count = std::fs::read_to_string(&full_path)
                .map(|content| content.lines().count() as u32)
                .unwrap_or(1)
                .max(1);

            // Read file content if requested
            let diff_content = if include_diff_content {
                std::fs::read_to_string(&full_path).ok().map(|content| {
                    format!(
                        "@@ -0,0 +1,{} @@\n{}",
                        line_count,
                        content
                            .lines()
                            .map(|l| format!("+{}", l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path: None,
                line_ranges: vec![(1, line_count)],
                change_type: ChangeType::Added,
                diff_content,
            });
        }
    }

    Ok(changes)
}

/// Get untracked files from git status
fn get_untracked_files(repo_path: &PathBuf) -> Result<Vec<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        return Ok(vec![]); // Silently ignore errors
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let untracked: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Untracked files start with "?? "
            line.strip_prefix("?? ").map(|stripped| stripped.to_owned())
        })
        .collect();

    Ok(untracked)
}

/// Get raw diff content for a file (the actual +/- lines)
fn get_diff_content(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<String> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout.to_string())
}

/// Get changed line ranges for a file
fn get_changed_lines(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<Vec<(u32, u32)>> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--unified=0");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;

    let mut ranges = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse @@ -start,count +start,count @@ lines
    for line in stdout.lines() {
        if line.starts_with("@@") {
            // Extract the new file range
            if let Some(plus_idx) = line.find('+') {
                let rest = &line[plus_idx + 1..];
                if let Some(space_idx) = rest.find(' ') {
                    let range_str = &rest[..space_idx];
                    let parts: Vec<&str> = range_str.split(',').collect();
                    if !parts.is_empty() {
                        let start: u32 = parts[0].parse().unwrap_or(1);
                        let count: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                        ranges.push((start, start + count.saturating_sub(1)));
                    }
                }
            }
        }
    }

    if ranges.is_empty() {
        // Fallback: get actual line count from file instead of arbitrary 100
        let full_path = repo_path.join(file_path);
        let line_count = std::fs::read_to_string(&full_path)
            .map(|content| content.lines().count() as u32)
            .unwrap_or(1)
            .max(1); // Ensure at least 1 line
        ranges.push((1, line_count));
    }

    Ok(ranges)
}

/// Resolve the base git reference from a reference string
/// For "main..feature" returns "main", for "HEAD~1" returns "HEAD~1"
fn resolve_base_ref(reference: Option<&str>, repo_path: &Path) -> Option<String> {
    let ref_str = reference.unwrap_or("HEAD");

    // Handle range format: "base..head" or "base...head"
    if let Some(base) = ref_str.split("..").next() {
        if !base.is_empty() && base != ref_str {
            return Some(base.to_owned());
        }
    }

    // For single refs like "HEAD~1", "main", etc., use as-is
    // But verify it's a valid ref first
    use std::process::Command;
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "--verify", ref_str])
        .output()
        .ok()?;

    if output.status.success() {
        Some(ref_str.to_owned())
    } else {
        None
    }
}

/// Read file content from a git reference
/// Uses `git show ref:path` to retrieve file content from a specific commit/ref
fn read_file_from_git(repo_path: &Path, git_ref: &str, file_path: &str) -> Option<String> {
    use std::process::Command;

    // Git show format: ref:path
    let ref_path = format!("{}:{}", git_ref, file_path);

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["show", &ref_path])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn is_index_fresh(repo_path: &Path, meta: &infiniloom_engine::index::IndexMeta) -> bool {
    let repo = match GitRepo::open(repo_path) {
        Ok(repo) => repo,
        Err(_) => return true,
    };

    let status = match repo.status() {
        Ok(status) => status,
        Err(_) => return false,
    };
    if !status.is_empty() {
        return false;
    }

    if let Ok(head) = repo.current_commit() {
        if let Some(ref index_commit) = meta.commit_hash {
            if index_commit != &head {
                return false;
            }
        }
    }

    true
}

fn diff_preamble(context: &infiniloom_engine::index::ExpandedContext) -> String {
    format!(
        "Use this diff context to understand changes. Start with changed file snippets, then dependent symbols/files/tests. Impact: {}.",
        context.impact_summary.level.name()
    )
}

/// Format diff context for output
fn format_diff_context(
    context: &infiniloom_engine::index::ExpandedContext,
    format: OutputFormat,
    history: &FileHistory,
) -> String {
    match format {
        OutputFormat::Xml => format_diff_context_xml(context, history),
        OutputFormat::Json => format_diff_context_json(context, history),
        OutputFormat::Markdown => format_diff_context_markdown(context, history),
        OutputFormat::Yaml => format_diff_context_yaml(context, history),
        OutputFormat::Toon => format_diff_context_toon(context, history),
        OutputFormat::Plain => format_diff_context_plain(context, history),
    }
}

fn format_diff_context_json(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "preamble": diff_preamble(context),
        "changed_files": context.changed_files.iter().map(|f| {
            let file_history = history.get(&f.path).map(|commits| {
                commits.iter().map(|c| serde_json::json!({
                    "hash": &c.short_hash,
                    "author": &c.author,
                    "date": &c.date,
                    "message": &c.message,
                })).collect::<Vec<_>>()
            });
            serde_json::json!({
                "path": &f.path,
                "language": &f.language,
                "tokens": f.tokens,
                "diff_content": &f.diff_content,
                "history": file_history,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "dependent_files": context.dependent_files.iter().map(|f| {
            serde_json::json!({
                "path": &f.path,
                "reason": &f.relevance_reason,
                "relevance": f.relevance_score,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "changed_symbols": context.changed_symbols.iter().map(|s| {
            serde_json::json!({
                "name": &s.name,
                "kind": &s.kind,
                "file": &s.file_path,
                "line": s.start_line,
            })
        }).collect::<Vec<_>>(),
        "dependent_symbols": context.dependent_symbols.iter().map(|s| {
            serde_json::json!({
                "name": &s.name,
                "kind": &s.kind,
                "file": &s.file_path,
                "line": s.start_line,
                "reason": &s.relevance_reason,
                "relevance": s.relevance_score,
            })
        }).collect::<Vec<_>>(),
        "related_tests": context.related_tests.iter().map(|f| {
            serde_json::json!({
                "path": &f.path,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "call_chains": context.call_chains.iter().map(|c| c.symbols.join(" → ")).collect::<Vec<_>>(),
        "impact": {
            "level": context.impact_summary.level.name(),
            "description": &context.impact_summary.description,
            "direct_files": context.impact_summary.direct_files,
            "transitive_files": context.impact_summary.transitive_files,
            "affected_symbols": context.impact_summary.affected_symbols,
            "affected_tests": context.impact_summary.affected_tests,
        },
        "tokens": context.total_tokens,
    }))
    .unwrap_or_else(|_| "{}".to_owned())
}

fn format_diff_context_markdown(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut md = String::new();

    // Header
    md.push_str("# Diff Context\n\n");
    md.push_str(&format!("> {}\n\n", diff_preamble(context)));

    // Impact summary
    md.push_str("## Impact Summary\n\n");
    md.push_str(&format!("**Level:** {}\n\n", context.impact_summary.level.name()));
    md.push_str(&format!("{}\n\n", context.impact_summary.description));
    md.push_str(&format!(
        "- Direct files: {}\n- Transitive files: {}\n- Affected symbols: {}\n- Affected tests: {}\n- Total tokens: {}\n\n",
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests,
        context.total_tokens
    ));

    // Changed files
    md.push_str("## Changed Files\n\n");
    for file in &context.changed_files {
        md.push_str(&format!("### `{}`\n\n", file.path));
        md.push_str(&format!("- Language: {}\n- Tokens: {}\n\n", file.language, file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                md.push_str("**Recent History**\n\n");
                md.push_str("| Commit | Author | Date | Message |\n");
                md.push_str("|--------|--------|------|--------|\n");
                for commit in commits {
                    md.push_str(&format!(
                        "| `{}` | {} | {} | {} |\n",
                        commit.short_hash, commit.author, commit.date, commit.message
                    ));
                }
                md.push('\n');
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            md.push_str("```diff\n");
            md.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                md.push('\n');
            }
            md.push_str("```\n\n");
        }
        if !file.snippets.is_empty() {
            md.push_str("**Snippets**\n\n");
            for snippet in &file.snippets {
                md.push_str(&format!(
                    "- {} (lines {}-{})\n\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                md.push_str("```text\n");
                md.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
        }
    }

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        md.push_str("## Changed Symbols\n\n");
        md.push_str("| Symbol | Kind | File | Line |\n");
        md.push_str("|--------|------|------|------|\n");
        for sym in &context.changed_symbols {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
        md.push('\n');
    }

    // Dependent symbols
    if !context.dependent_symbols.is_empty() {
        md.push_str("## Dependent Symbols\n\n");
        md.push_str("| Symbol | Kind | File | Line | Relevance |\n");
        md.push_str("|--------|------|------|------|-----------|\n");
        for sym in &context.dependent_symbols {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {:.2} |\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
        md.push('\n');
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        md.push_str("## Dependent Files\n\n");
        md.push_str("| File | Reason | Relevance |\n");
        md.push_str("|------|--------|----------|\n");
        for file in &context.dependent_files {
            md.push_str(&format!(
                "| `{}` | {} | {:.2} |\n",
                file.path, file.relevance_reason, file.relevance_score
            ));
        }
        md.push('\n');

        for file in &context.dependent_files {
            if !file.snippets.is_empty() {
                md.push_str(&format!("### `{}` Snippets\n\n", file.path));
                for snippet in &file.snippets {
                    md.push_str(&format!(
                        "- {} (lines {}-{})\n\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    md.push_str("```text\n");
                    md.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        md.push('\n');
                    }
                    md.push_str("```\n\n");
                }
            }
        }
    }

    // Related tests
    if !context.related_tests.is_empty() {
        md.push_str("## Related Tests\n\n");
        for test in &context.related_tests {
            md.push_str(&format!("### `{}`\n\n", test.path));
            if test.snippets.is_empty() {
                md.push_str("- No focused snippets selected\n\n");
                continue;
            }
            for snippet in &test.snippets {
                md.push_str(&format!(
                    "- {} (lines {}-{})\n\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                md.push_str("```text\n");
                md.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
        }
    }

    // Call chains
    if !context.call_chains.is_empty() {
        md.push_str("## Call Graph\n\n");
        for chain in &context.call_chains {
            md.push_str(&format!("- {}\n", chain.symbols.join(" → ")));
        }
        md.push('\n');
    }

    md
}

fn format_diff_context_yaml(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut yaml = String::new();

    yaml.push_str("# Diff Context\n\n");
    yaml.push_str("preamble: |\n");
    for line in diff_preamble(context).lines() {
        yaml.push_str(&format!("  {}\n", line));
    }
    yaml.push('\n');

    // Impact
    yaml.push_str("impact:\n");
    yaml.push_str(&format!("  level: {}\n", context.impact_summary.level.name()));
    yaml.push_str(&format!(
        "  description: \"{}\"\n",
        context.impact_summary.description.replace('"', "\\\"")
    ));
    yaml.push_str(&format!("  direct_files: {}\n", context.impact_summary.direct_files));
    yaml.push_str(&format!("  transitive_files: {}\n", context.impact_summary.transitive_files));
    yaml.push_str(&format!("  affected_symbols: {}\n", context.impact_summary.affected_symbols));
    yaml.push_str(&format!("  affected_tests: {}\n", context.impact_summary.affected_tests));
    yaml.push_str(&format!("total_tokens: {}\n\n", context.total_tokens));

    // Changed files
    yaml.push_str("changed_files:\n");
    for file in &context.changed_files {
        yaml.push_str(&format!("  - path: \"{}\"\n", file.path));
        yaml.push_str(&format!("    language: {}\n", file.language));
        yaml.push_str(&format!("    tokens: {}\n", file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                yaml.push_str("    history:\n");
                for commit in commits {
                    yaml.push_str(&format!("      - hash: \"{}\"\n", commit.short_hash));
                    yaml.push_str(&format!("        author: \"{}\"\n", commit.author));
                    yaml.push_str(&format!("        date: \"{}\"\n", commit.date));
                    yaml.push_str(&format!(
                        "        message: \"{}\"\n",
                        commit.message.replace('"', "\\\"")
                    ));
                }
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            yaml.push_str("    diff: |\n");
            for line in diff_content.lines() {
                yaml.push_str(&format!("      {}\n", line));
            }
        }
        if !file.snippets.is_empty() {
            yaml.push_str("    snippets:\n");
            for snippet in &file.snippets {
                yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                yaml.push_str(&format!(
                    "        reason: \"{}\"\n",
                    snippet.reason.replace('"', "\\\"")
                ));
                yaml.push_str("        content: |\n");
                for line in snippet.content.lines() {
                    yaml.push_str(&format!("          {}\n", line));
                }
            }
        }
    }

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        yaml.push_str("\nchanged_symbols:\n");
        for sym in &context.changed_symbols {
            yaml.push_str(&format!("  - name: \"{}\"\n", sym.name));
            yaml.push_str(&format!("    kind: {}\n", sym.kind));
            yaml.push_str(&format!("    file: \"{}\"\n", sym.file_path));
            yaml.push_str(&format!("    line: {}\n", sym.start_line));
        }
    }

    if !context.dependent_symbols.is_empty() {
        yaml.push_str("\ndependent_symbols:\n");
        for sym in &context.dependent_symbols {
            yaml.push_str(&format!("  - name: \"{}\"\n", sym.name));
            yaml.push_str(&format!("    kind: {}\n", sym.kind));
            yaml.push_str(&format!("    file: \"{}\"\n", sym.file_path));
            yaml.push_str(&format!("    line: {}\n", sym.start_line));
            yaml.push_str(&format!(
                "    reason: \"{}\"\n",
                sym.relevance_reason.replace('"', "\\\"")
            ));
            yaml.push_str(&format!("    relevance: {:.2}\n", sym.relevance_score));
        }
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        yaml.push_str("\ndependent_files:\n");
        for file in &context.dependent_files {
            yaml.push_str(&format!("  - path: \"{}\"\n", file.path));
            yaml.push_str(&format!("    reason: \"{}\"\n", file.relevance_reason));
            yaml.push_str(&format!("    relevance: {:.2}\n", file.relevance_score));
            if !file.snippets.is_empty() {
                yaml.push_str("    snippets:\n");
                for snippet in &file.snippets {
                    yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                    yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                    yaml.push_str(&format!(
                        "        reason: \"{}\"\n",
                        snippet.reason.replace('"', "\\\"")
                    ));
                    yaml.push_str("        content: |\n");
                    for line in snippet.content.lines() {
                        yaml.push_str(&format!("          {}\n", line));
                    }
                }
            }
        }
    }

    // Related tests
    if !context.related_tests.is_empty() {
        yaml.push_str("\nrelated_tests:\n");
        for test in &context.related_tests {
            yaml.push_str(&format!("  - path: \"{}\"\n", test.path));
            if !test.snippets.is_empty() {
                yaml.push_str("    snippets:\n");
                for snippet in &test.snippets {
                    yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                    yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                    yaml.push_str(&format!(
                        "        reason: \"{}\"\n",
                        snippet.reason.replace('"', "\\\"")
                    ));
                    yaml.push_str("        content: |\n");
                    for line in snippet.content.lines() {
                        yaml.push_str(&format!("          {}\n", line));
                    }
                }
            }
        }
    }

    // Call chains
    if !context.call_chains.is_empty() {
        yaml.push_str("\ncall_chains:\n");
        for chain in &context.call_chains {
            yaml.push_str(&format!("  - \"{}\"\n", chain.symbols.join(" → ")));
        }
    }

    yaml
}

fn format_diff_context_toon(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    // TOON = Token-Optimized Output Notation - minimal delimiters
    let mut toon = String::new();

    // Header
    toon.push_str(&format!(
        "DIFF|{}|d{}t{}s{}T{}\n",
        context.impact_summary.level.name(),
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.total_tokens
    ));
    toon.push_str(&format!("PRE|{}\n", diff_preamble(context)));

    // Changed files
    toon.push_str("FILES:\n");
    for file in &context.changed_files {
        toon.push_str(&format!("F|{}|{}|{}\n", file.path, file.language, file.tokens));

        // Include file history if available (compact format)
        if let Some(commits) = history.get(&file.path) {
            for commit in commits {
                toon.push_str(&format!(
                    "H|{}|{}|{}|{}\n",
                    commit.short_hash, commit.author, commit.date, commit.message
                ));
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            toon.push_str("D{\n");
            toon.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                toon.push('\n');
            }
            toon.push_str("}D\n");
        }
        for snippet in &file.snippets {
            toon.push_str(&format!(
                "N|{}|{}|{}\n",
                snippet.start_line, snippet.end_line, snippet.reason
            ));
            toon.push_str("C{\n");
            toon.push_str(&snippet.content);
            if !snippet.content.ends_with('\n') {
                toon.push('\n');
            }
            toon.push_str("}C\n");
        }
    }

    // Symbols
    if !context.changed_symbols.is_empty() {
        toon.push_str("SYMS:\n");
        for sym in &context.changed_symbols {
            toon.push_str(&format!(
                "S|{}|{}|{}|{}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
    }

    if !context.dependent_symbols.is_empty() {
        toon.push_str("DEPSYMS:\n");
        for sym in &context.dependent_symbols {
            toon.push_str(&format!(
                "S|{}|{}|{}|{}|{:.1}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
    }

    // Dependents
    if !context.dependent_files.is_empty() {
        toon.push_str("DEPS:\n");
        for file in &context.dependent_files {
            toon.push_str(&format!("P|{}|{:.1}\n", file.path, file.relevance_score));
            for snippet in &file.snippets {
                toon.push_str(&format!(
                    "N|{}|{}|{}\n",
                    snippet.start_line, snippet.end_line, snippet.reason
                ));
                toon.push_str("C{\n");
                toon.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    toon.push('\n');
                }
                toon.push_str("}C\n");
            }
        }
    }

    // Tests
    if !context.related_tests.is_empty() {
        toon.push_str("TESTS:\n");
        for test in &context.related_tests {
            toon.push_str(&format!("T|{}\n", test.path));
            for snippet in &test.snippets {
                toon.push_str(&format!(
                    "N|{}|{}|{}\n",
                    snippet.start_line, snippet.end_line, snippet.reason
                ));
                toon.push_str("C{\n");
                toon.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    toon.push('\n');
                }
                toon.push_str("}C\n");
            }
        }
    }

    toon
}

fn format_diff_context_plain(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut plain = String::new();

    // Header
    plain.push_str("=== DIFF CONTEXT ===\n\n");
    plain.push_str(&format!("{}\n\n", diff_preamble(context)));
    plain.push_str(&format!(
        "Impact: {} - {}\n",
        context.impact_summary.level.name(),
        context.impact_summary.description
    ));
    plain.push_str(&format!(
        "Stats: {} direct files, {} transitive, {} symbols, {} tests\n",
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    ));
    plain.push_str(&format!("Total tokens: {}\n\n", context.total_tokens));

    // Changed files
    plain.push_str("--- CHANGED FILES ---\n");
    for file in &context.changed_files {
        plain.push_str(&format!("\n{} ({}, {} tokens)\n", file.path, file.language, file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                plain.push_str("Recent history:\n");
                for commit in commits {
                    plain.push_str(&format!(
                        "  {} ({}, {}) {}\n",
                        commit.short_hash, commit.author, commit.date, commit.message
                    ));
                }
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            plain.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                plain.push('\n');
            }
        }
        if !file.snippets.is_empty() {
            plain.push_str("Snippets:\n");
            for snippet in &file.snippets {
                plain.push_str(&format!(
                    "- {} (lines {}-{})\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                plain.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    plain.push('\n');
                }
            }
        }
    }

    // Symbols
    if !context.changed_symbols.is_empty() {
        plain.push_str("\n--- CHANGED SYMBOLS ---\n");
        for sym in &context.changed_symbols {
            plain.push_str(&format!(
                "{} ({}) in {} line {}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
    }

    if !context.dependent_symbols.is_empty() {
        plain.push_str("\n--- DEPENDENT SYMBOLS ---\n");
        for sym in &context.dependent_symbols {
            plain.push_str(&format!(
                "{} ({}) in {} line {} (relevance: {:.2})\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
    }

    // Dependents
    if !context.dependent_files.is_empty() {
        plain.push_str("\n--- DEPENDENT FILES ---\n");
        for file in &context.dependent_files {
            plain.push_str(&format!(
                "{} - {} (relevance: {:.2})\n",
                file.path, file.relevance_reason, file.relevance_score
            ));
            if !file.snippets.is_empty() {
                plain.push_str("Snippets:\n");
                for snippet in &file.snippets {
                    plain.push_str(&format!(
                        "- {} (lines {}-{})\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    plain.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        plain.push('\n');
                    }
                }
            }
        }
    }

    // Tests
    if !context.related_tests.is_empty() {
        plain.push_str("\n--- RELATED TESTS ---\n");
        for test in &context.related_tests {
            plain.push_str(&format!("{}\n", test.path));
            if !test.snippets.is_empty() {
                plain.push_str("Snippets:\n");
                for snippet in &test.snippets {
                    plain.push_str(&format!(
                        "- {} (lines {}-{})\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    plain.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        plain.push('\n');
                    }
                }
            }
        }
    }

    plain
}

fn format_diff_context_xml(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<diff_context>\n");

    // Summary
    xml.push_str("  <summary>\n");
    xml.push_str(&format!(
        "    <preamble>{}</preamble>\n",
        escape_xml_text(&diff_preamble(context))
    ));
    xml.push_str(&format!(
        "    <impact level=\"{}\">{}</impact>\n",
        context.impact_summary.level.name(),
        context.impact_summary.description
    ));
    xml.push_str(&format!(
        "    <stats files=\"{}\" symbols=\"{}\" tests=\"{}\"/>\n",
        context.impact_summary.direct_files + context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    ));
    xml.push_str("  </summary>\n");

    // Changed files
    xml.push_str("  <changed_files>\n");
    for file in &context.changed_files {
        let has_snippets = !file.snippets.is_empty();
        let has_history = history.get(&file.path).is_some_and(|h| !h.is_empty());
        let needs_full_element = file.diff_content.is_some() || has_snippets || has_history;

        if needs_full_element {
            xml.push_str(&format!(
                "    <file path=\"{}\" language=\"{}\" tokens=\"{}\">\n",
                file.path, file.language, file.tokens
            ));

            // Include file history if available
            if let Some(commits) = history.get(&file.path) {
                if !commits.is_empty() {
                    xml.push_str("      <history>\n");
                    for commit in commits {
                        xml.push_str(&format!(
                            "        <commit hash=\"{}\" date=\"{}\" author=\"{}\">\n          {}\n        </commit>\n",
                            escape_xml_attr(&commit.short_hash),
                            escape_xml_attr(&commit.date),
                            escape_xml_attr(&commit.author),
                            escape_xml_text(&commit.message)
                        ));
                    }
                    xml.push_str("      </history>\n");
                }
            }

            if let Some(ref diff_content) = file.diff_content {
                xml.push_str("      <diff>\n<![CDATA[\n");
                xml.push_str(diff_content);
                xml.push_str("]]>\n      </diff>\n");
            }
            if has_snippets {
                xml.push_str("      <snippets>\n");
                for snippet in &file.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
            }
            xml.push_str("    </file>\n");
        } else {
            xml.push_str(&format!(
                "    <file path=\"{}\" language=\"{}\" tokens=\"{}\"/>\n",
                file.path, file.language, file.tokens
            ));
        }
    }
    xml.push_str("  </changed_files>\n");

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        xml.push_str("  <changed_symbols>\n");
        for sym in &context.changed_symbols {
            xml.push_str(&format!(
                "    <symbol name=\"{}\" kind=\"{}\" file=\"{}\" line=\"{}\"/>\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
        xml.push_str("  </changed_symbols>\n");
    }

    if !context.dependent_symbols.is_empty() {
        xml.push_str("  <dependent_symbols>\n");
        for sym in &context.dependent_symbols {
            let reason = sym.relevance_reason.replace('"', "&quot;");
            xml.push_str(&format!(
                "    <symbol name=\"{}\" kind=\"{}\" file=\"{}\" line=\"{}\" relevance=\"{:.2}\" reason=\"{}\"/>\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score, reason
            ));
        }
        xml.push_str("  </dependent_symbols>\n");
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        xml.push_str("  <dependent_files>\n");
        for file in &context.dependent_files {
            if file.snippets.is_empty() {
                xml.push_str(&format!(
                    "    <file path=\"{}\" reason=\"{}\" relevance=\"{:.2}\"/>\n",
                    file.path, file.relevance_reason, file.relevance_score
                ));
            } else {
                xml.push_str(&format!(
                    "    <file path=\"{}\" reason=\"{}\" relevance=\"{:.2}\">\n",
                    file.path, file.relevance_reason, file.relevance_score
                ));
                xml.push_str("      <snippets>\n");
                for snippet in &file.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
                xml.push_str("    </file>\n");
            }
        }
        xml.push_str("  </dependent_files>\n");
    }

    // Related tests
    if !context.related_tests.is_empty() {
        xml.push_str("  <related_tests>\n");
        for test in &context.related_tests {
            if test.snippets.is_empty() {
                xml.push_str(&format!("    <test path=\"{}\"/>\n", test.path));
            } else {
                xml.push_str(&format!("    <test path=\"{}\">\n", test.path));
                xml.push_str("      <snippets>\n");
                for snippet in &test.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
                xml.push_str("    </test>\n");
            }
        }
        xml.push_str("  </related_tests>\n");
    }

    // Call chains
    if !context.call_chains.is_empty() {
        xml.push_str("  <call_graph>\n");
        for chain in &context.call_chains {
            xml.push_str(&format!("    <chain>{}</chain>\n", chain.symbols.join(" → ")));
        }
        xml.push_str("  </call_graph>\n");
    }

    xml.push_str("</diff_context>\n");
    xml
}

// Helper structs and functions for snippet processing

#[derive(Clone)]
struct SnippetRange {
    start: u32,
    end: u32,
    reasons: Vec<String>,
}

fn merge_snippet_ranges(mut ranges: Vec<SnippetRange>) -> Vec<SnippetRange> {
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SnippetRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                for reason in range.reasons {
                    if !last.reasons.contains(&reason) {
                        last.reasons.push(reason);
                    }
                }
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    merged
}

fn line_contains_symbol_name(line: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut offset = 0;
    while let Some(pos) = line[offset..].find(name) {
        let start = offset + pos;
        let end = start + name.len();

        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();

        let before_ok = before.map(|c| !is_word_char(c)).unwrap_or(true);
        let after_ok = after.map(|c| !is_word_char(c)).unwrap_or(true);

        if before_ok && after_ok {
            return true;
        }

        offset = end;
    }

    false
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn enrich_diff_context(
    repo_path: &Path,
    changes: &[DiffChange],
    base_ref: Option<&str>,
    context: &mut infiniloom_engine::index::ExpandedContext,
    token_model: TokenModel,
) -> Result<()> {
    const CONTEXT_LINES: u32 = 3;
    let tokenizer = Tokenizer::new();
    let mut change_by_path: HashMap<String, &DiffChange> = HashMap::new();
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
                            .map(|d| tokenizer.count(d, token_model))
                            .unwrap_or(0);
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
            .map(|d| tokenizer.count(d, token_model))
            .unwrap_or(0);

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

    for file in context.changed_files.iter_mut() {
        process_file(file, false);
    }

    for file in context.dependent_files.iter_mut() {
        process_file(file, false);
    }

    for file in context.related_tests.iter_mut() {
        process_file(file, true);
    }

    Ok(())
}

fn apply_diff_budget(
    context: &mut infiniloom_engine::index::ExpandedContext,
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

    let allowed_paths: std::collections::HashSet<&str> = context
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

// XML escaping utilities

fn escape_xml_text(input: &str) -> String {
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

fn escape_xml_attr(input: &str) -> String {
    escape_xml_text(input)
}
