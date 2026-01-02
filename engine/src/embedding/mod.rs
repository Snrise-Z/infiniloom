//! Embedding chunks generation for vector databases
//!
//! This module provides deterministic, content-addressable code chunks optimized
//! for embedding in vector databases. The system generates stable chunk IDs based
//! on content hashes, enabling efficient incremental updates and cross-repository
//! deduplication.
//!
//! # Design Philosophy
//!
//! **This is a library/CLI tool, not a full enterprise platform.**
//!
//! It generates deterministic, semantically correct records for embedding into RAG systems.
//! You integrate it into your own pipelines - it does not provide:
//! - Multi-tenant access control (just namespace metadata)
//! - Distributed job coordination (process repos sequentially)
//! - Vector embedding generation (use OpenAI, Voyage, Cohere, etc.)
//! - Vector database storage (use Pinecone, Weaviate, Qdrant, etc.)
//!
//! Typical usage: run `infiniloom embed` on 300+ repos sequentially, ingest JSONL into your vector DB.
//!
//! # Features
//!
//! - **Deterministic**: Same repo + same settings = identical output every time
//! - **Content-addressable**: Same code anywhere = same chunk ID (enables deduplication)
//! - **Code-aware**: Chunks respect AST boundaries (never split mid-function)
//! - **Incremental**: Track added/modified/removed chunks via manifest
//! - **Cross-platform**: Identical output on Windows/Linux/macOS
//! - **Secure**: Secret scanning integration, DoS-resistant
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::{EmbedChunker, EmbedSettings, ResourceLimits};
//!
//! // Create chunker with default settings
//! let settings = EmbedSettings::default();
//! let limits = ResourceLimits::default();
//! let chunker = EmbedChunker::new(settings, limits);
//!
//! // Generate chunks for a repository
//! let chunks = chunker.chunk_repository(Path::new("/path/to/repo"))?;
//!
//! // Each chunk has a stable, content-addressable ID
//! for chunk in &chunks {
//!     println!("Chunk {}: {} tokens", chunk.id, chunk.tokens);
//! }
//! ```
//!
//! # Content Addressability
//!
//! Chunk IDs are derived from BLAKE3 hashes of normalized content:
//!
//! ```rust,ignore
//! // Same code = same ID, even in different files/repos
//! let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
//! let hash = embed_hash(code);
//! // Returns: "ec_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6" (32 hex chars)
//! ```
//!
//! # Incremental Updates
//!
//! Use manifests to track changes between runs:
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::{EmbedManifest, EmbedChunker};
//!
//! // Load previous manifest
//! let manifest = EmbedManifest::load(Path::new(".infiniloom-embed.bin"))?;
//!
//! // Generate current chunks
//! let chunks = chunker.chunk_repository(repo_path)?;
//!
//! // Compute diff
//! let diff = manifest.diff(&chunks);
//!
//! println!("Added: {}, Modified: {}, Removed: {}",
//!     diff.summary.added,
//!     diff.summary.modified,
//!     diff.summary.removed);
//! ```

mod audit;
mod batch;
mod checkpoint;
mod chunker;
mod error;
mod hasher;
mod hierarchy;
mod limits;
mod manifest;
mod normalizer;
mod progress;
mod streaming;
mod types;

// Re-export public types
pub use audit::{
    AuditEntry, AuditLog, AuditOperation, IntegrityError, IntegrityReport, AUDIT_LOG_VERSION,
};
pub use batch::{BatchEmbedder, BatchRepoConfig, BatchRepoResult, BatchResult};
pub use checkpoint::{
    CheckpointError, CheckpointManager, CheckpointPhase, CheckpointRepoId, CheckpointStats,
    ChunkReference, EmbedCheckpoint, CHECKPOINT_VERSION,
};
pub use chunker::EmbedChunker;
pub use error::{sanitize_path, sanitize_pathbuf, EmbedError, SafePath};
pub use hasher::{hash_content, HashResult};
pub use hierarchy::{
    get_hierarchy_summary, ChildReference, HierarchyBuilder, HierarchyConfig, HierarchySummary,
};
pub use limits::ResourceLimits;
pub use manifest::{
    BatchOperation, DiffBatch, DiffSummary, EmbedDiff, EmbedManifest, ManifestEntry, ModifiedChunk,
    RemovedChunk, MANIFEST_VERSION,
};
pub use normalizer::{needs_normalization, normalize_for_hash};
pub use progress::{ProgressReporter, QuietProgress, TerminalProgress};
pub use streaming::{
    BatchIterator, Batches, CancellationHandle, ChunkStream, StreamConfig, StreamStats,
};
pub use types::{
    ChunkContext, ChunkKind, ChunkPart, ChunkSource, EmbedChunk, EmbedSettings, RepoIdentifier,
    Visibility,
};
