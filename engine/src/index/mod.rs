//! Git context index module.
//!
//! This module provides fast diff-to-context functionality by maintaining
//! a pre-computed index of symbols, files, and their relationships.
//!
//! # Architecture
//!
//! ```text
//! .infiniloom/
//! ├── index.bin    # Symbol index (bincode)
//! ├── graph.bin    # Dependency graph (bincode)
//! ├── meta.json    # Human-readable metadata
//! └── config.toml  # Index configuration
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use infiniloom_engine::index::{IndexBuilder, IndexStorage, DiffContext};
//!
//! // Build or update index
//! let builder = IndexBuilder::new("/path/to/repo");
//! let (index, graph) = builder.build()?;
//!
//! // Save index
//! let storage = IndexStorage::new("/path/to/repo");
//! storage.save_all(&index, &graph)?;
//!
//! // Query diff context
//! let context = DiffContext::new(&index, &graph);
//! let result = context.expand_diff(diff, ContextDepth::L2)?;
//! ```

pub mod builder;
pub mod context;
mod convert;
pub mod lazy;
pub mod patterns;
pub mod query;
pub mod storage;
pub mod types;

// Re-exports
pub use builder::{BuildError, BuildOptions, IndexBuilder};
pub use context::{
    CallChain, ChangeClassification, ChangeType, ContextDepth, ContextExpander, ContextFile,
    ContextSnippet, ContextSymbol, DiffChange, ExpandedContext, ImpactLevel, ImpactSummary,
};
pub use lazy::{LazyContextBuilder, LazyError};
pub use query::{
    find_symbol, get_call_graph, get_call_graph_filtered, get_callees_by_id, get_callees_by_name,
    get_callers_by_id, get_callers_by_name, get_references_by_name, CallGraph, CallGraphEdge,
    CallGraphStats, ReferenceInfo, SymbolInfo,
};
pub use storage::{IndexMeta, IndexStorage, StorageError};
pub use types::{
    DepGraph, FileEntry, FileId, Import, IndexSymbol, IndexSymbolKind, Language, RefKind,
    Reference, Span, SymbolId, SymbolIndex, Visibility,
};
