//! Output formatting for diff context
//!
//! This module provides formatters for diff context in various output formats:
//! - XML (Claude-optimized)
//! - JSON (structured data)
//! - Markdown (human-readable)
//! - YAML (Gemini-optimized)
//! - TOON (token-efficient)
//! - Plain text (simple)

use std::collections::HashMap;

use infiniloom_engine::index::ExpandedContext;
use infiniloom_engine::output::{
    escaping::{escape_xml_attribute, escape_xml_text},
    OutputFormat,
};

/// Type alias for file history map (file path -> list of commits)
pub(crate) type FileHistory = HashMap<String, Vec<infiniloom_engine::git::Commit>>;

/// Generate preamble text for diff context
pub(crate) fn diff_preamble(context: &ExpandedContext) -> String {
    format!(
        "Use this diff context to understand changes. Start with changed file snippets, then dependent symbols/files/tests. Impact: {}.",
        context.impact_summary.level.name()
    )
}

/// Format diff context for output in the specified format
pub(crate) fn format_diff_context(
    context: &ExpandedContext,
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

fn format_diff_context_json(context: &ExpandedContext, history: &FileHistory) -> String {
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

fn format_diff_context_markdown(context: &ExpandedContext, history: &FileHistory) -> String {
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

fn format_diff_context_yaml(context: &ExpandedContext, history: &FileHistory) -> String {
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

fn format_diff_context_toon(context: &ExpandedContext, history: &FileHistory) -> String {
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

fn format_diff_context_plain(context: &ExpandedContext, history: &FileHistory) -> String {
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

fn format_diff_context_xml(context: &ExpandedContext, history: &FileHistory) -> String {
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
                            escape_xml_attribute(&commit.short_hash),
                            escape_xml_attribute(&commit.date),
                            escape_xml_attribute(&commit.author),
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
pub(crate) struct SnippetRange {
    pub start: u32,
    pub end: u32,
    pub reasons: Vec<String>,
}

pub(crate) fn merge_snippet_ranges(mut ranges: Vec<SnippetRange>) -> Vec<SnippetRange> {
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

pub(crate) fn line_contains_symbol_name(line: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut offset = 0;
    while let Some(pos) = line[offset..].find(name) {
        let start = offset + pos;
        let end = start + name.len();

        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();

        let before_ok = before.map_or(true, |c| !is_word_char(c));
        let after_ok = after.map_or(true, |c| !is_word_char(c));

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
