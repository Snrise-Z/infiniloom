//! Output formatters for different LLM models
//!
//! This module provides formatters that can output in different formats:
//! - XML (Claude-optimized)
//! - Markdown (GPT-optimized)
//! - TOON (Token-Oriented Object Notation - most compact)
//! - JSON, YAML, Plain text
//!
//! Formatters support both in-memory (`format()`) and streaming (`format_to_writer()`)
//! modes. Use streaming for large repositories to reduce memory usage.
//!
//! The [`escaping`] submodule provides text escaping utilities for XML, YAML,
//! and other formats.

pub mod escaping;
mod markdown;
mod toon;
mod xml;

use crate::repomap::RepoMap;
use crate::types::{Repository, TokenizerModel};
use std::io::{self, Write};

pub use markdown::MarkdownFormatter;
pub use toon::ToonFormatter;
pub use xml::XmlFormatter;

// Re-export StreamingFormatter from this module (defined locally above)

/// Output format type
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Claude-optimized XML
    #[default]
    Xml,
    /// GPT-optimized Markdown
    Markdown,
    /// JSON (generic)
    Json,
    /// YAML (Gemini)
    Yaml,
    /// TOON (Token-Oriented Object Notation) - most token-efficient
    Toon,
    /// Plain text (simple, no formatting)
    Plain,
}

/// Output formatter trait for in-memory formatting.
///
/// Use `format()` for formatting with a repository map (includes symbol rankings),
/// or `format_repo()` for formatting just the repository files.
///
/// For large repositories, prefer the `StreamingFormatter` trait to reduce memory usage.
pub trait Formatter {
    /// Format repository with map to string.
    ///
    /// The output includes both the repository files and the symbol rankings
    /// from the repository map.
    #[must_use]
    fn format(&self, repo: &Repository, map: &RepoMap) -> String;

    /// Format repository only to string (without map metadata).
    #[must_use]
    fn format_repo(&self, repo: &Repository) -> String;

    /// Get format name.
    fn name(&self) -> &'static str;
}

/// Streaming formatter trait for low-memory output
///
/// Implement this trait to enable streaming output directly to files,
/// stdout, or network sockets without building intermediate strings.
pub trait StreamingFormatter {
    /// Stream repository with map to writer
    ///
    /// # Example
    /// ```ignore
    /// use std::io::BufWriter;
    /// use std::fs::File;
    ///
    /// let file = File::create("output.xml")?;
    /// let mut writer = BufWriter::new(file);
    /// formatter.format_to_writer(&repo, &map, &mut writer)?;
    /// ```
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()>;

    /// Stream repository only to writer
    fn format_repo_to_writer<W: Write>(&self, repo: &Repository, writer: &mut W) -> io::Result<()>;
}

/// Output formatter factory
pub struct OutputFormatter;

impl OutputFormatter {
    /// Create Claude-optimized XML formatter
    pub fn claude() -> XmlFormatter {
        XmlFormatter::new(true).with_model(TokenizerModel::Claude)
    }

    /// Create GPT-optimized Markdown formatter
    pub fn gpt() -> MarkdownFormatter {
        MarkdownFormatter::new().with_model(TokenizerModel::Claude)
    }

    /// Create JSON formatter
    pub fn json() -> JsonFormatter {
        JsonFormatter
    }

    /// Create YAML formatter (Gemini)
    pub fn gemini() -> YamlFormatter {
        YamlFormatter::new(TokenizerModel::Gemini)
    }

    /// Create formatter by format type
    pub fn by_format(format: OutputFormat) -> Box<dyn Formatter> {
        Self::by_format_with_options(format, true)
    }

    /// Create formatter by format type with line numbers option
    pub fn by_format_with_options(format: OutputFormat, line_numbers: bool) -> Box<dyn Formatter> {
        Self::by_format_with_all_options(format, line_numbers, true)
    }

    /// Create formatter by format type with all options
    pub fn by_format_with_all_options(
        format: OutputFormat,
        line_numbers: bool,
        show_file_index: bool,
    ) -> Box<dyn Formatter> {
        let model = Self::default_model_for_format(format);
        Self::by_format_with_all_options_and_model(format, line_numbers, show_file_index, model)
    }

    /// Create formatter by format type with model override
    pub fn by_format_with_model(format: OutputFormat, model: TokenizerModel) -> Box<dyn Formatter> {
        Self::by_format_with_all_options_and_model(format, true, true, model)
    }

    /// Create formatter by format type with all options and model override
    pub fn by_format_with_all_options_and_model(
        format: OutputFormat,
        line_numbers: bool,
        show_file_index: bool,
        model: TokenizerModel,
    ) -> Box<dyn Formatter> {
        match format {
            OutputFormat::Xml => Box::new(
                XmlFormatter::new(true)
                    .with_line_numbers(line_numbers)
                    .with_file_index(show_file_index)
                    .with_model(model),
            ),
            OutputFormat::Markdown => Box::new(
                MarkdownFormatter::new()
                    .with_line_numbers(line_numbers)
                    .with_model(model),
            ),
            OutputFormat::Json => Box::new(JsonFormatter),
            OutputFormat::Yaml => Box::new(YamlFormatter::new(model)),
            OutputFormat::Toon => Box::new(
                ToonFormatter::new()
                    .with_line_numbers(line_numbers)
                    .with_file_index(show_file_index)
                    .with_model(model),
            ),
            OutputFormat::Plain => Box::new(
                PlainFormatter::new()
                    .with_line_numbers(line_numbers)
                    .with_model(model),
            ),
        }
    }

    /// Create TOON formatter (most token-efficient)
    pub fn toon() -> ToonFormatter {
        ToonFormatter::new().with_model(TokenizerModel::Claude)
    }

    fn default_model_for_format(format: OutputFormat) -> TokenizerModel {
        match format {
            OutputFormat::Yaml => TokenizerModel::Gemini,
            _ => TokenizerModel::Claude,
        }
    }
}

/// JSON formatter
pub struct JsonFormatter;

/// Output structure for JSON format with repository map
#[derive(serde::Serialize)]
struct JsonOutput<'a> {
    repository: &'a Repository,
    map: &'a RepoMap,
}

/// Output structure for JSON format without map (repo-only view)
#[derive(serde::Serialize)]
struct JsonRepoOutput<'a> {
    repository: &'a Repository,
}

impl Formatter for JsonFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        serde_json::to_string_pretty(&JsonOutput { repository: repo, map }).unwrap_or_default()
    }

    fn format_repo(&self, repo: &Repository) -> String {
        // Use consistent structure with 'repository' key wrapper for API consistency
        serde_json::to_string_pretty(&JsonRepoOutput { repository: repo }).unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "json"
    }
}

/// Plain text formatter (simple, no markup)
pub struct PlainFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Token model for counts in output
    token_model: TokenizerModel,
}

impl PlainFormatter {
    /// Create a new plain formatter
    pub fn new() -> Self {
        Self { include_line_numbers: true, token_model: TokenizerModel::Claude }
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
}

impl Default for PlainFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for PlainFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!("Repository: {}\n", repo.name));
        output.push_str(&format!(
            "Files: {} | Lines: {} | Tokens: {}\n",
            repo.metadata.total_files,
            repo.metadata.total_lines,
            repo.metadata.total_tokens.get(self.token_model)
        ));
        output.push_str(&"=".repeat(60));
        output.push('\n');
        output.push('\n');

        // Repository map summary
        output.push_str("REPOSITORY MAP\n");
        output.push_str(&"-".repeat(40));
        output.push('\n');
        output.push_str(&map.summary);
        output.push_str("\n\n");

        // Directory structure
        if let Some(structure) = &repo.metadata.directory_structure {
            output.push_str("DIRECTORY STRUCTURE\n");
            output.push_str(&"-".repeat(40));
            output.push('\n');
            output.push_str(structure);
            output.push_str("\n\n");
        }

        // Files
        output.push_str("FILES\n");
        output.push_str(&"=".repeat(60));
        output.push('\n');

        for file in &repo.files {
            output.push('\n');
            output.push_str(&format!("File: {}\n", file.relative_path));
            if let Some(lang) = &file.language {
                output.push_str(&format!("Language: {}\n", lang));
            }
            output.push_str(&format!("Tokens: {}\n", file.token_count.get(self.token_model)));
            output.push_str(&"-".repeat(40));
            output.push('\n');

            if let Some(content) = &file.content {
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        output.push_str(&format!("{:4} {}\n", i + 1, line));
                    }
                } else {
                    output.push_str(content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
            output.push_str(&"-".repeat(40));
            output.push('\n');
        }

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        let mut output = String::new();
        for file in &repo.files {
            output.push_str(&format!("=== {} ===\n", file.relative_path));
            if let Some(content) = &file.content {
                if self.include_line_numbers {
                    for (i, line) in content.lines().enumerate() {
                        output.push_str(&format!("{:4} {}\n", i + 1, line));
                    }
                } else {
                    output.push_str(content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
            output.push('\n');
        }
        output
    }

    fn name(&self) -> &'static str {
        "plain"
    }
}

/// YAML formatter (Gemini-optimized)
pub struct YamlFormatter {
    token_model: TokenizerModel,
}

impl YamlFormatter {
    /// Create a new YAML formatter with the specified token model
    pub fn new(model: TokenizerModel) -> Self {
        Self { token_model: model }
    }
}

impl Formatter for YamlFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        let mut output = String::new();

        // YAML header
        output.push_str("---\n");
        output.push_str("# Repository Context for Gemini\n");
        output.push_str("# Note: Query should be at the END of this context\n\n");

        // Metadata
        output.push_str("metadata:\n");
        output.push_str(&format!("  name: {}\n", repo.name));
        output.push_str(&format!("  files: {}\n", repo.metadata.total_files));
        output.push_str(&format!("  lines: {}\n", repo.metadata.total_lines));
        output
            .push_str(&format!("  tokens: {}\n", repo.metadata.total_tokens.get(self.token_model)));
        output.push('\n');

        // Languages
        output.push_str("languages:\n");
        for lang in &repo.metadata.languages {
            output.push_str(&format!(
                "  - name: {}\n    files: {}\n    percentage: {:.1}%\n",
                lang.language, lang.files, lang.percentage
            ));
        }
        output.push('\n');

        // Repository map
        output.push_str("repository_map:\n");
        output.push_str(&format!("  summary: |\n    {}\n", map.summary.replace('\n', "\n    ")));
        output.push_str("  key_symbols:\n");
        for sym in &map.key_symbols {
            output.push_str(&format!(
                "    - name: {}\n      type: {}\n      file: {}\n      rank: {}\n",
                sym.name, sym.kind, sym.file, sym.rank
            ));
            if let Some(ref summary) = sym.summary {
                output.push_str(&format!("      summary: {}\n", summary));
            }
        }
        output.push('\n');

        // Files
        output.push_str("files:\n");
        for file in &repo.files {
            output.push_str(&format!("  - path: {}\n", file.relative_path));
            if let Some(lang) = &file.language {
                output.push_str(&format!("    language: {}\n", lang));
            }
            output.push_str(&format!("    tokens: {}\n", file.token_count.get(self.token_model)));

            if let Some(content) = &file.content {
                output.push_str("    content: |\n");
                for line in content.lines() {
                    output.push_str(&format!("      {}\n", line));
                }
            }
        }

        // Query placeholder at end (Gemini best practice)
        output.push_str("\n# --- INSERT YOUR QUERY BELOW THIS LINE ---\n");
        output.push_str("query: |\n");
        output.push_str("  [Your question about this repository]\n");

        output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        serde_yaml::to_string(repo).unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "yaml"
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
                external_dependencies: vec!["requests".to_string()],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_json_formatter() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = OutputFormatter::json();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"files\""));
    }

    #[test]
    fn test_yaml_formatter() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = OutputFormatter::gemini();
        let output = formatter.format(&repo, &map);

        assert!(output.contains("name: test"));
        assert!(output.contains("# --- INSERT YOUR QUERY"));
    }
}
