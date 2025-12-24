//! Types for intelligent code chunking
//!
//! Public types exported by the chunking module.

use crate::types::TokenizerModel;
use serde::Serialize;

/// A chunk of repository content
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Chunk index (0-based)
    pub index: usize,
    /// Total number of chunks
    pub total: usize,
    /// Focus/theme of this chunk
    pub focus: String,
    /// Token count for this chunk
    pub tokens: u32,
    /// Files included in this chunk
    pub files: Vec<ChunkFile>,
    /// Context information
    pub context: ChunkContext,
}

/// A file within a chunk
#[derive(Debug, Clone, Serialize)]
pub struct ChunkFile {
    /// Relative file path
    pub path: String,
    /// File content (may be compressed)
    pub content: String,
    /// Token count
    pub tokens: u32,
    /// Whether content is truncated
    pub truncated: bool,
}

/// Context for chunk continuity
#[derive(Debug, Clone, Serialize)]
pub struct ChunkContext {
    /// Summary of previous chunks
    pub previous_summary: Option<String>,
    /// Current focus description
    pub current_focus: String,
    /// Preview of next chunk
    pub next_preview: Option<String>,
    /// Cross-references to other chunks
    pub cross_references: Vec<CrossReference>,
    /// Overlap content from previous chunk (for context continuity)
    pub overlap_content: Option<String>,
}

/// Reference to symbol in another chunk
#[derive(Debug, Clone, Serialize)]
pub struct CrossReference {
    /// Symbol name
    pub symbol: String,
    /// Chunk containing the symbol
    pub chunk_index: usize,
    /// File containing the symbol
    pub file: String,
}

/// Internal type for symbol-based chunking
#[derive(Debug, Clone)]
pub(crate) struct SymbolSnippet {
    pub file_path: String,
    pub symbol_name: String,
    pub start_line: u32,
    pub content: String,
    pub tokens: u32,
    pub importance: f32,
}

/// Chunking strategy
#[derive(Debug, Clone, Copy, Default)]
pub enum ChunkStrategy {
    /// Fixed token size chunks
    Fixed {
        /// Maximum tokens per chunk
        size: u32,
    },
    /// One file per chunk
    File,
    /// Group by module/directory
    Module,
    /// Group by symbols (AST-based)
    Symbol,
    /// Group by semantic similarity
    #[default]
    Semantic,
    /// Group by dependency order
    Dependency,
}

/// Chunker for splitting repositories
pub struct Chunker {
    /// Chunking strategy
    pub(crate) strategy: ChunkStrategy,
    /// Maximum tokens per chunk
    pub(crate) max_tokens: u32,
    /// Overlap tokens between chunks
    pub(crate) overlap_tokens: u32,
    /// Target model for token counting
    pub(crate) model: TokenizerModel,
}
