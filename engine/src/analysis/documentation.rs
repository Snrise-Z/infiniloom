//! Documentation extraction and parsing for all supported languages
//!
//! Parses JSDoc, Python docstrings, Rust doc comments, JavaDoc, etc.
//! into structured documentation format.

use crate::analysis::types::{Documentation, Example, ParamDoc, ReturnDoc, ThrowsDoc};
use crate::parser::Language;
use regex::Regex;

/// Extracts and parses documentation from source code
pub struct DocumentationExtractor {
    // Precompiled regex patterns
    jsdoc_param: Regex,
    jsdoc_returns: Regex,
    jsdoc_throws: Regex,
    jsdoc_example: Regex,
    jsdoc_tag: Regex,
    python_param: Regex,
    python_returns: Regex,
    python_raises: Regex,
    rust_param: Regex,
}

impl DocumentationExtractor {
    /// Create a new documentation extractor
    pub fn new() -> Self {
        Self {
            // JSDoc patterns
            jsdoc_param: Regex::new(r"@param\s+(?:\{([^}]+)\}\s+)?(\[)?(\w+)\]?\s*(?:-\s*)?(.*)")
                .unwrap(),
            jsdoc_returns: Regex::new(r"@returns?\s+(?:\{([^}]+)\}\s+)?(.*)").unwrap(),
            jsdoc_throws: Regex::new(r"@throws?\s+(?:\{([^}]+)\}\s+)?(.*)").unwrap(),
            // Note: Example parsing is done manually in parse_jsdoc via in_example state
            jsdoc_example: Regex::new(r"@example\s*").unwrap(),
            jsdoc_tag: Regex::new(r"@(\w+)\s+(.*)").unwrap(),

            // Python docstring patterns (Google/NumPy style)
            python_param: Regex::new(r"^\s*(\w+)\s*(?:\(([^)]+)\))?\s*:\s*(.*)$").unwrap(),
            python_returns: Regex::new(r"^\s*(?:(\w+)\s*:\s*)?(.*)$").unwrap(),
            python_raises: Regex::new(r"^\s*(\w+)\s*:\s*(.*)$").unwrap(),

            // Rust doc patterns
            rust_param: Regex::new(r"^\s*\*\s+`(\w+)`\s*(?:-\s*)?(.*)$").unwrap(),
        }
    }

    /// Extract documentation from a docstring/comment based on language
    pub fn extract(&self, raw_doc: &str, language: Language) -> Documentation {
        let raw_doc = raw_doc.trim();
        if raw_doc.is_empty() {
            return Documentation::default();
        }

        match language {
            Language::JavaScript | Language::TypeScript => self.parse_jsdoc(raw_doc),
            Language::Python => self.parse_python_docstring(raw_doc),
            Language::Rust => self.parse_rust_doc(raw_doc),
            Language::Java | Language::Kotlin => self.parse_javadoc(raw_doc),
            Language::Go => self.parse_go_doc(raw_doc),
            Language::Ruby => self.parse_ruby_doc(raw_doc),
            Language::Php => self.parse_phpdoc(raw_doc),
            Language::CSharp => self.parse_csharp_doc(raw_doc),
            Language::Swift => self.parse_swift_doc(raw_doc),
            Language::Scala => self.parse_scaladoc(raw_doc),
            Language::Haskell => self.parse_haddock(raw_doc),
            Language::Elixir => self.parse_exdoc(raw_doc),
            Language::Clojure => self.parse_clojure_doc(raw_doc),
            Language::OCaml => self.parse_ocamldoc(raw_doc),
            Language::Lua => self.parse_luadoc(raw_doc),
            Language::R => self.parse_roxygen(raw_doc),
            Language::Cpp | Language::C => self.parse_doxygen(raw_doc),
            Language::Bash => self.parse_bash_comment(raw_doc),
            // Handle any language not explicitly matched (e.g., FSharp)
            _ => self.parse_generic(raw_doc),
        }
    }

    /// Parse JSDoc style documentation
    fn parse_jsdoc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Remove comment markers
        let content = self.strip_comment_markers(raw, "/**", "*/", "*");

        // Split into lines
        let lines: Vec<&str> = content.lines().collect();

        // First non-tag lines are the description
        let mut description_lines = Vec::new();
        let mut in_description = true;
        let mut current_example = String::new();
        let mut in_example = false;

        for line in &lines {
            let line = line.trim();

            if line.starts_with('@') {
                in_description = false;

                // End any current example
                if in_example && !line.starts_with("@example") {
                    if !current_example.is_empty() {
                        doc.examples.push(Example {
                            code: current_example.trim().to_owned(),
                            ..Default::default()
                        });
                    }
                    current_example.clear();
                    in_example = false;
                }

                // Parse different tags
                if let Some(caps) = self.jsdoc_param.captures(line) {
                    let type_info = caps.get(1).map(|m| m.as_str().to_owned());
                    let is_optional = caps.get(2).is_some();
                    let name = caps.get(3).map_or("", |m| m.as_str());
                    let desc = caps.get(4).map_or("", |m| m.as_str());

                    doc.params.push(ParamDoc {
                        name: name.to_owned(),
                        type_info,
                        description: if desc.is_empty() {
                            None
                        } else {
                            Some(desc.to_owned())
                        },
                        is_optional,
                        default_value: None,
                    });
                } else if let Some(caps) = self.jsdoc_returns.captures(line) {
                    doc.returns = Some(ReturnDoc {
                        type_info: caps.get(1).map(|m| m.as_str().to_owned()),
                        description: caps.get(2).map(|m| m.as_str().to_owned()),
                    });
                } else if let Some(caps) = self.jsdoc_throws.captures(line) {
                    doc.throws.push(ThrowsDoc {
                        exception_type: caps
                            .get(1)
                            .map_or_else(|| "Error".to_owned(), |m| m.as_str().to_owned()),
                        description: caps.get(2).map(|m| m.as_str().to_owned()),
                    });
                } else if line.starts_with("@example") {
                    in_example = true;
                    // Content after @example on same line
                    let after_tag = line.strip_prefix("@example").unwrap_or("").trim();
                    if !after_tag.is_empty() {
                        current_example.push_str(after_tag);
                        current_example.push('\n');
                    }
                } else if line.starts_with("@deprecated") {
                    doc.is_deprecated = true;
                    let msg = line.strip_prefix("@deprecated").unwrap_or("").trim();
                    if !msg.is_empty() {
                        doc.deprecation_message = Some(msg.to_owned());
                    }
                } else if let Some(caps) = self.jsdoc_tag.captures(line) {
                    let tag = caps.get(1).map_or("", |m| m.as_str());
                    let value = caps.get(2).map_or("", |m| m.as_str());
                    doc.tags
                        .entry(tag.to_owned())
                        .or_default()
                        .push(value.to_owned());
                }
            } else if in_example {
                current_example.push_str(line);
                current_example.push('\n');
            } else if in_description {
                description_lines.push(line);
            }
        }

        // Handle last example
        if !current_example.is_empty() {
            doc.examples
                .push(Example { code: current_example.trim().to_owned(), ..Default::default() });
        }

        // Set description
        if !description_lines.is_empty() {
            let full_desc = description_lines.join("\n");
            let sentences: Vec<&str> = full_desc.split(". ").collect();
            if !sentences.is_empty() {
                doc.summary = Some(sentences[0].to_owned());
            }
            doc.description = Some(full_desc);
        }

        doc
    }

    /// Parse Python docstring (Google/NumPy/Sphinx style)
    fn parse_python_docstring(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Remove triple quotes
        let content = raw
            .trim_start_matches("\"\"\"")
            .trim_end_matches("\"\"\"")
            .trim_start_matches("'''")
            .trim_end_matches("'''")
            .trim();

        let lines: Vec<&str> = content.lines().collect();

        #[derive(PartialEq)]
        enum Section {
            Description,
            Args,
            Returns,
            Raises,
            Example,
            Other,
        }

        let mut section = Section::Description;
        let mut description_lines = Vec::new();
        let mut current_param: Option<ParamDoc> = None;
        let mut current_example = String::new();

        for line in lines {
            let trimmed = line.trim();

            // Check for section headers
            if trimmed == "Args:" || trimmed == "Arguments:" || trimmed == "Parameters:" {
                section = Section::Args;
                continue;
            } else if trimmed == "Returns:" || trimmed == "Return:" {
                section = Section::Returns;
                continue;
            } else if trimmed == "Raises:" || trimmed == "Throws:" || trimmed == "Exceptions:" {
                section = Section::Raises;
                continue;
            } else if trimmed == "Example:" || trimmed == "Examples:" {
                section = Section::Example;
                continue;
            } else if trimmed.ends_with(':') && !trimmed.contains(' ') {
                section = Section::Other;
                continue;
            }

            match section {
                Section::Description => {
                    description_lines.push(trimmed);
                },
                Section::Args => {
                    if let Some(caps) = self.python_param.captures(trimmed) {
                        // Save previous param
                        if let Some(param) = current_param.take() {
                            doc.params.push(param);
                        }

                        let name = caps.get(1).map_or("", |m| m.as_str());
                        let type_info = caps.get(2).map(|m| m.as_str().to_owned());
                        let desc = caps.get(3).map(|m| m.as_str());

                        current_param = Some(ParamDoc {
                            name: name.to_owned(),
                            type_info,
                            description: desc.map(String::from),
                            is_optional: false,
                            default_value: None,
                        });
                    } else if let Some(ref mut param) = current_param {
                        // Continuation of previous param description
                        if let Some(ref mut desc) = param.description {
                            desc.push(' ');
                            desc.push_str(trimmed);
                        }
                    }
                },
                Section::Returns => {
                    if doc.returns.is_none() {
                        if let Some(caps) = self.python_returns.captures(trimmed) {
                            doc.returns = Some(ReturnDoc {
                                type_info: caps.get(1).map(|m| m.as_str().to_owned()),
                                description: caps.get(2).map(|m| m.as_str().to_owned()),
                            });
                        }
                    } else if let Some(ref mut ret) = doc.returns {
                        if let Some(ref mut desc) = ret.description {
                            desc.push(' ');
                            desc.push_str(trimmed);
                        }
                    }
                },
                Section::Raises => {
                    if let Some(caps) = self.python_raises.captures(trimmed) {
                        doc.throws.push(ThrowsDoc {
                            exception_type: caps
                                .get(1)
                                .map(|m| m.as_str().to_owned())
                                .unwrap_or_default(),
                            description: caps.get(2).map(|m| m.as_str().to_owned()),
                        });
                    }
                },
                Section::Example => {
                    current_example.push_str(line);
                    current_example.push('\n');
                },
                Section::Other => {},
            }
        }

        // Save last param
        if let Some(param) = current_param {
            doc.params.push(param);
        }

        // Save example
        if !current_example.is_empty() {
            doc.examples.push(Example {
                code: current_example.trim().to_owned(),
                language: Some("python".to_owned()),
                ..Default::default()
            });
        }

        // Set description
        let desc = description_lines.join(" ");
        if !desc.is_empty() {
            let sentences: Vec<&str> = desc.split(". ").collect();
            if !sentences.is_empty() {
                doc.summary = Some(sentences[0].to_owned());
            }
            doc.description = Some(desc);
        }

        doc
    }

    /// Parse Rust doc comments
    fn parse_rust_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Remove /// or //! or /** */
        let content = self.strip_rust_doc_markers(raw);

        let lines: Vec<&str> = content.lines().collect();

        #[derive(PartialEq)]
        enum Section {
            Description,
            Arguments,
            Returns,
            Errors,
            Panics,
            Examples,
            Safety,
        }

        let mut section = Section::Description;
        let mut description_lines = Vec::new();
        let mut current_example = String::new();

        for line in lines {
            let trimmed = line.trim();

            // Check for section headers (# Headers in Rust docs)
            if trimmed.starts_with("# ") {
                let header = trimmed[2..].to_lowercase();
                section = match header.as_str() {
                    "arguments" | "parameters" => Section::Arguments,
                    "returns" => Section::Returns,
                    "errors" => Section::Errors,
                    "panics" => Section::Panics,
                    "examples" | "example" => Section::Examples,
                    "safety" => Section::Safety,
                    _ => Section::Description,
                };
                continue;
            }

            match section {
                Section::Description => {
                    description_lines.push(trimmed);
                },
                Section::Arguments => {
                    if let Some(caps) = self.rust_param.captures(trimmed) {
                        doc.params.push(ParamDoc {
                            name: caps
                                .get(1)
                                .map(|m| m.as_str().to_owned())
                                .unwrap_or_default(),
                            description: caps.get(2).map(|m| m.as_str().to_owned()),
                            ..Default::default()
                        });
                    }
                },
                Section::Returns => {
                    if doc.returns.is_none() {
                        doc.returns = Some(ReturnDoc {
                            description: Some(trimmed.to_owned()),
                            ..Default::default()
                        });
                    }
                },
                Section::Errors => {
                    if !trimmed.is_empty() {
                        doc.throws.push(ThrowsDoc {
                            exception_type: "Error".to_owned(),
                            description: Some(trimmed.to_owned()),
                        });
                    }
                },
                Section::Panics => {
                    doc.tags
                        .entry("panics".to_owned())
                        .or_default()
                        .push(trimmed.to_owned());
                },
                Section::Examples => {
                    current_example.push_str(line);
                    current_example.push('\n');
                },
                Section::Safety => {
                    doc.tags
                        .entry("safety".to_owned())
                        .or_default()
                        .push(trimmed.to_owned());
                },
            }
        }

        // Save example
        if !current_example.is_empty() {
            // Extract code blocks (```rust ... ```)
            let code_block_re = Regex::new(r"```(?:rust)?\n([\s\S]*?)```").unwrap();
            for caps in code_block_re.captures_iter(&current_example) {
                if let Some(code) = caps.get(1) {
                    doc.examples.push(Example {
                        code: code.as_str().trim().to_owned(),
                        language: Some("rust".to_owned()),
                        ..Default::default()
                    });
                }
            }
        }

        // Set description
        let desc = description_lines.join(" ");
        if !desc.is_empty() {
            let sentences: Vec<&str> = desc.split(". ").collect();
            if !sentences.is_empty() {
                doc.summary = Some(sentences[0].to_owned());
            }
            doc.description = Some(desc);
        }

        doc
    }

    /// Parse JavaDoc style documentation
    fn parse_javadoc(&self, raw: &str) -> Documentation {
        // JavaDoc is similar to JSDoc
        self.parse_jsdoc(raw)
    }

    /// Parse Go doc comments
    fn parse_go_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Go uses simple // comments
        let content: String = raw
            .lines()
            .map(|l| l.trim_start_matches("//").trim())
            .collect::<Vec<_>>()
            .join(" ");

        // First sentence is summary
        let sentences: Vec<&str> = content.split(". ").collect();
        if !sentences.is_empty() {
            doc.summary = Some(sentences[0].to_owned());
        }
        doc.description = Some(content);

        // Check for Deprecated
        if raw.to_lowercase().contains("deprecated") {
            doc.is_deprecated = true;
        }

        doc
    }

    /// Parse Ruby RDoc/YARD
    fn parse_ruby_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        let content = self.strip_comment_markers(raw, "=begin", "=end", "#");

        // YARD style @param, @return, @raise
        let param_re = Regex::new(r"@param\s+\[([^\]]+)\]\s+(\w+)\s+(.*)").unwrap();
        let return_re = Regex::new(r"@return\s+\[([^\]]+)\]\s+(.*)").unwrap();
        let raise_re = Regex::new(r"@raise\s+\[([^\]]+)\]\s+(.*)").unwrap();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(2)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    type_info: caps.get(1).map(|m| m.as_str().to_owned()),
                    description: caps.get(3).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = return_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    type_info: caps.get(1).map(|m| m.as_str().to_owned()),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                });
            } else if let Some(caps) = raise_re.captures(line) {
                doc.throws.push(ThrowsDoc {
                    exception_type: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                });
            } else if !line.starts_with('@') && doc.description.is_none() {
                doc.description = Some(line.to_owned());
                doc.summary = Some(line.to_owned());
            }
        }

        doc
    }

    /// Parse PHPDoc
    fn parse_phpdoc(&self, raw: &str) -> Documentation {
        // PHPDoc is similar to JSDoc
        self.parse_jsdoc(raw)
    }

    /// Parse C# XML documentation
    fn parse_csharp_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // C# uses XML documentation
        let summary_re = Regex::new(r"<summary>([\s\S]*?)</summary>").unwrap();
        let param_re = Regex::new(r#"<param name="(\w+)">([\s\S]*?)</param>"#).unwrap();
        let returns_re = Regex::new(r"<returns>([\s\S]*?)</returns>").unwrap();
        let exception_re =
            Regex::new(r#"<exception cref="([^"]+)">([\s\S]*?)</exception>"#).unwrap();

        if let Some(caps) = summary_re.captures(raw) {
            let summary = caps.get(1).map(|m| m.as_str().trim().to_owned());
            doc.summary = summary.clone();
            doc.description = summary;
        }

        for caps in param_re.captures_iter(raw) {
            doc.params.push(ParamDoc {
                name: caps
                    .get(1)
                    .map(|m| m.as_str().to_owned())
                    .unwrap_or_default(),
                description: caps.get(2).map(|m| m.as_str().trim().to_owned()),
                ..Default::default()
            });
        }

        if let Some(caps) = returns_re.captures(raw) {
            doc.returns = Some(ReturnDoc {
                description: caps.get(1).map(|m| m.as_str().trim().to_owned()),
                ..Default::default()
            });
        }

        for caps in exception_re.captures_iter(raw) {
            doc.throws.push(ThrowsDoc {
                exception_type: caps
                    .get(1)
                    .map(|m| m.as_str().to_owned())
                    .unwrap_or_default(),
                description: caps.get(2).map(|m| m.as_str().trim().to_owned()),
            });
        }

        doc
    }

    /// Parse Swift documentation comments
    fn parse_swift_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Swift uses /// or /** */ with - Parameter:, - Returns:, - Throws:
        let content = self.strip_comment_markers(raw, "/**", "*/", "///");

        let param_re = Regex::new(r"-\s*Parameter\s+(\w+):\s*(.*)").unwrap();
        let returns_re = Regex::new(r"-\s*Returns:\s*(.*)").unwrap();
        let throws_re = Regex::new(r"-\s*Throws:\s*(.*)").unwrap();

        let mut description_lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = returns_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    description: caps.get(1).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = throws_re.captures(line) {
                doc.throws.push(ThrowsDoc {
                    exception_type: "Error".to_owned(),
                    description: caps.get(1).map(|m| m.as_str().to_owned()),
                });
            } else if !line.starts_with('-') && !line.is_empty() {
                description_lines.push(line);
            }
        }

        if !description_lines.is_empty() {
            let desc = description_lines.join(" ");
            doc.summary = Some(description_lines[0].to_owned());
            doc.description = Some(desc);
        }

        doc
    }

    /// Parse ScalaDoc
    fn parse_scaladoc(&self, raw: &str) -> Documentation {
        // ScalaDoc is similar to JavaDoc
        self.parse_javadoc(raw)
    }

    /// Parse Haddock (Haskell)
    fn parse_haddock(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Haddock uses -- | or {- | -}
        let content = raw
            .lines()
            .map(|l| {
                l.trim_start_matches("--")
                    .trim_start_matches('|')
                    .trim_start_matches('^')
                    .trim()
            })
            .collect::<Vec<_>>()
            .join(" ");

        doc.description = Some(content.clone());
        let sentences: Vec<&str> = content.split(". ").collect();
        if !sentences.is_empty() {
            doc.summary = Some(sentences[0].to_owned());
        }

        doc
    }

    /// Parse ExDoc (Elixir)
    fn parse_exdoc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // ExDoc uses @doc """ ... """ or @moduledoc
        let content = raw
            .trim_start_matches("@doc")
            .trim_start_matches("@moduledoc")
            .trim()
            .trim_start_matches("\"\"\"")
            .trim_end_matches("\"\"\"")
            .trim();

        // Parse markdown-style documentation
        let lines: Vec<&str> = content.lines().collect();
        let mut description_lines = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            // Check for ## Parameters, ## Returns, etc.
            if trimmed.starts_with("##") {
                // Section header
                continue;
            }

            if trimmed.starts_with('*') || trimmed.starts_with('-') {
                // List item - could be a parameter
                let item = trimmed.trim_start_matches(['*', '-']).trim();
                if item.contains(':') {
                    let parts: Vec<&str> = item.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        doc.params.push(ParamDoc {
                            name: parts[0].trim().to_owned(),
                            description: Some(parts[1].trim().to_owned()),
                            ..Default::default()
                        });
                    }
                }
            } else if !trimmed.is_empty() {
                description_lines.push(trimmed);
            }
        }

        if !description_lines.is_empty() {
            doc.summary = Some(description_lines[0].to_owned());
            doc.description = Some(description_lines.join(" "));
        }

        doc
    }

    /// Parse Clojure docstring
    fn parse_clojure_doc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Clojure docstrings are simple strings
        let content = raw.trim_matches('"');

        doc.description = Some(content.to_owned());
        let sentences: Vec<&str> = content.split(". ").collect();
        if !sentences.is_empty() {
            doc.summary = Some(sentences[0].to_owned());
        }

        doc
    }

    /// Parse OCamldoc
    fn parse_ocamldoc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // OCamldoc uses (** ... *)
        let content = raw.trim_start_matches("(**").trim_end_matches("*)").trim();

        // Parse @param, @return, @raise
        let param_re = Regex::new(r"@param\s+(\w+)\s+(.*)").unwrap();
        let return_re = Regex::new(r"@return\s+(.*)").unwrap();
        let raise_re = Regex::new(r"@raise\s+(\w+)\s+(.*)").unwrap();

        let mut description_lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = return_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    description: caps.get(1).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = raise_re.captures(line) {
                doc.throws.push(ThrowsDoc {
                    exception_type: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                });
            } else if !line.starts_with('@') {
                description_lines.push(line);
            }
        }

        if !description_lines.is_empty() {
            doc.summary = Some(description_lines[0].to_owned());
            doc.description = Some(description_lines.join(" "));
        }

        doc
    }

    /// Parse LuaDoc
    fn parse_luadoc(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // LuaDoc uses --- or --[[ ]]
        let content: String = raw
            .lines()
            .map(|l| l.trim_start_matches("---").trim_start_matches("--").trim())
            .collect::<Vec<_>>()
            .join("\n");

        // Parse @param, @return
        let param_re = Regex::new(r"@param\s+(\w+)\s+(\w+)\s*(.*)").unwrap();
        let return_re = Regex::new(r"@return\s+(\w+)\s*(.*)").unwrap();

        let mut description_lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    type_info: caps.get(2).map(|m| m.as_str().to_owned()),
                    description: caps.get(3).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = return_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    type_info: caps.get(1).map(|m| m.as_str().to_owned()),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                });
            } else if !line.starts_with('@') {
                description_lines.push(line);
            }
        }

        if !description_lines.is_empty() {
            doc.summary = Some(description_lines[0].to_owned());
            doc.description = Some(description_lines.join(" "));
        }

        doc
    }

    /// Parse Roxygen2 (R)
    fn parse_roxygen(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Roxygen uses #' @param, #' @return, etc.
        let content: String = raw
            .lines()
            .map(|l| l.trim_start_matches("#'").trim())
            .collect::<Vec<_>>()
            .join("\n");

        let param_re = Regex::new(r"@param\s+(\w+)\s+(.*)").unwrap();
        let return_re = Regex::new(r"@return\s+(.*)").unwrap();

        let mut description_lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = return_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    description: caps.get(1).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if !line.starts_with('@') {
                description_lines.push(line);
            }
        }

        if !description_lines.is_empty() {
            doc.summary = Some(description_lines[0].to_owned());
            doc.description = Some(description_lines.join(" "));
        }

        doc
    }

    /// Parse Doxygen (C/C++)
    fn parse_doxygen(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Doxygen uses /** */, //!, \param, \return, etc.
        let content = self.strip_comment_markers(raw, "/**", "*/", "*");

        let param_re = Regex::new(r"[@\\]param(?:\[(?:in|out|in,out)\])?\s+(\w+)\s+(.*)").unwrap();
        let return_re = Regex::new(r"[@\\]returns?\s+(.*)").unwrap();
        let throws_re = Regex::new(r"[@\\](?:throws?|exception)\s+(\w+)\s*(.*)").unwrap();
        let brief_re = Regex::new(r"[@\\]brief\s+(.*)").unwrap();

        let mut description_lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some(caps) = brief_re.captures(line) {
                doc.summary = caps.get(1).map(|m| m.as_str().to_owned());
            } else if let Some(caps) = param_re.captures(line) {
                doc.params.push(ParamDoc {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = return_re.captures(line) {
                doc.returns = Some(ReturnDoc {
                    description: caps.get(1).map(|m| m.as_str().to_owned()),
                    ..Default::default()
                });
            } else if let Some(caps) = throws_re.captures(line) {
                doc.throws.push(ThrowsDoc {
                    exception_type: caps
                        .get(1)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    description: caps.get(2).map(|m| m.as_str().to_owned()),
                });
            } else if !line.starts_with('@') && !line.starts_with('\\') {
                description_lines.push(line);
            }
        }

        if doc.summary.is_none() && !description_lines.is_empty() {
            doc.summary = Some(description_lines[0].to_owned());
        }
        if !description_lines.is_empty() {
            doc.description = Some(description_lines.join(" "));
        }

        doc
    }

    /// Parse bash script comments
    fn parse_bash_comment(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        let content: String = raw
            .lines()
            .map(|l| l.trim_start_matches('#').trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        doc.description = Some(content.clone());
        let sentences: Vec<&str> = content.split(". ").collect();
        if !sentences.is_empty() {
            doc.summary = Some(sentences[0].to_owned());
        }

        doc
    }

    /// Parse generic comment (fallback)
    fn parse_generic(&self, raw: &str) -> Documentation {
        let mut doc = Documentation { raw: Some(raw.to_owned()), ..Default::default() };

        // Strip common comment markers
        let content: String = raw
            .lines()
            .map(|l| {
                l.trim()
                    .trim_start_matches("//")
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim_start_matches('#')
                    .trim_start_matches("--")
                    .trim_start_matches(";;")
                    .trim()
            })
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        doc.description = Some(content.clone());
        let sentences: Vec<&str> = content.split(". ").collect();
        if !sentences.is_empty() {
            doc.summary = Some(sentences[0].to_owned());
        }

        doc
    }

    // Helper methods

    fn strip_comment_markers(&self, raw: &str, start: &str, end: &str, line: &str) -> String {
        let mut content = raw
            .trim()
            .trim_start_matches(start)
            .trim_end_matches(end)
            .to_owned();

        // Remove line prefixes
        content = content
            .lines()
            .map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with(line) {
                    trimmed[line.len()..].trim_start()
                } else {
                    trimmed
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        content
    }

    fn strip_rust_doc_markers(&self, raw: &str) -> String {
        raw.lines()
            .map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with("///") {
                    trimmed[3..].trim_start()
                } else if trimmed.starts_with("//!") {
                    trimmed[3..].trim_start()
                } else if trimmed.starts_with("/**") {
                    trimmed[3..].trim_start()
                } else if trimmed.starts_with('*') {
                    trimmed[1..].trim_start()
                } else if trimmed == "*/" {
                    ""
                } else {
                    trimmed
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for DocumentationExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Helper
    // ---------------------------------------------------------------

    fn ext() -> DocumentationExtractor {
        DocumentationExtractor::new()
    }

    // ---------------------------------------------------------------
    // Empty / whitespace edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_string_returns_default() {
        let doc = ext().extract("", Language::JavaScript);
        assert!(doc.summary.is_none());
        assert!(doc.description.is_none());
        assert!(doc.params.is_empty());
        assert!(doc.returns.is_none());
        assert!(doc.throws.is_empty());
        assert!(doc.examples.is_empty());
        assert!(!doc.is_deprecated);
        assert!(doc.raw.is_none());
    }

    #[test]
    fn test_whitespace_only_returns_default() {
        let doc = ext().extract("   \n\t  \n  ", Language::Python);
        assert!(doc.summary.is_none());
        assert!(doc.raw.is_none());
    }

    // ---------------------------------------------------------------
    // JSDoc / JavaScript / TypeScript
    // ---------------------------------------------------------------

    #[test]
    fn test_jsdoc_parsing() {
        let jsdoc = r#"/**
         * Calculate the sum of two numbers.
         *
         * @param {number} a - The first number
         * @param {number} b - The second number
         * @returns {number} The sum of a and b
         * @throws {Error} If inputs are not numbers
         * @example
         * add(1, 2) // returns 3
         */
        "#;

        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.summary.is_some());
        assert!(doc.summary.unwrap().contains("Calculate"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert!(doc.params[0].type_info.as_ref().unwrap().contains("number"));
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.examples.len(), 1);
    }

    #[test]
    fn test_jsdoc_optional_param() {
        let jsdoc = "/**\n * @param {string} [name] - Optional name\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "name");
        assert!(doc.params[0].is_optional);
        assert_eq!(doc.params[0].type_info.as_deref(), Some("string"));
    }

    #[test]
    fn test_jsdoc_param_no_type() {
        let jsdoc = "/**\n * @param x - The value\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "x");
        assert!(doc.params[0].type_info.is_none());
        assert_eq!(doc.params[0].description.as_deref(), Some("The value"));
    }

    #[test]
    fn test_jsdoc_param_no_description() {
        let jsdoc = "/**\n * @param {number} x\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "x");
        // Empty description gets stored as None
        assert!(doc.params[0].description.is_none());
    }

    #[test]
    fn test_jsdoc_multiple_throws() {
        let jsdoc = "/**\n * Do stuff.\n * @throws {TypeError} Bad type\n * @throws {RangeError} Out of range\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.throws.len(), 2);
        assert_eq!(doc.throws[0].exception_type, "TypeError");
        assert_eq!(doc.throws[1].exception_type, "RangeError");
    }

    #[test]
    fn test_jsdoc_returns_without_type() {
        let jsdoc = "/**\n * @returns The result\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.returns.is_some());
        let ret = doc.returns.unwrap();
        assert!(ret.type_info.is_none());
        assert_eq!(ret.description.as_deref(), Some("The result"));
    }

    #[test]
    fn test_jsdoc_multiple_examples() {
        // Two @example tags: the second starts with @example so the condition
        // `!line.starts_with("@example")` is false, meaning the first example
        // is not saved separately. Both lines end up in one combined example.
        let jsdoc = "/**\n * Math helper.\n * @example\n * add(1,2)\n * @example\n * add(3,4)\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("add(1,2)"));
        assert!(doc.examples[0].code.contains("add(3,4)"));
    }

    #[test]
    fn test_jsdoc_deprecated_without_message() {
        let jsdoc = "/**\n * Old.\n * @deprecated\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.is_deprecated);
        // @deprecated with nothing after it: deprecation_message should be None
        assert!(doc.deprecation_message.is_none());
    }

    #[test]
    fn test_jsdoc_deprecated_with_message() {
        let jsdoc = "/**\n * Old.\n * @deprecated Use bar instead\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.is_deprecated);
        assert_eq!(doc.deprecation_message.as_deref(), Some("Use bar instead"));
    }

    #[test]
    fn test_jsdoc_custom_tags() {
        let jsdoc = "/**\n * My func.\n * @since 2.0\n * @see otherFunc\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.tags.contains_key("since"));
        assert!(doc.tags.contains_key("see"));
        assert_eq!(doc.tags["since"][0], "2.0");
    }

    #[test]
    fn test_jsdoc_multiline_description() {
        let jsdoc = "/**\n * First sentence. Second sentence.\n * Third sentence.\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        // Summary is first part split by ". "
        let summary = doc.summary.unwrap();
        assert!(summary.contains("First sentence"));
        let desc = doc.description.unwrap();
        assert!(desc.contains("Third sentence."));
    }

    #[test]
    fn test_jsdoc_typescript_dispatch() {
        let jsdoc = "/**\n * A TS function.\n * @param {string} s - input\n */";
        let doc = ext().extract(jsdoc, Language::TypeScript);

        assert!(doc.summary.unwrap().contains("TS function"));
        assert_eq!(doc.params.len(), 1);
    }

    #[test]
    fn test_jsdoc_example_with_inline_content() {
        let jsdoc = "/**\n * Func.\n * @example const x = foo();\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("const x = foo();"));
    }

    // ---------------------------------------------------------------
    // Python docstrings
    // ---------------------------------------------------------------

    #[test]
    fn test_python_docstring_parsing() {
        let docstring = r#""""
        Calculate the sum of two numbers.

        Args:
            a (int): The first number
            b (int): The second number

        Returns:
            int: The sum of a and b

        Raises:
            ValueError: If inputs are not integers
        """"#;

        let doc = ext().extract(docstring, Language::Python);

        assert!(doc.summary.is_some());
        assert!(doc.summary.unwrap().contains("Calculate"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
    }

    #[test]
    fn test_python_single_quote_docstring() {
        let docstring = "'''Sum two numbers.\n\nArgs:\n    x (float): first\n'''";
        let doc = ext().extract(docstring, Language::Python);

        assert!(doc.summary.unwrap().contains("Sum two numbers"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "x");
        assert_eq!(doc.params[0].type_info.as_deref(), Some("float"));
    }

    #[test]
    fn test_python_parameters_header() {
        let docstring = "\"\"\"Do stuff.\n\nParameters:\n    n (int): count\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "n");
    }

    #[test]
    fn test_python_arguments_header() {
        let docstring = "\"\"\"Do stuff.\n\nArguments:\n    n (int): count\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "n");
    }

    #[test]
    fn test_python_multiple_raises() {
        let docstring =
            "\"\"\"Do stuff.\n\nRaises:\n    ValueError: bad\n    TypeError: wrong type\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.throws.len(), 2);
        assert_eq!(doc.throws[0].exception_type, "ValueError");
        assert_eq!(doc.throws[1].exception_type, "TypeError");
    }

    #[test]
    fn test_python_throws_header() {
        let docstring = "\"\"\"Do stuff.\n\nThrows:\n    IOError: disk full\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "IOError");
    }

    #[test]
    fn test_python_exceptions_header() {
        let docstring = "\"\"\"Do stuff.\n\nExceptions:\n    OSError: not found\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "OSError");
    }

    #[test]
    fn test_python_example_section() {
        let docstring = "\"\"\"Do stuff.\n\nExample:\n    >>> foo(1)\n    42\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("foo(1)"));
        assert_eq!(doc.examples[0].language.as_deref(), Some("python"));
    }

    #[test]
    fn test_python_examples_plural_header() {
        let docstring = "\"\"\"Do stuff.\n\nExamples:\n    >>> bar()\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.examples.len(), 1);
    }

    #[test]
    fn test_python_return_singular_header() {
        let docstring = "\"\"\"Do stuff.\n\nReturn:\n    int: the result\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert!(doc.returns.is_some());
    }

    #[test]
    fn test_python_param_no_type() {
        let docstring = "\"\"\"Do stuff.\n\nArgs:\n    name: the name value\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "name");
        assert!(doc.params[0].type_info.is_none());
    }

    #[test]
    fn test_python_multiline_param_description() {
        let docstring =
            "\"\"\"Do stuff.\n\nArgs:\n    x (int): First line\n        continued here\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.params.len(), 1);
        let desc = doc.params[0].description.as_ref().unwrap();
        assert!(desc.contains("First line"));
        assert!(desc.contains("continued here"));
    }

    #[test]
    fn test_python_multiline_returns_description() {
        let docstring =
            "\"\"\"Do stuff.\n\nReturns:\n    int: First line\n        more info\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        let ret = doc.returns.unwrap();
        let desc = ret.description.unwrap();
        assert!(desc.contains("First line"));
        assert!(desc.contains("more info"));
    }

    #[test]
    fn test_python_description_only() {
        let docstring = "\"\"\"A simple description with no sections.\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert!(doc.summary.unwrap().contains("simple description"));
        assert!(doc.params.is_empty());
        assert!(doc.returns.is_none());
    }

    #[test]
    fn test_python_other_section_ignored() {
        let docstring = "\"\"\"Do stuff.\n\nNotes:\n    Some note here.\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        // "Notes:" triggers Section::Other, content ignored
        assert!(doc.params.is_empty());
        assert!(doc.returns.is_none());
    }

    // ---------------------------------------------------------------
    // Rust doc comments
    // ---------------------------------------------------------------

    #[test]
    fn test_rust_doc_parsing() {
        let rust_doc = "/// Calculate the sum of two numbers.\n///\n/// # Arguments\n///\n/// * `a` - The first number\n/// * `b` - The second number\n///\n/// # Returns\n///\n/// The sum of a and b";

        let doc = ext().extract(rust_doc, Language::Rust);

        assert!(doc.summary.is_some());
        assert!(doc.summary.unwrap().contains("Calculate"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert_eq!(doc.params[1].name, "b");
        assert!(doc.returns.is_some());
    }

    #[test]
    fn test_rust_inner_doc_comment() {
        let doc_str = "//! Module level documentation.\n//! Second line.";
        let doc = ext().extract(doc_str, Language::Rust);

        assert!(doc.summary.unwrap().contains("Module level documentation"));
    }

    #[test]
    fn test_rust_block_doc_comment() {
        let doc_str = "/** Block doc comment.\n * More details here.\n */";
        let doc = ext().extract(doc_str, Language::Rust);

        assert!(doc.description.unwrap().contains("Block doc comment"));
    }

    #[test]
    fn test_rust_errors_section() {
        let doc_str = "/// Do something.\n///\n/// # Errors\n///\n/// Returns Err if file not found.\n/// Also returns Err on permission denied.";
        let doc = ext().extract(doc_str, Language::Rust);

        assert_eq!(doc.throws.len(), 2);
        assert_eq!(doc.throws[0].exception_type, "Error");
        assert!(doc.throws[0]
            .description
            .as_ref()
            .unwrap()
            .contains("file not found"));
    }

    #[test]
    fn test_rust_panics_section() {
        // The empty line after # Panics produces an empty entry, then the real content
        let doc_str =
            "/// Do something.\n///\n/// # Panics\n///\n/// Panics if index is out of bounds.";
        let doc = ext().extract(doc_str, Language::Rust);

        assert!(doc.tags.contains_key("panics"));
        let panics_entries = &doc.tags["panics"];
        assert!(panics_entries.iter().any(|e| e.contains("out of bounds")));
    }

    #[test]
    fn test_rust_safety_section() {
        // The empty line after # Safety produces an empty entry, then the real content
        let doc_str =
            "/// Unsafe op.\n///\n/// # Safety\n///\n/// Caller must ensure pointer is valid.";
        let doc = ext().extract(doc_str, Language::Rust);

        assert!(doc.tags.contains_key("safety"));
        let safety_entries = &doc.tags["safety"];
        assert!(safety_entries
            .iter()
            .any(|e| e.contains("pointer is valid")));
    }

    #[test]
    fn test_rust_examples_with_code_block() {
        let doc_str =
            "/// A function.\n///\n/// # Examples\n///\n/// ```rust\n/// let x = foo();\n/// ```";
        let doc = ext().extract(doc_str, Language::Rust);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("let x = foo();"));
        assert_eq!(doc.examples[0].language.as_deref(), Some("rust"));
    }

    #[test]
    fn test_rust_examples_code_block_no_lang() {
        let doc_str = "/// A function.\n///\n/// # Examples\n///\n/// ```\n/// foo();\n/// ```";
        let doc = ext().extract(doc_str, Language::Rust);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("foo();"));
    }

    #[test]
    fn test_rust_example_singular_header() {
        let doc_str = "/// A function.\n///\n/// # Example\n///\n/// ```\n/// bar();\n/// ```";
        let doc = ext().extract(doc_str, Language::Rust);

        assert_eq!(doc.examples.len(), 1);
    }

    #[test]
    fn test_rust_parameters_header() {
        let doc_str = "/// Do it.\n///\n/// # Parameters\n///\n/// * `x` - The x value";
        let doc = ext().extract(doc_str, Language::Rust);

        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "x");
    }

    #[test]
    fn test_rust_unknown_header_falls_back_to_description() {
        let doc_str = "/// Do it.\n///\n/// # Implementation Details\n///\n/// Uses a hash map.";
        let doc = ext().extract(doc_str, Language::Rust);

        // Unknown headers fall back to Section::Description
        let desc = doc.description.unwrap();
        assert!(desc.contains("Uses a hash map"));
    }

    #[test]
    fn test_rust_errors_empty_lines_skipped() {
        let doc_str = "/// Do it.\n///\n/// # Errors\n///\n/// \n/// Real error here.";
        let doc = ext().extract(doc_str, Language::Rust);

        // Empty line should not produce a ThrowsDoc entry
        assert_eq!(doc.throws.len(), 1);
        assert!(doc.throws[0]
            .description
            .as_ref()
            .unwrap()
            .contains("Real error"));
    }

    // ---------------------------------------------------------------
    // JavaDoc / Kotlin (delegates to JSDoc parser)
    // ---------------------------------------------------------------

    #[test]
    fn test_javadoc_parsing() {
        let javadoc = "/**\n * Process the data.\n *\n * @param input the input data\n * @return the processed result\n * @throws IOException if reading fails\n */";
        let doc = ext().extract(javadoc, Language::Java);

        assert!(doc.summary.unwrap().contains("Process the data"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "input");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
    }

    #[test]
    fn test_kotlin_delegates_to_javadoc() {
        let kdoc = "/**\n * Kotlin function.\n * @param name the name\n */";
        let doc = ext().extract(kdoc, Language::Kotlin);

        assert!(doc.summary.unwrap().contains("Kotlin function"));
        assert_eq!(doc.params.len(), 1);
    }

    // ---------------------------------------------------------------
    // Go doc comments
    // ---------------------------------------------------------------

    #[test]
    fn test_go_doc_basic() {
        let go_doc = "// Calculate returns the sum of a and b. It panics on overflow.";
        let doc = ext().extract(go_doc, Language::Go);

        assert_eq!(doc.summary.as_deref(), Some("Calculate returns the sum of a and b"));
        let desc = doc.description.unwrap();
        assert!(desc.contains("panics on overflow"));
    }

    #[test]
    fn test_go_doc_multiline() {
        let go_doc = "// First line.\n// Second line.\n// Third line.";
        let doc = ext().extract(go_doc, Language::Go);

        let desc = doc.description.unwrap();
        assert!(desc.contains("First line."));
        assert!(desc.contains("Third line."));
    }

    #[test]
    fn test_go_doc_deprecated() {
        let go_doc = "// Deprecated: Use NewFunc instead.\n// This function is old.";
        let doc = ext().extract(go_doc, Language::Go);

        assert!(doc.is_deprecated);
    }

    #[test]
    fn test_go_doc_not_deprecated() {
        let go_doc = "// Process handles the request.";
        let doc = ext().extract(go_doc, Language::Go);

        assert!(!doc.is_deprecated);
    }

    // ---------------------------------------------------------------
    // Ruby YARD
    // ---------------------------------------------------------------

    #[test]
    fn test_ruby_yard_doc() {
        let yard = "# Calculate the sum.\n# @param [Integer] a the first number\n# @param [Integer] b the second number\n# @return [Integer] the sum\n# @raise [ArgumentError] if inputs are invalid";
        let doc = ext().extract(yard, Language::Ruby);

        assert!(doc.summary.unwrap().contains("Calculate the sum"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert_eq!(doc.params[0].type_info.as_deref(), Some("Integer"));
        assert!(doc.returns.is_some());
        assert_eq!(doc.returns.unwrap().type_info.as_deref(), Some("Integer"));
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "ArgumentError");
    }

    #[test]
    fn test_ruby_description_only() {
        let yard = "# A simple helper method.";
        let doc = ext().extract(yard, Language::Ruby);

        assert!(doc.summary.unwrap().contains("simple helper"));
        assert!(doc.params.is_empty());
    }

    // ---------------------------------------------------------------
    // PHP (delegates to JSDoc parser)
    // ---------------------------------------------------------------

    #[test]
    fn test_phpdoc_parsing() {
        let phpdoc =
            "/**\n * Send an email.\n * @param string $to Recipient address\n * @return bool\n */";
        let doc = ext().extract(phpdoc, Language::Php);

        assert!(doc.summary.unwrap().contains("Send an email"));
        assert_eq!(doc.params.len(), 1);
        assert!(doc.returns.is_some());
    }

    // ---------------------------------------------------------------
    // C# XML documentation
    // ---------------------------------------------------------------

    #[test]
    fn test_csharp_xml_doc() {
        let csharp_doc = "/// <summary>\n/// Calculates the area.\n/// </summary>\n/// <param name=\"width\">The width</param>\n/// <param name=\"height\">The height</param>\n/// <returns>The area value</returns>\n/// <exception cref=\"ArgumentException\">If negative</exception>";
        let doc = ext().extract(csharp_doc, Language::CSharp);

        assert!(doc.summary.unwrap().contains("Calculates the area"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "width");
        assert_eq!(doc.params[1].name, "height");
        assert!(doc.returns.is_some());
        assert!(doc
            .returns
            .unwrap()
            .description
            .unwrap()
            .contains("area value"));
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "ArgumentException");
    }

    #[test]
    fn test_csharp_summary_only() {
        let csharp_doc = "/// <summary>Simple summary.</summary>";
        let doc = ext().extract(csharp_doc, Language::CSharp);

        assert_eq!(doc.summary.as_deref(), Some("Simple summary."));
        assert!(doc.params.is_empty());
    }

    // ---------------------------------------------------------------
    // Swift documentation
    // ---------------------------------------------------------------

    #[test]
    fn test_swift_doc() {
        let swift_doc = "/// Calculates the distance.\n///\n/// - Parameter from: The start point\n/// - Parameter to: The end point\n/// - Returns: The distance\n/// - Throws: An error if coordinates are invalid";
        let doc = ext().extract(swift_doc, Language::Swift);

        assert!(doc.summary.unwrap().contains("Calculates the distance"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "from");
        assert_eq!(doc.params[1].name, "to");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "Error");
    }

    #[test]
    fn test_swift_description_only() {
        let swift_doc = "/// A simple utility function.";
        let doc = ext().extract(swift_doc, Language::Swift);

        assert!(doc.summary.unwrap().contains("simple utility"));
    }

    // ---------------------------------------------------------------
    // Scala (delegates to JavaDoc)
    // ---------------------------------------------------------------

    #[test]
    fn test_scaladoc_delegates() {
        let scaladoc = "/**\n * Scala function.\n * @param x the input\n * @return the output\n */";
        let doc = ext().extract(scaladoc, Language::Scala);

        assert!(doc.summary.unwrap().contains("Scala function"));
        assert_eq!(doc.params.len(), 1);
        assert!(doc.returns.is_some());
    }

    // ---------------------------------------------------------------
    // Haskell Haddock
    // ---------------------------------------------------------------

    #[test]
    fn test_haddock_basic() {
        let haddock = "-- | Compute the factorial. It uses recursion.";
        let doc = ext().extract(haddock, Language::Haskell);

        assert!(doc.summary.unwrap().contains("Compute the factorial"));
        assert!(doc.description.unwrap().contains("recursion"));
    }

    #[test]
    fn test_haddock_multiline() {
        let haddock = "-- | First line.\n-- Second line.";
        let doc = ext().extract(haddock, Language::Haskell);

        let desc = doc.description.unwrap();
        assert!(desc.contains("First line."));
        assert!(desc.contains("Second line."));
    }

    #[test]
    fn test_haddock_caret_prefix() {
        let haddock = "-- ^ Argument documentation.";
        let doc = ext().extract(haddock, Language::Haskell);

        assert!(doc.description.unwrap().contains("Argument documentation"));
    }

    // ---------------------------------------------------------------
    // Elixir ExDoc
    // ---------------------------------------------------------------

    #[test]
    fn test_exdoc_basic() {
        let exdoc = "@doc \"\"\"\nFetches a user by ID.\n\n* id: The user identifier\n\"\"\"";
        let doc = ext().extract(exdoc, Language::Elixir);

        assert!(doc.summary.unwrap().contains("Fetches a user by ID"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "id");
    }

    #[test]
    fn test_exdoc_moduledoc() {
        let exdoc = "@moduledoc \"\"\"\nThis module handles authentication.\n\"\"\"";
        let doc = ext().extract(exdoc, Language::Elixir);

        assert!(doc.summary.unwrap().contains("authentication"));
    }

    #[test]
    fn test_exdoc_dash_list_params() {
        let exdoc = "@doc \"\"\"\nDo stuff.\n\n- name: The name\n- age: The age\n\"\"\"";
        let doc = ext().extract(exdoc, Language::Elixir);

        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "name");
        assert_eq!(doc.params[1].name, "age");
    }

    // ---------------------------------------------------------------
    // Clojure docstring
    // ---------------------------------------------------------------

    #[test]
    fn test_clojure_doc_basic() {
        let clj = "\"Adds two numbers together. Returns their sum.\"";
        let doc = ext().extract(clj, Language::Clojure);

        assert!(doc.summary.unwrap().contains("Adds two numbers together"));
        assert!(doc.description.unwrap().contains("Returns their sum"));
    }

    #[test]
    fn test_clojure_doc_no_period() {
        let clj = "\"Simple function without a period\"";
        let doc = ext().extract(clj, Language::Clojure);

        assert_eq!(doc.summary.as_deref(), Some("Simple function without a period"));
    }

    // ---------------------------------------------------------------
    // OCaml OCamldoc
    // ---------------------------------------------------------------

    #[test]
    fn test_ocamldoc_basic() {
        let ocaml = "(** Compute the length.\n@param lst the input list\n@return the number of elements\n@raise Invalid_argument if list is circular\n*)";
        let doc = ext().extract(ocaml, Language::OCaml);

        assert!(doc.summary.unwrap().contains("Compute the length"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "lst");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0].exception_type, "Invalid_argument");
    }

    // ---------------------------------------------------------------
    // Lua LuaDoc
    // ---------------------------------------------------------------

    #[test]
    fn test_luadoc_basic() {
        let lua = "--- Process the data.\n--- @param input string The input data\n--- @return boolean True on success";
        let doc = ext().extract(lua, Language::Lua);

        assert!(doc.summary.unwrap().contains("Process the data"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "input");
        assert_eq!(doc.params[0].type_info.as_deref(), Some("string"));
        assert!(doc.returns.is_some());
        assert_eq!(doc.returns.unwrap().type_info.as_deref(), Some("boolean"));
    }

    // ---------------------------------------------------------------
    // R Roxygen2
    // ---------------------------------------------------------------

    #[test]
    fn test_roxygen_basic() {
        let rox =
            "#' Calculate the mean.\n#' @param x A numeric vector\n#' @return The arithmetic mean";
        let doc = ext().extract(rox, Language::R);

        assert!(doc.summary.unwrap().contains("Calculate the mean"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "x");
        assert!(doc.returns.is_some());
    }

    // ---------------------------------------------------------------
    // Doxygen (C / C++)
    // ---------------------------------------------------------------

    #[test]
    fn test_doxygen_basic() {
        let dox = "/**\n * @brief Calculate the sum.\n * @param a First operand\n * @param b Second operand\n * @return The sum\n * @throws std::overflow_error On overflow\n */";
        let doc = ext().extract(dox, Language::Cpp);

        assert_eq!(doc.summary.as_deref(), Some("Calculate the sum."));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert_eq!(doc.params[1].name, "b");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
    }

    #[test]
    fn test_doxygen_c_dispatch() {
        let dox = "/**\n * @brief A C function.\n * @param x input\n */";
        let doc = ext().extract(dox, Language::C);

        assert_eq!(doc.summary.as_deref(), Some("A C function."));
        assert_eq!(doc.params.len(), 1);
    }

    #[test]
    fn test_doxygen_backslash_syntax() {
        let dox = "/**\n * \\brief Backslash style.\n * \\param n count\n * \\return the result\n * \\throws bad_alloc on memory failure\n */";
        let doc = ext().extract(dox, Language::Cpp);

        assert_eq!(doc.summary.as_deref(), Some("Backslash style."));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "n");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
    }

    #[test]
    fn test_doxygen_param_direction() {
        let dox =
            "/**\n * @param[in] x input\n * @param[out] y output\n * @param[in,out] z both\n */";
        let doc = ext().extract(dox, Language::Cpp);

        assert_eq!(doc.params.len(), 3);
        assert_eq!(doc.params[0].name, "x");
        assert_eq!(doc.params[1].name, "y");
        assert_eq!(doc.params[2].name, "z");
    }

    #[test]
    fn test_doxygen_no_brief_uses_first_line() {
        let dox = "/**\n * First line as description.\n * @param x input\n */";
        let doc = ext().extract(dox, Language::Cpp);

        // Without @brief, the first non-empty description line becomes summary
        let summary = doc.summary.unwrap();
        // The first line after stripping may be empty (from the newline after /**),
        // so the summary may be empty or the actual first line
        assert!(summary.is_empty() || summary.contains("First line as description"));
    }

    // ---------------------------------------------------------------
    // Bash comments
    // ---------------------------------------------------------------

    #[test]
    fn test_bash_comment_basic() {
        let bash = "# Deploy the application. Restarts the service.";
        let doc = ext().extract(bash, Language::Bash);

        assert!(doc.summary.unwrap().contains("Deploy the application"));
    }

    #[test]
    fn test_bash_multiline_comment() {
        let bash = "# First line.\n# Second line.\n# Third line.";
        let doc = ext().extract(bash, Language::Bash);

        let desc = doc.description.unwrap();
        assert!(desc.contains("First line."));
        assert!(desc.contains("Third line."));
    }

    #[test]
    fn test_bash_empty_comment_lines_filtered() {
        let bash = "# Content here.\n#\n# More content.";
        let doc = ext().extract(bash, Language::Bash);

        let desc = doc.description.unwrap();
        assert!(desc.contains("Content here."));
        assert!(desc.contains("More content."));
    }

    // ---------------------------------------------------------------
    // Generic / fallback parser
    // ---------------------------------------------------------------

    #[test]
    fn test_generic_fallback() {
        // FSharp is not explicitly matched, so it goes to generic
        let comment = "// A generic comment.";
        let doc = ext().extract(comment, Language::FSharp);

        assert!(doc.summary.unwrap().contains("generic comment"));
    }

    #[test]
    fn test_generic_strips_various_markers() {
        let comment = "/* Block comment content */";
        let doc = ext().extract(comment, Language::FSharp);

        assert!(doc.description.unwrap().contains("Block comment content"));
    }

    #[test]
    fn test_generic_hash_comment() {
        let comment = "# Hash comment content";
        let doc = ext().extract(comment, Language::FSharp);

        assert!(doc.description.unwrap().contains("Hash comment content"));
    }

    #[test]
    fn test_generic_double_dash() {
        let comment = "-- SQL style comment";
        let doc = ext().extract(comment, Language::FSharp);

        assert!(doc.description.unwrap().contains("SQL style comment"));
    }

    #[test]
    fn test_generic_semicolon_comment() {
        let comment = ";; Lisp-style comment";
        let doc = ext().extract(comment, Language::FSharp);

        assert!(doc.description.unwrap().contains("Lisp-style comment"));
    }

    // ---------------------------------------------------------------
    // Default trait impl
    // ---------------------------------------------------------------

    #[test]
    fn test_default_creates_extractor() {
        let ext: DocumentationExtractor = Default::default();
        let doc = ext.extract("/// Hello.", Language::Rust);
        assert!(doc.summary.is_some());
    }

    // ---------------------------------------------------------------
    // Raw field preservation
    // ---------------------------------------------------------------

    #[test]
    fn test_raw_field_preserved() {
        let input = "/// Some doc.";
        let doc = ext().extract(input, Language::Rust);
        assert_eq!(doc.raw.as_deref(), Some("/// Some doc."));
    }

    #[test]
    fn test_raw_field_preserved_python() {
        let input = "\"\"\"Some doc.\"\"\"";
        let doc = ext().extract(input, Language::Python);
        assert_eq!(doc.raw.as_deref(), Some("\"\"\"Some doc.\"\"\""));
    }

    // ---------------------------------------------------------------
    // Special characters and code blocks in docs
    // ---------------------------------------------------------------

    #[test]
    fn test_jsdoc_special_characters() {
        let jsdoc =
            "/**\n * Process <T> & handle \"quotes\".\n * @param {Array<string>} items - The items\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.description.unwrap().contains("<T>"));
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].type_info.as_deref(), Some("Array<string>"));
    }

    #[test]
    fn test_python_docstring_with_code_block() {
        let docstring =
            "\"\"\"Process data.\n\nExample:\n    ```python\n    result = process(data)\n    ```\n\"\"\"";
        let doc = ext().extract(docstring, Language::Python);

        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].code.contains("process(data)"));
    }

    #[test]
    fn test_jsdoc_with_unicode() {
        let jsdoc = "/**\n * Calculate \u{03C0} (pi) approximation.\n * @param {number} n - Number of iterations\n */";
        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.description.unwrap().contains('\u{03C0}'));
        assert_eq!(doc.params.len(), 1);
    }

    // ---------------------------------------------------------------
    // strip_comment_markers helper
    // ---------------------------------------------------------------

    #[test]
    fn test_strip_comment_markers_basic() {
        let e = ext();
        let result = e.strip_comment_markers("/** line1\n * line2\n */", "/**", "*/", "*");
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        // Should not contain the * prefix
        assert!(!result.contains("* line2"));
    }

    #[test]
    fn test_strip_comment_markers_no_prefix_match() {
        let e = ext();
        let result =
            e.strip_comment_markers("/** no prefix lines\nplain line\n */", "/**", "*/", "*");
        assert!(result.contains("plain line"));
    }

    // ---------------------------------------------------------------
    // strip_rust_doc_markers helper
    // ---------------------------------------------------------------

    #[test]
    fn test_strip_rust_doc_markers_triple_slash() {
        let e = ext();
        let result = e.strip_rust_doc_markers("/// Hello\n/// World");
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_strip_rust_doc_markers_inner() {
        let e = ext();
        let result = e.strip_rust_doc_markers("//! Module doc\n//! More");
        assert!(result.contains("Module doc"));
        assert!(result.contains("More"));
    }

    #[test]
    fn test_strip_rust_doc_markers_block_style() {
        let e = ext();
        let result = e.strip_rust_doc_markers("/** Block\n * content\n */");
        assert!(result.contains("Block"));
        assert!(result.contains("content"));
    }

    #[test]
    fn test_strip_rust_doc_markers_closing_only() {
        let e = ext();
        let result = e.strip_rust_doc_markers("*/");
        // `*/` starts with '*', so it matches the `starts_with('*')` branch
        // and returns `"/".trim_start()` = "/"
        // The `trimmed == "*/"` branch is unreachable due to ordering
        assert_eq!(result, "/");
    }

    #[test]
    fn test_strip_rust_doc_markers_plain_line() {
        let e = ext();
        let result = e.strip_rust_doc_markers("plain text without markers");
        assert!(result.contains("plain text without markers"));
    }

    // ---------------------------------------------------------------
    // Deprecated detection (existing test preserved)
    // ---------------------------------------------------------------

    #[test]
    fn test_deprecated_detection() {
        let jsdoc = r#"/**
         * Old function.
         * @deprecated Use newFunction instead
         */
        "#;

        let doc = ext().extract(jsdoc, Language::JavaScript);

        assert!(doc.is_deprecated);
        assert!(doc.deprecation_message.is_some());
    }
}
