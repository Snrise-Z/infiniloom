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

    #[test]
    fn test_jsdoc_parsing() {
        let extractor = DocumentationExtractor::new();

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

        let doc = extractor.extract(jsdoc, Language::JavaScript);

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
    fn test_python_docstring_parsing() {
        let extractor = DocumentationExtractor::new();

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

        let doc = extractor.extract(docstring, Language::Python);

        assert!(doc.summary.is_some());
        assert!(doc.summary.unwrap().contains("Calculate"));
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert!(doc.returns.is_some());
        assert_eq!(doc.throws.len(), 1);
    }

    #[test]
    fn test_rust_doc_parsing() {
        let extractor = DocumentationExtractor::new();

        let rust_doc = r#"/// Calculate the sum of two numbers.
        ///
        /// # Arguments
        ///
        /// * `a` - The first number
        /// * `b` - The second number
        ///
        /// # Returns
        ///
        /// The sum of a and b
        "#;

        let doc = extractor.extract(rust_doc, Language::Rust);

        assert!(doc.summary.is_some());
        assert!(doc.summary.unwrap().contains("Calculate"));
        assert!(doc.returns.is_some());
    }

    #[test]
    fn test_deprecated_detection() {
        let extractor = DocumentationExtractor::new();

        let jsdoc = r#"/**
         * Old function.
         * @deprecated Use newFunction instead
         */
        "#;

        let doc = extractor.extract(jsdoc, Language::JavaScript);

        assert!(doc.is_deprecated);
        assert!(doc.deprecation_message.is_some());
    }
}
