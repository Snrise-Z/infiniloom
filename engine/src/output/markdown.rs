//! GPT-optimized Markdown output formatter
//!
//! Supports both in-memory (`format()`) and streaming (`format_to_writer()`) modes.

use crate::output::{Formatter, StreamingFormatter};
use crate::repomap::RepoMap;
use crate::types::{Repository, TokenizerModel};
use std::io::{self, Write};

/// Markdown formatter optimized for GPT
pub struct MarkdownFormatter {
    /// Include overview tables
    include_tables: bool,
    /// Include Mermaid diagrams
    include_mermaid: bool,
    /// Include file tree
    include_tree: bool,
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Token model for counts in output
    token_model: TokenizerModel,
}

impl MarkdownFormatter {
    /// Create a new Markdown formatter
    pub fn new() -> Self {
        Self {
            include_tables: true,
            include_mermaid: true,
            include_tree: true,
            include_line_numbers: true,
            token_model: TokenizerModel::Claude,
        }
    }

    /// Set tables option
    pub fn with_tables(mut self, enabled: bool) -> Self {
        self.include_tables = enabled;
        self
    }

    /// Set Mermaid option
    pub fn with_mermaid(mut self, enabled: bool) -> Self {
        self.include_mermaid = enabled;
        self
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    /// Set token model for token counts in output
    pub fn with_model(mut self, model: TokenizerModel) -> Self {
        self.token_model = model;
        self
    }

    /// Estimate output size for pre-allocation
    fn estimate_output_size(repo: &Repository) -> usize {
        let base = 1000;
        let files = repo.files.len() * 400;
        let content: usize = repo
            .files
            .iter()
            .filter_map(|f| f.content.as_ref())
            .map(|c| c.len())
            .sum();
        base + files + content
    }

    // =========================================================================
    // Streaming methods (write to impl std::io::Write)
    // =========================================================================

    fn stream_header<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "# Repository: {}", repo.name)?;
        writeln!(w)?;
        writeln!(
            w,
            "> **Files**: {} | **Lines**: {} | **Tokens**: {}",
            repo.metadata.total_files,
            repo.metadata.total_lines,
            repo.metadata.total_tokens.get(self.token_model)
        )?;
        writeln!(w)
    }

    fn stream_overview<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if !self.include_tables {
            return Ok(());
        }

        writeln!(w, "## Overview")?;
        writeln!(w)?;
        writeln!(w, "| Metric | Value |")?;
        writeln!(w, "|--------|-------|")?;
        writeln!(w, "| Files | {} |", repo.metadata.total_files)?;
        writeln!(w, "| Lines | {} |", repo.metadata.total_lines)?;

        if let Some(lang) = repo.metadata.languages.first() {
            writeln!(w, "| Primary Language | {} |", lang.language)?;
        }
        if let Some(framework) = &repo.metadata.framework {
            writeln!(w, "| Framework | {} |", framework)?;
        }
        writeln!(w)?;

        if repo.metadata.languages.len() > 1 {
            writeln!(w, "### Languages")?;
            writeln!(w)?;
            writeln!(w, "| Language | Files | Percentage |")?;
            writeln!(w, "|----------|-------|------------|")?;
            for lang in &repo.metadata.languages {
                writeln!(w, "| {} | {} | {:.1}% |", lang.language, lang.files, lang.percentage)?;
            }
            writeln!(w)?;
        }
        Ok(())
    }

    fn stream_repomap<W: Write>(&self, w: &mut W, map: &RepoMap) -> io::Result<()> {
        writeln!(w, "## Repository Map")?;
        writeln!(w)?;
        writeln!(w, "{}", map.summary)?;
        writeln!(w)?;

        writeln!(w, "### Key Symbols")?;
        writeln!(w)?;
        writeln!(w, "| Rank | Symbol | Type | File | Line | Summary |")?;
        writeln!(w, "|------|--------|------|------|------|---------|")?;
        for sym in map.key_symbols.iter().take(15) {
            let summary = sym
                .summary
                .as_deref()
                .map(escape_markdown_cell)
                .unwrap_or_default();
            writeln!(
                w,
                "| {} | `{}` | {} | {} | {} | {} |",
                sym.rank, sym.name, sym.kind, sym.file, sym.line, summary
            )?;
        }
        writeln!(w)?;

        if self.include_mermaid && !map.module_graph.edges.is_empty() {
            writeln!(w, "### Module Dependencies")?;
            writeln!(w)?;
            writeln!(w, "```mermaid")?;
            writeln!(w, "graph LR")?;
            for edge in &map.module_graph.edges {
                let sanitize_id = |s: &str| -> String {
                    s.chars()
                        .map(|c| if c == '-' || c == '.' { '_' } else { c })
                        .collect()
                };
                let from_id = sanitize_id(&edge.from);
                let to_id = sanitize_id(&edge.to);
                writeln!(w, "    {}[\"{}\"] --> {}[\"{}\"]", from_id, edge.from, to_id, edge.to)?;
            }
            writeln!(w, "```")?;
            writeln!(w)?;
        }
        Ok(())
    }

    fn stream_structure<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if !self.include_tree {
            return Ok(());
        }

        writeln!(w, "## Project Structure")?;
        writeln!(w)?;
        writeln!(w, "```")?;

        let mut paths: Vec<_> = repo
            .files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect();
        paths.sort();

        let mut prev_parts: Vec<&str> = Vec::new();
        for path in paths {
            let parts: Vec<_> = path.split('/').collect();
            let mut common = 0;
            for (i, part) in parts.iter().enumerate() {
                if i < prev_parts.len() && prev_parts[i] == *part {
                    common = i + 1;
                } else {
                    break;
                }
            }
            for (i, part) in parts.iter().enumerate().skip(common) {
                let indent = "  ".repeat(i);
                let prefix = if i == parts.len() - 1 {
                    "📄 "
                } else {
                    "📁 "
                };
                writeln!(w, "{}{}{}", indent, prefix, part)?;
            }
            prev_parts = parts;
        }

        writeln!(w, "```")?;
        writeln!(w)
    }

    fn stream_files<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "## Files")?;
        writeln!(w)?;

        for file in &repo.files {
            if let Some(content) = &file.content {
                writeln!(w, "### {}", file.relative_path)?;
                writeln!(w)?;
                writeln!(
                    w,
                    "> **Tokens**: {} | **Language**: {}",
                    file.token_count.get(self.token_model),
                    file.language.as_deref().unwrap_or("unknown")
                )?;
                writeln!(w)?;

                let lang = file.language.as_deref().unwrap_or("");
                writeln!(w, "```{}", lang)?;
                if self.include_line_numbers {
                    // Check if content has embedded line numbers (format: "N:content")
                    // This preserves original line numbers when content has been compressed
                    let first_line = content.lines().next().unwrap_or("");
                    let has_embedded_line_nums = first_line.contains(':')
                        && first_line
                            .split(':')
                            .next()
                            .map(|s| s.parse::<u32>().is_ok())
                            .unwrap_or(false);

                    if has_embedded_line_nums {
                        // Content has embedded line numbers - parse and output
                        for line in content.lines() {
                            if let Some((num_str, rest)) = line.split_once(':') {
                                if let Ok(line_num) = num_str.parse::<u32>() {
                                    writeln!(w, "{:4} {}", line_num, rest)?;
                                } else {
                                    // Fallback for malformed lines
                                    writeln!(w, "     {}", line)?;
                                }
                            } else {
                                writeln!(w, "     {}", line)?;
                            }
                        }
                    } else {
                        // No embedded line numbers - use sequential (uncompressed content)
                        for (i, line) in content.lines().enumerate() {
                            writeln!(w, "{:4} {}", i + 1, line)?;
                        }
                    }
                } else {
                    writeln!(w, "{}", content)?;
                }
                writeln!(w, "```")?;
                writeln!(w)?;
            }
        }
        Ok(())
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for MarkdownFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        // Use streaming internally for consistency
        let mut output = Vec::with_capacity(Self::estimate_output_size(repo));
        // Vec<u8> write cannot fail, ignore result
        drop(self.format_to_writer(repo, map, &mut output));
        // Use lossy conversion to handle any edge cases with invalid UTF-8
        String::from_utf8(output)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = Vec::with_capacity(Self::estimate_output_size(repo));
        // Vec<u8> write cannot fail, ignore result
        drop(self.format_repo_to_writer(repo, &mut output));
        // Use lossy conversion to handle any edge cases with invalid UTF-8
        String::from_utf8(output)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }

    fn name(&self) -> &'static str {
        "markdown"
    }
}

impl StreamingFormatter for MarkdownFormatter {
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()> {
        self.stream_header(writer, repo)?;
        self.stream_overview(writer, repo)?;
        self.stream_repomap(writer, map)?;
        self.stream_structure(writer, repo)?;
        self.stream_files(writer, repo)?;
        Ok(())
    }

    fn format_repo_to_writer<W: Write>(&self, repo: &Repository, writer: &mut W) -> io::Result<()> {
        self.stream_header(writer, repo)?;
        self.stream_overview(writer, repo)?;
        self.stream_structure(writer, repo)?;
        self.stream_files(writer, repo)?;
        Ok(())
    }
}

fn escape_markdown_cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace('\n', " ")
        .trim()
        .to_owned()
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::repomap::RepoMapGenerator;
    use crate::types::{LanguageStats, RepoFile, RepoMetadata, TokenCounts};

    fn create_test_repo() -> Repository {
        Repository {
            name: "test".to_string(),
            path: "/tmp/test".into(),
            files: vec![RepoFile {
                path: "/tmp/test/main.py".into(),
                relative_path: "main.py".to_string(),
                language: Some("python".to_string()),
                size_bytes: 100,
                token_count: TokenCounts {
                    o200k: 48,
                    cl100k: 49,
                    claude: 50,
                    gemini: 47,
                    llama: 46,
                    mistral: 46,
                    deepseek: 46,
                    qwen: 46,
                    cohere: 47,
                    grok: 46,
                },
                symbols: Vec::new(),
                importance: 0.8,
                content: Some("def main():\n    print('hello')".to_string()),
            }],
            metadata: RepoMetadata {
                total_files: 1,
                total_lines: 2,
                total_tokens: TokenCounts {
                    o200k: 48,
                    cl100k: 49,
                    claude: 50,
                    gemini: 47,
                    llama: 46,
                    mistral: 46,
                    deepseek: 46,
                    qwen: 46,
                    cohere: 47,
                    grok: 46,
                },
                languages: vec![LanguageStats {
                    language: "Python".to_string(),
                    files: 1,
                    lines: 2,
                    percentage: 100.0,
                }],
                framework: None,
                description: None,
                branch: None,
                commit: None,
                directory_structure: None,
                external_dependencies: vec![],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_markdown_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = MarkdownFormatter::new();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("# Repository: test"));
        assert!(output.contains("## Overview"));
        assert!(output.contains("```python"));
    }
}
