//! Types for index building.
//!
//! Contains error types, build options, and intermediate structures.

use crate::parser::Parser;
use crate::types::Visibility as TypesVisibility;
use crate::SymbolKind;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use thiserror::Error;

use super::super::types::{Import, Language};

// Thread-local parser storage to avoid re-initialization
thread_local! {
    pub(super) static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Errors that can occur during index building
#[derive(Error, Debug)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {file}: {message}")]
    Parse { file: String, message: String },

    #[error("Repository not found: {0}")]
    RepoNotFound(PathBuf),

    #[error("Git error: {0}")]
    Git(String),
}

pub(super) static IDENT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("IDENT_RE: invalid regex pattern"));

pub(super) static COMMON_KEYWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "if",
        "else",
        "for",
        "while",
        "return",
        "break",
        "continue",
        "match",
        "case",
        "switch",
        "default",
        "try",
        "catch",
        "throw",
        "finally",
        "yield",
        "await",
        "async",
        "new",
        "in",
        "of",
        "do",
        "fn",
        "function",
        "def",
        "class",
        "struct",
        "enum",
        "trait",
        "interface",
        "type",
        "impl",
        "let",
        "var",
        "const",
        "static",
        "public",
        "private",
        "protected",
        "internal",
        "use",
        "import",
        "from",
        "package",
        "module",
        "export",
        "super",
        "self",
        "this",
        "crate",
        "pub",
        "mod",
    ]
    .into_iter()
    .collect()
});

/// Options for index building
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Maximum file size to index (in bytes)
    pub max_file_size: u64,
    /// File extensions to include (empty = all supported)
    pub include_extensions: Vec<String>,
    /// Directories to exclude
    pub exclude_dirs: Vec<String>,
    /// Whether to compute PageRank
    pub compute_pagerank: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            include_extensions: vec![],
            exclude_dirs: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "build".into(),
                "dist".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
            ],
            compute_pagerank: true,
        }
    }
}

/// Intermediate parsed file structure
pub(super) struct ParsedFile {
    pub path: String,
    pub language: Language,
    pub content_hash: [u8; 32],
    pub lines: u32,
    pub tokens: u32,
    pub symbols: Vec<ParsedSymbol>,
    pub imports: Vec<Import>,
}

/// Intermediate parsed symbol
pub(super) struct ParsedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub parent: Option<String>,
    pub visibility: TypesVisibility,
    pub calls: Vec<String>,
}
