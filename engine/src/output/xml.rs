//! Claude-optimized XML output formatter
//!
//! This formatter is designed to maximize LLM comprehension of codebases by:
//! 1. Providing an executive summary for quick understanding
//! 2. Identifying entry points and key files
//! 3. Showing architecture and dependencies
//! 4. Prioritizing files by importance for code tasks
//!
//! Supports both in-memory (`format()`) and streaming (`format_to_writer()`) modes.

use crate::output::{Formatter, StreamingFormatter};
use crate::repomap::RepoMap;
use crate::types::{Repository, TokenizerModel};
use std::io::{self, Write};

/// XML formatter optimized for Claude
pub struct XmlFormatter {
    /// Include line numbers in code
    include_line_numbers: bool,
    /// Optimize for prompt caching
    cache_optimized: bool,
    /// Include CDATA sections for code
    use_cdata: bool,
    /// Include file index/summary section
    show_file_index: bool,
    /// Token model for counts in output
    token_model: TokenizerModel,
}

impl XmlFormatter {
    /// Create a new XML formatter
    pub fn new(cache_optimized: bool) -> Self {
        Self {
            include_line_numbers: true,
            cache_optimized,
            use_cdata: true,
            show_file_index: true,
            token_model: TokenizerModel::Claude,
        }
    }

    /// Set line numbers option
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.include_line_numbers = enabled;
        self
    }

    /// Set CDATA option
    pub fn with_cdata(mut self, enabled: bool) -> Self {
        self.use_cdata = enabled;
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
        let base = 2000;
        let files = repo.files.len() * 500;
        let content: usize = repo
            .files
            .iter()
            .filter_map(|f| f.content.as_ref())
            .map(|c| c.len())
            .sum();
        base + files + content
    }

    fn detect_project_type(&self, repo: &Repository) -> String {
        let has_cargo = repo.files.iter().any(|f| f.relative_path == "Cargo.toml");
        let has_package_json = repo.files.iter().any(|f| f.relative_path == "package.json");
        let has_pyproject = repo
            .files
            .iter()
            .any(|f| f.relative_path == "pyproject.toml" || f.relative_path == "setup.py");
        let has_go_mod = repo.files.iter().any(|f| f.relative_path == "go.mod");

        let has_routes = repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("routes") || f.relative_path.contains("api/"));
        let has_components = repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("components/") || f.relative_path.contains("views/"));

        if has_cargo {
            if repo
                .files
                .iter()
                .any(|f| f.relative_path.ends_with("lib.rs"))
            {
                "Rust Library"
            } else {
                "Rust Application"
            }
        } else if has_package_json {
            if has_components {
                "Frontend Application (JavaScript/TypeScript)"
            } else if has_routes {
                "Backend API (Node.js)"
            } else {
                "JavaScript/TypeScript Project"
            }
        } else if has_pyproject {
            if has_routes {
                "Python Web API"
            } else {
                "Python Package"
            }
        } else if has_go_mod {
            "Go Application"
        } else {
            "Software Project"
        }
        .to_owned()
    }

    fn is_entry_point(&self, path: &str) -> bool {
        let entry_patterns = [
            "main.rs",
            "main.go",
            "main.py",
            "main.ts",
            "main.js",
            "main.c",
            "main.cpp",
            "index.ts",
            "index.js",
            "index.tsx",
            "index.jsx",
            "index.py",
            "app.py",
            "app.ts",
            "app.js",
            "app.tsx",
            "app.jsx",
            "app.go",
            "server.py",
            "server.ts",
            "server.js",
            "server.go",
            "mod.rs",
            "lib.rs",
            "__main__.py",
            "__init__.py",
            "cmd/main.go",
        ];
        entry_patterns
            .iter()
            .any(|p| path.ends_with(p) || path.contains(&format!("/{}", p)))
    }

    fn get_entry_type(&self, path: &str) -> &'static str {
        if path.contains("main") {
            "main"
        } else if path.contains("index") {
            "index"
        } else if path.contains("app") {
            "app"
        } else if path.contains("server") {
            "server"
        } else if path.contains("lib") {
            "library"
        } else if path.contains("mod.rs") {
            "module"
        } else {
            "entry"
        }
    }

    fn is_config_file(&self, path: &str) -> bool {
        let config_files = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tsconfig.json",
            "webpack.config",
            "vite.config",
            "next.config",
            "Makefile",
            "CMakeLists.txt",
            "Dockerfile",
            "docker-compose",
            ".env.example",
            "config.yaml",
            "config.yml",
            "config.json",
        ];
        let filename = path.rsplit('/').next().unwrap_or(path);
        config_files.iter().any(|c| filename.contains(c)) && path.matches('/').count() <= 1
    }

    // =========================================================================
    // Streaming methods (write to impl std::io::Write)
    // =========================================================================

    fn stream_llm_instructions<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "  <llm_context_guide>")?;
        writeln!(w, "    <purpose>This is a comprehensive code context for the {} repository, optimized for AI-assisted code understanding and generation.</purpose>", escape_xml(&repo.name))?;
        writeln!(w, "    <how_to_use>")?;
        writeln!(w, "      <tip>Start with the &lt;overview&gt; section to understand the project's purpose and structure</tip>")?;
        writeln!(w, "      <tip>Check &lt;entry_points&gt; to find main application files</tip>")?;
        writeln!(
            w,
            "      <tip>Use &lt;repository_map&gt; to understand relationships between modules</tip>"
        )?;
        writeln!(
            w,
            "      <tip>Files are ordered by importance - most critical files come first</tip>"
        )?;
        writeln!(w, "    </how_to_use>")?;
        writeln!(w, "  </llm_context_guide>")
    }

    fn stream_overview<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "  <overview>")?;
        let project_type = self.detect_project_type(repo);
        writeln!(w, "    <project_type>{}</project_type>", escape_xml(&project_type))?;

        if let Some(lang) = repo.metadata.languages.iter().max_by_key(|l| l.files) {
            writeln!(w, "    <primary_language>{}</primary_language>", escape_xml(&lang.language))?;
        }
        if let Some(framework) = &repo.metadata.framework {
            writeln!(w, "    <framework>{}</framework>", escape_xml(framework))?;
        }

        writeln!(w, "    <entry_points>")?;
        let mut entry_count = 0;
        for file in &repo.files {
            if self.is_entry_point(&file.relative_path) {
                if file.relative_path.ends_with("__init__.py")
                    && file.token_count.get(self.token_model) < 50
                {
                    continue;
                }
                let entry_type = self.get_entry_type(&file.relative_path);
                writeln!(
                    w,
                    "      <entry path=\"{}\" type=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&file.relative_path),
                    entry_type,
                    file.token_count.get(self.token_model)
                )?;
                entry_count += 1;
                if entry_count >= 10 {
                    break;
                }
            }
        }
        writeln!(w, "    </entry_points>")?;

        writeln!(w, "    <config_files>")?;
        for file in &repo.files {
            if self.is_config_file(&file.relative_path) {
                writeln!(
                    w,
                    "      <config path=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&file.relative_path),
                    file.token_count.get(self.token_model)
                )?;
            }
        }
        writeln!(w, "    </config_files>")?;
        writeln!(w, "  </overview>")
    }

    fn stream_metadata<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "  <metadata>")?;
        if let Some(desc) = &repo.metadata.description {
            writeln!(w, "    <description>{}</description>", escape_xml(desc))?;
        }
        writeln!(w, "    <stats>")?;
        writeln!(w, "      <files>{}</files>", repo.metadata.total_files)?;
        writeln!(w, "      <lines>{}</lines>", repo.metadata.total_lines)?;
        writeln!(
            w,
            "      <tokens model=\"claude\">{}</tokens>",
            repo.metadata.total_tokens.get(self.token_model)
        )?;
        writeln!(w, "    </stats>")?;

        if !repo.metadata.languages.is_empty() {
            writeln!(w, "    <languages>")?;
            for lang in &repo.metadata.languages {
                writeln!(
                    w,
                    "      <language name=\"{}\" files=\"{}\" percentage=\"{:.1}\"/>",
                    escape_xml(&lang.language),
                    lang.files,
                    lang.percentage
                )?;
            }
            writeln!(w, "    </languages>")?;
        }

        if let Some(ref structure) = repo.metadata.directory_structure {
            writeln!(w, "    <directory_structure><![CDATA[")?;
            write!(w, "{}", structure)?;
            writeln!(w, "]]></directory_structure>")?;
        }

        if !repo.metadata.external_dependencies.is_empty() {
            writeln!(
                w,
                "    <dependencies count=\"{}\">",
                repo.metadata.external_dependencies.len()
            )?;
            for dep in &repo.metadata.external_dependencies {
                writeln!(w, "      <dependency name=\"{}\"/>", escape_xml(dep))?;
            }
            writeln!(w, "    </dependencies>")?;
        }

        // Add explicit file extension counts for accurate file counting queries
        let mut ext_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for file in &repo.files {
            if let Some(ext) = std::path::Path::new(&file.relative_path).extension() {
                *ext_counts
                    .entry(ext.to_string_lossy().to_string())
                    .or_insert(0) += 1;
            }
        }
        if !ext_counts.is_empty() {
            writeln!(w, "    <file_extensions>")?;
            let mut sorted_exts: Vec<_> = ext_counts.iter().collect();
            sorted_exts.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            for (ext, count) in sorted_exts {
                writeln!(
                    w,
                    "      <extension name=\".{}\" count=\"{}\"/>",
                    escape_xml(ext),
                    count
                )?;
            }
            writeln!(w, "    </file_extensions>")?;
        }

        writeln!(w, "  </metadata>")
    }

    fn stream_git_history<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        if let Some(ref git_history) = repo.metadata.git_history {
            writeln!(w, "  <git_history>")?;
            if !git_history.commits.is_empty() {
                writeln!(w, "    <recent_commits count=\"{}\">", git_history.commits.len())?;
                for commit in &git_history.commits {
                    writeln!(
                        w,
                        "      <commit hash=\"{}\" author=\"{}\" date=\"{}\">",
                        escape_xml(&commit.short_hash),
                        escape_xml(&commit.author),
                        escape_xml(&commit.date)
                    )?;
                    writeln!(w, "        <message><![CDATA[{}]]></message>", commit.message)?;
                    writeln!(w, "      </commit>")?;
                }
                writeln!(w, "    </recent_commits>")?;
            }
            if !git_history.changed_files.is_empty() {
                writeln!(
                    w,
                    "    <uncommitted_changes count=\"{}\">",
                    git_history.changed_files.len()
                )?;
                for file in &git_history.changed_files {
                    if let Some(diff) = &file.diff_content {
                        writeln!(
                            w,
                            "      <change path=\"{}\" status=\"{}\">",
                            escape_xml(&file.path),
                            escape_xml(&file.status)
                        )?;
                        writeln!(w, "        <diff><![CDATA[{}]]></diff>", diff)?;
                        writeln!(w, "      </change>")?;
                    } else {
                        writeln!(
                            w,
                            "      <change path=\"{}\" status=\"{}\"/>",
                            escape_xml(&file.path),
                            escape_xml(&file.status)
                        )?;
                    }
                }
                writeln!(w, "    </uncommitted_changes>")?;
            }
            writeln!(w, "  </git_history>")?;
        }
        Ok(())
    }

    fn stream_repomap<W: Write>(&self, w: &mut W, map: &RepoMap) -> io::Result<()> {
        writeln!(w, "  <repository_map token_budget=\"{}\">", map.token_count)?;
        writeln!(w, "    <summary><![CDATA[{}]]></summary>", map.summary)?;

        writeln!(w, "    <key_symbols>")?;
        for symbol in &map.key_symbols {
            writeln!(
                w,
                "      <symbol name=\"{}\" type=\"{}\" file=\"{}\" line=\"{}\" rank=\"{}\">",
                escape_xml(&symbol.name),
                escape_xml(&symbol.kind),
                escape_xml(&symbol.file),
                symbol.line,
                symbol.rank
            )?;
            if let Some(sig) = &symbol.signature {
                writeln!(w, "        <signature><![CDATA[{}]]></signature>", sig)?;
            }
            if let Some(summary) = &symbol.summary {
                writeln!(w, "        <summary><![CDATA[{}]]></summary>", summary)?;
            }
            writeln!(w, "      </symbol>")?;
        }
        writeln!(w, "    </key_symbols>")?;

        if !map.module_graph.nodes.is_empty() {
            writeln!(w, "    <modules>")?;
            for module in &map.module_graph.nodes {
                writeln!(
                    w,
                    "      <module name=\"{}\" files=\"{}\" tokens=\"{}\"/>",
                    escape_xml(&module.name),
                    module.files,
                    module.tokens
                )?;
            }
            writeln!(w, "    </modules>")?;
        }
        writeln!(w, "  </repository_map>")
    }

    fn stream_file_index<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "  <file_index entries=\"{}\">", repo.files.len())?;
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
                "    <file path=\"{}\" tokens=\"{}\" importance=\"{}\"/>",
                escape_xml(&file.relative_path),
                file.token_count.get(self.token_model),
                importance
            )?;
        }
        writeln!(w, "  </file_index>")
    }

    fn stream_files<W: Write>(&self, w: &mut W, repo: &Repository) -> io::Result<()> {
        writeln!(w, "  <files>")?;
        for file in &repo.files {
            if let Some(content) = &file.content {
                writeln!(
                    w,
                    "    <file path=\"{}\" language=\"{}\" tokens=\"{}\">",
                    escape_xml(&file.relative_path),
                    file.language.as_deref().unwrap_or("unknown"),
                    file.token_count.get(self.token_model)
                )?;

                if self.include_line_numbers {
                    writeln!(w, "      <content line_numbers=\"original\"><![CDATA[")?;
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
                                    writeln!(w, "{:4} | {}", line_num, rest)?;
                                } else {
                                    // Fallback for malformed lines
                                    writeln!(w, "     | {}", line)?;
                                }
                            } else {
                                writeln!(w, "     | {}", line)?;
                            }
                        }
                    } else {
                        // No embedded line numbers - use sequential (uncompressed content)
                        for (i, line) in content.lines().enumerate() {
                            writeln!(w, "{:4} | {}", i + 1, line)?;
                        }
                    }
                    writeln!(w, "]]></content>")?;
                } else if self.use_cdata {
                    writeln!(w, "      <content><![CDATA[{}]]></content>", content)?;
                } else {
                    writeln!(w, "      <content>{}</content>", escape_xml(content))?;
                }
                writeln!(w, "    </file>")?;
            }
        }
        writeln!(w, "  </files>")
    }
}

impl Formatter for XmlFormatter {
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
        "xml"
    }
}

impl StreamingFormatter for XmlFormatter {
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()> {
        writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(writer, r#"<repository name="{}" version="1.0.0">"#, escape_xml(&repo.name))?;

        self.stream_llm_instructions(writer, repo)?;

        if self.cache_optimized {
            writeln!(writer, "  <!-- CACHEABLE_PREFIX_START -->")?;
        }

        self.stream_overview(writer, repo)?;
        self.stream_metadata(writer, repo)?;
        self.stream_git_history(writer, repo)?;
        self.stream_repomap(writer, map)?;

        if self.show_file_index {
            self.stream_file_index(writer, repo)?;
        }

        if self.cache_optimized {
            writeln!(writer, "  <!-- CACHEABLE_PREFIX_END -->")?;
            writeln!(writer, "  <!-- DYNAMIC_CONTENT_START -->")?;
        }

        self.stream_files(writer, repo)?;

        if self.cache_optimized {
            writeln!(writer, "  <!-- DYNAMIC_CONTENT_END -->")?;
        }

        writeln!(writer, "</repository>")?;
        Ok(())
    }

    fn format_repo_to_writer<W: Write>(&self, repo: &Repository, writer: &mut W) -> io::Result<()> {
        writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(writer, r#"<repository name="{}">"#, escape_xml(&repo.name))?;

        self.stream_metadata(writer, repo)?;
        if self.show_file_index {
            self.stream_file_index(writer, repo)?;
        }
        self.stream_files(writer, repo)?;

        writeln!(writer, "</repository>")?;
        Ok(())
    }
}

/// Escape XML special characters (single-pass for performance)
fn escape_xml(s: &str) -> String {
    // Pre-allocate with some extra capacity for escapes
    let mut result = String::with_capacity(s.len() + s.len() / 10);

    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }

    result
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
    fn test_xml_output() {
        let repo = create_test_repo();
        let map = RepoMapGenerator::new(1000).generate(&repo);

        let formatter = XmlFormatter::new(true);
        let output = formatter.format(&repo, &map);

        assert!(output.contains("<?xml version=\"1.0\""));
        assert!(output.contains("<repository name=\"test\""));
        assert!(output.contains("CACHEABLE_PREFIX_START"));
        assert!(output.contains("<file path=\"main.py\""));
    }

    #[test]
    fn test_xml_escaping() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
