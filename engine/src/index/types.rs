//! Core data structures for the Git context index.
//!
//! This module defines the types used to build and query a pre-computed
//! index of symbols, files, and their relationships for fast diff context.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;

/// Type-safe symbol ID to prevent mixing with other integer types.
/// Use `SymbolId::new()` to create and `id.0` or `id.as_u32()` to access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(transparent)]
pub struct SymbolId(pub u32);

impl SymbolId {
    /// Create a new SymbolId
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the underlying u32 value
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for SymbolId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<SymbolId> for u32 {
    #[inline]
    fn from(id: SymbolId) -> Self {
        id.0
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sym#{}", self.0)
    }
}

/// Type-safe file ID to prevent mixing with other integer types.
/// Use `FileId::new()` to create and `id.0` or `id.as_u32()` to access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(transparent)]
pub struct FileId(pub u32);

impl FileId {
    /// Create a new FileId
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the underlying u32 value
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for FileId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<FileId> for u32 {
    #[inline]
    fn from(id: FileId) -> Self {
        id.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file#{}", self.0)
    }
}

/// A symbol in the index with unique ID for graph operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSymbol {
    /// Unique symbol ID within this index
    pub id: SymbolId,
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: IndexSymbolKind,
    /// File ID containing this symbol
    pub file_id: FileId,
    /// Source span (line/column positions)
    pub span: Span,
    /// Full signature for functions/methods
    pub signature: Option<String>,
    /// Parent symbol ID (for methods inside classes)
    pub parent: Option<SymbolId>,
    /// Visibility modifier
    pub visibility: Visibility,
    /// Documentation string
    pub docstring: Option<String>,
}

/// Symbol kind for the index (extended from core SymbolKind)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndexSymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Constant,
    Variable,
    Module,
    Import,
    Export,
    TypeAlias,
    Macro,
}

impl IndexSymbolKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Module => "module",
            Self::Import => "import",
            Self::Export => "export",
            Self::TypeAlias => "type",
            Self::Macro => "macro",
        }
    }

    /// Check if this symbol kind defines a scope (can contain other symbols)
    pub fn is_scope(&self) -> bool {
        matches!(
            self,
            Self::Class | Self::Struct | Self::Interface | Self::Trait | Self::Module | Self::Enum
        )
    }
}

/// Visibility modifier for symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,
}

/// Source code span (start and end positions)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u16,
    pub end_line: u32,
    pub end_col: u16,
}

impl Span {
    pub fn new(start_line: u32, start_col: u16, end_line: u32, end_col: u16) -> Self {
        Self { start_line, start_col, end_line, end_col }
    }

    /// Check if a line falls within this span
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Number of lines in this span
    pub fn line_count(&self) -> u32 {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
}

/// A file entry in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Unique file ID
    pub id: FileId,
    /// Relative path from repository root
    pub path: String,
    /// Detected language
    pub language: Language,
    /// BLAKE3 content hash for change detection
    pub content_hash: [u8; 32],
    /// Index range into the symbols vector (raw u32 for Range compatibility)
    pub symbols: Range<u32>,
    /// Import statements in this file
    pub imports: Vec<Import>,
    /// Number of lines
    pub lines: u32,
    /// Pre-computed token count (Claude model)
    pub tokens: u32,
}

/// Detected programming language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
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
    Lua,
    R,
    #[default]
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" | "pyi" | "pyw" => Self::Python,
            "js" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" | "jsx" => Self::TypeScript,
            "go" => Self::Go,
            "java" => Self::Java,
            "c" | "h" => Self::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Self::Cpp,
            "cs" => Self::CSharp,
            "rb" => Self::Ruby,
            "sh" | "bash" | "zsh" => Self::Bash,
            "php" | "php3" | "php4" | "php5" | "phtml" => Self::Php,
            "kt" | "kts" => Self::Kotlin,
            "swift" => Self::Swift,
            "scala" | "sc" => Self::Scala,
            "hs" | "lhs" => Self::Haskell,
            "ex" | "exs" => Self::Elixir,
            "clj" | "cljs" | "cljc" | "edn" => Self::Clojure,
            "ml" | "mli" => Self::OCaml,
            "lua" => Self::Lua,
            "r" | "rmd" => Self::R,
            _ => Self::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
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
            Self::Lua => "lua",
            Self::R => "r",
            Self::Unknown => "unknown",
        }
    }
}

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// Source path or module name (e.g., "src/utils" or "lodash")
    pub source: String,
    /// Resolved file ID if it's an internal import
    pub resolved_file: Option<u32>,
    /// Specific symbols imported (empty for wildcard imports)
    pub symbols: Vec<String>,
    /// Source span
    pub span: Span,
    /// Whether this is an external dependency
    pub is_external: bool,
}

/// Main symbol index structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIndex {
    /// Index version (for compatibility checking)
    pub version: u32,
    /// Repository name
    pub repo_name: String,
    /// Git commit hash when index was built
    pub commit_hash: Option<String>,
    /// Timestamp of index creation
    pub created_at: u64,
    /// All files in the repository
    pub files: Vec<FileEntry>,
    /// All symbols across all files
    pub symbols: Vec<IndexSymbol>,

    // Lookup tables (built on load, not serialized)
    #[serde(skip)]
    pub file_by_path: HashMap<String, u32>,
    #[serde(skip)]
    pub symbols_by_name: HashMap<String, Vec<u32>>,
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolIndex {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            repo_name: String::new(),
            commit_hash: None,
            created_at: 0,
            files: Vec::new(),
            symbols: Vec::new(),
            file_by_path: HashMap::new(),
            symbols_by_name: HashMap::new(),
        }
    }

    /// Rebuild lookup tables after deserialization
    pub fn rebuild_lookups(&mut self) {
        self.file_by_path.clear();
        self.symbols_by_name.clear();

        for file in &self.files {
            self.file_by_path
                .insert(file.path.clone(), file.id.as_u32());
        }

        for symbol in &self.symbols {
            self.symbols_by_name
                .entry(symbol.name.clone())
                .or_default()
                .push(symbol.id.as_u32());
        }
    }

    /// Get file by path
    pub fn get_file(&self, path: &str) -> Option<&FileEntry> {
        self.file_by_path
            .get(path)
            .and_then(|&id| self.files.get(id as usize))
    }

    /// Get file by ID
    pub fn get_file_by_id(&self, id: u32) -> Option<&FileEntry> {
        self.files.get(id as usize)
    }

    /// Get symbol by ID
    pub fn get_symbol(&self, id: u32) -> Option<&IndexSymbol> {
        self.symbols.get(id as usize)
    }

    /// Get all symbols in a file
    pub fn get_file_symbols(&self, file_id: FileId) -> &[IndexSymbol] {
        if let Some(file) = self.get_file_by_id(file_id.as_u32()) {
            &self.symbols[file.symbols.start as usize..file.symbols.end as usize]
        } else {
            &[]
        }
    }

    /// Find symbols by name
    pub fn find_symbols(&self, name: &str) -> Vec<&IndexSymbol> {
        self.symbols_by_name
            .get(name)
            .map(|ids| ids.iter().filter_map(|&id| self.get_symbol(id)).collect())
            .unwrap_or_default()
    }

    /// Find symbol containing a specific line in a file
    pub fn find_symbol_at_line(&self, file_id: FileId, line: u32) -> Option<&IndexSymbol> {
        self.get_file_symbols(file_id)
            .iter()
            .filter(|s| s.span.contains_line(line))
            // Return the innermost (smallest) symbol containing the line
            .min_by_key(|s| s.span.line_count())
    }
}

/// Dependency graph for impact analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepGraph {
    // Forward edges: X depends on Y
    /// File imports: (file_id, imported_file_id)
    pub file_imports: Vec<(u32, u32)>,
    /// Symbol references: (symbol_id, referenced_symbol_id)
    pub symbol_refs: Vec<(u32, u32)>,

    // Reverse edges: Y is depended on by X
    /// File imported by: (file_id, importing_file_id)
    pub file_imported_by: Vec<(u32, u32)>,
    /// Symbol referenced by: (symbol_id, referencing_symbol_id)
    pub symbol_ref_by: Vec<(u32, u32)>,

    // Call graph
    /// Function calls: (caller_symbol_id, callee_symbol_id)
    pub calls: Vec<(u32, u32)>,
    /// Called by: (callee_symbol_id, caller_symbol_id)
    pub called_by: Vec<(u32, u32)>,

    // Pre-computed metrics
    /// PageRank importance score per file
    pub file_pagerank: Vec<f32>,
    /// PageRank importance score per symbol
    pub symbol_pagerank: Vec<f32>,
}

impl DepGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file import edge
    pub fn add_file_import(&mut self, from_file: u32, to_file: u32) {
        self.file_imports.push((from_file, to_file));
        self.file_imported_by.push((to_file, from_file));
    }

    /// Add a symbol reference edge
    pub fn add_symbol_ref(&mut self, from_symbol: u32, to_symbol: u32) {
        self.symbol_refs.push((from_symbol, to_symbol));
        self.symbol_ref_by.push((to_symbol, from_symbol));
    }

    /// Add a function call edge
    pub fn add_call(&mut self, caller: u32, callee: u32) {
        self.calls.push((caller, callee));
        self.called_by.push((callee, caller));
    }

    /// Get files that import a given file
    pub fn get_importers(&self, file_id: u32) -> Vec<u32> {
        self.file_imported_by
            .iter()
            .filter_map(|&(f, importer)| if f == file_id { Some(importer) } else { None })
            .collect()
    }

    /// Get files that a given file imports
    pub fn get_imports(&self, file_id: u32) -> Vec<u32> {
        self.file_imports
            .iter()
            .filter_map(|&(f, imported)| if f == file_id { Some(imported) } else { None })
            .collect()
    }

    /// Get symbols that reference a given symbol
    pub fn get_referencers(&self, symbol_id: u32) -> Vec<u32> {
        self.symbol_ref_by
            .iter()
            .filter_map(|&(s, referencer)| {
                if s == symbol_id {
                    Some(referencer)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get callers of a function
    pub fn get_callers(&self, symbol_id: u32) -> Vec<u32> {
        self.called_by
            .iter()
            .filter_map(|&(callee, caller)| {
                if callee == symbol_id {
                    Some(caller)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get callees of a function
    pub fn get_callees(&self, symbol_id: u32) -> Vec<u32> {
        self.calls
            .iter()
            .filter_map(|&(caller, callee)| {
                if caller == symbol_id {
                    Some(callee)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// A reference to a symbol (for tracking usages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Referenced symbol ID
    pub symbol_id: u32,
    /// File containing the reference
    pub file_id: u32,
    /// Location of the reference
    pub span: Span,
    /// Kind of reference
    pub kind: RefKind,
}

/// Kind of reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefKind {
    /// Function/method call
    Call,
    /// Variable read
    Read,
    /// Variable write
    Write,
    /// Import statement
    Import,
    /// Type annotation
    TypeRef,
    /// Class inheritance
    Inheritance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_contains_line() {
        let span = Span::new(10, 0, 20, 0);
        assert!(span.contains_line(10));
        assert!(span.contains_line(15));
        assert!(span.contains_line(20));
        assert!(!span.contains_line(9));
        assert!(!span.contains_line(21));
    }

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    }

    #[test]
    fn test_symbol_index_lookups() {
        let mut index = SymbolIndex::new();
        index.files.push(FileEntry {
            id: FileId::new(0),
            path: "src/main.rs".to_owned(),
            language: Language::Rust,
            content_hash: [0; 32],
            symbols: 0..2,
            imports: vec![],
            lines: 100,
            tokens: 500,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(0),
            name: "main".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(1, 0, 10, 0),
            signature: Some("fn main()".to_owned()),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(1),
            name: "helper".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(15, 0, 25, 0),
            signature: Some("fn helper()".to_owned()),
            parent: None,
            visibility: Visibility::Private,
            docstring: None,
        });

        index.rebuild_lookups();

        assert!(index.get_file("src/main.rs").is_some());
        assert!(index.get_file("nonexistent.rs").is_none());

        let main_symbols = index.find_symbols("main");
        assert_eq!(main_symbols.len(), 1);
        assert_eq!(main_symbols[0].name, "main");

        let symbol = index.find_symbol_at_line(FileId::new(0), 5);
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "main");

        let symbol = index.find_symbol_at_line(FileId::new(0), 20);
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "helper");
    }

    #[test]
    fn test_dep_graph() {
        let mut graph = DepGraph::new();
        graph.add_file_import(0, 1);
        graph.add_file_import(0, 2);
        graph.add_file_import(1, 2);

        assert_eq!(graph.get_imports(0), vec![1, 2]);
        assert_eq!(graph.get_importers(2), vec![0, 1]);

        graph.add_call(10, 20);
        graph.add_call(10, 21);

        assert_eq!(graph.get_callees(10), vec![20, 21]);
        assert_eq!(graph.get_callers(20), vec![10]);
    }
}
