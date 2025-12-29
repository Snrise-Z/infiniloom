//! Core type definitions for Infiniloom

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

// Re-export canonical tokenizer types from tokenizer module
pub use crate::tokenizer::{TokenCounts, TokenModel};

/// Backward-compatible alias for TokenModel
///
/// # Important: No Conversion Needed
///
/// `TokenizerModel` and `TokenModel` are the **same type** - this is a type alias,
/// not a separate type. Any function expecting `TokenModel` can directly accept
/// `TokenizerModel` without conversion.
///
/// **Before (incorrect, ~30 lines of duplication)**:
/// ```ignore
/// fn to_token_model(model: TokenizerModel) -> TokenModel {
///     match model {
///         TokenizerModel::Claude => TokenModel::Claude,
///         // ... 26 more identical mappings
///     }
/// }
/// ```
///
/// **After (correct)**:
/// ```ignore
/// // Direct usage - no conversion function needed
/// let tokenizer = Tokenizer::new();
/// tokenizer.count(text, model) // Works with TokenizerModel directly
/// ```
///
/// This alias exists solely for backward compatibility with legacy CLI code that
/// used the name `TokenizerModel`. All new code should prefer `TokenModel`.
///
/// Eliminated in Phase 1 refactoring (Item 2): Removed 193 lines of duplicate
/// conversion functions and tests from pack.rs and diff.rs.
pub type TokenizerModel = TokenModel;

/// A scanned repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// Repository name (usually directory name)
    pub name: String,
    /// Absolute path to repository root
    pub path: PathBuf,
    /// List of files in the repository
    pub files: Vec<RepoFile>,
    /// Repository metadata and statistics
    pub metadata: RepoMetadata,
}

impl Repository {
    /// Create a new empty repository
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            files: Vec::new(),
            metadata: RepoMetadata::default(),
        }
    }

    /// Get total token count for a specific model
    pub fn total_tokens(&self, model: TokenizerModel) -> u32 {
        self.files.iter().map(|f| f.token_count.get(model)).sum()
    }

    /// Get files filtered by language
    pub fn files_by_language(&self, language: &str) -> Vec<&RepoFile> {
        self.files
            .iter()
            .filter(|f| f.language.as_deref() == Some(language))
            .collect()
    }

    /// Get files sorted by importance
    #[must_use]
    pub fn files_by_importance(&self) -> Vec<&RepoFile> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        files
    }
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Repository({}: {} files, {} lines)",
            self.name, self.metadata.total_files, self.metadata.total_lines
        )
    }
}

/// A single file in the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFile {
    /// Absolute path to file
    pub path: PathBuf,
    /// Path relative to repository root
    pub relative_path: String,
    /// Detected programming language
    pub language: Option<String>,
    /// File size in bytes
    pub size_bytes: u64,
    /// Token counts for different models
    pub token_count: TokenCounts,
    /// Extracted symbols (functions, classes, etc.)
    pub symbols: Vec<Symbol>,
    /// Calculated importance score (0.0 - 1.0)
    pub importance: f32,
    /// File content (may be None to save memory)
    pub content: Option<String>,
}

impl RepoFile {
    /// Create a new file entry
    pub fn new(path: impl Into<PathBuf>, relative_path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            relative_path: relative_path.into(),
            language: None,
            size_bytes: 0,
            token_count: TokenCounts::default(),
            symbols: Vec::new(),
            importance: 0.5,
            content: None,
        }
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }

    /// Get filename without path
    #[must_use]
    pub fn filename(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }
}

impl fmt::Display for RepoFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {} tokens)",
            self.relative_path,
            self.language.as_deref().unwrap_or("unknown"),
            self.token_count.claude
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
    Internal, // For languages like C# or package-private in Java
}

impl Visibility {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Internal => "internal",
        }
    }
}

/// A code symbol (function, class, variable, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Function/method signature (if applicable)
    pub signature: Option<String>,
    /// Documentation string
    pub docstring: Option<String>,
    /// Starting line number (1-indexed)
    pub start_line: u32,
    /// Ending line number (1-indexed)
    pub end_line: u32,
    /// Number of references to this symbol
    pub references: u32,
    /// Calculated importance (0.0 - 1.0)
    pub importance: f32,
    /// Parent symbol name (for methods inside classes)
    pub parent: Option<String>,
    /// Visibility modifier (public, private, etc.)
    pub visibility: Visibility,
    /// Function/method calls made by this symbol (callee names)
    pub calls: Vec<String>,
    /// Base class/parent class name (for class inheritance)
    pub extends: Option<String>,
    /// Implemented interfaces/protocols/traits
    pub implements: Vec<String>,
}

impl Symbol {
    /// Create a new symbol
    pub fn new(name: impl Into<String>, kind: SymbolKind) -> Self {
        Self {
            name: name.into(),
            kind,
            signature: None,
            docstring: None,
            start_line: 0,
            end_line: 0,
            references: 0,
            importance: 0.5,
            parent: None,
            visibility: Visibility::default(),
            calls: Vec::new(),
            extends: None,
            implements: Vec::new(),
        }
    }

    /// Get line count
    #[must_use]
    pub fn line_count(&self) -> u32 {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} (lines {}-{})",
            self.kind.name(),
            self.name,
            self.start_line,
            self.end_line
        )
    }
}

/// Kind of code symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Struct,
    Enum,
    Constant,
    Variable,
    Import,
    Export,
    TypeAlias,
    Module,
    Trait,
    Macro,
}

impl SymbolKind {
    /// Get human-readable name
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Import => "import",
            Self::Export => "export",
            Self::TypeAlias => "type",
            Self::Module => "module",
            Self::Trait => "trait",
            Self::Macro => "macro",
        }
    }

    /// Parse from string name (inverse of name())
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "function" => Some(Self::Function),
            "method" => Some(Self::Method),
            "class" => Some(Self::Class),
            "interface" => Some(Self::Interface),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "constant" => Some(Self::Constant),
            "variable" => Some(Self::Variable),
            "import" => Some(Self::Import),
            "export" => Some(Self::Export),
            "type" | "typealias" => Some(Self::TypeAlias),
            "module" => Some(Self::Module),
            "trait" => Some(Self::Trait),
            "macro" => Some(Self::Macro),
            _ => None,
        }
    }
}

impl std::str::FromStr for SymbolKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SymbolKind::from_str(s).ok_or(())
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Repository metadata and statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoMetadata {
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u64,
    /// Aggregate token counts
    pub total_tokens: TokenCounts,
    /// Language breakdown
    pub languages: Vec<LanguageStats>,
    /// Detected framework (e.g., "React", "Django")
    pub framework: Option<String>,
    /// Repository description
    pub description: Option<String>,
    /// Git branch (if in git repo)
    pub branch: Option<String>,
    /// Git commit hash (if in git repo)
    pub commit: Option<String>,
    /// Directory structure tree
    pub directory_structure: Option<String>,
    /// External dependencies (packages/libraries)
    pub external_dependencies: Vec<String>,
    /// Git history (commits and changes) - for structured output
    pub git_history: Option<GitHistory>,
}

/// Statistics for a single language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStats {
    /// Language name
    pub language: String,
    /// Number of files
    pub files: u32,
    /// Total lines in this language
    pub lines: u64,
    /// Percentage of total codebase
    pub percentage: f32,
}

/// A git commit entry for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitInfo {
    /// Full commit hash
    pub hash: String,
    /// Short commit hash (7 chars)
    pub short_hash: String,
    /// Author name
    pub author: String,
    /// Commit date (YYYY-MM-DD)
    pub date: String,
    /// Commit message
    pub message: String,
}

/// Git history information for structured output
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHistory {
    /// Recent commits
    pub commits: Vec<GitCommitInfo>,
    /// Files with uncommitted changes
    pub changed_files: Vec<GitChangedFile>,
}

/// A file with uncommitted changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitChangedFile {
    /// File path relative to repo root
    pub path: String,
    /// Change status (A=Added, M=Modified, D=Deleted, R=Renamed)
    pub status: String,
    /// Diff content (optional, only populated when --include-diffs is used)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_content: Option<String>,
}

/// Compression level for output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression
    None,
    /// Remove empty lines, trim whitespace
    Minimal,
    /// Remove comments, normalize whitespace
    #[default]
    Balanced,
    /// Remove docstrings, keep signatures only
    Aggressive,
    /// Key symbols only
    Extreme,
    /// Focused: key symbols with small surrounding context
    Focused,
    /// Semantic compression using code understanding
    ///
    /// Uses chunk-based compression that:
    /// - Splits content at semantic boundaries (paragraphs, functions)
    /// - Applies budget-ratio-based selection
    /// - When `embeddings` feature is enabled: clusters similar code and keeps representatives
    /// - When disabled: uses heuristic-based sampling
    ///
    /// This provides intelligent compression that preserves code structure better than
    /// character-based approaches, though it's not as sophisticated as full neural
    /// semantic analysis.
    ///
    /// Expected reduction: ~60-70% (may vary based on content structure)
    Semantic,
}

impl CompressionLevel {
    /// Expected reduction percentage
    ///
    /// Note: These are approximate values. Actual reduction depends on:
    /// - Content structure (more repetitive = higher reduction)
    /// - Code density (comments/whitespace ratio)
    /// - For Semantic: whether `embeddings` feature is enabled
    pub fn expected_reduction(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Minimal => 15,
            Self::Balanced => 35,
            Self::Aggressive => 60,
            Self::Extreme => 80,
            Self::Focused => 75,
            // Semantic uses chunk-based compression with ~50% budget ratio default
            // Combined with structure preservation, typically achieves 60-70%
            Self::Semantic => 65,
        }
    }

    /// Get a human-readable description of this compression level
    pub fn description(&self) -> &'static str {
        match self {
            Self::None => "No compression - original content preserved",
            Self::Minimal => "Remove empty lines, trim whitespace",
            Self::Balanced => "Remove comments, normalize whitespace",
            Self::Aggressive => "Remove docstrings, keep signatures only",
            Self::Extreme => "Key symbols only - minimal context",
            Self::Focused => "Focused symbols with small surrounding context",
            Self::Semantic => "Semantic chunking with intelligent sampling",
        }
    }

    /// Parse compression level from string
    ///
    /// Accepts: "none", "minimal", "balanced", "aggressive", "extreme", "semantic"
    /// Case-insensitive. Returns `None` for unrecognized values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "balanced" => Some(Self::Balanced),
            "aggressive" => Some(Self::Aggressive),
            "extreme" => Some(Self::Extreme),
            "focused" => Some(Self::Focused),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }

    /// Get string name of this compression level
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
            Self::Extreme => "extreme",
            Self::Focused => "focused",
            Self::Semantic => "semantic",
        }
    }

    /// Get all available compression levels
    pub fn all() -> &'static [Self] {
        &[
            Self::None,
            Self::Minimal,
            Self::Balanced,
            Self::Aggressive,
            Self::Extreme,
            Self::Focused,
            Self::Semantic,
        ]
    }
}

impl std::str::FromStr for CompressionLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        CompressionLevel::from_str(s).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_new() {
        let repo = Repository::new("test", "/tmp/test");
        assert_eq!(repo.name, "test");
        assert!(repo.files.is_empty());
    }

    #[test]
    fn test_repository_total_tokens() {
        let mut repo = Repository::new("test", "/tmp/test");
        let mut file1 = RepoFile::new("/tmp/test/a.rs", "a.rs");
        file1.token_count.set(TokenizerModel::Claude, 100);
        let mut file2 = RepoFile::new("/tmp/test/b.rs", "b.rs");
        file2.token_count.set(TokenizerModel::Claude, 200);
        repo.files.push(file1);
        repo.files.push(file2);
        assert_eq!(repo.total_tokens(TokenizerModel::Claude), 300);
    }

    #[test]
    fn test_repository_files_by_language() {
        let mut repo = Repository::new("test", "/tmp/test");
        let mut file1 = RepoFile::new("/tmp/test/a.rs", "a.rs");
        file1.language = Some("rust".to_string());
        let mut file2 = RepoFile::new("/tmp/test/b.py", "b.py");
        file2.language = Some("python".to_string());
        let mut file3 = RepoFile::new("/tmp/test/c.rs", "c.rs");
        file3.language = Some("rust".to_string());
        repo.files.push(file1);
        repo.files.push(file2);
        repo.files.push(file3);

        let rust_files = repo.files_by_language("rust");
        assert_eq!(rust_files.len(), 2);
        let python_files = repo.files_by_language("python");
        assert_eq!(python_files.len(), 1);
        let go_files = repo.files_by_language("go");
        assert_eq!(go_files.len(), 0);
    }

    #[test]
    fn test_repository_files_by_importance() {
        let mut repo = Repository::new("test", "/tmp/test");
        let mut file1 = RepoFile::new("/tmp/test/a.rs", "a.rs");
        file1.importance = 0.3;
        let mut file2 = RepoFile::new("/tmp/test/b.rs", "b.rs");
        file2.importance = 0.9;
        let mut file3 = RepoFile::new("/tmp/test/c.rs", "c.rs");
        file3.importance = 0.6;
        repo.files.push(file1);
        repo.files.push(file2);
        repo.files.push(file3);

        let sorted = repo.files_by_importance();
        assert_eq!(sorted[0].relative_path, "b.rs");
        assert_eq!(sorted[1].relative_path, "c.rs");
        assert_eq!(sorted[2].relative_path, "a.rs");
    }

    #[test]
    fn test_repository_display() {
        let mut repo = Repository::new("my-project", "/tmp/my-project");
        repo.metadata.total_files = 42;
        repo.metadata.total_lines = 1000;
        let display = format!("{}", repo);
        assert!(display.contains("my-project"));
        assert!(display.contains("42 files"));
        assert!(display.contains("1000 lines"));
    }

    #[test]
    fn test_repo_file_new() {
        let file = RepoFile::new("/tmp/test/src/main.rs", "src/main.rs");
        assert_eq!(file.relative_path, "src/main.rs");
        assert!(file.language.is_none());
        assert_eq!(file.importance, 0.5);
    }

    #[test]
    fn test_repo_file_extension() {
        let file = RepoFile::new("/tmp/test/main.rs", "main.rs");
        assert_eq!(file.extension(), Some("rs"));

        let file_no_ext = RepoFile::new("/tmp/test/Makefile", "Makefile");
        assert_eq!(file_no_ext.extension(), None);
    }

    #[test]
    fn test_repo_file_filename() {
        let file = RepoFile::new("/tmp/test/src/main.rs", "src/main.rs");
        assert_eq!(file.filename(), "main.rs");
    }

    #[test]
    fn test_repo_file_display() {
        let mut file = RepoFile::new("/tmp/test/main.rs", "main.rs");
        file.language = Some("rust".to_string());
        file.token_count.claude = 150;
        let display = format!("{}", file);
        assert!(display.contains("main.rs"));
        assert!(display.contains("rust"));
        assert!(display.contains("150"));
    }

    #[test]
    fn test_repo_file_display_unknown_language() {
        let file = RepoFile::new("/tmp/test/data.xyz", "data.xyz");
        let display = format!("{}", file);
        assert!(display.contains("unknown"));
    }

    #[test]
    fn test_token_counts() {
        let mut counts = TokenCounts::default();
        counts.set(TokenizerModel::Claude, 100);
        assert_eq!(counts.get(TokenizerModel::Claude), 100);
    }

    #[test]
    fn test_symbol_new() {
        let sym = Symbol::new("my_function", SymbolKind::Function);
        assert_eq!(sym.name, "my_function");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.importance, 0.5);
        assert!(sym.signature.is_none());
        assert!(sym.calls.is_empty());
    }

    #[test]
    fn test_symbol_line_count() {
        let mut sym = Symbol::new("test", SymbolKind::Function);
        sym.start_line = 10;
        sym.end_line = 20;
        assert_eq!(sym.line_count(), 11);
    }

    #[test]
    fn test_symbol_line_count_single_line() {
        let mut sym = Symbol::new("test", SymbolKind::Variable);
        sym.start_line = 5;
        sym.end_line = 5;
        assert_eq!(sym.line_count(), 1);
    }

    #[test]
    fn test_symbol_line_count_inverted() {
        let mut sym = Symbol::new("test", SymbolKind::Variable);
        sym.start_line = 20;
        sym.end_line = 10; // Inverted
        assert_eq!(sym.line_count(), 1);
    }

    #[test]
    fn test_symbol_display() {
        let mut sym = Symbol::new("calculate", SymbolKind::Function);
        sym.start_line = 10;
        sym.end_line = 25;
        let display = format!("{}", sym);
        assert!(display.contains("function"));
        assert!(display.contains("calculate"));
        assert!(display.contains("10-25"));
    }

    #[test]
    fn test_symbol_kind_name() {
        assert_eq!(SymbolKind::Function.name(), "function");
        assert_eq!(SymbolKind::Method.name(), "method");
        assert_eq!(SymbolKind::Class.name(), "class");
        assert_eq!(SymbolKind::Interface.name(), "interface");
        assert_eq!(SymbolKind::Struct.name(), "struct");
        assert_eq!(SymbolKind::Enum.name(), "enum");
        assert_eq!(SymbolKind::Constant.name(), "constant");
        assert_eq!(SymbolKind::Variable.name(), "variable");
        assert_eq!(SymbolKind::Import.name(), "import");
        assert_eq!(SymbolKind::Export.name(), "export");
        assert_eq!(SymbolKind::TypeAlias.name(), "type");
        assert_eq!(SymbolKind::Module.name(), "module");
        assert_eq!(SymbolKind::Trait.name(), "trait");
        assert_eq!(SymbolKind::Macro.name(), "macro");
    }

    #[test]
    fn test_symbol_kind_from_str() {
        assert_eq!(SymbolKind::from_str("function"), Some(SymbolKind::Function));
        assert_eq!(SymbolKind::from_str("method"), Some(SymbolKind::Method));
        assert_eq!(SymbolKind::from_str("class"), Some(SymbolKind::Class));
        assert_eq!(SymbolKind::from_str("interface"), Some(SymbolKind::Interface));
        assert_eq!(SymbolKind::from_str("struct"), Some(SymbolKind::Struct));
        assert_eq!(SymbolKind::from_str("enum"), Some(SymbolKind::Enum));
        assert_eq!(SymbolKind::from_str("constant"), Some(SymbolKind::Constant));
        assert_eq!(SymbolKind::from_str("variable"), Some(SymbolKind::Variable));
        assert_eq!(SymbolKind::from_str("import"), Some(SymbolKind::Import));
        assert_eq!(SymbolKind::from_str("export"), Some(SymbolKind::Export));
        assert_eq!(SymbolKind::from_str("type"), Some(SymbolKind::TypeAlias));
        assert_eq!(SymbolKind::from_str("typealias"), Some(SymbolKind::TypeAlias));
        assert_eq!(SymbolKind::from_str("module"), Some(SymbolKind::Module));
        assert_eq!(SymbolKind::from_str("trait"), Some(SymbolKind::Trait));
        assert_eq!(SymbolKind::from_str("macro"), Some(SymbolKind::Macro));
        // Case insensitive
        assert_eq!(SymbolKind::from_str("FUNCTION"), Some(SymbolKind::Function));
        assert_eq!(SymbolKind::from_str("Class"), Some(SymbolKind::Class));
        // Unknown
        assert_eq!(SymbolKind::from_str("unknown"), None);
        assert_eq!(SymbolKind::from_str(""), None);
    }

    #[test]
    fn test_symbol_kind_std_from_str() {
        use std::str::FromStr;
        assert_eq!("function".parse::<SymbolKind>(), Ok(SymbolKind::Function));
        assert_eq!("class".parse::<SymbolKind>(), Ok(SymbolKind::Class));
        assert!("invalid".parse::<SymbolKind>().is_err());
    }

    #[test]
    fn test_symbol_kind_display() {
        assert_eq!(format!("{}", SymbolKind::Function), "function");
        assert_eq!(format!("{}", SymbolKind::Class), "class");
    }

    #[test]
    fn test_visibility_name() {
        assert_eq!(Visibility::Public.name(), "public");
        assert_eq!(Visibility::Private.name(), "private");
        assert_eq!(Visibility::Protected.name(), "protected");
        assert_eq!(Visibility::Internal.name(), "internal");
    }

    #[test]
    fn test_visibility_default() {
        let vis = Visibility::default();
        assert_eq!(vis, Visibility::Public);
    }

    #[test]
    fn test_language_stats() {
        let stats = LanguageStats {
            language: "rust".to_string(),
            files: 10,
            lines: 5000,
            percentage: 45.5,
        };
        assert_eq!(stats.language, "rust");
        assert_eq!(stats.files, 10);
        assert_eq!(stats.lines, 5000);
        assert!((stats.percentage - 45.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_git_commit_info() {
        let commit = GitCommitInfo {
            hash: "abc123def456".to_string(),
            short_hash: "abc123d".to_string(),
            author: "Test Author".to_string(),
            date: "2025-01-01".to_string(),
            message: "Test commit".to_string(),
        };
        assert_eq!(commit.hash, "abc123def456");
        assert_eq!(commit.short_hash, "abc123d");
        assert_eq!(commit.author, "Test Author");
    }

    #[test]
    fn test_git_changed_file() {
        let changed = GitChangedFile {
            path: "src/main.rs".to_string(),
            status: "M".to_string(),
            diff_content: Some("+new line".to_string()),
        };
        assert_eq!(changed.path, "src/main.rs");
        assert_eq!(changed.status, "M");
        assert!(changed.diff_content.is_some());
    }

    #[test]
    fn test_git_history_default() {
        let history = GitHistory::default();
        assert!(history.commits.is_empty());
        assert!(history.changed_files.is_empty());
    }

    #[test]
    fn test_repo_metadata_default() {
        let meta = RepoMetadata::default();
        assert_eq!(meta.total_files, 0);
        assert_eq!(meta.total_lines, 0);
        assert!(meta.languages.is_empty());
        assert!(meta.framework.is_none());
        assert!(meta.branch.is_none());
    }

    #[test]
    fn test_compression_level_from_str() {
        assert_eq!(CompressionLevel::from_str("none"), Some(CompressionLevel::None));
        assert_eq!(CompressionLevel::from_str("minimal"), Some(CompressionLevel::Minimal));
        assert_eq!(CompressionLevel::from_str("balanced"), Some(CompressionLevel::Balanced));
        assert_eq!(CompressionLevel::from_str("aggressive"), Some(CompressionLevel::Aggressive));
        assert_eq!(CompressionLevel::from_str("extreme"), Some(CompressionLevel::Extreme));
        assert_eq!(CompressionLevel::from_str("focused"), Some(CompressionLevel::Focused));
        assert_eq!(CompressionLevel::from_str("semantic"), Some(CompressionLevel::Semantic));

        // Case insensitive
        assert_eq!(CompressionLevel::from_str("SEMANTIC"), Some(CompressionLevel::Semantic));
        assert_eq!(CompressionLevel::from_str("Balanced"), Some(CompressionLevel::Balanced));

        // Unknown
        assert_eq!(CompressionLevel::from_str("unknown"), None);
        assert_eq!(CompressionLevel::from_str(""), None);
    }

    #[test]
    fn test_compression_level_std_from_str() {
        use std::str::FromStr;
        assert_eq!("balanced".parse::<CompressionLevel>(), Ok(CompressionLevel::Balanced));
        assert!("invalid".parse::<CompressionLevel>().is_err());
    }

    #[test]
    fn test_compression_level_name() {
        assert_eq!(CompressionLevel::None.name(), "none");
        assert_eq!(CompressionLevel::Semantic.name(), "semantic");
    }

    #[test]
    fn test_compression_level_expected_reduction() {
        assert_eq!(CompressionLevel::None.expected_reduction(), 0);
        assert_eq!(CompressionLevel::Minimal.expected_reduction(), 15);
        assert_eq!(CompressionLevel::Balanced.expected_reduction(), 35);
        assert_eq!(CompressionLevel::Aggressive.expected_reduction(), 60);
        assert_eq!(CompressionLevel::Extreme.expected_reduction(), 80);
        assert_eq!(CompressionLevel::Focused.expected_reduction(), 75);
        assert_eq!(CompressionLevel::Semantic.expected_reduction(), 65);
    }

    #[test]
    fn test_compression_level_description() {
        // All levels should have non-empty descriptions
        for level in CompressionLevel::all() {
            assert!(!level.description().is_empty());
        }
    }

    #[test]
    fn test_compression_level_all() {
        let all = CompressionLevel::all();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&CompressionLevel::Semantic));
    }

    #[test]
    fn test_compression_level_default() {
        let level = CompressionLevel::default();
        assert_eq!(level, CompressionLevel::Balanced);
    }
}
