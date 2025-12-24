//! Core parser implementation for symbol extraction
//!
//! This module contains the main Parser struct and symbol extraction logic.
//! Language definitions and queries are in separate modules for better organization.

use super::extraction;
use super::init;
use super::language::Language;
use super::query_builder;
use crate::types::{Symbol, SymbolKind};
use std::collections::HashMap;
use thiserror::Error;
use tree_sitter::{Parser as TSParser, Query, QueryCursor, StreamingIterator, Tree};

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
                    init::python()?,
                    query_builder::python_query()?,
                    query_builder::python_super_query()?,
                ),
                Language::JavaScript => (
                    init::javascript()?,
                    query_builder::javascript_query()?,
                    query_builder::javascript_super_query()?,
                ),
                Language::TypeScript => (
                    init::typescript()?,
                    query_builder::typescript_query()?,
                    query_builder::typescript_super_query()?,
                ),
                Language::Rust => (
                    init::rust()?,
                    query_builder::rust_query()?,
                    query_builder::rust_super_query()?,
                ),
                Language::Go => {
                    (init::go()?, query_builder::go_query()?, query_builder::go_super_query()?)
                },
                Language::Java => (
                    init::java()?,
                    query_builder::java_query()?,
                    query_builder::java_super_query()?,
                ),
                Language::C => {
                    (init::c()?, query_builder::c_query()?, query_builder::c_super_query()?)
                },
                Language::Cpp => {
                    (init::cpp()?, query_builder::cpp_query()?, query_builder::cpp_super_query()?)
                },
                Language::CSharp => (
                    init::csharp()?,
                    query_builder::csharp_query()?,
                    query_builder::csharp_super_query()?,
                ),
                Language::Ruby => (
                    init::ruby()?,
                    query_builder::ruby_query()?,
                    query_builder::ruby_super_query()?,
                ),
                Language::Bash => (
                    init::bash()?,
                    query_builder::bash_query()?,
                    query_builder::bash_super_query()?,
                ),
                Language::Php => {
                    (init::php()?, query_builder::php_query()?, query_builder::php_super_query()?)
                },
                Language::Kotlin => (
                    init::kotlin()?,
                    query_builder::kotlin_query()?,
                    query_builder::kotlin_super_query()?,
                ),
                Language::Swift => (
                    init::swift()?,
                    query_builder::swift_query()?,
                    query_builder::swift_super_query()?,
                ),
                Language::Scala => (
                    init::scala()?,
                    query_builder::scala_query()?,
                    query_builder::scala_super_query()?,
                ),
                Language::Haskell => (
                    init::haskell()?,
                    query_builder::haskell_query()?,
                    query_builder::haskell_super_query()?,
                ),
                Language::Elixir => (
                    init::elixir()?,
                    query_builder::elixir_query()?,
                    query_builder::elixir_super_query()?,
                ),
                Language::Clojure => (
                    init::clojure()?,
                    query_builder::clojure_query()?,
                    query_builder::clojure_super_query()?,
                ),
                Language::OCaml => (
                    init::ocaml()?,
                    query_builder::ocaml_query()?,
                    query_builder::ocaml_super_query()?,
                ),
                Language::FSharp => {
                    return Err(ParserError::UnsupportedLanguage(
                        "F# not yet supported (no tree-sitter grammar available)".to_owned(),
                    ));
                },
                Language::Lua => {
                    (init::lua()?, query_builder::lua_query()?, query_builder::lua_super_query()?)
                },
                Language::R => {
                    (init::r()?, query_builder::r_query()?, query_builder::r_super_query()?)
                },
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
        let mut symbol_kind = extraction::map_symbol_kind(kind_name);

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
        let signature = extraction::extract_signature(def_node, source_code, language);
        let docstring = extraction::extract_docstring(def_node, source_code, language);
        let parent = if symbol_kind == SymbolKind::Method {
            extraction::extract_parent(def_node, source_code)
        } else {
            None
        };
        let visibility = extraction::extract_visibility(def_node, source_code, language);
        let calls = if matches!(symbol_kind, SymbolKind::Function | SymbolKind::Method) {
            extraction::extract_calls(def_node, source_code, language)
        } else {
            Vec::new()
        };

        // Extract inheritance info for classes, structs, interfaces
        let (extends, implements) = if matches!(
            symbol_kind,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface
        ) {
            extraction::extract_inheritance(def_node, source_code, language)
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
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

// NOTE: The following extraction functions have been moved to super::extraction module:
// - extract_signature
// - extract_docstring
// - extract_parent
// - extract_visibility
// - extract_calls
// - find_body_node
// - collect_calls_recursive
// - is_builtin
// - clean_jsdoc
// - clean_javadoc
// - extract_inheritance
// - map_symbol_kind

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
