//! TOON (Token-Oriented Object Notation) output formatter
//!
//! TOON is a compact, human-readable format designed for LLM context.
//! It provides ~40% fewer tokens than JSON while maintaining readability.
//!
//! Supports both in-memory (`format()`) and streaming (`format_to_writer()`) modes.
//!
//! Format specification: https://github.com/toon-format/toon

use crate::output::{Formatter, StreamingFormatter};
use crate::repomap::RepoMap;
use crate::types::{Repository, TokenizerModel};
use std::io::{self, Write};

/// TOON formatter - most token-efficient format for LLMs
pub struct ToonFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Use tabular format for file metadata
    use_tabular: bool,
    /// Include file index/summary section
    show_file_index: bool,
    /// Token model for counts in output
    token_model: TokenizerModel,
}

impl ToonFormatter {
    /// Create a new TOON formatter with default settings
    pub fn new() -> Self {
        Self {
            include_line_numbers: true,
            use_tabular: true,
            show_file_index: true,
            token_model: TokenizerModel::Claude,
        }
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    /// Set tabular format option
    pub fn with_tabular(mut self, enabled: bool) -> Self {
        self.use_tabular = enabled;
        self
    }

    /// Set file index/summary option
    pub fn with_file_index(mut self, enabled: bool) -> Self {
        self.show_file_index = enabled;
        self
    }

    /// Set token model for token counts in output
    pub fn with_model(mut self, model: TokenizerModel) -> Self {
        self.token_model = model;
        self
    }

    /// Estimate output size for pre-allocation
    fn estimate_output_size(repo: &Repository) -> usize {
        // Base overhead for headers and metadata
        let base = 500;
        // Estimate ~300 bytes per file for content + metadata
        let files = repo.files.len() * 300;
        // Estimate content size
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

    fn stream_metadata<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "metadata:")?;
        writeln!(w, "  name: {}", repo.name)?;
        writeln!(w, "  files: {}", repo.metadata.total_files)?;
        writeln!(w, "  lines: {}", repo.metadata.total_lines)?;
        writeln!(w, "  tokens: {}", repo.metadata.total_tokens.get(self.token_model))?;

        if let Some(ref desc) = repo.metadata.description {
            writeln!(w, "  description: {}", escape_toon(desc))?;
        }
        if let Some(ref branch) = repo.metadata.branch {
            writeln!(w, "  branch: {}", branch)?;
        }
        if let Some(ref commit) = repo.metadata.commit {
            writeln!(w, "  commit: {}", commit)?;
        }
        writeln!(w)
    }

    fn stream_languages<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if repo.metadata.languages.is_empty() {
            return Ok(());
        }

        let count = repo.metadata.languages.len();
        writeln!(w, "languages[{}]{{name,files,percentage}}:", count)?;
        for lang in &repo.metadata.languages {
            writeln!(w, "  {},{},{:.1}", lang.language, lang.files, lang.percentage)?;
        }
        writeln!(w)
    }

    fn stream_directory_structure<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if let Some(ref structure) = repo.metadata.directory_structure {
            writeln!(w, "directory_structure: |")?;
            for line in structure.lines() {
                writeln!(w, "  {}", line)?;
            }
            writeln!(w)?;
        }
        Ok(())
    }

    fn stream_dependencies<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if repo.metadata.external_dependencies.is_empty() {
            return Ok(());
        }

        let count = repo.metadata.external_dependencies.len();
        writeln!(w, "dependencies[{}]:", count)?;
        for dep in &repo.metadata.external_dependencies {
            writeln!(w, "  {}", escape_toon(dep))?;
        }
        writeln!(w)
    }

    fn stream_repomap<W: Write>(&self, w: &mut W, map: &RepoMap) -> io::Result<()> {
        writeln!(w, "repository_map:")?;
        writeln!(w, "  token_budget: {}", map.token_count)?;
        writeln!(w, "  summary: |")?;
        for line in map.summary.lines() {
            writeln!(w, "    {}", line)?;
        }

        if !map.key_symbols.is_empty() {
            let count = map.key_symbols.len();
            writeln!(w, "  symbols[{}]{{name,type,file,line,rank,summary}}:", count)?;
            for sym in &map.key_symbols {
                writeln!(
                    w,
                    "    {},{},{},{},{},{}",
                    escape_toon(&sym.name),
                    escape_toon(&sym.kind),
                    escape_toon(&sym.file),
                    sym.line,
                    sym.rank,
                    escape_toon(sym.summary.as_deref().unwrap_or(""))
                )?;
            }
        }

        if !map.module_graph.nodes.is_empty() {
            let count = map.module_graph.nodes.len();
            writeln!(w, "  modules[{}]{{name,files,tokens}}:", count)?;
            for module in &map.module_graph.nodes {
                writeln!(
                    w,
                    "    {},{},{}",
                    escape_toon(&module.name),
                    module.files,
                    module.tokens
                )?;
            }
        }
        writeln!(w)
    }

    fn stream_file_index<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if repo.files.is_empty() {
            return Ok(());
        }

        let count = repo.files.len();
        writeln!(w, "file_index[{}]{{path,tokens,importance}}:", count)?;
        for file in &repo.files {
            let importance = if file.importance > 0.8 {
                "critical"
            } else if file.importance > 0.6 {
                "high"
            } else if file.importance > 0.3 {
                "normal"
            } else {
                "low"
            };
            writeln!(
                w,
                "  {},{},{}",
                escape_toon(&file.relative_path),
                file.token_count.get(self.token_model),
                importance
            )?;
        }
        writeln!(w)
    }

    fn stream_files<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "files:")?;

        for file in &repo.files {
            if let Some(ref content) = file.content {
                let lang = file.language.as_deref().unwrap_or("?");
                writeln!(
                    w,
                    "- {}|{}|{}:",
                    escape_toon(&file.relative_path),
                    lang,
                    file.token_count.get(self.token_model)
                )?;

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
                                    writeln!(w, "  {}:{}", line_num, rest)?;
                                } else {
                                    // Fallback for malformed lines
                                    writeln!(w, "  {}", line)?;
                                }
                            } else {
                                writeln!(w, "  {}", line)?;
                            }
                        }
                    } else {
                        // No embedded line numbers - use sequential (uncompressed content)
                        for (i, line) in content.lines().enumerate() {
                            writeln!(w, "  {}:{}", i + 1, line)?;
                        }
                    }
                } else {
                    for line in content.lines() {
                        writeln!(w, "  {}", line)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for ToonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for ToonFormatter {
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
        "toon"
    }
}

impl StreamingFormatter for ToonFormatter {
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()> {
        writeln!(writer, "# Infiniloom Repository Context (TOON format)")?;
        writeln!(writer, "# Format: https://github.com/toon-format/toon")?;
        writeln!(writer)?;

        self.stream_metadata(writer, repo)?;
        self.stream_languages(writer, repo)?;
        self.stream_directory_structure(writer, repo)?;
        self.stream_dependencies(writer, repo)?;
        self.stream_repomap(writer, map)?;
        if self.show_file_index {
            self.stream_file_index(writer, repo)?;
        }
        self.stream_files(writer, repo)?;
        Ok(())
    }

    fn format_repo_to_writer<W: Write>(&self, repo: &Repository, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "# Infiniloom Repository Context (TOON format)")?;
        writeln!(writer)?;

        self.stream_metadata(writer, repo)?;
        self.stream_languages(writer, repo)?;
        self.stream_directory_structure(writer, repo)?;
        self.stream_dependencies(writer, repo)?;
        if self.show_file_index {
            self.stream_file_index(writer, repo)?;
        }
        self.stream_files(writer, repo)?;
        Ok(())
    }
}

/// Escape special characters for TOON format (v3.0 spec compliant)
///
/// Per TOON v3.0 spec, strings MUST be quoted if:
/// - String is empty
/// - Contains leading/trailing whitespace
/// - Matches reserved literals (true, false, null)
/// - Matches numeric patterns
/// - Contains structural characters (colon, comma, pipe)
/// - Contains control characters (newline, carriage return, tab)
/// - Contains quote or backslash character
///
/// Only five escape sequences are valid per spec:
/// - \\ (backslash)
/// - \" (quote)
/// - \n (newline)
/// - \r (carriage return)
/// - \t (tab)
fn escape_toon(s: &str) -> String {
    // Check if quoting is needed per TOON v3.0 spec
    let needs_quotes = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s == "true"
        || s == "false"
        || s == "null"
        || s.parse::<f64>().is_ok()
        || s.contains(':')  // structural: key-value separator
        || s.contains(',')  // structural: default delimiter
        || s.contains('|')  // structural: alternate delimiter
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
        || s.contains('"')
        || s.contains('\\'); // backslash needs escaping

    if needs_quotes {
        // Only the five escapes allowed by TOON v3.0 spec
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        format!("\"{}\"", escaped)
    } else {
        s.to_owned()
    }
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
                directory_structure: Some("main.py\n".to_string()),
                external_dependencies: vec!["requests".to_string(), "numpy".to_string()],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_toon_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = ToonFormatter::new();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("# Infiniloom Repository Context"));
        assert!(output.contains("metadata:"));
        assert!(output.contains("name: test"));
        assert!(output.contains("files: 1"));
        assert!(output.contains("languages[1]{name,files,percentage}:"));
        assert!(output.contains("directory_structure: |"));
        // Files are formatted as "- path|lang|tokens:"
        assert!(output.contains("main.py|python|50:"));
    }

    #[test]
    fn test_toon_escaping() {
        // Plain strings - no quoting needed
        assert_eq!(escape_toon("hello"), "hello");
        assert_eq!(escape_toon("hello_world"), "hello_world");
        assert_eq!(escape_toon("CamelCase"), "CamelCase");

        // Empty string - must be quoted
        assert_eq!(escape_toon(""), "\"\"");

        // Reserved literals - must be quoted
        assert_eq!(escape_toon("true"), "\"true\"");
        assert_eq!(escape_toon("false"), "\"false\"");
        assert_eq!(escape_toon("null"), "\"null\"");

        // Numeric patterns - must be quoted
        assert_eq!(escape_toon("123"), "\"123\"");
        assert_eq!(escape_toon("3.14"), "\"3.14\"");
        assert_eq!(escape_toon("-42"), "\"-42\"");
        assert_eq!(escape_toon("0"), "\"0\"");

        // Structural characters - must be quoted (TOON v3.0)
        assert_eq!(escape_toon("a,b"), "\"a,b\""); // comma (default delimiter)
        assert_eq!(escape_toon("a|b"), "\"a|b\""); // pipe (alt delimiter)
        assert_eq!(escape_toon("key:value"), "\"key:value\""); // colon (key-value sep)

        // Control characters - must be quoted and escaped
        assert_eq!(escape_toon("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(escape_toon("tab\there"), "\"tab\\there\"");
        assert_eq!(escape_toon("cr\rhere"), "\"cr\\rhere\"");

        // Quote character - must be quoted and escaped
        assert_eq!(escape_toon("say \"hello\""), "\"say \\\"hello\\\"\"");

        // Backslash - must be quoted and escaped
        assert_eq!(escape_toon("path\\to\\file"), "\"path\\\\to\\\\file\"");

        // Leading/trailing whitespace - must be quoted
        assert_eq!(escape_toon(" leading"), "\" leading\"");
        assert_eq!(escape_toon("trailing "), "\"trailing \"");
        assert_eq!(escape_toon(" both "), "\" both \"");
    }

    #[test]
    fn test_toon_tabular_format() {
        let repo = create_test_repo();
        let formatter = ToonFormatter::new();
        let output = formatter.format_repo(&repo);

        // Should use tabular format for languages and file_index
        assert!(output.contains("languages[1]{name,files,percentage}:"));
        assert!(output.contains("file_index[1]{path,tokens,importance}:"));
    }
}
