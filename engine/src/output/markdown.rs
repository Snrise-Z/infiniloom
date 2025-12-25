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
    use crate::repomap::{
        FileIndexEntry, ModuleEdge, ModuleGraph, ModuleNode, RankedSymbol, RepoMap,
        RepoMapGenerator,
    };
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

    fn create_multi_language_repo() -> Repository {
        Repository {
            name: "multi-lang".to_string(),
            path: "/tmp/multi".into(),
            files: vec![
                RepoFile {
                    path: "/tmp/multi/src/main.rs".into(),
                    relative_path: "src/main.rs".to_string(),
                    language: Some("rust".to_string()),
                    size_bytes: 200,
                    token_count: TokenCounts::default(),
                    symbols: Vec::new(),
                    importance: 0.9,
                    content: Some("fn main() {\n    println!(\"hello\");\n}".to_string()),
                },
                RepoFile {
                    path: "/tmp/multi/src/lib.rs".into(),
                    relative_path: "src/lib.rs".to_string(),
                    language: Some("rust".to_string()),
                    size_bytes: 150,
                    token_count: TokenCounts::default(),
                    symbols: Vec::new(),
                    importance: 0.8,
                    content: Some("pub mod utils;".to_string()),
                },
                RepoFile {
                    path: "/tmp/multi/tests/test.py".into(),
                    relative_path: "tests/test.py".to_string(),
                    language: Some("python".to_string()),
                    size_bytes: 100,
                    token_count: TokenCounts::default(),
                    symbols: Vec::new(),
                    importance: 0.5,
                    content: Some("def test_it(): pass".to_string()),
                },
            ],
            metadata: RepoMetadata {
                total_files: 3,
                total_lines: 5,
                total_tokens: TokenCounts::default(),
                languages: vec![
                    LanguageStats {
                        language: "Rust".to_string(),
                        files: 2,
                        lines: 4,
                        percentage: 66.7,
                    },
                    LanguageStats {
                        language: "Python".to_string(),
                        files: 1,
                        lines: 1,
                        percentage: 33.3,
                    },
                ],
                framework: Some("Actix".to_string()),
                description: Some("Test project".to_string()),
                branch: Some("main".to_string()),
                commit: Some("abc123".to_string()),
                directory_structure: None,
                external_dependencies: vec!["tokio".to_string()],
                git_history: None,
            },
        }
    }

    fn create_test_map() -> RepoMap {
        RepoMap {
            summary: "Test repository with 1 key symbol".to_string(),
            key_symbols: vec![RankedSymbol {
                rank: 1,
                name: "main".to_string(),
                kind: "function".to_string(),
                file: "main.py".to_string(),
                line: 1,
                signature: None,
                summary: Some("Entry point".to_string()),
                references: 0,
                importance: 0.95,
            }],
            module_graph: ModuleGraph {
                nodes: vec![ModuleNode { name: "main".to_string(), files: 1, tokens: 50 }],
                edges: vec![],
            },
            file_index: vec![FileIndexEntry {
                path: "main.py".to_string(),
                tokens: 50,
                importance: "high".to_string(),
                summary: None,
            }],
            token_count: 50,
        }
    }

    fn create_map_with_mermaid() -> RepoMap {
        RepoMap {
            summary: "Test with dependencies".to_string(),
            key_symbols: vec![
                RankedSymbol {
                    rank: 1,
                    name: "main".to_string(),
                    kind: "function".to_string(),
                    file: "main.rs".to_string(),
                    line: 1,
                    signature: Some("fn main()".to_string()),
                    summary: Some("Entry | point".to_string()),
                    references: 5,
                    importance: 0.95,
                },
                RankedSymbol {
                    rank: 2,
                    name: "helper".to_string(),
                    kind: "function".to_string(),
                    file: "lib.rs".to_string(),
                    line: 5,
                    signature: None,
                    summary: None,
                    references: 2,
                    importance: 0.7,
                },
            ],
            module_graph: ModuleGraph {
                nodes: vec![
                    ModuleNode { name: "main".to_string(), files: 1, tokens: 100 },
                    ModuleNode { name: "lib".to_string(), files: 1, tokens: 80 },
                ],
                edges: vec![ModuleEdge {
                    from: "main-mod".to_string(),
                    to: "lib.rs".to_string(),
                    weight: 1,
                }],
            },
            file_index: vec![
                FileIndexEntry {
                    path: "main.rs".to_string(),
                    tokens: 100,
                    importance: "critical".to_string(),
                    summary: None,
                },
                FileIndexEntry {
                    path: "lib.rs".to_string(),
                    tokens: 80,
                    importance: "high".to_string(),
                    summary: None,
                },
            ],
            token_count: 100,
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

    #[test]
    fn test_markdown_default() {
        let formatter = MarkdownFormatter::default();
        assert_eq!(formatter.name(), "markdown");
    }

    #[test]
    fn test_builder_with_tables() {
        let formatter = MarkdownFormatter::new().with_tables(false);
        let repo = create_test_repo();
        let map = create_test_map();
        let output = formatter.format(&repo, &map);
        assert!(!output.contains("## Overview"));
        assert!(!output.contains("| Metric | Value |"));
    }

    #[test]
    fn test_builder_with_mermaid_disabled() {
        let formatter = MarkdownFormatter::new().with_mermaid(false);
        let repo = create_multi_language_repo();
        let map = create_map_with_mermaid();
        let output = formatter.format(&repo, &map);
        assert!(!output.contains("```mermaid"));
    }

    #[test]
    fn test_builder_with_mermaid_enabled() {
        let formatter = MarkdownFormatter::new().with_mermaid(true);
        let repo = create_multi_language_repo();
        let map = create_map_with_mermaid();
        let output = formatter.format(&repo, &map);
        assert!(output.contains("```mermaid"));
        assert!(output.contains("graph LR"));
        // Check ID sanitization (- and . replaced with _)
        assert!(output.contains("main_mod"));
        assert!(output.contains("lib_rs"));
    }

    #[test]
    fn test_builder_with_line_numbers_disabled() {
        let formatter = MarkdownFormatter::new().with_line_numbers(false);
        let repo = create_test_repo();
        let map = create_test_map();
        let output = formatter.format(&repo, &map);
        // Should NOT have line numbers like "   1 def main():"
        assert!(!output.contains("   1 def main"));
        // Should have raw content
        assert!(output.contains("def main():"));
    }

    #[test]
    fn test_builder_with_model() {
        let formatter = MarkdownFormatter::new().with_model(TokenizerModel::Gpt4o);
        let repo = create_test_repo();
        let map = create_test_map();
        let output = formatter.format(&repo, &map);
        // GPT-4o uses o200k encoding, which is 48 in our test data
        assert!(output.contains("**Tokens**: 48"));
    }

    #[test]
    fn test_estimate_output_size() {
        let repo = create_test_repo();
        let size = MarkdownFormatter::estimate_output_size(&repo);
        // base (1000) + files (1 * 400) + content length (~30)
        assert!(size > 1000);
        assert!(size < 2000);
    }

    #[test]
    fn test_stream_header() {
        let formatter = MarkdownFormatter::new();
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.stream_header(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("# Repository: test"));
        assert!(output.contains("**Files**: 1"));
        assert!(output.contains("**Lines**: 2"));
        assert!(output.contains("**Tokens**: 50")); // Claude tokens
    }

    #[test]
    fn test_stream_overview_with_framework() {
        let formatter = MarkdownFormatter::new();
        let repo = create_multi_language_repo();
        let mut buf = Vec::new();
        formatter.stream_overview(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("| Framework | Actix |"));
        assert!(output.contains("| Primary Language | Rust |"));
    }

    #[test]
    fn test_stream_overview_multiple_languages() {
        let formatter = MarkdownFormatter::new();
        let repo = create_multi_language_repo();
        let mut buf = Vec::new();
        formatter.stream_overview(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("### Languages"));
        assert!(output.contains("| Rust | 2 | 66.7% |"));
        assert!(output.contains("| Python | 1 | 33.3% |"));
    }

    #[test]
    fn test_stream_overview_disabled() {
        let formatter = MarkdownFormatter::new().with_tables(false);
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.stream_overview(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_stream_repomap() {
        let formatter = MarkdownFormatter::new();
        let map = create_test_map();
        let mut buf = Vec::new();
        formatter.stream_repomap(&mut buf, &map).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("## Repository Map"));
        assert!(output.contains("### Key Symbols"));
        assert!(output.contains("| 1 | `main` | function | main.py | 1 | Entry point |"));
    }

    #[test]
    fn test_stream_repomap_escapes_pipe_in_summary() {
        let formatter = MarkdownFormatter::new();
        let map = create_map_with_mermaid();
        let mut buf = Vec::new();
        formatter.stream_repomap(&mut buf, &map).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Pipe should be escaped
        assert!(output.contains("Entry \\| point"));
    }

    #[test]
    fn test_stream_structure() {
        let formatter = MarkdownFormatter::new();
        let repo = create_multi_language_repo();
        let mut buf = Vec::new();
        formatter.stream_structure(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("## Project Structure"));
        assert!(output.contains("```"));
    }

    #[test]
    fn test_stream_structure_disabled() {
        // Create a formatter with tree disabled by modifying internal state
        let mut formatter = MarkdownFormatter::new();
        formatter.include_tree = false;
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.stream_structure(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_stream_files_with_line_numbers() {
        let formatter = MarkdownFormatter::new().with_line_numbers(true);
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("### main.py"));
        assert!(output.contains("**Tokens**: 50"));
        assert!(output.contains("**Language**: python"));
        // Line numbers should be present
        assert!(output.contains("   1 def main():"));
        assert!(output.contains("   2     print('hello')"));
    }

    #[test]
    fn test_stream_files_without_line_numbers() {
        let formatter = MarkdownFormatter::new().with_line_numbers(false);
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should have raw content without line numbers
        assert!(output.contains("def main():\n    print('hello')"));
    }

    #[test]
    fn test_stream_files_with_embedded_line_numbers() {
        let mut repo = create_test_repo();
        // Set content with embedded line numbers (compressed format)
        repo.files[0].content = Some("1:def main():\n5:    print('hello')".to_string());
        let formatter = MarkdownFormatter::new().with_line_numbers(true);
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should parse and display original line numbers
        assert!(output.contains("   1 def main():"));
        assert!(output.contains("   5     print('hello')"));
    }

    #[test]
    fn test_stream_files_with_malformed_embedded_line_numbers() {
        let mut repo = create_test_repo();
        // Malformed embedded line numbers
        repo.files[0].content = Some("abc:def main():\nno_colon_here".to_string());
        let formatter = MarkdownFormatter::new().with_line_numbers(true);
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should handle gracefully - use sequential numbers since first line doesn't parse
        assert!(output.contains("   1 abc:def main():"));
    }

    #[test]
    fn test_stream_files_with_no_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = None;
        let formatter = MarkdownFormatter::new();
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should still have ## Files header but no file content
        assert!(output.contains("## Files"));
        assert!(!output.contains("### main.py"));
    }

    #[test]
    fn test_stream_files_unknown_language() {
        let mut repo = create_test_repo();
        repo.files[0].language = None;
        let formatter = MarkdownFormatter::new();
        let mut buf = Vec::new();
        formatter.stream_files(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("**Language**: unknown"));
    }

    #[test]
    fn test_format_repo_without_map() {
        let formatter = MarkdownFormatter::new();
        let repo = create_test_repo();
        let output = formatter.format_repo(&repo);
        assert!(output.contains("# Repository: test"));
        assert!(output.contains("## Overview"));
        // Should NOT have repomap section
        assert!(!output.contains("## Repository Map"));
    }

    #[test]
    fn test_streaming_formatter_trait() {
        let formatter = MarkdownFormatter::new();
        let repo = create_test_repo();
        let map = create_test_map();
        let mut buf = Vec::new();
        formatter.format_to_writer(&repo, &map, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("# Repository: test"));
        assert!(output.contains("## Repository Map"));
    }

    #[test]
    fn test_streaming_formatter_repo_only() {
        let formatter = MarkdownFormatter::new();
        let repo = create_test_repo();
        let mut buf = Vec::new();
        formatter.format_repo_to_writer(&repo, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("# Repository: test"));
        assert!(!output.contains("## Repository Map"));
    }

    #[test]
    fn test_escape_markdown_cell() {
        assert_eq!(escape_markdown_cell("hello"), "hello");
        assert_eq!(escape_markdown_cell("a|b"), "a\\|b");
        assert_eq!(escape_markdown_cell("line1\nline2"), "line1 line2");
        assert_eq!(escape_markdown_cell("  spaced  "), "spaced");
        assert_eq!(escape_markdown_cell("a|b\nc|d"), "a\\|b c\\|d");
    }

    #[test]
    fn test_escape_markdown_cell_complex() {
        // Multiple pipes and newlines
        let input = "col1|col2|col3\nrow1|row2|row3";
        let expected = "col1\\|col2\\|col3 row1\\|row2\\|row3";
        assert_eq!(escape_markdown_cell(input), expected);
    }

    #[test]
    fn test_full_format_with_all_features() {
        let formatter = MarkdownFormatter::new()
            .with_tables(true)
            .with_mermaid(true)
            .with_line_numbers(true)
            .with_model(TokenizerModel::Claude);
        let repo = create_multi_language_repo();
        let map = create_map_with_mermaid();
        let output = formatter.format(&repo, &map);

        // All sections present
        assert!(output.contains("# Repository: multi-lang"));
        assert!(output.contains("## Overview"));
        assert!(output.contains("## Repository Map"));
        assert!(output.contains("## Project Structure"));
        assert!(output.contains("## Files"));
        assert!(output.contains("```mermaid"));
    }

    #[test]
    fn test_format_with_empty_repo() {
        let repo = Repository {
            name: "empty".to_string(),
            path: "/tmp/empty".into(),
            files: vec![],
            metadata: RepoMetadata::default(),
        };
        let map = RepoMap {
            summary: "Empty repository".to_string(),
            key_symbols: vec![],
            module_graph: ModuleGraph { nodes: vec![], edges: vec![] },
            file_index: vec![],
            token_count: 0,
        };
        let formatter = MarkdownFormatter::new();
        let output = formatter.format(&repo, &map);
        assert!(output.contains("# Repository: empty"));
        // Should not fail
    }

    #[test]
    fn test_estimate_output_size_empty_repo() {
        let repo = Repository {
            name: "empty".to_string(),
            path: "/tmp/empty".into(),
            files: vec![],
            metadata: RepoMetadata::default(),
        };
        let size = MarkdownFormatter::estimate_output_size(&repo);
        assert_eq!(size, 1000); // Just base size
    }

    #[test]
    fn test_estimate_output_size_with_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("x".repeat(5000));
        let size = MarkdownFormatter::estimate_output_size(&repo);
        // base (1000) + files (1 * 400) + content (5000)
        assert_eq!(size, 6400);
    }

    #[test]
    fn test_structure_nested_paths() {
        let repo = Repository {
            name: "nested".to_string(),
            path: "/tmp/nested".into(),
            files: vec![
                RepoFile::new("/tmp/nested/src/a/b/c.rs", "src/a/b/c.rs"),
                RepoFile::new("/tmp/nested/src/a/b/d.rs", "src/a/b/d.rs"),
                RepoFile::new("/tmp/nested/src/a/e.rs", "src/a/e.rs"),
                RepoFile::new("/tmp/nested/tests/test.rs", "tests/test.rs"),
            ],
            metadata: RepoMetadata::default(),
        };
        let formatter = MarkdownFormatter::new();
        let mut buf = Vec::new();
        formatter.stream_structure(&mut buf, &repo).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should show directory structure
        assert!(output.contains("src"));
        assert!(output.contains("tests"));
    }

    #[test]
    fn test_name_method() {
        let formatter = MarkdownFormatter::new();
        assert_eq!(formatter.name(), "markdown");
    }
}
