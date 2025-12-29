//! # Infiniloom Engine - Repository Context Generation for LLMs
//!
//! `infiniloom_engine` is a high-performance library for generating optimized
//! repository context for Large Language Models. It transforms codebases into
//! structured formats optimized for Claude, GPT-4, Gemini, and other LLMs.
//!
//! ## Features
//!
//! - **AST-based symbol extraction** via Tree-sitter (21 programming languages)
//! - **PageRank-based importance ranking** for intelligent code prioritization
//! - **Model-specific output formats** (XML for Claude, Markdown for GPT, YAML for Gemini)
//! - **Automatic secret detection** and redaction (API keys, credentials, tokens)
//! - **Accurate token counting** using tiktoken-rs for OpenAI models (~95% accuracy)
//! - **Full dependency resolution** with transitive dependency analysis
//! - **Remote Git repository support** (GitHub, GitLab, Bitbucket)
//! - **Incremental scanning** with content-addressed caching
//! - **Semantic compression** for intelligent code summarization
//! - **Token budget enforcement** with smart truncation strategies
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use infiniloom_engine::{Repository, RepoMapGenerator, OutputFormatter, OutputFormat};
//!
//! // Create a repository from scanned files
//! let repo = Repository::new("my-project", "/path/to/project");
//!
//! // Generate a repository map with key symbols ranked by importance
//! let map = RepoMapGenerator::new(2000).generate(&repo);
//!
//! // Format for Claude (XML output)
//! let formatter = OutputFormatter::by_format(OutputFormat::Xml);
//! let output = formatter.format(&repo, &map);
//! ```
//!
//! ## Output Formats
//!
//! Each LLM has an optimal input format:
//!
//! | Format | Best For | Notes |
//! |--------|----------|-------|
//! | XML | Claude | Optimized structure, CDATA sections |
//! | Markdown | GPT-4 | Fenced code blocks with syntax highlighting |
//! | YAML | Gemini | Query at end (Gemini best practice) |
//! | TOON | All | Token-efficient, 30-40% fewer tokens |
//! | JSON | APIs | Machine-readable, fully structured |
//!
//! ## Token Counting
//!
//! The library provides accurate token counts for multiple LLM families:
//!
//! ```rust,ignore
//! use infiniloom_engine::{Tokenizer, TokenModel};
//!
//! let tokenizer = Tokenizer::new();
//! let content = "fn main() { println!(\"Hello\"); }";
//!
//! // Exact counts via tiktoken for OpenAI models
//! let gpt4o_tokens = tokenizer.count(content, TokenModel::Gpt4o);
//!
//! // Calibrated estimation for other models
//! let claude_tokens = tokenizer.count(content, TokenModel::Claude);
//! ```
//!
//! ## Security Scanning
//!
//! Automatically detect and redact sensitive information:
//!
//! ```rust,ignore
//! use infiniloom_engine::SecurityScanner;
//!
//! let scanner = SecurityScanner::new();
//! let content = "AWS_KEY=AKIAIOSFODNN7EXAMPLE";
//!
//! // Check if content is safe
//! if !scanner.is_safe(content, "config.env") {
//!     // Redact sensitive content
//!     let redacted = scanner.redact_content(content, "config.env");
//! }
//! ```
//!
//! ## Feature Flags
//!
//! Enable optional functionality:
//!
//! - `async` - Async/await support with Tokio
//! - `embeddings` - Character-frequency similarity (NOT neural - see semantic module docs)
//! - `watch` - File watching for incremental updates
//! - `full` - All features enabled
//!
//! Note: Git operations use the system `git` CLI via `std::process::Command`.
//!
//! ## Module Overview
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`parser`] | AST-based symbol extraction using Tree-sitter |
//! | [`repomap`] | PageRank-based symbol importance ranking |
//! | [`output`] | Model-specific formatters (XML, Markdown, etc.) |
//! | [`content_processing`] | Content transformation utilities (base64 truncation) |
//! | [`content_transformation`] | Code compression (comment removal, signature extraction) |
//! | [`filtering`] | Centralized file filtering and pattern matching |
//! | [`security`] | Secret detection and redaction |
//! | [`tokenizer`] | Multi-model token counting |
//! | [`chunking`] | Semantic code chunking |
//! | [`budget`] | Token budget enforcement |
//! | [`incremental`] | Caching and incremental scanning |
//! | [`semantic`] | Heuristic-based compression (char-frequency, NOT neural) |
//! | [`error`] | Unified error types |

// Core modules
pub mod chunking;
pub mod constants;
pub mod content_processing;
pub mod content_transformation;
pub mod default_ignores;
pub mod filtering;
pub mod newtypes;
pub mod output;
pub mod parser;
pub mod ranking;
pub mod repomap;
pub mod scanner;
pub mod security;
pub mod types;

// New modules
pub mod config;
pub mod dependencies;
pub mod git;
pub mod remote;
pub mod tokenizer;

// Git context index module
pub mod index;

// Memory-mapped file scanner for large files
pub mod mmap_scanner;

// Semantic analysis module (always available, embeddings feature enables neural compression)
pub mod semantic;

// Smart token budget enforcement
pub mod budget;

// Incremental scanning and caching
pub mod incremental;

// Unified error types
pub mod error;

// Re-exports from core modules
pub use chunking::{Chunk, ChunkStrategy, Chunker};
pub use constants::{
    budget as budget_constants, compression as compression_constants, files as file_constants,
    index as index_constants, pagerank as pagerank_constants, parser as parser_constants,
    repomap as repomap_constants, security as security_constants, timeouts as timeout_constants,
};
pub use content_transformation::{
    extract_key_symbols, extract_key_symbols_with_context, extract_signatures, remove_comments,
    remove_empty_lines,
};
pub use filtering::{
    apply_exclude_patterns, apply_include_patterns, compile_patterns, matches_exclude_pattern,
    matches_include_pattern,
};
pub use newtypes::{ByteOffset, FileSize, ImportanceScore, LineNumber, SymbolId, TokenCount};
pub use output::{OutputFormat, OutputFormatter};
pub use parser::{
    detect_file_language, parse_file_symbols, parse_with_language, Language, Parser, ParserError,
};
pub use ranking::{count_symbol_references, rank_files, sort_files_by_importance, SymbolRanker};
pub use repomap::{RepoMap, RepoMapGenerator};
pub use security::SecurityScanner;
pub use types::*;

// Re-exports from new modules
pub use budget::{BudgetConfig, BudgetEnforcer, EnforcementResult, TruncationStrategy};
pub use config::{
    Config, OutputConfig, PerformanceConfig, ScanConfig, SecurityConfig, SymbolConfig,
};
pub use dependencies::{DependencyEdge, DependencyGraph, DependencyNode, ResolvedImport};
pub use git::{ChangedFile, Commit, FileStatus, GitError, GitRepo};
pub use incremental::{CacheError, CacheStats, CachedFile, CachedSymbol, RepoCache};
pub use mmap_scanner::{MappedFile, MmapScanner, ScanStats, ScannedFile, StreamingProcessor};
pub use remote::{GitProvider, RemoteError, RemoteRepo};
pub use semantic::{
    CodeChunk,
    HeuristicCompressionConfig,
    // Note: SemanticAnalyzer and CharacterFrequencyAnalyzer are available via semantic:: module
    // but not re-exported at top level since they're primarily internal implementation details
    // Honest type aliases - recommended for new code
    HeuristicCompressor,
    SemanticCompressor,
    SemanticConfig,
    SemanticError,
};
/// Backward-compatible alias for TokenCounts
pub use tokenizer::TokenCounts as AccurateTokenCounts;
pub use tokenizer::Tokenizer;
// Note: IncrementalScanner is available via incremental:: module but not re-exported
// at top level since CLI uses RepoCache directly
pub use error::{InfiniloomError, Result as InfiniloomResult};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default token budget for repository maps
pub const DEFAULT_MAP_BUDGET: u32 = budget_constants::DEFAULT_MAP_BUDGET;

/// Default chunk size in tokens
pub const DEFAULT_CHUNK_SIZE: u32 = budget_constants::DEFAULT_CHUNK_SIZE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        // Verify version follows semver format (at least has a number)
        assert!(VERSION.chars().any(|c| c.is_ascii_digit()));
    }
}
