//! Output formatting and extras for pack command
//!
//! This module provides functions for enriching generated output with:
//! - Header text and instructions
//! - Token tree (per-file token counts)
//! - Security scan results
//! - Git context (commits, uncommitted changes)

use anyhow::{Context, Result};
use infiniloom_engine::{
    output::{
        escaping::{escape_xml_text, escape_yaml_string},
        OutputFormat,
    },
    types::TokenizerModel,
};
use std::path::PathBuf;

// Type definitions for serializable data structures
#[derive(serde::Serialize)]
pub(crate) struct TokenTreeEntry {
    pub path: String,
    pub tokens: u32,
}

#[derive(serde::Serialize)]
pub(crate) struct SecurityIssueEntry {
    pub file: String,
    pub line: u32,
    pub kind: String,
    pub severity: String,
}

/// Read instruction file contents
///
/// # Arguments
///
/// * `instruction_file` - Optional path to instruction file
///
/// # Returns
///
/// Returns `Some(String)` with file contents if path provided and file exists, `None` otherwise.
pub(crate) fn read_instruction_file(instruction_file: &Option<PathBuf>) -> Result<Option<String>> {
    let path = match instruction_file {
        Some(path) => path,
        None => return Ok(None),
    };
    let instructions = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read instruction file: {}", path.display()))?;
    Ok(Some(instructions))
}

/// Generate token tree entries for all files in repository
fn token_tree_entries(
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
) -> Vec<TokenTreeEntry> {
    repo.files
        .iter()
        .map(|file| TokenTreeEntry {
            path: file.relative_path.clone(),
            tokens: file.token_count.get(model),
        })
        .collect()
}

/// Convert security findings to serializable entries
fn security_issue_entries(
    issues: &[infiniloom_engine::security::SecretFinding],
) -> Vec<SecurityIssueEntry> {
    issues
        .iter()
        .map(|issue| SecurityIssueEntry {
            file: issue.file.clone(),
            line: issue.line,
            kind: issue.kind.name().to_owned(),
            severity: format!("{:?}", issue.severity),
        })
        .collect()
}

/// Append YAML block with multi-line value
pub(crate) fn append_yaml_block(output: &mut String, key: &str, value: &str) {
    output.push_str(&format!("\n{}: |\n", key));
    for line in value.lines() {
        output.push_str(&format!("  {}\n", line));
    }
}

/// Append git context in Markdown format
fn append_git_context_markdown(
    output: &mut String,
    history: &infiniloom_engine::types::GitHistory,
) {
    if !history.commits.is_empty() {
        output.push_str("\n\n## Recent Commits\n\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "- **{}** {} - {}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\n\n## Uncommitted Changes\n\n");
        for file in &history.changed_files {
            output.push_str(&format!("- [{}] {}\n", file.status, file.path));
        }
    }
}

/// Append git context in Plain text format
fn append_git_context_plain(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str("\n\nRECENT COMMITS\n");
        output.push_str("--------------\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "{} {} - {}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\n\nUNCOMMITTED CHANGES\n");
        output.push_str("-------------------\n");
        for file in &history.changed_files {
            output.push_str(&format!("[{}] {}\n", file.status, file.path));
        }
    }
}

/// Append git context in TOON format
fn append_git_context_toon(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str(&format!(
            "\n\nrecent_commits[{}]{{hash,message,author}}:\n",
            history.commits.len()
        ));
        for commit in &history.commits {
            output.push_str(&format!(
                "  {},{},{}\n",
                commit.short_hash, commit.message, commit.author
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str(&format!(
            "\n\nuncommitted_changes[{}]{{status,path}}:\n",
            history.changed_files.len()
        ));
        for file in &history.changed_files {
            output.push_str(&format!("  {},{}\n", file.status, file.path));
        }
    }
}

/// Append git context in YAML format
fn append_git_context_yaml(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if !history.commits.is_empty() {
        output.push_str("\nrecent_commits:\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "  - hash: {}\n    message: {}\n    author: {}\n",
                escape_yaml_string(&commit.short_hash),
                escape_yaml_string(&commit.message),
                escape_yaml_string(&commit.author)
            ));
        }
    }
    if !history.changed_files.is_empty() {
        output.push_str("\nuncommitted_changes:\n");
        for file in &history.changed_files {
            output.push_str(&format!(
                "  - status: {}\n    path: {}\n",
                escape_yaml_string(&file.status),
                escape_yaml_string(&file.path)
            ));
        }
    }
}

/// Apply pack extras to formatted output
///
/// Enriches the formatted output with optional extras:
/// - Header text (custom text at the start)
/// - Instructions (from instruction file)
/// - Token tree (per-file token counts)
/// - Security scan results
/// - Git context (commits, uncommitted changes)
///
/// The formatting varies by output format (JSON, YAML, XML, Markdown, Plain, TOON).
///
/// # Arguments
///
/// * `output_text` - Base formatted output
/// * `format` - Output format
/// * `repo` - Repository with metadata
/// * `model` - Tokenizer model for token tree
/// * `header_text` - Optional header text
/// * `instructions` - Optional instructions
/// * `token_tree` - Whether to include token tree
/// * `security_issues` - Optional security findings
/// * `include_git_context` - Whether to include git history
///
/// # Returns
///
/// Enriched output text with extras appended/inserted based on format.
pub(crate) fn apply_pack_extras(
    output_text: String,
    format: OutputFormat,
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
    header_text: Option<&str>,
    instructions: Option<&str>,
    token_tree: bool,
    security_issues: Option<&[infiniloom_engine::security::SecretFinding]>,
    include_git_context: bool,
) -> Result<String> {
    let token_tree_entries = if token_tree {
        Some(token_tree_entries(repo, model))
    } else {
        None
    };
    let security_entries = security_issues.map(security_issue_entries);
    let git_history = if include_git_context {
        repo.metadata.git_history.as_ref()
    } else {
        None
    };

    match format {
        OutputFormat::Json => {
            let mut root: serde_json::Value =
                serde_json::from_str(&output_text).context("Failed to parse JSON output")?;
            let obj = root
                .as_object_mut()
                .context("JSON output is not an object")?;

            if let Some(header) = header_text {
                obj.insert("header_text".to_owned(), serde_json::Value::String(header.to_owned()));
            }
            if let Some(instructions) = instructions {
                obj.insert(
                    "instructions".to_owned(),
                    serde_json::Value::String(instructions.to_owned()),
                );
            }
            if let Some(entries) = token_tree_entries {
                obj.insert(
                    "token_tree".to_owned(),
                    serde_json::json!({ "model": model.name(), "files": entries }),
                );
            }
            if let Some(entries) = security_entries {
                obj.insert(
                    "security_scan".to_owned(),
                    serde_json::json!({ "issues_found": entries.len(), "issues": entries }),
                );
            }

            serde_json::to_string_pretty(&root)
                .context("Failed to serialize JSON output with extras")
        },
        OutputFormat::Yaml => {
            let mut output = output_text;
            if !output.ends_with('\n') {
                output.push('\n');
            }

            if let Some(header) = header_text {
                append_yaml_block(&mut output, "header_text", header);
            }
            if let Some(instructions) = instructions {
                append_yaml_block(&mut output, "instructions", instructions);
            }
            if let Some(entries) = token_tree_entries {
                output.push_str("\ntoken_tree:\n");
                output.push_str(&format!("  model: {}\n", escape_yaml_string(model.name())));
                output.push_str("  files:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - path: {}\n      tokens: {}\n",
                        escape_yaml_string(&entry.path),
                        entry.tokens
                    ));
                }
            }
            if let Some(entries) = security_entries {
                output.push_str("\nsecurity_scan:\n");
                output.push_str(&format!("  issues_found: {}\n", entries.len()));
                output.push_str("  issues:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - file: {}\n      line: {}\n      kind: {}\n      severity: {}\n",
                        escape_yaml_string(&entry.file),
                        entry.line,
                        escape_yaml_string(&entry.kind),
                        escape_yaml_string(&entry.severity)
                    ));
                }
            }
            if let Some(history) = git_history {
                append_git_context_yaml(&mut output, history);
            }

            Ok(output)
        },
        OutputFormat::Xml => {
            let mut extras = String::new();
            if header_text.is_some()
                || instructions.is_some()
                || token_tree_entries.is_some()
                || security_entries.is_some()
            {
                extras.push_str("  <extras>\n");
                if let Some(header) = header_text {
                    extras.push_str(&format!(
                        "    <header_text>{}</header_text>\n",
                        escape_xml_text(header)
                    ));
                }
                if let Some(instructions) = instructions {
                    extras.push_str(&format!(
                        "    <instructions>{}</instructions>\n",
                        escape_xml_text(instructions)
                    ));
                }
                if let Some(entries) = token_tree_entries {
                    extras.push_str(&format!(
                        "    <token_tree model=\"{}\">\n",
                        escape_xml_text(model.name())
                    ));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <file path=\"{}\" tokens=\"{}\"/>\n",
                            escape_xml_text(&entry.path),
                            entry.tokens
                        ));
                    }
                    extras.push_str("    </token_tree>\n");
                }
                if let Some(entries) = security_entries {
                    extras.push_str(&format!("    <security_scan issues=\"{}\">\n", entries.len()));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <issue file=\"{}\" line=\"{}\" kind=\"{}\" severity=\"{}\"/>\n",
                            escape_xml_text(&entry.file),
                            entry.line,
                            escape_xml_text(&entry.kind),
                            escape_xml_text(&entry.severity)
                        ));
                    }
                    extras.push_str("    </security_scan>\n");
                }
                extras.push_str("  </extras>\n");
            }

            if extras.is_empty() {
                return Ok(output_text);
            }

            // Insert extras before closing </repository> tag
            if let Some(pos) = output_text.rfind("</repository>") {
                let mut output = String::with_capacity(output_text.len() + extras.len() + 2);
                output.push_str(&output_text[..pos]);
                output.push('\n');
                output.push_str(&extras);
                output.push_str(&output_text[pos..]);
                Ok(output)
            } else {
                Ok(format!("{}\n{}", output_text, extras))
            }
        },
        OutputFormat::Markdown => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_markdown(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\n## Security Scan Results\n\n");
                output.push_str(&format!("Found {} potential security issues.\n\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!(
                    "\n\n## Token Tree\n\n| File | Tokens ({}) |\n|------|--------|\n",
                    model.name()
                ));
                for entry in entries {
                    output.push_str(&format!("| {} | {} |\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\n## Instructions\n\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Plain => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_plain(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\nSECURITY SCAN RESULTS\n");
                output.push_str("----------------------\n");
                output.push_str(&format!("Found {} potential security issues.\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\nTOKEN TREE ({})\n", model.name()));
                output.push_str("----------------------\n");
                for entry in entries {
                    output.push_str(&format!("- {}: {}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\nINSTRUCTIONS\n");
                output.push_str("------------\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Toon => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str("header_text: |\n");
                for line in header.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
                output.push('\n');
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_toon(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str(&format!(
                    "\n\nsecurity_scan[{}]{{severity,kind,file,line}}:\n",
                    entries.len()
                ));
                for entry in entries {
                    output.push_str(&format!(
                        "  {},{},{},{}\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\ntoken_tree_model: {}\n", model.name()));
                output.push_str(&format!("token_tree[{}]{{path,tokens}}:\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!("  {},{}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\ninstructions: |\n");
                for line in instructions.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }

            Ok(output)
        },
    }
}
