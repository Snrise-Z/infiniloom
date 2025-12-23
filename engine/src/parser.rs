//! Tree-sitter based code parser for extracting symbols from source files
//!
//! This module provides a unified interface for parsing source code across
//! multiple programming languages and extracting symbols (functions, classes,
//! methods, structs, enums, etc.) with their metadata.
//!
//! # Supported Languages
//!
//! Full symbol extraction support (with tree-sitter queries):
//! - Python
//! - JavaScript
//! - TypeScript
//! - Rust
//! - Go
//! - Java
//! - C
//! - C++
//! - C#
//! - Ruby
//! - Bash
//! - PHP
//! - Kotlin
//! - Swift
//! - Scala
//! - Haskell
//! - Elixir
//! - Clojure
//! - OCaml
//! - Lua
//! - R
//!
//! Note: F# is recognized by file extension but tree-sitter parser support
//! is not yet implemented.
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::parser::{Parser, Language};
//!
//! let parser = Parser::new();
//! let source_code = std::fs::read_to_string("example.py")?;
//! let symbols = parser.parse(&source_code, Language::Python)?;
//!
//! for symbol in symbols {
//!     println!("{}: {} (lines {}-{})",
//!         symbol.kind.name(),
//!         symbol.name,
//!         symbol.start_line,
//!         symbol.end_line
//!     );
//! }
//! ```

use crate::types::{Symbol, SymbolKind, Visibility};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tree_sitter::{Node, Parser as TSParser, Query, QueryCursor, StreamingIterator, Tree};

/// Parser errors
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Invalid UTF-8 in source code")]
    InvalidUtf8,
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Rust,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Bash,
    Php,
    Kotlin,
    Swift,
    Scala,
    Haskell,
    Elixir,
    Clojure,
    OCaml,
    FSharp,
    Lua,
    R,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" | "pyw" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "rb" | "rake" | "gemspec" => Some(Self::Ruby),
            "sh" | "bash" | "zsh" | "fish" => Some(Self::Bash),
            "php" | "phtml" | "php3" | "php4" | "php5" | "phps" => Some(Self::Php),
            "kt" | "kts" => Some(Self::Kotlin),
            "swift" => Some(Self::Swift),
            "scala" | "sc" => Some(Self::Scala),
            "hs" | "lhs" => Some(Self::Haskell),
            "ex" | "exs" | "eex" | "heex" | "leex" => Some(Self::Elixir),
            "clj" | "cljs" | "cljc" | "edn" => Some(Self::Clojure),
            "ml" | "mli" => Some(Self::OCaml),
            "fs" | "fsi" | "fsx" | "fsscript" => Some(Self::FSharp),
            "lua" => Some(Self::Lua),
            "r" | "rmd" => Some(Self::R),
            _ => None,
        }
    }

    /// Get language name as string
    pub fn name(&self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Bash => "bash",
            Self::Php => "php",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Scala => "scala",
            Self::Haskell => "haskell",
            Self::Elixir => "elixir",
            Self::Clojure => "clojure",
            Self::OCaml => "ocaml",
            Self::FSharp => "fsharp",
            Self::Lua => "lua",
            Self::R => "r",
        }
    }
}

/// Main parser struct for extracting code symbols
/// Uses lazy initialization - parsers are only created when first needed
///
/// # Performance
///
/// The parser uses "super-queries" that combine symbol extraction, imports, and call
/// expressions into a single tree traversal per file. This is more efficient than
/// running multiple separate queries.
pub struct Parser {
    parsers: HashMap<Language, TSParser>,
    queries: HashMap<Language, Query>,
    /// Super-queries that combine symbols + imports in one pass
    super_queries: HashMap<Language, Query>,
}

impl Parser {
    /// Create a new parser instance with lazy initialization
    /// Parsers and queries are created on-demand when parse() is called
    pub fn new() -> Self {
        Self { parsers: HashMap::new(), queries: HashMap::new(), super_queries: HashMap::new() }
    }

    /// Ensure parser and query are initialized for a language
    fn ensure_initialized(&mut self, language: Language) -> Result<(), ParserError> {
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(parser_entry) = self.parsers.entry(language) {
            let (parser, query, super_query) = match language {
                Language::Python => (
                    Self::init_python_parser()?,
                    Self::python_query()?,
                    Self::python_super_query()?,
                ),
                Language::JavaScript => (
                    Self::init_javascript_parser()?,
                    Self::javascript_query()?,
                    Self::javascript_super_query()?,
                ),
                Language::TypeScript => (
                    Self::init_typescript_parser()?,
                    Self::typescript_query()?,
                    Self::typescript_super_query()?,
                ),
                Language::Rust => {
                    (Self::init_rust_parser()?, Self::rust_query()?, Self::rust_super_query()?)
                },
                Language::Go => {
                    (Self::init_go_parser()?, Self::go_query()?, Self::go_super_query()?)
                },
                Language::Java => {
                    (Self::init_java_parser()?, Self::java_query()?, Self::java_super_query()?)
                },
                Language::C => (Self::init_c_parser()?, Self::c_query()?, Self::c_super_query()?),
                Language::Cpp => {
                    (Self::init_cpp_parser()?, Self::cpp_query()?, Self::cpp_super_query()?)
                },
                Language::CSharp => (
                    Self::init_csharp_parser()?,
                    Self::csharp_query()?,
                    Self::csharp_super_query()?,
                ),
                Language::Ruby => {
                    (Self::init_ruby_parser()?, Self::ruby_query()?, Self::ruby_super_query()?)
                },
                Language::Bash => {
                    (Self::init_bash_parser()?, Self::bash_query()?, Self::bash_super_query()?)
                },
                Language::Php => {
                    (Self::init_php_parser()?, Self::php_query()?, Self::php_super_query()?)
                },
                Language::Kotlin => (
                    Self::init_kotlin_parser()?,
                    Self::kotlin_query()?,
                    Self::kotlin_super_query()?,
                ),
                Language::Swift => {
                    (Self::init_swift_parser()?, Self::swift_query()?, Self::swift_super_query()?)
                },
                Language::Scala => {
                    (Self::init_scala_parser()?, Self::scala_query()?, Self::scala_super_query()?)
                },
                Language::Haskell => (
                    Self::init_haskell_parser()?,
                    Self::haskell_query()?,
                    Self::haskell_super_query()?,
                ),
                Language::Elixir => (
                    Self::init_elixir_parser()?,
                    Self::elixir_query()?,
                    Self::elixir_super_query()?,
                ),
                Language::Clojure => (
                    Self::init_clojure_parser()?,
                    Self::clojure_query()?,
                    Self::clojure_super_query()?,
                ),
                Language::OCaml => {
                    (Self::init_ocaml_parser()?, Self::ocaml_query()?, Self::ocaml_super_query()?)
                },
                Language::FSharp => {
                    return Err(ParserError::UnsupportedLanguage(
                        "F# not yet supported (no tree-sitter grammar available)".to_owned(),
                    ));
                },
                Language::Lua => {
                    (Self::init_lua_parser()?, Self::lua_query()?, Self::lua_super_query()?)
                },
                Language::R => (Self::init_r_parser()?, Self::r_query()?, Self::r_super_query()?),
            };
            parser_entry.insert(parser);
            self.queries.insert(language, query);
            self.super_queries.insert(language, super_query);
        }
        Ok(())
    }

    /// Parse source code and extract symbols
    ///
    /// This method now uses "super-queries" that combine symbol extraction and imports
    /// into a single AST traversal for better performance.
    pub fn parse(
        &mut self,
        source_code: &str,
        language: Language,
    ) -> Result<Vec<Symbol>, ParserError> {
        // Lazy initialization - only init parser for this language
        self.ensure_initialized(language)?;

        let parser = self
            .parsers
            .get_mut(&language)
            .ok_or_else(|| ParserError::UnsupportedLanguage(language.name().to_owned()))?;

        let tree = parser
            .parse(source_code, None)
            .ok_or_else(|| ParserError::ParseError("Failed to parse source code".to_owned()))?;

        // Use super-query for single-pass extraction (symbols + imports)
        let super_query = self
            .super_queries
            .get(&language)
            .ok_or_else(|| ParserError::QueryError("No super-query available".to_owned()))?;

        self.extract_symbols_single_pass(&tree, source_code, super_query, language)
    }

    /// Extract symbols using single-pass super-query (combines symbols + imports)
    fn extract_symbols_single_pass(
        &self,
        tree: &Tree,
        source_code: &str,
        query: &Query,
        language: Language,
    ) -> Result<Vec<Symbol>, ParserError> {
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        let mut matches = cursor.matches(query, root_node, source_code.as_bytes());
        let capture_names: Vec<&str> = query.capture_names().to_vec();

        while let Some(m) = matches.next() {
            // Process imports (captured with @import)
            if let Some(import_symbol) = self.process_import_match(m, source_code, &capture_names) {
                symbols.push(import_symbol);
                continue;
            }

            // Process regular symbols (functions, classes, etc.)
            if let Some(symbol) =
                self.process_match_single_pass(m, source_code, &capture_names, language)
            {
                symbols.push(symbol);
            }
        }

        Ok(symbols)
    }

    /// Process an import match from super-query
    fn process_import_match(
        &self,
        m: &tree_sitter::QueryMatch<'_, '_>,
        source_code: &str,
        capture_names: &[&str],
    ) -> Option<Symbol> {
        let captures = &m.captures;

        // Look for import capture
        let import_capture = captures.iter().find(|c| {
            capture_names
                .get(c.index as usize)
                .map(|n| *n == "import")
                .unwrap_or(false)
        })?;

        let node = import_capture.node;
        let text = node.utf8_text(source_code.as_bytes()).ok()?;

        let mut symbol = Symbol::new(text.trim(), SymbolKind::Import);
        symbol.start_line = node.start_position().row as u32 + 1;
        symbol.end_line = node.end_position().row as u32 + 1;

        Some(symbol)
    }

    /// Process a symbol match from super-query (single-pass version)
    fn process_match_single_pass(
        &self,
        m: &tree_sitter::QueryMatch<'_, '_>,
        source_code: &str,
        capture_names: &[&str],
        language: Language,
    ) -> Option<Symbol> {
        let captures = &m.captures;

        // Find name capture
        let name_node = captures
            .iter()
            .find(|c| {
                capture_names
                    .get(c.index as usize)
                    .map(|n| *n == "name")
                    .unwrap_or(false)
            })?
            .node;

        // Find kind capture (function, class, method, etc.)
        let kind_capture = captures.iter().find(|c| {
            capture_names
                .get(c.index as usize)
                .map(|n| {
                    ["function", "class", "method", "struct", "enum", "interface", "trait"]
                        .contains(n)
                })
                .unwrap_or(false)
        })?;

        let kind_name = capture_names.get(kind_capture.index as usize)?;
        let mut symbol_kind = self.map_symbol_kind(kind_name);

        let name = name_node.utf8_text(source_code.as_bytes()).ok()?;

        // Find the definition node (usually the largest capture)
        let def_node = captures
            .iter()
            .max_by_key(|c| c.node.byte_range().len())
            .map(|c| c.node)
            .unwrap_or(name_node);

        if language == Language::Kotlin && def_node.kind() == "class_declaration" {
            let mut cursor = def_node.walk();
            for child in def_node.children(&mut cursor) {
                if child.kind() == "interface" {
                    symbol_kind = SymbolKind::Interface;
                    break;
                }
            }
        }

        let start_line = def_node.start_position().row as u32 + 1;
        let end_line = def_node.end_position().row as u32 + 1;

        // Extract signature, docstring, parent, visibility, calls
        let signature = self.extract_signature(def_node, source_code, language);
        let docstring = self.extract_docstring(def_node, source_code, language);
        let parent = if symbol_kind == SymbolKind::Method {
            self.extract_parent(def_node, source_code)
        } else {
            None
        };
        let visibility = self.extract_visibility(def_node, source_code, language);
        let calls = if matches!(symbol_kind, SymbolKind::Function | SymbolKind::Method) {
            self.extract_calls(def_node, source_code, language)
        } else {
            Vec::new()
        };

        // Extract inheritance info for classes, structs, interfaces
        let (extends, implements) = if matches!(
            symbol_kind,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface
        ) {
            self.extract_inheritance(def_node, source_code, language)
        } else {
            (None, Vec::new())
        };

        let mut symbol = Symbol::new(name, symbol_kind);
        symbol.start_line = start_line;
        symbol.end_line = end_line;
        symbol.signature = signature;
        symbol.docstring = docstring;
        symbol.parent = parent;
        symbol.visibility = visibility;
        symbol.calls = calls;
        symbol.extends = extends;
        symbol.implements = implements;

        Some(symbol)
    }

    /// Map query capture name to SymbolKind
    fn map_symbol_kind(&self, capture_name: &str) -> SymbolKind {
        match capture_name {
            "function" => SymbolKind::Function,
            "class" => SymbolKind::Class,
            "method" => SymbolKind::Method,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "interface" => SymbolKind::Interface,
            "trait" => SymbolKind::Trait,
            _ => SymbolKind::Function,
        }
    }

    /// Extract function/method signature
    fn extract_signature(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Option<String> {
        // Find the signature node based on language
        let sig_node = match language {
            Language::Python => {
                // For Python, find function_definition and get first line
                if node.kind() == "function_definition" {
                    // Get the line from 'def' to ':'
                    let start = node.start_byte();
                    let mut end = start;
                    for byte in &source_code.as_bytes()[start..] {
                        end += 1;
                        if *byte == b':' {
                            break;
                        }
                        if *byte == b'\n' {
                            break;
                        }
                    }
                    return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                }
                None
            },
            Language::JavaScript | Language::TypeScript => {
                // For JS/TS, try to find the function declaration
                if node.kind().contains("function") || node.kind().contains("method") {
                    // Get first line up to opening brace
                    let start = node.start_byte();
                    let mut end = start;
                    let mut brace_count = 0;
                    for byte in &source_code.as_bytes()[start..] {
                        if *byte == b'{' {
                            brace_count += 1;
                            if brace_count == 1 {
                                break;
                            }
                        }
                        end += 1;
                    }
                    return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                }
                None
            },
            Language::Rust => {
                // For Rust, get the function signature
                if node.kind() == "function_item" {
                    // Get everything before the body
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end].trim().to_owned().replace('\n', " "),
                            );
                        }
                    }
                }
                None
            },
            Language::Go => {
                // For Go, get function declaration
                if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end].trim().to_owned().replace('\n', " "),
                            );
                        }
                    }
                }
                None
            },
            Language::Java => {
                // For Java, get method declaration
                if node.kind() == "method_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end].trim().to_owned().replace('\n', " "),
                            );
                        }
                    }
                }
                None
            },
            // Languages with block-based bodies (similar to Java/Go)
            Language::C
            | Language::Cpp
            | Language::CSharp
            | Language::Php
            | Language::Kotlin
            | Language::Swift
            | Language::Scala => {
                // Get everything before the body block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block"
                        || child.kind() == "compound_statement"
                        || child.kind() == "function_body"
                    {
                        let start = node.start_byte();
                        let end = child.start_byte();
                        return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                    }
                }
                None
            },
            // Ruby uses 'end' keyword, get first line
            Language::Ruby | Language::Lua => {
                let start = node.start_byte();
                let mut end = start;
                for byte in &source_code.as_bytes()[start..] {
                    end += 1;
                    if *byte == b'\n' {
                        break;
                    }
                }
                Some(source_code[start..end].trim().to_owned())
            },
            // Bash functions
            Language::Bash => {
                let start = node.start_byte();
                let mut end = start;
                for byte in &source_code.as_bytes()[start..] {
                    if *byte == b'{' {
                        break;
                    }
                    end += 1;
                }
                Some(source_code[start..end].trim().to_owned())
            },
            // Functional languages - get first line or up to '='
            Language::Haskell
            | Language::OCaml
            | Language::FSharp
            | Language::Elixir
            | Language::Clojure
            | Language::R => {
                let start = node.start_byte();
                let mut end = start;
                for byte in &source_code.as_bytes()[start..] {
                    end += 1;
                    if *byte == b'\n' || *byte == b'=' {
                        break;
                    }
                }
                Some(source_code[start..end].trim().to_owned())
            },
        };

        sig_node.or_else(|| {
            // Fallback: get first line of the node
            let start = node.start_byte();
            let end = std::cmp::min(start + 200, source_code.len());
            let text = &source_code[start..end];
            text.lines().next().map(|s| s.trim().to_owned())
        })
    }

    /// Extract docstring/documentation comment
    fn extract_docstring(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Option<String> {
        match language {
            Language::Python => {
                // Look for string literal as first child of function body
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" {
                        // Look for first expression_statement with string
                        for stmt in child.children(&mut child.walk()) {
                            if stmt.kind() == "expression_statement" {
                                for expr in stmt.children(&mut stmt.walk()) {
                                    if expr.kind() == "string" {
                                        if let Ok(text) = expr.utf8_text(source_code.as_bytes()) {
                                            // Remove quotes and clean up
                                            return Some(
                                                text.trim_matches(|c| c == '"' || c == '\'')
                                                    .trim()
                                                    .to_owned(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                None
            },
            Language::JavaScript | Language::TypeScript => {
                // Look for JSDoc comment immediately before the node
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") {
                                return Some(self.clean_jsdoc(text));
                            }
                        }
                    }
                }
                None
            },
            Language::Rust => {
                // Look for doc comment (///) above the node
                let start_byte = node.start_byte();
                let lines_before: Vec<_> = source_code[..start_byte]
                    .lines()
                    .rev()
                    .take_while(|line| line.trim().starts_with("///") || line.trim().is_empty())
                    .collect();

                if !lines_before.is_empty() {
                    let doc: Vec<String> = lines_before
                        .into_iter()
                        .rev()
                        .filter_map(|line| {
                            let trimmed = line.trim();
                            trimmed.strip_prefix("///").map(|s| s.trim().to_owned())
                        })
                        .collect();

                    if !doc.is_empty() {
                        return Some(doc.join(" "));
                    }
                }
                None
            },
            Language::Go => {
                // Look for comment immediately before
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches("//").trim().to_owned());
                        }
                    }
                }
                None
            },
            Language::Java => {
                // Look for JavaDoc comment
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "block_comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") {
                                return Some(self.clean_javadoc(text));
                            }
                        }
                    }
                }
                None
            },
            // C/C++ - look for /* */ or // comments
            Language::C | Language::Cpp => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") || text.starts_with("/*") {
                                return Some(self.clean_jsdoc(text));
                            }
                            return Some(text.trim_start_matches("//").trim().to_owned());
                        }
                    }
                }
                None
            },
            // C# - XML doc comments (///)
            Language::CSharp => {
                let start_byte = node.start_byte();
                let lines_before: Vec<_> = source_code[..start_byte]
                    .lines()
                    .rev()
                    .take_while(|line| line.trim().starts_with("///") || line.trim().is_empty())
                    .collect();

                if !lines_before.is_empty() {
                    let doc: Vec<String> = lines_before
                        .into_iter()
                        .rev()
                        .filter_map(|line| {
                            let trimmed = line.trim();
                            trimmed.strip_prefix("///").map(|s| s.trim().to_owned())
                        })
                        .collect();

                    if !doc.is_empty() {
                        return Some(doc.join(" "));
                    }
                }
                None
            },
            // Ruby - look for # comments
            Language::Ruby => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches('#').trim().to_owned());
                        }
                    }
                }
                None
            },
            // PHP - look for /** */ comments
            Language::Php | Language::Kotlin | Language::Swift | Language::Scala => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    let kind = prev_sibling.kind();
                    if kind == "comment" || kind == "multiline_comment" || kind == "block_comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") {
                                return Some(self.clean_jsdoc(text));
                            }
                        }
                    }
                }
                None
            },
            // Bash - look for # comments
            Language::Bash => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches('#').trim().to_owned());
                        }
                    }
                }
                None
            },
            // Haskell - look for {- -} or -- comments
            Language::Haskell => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            let cleaned = text
                                .trim_start_matches("{-")
                                .trim_end_matches("-}")
                                .trim_start_matches("--")
                                .trim();
                            return Some(cleaned.to_owned());
                        }
                    }
                }
                None
            },
            // Elixir - look for @doc or @moduledoc
            Language::Elixir => {
                // Simplified: just look for preceding comment
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches('#').trim().to_owned());
                        }
                    }
                }
                None
            },
            // Clojure - look for docstring in defn
            Language::Clojure => {
                // Docstrings are typically the second element in defn forms
                None
            },
            // OCaml - look for (** *) comments
            Language::OCaml | Language::FSharp => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            let cleaned = text
                                .trim_start_matches("(**")
                                .trim_start_matches("(*")
                                .trim_end_matches("*)")
                                .trim();
                            return Some(cleaned.to_owned());
                        }
                    }
                }
                None
            },
            // Lua - look for -- or --[[ ]] comments
            Language::Lua => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            let cleaned = text
                                .trim_start_matches("--[[")
                                .trim_end_matches("]]")
                                .trim_start_matches("--")
                                .trim();
                            return Some(cleaned.to_owned());
                        }
                    }
                }
                None
            },
            // R - look for # comments
            Language::R => {
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches('#').trim().to_owned());
                        }
                    }
                }
                None
            },
        }
    }

    /// Extract parent class/struct name for methods
    fn extract_parent(&self, node: Node<'_>, source_code: &str) -> Option<String> {
        let mut current = node.parent()?;

        while let Some(parent) = current.parent() {
            if ["class_definition", "class_declaration", "struct_item", "impl_item"]
                .contains(&parent.kind())
            {
                // Find the name node
                for child in parent.children(&mut parent.walk()) {
                    if child.kind() == "identifier" || child.kind() == "type_identifier" {
                        if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                            return Some(name.to_owned());
                        }
                    }
                }
            }
            current = parent;
        }

        None
    }

    /// Extract visibility modifier from a node
    fn extract_visibility(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Visibility {
        match language {
            Language::Python => {
                // Python uses naming convention: _private, __dunder__
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                        if name.starts_with("__") && !name.ends_with("__") {
                            return Visibility::Private;
                        } else if name.starts_with('_') {
                            return Visibility::Protected; // Convention for "internal"
                        }
                    }
                }
                Visibility::Public
            },
            Language::Rust => {
                // Check for pub keyword
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "visibility_modifier" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            if text.contains("pub(crate)") || text.contains("pub(super)") {
                                return Visibility::Internal;
                            } else if text.starts_with("pub") {
                                return Visibility::Public;
                            }
                        }
                    }
                }
                Visibility::Private // Rust default is private
            },
            Language::JavaScript | Language::TypeScript => {
                // Check for private/protected keywords (TypeScript/ES2022)
                for child in node.children(&mut node.walk()) {
                    let kind = child.kind();
                    if kind == "private" || kind == "accessibility_modifier" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            return match text {
                                "private" => Visibility::Private,
                                "protected" => Visibility::Protected,
                                _ => Visibility::Public,
                            };
                        }
                    }
                }
                // Check for # prefix (private fields in JS)
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                        if name.starts_with('#') {
                            return Visibility::Private;
                        }
                    }
                }
                Visibility::Public
            },
            Language::Go => {
                // Go uses capitalization: Exported vs unexported
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                        if let Some(first_char) = name.chars().next() {
                            if first_char.is_lowercase() {
                                return Visibility::Private;
                            }
                        }
                    }
                }
                Visibility::Public
            },
            Language::Java => {
                // Check for visibility modifiers
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "modifiers" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            if text.contains("private") {
                                return Visibility::Private;
                            } else if text.contains("protected") {
                                return Visibility::Protected;
                            } else if text.contains("public") {
                                return Visibility::Public;
                            }
                        }
                    }
                }
                Visibility::Internal // Java default is package-private
            },
            // C/C++ - check for static keyword (file-local)
            Language::C | Language::Cpp => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "storage_class_specifier" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            if text == "static" {
                                return Visibility::Private;
                            }
                        }
                    }
                }
                Visibility::Public
            },
            // C# - check for access modifiers
            Language::CSharp | Language::Kotlin | Language::Swift | Language::Scala => {
                for child in node.children(&mut node.walk()) {
                    let kind = child.kind();
                    if kind == "modifier" || kind == "modifiers" || kind == "visibility_modifier" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            if text.contains("private") {
                                return Visibility::Private;
                            } else if text.contains("protected") {
                                return Visibility::Protected;
                            } else if text.contains("internal") {
                                return Visibility::Internal;
                            } else if text.contains("public") {
                                return Visibility::Public;
                            }
                        }
                    }
                }
                Visibility::Internal
            },
            // Ruby - uses naming convention like Python
            Language::Ruby => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                        if name.starts_with("_") {
                            return Visibility::Private;
                        }
                    }
                }
                Visibility::Public
            },
            // PHP - check for visibility keywords
            Language::Php => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "visibility_modifier" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            return match text {
                                "private" => Visibility::Private,
                                "protected" => Visibility::Protected,
                                "public" => Visibility::Public,
                                _ => Visibility::Public,
                            };
                        }
                    }
                }
                Visibility::Public
            },
            // Bash - all functions are effectively public (no visibility concept)
            Language::Bash => Visibility::Public,
            // Functional languages - generally public by default
            Language::Haskell
            | Language::Elixir
            | Language::Clojure
            | Language::OCaml
            | Language::FSharp
            | Language::Lua
            | Language::R => Visibility::Public,
        }
    }

    /// Extract function calls from a function/method body (Bug #1 fix - improved extraction)
    fn extract_calls(&self, node: Node<'_>, source_code: &str, language: Language) -> Vec<String> {
        let mut calls = HashSet::new();

        // Find the function body
        let body_node = self.find_body_node(node, language);
        if let Some(body) = body_node {
            self.collect_calls_recursive(body, source_code, language, &mut calls);
        }

        // Fallback: If no calls found and we have a body, scan the entire node
        // This handles cases where body detection might miss some patterns
        if calls.is_empty() {
            self.collect_calls_recursive(node, source_code, language, &mut calls);
        }

        calls.into_iter().collect()
    }

    /// Find the body node of a function/method
    fn find_body_node<'a>(&self, node: Node<'a>, language: Language) -> Option<Node<'a>> {
        match language {
            Language::Python => {
                // Python: function_definition > block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        return Some(child);
                    }
                }
            },
            Language::Rust => {
                // Rust: function_item > block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        return Some(child);
                    }
                }
            },
            Language::JavaScript | Language::TypeScript => {
                // JS/TS: various function forms - statement_block, arrow body, or expression body
                for child in node.children(&mut node.walk()) {
                    let kind = child.kind();
                    if kind == "statement_block" {
                        return Some(child);
                    }
                    // Arrow functions can have expression bodies
                    if kind == "arrow_function" {
                        // Recursively find body in arrow function
                        if let Some(body) = self.find_body_node(child, language) {
                            return Some(body);
                        }
                        // Or the arrow function itself has the calls
                        return Some(child);
                    }
                }
                // Fallback: for arrow functions without block body, return the node itself
                // This handles cases like: const fn = () => doSomething()
                if node.kind() == "arrow_function" {
                    for child in node.children(&mut node.walk()) {
                        // Skip parameter list and arrow
                        let kind = child.kind();
                        if kind != "formal_parameters"
                            && kind != "identifier"
                            && kind != "=>"
                            && kind != "("
                            && kind != ")"
                            && kind != ","
                        {
                            return Some(child);
                        }
                    }
                    return Some(node);
                }
            },
            Language::Go => {
                // Go: function_declaration > block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        return Some(child);
                    }
                }
            },
            Language::Java => {
                // Java: method_declaration > block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        return Some(child);
                    }
                }
            },
            // C/C++ - compound_statement
            Language::C | Language::Cpp => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "compound_statement" {
                        return Some(child);
                    }
                }
            },
            // Languages with block bodies (similar to Java)
            Language::CSharp
            | Language::Php
            | Language::Kotlin
            | Language::Swift
            | Language::Scala => {
                for child in node.children(&mut node.walk()) {
                    let kind = child.kind();
                    if kind == "block" || kind == "compound_statement" || kind == "function_body" {
                        return Some(child);
                    }
                }
            },
            // Ruby - uses do/end blocks
            Language::Ruby => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "body_statement" || child.kind() == "do_block" {
                        return Some(child);
                    }
                }
            },
            // Bash - compound_statement
            Language::Bash => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "compound_statement" {
                        return Some(child);
                    }
                }
            },
            // Functional languages - typically expression-based
            Language::Haskell
            | Language::Elixir
            | Language::Clojure
            | Language::OCaml
            | Language::FSharp
            | Language::R => {
                // Return the node itself as expressions are the body
                return Some(node);
            },
            // Lua - block body
            Language::Lua => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        return Some(child);
                    }
                }
            },
        }
        None
    }

    /// Recursively collect function calls from a node
    #[allow(clippy::only_used_in_recursion)]
    fn collect_calls_recursive(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
        calls: &mut HashSet<String>,
    ) {
        let kind = node.kind();

        // Check if this node is a call expression
        let call_name = match language {
            Language::Python => {
                if kind == "call" {
                    // Python: call > function (identifier or attribute)
                    node.child_by_field_name("function").and_then(|f| {
                        if f.kind() == "identifier" {
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else if f.kind() == "attribute" {
                            // Get the attribute name (method name)
                            f.child_by_field_name("attribute")
                                .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                                .map(String::from)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            },
            Language::Rust => {
                if kind == "call_expression" {
                    // Rust: call_expression > function
                    node.child_by_field_name("function").and_then(|f| {
                        if f.kind() == "identifier" {
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else if f.kind() == "field_expression" {
                            // Method call: get the field name
                            f.child_by_field_name("field")
                                .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                                .map(String::from)
                        } else if f.kind() == "scoped_identifier" {
                            // Path call like Module::function
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else {
                            None
                        }
                    })
                } else if kind == "macro_invocation" {
                    // Rust macros
                    node.child_by_field_name("macro")
                        .and_then(|m| m.utf8_text(source_code.as_bytes()).ok())
                        .map(|s| format!("{}!", s))
                } else {
                    None
                }
            },
            Language::JavaScript | Language::TypeScript => {
                if kind == "call_expression" {
                    // JS/TS: call_expression > function
                    node.child_by_field_name("function").and_then(|f| {
                        if f.kind() == "identifier" {
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else if f.kind() == "member_expression" {
                            // Method call: get the property
                            f.child_by_field_name("property")
                                .and_then(|p| p.utf8_text(source_code.as_bytes()).ok())
                                .map(String::from)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            },
            Language::Go => {
                if kind == "call_expression" {
                    // Go: call_expression > function
                    node.child_by_field_name("function").and_then(|f| {
                        if f.kind() == "identifier" {
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else if f.kind() == "selector_expression" {
                            // Method call: get the field
                            f.child_by_field_name("field")
                                .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                                .map(String::from)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            },
            Language::Java => {
                if kind == "method_invocation" {
                    // Java: method_invocation > name
                    node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                        .map(String::from)
                } else {
                    None
                }
            },
            // C/C++ - call_expression
            Language::C | Language::Cpp => {
                if kind == "call_expression" {
                    node.child_by_field_name("function").and_then(|f| {
                        if f.kind() == "identifier" {
                            f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else if f.kind() == "field_expression" {
                            f.child_by_field_name("field")
                                .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                                .map(String::from)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            },
            // C#/Kotlin/Swift/Scala - invocation_expression or call_expression
            Language::CSharp | Language::Kotlin | Language::Swift | Language::Scala => {
                if kind == "invocation_expression"
                    || kind == "call_expression"
                    || kind == "method_invocation"
                {
                    node.child_by_field_name("function")
                        .or_else(|| node.child_by_field_name("name"))
                        .and_then(|f| {
                            if f.kind() == "identifier" || f.kind() == "simple_identifier" {
                                f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                            } else if f.kind() == "member_access_expression"
                                || f.kind() == "member_expression"
                            {
                                f.child_by_field_name("name")
                                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                                    .map(String::from)
                            } else {
                                None
                            }
                        })
                } else {
                    None
                }
            },
            // Ruby - method_call or call
            Language::Ruby => {
                if kind == "call" || kind == "method_call" {
                    node.child_by_field_name("method")
                        .and_then(|m| m.utf8_text(source_code.as_bytes()).ok())
                        .map(String::from)
                } else {
                    None
                }
            },
            // PHP - function_call_expression
            Language::Php => {
                if kind == "function_call_expression" || kind == "method_call_expression" {
                    node.child_by_field_name("function")
                        .or_else(|| node.child_by_field_name("name"))
                        .and_then(|f| f.utf8_text(source_code.as_bytes()).ok())
                        .map(String::from)
                } else {
                    None
                }
            },
            // Bash - command
            Language::Bash => {
                if kind == "command" {
                    node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                        .map(String::from)
                } else {
                    None
                }
            },
            // Functional languages - application or call nodes
            Language::Haskell
            | Language::Elixir
            | Language::Clojure
            | Language::OCaml
            | Language::FSharp
            | Language::Lua
            | Language::R => {
                if kind == "application" || kind == "call" || kind == "function_call" {
                    // Get first child as function name
                    node.child(0).and_then(|c| {
                        if c.kind() == "identifier" || c.kind() == "variable" {
                            c.utf8_text(source_code.as_bytes()).ok().map(String::from)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            },
        };

        if let Some(name) = call_name {
            // Filter out common built-ins and very short names
            if name.len() > 1 && !Self::is_builtin(&name, language) {
                calls.insert(name);
            }
        }

        // Recurse into children
        for child in node.children(&mut node.walk()) {
            self.collect_calls_recursive(child, source_code, language, calls);
        }
    }

    /// Check if a function name is a common built-in (to filter noise)
    fn is_builtin(name: &str, language: Language) -> bool {
        match language {
            Language::Python => {
                matches!(
                    name,
                    "print"
                        | "len"
                        | "range"
                        | "str"
                        | "int"
                        | "float"
                        | "list"
                        | "dict"
                        | "set"
                        | "tuple"
                        | "bool"
                        | "type"
                        | "isinstance"
                        | "hasattr"
                        | "getattr"
                        | "setattr"
                        | "super"
                        | "iter"
                        | "next"
                        | "open"
                        | "input"
                        | "format"
                        | "enumerate"
                        | "zip"
                        | "map"
                        | "filter"
                        | "sorted"
                        | "reversed"
                        | "sum"
                        | "min"
                        | "max"
                        | "abs"
                        | "round"
                        | "ord"
                        | "chr"
                        | "hex"
                        | "bin"
                        | "oct"
                )
            },
            Language::JavaScript | Language::TypeScript => {
                matches!(
                    name,
                    "console"
                        | "log"
                        | "error"
                        | "warn"
                        | "parseInt"
                        | "parseFloat"
                        | "setTimeout"
                        | "setInterval"
                        | "clearTimeout"
                        | "clearInterval"
                        | "JSON"
                        | "stringify"
                        | "parse"
                        | "toString"
                        | "valueOf"
                        | "push"
                        | "pop"
                        | "shift"
                        | "unshift"
                        | "slice"
                        | "splice"
                        | "map"
                        | "filter"
                        | "reduce"
                        | "forEach"
                        | "find"
                        | "findIndex"
                        | "includes"
                        | "indexOf"
                        | "join"
                        | "split"
                        | "replace"
                )
            },
            Language::Rust => {
                matches!(
                    name,
                    "println!"
                        | "print!"
                        | "eprintln!"
                        | "eprint!"
                        | "format!"
                        | "vec!"
                        | "panic!"
                        | "assert!"
                        | "assert_eq!"
                        | "assert_ne!"
                        | "debug!"
                        | "info!"
                        | "warn!"
                        | "error!"
                        | "trace!"
                        | "unwrap"
                        | "expect"
                        | "ok"
                        | "err"
                        | "some"
                        | "none"
                        | "clone"
                        | "to_string"
                        | "into"
                        | "from"
                        | "default"
                        | "iter"
                        | "into_iter"
                        | "collect"
                        | "map"
                        | "filter"
                )
            },
            Language::Go => {
                matches!(
                    name,
                    "fmt"
                        | "Println"
                        | "Printf"
                        | "Sprintf"
                        | "Errorf"
                        | "make"
                        | "new"
                        | "len"
                        | "cap"
                        | "append"
                        | "copy"
                        | "delete"
                        | "close"
                        | "panic"
                        | "recover"
                        | "print"
                )
            },
            Language::Java => {
                matches!(
                    name,
                    "println"
                        | "print"
                        | "printf"
                        | "toString"
                        | "equals"
                        | "hashCode"
                        | "getClass"
                        | "clone"
                        | "notify"
                        | "wait"
                        | "get"
                        | "set"
                        | "add"
                        | "remove"
                        | "size"
                        | "isEmpty"
                        | "contains"
                        | "iterator"
                        | "valueOf"
                        | "parseInt"
                )
            },
            Language::C | Language::Cpp => {
                matches!(
                    name,
                    "printf"
                        | "scanf"
                        | "malloc"
                        | "free"
                        | "memcpy"
                        | "memset"
                        | "strlen"
                        | "strcpy"
                        | "strcmp"
                        | "strcat"
                        | "sizeof"
                        | "cout"
                        | "cin"
                        | "endl"
                        | "cerr"
                        | "clog"
                )
            },
            Language::CSharp => {
                matches!(
                    name,
                    "WriteLine"
                        | "Write"
                        | "ReadLine"
                        | "ToString"
                        | "Equals"
                        | "GetHashCode"
                        | "GetType"
                        | "Add"
                        | "Remove"
                        | "Contains"
                        | "Count"
                        | "Clear"
                        | "ToList"
                        | "ToArray"
                )
            },
            Language::Ruby => {
                matches!(
                    name,
                    "puts"
                        | "print"
                        | "p"
                        | "gets"
                        | "each"
                        | "map"
                        | "select"
                        | "reject"
                        | "reduce"
                        | "inject"
                        | "find"
                        | "any?"
                        | "all?"
                        | "include?"
                        | "empty?"
                        | "nil?"
                        | "length"
                        | "size"
                )
            },
            Language::Php => {
                matches!(
                    name,
                    "echo"
                        | "print"
                        | "var_dump"
                        | "print_r"
                        | "isset"
                        | "empty"
                        | "array"
                        | "count"
                        | "strlen"
                        | "strpos"
                        | "substr"
                        | "explode"
                        | "implode"
                        | "json_encode"
                        | "json_decode"
                )
            },
            Language::Kotlin => {
                matches!(
                    name,
                    "println"
                        | "print"
                        | "readLine"
                        | "toString"
                        | "equals"
                        | "hashCode"
                        | "map"
                        | "filter"
                        | "forEach"
                        | "let"
                        | "also"
                        | "apply"
                        | "run"
                        | "with"
                        | "listOf"
                        | "mapOf"
                        | "setOf"
                )
            },
            Language::Swift => {
                matches!(
                    name,
                    "print"
                        | "debugPrint"
                        | "dump"
                        | "map"
                        | "filter"
                        | "reduce"
                        | "forEach"
                        | "contains"
                        | "count"
                        | "isEmpty"
                        | "append"
                )
            },
            Language::Scala => {
                matches!(
                    name,
                    "println"
                        | "print"
                        | "map"
                        | "filter"
                        | "flatMap"
                        | "foreach"
                        | "reduce"
                        | "fold"
                        | "foldLeft"
                        | "foldRight"
                        | "collect"
                )
            },
            // Languages where builtins are less common or harder to filter
            Language::Bash
            | Language::Haskell
            | Language::Elixir
            | Language::Clojure
            | Language::OCaml
            | Language::FSharp
            | Language::Lua
            | Language::R => false,
        }
    }

    /// Clean JSDoc comment
    fn clean_jsdoc(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_start_matches('*')
                    .trim_end_matches("*/")
                    .trim()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Clean JavaDoc comment
    fn clean_javadoc(&self, text: &str) -> String {
        self.clean_jsdoc(text) // Same format as JSDoc
    }

    /// Extract class inheritance (extends) and interface implementations (implements)
    /// Returns (base_class, implemented_interfaces)
    fn extract_inheritance(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> (Option<String>, Vec<String>) {
        let mut extends = None;
        let mut implements = Vec::new();

        match language {
            Language::Python => {
                // Python: class Foo(Bar, Baz): - first is base class, rest could be mixins
                if node.kind() == "class_definition" {
                    if let Some(args) = node.child_by_field_name("superclasses") {
                        let mut first = true;
                        for child in args.children(&mut args.walk()) {
                            if child.kind() == "identifier" || child.kind() == "attribute" {
                                if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                                    if first {
                                        extends = Some(name.to_owned());
                                        first = false;
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::JavaScript | Language::TypeScript => {
                // JS/TS: class Foo extends Bar implements IBaz, IQux
                if node.kind() == "class_declaration" {
                    // Look for heritage clause
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "class_heritage" {
                            for heritage in child.children(&mut child.walk()) {
                                if heritage.kind() == "extends_clause" {
                                    // Get the extended class
                                    for ext_child in heritage.children(&mut heritage.walk()) {
                                        if ext_child.kind() == "identifier"
                                            || ext_child.kind() == "type_identifier"
                                        {
                                            if let Ok(name) =
                                                ext_child.utf8_text(source_code.as_bytes())
                                            {
                                                extends = Some(name.to_owned());
                                            }
                                        }
                                    }
                                } else if heritage.kind() == "implements_clause" {
                                    // Get implemented interfaces
                                    for impl_child in heritage.children(&mut heritage.walk()) {
                                        if impl_child.kind() == "type_identifier"
                                            || impl_child.kind() == "identifier"
                                        {
                                            if let Ok(name) =
                                                impl_child.utf8_text(source_code.as_bytes())
                                            {
                                                implements.push(name.to_owned());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::Java => {
                // Java: class Foo extends Bar implements IBaz, IQux
                if node.kind() == "class_declaration" {
                    if let Some(superclass) = node.child_by_field_name("superclass") {
                        if let Ok(name) = superclass.utf8_text(source_code.as_bytes()) {
                            extends = Some(name.trim_start_matches("extends ").trim().to_owned());
                        }
                    }
                    if let Some(interfaces) = node.child_by_field_name("interfaces") {
                        for child in interfaces.children(&mut interfaces.walk()) {
                            if child.kind() == "type_identifier" {
                                if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                                    implements.push(name.to_owned());
                                }
                            }
                        }
                    }
                }
            },
            Language::Rust => {
                // Rust: impl Trait for Struct - captured differently
                // For structs, we don't have traditional inheritance
                // We could look at impl blocks separately if needed
            },
            Language::Go => {
                // Go: struct embedding (composition, not inheritance)
                // Go uses implicit interface implementation
                if node.kind() == "type_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "type_spec" {
                            // Look for embedded types in struct
                            for spec_child in child.children(&mut child.walk()) {
                                if spec_child.kind() == "struct_type" {
                                    for field in spec_child.children(&mut spec_child.walk()) {
                                        if field.kind() == "field_declaration" {
                                            // Embedded field has no name, just type
                                            let has_name =
                                                field.child_by_field_name("name").is_some();
                                            if !has_name {
                                                if let Some(type_node) =
                                                    field.child_by_field_name("type")
                                                {
                                                    if let Ok(name) =
                                                        type_node.utf8_text(source_code.as_bytes())
                                                    {
                                                        implements.push(name.to_owned());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::Cpp => {
                // C++: class Foo : public Bar, public IBaz
                if node.kind() == "class_specifier" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "base_class_clause" {
                            let mut first = true;
                            for base in child.children(&mut child.walk()) {
                                if base.kind() == "type_identifier" {
                                    if let Ok(name) = base.utf8_text(source_code.as_bytes()) {
                                        if first {
                                            extends = Some(name.to_owned());
                                            first = false;
                                        } else {
                                            implements.push(name.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::CSharp => {
                // C#: class Foo : Bar, IFoo, IBar
                if node.kind() == "class_declaration" {
                    if let Some(base_list) = node.child_by_field_name("bases") {
                        let mut first = true;
                        for child in base_list.children(&mut base_list.walk()) {
                            if child.kind() == "identifier" || child.kind() == "generic_name" {
                                if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                                    // Convention: interfaces start with 'I'
                                    if first && !name.starts_with('I') {
                                        extends = Some(name.to_owned());
                                        first = false;
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::Ruby => {
                // Ruby: class Foo < Bar
                if node.kind() == "class" {
                    if let Some(superclass) = node.child_by_field_name("superclass") {
                        if let Ok(name) = superclass.utf8_text(source_code.as_bytes()) {
                            extends = Some(name.to_owned());
                        }
                    }
                }
            },
            Language::Kotlin => {
                // Kotlin: class Foo : Bar(), IBaz, IQux
                if node.kind() == "class_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "delegation_specifiers" {
                            let mut first = true;
                            for spec in child.children(&mut child.walk()) {
                                if spec.kind() == "delegation_specifier" {
                                    if let Ok(name) = spec.utf8_text(source_code.as_bytes()) {
                                        // Remove constructor call parentheses
                                        let clean_name =
                                            name.split('(').next().unwrap_or(name).trim();
                                        if first {
                                            extends = Some(clean_name.to_owned());
                                            first = false;
                                        } else {
                                            implements.push(clean_name.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::Swift => {
                // Swift: class Foo: Bar, Protocol1, Protocol2
                if node.kind() == "class_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "type_inheritance_clause" {
                            let mut first = true;
                            for type_child in child.children(&mut child.walk()) {
                                if type_child.kind() == "type_identifier" {
                                    if let Ok(name) = type_child.utf8_text(source_code.as_bytes()) {
                                        if first {
                                            extends = Some(name.to_owned());
                                            first = false;
                                        } else {
                                            implements.push(name.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Language::Scala => {
                // Scala: class Foo extends Bar with Trait1 with Trait2
                if node.kind() == "class_definition" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "extends_clause" {
                            if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                                // Parse "extends Bar with Trait1 with Trait2"
                                let text = name.trim_start_matches("extends").trim();
                                let parts: Vec<&str> = text.split(" with ").collect();
                                if let Some(base) = parts.first() {
                                    extends = Some(base.trim().to_owned());
                                }
                                for trait_name in parts.iter().skip(1) {
                                    implements.push(trait_name.trim().to_owned());
                                }
                            }
                        }
                    }
                }
            },
            // Languages without traditional class inheritance
            Language::C
            | Language::Bash
            | Language::Php
            | Language::Haskell
            | Language::Elixir
            | Language::Clojure
            | Language::OCaml
            | Language::FSharp
            | Language::Lua
            | Language::R => {},
        }

        (extends, implements)
    }

    // Language-specific parser initializers

    fn init_python_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_javascript_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_typescript_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_rust_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_go_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_java_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_c_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_cpp_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_csharp_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_ruby_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_ruby::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_bash_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_php_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_php::LANGUAGE_PHP.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_kotlin_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_swift_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_swift::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_scala_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_scala::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_haskell_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_haskell::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_elixir_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_elixir::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_clojure_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_clojure::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_ocaml_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_ocaml::LANGUAGE_OCAML.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_lua_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_lua::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_r_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_r::LANGUAGE.into())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    // Language-specific queries

    fn python_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              name: (identifier) @name) @function

            (class_definition
              name: (identifier) @name) @class

            (class_definition
              body: (block
                (function_definition
                  name: (identifier) @name))) @method
        "#;

        Query::new(&tree_sitter_python::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn javascript_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (_) @name) @function

            (class_declaration
              name: (_) @name) @class

            (method_definition
              name: (property_identifier) @name) @method

            (arrow_function) @function

            (function_expression) @function
        "#;

        Query::new(&tree_sitter_javascript::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn typescript_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (class_declaration
              name: (type_identifier) @name) @class

            (interface_declaration
              name: (type_identifier) @name) @interface

            (method_definition
              name: (property_identifier) @name) @method

            (enum_declaration
              name: (identifier) @name) @enum

            ; Arrow functions (named via variable) - Bug #1 fix
            (lexical_declaration
              (variable_declarator
                name: (identifier) @name
                value: (arrow_function))) @function
        "#;

        Query::new(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn rust_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_item
              name: (identifier) @name) @function

            (struct_item
              name: (type_identifier) @name) @struct

            (enum_item
              name: (type_identifier) @name) @enum

            (trait_item
              name: (type_identifier) @name) @trait
        "#;

        Query::new(&tree_sitter_rust::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn go_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (method_declaration
              name: (field_identifier) @name) @method

            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (struct_type))) @struct

            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (interface_type))) @interface
        "#;

        Query::new(&tree_sitter_go::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn java_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (method_declaration
              name: (identifier) @name) @method

            (class_declaration
              name: (identifier) @name) @class

            (interface_declaration
              name: (identifier) @name) @interface

            (enum_declaration
              name: (identifier) @name) @enum
        "#;

        Query::new(&tree_sitter_java::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Super-queries: combine symbols + imports in a single pass for performance
    // ==========================================================================

    fn python_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              name: (identifier) @name) @function

            ; Classes
            (class_definition
              name: (identifier) @name) @class

            ; Methods inside classes
            (class_definition
              body: (block
                (function_definition
                  name: (identifier) @name))) @method

            ; Imports
            (import_statement) @import
            (import_from_statement) @import
        "#;

        Query::new(&tree_sitter_python::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn javascript_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_declaration
              name: (identifier) @name) @function

            ; Classes
            (class_declaration
              name: (identifier) @name) @class

            ; Methods
            (method_definition
              name: (property_identifier) @name) @method

            ; Arrow functions (named via variable)
            (lexical_declaration
              (variable_declarator
                name: (identifier) @name
                value: (arrow_function))) @function

            ; Imports
            (import_statement) @import
        "#;

        Query::new(&tree_sitter_javascript::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn typescript_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_declaration
              name: (identifier) @name) @function

            ; Classes
            (class_declaration
              name: (type_identifier) @name) @class

            ; Interfaces
            (interface_declaration
              name: (type_identifier) @name) @interface

            ; Methods
            (method_definition
              name: (property_identifier) @name) @method

            ; Enums
            (enum_declaration
              name: (identifier) @name) @enum

            ; Arrow functions (named via variable) - Bug #1 fix
            (lexical_declaration
              (variable_declarator
                name: (identifier) @name
                value: (arrow_function))) @function

            ; Arrow functions (exported)
            (export_statement
              declaration: (lexical_declaration
                (variable_declarator
                  name: (identifier) @name
                  value: (arrow_function)))) @function

            ; Type aliases
            (type_alias_declaration
              name: (type_identifier) @name) @struct

            ; Imports
            (import_statement) @import

            ; Exports (re-exports)
            (export_statement) @export
        "#;

        Query::new(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn rust_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_item
              name: (identifier) @name) @function

            ; Structs
            (struct_item
              name: (type_identifier) @name) @struct

            ; Enums
            (enum_item
              name: (type_identifier) @name) @enum

            ; Traits
            (trait_item
              name: (type_identifier) @name) @trait

            ; Use statements (imports)
            (use_declaration) @import
        "#;

        Query::new(&tree_sitter_rust::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn go_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_declaration
              name: (identifier) @name) @function

            ; Methods
            (method_declaration
              name: (field_identifier) @name) @method

            ; Structs
            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (struct_type))) @struct

            ; Interfaces
            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (interface_type))) @interface

            ; Imports
            (import_declaration) @import
        "#;

        Query::new(&tree_sitter_go::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn java_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Methods
            (method_declaration
              name: (identifier) @name) @method

            ; Classes
            (class_declaration
              name: (identifier) @name) @class

            ; Interfaces
            (interface_declaration
              name: (identifier) @name) @interface

            ; Enums
            (enum_declaration
              name: (identifier) @name) @enum

            ; Imports
            (import_declaration) @import
        "#;

        Query::new(&tree_sitter_java::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // C language queries
    // ==========================================================================

    fn c_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              declarator: (function_declarator
                declarator: (identifier) @name)) @function

            (struct_specifier
              name: (type_identifier) @name) @struct

            (enum_specifier
              name: (type_identifier) @name) @enum
        "#;

        Query::new(&tree_sitter_c::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn c_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              declarator: (function_declarator
                declarator: (identifier) @name)) @function

            ; Structs
            (struct_specifier
              name: (type_identifier) @name) @struct

            ; Enums
            (enum_specifier
              name: (type_identifier) @name) @enum

            ; Includes (imports)
            (preproc_include) @import
        "#;

        Query::new(&tree_sitter_c::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // C++ language queries
    // ==========================================================================

    fn cpp_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              declarator: (function_declarator
                declarator: (identifier) @name)) @function

            (class_specifier
              name: (type_identifier) @name) @class

            (struct_specifier
              name: (type_identifier) @name) @struct

            (enum_specifier
              name: (type_identifier) @name) @enum
        "#;

        Query::new(&tree_sitter_cpp::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn cpp_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              declarator: (function_declarator
                declarator: (identifier) @name)) @function

            ; Classes
            (class_specifier
              name: (type_identifier) @name) @class

            ; Structs
            (struct_specifier
              name: (type_identifier) @name) @struct

            ; Enums
            (enum_specifier
              name: (type_identifier) @name) @enum

            ; Includes (imports)
            (preproc_include) @import
        "#;

        Query::new(&tree_sitter_cpp::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // C# language queries
    // ==========================================================================

    fn csharp_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (method_declaration
              name: (identifier) @name) @method

            (class_declaration
              name: (identifier) @name) @class

            (interface_declaration
              name: (identifier) @name) @interface

            (struct_declaration
              name: (identifier) @name) @struct

            (enum_declaration
              name: (identifier) @name) @enum
        "#;

        Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn csharp_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Methods
            (method_declaration
              name: (identifier) @name) @method

            ; Classes
            (class_declaration
              name: (identifier) @name) @class

            ; Interfaces
            (interface_declaration
              name: (identifier) @name) @interface

            ; Structs
            (struct_declaration
              name: (identifier) @name) @struct

            ; Enums
            (enum_declaration
              name: (identifier) @name) @enum

            ; Imports (using directives)
            (using_directive) @import
        "#;

        Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Ruby language queries
    // ==========================================================================

    fn ruby_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (method
              name: (identifier) @name) @function

            (class
              name: (constant) @name) @class

            (module
              name: (constant) @name) @class
        "#;

        Query::new(&tree_sitter_ruby::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn ruby_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Methods
            (method
              name: (identifier) @name) @function

            ; Classes
            (class
              name: (constant) @name) @class

            ; Modules
            (module
              name: (constant) @name) @class

            ; Requires (imports)
            (call
              method: (identifier) @_method
              (#match? @_method "^require")
              arguments: (argument_list)) @import
        "#;

        Query::new(&tree_sitter_ruby::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Bash language queries
    // ==========================================================================

    fn bash_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              name: (word) @name) @function
        "#;

        Query::new(&tree_sitter_bash::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn bash_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              name: (word) @name) @function

            ; Source commands (imports)
            (command
              name: (command_name) @_cmd
              (#match? @_cmd "^(source|\\.)$")
              argument: (word)) @import
        "#;

        Query::new(&tree_sitter_bash::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // PHP language queries
    // ==========================================================================

    fn php_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              name: (name) @name) @function

            (method_declaration
              name: (name) @name) @method

            (class_declaration
              name: (name) @name) @class

            (interface_declaration
              name: (name) @name) @interface

            (trait_declaration
              name: (name) @name) @trait
        "#;

        Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn php_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              name: (name) @name) @function

            ; Methods
            (method_declaration
              name: (name) @name) @method

            ; Classes
            (class_declaration
              name: (name) @name) @class

            ; Interfaces
            (interface_declaration
              name: (name) @name) @interface

            ; Traits
            (trait_declaration
              name: (name) @name) @trait

            ; Use statements (imports)
            (namespace_use_declaration) @import
        "#;

        Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Kotlin language queries
    // ==========================================================================

    fn kotlin_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (_) @name) @function

            (class_declaration
              name: (_) @name) @class

            (object_declaration
              name: (_) @name) @class
        "#;

        Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn kotlin_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_declaration
              name: (_) @name) @function

            ; Classes
            (class_declaration
              name: (_) @name) @class

            ; Objects
            (object_declaration
              name: (_) @name) @class

            ; Imports
            (import) @import
        "#;

        Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Swift language queries
    // ==========================================================================

    fn swift_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (simple_identifier) @name) @function

            (class_declaration
              declaration_kind: "class"
              name: (type_identifier) @name) @class

            (protocol_declaration
              name: (type_identifier) @name) @interface

            (class_declaration
              declaration_kind: "struct"
              name: (type_identifier) @name) @struct

            (class_declaration
              declaration_kind: "enum"
              name: (type_identifier) @name) @enum
        "#;

        Query::new(&tree_sitter_swift::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn swift_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_declaration
              name: (simple_identifier) @name) @function

            ; Classes
            (class_declaration
              declaration_kind: "class"
              name: (type_identifier) @name) @class

            ; Protocols (interfaces)
            (protocol_declaration
              name: (type_identifier) @name) @interface

            ; Structs
            (class_declaration
              declaration_kind: "struct"
              name: (type_identifier) @name) @struct

            ; Enums
            (class_declaration
              declaration_kind: "enum"
              name: (type_identifier) @name) @enum

            ; Imports
            (import_declaration) @import
        "#;

        Query::new(&tree_sitter_swift::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Scala language queries
    // ==========================================================================

    fn scala_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              name: (identifier) @name) @function

            (class_definition
              name: (identifier) @name) @class

            (object_definition
              name: (identifier) @name) @class

            (trait_definition
              name: (identifier) @name) @trait
        "#;

        Query::new(&tree_sitter_scala::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn scala_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function_definition
              name: (identifier) @name) @function

            ; Classes
            (class_definition
              name: (identifier) @name) @class

            ; Objects
            (object_definition
              name: (identifier) @name) @class

            ; Traits
            (trait_definition
              name: (identifier) @name) @trait

            ; Imports
            (import_declaration) @import
        "#;

        Query::new(&tree_sitter_scala::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Haskell language queries
    // ==========================================================================

    fn haskell_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function
              name: (variable) @name) @function

            (signature
              name: (variable) @name) @function

            (function
              name: (prefix_id) @name) @function

            (signature
              name: (prefix_id) @name) @function

            (newtype
              name: (name) @name) @struct

            (type_synomym
              name: (name) @name) @struct

            (data_type
              name: (name) @name) @enum
        "#;

        Query::new(&tree_sitter_haskell::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn haskell_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (function
              name: (variable) @name) @function

            ; Type signatures
            (signature
              name: (variable) @name) @function

            ; Type aliases
            (function
              name: (prefix_id) @name) @function

            (signature
              name: (prefix_id) @name) @function

            ; Newtypes
            (newtype
              name: (name) @name) @struct

            ; ADTs (data declarations)
            (type_synomym
              name: (name) @name) @struct

            (data_type
              name: (name) @name) @enum

            ; Imports
            (import) @import
        "#;

        Query::new(&tree_sitter_haskell::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Elixir language queries
    // ==========================================================================

    fn elixir_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (call
              target: (identifier) @_type
              (#match? @_type "^(def|defp|defmacro|defmacrop)$")
              (arguments
                (call
                  target: (identifier) @name))) @function

            (call
              target: (identifier) @_type
              (#match? @_type "^defmodule$")
              (arguments
                (alias) @name)) @class
        "#;

        Query::new(&tree_sitter_elixir::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn elixir_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions (def, defp, defmacro)
            (call
              target: (identifier) @_type
              (#match? @_type "^(def|defp|defmacro|defmacrop)$")
              (arguments
                (call
                  target: (identifier) @name))) @function

            ; Modules
            (call
              target: (identifier) @_type
              (#match? @_type "^defmodule$")
              (arguments
                (alias) @name)) @class

            ; Imports (alias, import, use, require)
            (call
              target: (identifier) @_type
              (#match? @_type "^(alias|import|use|require)$")) @import
        "#;

        Query::new(&tree_sitter_elixir::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Clojure language queries
    // ==========================================================================

    fn clojure_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (list_lit
              (sym_lit) @_type
              (#match? @_type "^(defn|defn-|defmacro)$")
              (sym_lit) @name) @function

            (list_lit
              (sym_lit) @_type
              (#match? @_type "^(defrecord|deftype|defprotocol)$")
              (sym_lit) @name) @class
        "#;

        Query::new(&tree_sitter_clojure::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn clojure_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions
            (list_lit
              (sym_lit) @_type
              (#match? @_type "^(defn|defn-|defmacro)$")
              (sym_lit) @name) @function

            ; Records/Types/Protocols
            (list_lit
              (sym_lit) @_type
              (#match? @_type "^(defrecord|deftype|defprotocol)$")
              (sym_lit) @name) @class

            ; Namespace (imports)
            (list_lit
              (sym_lit) @_type
              (#match? @_type "^(ns|require|use|import)$")) @import
        "#;

        Query::new(&tree_sitter_clojure::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // OCaml language queries
    // ==========================================================================

    fn ocaml_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (value_definition
              (let_binding
                pattern: (value_name) @name)) @function

            (type_definition
              (type_binding
                name: (type_constructor) @name)) @struct

            (module_definition
              (module_binding
                name: (module_name) @name)) @class
        "#;

        Query::new(&tree_sitter_ocaml::LANGUAGE_OCAML.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn ocaml_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions (let bindings)
            (value_definition
              (let_binding
                pattern: (value_name) @name)) @function

            ; Types
            (type_definition
              (type_binding
                name: (type_constructor) @name)) @struct

            ; Modules
            (module_definition
              (module_binding
                name: (module_name) @name)) @class

            ; Opens (imports)
            (open_module) @import
        "#;

        Query::new(&tree_sitter_ocaml::LANGUAGE_OCAML.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // Lua language queries
    // ==========================================================================

    fn lua_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (function_declaration
              name: (dot_index_expression) @name) @method

            (function_declaration
              name: (method_index_expression) @name) @method
        "#;

        Query::new(&tree_sitter_lua::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn lua_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Global functions
            (function_declaration
              name: (identifier) @name) @function

            ; Method-like functions
            (function_declaration
              name: (dot_index_expression) @name) @method

            (function_declaration
              name: (method_index_expression) @name) @method

            ; Requires (imports)
            (function_call
              name: (variable
                (identifier) @_func)
              (#eq? @_func "require")
              arguments: (arguments)) @import
        "#;

        Query::new(&tree_sitter_lua::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    // ==========================================================================
    // R language queries
    // ==========================================================================

    fn r_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (binary_operator
              lhs: (identifier) @name
              operator: "<-"
              rhs: (function_definition)) @function

            (binary_operator
              lhs: (identifier) @name
              operator: "="
              rhs: (function_definition)) @function
        "#;

        Query::new(&tree_sitter_r::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn r_super_query() -> Result<Query, ParserError> {
        let query_string = r#"
            ; Functions (left assignment)
            (binary_operator
              lhs: (identifier) @name
              operator: "<-"
              rhs: (function_definition)) @function

            ; Functions (equals assignment)
            (binary_operator
              lhs: (identifier) @name
              operator: "="
              rhs: (function_definition)) @function

            ; Library/require calls (imports)
            (call
              function: (identifier) @_func
              (#match? @_func "^(library|require|source)$")) @import
        "#;

        Query::new(&tree_sitter_r::LANGUAGE.into(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_parse_python() {
        let mut parser = Parser::new();
        let source = r#"
def hello_world():
    """This is a docstring"""
    print("Hello, World!")

class MyClass:
    def method(self, x):
        return x * 2
"#;

        let symbols = parser.parse(source, Language::Python).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "hello_world" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find class
        let class = symbols
            .iter()
            .find(|s| s.name == "MyClass" && s.kind == SymbolKind::Class);
        assert!(class.is_some());

        // Find method
        let method = symbols
            .iter()
            .find(|s| s.name == "method" && s.kind == SymbolKind::Method);
        assert!(method.is_some());
    }

    #[test]
    fn test_parse_rust() {
        let mut parser = Parser::new();
        let source = r#"
/// A test function
fn test_function() -> i32 {
    42
}

struct MyStruct {
    field: i32,
}

enum MyEnum {
    Variant1,
    Variant2,
}
"#;

        let symbols = parser.parse(source, Language::Rust).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "test_function" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find struct
        let struct_sym = symbols
            .iter()
            .find(|s| s.name == "MyStruct" && s.kind == SymbolKind::Struct);
        assert!(struct_sym.is_some());

        // Find enum
        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "MyEnum" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some());
    }

    #[test]
    fn test_parse_javascript() {
        let mut parser = Parser::new();
        let source = r#"
function testFunction() {
    return 42;
}

class TestClass {
    testMethod() {
        return "test";
    }
}

const arrowFunc = () => {
    console.log("arrow");
};
"#;

        let symbols = parser.parse(source, Language::JavaScript).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "testFunction" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find class
        let class = symbols
            .iter()
            .find(|s| s.name == "TestClass" && s.kind == SymbolKind::Class);
        assert!(class.is_some());
    }

    #[test]
    fn test_parse_typescript() {
        let mut parser = Parser::new();
        let source = r#"
interface TestInterface {
    method(): void;
}

enum TestEnum {
    Value1,
    Value2
}

class TestClass implements TestInterface {
    method(): void {
        console.log("test");
    }
}
"#;

        let symbols = parser.parse(source, Language::TypeScript).unwrap();
        assert!(!symbols.is_empty());

        // Find interface
        let interface = symbols
            .iter()
            .find(|s| s.name == "TestInterface" && s.kind == SymbolKind::Interface);
        assert!(interface.is_some());

        // Find enum
        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "TestEnum" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some());
    }

    #[test]
    fn test_symbol_metadata() {
        let mut parser = Parser::new();
        let source = r#"
def test_func(x, y):
    """A test function with params"""
    return x + y
"#;

        let symbols = parser.parse(source, Language::Python).unwrap();
        let func = symbols
            .iter()
            .find(|s| s.name == "test_func")
            .expect("Function not found");

        assert!(func.start_line > 0);
        assert!(func.end_line >= func.start_line);
        assert!(func.signature.is_some());
        assert!(func.docstring.is_some());
    }
}
