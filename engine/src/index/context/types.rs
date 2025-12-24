//! Type definitions for context expansion.
//!
//! This module contains all the data structures used by the context expander,
//! including change classification, context results, and impact summaries.

/// Context expansion depth levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextDepth {
    /// L1: Containing functions only
    L1,
    /// L2: Direct dependents (default)
    #[default]
    L2,
    /// L3: Transitive dependents
    L3,
}

impl PartialOrd for ContextDepth {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ContextDepth {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_num = match self {
            ContextDepth::L1 => 1,
            ContextDepth::L2 => 2,
            ContextDepth::L3 => 3,
        };
        let other_num = match other {
            ContextDepth::L1 => 1,
            ContextDepth::L2 => 2,
            ContextDepth::L3 => 3,
        };
        self_num.cmp(&other_num)
    }
}

/// A change in a diff
#[derive(Debug, Clone)]
pub struct DiffChange {
    /// File path (for renames, this is the NEW path)
    pub file_path: String,
    /// Old file path (only set for renames)
    pub old_path: Option<String>,
    /// Changed line ranges (start, end)
    pub line_ranges: Vec<(u32, u32)>,
    /// Type of change
    pub change_type: ChangeType,
    /// Raw diff content (the actual +/- lines), if requested
    pub diff_content: Option<String>,
}

/// Type of change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// More detailed change classification for smart expansion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeClassification {
    /// New code added - include dependents at normal priority
    NewCode,
    /// Function signature changed - include ALL callers at high priority
    SignatureChange,
    /// Type/struct/class definition changed - include all usages
    TypeDefinitionChange,
    /// Function body only changed - lower impact, only direct dependents
    ImplementationChange,
    /// Code deleted - callers will break, highest priority
    Deletion,
    /// File renamed - importers need updating
    FileRename,
    /// Import/dependency changed - may affect resolution
    ImportChange,
    /// Documentation/comment only - minimal impact
    DocumentationOnly,
}

/// Expanded context result
#[derive(Debug, Clone)]
pub struct ExpandedContext {
    /// Changed symbols (directly modified)
    pub changed_symbols: Vec<ContextSymbol>,
    /// Changed files
    pub changed_files: Vec<ContextFile>,
    /// Dependent symbols (affected by changes)
    pub dependent_symbols: Vec<ContextSymbol>,
    /// Dependent files
    pub dependent_files: Vec<ContextFile>,
    /// Related tests
    pub related_tests: Vec<ContextFile>,
    /// Call chains involving changed symbols
    pub call_chains: Vec<CallChain>,
    /// Summary of impact
    pub impact_summary: ImpactSummary,
    /// Total estimated tokens
    pub total_tokens: u32,
}

/// A symbol in the context
#[derive(Debug, Clone)]
pub struct ContextSymbol {
    /// Symbol ID
    pub id: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, etc.)
    pub kind: String,
    /// File path
    pub file_path: String,
    /// Start line
    pub start_line: u32,
    /// End line
    pub end_line: u32,
    /// Signature
    pub signature: Option<String>,
    /// Why this symbol is relevant
    pub relevance_reason: String,
    /// Relevance score (0.0 - 1.0)
    pub relevance_score: f32,
}

/// A file in the context
#[derive(Debug, Clone)]
pub struct ContextFile {
    /// File ID
    pub id: u32,
    /// File path
    pub path: String,
    /// Language
    pub language: String,
    /// Why this file is relevant
    pub relevance_reason: String,
    /// Relevance score (0.0 - 1.0)
    pub relevance_score: f32,
    /// Estimated tokens
    pub tokens: u32,
    /// Relevant sections (line ranges)
    pub relevant_sections: Vec<(u32, u32)>,
    /// Raw diff content (the actual +/- lines), if available
    pub diff_content: Option<String>,
    /// Extracted snippets for LLM context
    pub snippets: Vec<ContextSnippet>,
}

/// A snippet of source context for a file
#[derive(Debug, Clone)]
pub struct ContextSnippet {
    /// Start line (1-indexed)
    pub start_line: u32,
    /// End line (1-indexed)
    pub end_line: u32,
    /// Why this snippet was included
    pub reason: String,
    /// Snippet content
    pub content: String,
}

/// A call chain for understanding impact
#[derive(Debug, Clone)]
pub struct CallChain {
    /// Symbols in the chain (from caller to callee)
    pub symbols: Vec<String>,
    /// Files involved
    pub files: Vec<String>,
}

/// Summary of the impact
#[derive(Debug, Clone, Default)]
pub struct ImpactSummary {
    /// Impact level (low, medium, high, critical)
    pub level: ImpactLevel,
    /// Number of directly affected files
    pub direct_files: usize,
    /// Number of transitively affected files
    pub transitive_files: usize,
    /// Number of affected symbols
    pub affected_symbols: usize,
    /// Number of affected tests
    pub affected_tests: usize,
    /// Breaking changes detected
    pub breaking_changes: Vec<String>,
    /// Description of the impact
    pub description: String,
}

/// Impact severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImpactLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl ImpactLevel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}
