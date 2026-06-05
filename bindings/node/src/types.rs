//! NAPI type definitions for Node.js bindings
//!
//! This module contains all NAPI struct definitions used in the Node.js API.
//! Organized by feature area for maintainability.

use napi_derive::napi;
use infiniloom_engine::index::{
    CallGraph as EngineCallGraph, CallGraphEdge as EngineCallGraphEdge,
    CallGraphStats as EngineCallGraphStats, DependencyCycle as EngineDependencyCycle,
    ReferenceInfo as EngineReferenceInfo, SymbolInfo as EngineSymbolInfo,
};

// ============================================================================
// Pack & Scan Types
// ============================================================================

/// Options for packing a repository
#[napi(object)]
pub struct PackOptions {
    /// Output format: "xml", "markdown", "json", "yaml", "toon", or "plain"
    pub format: Option<String>,
    /// Target model: "claude", "gpt-5.2", "gpt-5.1", "gpt-5", "o4-mini", "o3", "o1", "gpt-4o", "gpt-4", "gemini", "llama", "mistral", "deepseek", "qwen", "cohere", "grok"
    pub model: Option<String>,
    /// Compression level: "none", "minimal", "balanced", "aggressive", "extreme", "focused", "semantic"
    pub compression: Option<String>,
    /// Token budget for repository map
    pub map_budget: Option<u32>,
    /// Maximum number of symbols in map
    pub max_symbols: Option<u32>,
    /// Skip security scanning (fail on critical findings)
    pub skip_security: Option<bool>,
    /// Redact detected secrets in output (default: true)
    pub redact_secrets: Option<bool>,
    /// Skip symbol extraction for faster scanning
    pub skip_symbols: Option<bool>,
    /// Glob patterns to include (e.g., ["src/**/*.ts", "lib/**/*.js"])
    pub include: Option<Vec<String>>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Include test files (default: false)
    pub include_tests: Option<bool>,
    /// Minimum security severity to block on: "critical", "high", "medium", "low" (default: "critical")
    pub security_threshold: Option<String>,
    /// Token budget for total output (0 = no limit). Files are included by importance until budget is reached.
    /// Negative values are invalid and will throw an error.
    pub token_budget: Option<i64>,
    /// Only include files changed in git (requires baseSha or uses uncommitted changes)
    pub changed_only: Option<bool>,
    /// Base SHA/ref for diff comparison (e.g., "main", "HEAD~5", commit hash)
    pub base_sha: Option<String>,
    /// Head SHA/ref for diff comparison (default: working tree or HEAD)
    pub head_sha: Option<String>,
    /// Include staged changes only (if changedOnly is true and no refs specified)
    pub staged_only: Option<bool>,
    /// Include related files (importers/dependencies of changed files)
    pub include_related: Option<bool>,
    /// Depth for related file traversal (1-3, default: 1)
    pub related_depth: Option<u32>,
}

/// Statistics from scanning a repository
#[napi(object)]
pub struct ScanStats {
    /// Repository name
    pub name: String,
    /// Total number of files
    pub total_files: u32,
    /// Total lines of code
    pub total_lines: u32,
    /// Total tokens for target model
    pub total_tokens: u32,
    /// Primary language
    pub primary_language: Option<String>,
    /// Language breakdown
    pub languages: Vec<LanguageStat>,
    /// Number of security findings
    pub security_findings: u32,
}

/// Statistics for a single language
#[napi(object)]
pub struct LanguageStat {
    /// Language name
    pub language: String,
    /// Number of files
    pub files: u32,
    /// Total lines
    pub lines: u32,
    /// Percentage of codebase
    pub percentage: f64,
}

/// Options for scanning a repository
#[napi(object)]
pub struct ScanOptions {
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Glob patterns to include (e.g., ["src/**/*.ts", "lib/**/*.js"])
    pub include: Option<Vec<String>>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Include test files (default: false)
    pub include_tests: Option<bool>,
    /// Apply default ignores for dist/, node_modules/, etc. (default: true)
    pub apply_default_ignores: Option<bool>,
}

// ============================================================================
// Git Types
// ============================================================================

/// File status information
#[napi(object)]
pub struct GitFileStatus {
    /// File path
    pub path: String,
    /// Old path (for renames)
    pub old_path: Option<String>,
    /// Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
    pub status: String,
}

/// Changed file with diff stats
#[napi(object)]
pub struct GitChangedFile {
    /// File path
    pub path: String,
    /// Old path (for renames)
    pub old_path: Option<String>,
    /// Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
    pub status: String,
    /// Number of lines added
    pub additions: u32,
    /// Number of lines deleted
    pub deletions: u32,
}

/// Commit information
#[napi(object)]
pub struct GitCommit {
    /// Full commit hash
    pub hash: String,
    /// Short commit hash (7 characters)
    pub short_hash: String,
    /// Author name
    pub author: String,
    /// Author email
    pub email: String,
    /// Commit date (ISO 8601 format)
    pub date: String,
    /// Commit message (first line)
    pub message: String,
}

/// Blame line information
#[napi(object)]
pub struct GitBlameLine {
    /// Commit hash that introduced the line
    pub commit: String,
    /// Author who wrote the line
    pub author: String,
    /// Date when line was written
    pub date: String,
    /// Line number (1-indexed)
    pub line_number: u32,
}

/// A single line change within a diff hunk
#[napi(object)]
pub struct GitDiffLine {
    /// Type of change: "add", "remove", or "context"
    pub change_type: String,
    /// Line number in the old file (null for additions)
    pub old_line: Option<u32>,
    /// Line number in the new file (null for deletions)
    pub new_line: Option<u32>,
    /// The actual line content (without +/- prefix)
    pub content: String,
}

/// A diff hunk representing a contiguous block of changes
#[napi(object)]
pub struct GitDiffHunk {
    /// Starting line in the old file
    pub old_start: u32,
    /// Number of lines in the old file section
    pub old_count: u32,
    /// Starting line in the new file
    pub new_start: u32,
    /// Number of lines in the new file section
    pub new_count: u32,
    /// Header line (e.g., "@@ -1,5 +1,7 @@ function name")
    pub header: String,
    /// Individual line changes within this hunk
    pub lines: Vec<GitDiffLine>,
}

// ============================================================================
// Security Types
// ============================================================================

#[napi(object)]
pub struct SecurityFinding {
    /// File where the finding was detected
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Severity level: "Critical", "High", "Medium", "Low", "Info"
    pub severity: String,
    /// Type of finding
    pub kind: String,
    /// Matched pattern
    pub pattern: String,
}

// ============================================================================
// Index Types
// ============================================================================

/// Options for building an index
#[napi(object)]
pub struct IndexOptions {
    /// Force full rebuild even if index exists
    pub force: Option<bool>,
    /// Include test files in index
    pub include_tests: Option<bool>,
    /// Maximum file size to index (bytes)
    pub max_file_size: Option<u32>,
    /// Directories/patterns to exclude (e.g., ["node_modules", "dist", "vendor", "*.generated.*"])
    pub exclude: Option<Vec<String>>,
    /// Incremental update - only re-index changed files (default: false)
    /// When true, compares file hashes with existing index and only rebuilds changed files
    pub incremental: Option<bool>,
}

/// Index status information
#[napi(object)]
pub struct IndexStatus {
    /// Whether an index exists
    pub exists: bool,
    /// Number of files indexed
    pub file_count: u32,
    /// Number of symbols indexed
    pub symbol_count: u32,
    /// Last build timestamp (ISO 8601)
    pub last_built: Option<String>,
    /// Index version
    pub version: Option<String>,
    /// Number of files updated in incremental build (only set for incremental builds)
    pub files_updated: Option<u32>,
    /// Whether this was an incremental update
    pub incremental: Option<bool>,
}

// ============================================================================
// Call Graph & Symbol Types
// ============================================================================

/// Information about a symbol in the call graph
#[napi(object)]
pub struct SymbolInfo {
    /// Symbol ID
    pub id: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// File path containing the symbol
    pub file: String,
    /// Start line number (1-indexed, consistent with editors/IDEs)
    pub line: u32,
    /// End line number (1-indexed, consistent with editors/IDEs)
    pub end_line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Visibility (public, private, etc.)
    pub visibility: String,
}

impl From<EngineSymbolInfo> for SymbolInfo {
    fn from(s: EngineSymbolInfo) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            file: s.file,
            line: s.line,
            end_line: s.end_line,
            signature: s.signature,
            visibility: s.visibility,
        }
    }
}

/// A reference to a symbol with context
#[napi(object)]
pub struct ReferenceInfo {
    /// Symbol making the reference
    pub symbol: SymbolInfo,
    /// Reference kind (call, import, inherit, implement)
    pub kind: String,
    // Convenience fields for easier access (mirrors symbol fields)
    /// File path containing the reference (convenience field, same as symbol.file)
    pub file: String,
    /// Line number of the reference (1-indexed, convenience field, same as symbol.line)
    /// Note: This is the line where the referencing symbol is defined, not where the
    /// actual reference occurs. For call site line numbers, use getCallSites() instead.
    pub line: u32,
}

impl From<EngineReferenceInfo> for ReferenceInfo {
    fn from(r: EngineReferenceInfo) -> Self {
        let symbol: SymbolInfo = r.symbol.into();
        let file = symbol.file.clone();
        let line = symbol.line;
        Self { symbol, kind: r.kind, file, line }
    }
}

/// An edge in the call graph
#[napi(object)]
pub struct CallGraphEdge {
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
    /// Caller symbol name
    pub caller: String,
    /// Callee symbol name
    pub callee: String,
    /// File containing the call site
    pub file: String,
    /// Line number of the call
    pub line: u32,
}

impl From<EngineCallGraphEdge> for CallGraphEdge {
    fn from(e: EngineCallGraphEdge) -> Self {
        Self {
            caller_id: e.caller_id,
            callee_id: e.callee_id,
            caller: e.caller,
            callee: e.callee,
            file: e.file,
            line: e.line,
        }
    }
}

/// Call graph statistics
#[napi(object)]
pub struct CallGraphStats {
    /// Total number of symbols
    pub total_symbols: u32,
    /// Total number of call edges
    pub total_calls: u32,
    /// Number of functions/methods
    pub functions: u32,
    /// Number of classes/structs
    pub classes: u32,
}

impl From<EngineCallGraphStats> for CallGraphStats {
    fn from(s: EngineCallGraphStats) -> Self {
        Self {
            total_symbols: s.total_symbols as u32,
            total_calls: s.total_calls as u32,
            functions: s.functions as u32,
            classes: s.classes as u32,
        }
    }
}

/// Complete call graph with nodes and edges
#[napi(object)]
pub struct CallGraph {
    /// All symbols (nodes)
    pub nodes: Vec<SymbolInfo>,
    /// Call relationships (edges)
    pub edges: Vec<CallGraphEdge>,
    /// Summary statistics
    pub stats: CallGraphStats,
}

impl From<EngineCallGraph> for CallGraph {
    fn from(g: EngineCallGraph) -> Self {
        Self {
            nodes: g.nodes.into_iter().map(Into::into).collect(),
            edges: g.edges.into_iter().map(Into::into).collect(),
            stats: g.stats.into(),
        }
    }
}

/// A cycle in the dependency graph (circular import)
#[napi(object)]
pub struct DependencyCycle {
    /// File paths in the cycle (e.g., ["a.ts", "b.ts", "c.ts"] means a->b->c->a)
    pub files: Vec<String>,
    /// Internal file IDs corresponding to the files
    pub file_ids: Vec<u32>,
    /// Number of files in the cycle
    pub length: u32,
}

impl From<EngineDependencyCycle> for DependencyCycle {
    fn from(c: EngineDependencyCycle) -> Self {
        Self {
            files: c.files,
            file_ids: c.file_ids,
            length: c.length as u32,
        }
    }
}

/// Options for call graph queries
#[napi(object)]
pub struct CallGraphOptions {
    /// Maximum number of nodes to return (default: unlimited)
    pub max_nodes: Option<u32>,
    /// Maximum number of edges to return (default: unlimited)
    pub max_edges: Option<u32>,
}

/// Result from getSymbolSource containing source code and metadata
#[napi(object)]
pub struct SymbolSourceResult {
    /// The source code of the symbol
    pub source: String,
    /// Path to the file containing the symbol (relative to repo root)
    pub path: String,
    /// Start line number (1-indexed)
    pub start_line: u32,
    /// End line number (1-indexed)
    pub end_line: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, method, class, etc.)
    pub kind: String,
}

/// Options for generateMap
#[napi(object)]
pub struct GenerateMapOptions {
    /// Token budget for the map (default: 2000)
    pub budget: Option<u32>,
    /// Maximum number of symbols to include (default: 50)
    pub max_symbols: Option<u32>,
}

/// Options for semanticCompress
#[napi(object)]
pub struct SemanticCompressOptions {
    /// Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
    /// Note: Only affects output when built with "embeddings" feature.
    pub similarity_threshold: Option<f64>,
    /// Target size as ratio of original (0.0-1.0, default: 0.5)
    /// Lower values = more aggressive compression
    pub budget_ratio: Option<f64>,
    /// Minimum chunk size in characters (default: 100)
    pub min_chunk_size: Option<u32>,
    /// Maximum chunk size in characters (default: 2000)
    pub max_chunk_size: Option<u32>,
}

/// Feature #2: Filter options for symbol queries
///
/// Allows filtering query results by symbol kind.
#[napi(object)]
pub struct QueryFilter {
    /// Filter by symbol kinds: "function", "method", "class", "struct", "interface", "trait", "enum", etc.
    /// If specified, only symbols of these kinds are returned.
    pub kinds: Option<Vec<String>>,
    /// Exclude specific kinds (e.g., exclude "import" to skip import statements)
    pub exclude_kinds: Option<Vec<String>>,
}

// ============================================================================
// Chunk Types
// ============================================================================

/// Options for chunking a repository
#[napi(object)]
pub struct ChunkOptions {
    /// Chunking strategy: "fixed", "file", "module", "symbol", "semantic", "dependency"
    pub strategy: Option<String>,
    /// Maximum tokens per chunk (default: 8000)
    pub max_tokens: Option<u32>,
    /// Token overlap between chunks (default: 0)
    pub overlap: Option<u32>,
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Output format: "xml", "markdown", "json" (default: "xml")
    pub format: Option<String>,
    /// Sort chunks by priority (core modules first)
    pub priority_first: Option<bool>,
    /// Directories/patterns to exclude (e.g., ["vendor", "generated", "*.test.*"])
    pub exclude: Option<Vec<String>>,
}

/// A chunk of repository content
#[napi(object)]
pub struct RepoChunk {
    /// Chunk index (0-based)
    pub index: u32,
    /// Total number of chunks
    pub total: u32,
    /// Primary focus/topic of this chunk
    pub focus: String,
    /// Estimated token count
    pub tokens: u32,
    /// Files included in this chunk
    pub files: Vec<String>,
    /// Formatted content of the chunk
    pub content: String,
}

// ============================================================================
// Impact Analysis Types
// ============================================================================

/// Options for impact analysis
#[napi(object)]
pub struct ImpactOptions {
    /// Depth of dependency traversal (1-3, default: 2)
    pub depth: Option<u32>,
    /// Include test files in analysis
    pub include_tests: Option<bool>,
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Glob patterns to include (e.g., ["src/**/*.ts"])
    pub include: Option<Vec<String>>,
}

/// Symbol affected by a change
#[napi(object)]
pub struct AffectedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, etc.)
    pub kind: String,
    /// File containing the symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// How the symbol is affected: "direct", "caller", "callee", "dependent"
    pub impact_type: String,
}

/// Impact analysis result
#[napi(object)]
pub struct ImpactResult {
    /// Files directly changed
    pub changed_files: Vec<String>,
    /// Files that depend on changed files
    pub dependent_files: Vec<String>,
    /// Related test files
    pub test_files: Vec<String>,
    /// Symbols affected by the changes
    pub affected_symbols: Vec<AffectedSymbol>,
    /// Overall impact level: "low", "medium", "high", "critical"
    pub impact_level: String,
    /// Summary of the impact
    pub summary: String,
}

// ============================================================================
// Diff Context Types
// ============================================================================

/// Options for diff context
#[napi(object)]
pub struct DiffContextOptions {
    /// Depth of context expansion (1-3, default: 2)
    pub depth: Option<u32>,
    /// Token budget for context (default: 50000)
    pub budget: Option<u32>,
    /// Include the actual diff content (default: false)
    pub include_diff: Option<bool>,
    /// Output format: "xml", "markdown", "json" (default: "xml")
    pub format: Option<String>,
    /// Target model for token counting (default: "claude")
    pub model: Option<String>,
    /// Glob patterns to exclude (e.g., ["**/*.test.ts", "dist/**"])
    pub exclude: Option<Vec<String>>,
    /// Glob patterns to include (e.g., ["src/**/*.ts"])
    pub include: Option<Vec<String>>,
}

/// Context-aware diff result
#[napi(object)]
pub struct DiffContextResult {
    /// Changed files with context
    pub changed_files: Vec<DiffFileContext>,
    /// Related symbols and their context
    pub context_symbols: Vec<ContextSymbolInfo>,
    /// Related test files
    pub related_tests: Vec<String>,
    /// Formatted output (if format specified)
    pub formatted_output: Option<String>,
    /// Total token count
    pub total_tokens: u32,
}

/// A changed file with surrounding context
#[napi(object)]
pub struct DiffFileContext {
    /// File path
    pub path: String,
    /// Change type: "Added", "Modified", "Deleted", "Renamed"
    pub change_type: String,
    /// Lines added
    pub additions: u32,
    /// Lines deleted
    pub deletions: u32,
    /// Unified diff content (if include_diff is true)
    pub diff: Option<String>,
    /// Relevant code context around changes
    pub context_snippets: Vec<String>,
}

/// Symbol context information
#[napi(object)]
pub struct ContextSymbolInfo {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File containing symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// Why this symbol is included: "changed", "caller", "callee", "dependent"
    pub reason: String,
    /// Symbol signature/definition
    pub signature: Option<String>,
}

// ============================================================================
// Symbol Filter Types
// ============================================================================

/// Options for filtering symbols
#[napi(object)]
pub struct SymbolFilter {
    /// Filter by symbol kind: "function", "class", "method", etc.
    pub kind: Option<String>,
    /// Filter by visibility: "public", "private", "protected"
    pub visibility: Option<String>,
}

/// A call site where a symbol is called
#[napi(object)]
pub struct CallSite {
    /// Name of the calling function/method
    pub caller: String,
    /// Name of the function/method being called
    pub callee: String,
    /// File containing the call
    pub file: String,
    /// Line number of the call (1-indexed)
    pub line: u32,
    /// Column number of the call (0-indexed, if available)
    pub column: Option<u32>,
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
}

/// Call site with surrounding code context
#[napi(object)]
pub struct CallSiteWithContext {
    /// Name of the calling function/method
    pub caller: String,
    /// Name of the function/method being called
    pub callee: String,
    /// File containing the call
    pub file: String,
    /// Line number of the call (1-indexed)
    pub line: u32,
    /// Column number of the call (0-indexed, if available)
    pub column: Option<u32>,
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
    /// Code context around the call site (configurable number of lines)
    pub context: Option<String>,
    /// Start line of context
    pub context_start_line: Option<u32>,
    /// End line of context
    pub context_end_line: Option<u32>,
}

/// Options for call sites with context
#[napi(object)]
pub struct CallSitesContextOptions {
    /// Number of lines of context before the call (default: 3)
    pub lines_before: Option<u32>,
    /// Number of lines of context after the call (default: 3)
    pub lines_after: Option<u32>,
}

/// Filter for changed symbols query
#[napi(object)]
pub struct ChangedSymbolsFilter {
    /// Filter by symbol kinds: "function", "method", "class", etc.
    /// If specified, only symbols of these kinds are returned.
    pub kinds: Option<Vec<String>>,
    /// Exclude specific kinds (e.g., exclude "import" to skip import statements)
    pub exclude_kinds: Option<Vec<String>>,
}

/// A symbol with change type information
#[napi(object)]
pub struct ChangedSymbolInfo {
    /// Symbol ID
    pub id: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// File path containing the symbol
    pub file: String,
    /// Start line number
    pub line: u32,
    /// End line number
    pub end_line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Visibility (public, private, etc.)
    pub visibility: String,
    /// Change type: "added", "modified", or "deleted"
    pub change_type: String,
}

/// Transitive caller information
#[napi(object)]
pub struct TransitiveCallerInfo {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Depth from the target symbol (1 = direct caller, 2 = caller of caller, etc.)
    pub depth: u32,
    /// Call path from this caller to the target (e.g., ["main", "process", "validate", "target"])
    pub call_path: Vec<String>,
}

/// Options for transitive callers query
#[napi(object)]
pub struct TransitiveCallersOptions {
    /// Maximum depth to traverse (default: 3)
    pub max_depth: Option<u32>,
    /// Maximum number of results (default: 100)
    pub max_results: Option<u32>,
}

// ============================================================================
// Embedding Types
// ============================================================================

/// Options for embedding chunk generation
#[napi(object)]
pub struct EmbedOptions {
    /// Maximum tokens per chunk (default: 1000)
    pub max_tokens: Option<u32>,
    /// Minimum tokens for a chunk (default: 50)
    pub min_tokens: Option<u32>,
    /// Lines of context around symbols (default: 5)
    pub context_lines: Option<u32>,
    /// Include imports in chunks (default: true)
    pub include_imports: Option<bool>,
    /// Include top-level code (default: true)
    pub include_top_level: Option<bool>,
    /// Include test files (default: false)
    pub include_tests: Option<bool>,
    /// Enable secret scanning (default: true)
    pub security_scan: Option<bool>,
    /// Include patterns (glob)
    pub include_patterns: Option<Vec<String>>,
    /// Exclude patterns (glob)
    pub exclude_patterns: Option<Vec<String>>,
    /// Path to manifest file (default: .infiniloom-embed.bin)
    pub manifest_path: Option<String>,
    /// Only return changed chunks (diff mode)
    pub diff_only: Option<bool>,
}

/// Source information for a chunk
#[napi(object)]
pub struct EmbedChunkSource {
    /// File path
    pub file: String,
    /// Line range (start, end) - 1-indexed
    pub lines_start: u32,
    pub lines_end: u32,
    /// Byte range within a single source line for overlong-line slices
    pub line_byte_start: Option<u32>,
    pub line_byte_end: Option<u32>,
    /// Symbol name
    pub symbol: String,
    /// Fully qualified name (if available)
    pub fqn: Option<String>,
    /// Programming language
    pub language: String,
    /// Parent symbol (if any)
    pub parent: Option<String>,
    /// Visibility: "public", "private", "protected", "internal"
    pub visibility: String,
    /// Whether this is test code
    pub is_test: bool,
    /// Module path derived from file path and language conventions
    pub module_path: Option<String>,
    /// Chunk ID of the parent container (class/struct/enum/trait/interface)
    pub parent_chunk_id: Option<String>,
    /// Source content transform applied before chunking
    pub content_transform: Option<String>,
}

/// Context information for a chunk
#[napi(object)]
pub struct EmbedChunkContext {
    /// Extracted docstring for natural language retrieval
    pub docstring: Option<String>,
    /// Extracted comments within the chunk
    pub comments: Vec<String>,
    /// Function/class signature (always included, even in split parts)
    pub signature: Option<String>,
    /// Symbols this chunk calls
    pub calls: Vec<String>,
    /// Symbols that call this chunk
    pub called_by: Vec<String>,
    /// Imports in this chunk
    pub imports: Vec<String>,
    /// Auto-generated semantic tags (async, security, database, etc.)
    pub tags: Vec<String>,
    /// Fully qualified calls resolved via import scope
    pub qualified_calls: Vec<String>,
    /// Calls that couldn't be resolved via imports
    pub unresolved_calls: Vec<String>,

    /// Clean type signature: "(i32, &str) -> Result<Claims, AuthError>"
    pub type_signature: Option<String>,
    /// Individual parameter types: ["i32", "&str"]
    pub parameter_types: Vec<String>,
    /// Return type: "Result<Claims, AuthError>"
    pub return_type: Option<String>,
    /// Error/exception types: ["AuthError"]
    pub error_types: Vec<String>,
    /// Lines of code (excluding blank lines and comments)
    pub lines_of_code: u32,
    /// Maximum nesting depth (control flow, blocks)
    pub max_nesting_depth: u32,
    /// Number of symbols that call/depend on this chunk
    pub dependents_count: Option<u32>,
}

/// Chunk part info for split chunks
#[napi(object)]
pub struct EmbedChunkPart {
    /// Part number (1-indexed)
    pub part: u32,
    /// Total number of parts
    pub of: u32,
    /// ID of the logical parent (full symbol hash)
    pub parent_id: String,
    /// Signature repeated for context
    pub parent_signature: Option<String>,
}

/// A single embedding chunk
#[napi(object)]
pub struct EmbedChunk {
    /// Content-addressable chunk ID (ec_ prefix + 32 hex chars)
    pub id: String,
    /// Full content hash for collision detection
    pub full_hash: String,
    /// Chunk content (code)
    pub content: String,
    /// Token count
    pub tokens: u32,
    /// Chunk kind: "function", "class", "struct", "method", etc.
    pub kind: String,
    /// Source information
    pub source: EmbedChunkSource,
    /// Context information
    pub context: EmbedChunkContext,
    /// IDs of child chunks (methods inside a class, etc.)
    pub children_ids: Vec<String>,
    /// Representation type: "code" (default) or "signature"
    pub repr: String,
    /// For non-code representations, ID of the full code chunk
    pub code_chunk_id: Option<String>,
    /// Part info (for multi-part chunks)
    pub part: Option<EmbedChunkPart>,
}

/// Diff summary statistics
#[napi(object)]
pub struct EmbedDiffSummary {
    /// Number of added chunks
    pub added: u32,
    /// Number of modified chunks
    pub modified: u32,
    /// Number of removed chunks
    pub removed: u32,
    /// Number of unchanged chunks
    pub unchanged: u32,
    /// Total chunks in current state
    pub total_chunks: u32,
}

/// Result from embedding operation
#[napi(object)]
pub struct EmbedResult {
    /// Generated chunks
    pub chunks: Vec<EmbedChunk>,
    /// Diff summary (if manifest existed)
    pub diff: Option<EmbedDiffSummary>,
    /// Manifest version
    pub manifest_version: u32,
    /// Processing time in milliseconds
    pub elapsed_ms: f64,
}

/// Manifest status information
#[napi(object)]
pub struct EmbedManifestStatus {
    /// Whether manifest exists
    pub exists: bool,
    /// Number of chunks in manifest
    pub chunk_count: Option<u32>,
    /// Repository path stored in manifest
    pub repo_path: Option<String>,
    /// Last update timestamp (Unix seconds)
    pub updated_at: Option<f64>,
    /// Manifest format version
    pub version: Option<u32>,
}
