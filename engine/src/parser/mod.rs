//! Tree-sitter based code parser for extracting symbols from source files
//!
//! This module provides a unified interface for parsing source code across
//! multiple programming languages and extracting symbols (functions, classes,
//! methods, structs, enums, etc.) with their metadata.
//!
//! # Module Structure
//!
//! - [`core`] - Main Parser struct and symbol extraction logic
//! - [`language`] - Language enum and support utilities
//! - [`queries`] - Tree-sitter query strings for all languages
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

// Sub-modules
mod core;
pub mod extraction;
pub mod init;
pub mod language;
pub mod queries;
pub mod query_builder;
pub mod thread_local;

// Re-export core parser functionality
pub use core::{Parser, ParserError};

// Re-export Language from language module (new location)
// For backward compatibility, also keep it accessible from core
pub use language::{detect_file_language, Language};

// Re-export optimized thread-local parser API
pub use thread_local::{parse_file_symbols, parse_with_language};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = Parser::new();
        // Parser should be created without errors
        drop(parser);
    }

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("unknown"), None);
    }
}
