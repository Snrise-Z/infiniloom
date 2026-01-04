//! Core types for the analysis module
//!
//! These types support all 21 languages: Python, JavaScript, TypeScript, Rust, Go, Java,
//! C, C++, C#, Ruby, Bash, PHP, Kotlin, Swift, Scala, Haskell, Elixir, Clojure, OCaml, Lua, R

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full type signature with parameters, return type, generics, and throws
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeSignature {
    /// Function/method parameters with full type information
    pub parameters: Vec<ParameterInfo>,
    /// Return type (None for void/unit)
    pub return_type: Option<TypeInfo>,
    /// Generic type parameters (e.g., <T, U: Clone>)
    pub generics: Vec<GenericParam>,
    /// Exceptions/errors that can be thrown
    pub throws: Vec<String>,
    /// Whether the function is async
    pub is_async: bool,
    /// Whether the function is a generator/iterator
    pub is_generator: bool,
    /// Receiver type for methods (self, &self, &mut self, etc.)
    pub receiver: Option<String>,
}

/// Parameter information with type details
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ParameterInfo {
    /// Parameter name
    pub name: String,
    /// Type annotation (if available)
    pub type_info: Option<TypeInfo>,
    /// Whether the parameter is optional
    pub is_optional: bool,
    /// Default value expression (if any)
    pub default_value: Option<String>,
    /// Whether this is a rest/variadic parameter
    pub is_variadic: bool,
    /// Parameter kind (positional, keyword, etc.)
    pub kind: ParameterKind,
}

/// Kind of parameter
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParameterKind {
    #[default]
    Positional,
    Keyword,
    PositionalOrKeyword,
    KeywordOnly,
    VarPositional, // *args in Python
    VarKeyword,    // **kwargs in Python
}

/// Type information
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeInfo {
    /// The type name (e.g., "String", "Vec<T>", "int")
    pub name: String,
    /// Generic arguments (e.g., for Vec<String>, this would be ["String"])
    pub generic_args: Vec<TypeInfo>,
    /// Whether this is a nullable/optional type
    pub is_nullable: bool,
    /// Whether this is a reference type (&, &mut in Rust)
    pub is_reference: bool,
    /// Whether this is mutable
    pub is_mutable: bool,
    /// Array dimensions (0 for non-arrays)
    pub array_dimensions: u32,
    /// Union types (for TypeScript unions, Python Union, etc.)
    pub union_types: Vec<TypeInfo>,
}

/// Generic type parameter
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GenericParam {
    /// Parameter name (e.g., "T")
    pub name: String,
    /// Constraints/bounds (e.g., "Clone + Send" in Rust, "extends Comparable" in Java)
    pub constraints: Vec<String>,
    /// Default type (if any)
    pub default_type: Option<String>,
    /// Variance (covariant, contravariant, invariant)
    pub variance: Variance,
}

/// Type variance for generics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Variance {
    #[default]
    Invariant,
    Covariant,     // out in Kotlin, + in Scala
    Contravariant, // in in Kotlin, - in Scala
}

/// Type hierarchy information for a symbol
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeHierarchy {
    /// The symbol this hierarchy is for
    pub symbol_name: String,
    /// Direct parent class/struct (extends)
    pub extends: Option<String>,
    /// Interfaces/traits implemented
    pub implements: Vec<String>,
    /// Full ancestor chain (parent, grandparent, etc.)
    pub ancestors: Vec<AncestorInfo>,
    /// Known descendants (classes that extend this)
    pub descendants: Vec<String>,
    /// Mixins/traits included (Ruby, Scala, etc.)
    pub mixins: Vec<String>,
}

/// Information about an ancestor in the type hierarchy
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AncestorInfo {
    /// Name of the ancestor
    pub name: String,
    /// Whether this is a class or interface/trait
    pub kind: AncestorKind,
    /// Depth in the hierarchy (1 = direct parent)
    pub depth: u32,
    /// File where the ancestor is defined (if known)
    pub file_path: Option<String>,
}

/// Kind of ancestor
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AncestorKind {
    #[default]
    Class,
    Interface,
    Trait,
    Protocol, // Swift
    Mixin,
    AbstractClass,
}

/// Structured documentation extracted from docstrings/comments
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Documentation {
    /// Brief summary (first line/sentence)
    pub summary: Option<String>,
    /// Full description
    pub description: Option<String>,
    /// Parameter documentation
    pub params: Vec<ParamDoc>,
    /// Return value documentation
    pub returns: Option<ReturnDoc>,
    /// Exception/error documentation
    pub throws: Vec<ThrowsDoc>,
    /// Code examples
    pub examples: Vec<Example>,
    /// Other tags (@deprecated, @since, @see, etc.)
    pub tags: HashMap<String, Vec<String>>,
    /// Whether the symbol is deprecated
    pub is_deprecated: bool,
    /// Deprecation message
    pub deprecation_message: Option<String>,
    /// Raw docstring before parsing
    pub raw: Option<String>,
}

/// Parameter documentation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ParamDoc {
    /// Parameter name
    pub name: String,
    /// Type (from documentation, may differ from actual type)
    pub type_info: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Whether marked as optional in docs
    pub is_optional: bool,
    /// Default value mentioned in docs
    pub default_value: Option<String>,
}

/// Return value documentation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReturnDoc {
    /// Return type (from documentation)
    pub type_info: Option<String>,
    /// Description of return value
    pub description: Option<String>,
}

/// Exception/error documentation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThrowsDoc {
    /// Exception/error type
    pub exception_type: String,
    /// When this exception is thrown
    pub description: Option<String>,
}

/// Code example from documentation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Example {
    /// Example title/description
    pub title: Option<String>,
    /// The code
    pub code: String,
    /// Language hint for syntax highlighting
    pub language: Option<String>,
    /// Expected output (for doctests)
    pub expected_output: Option<String>,
}

/// Code complexity metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ComplexityMetrics {
    /// Cyclomatic complexity (number of independent paths)
    pub cyclomatic: u32,
    /// Cognitive complexity (how hard to understand)
    pub cognitive: u32,
    /// Halstead metrics
    pub halstead: Option<HalsteadMetrics>,
    /// Lines of code metrics
    pub loc: LocMetrics,
    /// Maintainability index (0-100, higher is better)
    pub maintainability_index: Option<f32>,
    /// Nesting depth
    pub max_nesting_depth: u32,
    /// Number of parameters
    pub parameter_count: u32,
    /// Number of return points
    pub return_count: u32,
}

/// Halstead complexity metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HalsteadMetrics {
    /// Number of distinct operators
    pub distinct_operators: u32,
    /// Number of distinct operands
    pub distinct_operands: u32,
    /// Total operators
    pub total_operators: u32,
    /// Total operands
    pub total_operands: u32,
    /// Program vocabulary (n1 + n2)
    pub vocabulary: u32,
    /// Program length (N1 + N2)
    pub length: u32,
    /// Calculated program length
    pub calculated_length: f32,
    /// Volume
    pub volume: f32,
    /// Difficulty
    pub difficulty: f32,
    /// Effort
    pub effort: f32,
    /// Time to program (seconds)
    pub time: f32,
    /// Estimated bugs
    pub bugs: f32,
}

/// Lines of code metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LocMetrics {
    /// Total lines
    pub total: u32,
    /// Source lines of code (non-blank, non-comment)
    pub source: u32,
    /// Comment lines
    pub comments: u32,
    /// Blank lines
    pub blank: u32,
}

/// Dead code detection result
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeadCodeInfo {
    /// Unused public exports
    pub unused_exports: Vec<UnusedExport>,
    /// Unreachable code segments
    pub unreachable_code: Vec<UnreachableCode>,
    /// Unused private symbols
    pub unused_private: Vec<UnusedSymbol>,
    /// Unused imports
    pub unused_imports: Vec<UnusedImport>,
    /// Unused variables
    pub unused_variables: Vec<UnusedVariable>,
}

/// An unused export
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnusedExport {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File path
    pub file_path: String,
    /// Line number
    pub line: u32,
    /// Confidence level (0.0-1.0)
    pub confidence: f32,
    /// Reason why it's considered unused
    pub reason: String,
}

/// Unreachable code segment
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnreachableCode {
    /// File path
    pub file_path: String,
    /// Start line
    pub start_line: u32,
    /// End line
    pub end_line: u32,
    /// Code snippet
    pub snippet: String,
    /// Reason (after return, after throw, etc.)
    pub reason: String,
}

/// An unused symbol (private)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnusedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File path
    pub file_path: String,
    /// Line number
    pub line: u32,
}

/// An unused import
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnusedImport {
    /// Import name
    pub name: String,
    /// Full import path
    pub import_path: String,
    /// File path
    pub file_path: String,
    /// Line number
    pub line: u32,
}

/// An unused variable
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnusedVariable {
    /// Variable name
    pub name: String,
    /// File path
    pub file_path: String,
    /// Line number
    pub line: u32,
    /// Scope (function name, etc.)
    pub scope: Option<String>,
}

/// Breaking change between two versions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BreakingChange {
    /// Type of change
    pub change_type: BreakingChangeType,
    /// Symbol name affected
    pub symbol_name: String,
    /// Symbol kind
    pub symbol_kind: String,
    /// File path
    pub file_path: String,
    /// Line number in new version (None if removed)
    pub line: Option<u32>,
    /// Old signature/definition
    pub old_signature: Option<String>,
    /// New signature/definition
    pub new_signature: Option<String>,
    /// Detailed description of the change
    pub description: String,
    /// Severity (how breaking is this change)
    pub severity: ChangeSeverity,
    /// Migration hint
    pub migration_hint: Option<String>,
}

/// Type of breaking change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BreakingChangeType {
    /// Symbol was removed
    Removed,
    /// Symbol signature changed
    SignatureChanged,
    /// Parameter added (required)
    ParameterAdded,
    /// Parameter removed
    ParameterRemoved,
    /// Parameter type changed
    ParameterTypeChanged,
    /// Return type changed
    ReturnTypeChanged,
    /// Visibility reduced (public -> private)
    VisibilityReduced,
    /// Symbol renamed
    Renamed,
    /// Type constraint added/changed
    TypeConstraintChanged,
    /// Generic parameter changed
    GenericChanged,
    /// Exception/error type changed
    ThrowsChanged,
    /// Async/sync changed
    AsyncChanged,
    /// Moved to different module/package
    Moved,
}

/// Severity of a breaking change
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeSeverity {
    /// Will definitely break dependent code
    Critical,
    #[default]
    /// Will likely break dependent code
    High,
    /// May break dependent code
    Medium,
    /// Unlikely to break code but is a change in contract
    Low,
}

/// Result of breaking change detection
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BreakingChangeReport {
    /// Git ref for old version
    pub old_ref: String,
    /// Git ref for new version
    pub new_ref: String,
    /// List of breaking changes
    pub changes: Vec<BreakingChange>,
    /// Summary statistics
    pub summary: BreakingChangeSummary,
}

/// Summary of breaking changes
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BreakingChangeSummary {
    /// Total breaking changes
    pub total: u32,
    /// Critical severity count
    pub critical: u32,
    /// High severity count
    pub high: u32,
    /// Medium severity count
    pub medium: u32,
    /// Low severity count
    pub low: u32,
    /// Files affected
    pub files_affected: u32,
    /// Symbols affected
    pub symbols_affected: u32,
}

/// Multi-repository index
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiRepoIndex {
    /// Repository entries
    pub repositories: Vec<RepoEntry>,
    /// Cross-repository symbol links
    pub cross_repo_links: Vec<CrossRepoLink>,
    /// Unified symbol index across all repos
    pub unified_symbols: HashMap<String, Vec<UnifiedSymbolRef>>,
}

/// Entry for a repository in the multi-repo index
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RepoEntry {
    /// Unique identifier for the repo
    pub id: String,
    /// Repository name
    pub name: String,
    /// Repository path (local or URL)
    pub path: String,
    /// Git commit hash
    pub commit: Option<String>,
    /// Number of files indexed
    pub file_count: u32,
    /// Number of symbols indexed
    pub symbol_count: u32,
    /// Last indexed timestamp
    pub indexed_at: Option<u64>,
}

/// A cross-repository link (dependency, reference)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CrossRepoLink {
    /// Source repository ID
    pub source_repo: String,
    /// Source file path
    pub source_file: String,
    /// Source symbol name
    pub source_symbol: Option<String>,
    /// Source line number
    pub source_line: u32,
    /// Target repository ID
    pub target_repo: String,
    /// Target symbol name
    pub target_symbol: String,
    /// Link type
    pub link_type: CrossRepoLinkType,
}

/// Type of cross-repository link
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CrossRepoLinkType {
    #[default]
    /// Import/dependency
    Import,
    /// Type reference
    TypeReference,
    /// Function call
    Call,
    /// Inheritance
    Extends,
    /// Interface implementation
    Implements,
}

/// Reference to a symbol in the unified index
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnifiedSymbolRef {
    /// Repository ID
    pub repo_id: String,
    /// File path within repo
    pub file_path: String,
    /// Line number
    pub line: u32,
    /// Symbol kind
    pub kind: String,
    /// Fully qualified name
    pub qualified_name: Option<String>,
}
