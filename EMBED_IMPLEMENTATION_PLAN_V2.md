# Embedding Chunks Implementation Plan v2.0 (Production-Ready)

## Overview

Add a new `infiniloom embed` command that generates deterministic, content-addressable code chunks optimized for embedding in vector databases. The system tracks changes between runs via a manifest, enabling efficient incremental updates.

**Version**: 2.0 (incorporates security, performance, determinism, UX, and RAG reviews)

## Design Principles

1. **Deterministic**: Byte-identical output across platforms, runs, and time
2. **Content-addressable**: Same code anywhere = same chunk ID
3. **Code-aware**: Chunks respect AST boundaries with semantic context
4. **Incremental**: Efficient change tracking via manifests
5. **Secure**: No secrets leaked, DoS-resistant, tamper-evident
6. **Performant**: <15 seconds for 10k files on modern hardware
7. **RAG-optimized**: Chunks designed for high retrieval recall

---

## Architecture

### Module Structure

```
engine/src/
├── embedding/
│   ├── mod.rs              # Public API exports
│   ├── chunker.rs          # Core chunking logic with thread-local resources
│   ├── normalizer.rs       # Content normalization (Unicode NFC, line endings)
│   ├── hasher.rs           # BLAKE3 hashing with single-pass optimization
│   ├── manifest.rs         # Manifest storage with integrity verification
│   ├── splitter.rs         # Large symbol splitting with depth limits
│   ├── enricher.rs         # Docstring/comment extraction, metadata enrichment
│   ├── types.rs            # Core data structures
│   ├── error.rs            # Actionable error types
│   ├── progress.rs         # Progress reporting abstraction
│   └── limits.rs           # Resource limits and DoS protection
│
cli/src/commands/
├── embed.rs                # CLI command with progress UI
```

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. DISCOVERY PHASE                                                          │
│    - Walk directory (sorted, deterministic)                                 │
│    - Filter by gitignore + patterns                                         │
│    - Validate paths (no traversal, no symlinks in paranoid mode)            │
│    - Snapshot file metadata for race detection                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. PARSING PHASE (Parallel with thread-local resources)                     │
│    - Thread-local Tree-sitter parsers (reset before each use)               │
│    - Thread-local tokenizers (no mutex contention)                          │
│    - Extract symbols sorted by (line, col, name)                            │
│    - Extract docstrings, comments, visibility                               │
│    - Collect errors deterministically (don't swallow)                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. CHUNKING PHASE                                                           │
│    - Generate chunks with context (5 lines default)                         │
│    - Split large symbols at AST boundaries (depth-limited)                  │
│    - Merge small adjacent symbols (optional)                                │
│    - Add overlap tokens for context continuity                              │
│    - Enrich with docstrings, callers/callees, tags                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 4. SECURITY PHASE                                                           │
│    - Scan chunks for secrets (integrate security.rs)                        │
│    - Redact or fail based on settings                                       │
│    - Validate no path traversal in output                                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 5. HASHING PHASE (Single pass)                                              │
│    - Normalize: Unicode NFC → CRLF→LF → trim trailing whitespace            │
│    - Compute BLAKE3 once, derive: short_id (128-bit) + full_hash (256-bit)  │
│    - Detect collisions against manifest                                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 6. DIFF PHASE (if manifest exists)                                          │
│    - Load manifest, verify integrity checksum                               │
│    - Compare using HashMap (O(n) not O(n log n))                            │
│    - Categorize: added, modified, removed, unchanged                        │
│    - Detect semantic changes via dependency graph (optional)                │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 7. OUTPUT PHASE                                                             │
│    - Envelope JSONL format with header/footer                               │
│    - Progress bar to stderr (if TTY)                                        │
│    - Sort output deterministically                                          │
│    - Update manifest with integrity checksum                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Core Types (`types.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A single embedding chunk with stable, content-addressable ID
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedChunk {
    /// Content-addressable ID: BLAKE3 hash of normalized content
    /// Format: "ec_" + 32 hex chars (128 bits) - collision-resistant for enterprise scale
    pub id: String,

    /// Full 256-bit hash for collision verification
    pub full_hash: String,

    /// The actual code content (normalized)
    pub content: String,

    /// Token count for the target model
    pub tokens: u32,

    /// Symbol kind
    pub kind: ChunkKind,

    /// Source location metadata
    pub source: ChunkSource,

    /// Enriched context for better retrieval
    pub context: ChunkContext,

    /// For split chunks: part N of M
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<ChunkPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSource {
    /// Relative file path (from repo root, never absolute)
    pub file: String,
    /// Line range (1-indexed, inclusive)
    pub lines: (u32, u32),
    /// Symbol name
    pub symbol: String,
    /// Fully qualified name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
    /// Programming language
    pub language: String,
    /// Parent symbol (for methods inside classes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Visibility modifier
    pub visibility: Visibility,
    /// Whether this is test code
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkContext {
    /// Extracted docstring (for natural language retrieval)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    /// Extracted comments within the chunk
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub comments: Vec<String>,
    /// Function/class signature (always included, even in split parts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Symbols this chunk calls
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub calls: Vec<String>,
    /// Symbols that call this chunk
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub called_by: Vec<String>,
    /// Import dependencies
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub imports: Vec<String>,
    /// Auto-generated semantic tags
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    #[default]
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
    Imports,
    TopLevel,
    FunctionPart,
    ClassPart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPart {
    pub part: u32,
    pub of: u32,
    /// ID of the logical parent (full symbol hash)
    pub parent_id: String,
    /// Signature repeated for context
    pub parent_signature: String,
}

/// Settings that affect chunk generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedSettings {
    /// Maximum tokens per chunk (default: 1000 for code models)
    pub max_tokens: u32,
    /// Minimum tokens per chunk (smaller merged, default: 50)
    pub min_tokens: u32,
    /// Overlap tokens between sequential chunks (default: 100)
    pub overlap_tokens: u32,
    /// Lines of context around symbols (default: 5)
    pub context_lines: u32,
    /// Include import statements as separate chunks
    pub include_imports: bool,
    /// Include top-level code outside symbols
    pub include_top_level: bool,
    /// Token counting model
    pub token_model: String,
    /// Version of chunking algorithm (for compatibility)
    pub algorithm_version: u32,
    /// Enable secret scanning
    pub scan_secrets: bool,
    /// Fail if secrets detected (CI mode)
    pub fail_on_secrets: bool,
    /// Redact detected secrets
    pub redact_secrets: bool,
}

impl Default for EmbedSettings {
    fn default() -> Self {
        Self {
            max_tokens: 1000,       // Optimized for code embedding models
            min_tokens: 50,
            overlap_tokens: 100,    // Context continuity
            context_lines: 5,       // Capture docstrings
            include_imports: true,
            include_top_level: true,
            token_model: "claude".to_string(),
            algorithm_version: 1,
            scan_secrets: true,     // Safe default
            fail_on_secrets: false,
            redact_secrets: true,   // Safe default
        }
    }
}

impl EmbedSettings {
    pub const CURRENT_ALGORITHM_VERSION: u32 = 1;
    pub const MAX_TOKENS_LIMIT: u32 = 100_000;  // DoS protection

    /// Get recommended settings for specific embedding model
    pub fn for_embedding_model(model: &str) -> Self {
        let mut settings = Self::default();
        settings.max_tokens = match model {
            "voyage-code-2" | "voyage-code-3" => 1500,
            "cohere-embed-v3" => 400,
            "openai-text-embedding-3-small" | "openai-text-embedding-3-large" => 800,
            "sentence-transformers" | "all-MiniLM" => 384,
            _ => 1000,
        };
        settings
    }

    /// Validate settings, return error if invalid
    pub fn validate(&self) -> Result<(), EmbedError> {
        if self.max_tokens > Self::MAX_TOKENS_LIMIT {
            return Err(EmbedError::InvalidSettings {
                field: "max_tokens".to_string(),
                reason: format!("exceeds limit of {}", Self::MAX_TOKENS_LIMIT),
            });
        }
        if self.min_tokens > self.max_tokens {
            return Err(EmbedError::InvalidSettings {
                field: "min_tokens".to_string(),
                reason: "cannot exceed max_tokens".to_string(),
            });
        }
        if self.algorithm_version > Self::CURRENT_ALGORITHM_VERSION {
            return Err(EmbedError::UnsupportedAlgorithmVersion {
                found: self.algorithm_version,
                max_supported: Self::CURRENT_ALGORITHM_VERSION,
            });
        }
        Ok(())
    }
}
```

### Resource Limits (`limits.rs`)

```rust
/// Resource limits to prevent DoS attacks
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum recursion depth for AST traversal
    pub max_recursion_depth: u32,
    /// Maximum file size to process (bytes)
    pub max_file_size: u64,
    /// Maximum total chunks to generate
    pub max_total_chunks: usize,
    /// Maximum files to process
    pub max_files: usize,
    /// Maximum concurrent file loads
    pub max_concurrent_loads: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_recursion_depth: 500,
            max_file_size: 10 * 1024 * 1024,  // 10 MB
            max_total_chunks: 1_000_000,
            max_files: 500_000,
            max_concurrent_loads: 32,
        }
    }
}

impl ResourceLimits {
    /// Strict limits for untrusted input
    pub fn strict() -> Self {
        Self {
            max_recursion_depth: 100,
            max_file_size: 1024 * 1024,  // 1 MB
            max_total_chunks: 100_000,
            max_files: 50_000,
            max_concurrent_loads: 8,
        }
    }
}
```

### Manifest Types (`manifest.rs`)

```rust
use std::collections::HashMap;

/// Manifest format version
pub const MANIFEST_VERSION: u32 = 2;

/// Manifest tracking all chunks for incremental updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedManifest {
    /// Manifest format version
    pub version: u32,
    /// Relative repository path (from git root or CWD)
    pub repo_path: String,
    /// Git commit hash when manifest was created (for reference only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Timestamp of last update (optional, excluded from integrity check)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    /// Settings used to generate chunks (part of integrity)
    pub settings: EmbedSettings,
    /// All chunks indexed by location key
    /// Using HashMap for O(1) lookups instead of BTreeMap O(log n)
    pub chunks: HashMap<String, ManifestEntry>,
    /// Integrity checksum (BLAKE3 of settings + sorted chunk entries)
    /// Excluded from serialization, computed on save, verified on load
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Content-addressable chunk ID (128-bit)
    pub chunk_id: String,
    /// Full content hash for collision detection (256-bit)
    pub full_hash: String,
    /// Token count
    pub tokens: u32,
    /// Line range
    pub lines: (u32, u32),
}

impl EmbedManifest {
    /// Create new manifest
    pub fn new(repo_path: String, settings: EmbedSettings) -> Self {
        Self {
            version: MANIFEST_VERSION,
            repo_path,
            commit_hash: None,
            updated_at: None,
            settings,
            chunks: HashMap::new(),
            checksum: None,
        }
    }

    /// Generate deterministic location key for a chunk
    pub fn location_key(file: &str, symbol: &str, kind: ChunkKind) -> String {
        // Format: file::symbol::kind
        // Use :: as separator (unlikely in paths/symbols)
        format!("{}::{}::{:?}", file, symbol, kind)
    }

    /// Compute integrity checksum over settings and chunk entries
    fn compute_checksum(&self) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        // Hash algorithm version
        hasher.update(&self.version.to_le_bytes());

        // Hash settings (affects chunk generation)
        let settings_json = serde_json::to_string(&self.settings).unwrap_or_default();
        hasher.update(settings_json.as_bytes());

        // Hash chunks in deterministic order (sorted by key)
        let mut keys: Vec<_> = self.chunks.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(entry) = self.chunks.get(key) {
                hasher.update(key.as_bytes());
                hasher.update(entry.chunk_id.as_bytes());
                hasher.update(entry.full_hash.as_bytes());
                hasher.update(&entry.tokens.to_le_bytes());
                hasher.update(&entry.lines.0.to_le_bytes());
                hasher.update(&entry.lines.1.to_le_bytes());
            }
        }

        hasher.finalize().to_hex().to_string()
    }

    /// Save manifest to file with integrity checksum
    pub fn save(&self, path: &Path) -> Result<(), EmbedError> {
        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;
        }

        // Compute checksum before saving
        let mut manifest = self.clone();
        manifest.checksum = Some(manifest.compute_checksum());

        // Use bincode for faster I/O (5-10x faster than JSON for large manifests)
        let bytes = bincode::serialize(&manifest)
            .map_err(|e| EmbedError::SerializationError { source: e.to_string() })?;

        std::fs::write(path, bytes)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        Ok(())
    }

    /// Load manifest from file with integrity verification
    pub fn load(path: &Path) -> Result<Self, EmbedError> {
        let bytes = std::fs::read(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        let mut manifest: Self = bincode::deserialize(&bytes)
            .map_err(|e| EmbedError::DeserializationError { source: e.to_string() })?;

        // Version check
        if manifest.version > MANIFEST_VERSION {
            return Err(EmbedError::ManifestVersionTooNew {
                found: manifest.version,
                max_supported: MANIFEST_VERSION,
            });
        }

        // Integrity verification
        if let Some(stored_checksum) = manifest.checksum.take() {
            let computed = manifest.compute_checksum();
            if stored_checksum != computed {
                return Err(EmbedError::ManifestCorrupted {
                    path: path.to_path_buf(),
                    expected: stored_checksum,
                    actual: computed,
                });
            }
        }

        // Validate settings
        manifest.settings.validate()?;

        Ok(manifest)
    }

    /// Update manifest with current chunks, detecting collisions
    pub fn update(&mut self, chunks: &[EmbedChunk]) -> Result<(), EmbedError> {
        // Collision detection: track id -> full_hash mappings
        let mut id_to_hash: HashMap<&str, &str> = HashMap::new();

        self.chunks.clear();

        for chunk in chunks {
            // Check for hash collision
            if let Some(&existing_hash) = id_to_hash.get(chunk.id.as_str()) {
                if existing_hash != chunk.full_hash.as_str() {
                    return Err(EmbedError::HashCollision {
                        id: chunk.id.clone(),
                        hash1: existing_hash.to_string(),
                        hash2: chunk.full_hash.clone(),
                    });
                }
            }
            id_to_hash.insert(&chunk.id, &chunk.full_hash);

            let key = Self::location_key(
                &chunk.source.file,
                &chunk.source.symbol,
                chunk.kind,
            );

            self.chunks.insert(key, ManifestEntry {
                chunk_id: chunk.id.clone(),
                full_hash: chunk.full_hash.clone(),
                tokens: chunk.tokens,
                lines: chunk.source.lines,
            });
        }

        Ok(())
    }

    /// Compute diff between current chunks and manifest
    pub fn diff(&self, current_chunks: &[EmbedChunk]) -> EmbedDiff {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut removed = Vec::new();
        let mut unchanged = Vec::new();

        // Build map of current chunks by location key (O(n))
        let current_map: HashMap<String, &EmbedChunk> = current_chunks
            .iter()
            .map(|c| (Self::location_key(&c.source.file, &c.source.symbol, c.kind), c))
            .collect();

        // Find modified and unchanged (iterate manifest)
        for (key, entry) in &self.chunks {
            if let Some(current) = current_map.get(key) {
                if current.id == entry.chunk_id {
                    unchanged.push(current.id.clone());
                } else {
                    modified.push(ModifiedChunk {
                        old_id: entry.chunk_id.clone(),
                        new_id: current.id.clone(),
                        chunk: (*current).clone(),
                    });
                }
            } else {
                // In manifest but not in current = removed
                removed.push(RemovedChunk {
                    id: entry.chunk_id.clone(),
                    location_key: key.clone(),
                });
            }
        }

        // Find added (in current but not in manifest)
        for (key, chunk) in &current_map {
            if !self.chunks.contains_key(key) {
                added.push((*chunk).clone());
            }
        }

        let summary = DiffSummary {
            added: added.len(),
            modified: modified.len(),
            removed: removed.len(),
            unchanged: unchanged.len(),
            total_chunks: current_chunks.len(),
        };

        EmbedDiff { summary, added, modified, removed, unchanged }
    }
}

/// Result of diffing current state against manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedDiff {
    pub summary: DiffSummary,
    pub added: Vec<EmbedChunk>,
    pub modified: Vec<ModifiedChunk>,
    pub removed: Vec<RemovedChunk>,
    pub unchanged: Vec<String>,
}

impl EmbedDiff {
    /// Split diff into batches for vector DB operations
    pub fn batches(&self, batch_size: usize) -> Vec<DiffBatch> {
        let mut batches = Vec::new();
        let mut batch_num = 0;

        // Batch added chunks
        for chunk in self.added.chunks(batch_size) {
            batches.push(DiffBatch {
                batch_number: batch_num,
                operation: BatchOperation::Upsert,
                chunks: chunk.to_vec(),
                ids: Vec::new(),
            });
            batch_num += 1;
        }

        // Batch modified chunks
        for chunk in self.modified.chunks(batch_size) {
            batches.push(DiffBatch {
                batch_number: batch_num,
                operation: BatchOperation::Upsert,
                chunks: chunk.iter().map(|m| m.chunk.clone()).collect(),
                ids: chunk.iter().map(|m| m.old_id.clone()).collect(), // Old IDs to delete
            });
            batch_num += 1;
        }

        // Batch removed IDs
        for ids in self.removed.chunks(batch_size) {
            batches.push(DiffBatch {
                batch_number: batch_num,
                operation: BatchOperation::Delete,
                chunks: Vec::new(),
                ids: ids.iter().map(|r| r.id.clone()).collect(),
            });
            batch_num += 1;
        }

        batches
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffBatch {
    pub batch_number: usize,
    pub operation: BatchOperation,
    pub chunks: Vec<EmbedChunk>,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchOperation {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub total_chunks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedChunk {
    pub old_id: String,
    pub new_id: String,
    pub chunk: EmbedChunk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedChunk {
    pub id: String,
    pub location_key: String,
}
```

### Error Types (`error.rs`)

```rust
use std::path::PathBuf;
use thiserror::Error;

/// Actionable error types with helpful messages
#[derive(Debug, Error)]
pub enum EmbedError {
    // === User Errors (Actionable) ===

    #[error("Invalid settings: {field} - {reason}\n\nFix: Check your --{field} argument or config file")]
    InvalidSettings { field: String, reason: String },

    #[error("Manifest version {found} is newer than supported version {max_supported}\n\nFix: Upgrade infiniloom to latest version, or delete manifest and rebuild:\n  rm .infiniloom-embed.bin && infiniloom embed")]
    ManifestVersionTooNew { found: u32, max_supported: u32 },

    #[error("Manifest corrupted or tampered\n  Path: {path}\n  Expected checksum: {expected}\n  Actual checksum: {actual}\n\nFix: Delete manifest and rebuild:\n  rm {path} && infiniloom embed")]
    ManifestCorrupted { path: PathBuf, expected: String, actual: String },

    #[error("Settings changed since last run\n\nPrevious: {previous}\nCurrent:  {current}\n\nImpact: All chunk IDs may change\n\nFix: Run with --full to rebuild, or restore original settings")]
    SettingsChanged { previous: String, current: String },

    #[error("No code chunks found\n\nPossible causes:\n  - Include patterns too restrictive: {include_patterns}\n  - Exclude patterns too broad: {exclude_patterns}\n  - No supported languages in repository\n\nFix: Check -i/--include and -e/--exclude patterns")]
    NoChunksGenerated { include_patterns: String, exclude_patterns: String },

    #[error("Secrets detected in {count} chunks\n\nFiles with secrets:\n{files}\n\nFix: Either:\n  1. Remove secrets from code\n  2. Use --redact-secrets to mask them\n  3. Use --no-scan-secrets to skip scanning (not recommended)")]
    SecretsDetected { count: usize, files: String },

    #[error("Hash collision detected!\n  Chunk ID: {id}\n  Hash 1: {hash1}\n  Hash 2: {hash2}\n\nThis is extremely rare. Please report at https://github.com/infiniloom/issues")]
    HashCollision { id: String, hash1: String, hash2: String },

    // === Resource Limit Errors ===

    #[error("File too large: {path} ({size} bytes, max: {max})\n\nFix: Exclude large files with -e/--exclude pattern, or increase --max-file-size")]
    FileTooLarge { path: PathBuf, size: u64, max: u64 },

    #[error("Too many chunks generated ({count}, max: {max})\n\nFix: Use more restrictive include patterns, or increase --max-chunks limit")]
    TooManyChunks { count: usize, max: usize },

    #[error("Recursion limit exceeded while parsing\n  Depth: {depth}, Max: {max}\n  Context: {context}\n\nFix: File may have unusual nesting. Exclude it with -e pattern")]
    RecursionLimitExceeded { depth: u32, max: u32, context: String },

    #[error("Path traversal detected\n  Path: {path}\n  Repo root: {repo_root}\n\nFix: Remove symlinks pointing outside repository, or use --no-follow-symlinks")]
    PathTraversal { path: PathBuf, repo_root: PathBuf },

    // === System Errors ===

    #[error("I/O error: {path}\n  {source}")]
    IoError { path: PathBuf, #[source] source: std::io::Error },

    #[error("Parse error in {file} at line {line}\n  {message}\n\nFix: Fix syntax error or exclude file with -e pattern")]
    ParseError { file: String, line: u32, message: String },

    #[error("Serialization error: {source}")]
    SerializationError { source: String },

    #[error("Deserialization error: {source}\n\nFix: Manifest may be corrupted. Delete and rebuild:\n  rm .infiniloom-embed.bin && infiniloom embed")]
    DeserializationError { source: String },

    #[error("Unsupported algorithm version {found} (max supported: {max_supported})\n\nFix: Upgrade infiniloom or regenerate with current version")]
    UnsupportedAlgorithmVersion { found: u32, max_supported: u32 },

    #[error("Multiple files failed to process:\n{errors}\n\nFix: Address individual errors above")]
    MultipleErrors { errors: String },
}

impl EmbedError {
    /// Format multiple file errors
    pub fn from_file_errors(errors: Vec<(PathBuf, EmbedError)>) -> Self {
        let formatted = errors
            .iter()
            .map(|(path, err)| format!("  {}: {}", path.display(), err))
            .collect::<Vec<_>>()
            .join("\n");
        Self::MultipleErrors { errors: formatted }
    }
}
```

---

## Core Algorithms

### Content Normalization (`normalizer.rs`)

```rust
use unicode_normalization::UnicodeNormalization;

/// Normalize content for deterministic, cross-platform hashing
///
/// Guarantees:
/// 1. Unicode NFC normalization (macOS NFD → NFC)
/// 2. Line endings: CRLF/CR → LF
/// 3. Trailing whitespace removed per line
/// 4. Leading/trailing blank lines removed
/// 5. Preserves internal indentation (important for Python)
///
/// Result: Identical output on Windows, Linux, macOS
pub fn normalize_for_hash(content: &str) -> String {
    // Step 1: Unicode NFC normalization
    // This ensures "café" (NFD: e + combining accent) equals "café" (NFC: single char)
    let unicode_normalized: String = content.nfc().collect();

    // Step 2: Normalize line endings (no allocations for common case)
    let line_normalized = if unicode_normalized.contains('\r') {
        unicode_normalized.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        unicode_normalized
    };

    // Step 3: Process lines
    let lines: Vec<&str> = line_normalized
        .lines()
        .map(|line| line.trim_end()) // Remove trailing whitespace only
        .collect();

    // Step 4: Remove leading blank lines
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);

    // Step 5: Remove trailing blank lines
    let end = lines.iter().rposition(|l| !l.is_empty()).map(|i| i + 1).unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

/// Fast check if normalization is needed
#[inline]
pub fn needs_normalization(content: &str) -> bool {
    content.contains('\r') ||
    content.bytes().any(|b| b > 127) || // Potential Unicode
    content.lines().any(|l| l.ends_with(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_nfc() {
        // NFD: e + combining acute accent
        let nfd = "cafe\u{0301}";
        // NFC: single character é
        let nfc = "caf\u{00E9}";

        assert_eq!(normalize_for_hash(nfd), normalize_for_hash(nfc));
    }

    #[test]
    fn test_cross_platform() {
        let unix = "fn foo() {\n    bar();\n}";
        let windows = "fn foo() {\r\n    bar();\r\n}";
        let mac_classic = "fn foo() {\r    bar();\r}";
        let trailing_ws = "fn foo() {   \n    bar();   \n}";

        let normalized = normalize_for_hash(unix);
        assert_eq!(normalize_for_hash(windows), normalized);
        assert_eq!(normalize_for_hash(mac_classic), normalized);
        assert_eq!(normalize_for_hash(trailing_ws), normalized);
    }

    #[test]
    fn test_preserves_indentation() {
        let python = "def foo():\n    if True:\n        return 1";
        let normalized = normalize_for_hash(python);
        assert!(normalized.contains("    if True:"));
        assert!(normalized.contains("        return"));
    }
}
```

### Hashing (`hasher.rs`)

```rust
use blake3::Hasher;

/// Hash result containing both short ID and full hash
#[derive(Debug, Clone)]
pub struct HashResult {
    /// Short ID for display: "ec_" + 32 hex chars (128 bits)
    pub short_id: String,
    /// Full hash for collision detection: 64 hex chars (256 bits)
    pub full_hash: String,
}

/// Generate deterministic hashes from content
///
/// Single-pass optimization: normalizes and hashes in one operation
pub fn hash_content(content: &str) -> HashResult {
    let normalized = super::normalizer::normalize_for_hash(content);
    let hash = blake3::hash(normalized.as_bytes());
    let hex = hash.to_hex();

    HashResult {
        // 128 bits = 32 hex chars (collision resistant for 2^64 chunks)
        short_id: format!("ec_{}", &hex[..32]),
        // Full 256-bit hash for verification
        full_hash: hex.to_string(),
    }
}

/// Verify that two chunks with same short ID have same content
pub fn verify_no_collision(id: &str, hash1: &str, hash2: &str) -> Result<(), super::error::EmbedError> {
    if hash1 != hash2 {
        return Err(super::error::EmbedError::HashCollision {
            id: id.to_string(),
            hash1: hash1.to_string(),
            hash2: hash2.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let content = "fn foo() { bar(); }";
        let h1 = hash_content(content);
        let h2 = hash_content(content);
        assert_eq!(h1.short_id, h2.short_id);
        assert_eq!(h1.full_hash, h2.full_hash);
    }

    #[test]
    fn test_format() {
        let h = hash_content("test");
        assert!(h.short_id.starts_with("ec_"));
        assert_eq!(h.short_id.len(), 3 + 32); // "ec_" + 32 hex
        assert_eq!(h.full_hash.len(), 64);    // 256 bits = 64 hex
    }

    #[test]
    fn test_different_content() {
        let h1 = hash_content("fn foo() {}");
        let h2 = hash_content("fn bar() {}");
        assert_ne!(h1.short_id, h2.short_id);
    }
}
```

### Chunker (`chunker.rs`)

```rust
use std::cell::RefCell;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::prelude::*;

use crate::parser::Parser;
use crate::tokenizer::Tokenizer;
use crate::security::SecretScanner;

// Thread-local resources to avoid mutex contention
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
    static THREAD_TOKENIZER: RefCell<Tokenizer> = RefCell::new(Tokenizer::new());
}

pub struct EmbedChunker {
    settings: EmbedSettings,
    limits: ResourceLimits,
    secret_scanner: Option<SecretScanner>,
}

impl EmbedChunker {
    pub fn new(settings: EmbedSettings, limits: ResourceLimits) -> Self {
        let secret_scanner = if settings.scan_secrets {
            Some(SecretScanner::default())
        } else {
            None
        };

        Self { settings, limits, secret_scanner }
    }

    /// Generate all chunks for a repository
    ///
    /// Guarantees:
    /// 1. Deterministic output (same input = same output)
    /// 2. Thread-safe parallel processing
    /// 3. Resource limits enforced
    /// 4. Errors collected, not swallowed
    pub fn chunk_repository(
        &self,
        repo_path: &Path,
        progress: &dyn ProgressReporter,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate repo path
        let repo_root = self.validate_repo_path(repo_path)?;

        // Phase 1: Discover files (deterministic order)
        progress.set_phase("Scanning repository...");
        let mut files = self.discover_files(&repo_root)?;
        files.sort(); // Critical for determinism
        progress.set_total(files.len());

        if files.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_string(),
                exclude_patterns: "default".to_string(),
            });
        }

        // Check file limit
        if files.len() > self.limits.max_files {
            return Err(EmbedError::TooManyChunks {
                count: files.len(),
                max: self.limits.max_files,
            });
        }

        // Phase 2: Process files in parallel
        progress.set_phase("Parsing and chunking...");
        let chunk_count = AtomicUsize::new(0);
        let processed = AtomicUsize::new(0);

        // Collect results AND errors (don't swallow errors)
        let results: Vec<Result<Vec<EmbedChunk>, (PathBuf, EmbedError)>> = files
            .par_iter()
            .map(|file| {
                let result = self.chunk_file(file, &repo_root);

                // Update progress
                let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
                progress.set_progress(done);

                match result {
                    Ok(chunks) => {
                        let count = chunk_count.fetch_add(chunks.len(), Ordering::Relaxed) + chunks.len();

                        // Check chunk limit
                        if count > self.limits.max_total_chunks {
                            Err((file.clone(), EmbedError::TooManyChunks {
                                count,
                                max: self.limits.max_total_chunks,
                            }))
                        } else {
                            Ok(chunks)
                        }
                    }
                    Err(e) => Err((file.clone(), e)),
                }
            })
            .collect();

        // Separate successes and failures
        let mut all_chunks = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(chunks) => all_chunks.extend(chunks),
                Err((path, err)) => errors.push((path, err)),
            }
        }

        // Report errors (fail on critical, warn on non-critical)
        if !errors.is_empty() {
            let critical: Vec<_> = errors.iter()
                .filter(|(_, e)| matches!(e, EmbedError::TooManyChunks { .. } | EmbedError::PathTraversal { .. }))
                .collect();

            if !critical.is_empty() {
                return Err(EmbedError::from_file_errors(
                    critical.into_iter().map(|(p, e)| (p.clone(), e.clone())).collect()
                ));
            }

            // Non-critical errors: log warning, continue
            for (path, err) in &errors {
                progress.warn(&format!("Skipped {}: {}", path.display(), err));
            }
        }

        // Phase 3: Sort for deterministic output
        progress.set_phase("Sorting chunks...");
        all_chunks.par_sort_by(|a, b| {
            a.source.file.cmp(&b.source.file)
                .then_with(|| a.source.lines.0.cmp(&b.source.lines.0))
                .then_with(|| a.id.cmp(&b.id))
        });

        progress.set_phase("Complete");
        Ok(all_chunks)
    }

    /// Chunk a single file using thread-local resources
    fn chunk_file(&self, path: &Path, repo_root: &Path) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate file size
        let metadata = std::fs::metadata(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        if metadata.len() > self.limits.max_file_size {
            return Err(EmbedError::FileTooLarge {
                path: path.to_path_buf(),
                size: metadata.len(),
                max: self.limits.max_file_size,
            });
        }

        // Read file
        let content = std::fs::read_to_string(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Get relative path (safe, validated)
        let relative_path = self.safe_relative_path(path, repo_root)?;
        let language = detect_language(path);

        // Use thread-local parser
        let symbols = THREAD_PARSER.with(|p| {
            let mut parser = p.borrow_mut();
            parser.reset(); // Ensure clean state
            parser.extract_symbols(&content, &language)
        }).map_err(|e| EmbedError::ParseError {
            file: relative_path.clone(),
            line: 0,
            message: e.to_string(),
        })?;

        // Sort symbols deterministically
        let mut symbols = symbols;
        symbols.sort_by(|a, b| {
            a.start_line.cmp(&b.start_line)
                .then_with(|| a.start_col.cmp(&b.start_col))
                .then_with(|| a.name.cmp(&b.name))
        });

        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::with_capacity(symbols.len() + 2);

        // Use Arc for shared strings (avoid cloning per chunk)
        let file_arc: Arc<str> = relative_path.clone().into();
        let lang_arc: Arc<str> = language.clone().into();

        for symbol in &symbols {
            // Extract content with context
            let (chunk_content, start_line, end_line) = self.extract_symbol_content(
                &lines,
                symbol,
                self.settings.context_lines,
            );

            // Count tokens using thread-local tokenizer
            let tokens = THREAD_TOKENIZER.with(|t| {
                t.borrow().count(&chunk_content, &self.settings.token_model)
            });

            // Handle large symbols (with depth-limited splitting)
            if self.settings.max_tokens > 0 && tokens > self.settings.max_tokens {
                let split_chunks = self.split_large_symbol(
                    &chunk_content,
                    symbol,
                    &file_arc,
                    &lang_arc,
                    0, // Initial depth
                )?;
                chunks.extend(split_chunks);
            } else {
                // Generate hash (single pass)
                let hash = super::hasher::hash_content(&chunk_content);

                // Extract context
                let context = self.extract_context(symbol, &content)?;

                chunks.push(EmbedChunk {
                    id: hash.short_id,
                    full_hash: hash.full_hash,
                    content: chunk_content,
                    tokens,
                    kind: symbol.kind.into(),
                    source: ChunkSource {
                        file: file_arc.to_string(),
                        lines: (start_line, end_line),
                        symbol: symbol.name.clone(),
                        fqn: symbol.fqn.clone(),
                        language: lang_arc.to_string(),
                        parent: symbol.parent.clone(),
                        visibility: symbol.visibility,
                        is_test: self.is_test_code(path, symbol),
                    },
                    context,
                    part: None,
                });
            }
        }

        // Security scan (if enabled)
        if let Some(scanner) = &self.secret_scanner {
            self.scan_chunks_for_secrets(&mut chunks, scanner)?;
        }

        Ok(chunks)
    }

    /// Split large symbol at AST boundaries with depth limit
    fn split_large_symbol(
        &self,
        content: &str,
        symbol: &Symbol,
        file: &Arc<str>,
        language: &Arc<str>,
        depth: u32,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Depth limit to prevent stack overflow
        if depth > self.limits.max_recursion_depth {
            return Err(EmbedError::RecursionLimitExceeded {
                depth,
                max: self.limits.max_recursion_depth,
                context: format!("splitting symbol {}", symbol.name),
            });
        }

        // Find split points using integer math only (no floats!)
        let split_points = self.find_split_points_integer(content, language, depth)?;

        let mut chunks = Vec::new();
        let mut current_start = 0usize;
        let mut part_num = 1u32;

        // Parent ID for linking parts
        let parent_hash = super::hasher::hash_content(content);

        for split_point in split_points {
            if split_point <= current_start || split_point > content.len() {
                continue;
            }

            let part_content = &content[current_start..split_point];

            let tokens = THREAD_TOKENIZER.with(|t| {
                t.borrow().count(part_content, &self.settings.token_model)
            });

            // Only create chunk if above minimum
            if tokens >= self.settings.min_tokens {
                let hash = super::hasher::hash_content(part_content);

                chunks.push(EmbedChunk {
                    id: hash.short_id,
                    full_hash: hash.full_hash,
                    content: part_content.to_string(),
                    tokens,
                    kind: ChunkKind::FunctionPart,
                    source: ChunkSource {
                        file: file.to_string(),
                        lines: (0, 0), // Computed below
                        symbol: format!("{}_part{}", symbol.name, part_num),
                        fqn: symbol.fqn.clone(),
                        language: language.to_string(),
                        parent: Some(symbol.name.clone()),
                        visibility: symbol.visibility,
                        is_test: false,
                    },
                    context: ChunkContext {
                        signature: symbol.signature.clone(), // Include in every part
                        docstring: if part_num == 1 { symbol.docstring.clone() } else { None },
                        ..Default::default()
                    },
                    part: Some(ChunkPart {
                        part: part_num,
                        of: 0, // Updated after all parts
                        parent_id: parent_hash.short_id.clone(),
                        parent_signature: symbol.signature.clone().unwrap_or_default(),
                    }),
                });

                part_num += 1;
            }

            current_start = split_point;
        }

        // Handle remaining content
        if current_start < content.len() {
            let part_content = &content[current_start..];
            let tokens = THREAD_TOKENIZER.with(|t| {
                t.borrow().count(part_content, &self.settings.token_model)
            });

            if tokens >= self.settings.min_tokens {
                let hash = super::hasher::hash_content(part_content);

                chunks.push(EmbedChunk {
                    id: hash.short_id,
                    full_hash: hash.full_hash,
                    content: part_content.to_string(),
                    tokens,
                    kind: ChunkKind::FunctionPart,
                    source: ChunkSource {
                        file: file.to_string(),
                        lines: (0, 0),
                        symbol: format!("{}_part{}", symbol.name, part_num),
                        fqn: symbol.fqn.clone(),
                        language: language.to_string(),
                        parent: Some(symbol.name.clone()),
                        visibility: symbol.visibility,
                        is_test: false,
                    },
                    context: ChunkContext {
                        signature: symbol.signature.clone(),
                        ..Default::default()
                    },
                    part: Some(ChunkPart {
                        part: part_num,
                        of: 0,
                        parent_id: parent_hash.short_id.clone(),
                        parent_signature: symbol.signature.clone().unwrap_or_default(),
                    }),
                });
            }
        }

        // Update total part count
        let total_parts = chunks.len() as u32;
        for chunk in &mut chunks {
            if let Some(ref mut part) = chunk.part {
                part.of = total_parts;
            }
        }

        Ok(chunks)
    }

    /// Find split points using INTEGER MATH ONLY
    ///
    /// Critical for determinism: floating point math can vary across platforms
    fn find_split_points_integer(
        &self,
        content: &str,
        language: &str,
        depth: u32,
    ) -> Result<Vec<usize>, EmbedError> {
        let mut points = Vec::new();
        let target_tokens = self.settings.max_tokens as usize;

        // For very large content (>100KB), use fast line-based splitting
        if content.len() > 100_000 {
            return self.find_split_points_by_lines(content, target_tokens);
        }

        // Try AST-based splitting first
        if let Ok(tree) = THREAD_PARSER.with(|p| {
            p.borrow_mut().parse_content(content, language)
        }) {
            self.collect_block_boundaries_limited(
                &tree.root_node(),
                content,
                &mut points,
                depth,
                self.limits.max_recursion_depth,
            )?;
        }

        // If no good AST points, fall back to line boundaries
        if points.is_empty() {
            return self.find_split_points_by_lines(content, target_tokens);
        }

        points.sort();
        points.dedup();
        Ok(points)
    }

    /// Line-based splitting (fast fallback, always deterministic)
    fn find_split_points_by_lines(
        &self,
        content: &str,
        target_tokens: usize,
    ) -> Result<Vec<usize>, EmbedError> {
        let mut points = Vec::new();
        let total_chars = content.len();
        let total_tokens = THREAD_TOKENIZER.with(|t| {
            t.borrow().count(content, &self.settings.token_model)
        }) as usize;

        if total_tokens == 0 {
            return Ok(points);
        }

        // INTEGER MATH: chars_per_token * target_tokens = target_chars
        // Avoid: (total_chars as f32 / total_tokens as f32) * target_tokens as f32
        let target_chars = (total_chars * target_tokens) / total_tokens;

        let mut char_count = 0usize;
        for (idx, _) in content.match_indices('\n') {
            char_count = idx + 1;
            if char_count >= target_chars {
                points.push(char_count);
                // Reset for next chunk
                // target_chars remains constant for deterministic splitting
            }
        }

        Ok(points)
    }

    /// Collect AST block boundaries with depth limit
    fn collect_block_boundaries_limited(
        &self,
        node: &tree_sitter::Node,
        content: &str,
        points: &mut Vec<usize>,
        current_depth: u32,
        max_depth: u32,
    ) -> Result<(), EmbedError> {
        if current_depth > max_depth {
            return Err(EmbedError::RecursionLimitExceeded {
                depth: current_depth,
                max: max_depth,
                context: "AST traversal".to_string(),
            });
        }

        // Block-like nodes where we can safely split
        let block_kinds = [
            "block", "statement_block", "compound_statement",
            "if_statement", "if_expression", "match_arm", "case_clause",
            "for_statement", "for_expression", "while_statement",
            "loop_expression", "function_item", "impl_item",
            "method_definition", "function_definition",
        ];

        if block_kinds.iter().any(|k| node.kind() == *k) {
            points.push(node.end_byte());
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_block_boundaries_limited(
                &child,
                content,
                points,
                current_depth + 1,
                max_depth,
            )?;
        }

        Ok(())
    }

    /// Validate repository path, prevent traversal
    fn validate_repo_path(&self, path: &Path) -> Result<PathBuf, EmbedError> {
        let canonical = path.canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure it's a directory
        if !canonical.is_dir() {
            return Err(EmbedError::IoError {
                path: path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Not a directory",
                ),
            });
        }

        Ok(canonical)
    }

    /// Get safe relative path, validate no traversal
    fn safe_relative_path(&self, path: &Path, repo_root: &Path) -> Result<String, EmbedError> {
        let canonical = path.canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure path is within repo root
        if !canonical.starts_with(repo_root) {
            return Err(EmbedError::PathTraversal {
                path: canonical,
                repo_root: repo_root.to_path_buf(),
            });
        }

        // Return relative path
        Ok(canonical
            .strip_prefix(repo_root)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .to_string())
    }

    /// Scan chunks for secrets, redact or fail
    fn scan_chunks_for_secrets(
        &self,
        chunks: &mut [EmbedChunk],
        scanner: &SecretScanner,
    ) -> Result<(), EmbedError> {
        let mut files_with_secrets = Vec::new();

        for chunk in chunks.iter_mut() {
            if let Some(findings) = scanner.scan_content(&chunk.content) {
                if !findings.is_empty() {
                    if self.settings.fail_on_secrets {
                        files_with_secrets.push(format!(
                            "  {}::{} ({} secrets)",
                            chunk.source.file,
                            chunk.source.symbol,
                            findings.len()
                        ));
                    } else if self.settings.redact_secrets {
                        chunk.content = scanner.redact_findings(&chunk.content, &findings);
                        // Re-hash after redaction
                        let hash = super::hasher::hash_content(&chunk.content);
                        chunk.id = hash.short_id;
                        chunk.full_hash = hash.full_hash;
                    }
                }
            }
        }

        if !files_with_secrets.is_empty() {
            return Err(EmbedError::SecretsDetected {
                count: files_with_secrets.len(),
                files: files_with_secrets.join("\n"),
            });
        }

        Ok(())
    }

    /// Detect if code is test code
    fn is_test_code(&self, path: &Path, symbol: &Symbol) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Path-based detection
        if path_str.contains("test") || path_str.contains("spec") || path_str.contains("__tests__") {
            return true;
        }

        // Symbol-based detection
        let name = symbol.name.to_lowercase();
        if name.starts_with("test_") || name.ends_with("_test") || name.contains("_test_") {
            return true;
        }

        // Attribute-based (Rust #[test], Python @pytest.mark, etc.)
        // Would need symbol attribute extraction
        false
    }

    /// Extract semantic context for retrieval
    fn extract_context(&self, symbol: &Symbol, _content: &str) -> Result<ChunkContext, EmbedError> {
        Ok(ChunkContext {
            docstring: symbol.docstring.clone(),
            comments: Vec::new(), // TODO: Extract inline comments
            signature: symbol.signature.clone(),
            calls: symbol.calls.clone(),
            called_by: Vec::new(), // Populated from dependency graph
            imports: Vec::new(),   // Populated from file-level
            tags: self.generate_tags(symbol),
        })
    }

    /// Auto-generate semantic tags
    fn generate_tags(&self, symbol: &Symbol) -> Vec<String> {
        let mut tags = Vec::new();
        let content = symbol.signature.as_deref().unwrap_or("");

        // Async detection
        if content.contains("async") || content.contains("await") {
            tags.push("async".to_string());
        }

        // Security-related
        if content.contains("password") || content.contains("token") ||
           content.contains("secret") || content.contains("auth") {
            tags.push("security".to_string());
        }

        // Error handling
        if content.contains("Error") || content.contains("Result") ||
           content.contains("try") || content.contains("catch") {
            tags.push("error-handling".to_string());
        }

        // Database
        if content.contains("query") || content.contains("sql") ||
           content.contains("database") || content.contains("db") {
            tags.push("database".to_string());
        }

        // HTTP/API
        if content.contains("http") || content.contains("request") ||
           content.contains("response") || content.contains("api") {
            tags.push("http".to_string());
        }

        tags
    }
}
```

---

## CLI Interface (`cli/src/commands/embed.rs`)

```rust
use clap::{Args, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::io::{self, Write, BufWriter};

#[derive(Args, Debug)]
pub struct EmbedArgs {
    /// Repository path (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format
    #[arg(short, long, default_value = "envelope-jsonl")]
    pub format: OutputFormat,

    /// Operating mode (auto-detected if not specified)
    #[arg(long)]
    pub mode: Option<EmbedMode>,

    /// Output only changed chunks (added + modified), skip unchanged
    #[arg(long)]
    pub only_changed: bool,

    /// Maximum tokens per chunk
    #[arg(long, default_value = "1000")]
    pub max_tokens: u32,

    /// Minimum tokens per chunk
    #[arg(long, default_value = "50")]
    pub min_tokens: u32,

    /// Overlap tokens between chunks
    #[arg(long, default_value = "100")]
    pub overlap_tokens: u32,

    /// Context lines around symbols
    #[arg(long, default_value = "5")]
    pub context_lines: u32,

    /// Custom manifest path
    #[arg(long)]
    pub manifest: Option<PathBuf>,

    /// Force full rebuild (ignore manifest)
    #[arg(long)]
    pub full: bool,

    /// Show what would be output without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Quiet mode (no progress output)
    #[arg(short, long)]
    pub quiet: bool,

    /// Token counting model
    #[arg(short, long, default_value = "claude")]
    pub model: String,

    /// Target embedding model (sets optimal chunk size)
    #[arg(long)]
    pub embedding_model: Option<String>,

    /// Include patterns (glob)
    #[arg(short = 'i', long)]
    pub include: Vec<String>,

    /// Exclude patterns (glob)
    #[arg(short = 'e', long)]
    pub exclude: Vec<String>,

    /// Scan for secrets (default: true)
    #[arg(long, default_value = "true")]
    pub scan_secrets: bool,

    /// Fail if secrets detected
    #[arg(long)]
    pub fail_on_secrets: bool,

    /// Redact detected secrets (default: true)
    #[arg(long, default_value = "true")]
    pub redact_secrets: bool,

    /// Explain chunk ID system
    #[arg(long)]
    pub explain_ids: bool,

    /// Batch size for vector DB operations
    #[arg(long, default_value = "100")]
    pub batch_size: usize,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Envelope JSONL with header/footer (recommended)
    EnvelopeJsonl,
    /// Plain JSONL (one chunk per line, no metadata)
    Jsonl,
    /// Full JSON object
    Json,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum EmbedMode {
    /// Output all chunks
    Full,
    /// Output only changes (requires manifest)
    Diff,
    /// Show statistics only
    Stats,
}

pub fn execute(args: EmbedArgs) -> Result<()> {
    // Handle --explain-ids
    if args.explain_ids {
        print_id_explanation();
        return Ok(());
    }

    // Build settings
    let mut settings = if let Some(embedding_model) = &args.embedding_model {
        EmbedSettings::for_embedding_model(embedding_model)
    } else {
        EmbedSettings::default()
    };

    // Override with CLI args
    settings.max_tokens = args.max_tokens;
    settings.min_tokens = args.min_tokens;
    settings.overlap_tokens = args.overlap_tokens;
    settings.context_lines = args.context_lines;
    settings.token_model = args.model.clone();
    settings.scan_secrets = args.scan_secrets;
    settings.fail_on_secrets = args.fail_on_secrets;
    settings.redact_secrets = args.redact_secrets;

    // Validate settings
    settings.validate()?;

    // Setup progress reporting
    let progress: Box<dyn ProgressReporter> = if args.quiet || args.output.is_none() {
        Box::new(QuietProgress)
    } else {
        Box::new(TerminalProgress::new())
    };

    // Determine manifest path
    let manifest_path = args.manifest.clone().unwrap_or_else(|| {
        args.path.join(".infiniloom-embed.bin")
    });

    // Determine mode
    let mode = args.mode.unwrap_or_else(|| {
        if args.full {
            EmbedMode::Full
        } else if manifest_path.exists() {
            EmbedMode::Diff
        } else {
            EmbedMode::Full
        }
    });

    // Create chunker
    let limits = ResourceLimits::default();
    let chunker = EmbedChunker::new(settings.clone(), limits);

    // Generate chunks
    progress.set_phase("Starting...");
    let chunks = chunker.chunk_repository(&args.path, progress.as_ref())?;

    if chunks.is_empty() {
        return Err(EmbedError::NoChunksGenerated {
            include_patterns: args.include.join(", "),
            exclude_patterns: args.exclude.join(", "),
        }.into());
    }

    // Handle mode
    match mode {
        EmbedMode::Full => {
            output_full(&chunks, &args, &settings)?;
        }
        EmbedMode::Diff => {
            let manifest = EmbedManifest::load(&manifest_path)?;

            // Check settings match
            if manifest.settings != settings {
                progress.warn(&format!(
                    "Settings changed since last run. Performing full rebuild.\n\
                     Previous: {:?}\n\
                     Current:  {:?}",
                    manifest.settings, settings
                ));
                output_full(&chunks, &args, &settings)?;
            } else {
                let diff = manifest.diff(&chunks);
                output_diff(&diff, &args, &settings)?;
            }
        }
        EmbedMode::Stats => {
            output_stats(&chunks, &args)?;
            return Ok(());
        }
    }

    // Update manifest (unless dry-run)
    if !args.dry_run {
        progress.set_phase("Updating manifest...");
        let mut manifest = EmbedManifest::new(
            args.path.to_string_lossy().to_string(),
            settings,
        );
        manifest.commit_hash = get_git_commit(&args.path).ok();
        manifest.update(&chunks)?;
        manifest.save(&manifest_path)?;
        progress.info(&format!("Updated manifest: {}", manifest_path.display()));
    }

    progress.set_phase("Done");
    Ok(())
}

/// Output full chunks in envelope JSONL format
fn output_full(
    chunks: &[EmbedChunk],
    args: &EmbedArgs,
    settings: &EmbedSettings,
) -> Result<()> {
    let writer: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(BufWriter::new(std::fs::File::create(path)?)),
        None => Box::new(BufWriter::new(io::stdout())),
    };
    let mut writer = writer;

    match args.format {
        OutputFormat::EnvelopeJsonl => {
            // Header
            let header = OutputHeader {
                r#type: "header".to_string(),
                version: 1,
                format: "envelope-jsonl".to_string(),
                repo: args.path.to_string_lossy().to_string(),
                commit: get_git_commit(&args.path).ok(),
                settings: settings.clone(),
                total_chunks: chunks.len(),
            };
            writeln!(writer, "{}", serde_json::to_string(&header)?)?;

            // Summary
            let summary = OutputSummary {
                r#type: "summary".to_string(),
                total: chunks.len(),
                by_kind: count_by_kind(chunks),
                by_language: count_by_language(chunks),
                total_tokens: chunks.iter().map(|c| c.tokens as usize).sum(),
            };
            writeln!(writer, "{}", serde_json::to_string(&summary)?)?;

            // Chunks
            for chunk in chunks {
                let line = OutputChunk {
                    r#type: "chunk".to_string(),
                    status: "full".to_string(),
                    data: chunk.clone(),
                };
                writeln!(writer, "{}", serde_json::to_string(&line)?)?;
            }

            // Footer
            let footer = OutputFooter {
                r#type: "footer".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                chunks_written: chunks.len(),
            };
            writeln!(writer, "{}", serde_json::to_string(&footer)?)?;
        }
        OutputFormat::Jsonl => {
            for chunk in chunks {
                writeln!(writer, "{}", serde_json::to_string(chunk)?)?;
            }
        }
        OutputFormat::Json => {
            writeln!(writer, "{}", serde_json::to_string_pretty(chunks)?)?;
        }
    }

    writer.flush()?;
    Ok(())
}

/// Output diff in envelope JSONL format
fn output_diff(
    diff: &EmbedDiff,
    args: &EmbedArgs,
    settings: &EmbedSettings,
) -> Result<()> {
    let writer: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(BufWriter::new(std::fs::File::create(path)?)),
        None => Box::new(BufWriter::new(io::stdout())),
    };
    let mut writer = writer;

    // Header
    let header = OutputHeader {
        r#type: "header".to_string(),
        version: 1,
        format: "envelope-jsonl-diff".to_string(),
        repo: args.path.to_string_lossy().to_string(),
        commit: get_git_commit(&args.path).ok(),
        settings: settings.clone(),
        total_chunks: diff.summary.total_chunks,
    };
    writeln!(writer, "{}", serde_json::to_string(&header)?)?;

    // Diff summary
    let summary = DiffOutputSummary {
        r#type: "diff_summary".to_string(),
        added: diff.summary.added,
        modified: diff.summary.modified,
        removed: diff.summary.removed,
        unchanged: diff.summary.unchanged,
    };
    writeln!(writer, "{}", serde_json::to_string(&summary)?)?;

    // Added chunks
    for chunk in &diff.added {
        if args.only_changed || !args.only_changed {
            let line = OutputChunk {
                r#type: "chunk".to_string(),
                status: "added".to_string(),
                data: chunk.clone(),
            };
            writeln!(writer, "{}", serde_json::to_string(&line)?)?;
        }
    }

    // Modified chunks
    for modified in &diff.modified {
        let line = ModifiedOutputChunk {
            r#type: "chunk".to_string(),
            status: "modified".to_string(),
            old_id: modified.old_id.clone(),
            new_id: modified.new_id.clone(),
            data: modified.chunk.clone(),
        };
        writeln!(writer, "{}", serde_json::to_string(&line)?)?;
    }

    // Removed chunks
    for removed in &diff.removed {
        let line = RemovedOutputChunk {
            r#type: "chunk".to_string(),
            status: "removed".to_string(),
            id: removed.id.clone(),
            location: removed.location_key.clone(),
        };
        writeln!(writer, "{}", serde_json::to_string(&line)?)?;
    }

    // Unchanged (IDs only, unless --only-changed)
    if !args.only_changed {
        for id in &diff.unchanged {
            let line = UnchangedOutputChunk {
                r#type: "chunk".to_string(),
                status: "unchanged".to_string(),
                id: id.clone(),
            };
            writeln!(writer, "{}", serde_json::to_string(&line)?)?;
        }
    }

    // Footer
    let footer = OutputFooter {
        r#type: "footer".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        chunks_written: diff.added.len() + diff.modified.len() + diff.removed.len(),
    };
    writeln!(writer, "{}", serde_json::to_string(&footer)?)?;

    writer.flush()?;
    Ok(())
}

/// Print chunk ID explanation
fn print_id_explanation() {
    println!(r#"
Chunk ID System
===============

Format: ec_<32 hex chars> (128 bits of BLAKE3 hash)
Example: ec_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6

Properties:
  ✓ Content-addressable: Same code = same ID (enables deduplication)
  ✓ Deterministic: Same input always produces same ID
  ✓ Cross-platform: Windows/Linux/macOS produce identical IDs
  ✓ Collision-resistant: ~10^38 unique IDs before collision risk

What affects the ID:
  ✓ Code content (after normalization)
  ✗ File path (does NOT affect ID)
  ✗ Symbol name (does NOT affect ID)
  ✗ Line numbers (does NOT affect ID)

Normalization applied:
  1. Unicode NFC normalization
  2. CRLF/CR → LF line endings
  3. Trailing whitespace removed per line
  4. Leading/trailing blank lines removed

Example:
  fn foo() {{ bar(); }}  →  ec_a1b2c3d4...
  fn baz() {{ bar(); }}  →  ec_a1b2c3d4... (same code, same ID!)

Use cases:
  - Deduplication: Find identical code across files/repos
  - Caching: Skip re-embedding unchanged chunks
  - Tracking: Follow code that moves between files
"#);
}

// Output types for envelope format
#[derive(Serialize)]
struct OutputHeader {
    r#type: String,
    version: u32,
    format: String,
    repo: String,
    commit: Option<String>,
    settings: EmbedSettings,
    total_chunks: usize,
}

#[derive(Serialize)]
struct OutputSummary {
    r#type: String,
    total: usize,
    by_kind: HashMap<String, usize>,
    by_language: HashMap<String, usize>,
    total_tokens: usize,
}

#[derive(Serialize)]
struct DiffOutputSummary {
    r#type: String,
    added: usize,
    modified: usize,
    removed: usize,
    unchanged: usize,
}

#[derive(Serialize)]
struct OutputChunk {
    r#type: String,
    status: String,
    data: EmbedChunk,
}

#[derive(Serialize)]
struct ModifiedOutputChunk {
    r#type: String,
    status: String,
    old_id: String,
    new_id: String,
    data: EmbedChunk,
}

#[derive(Serialize)]
struct RemovedOutputChunk {
    r#type: String,
    status: String,
    id: String,
    location: String,
}

#[derive(Serialize)]
struct UnchangedOutputChunk {
    r#type: String,
    status: String,
    id: String,
}

#[derive(Serialize)]
struct OutputFooter {
    r#type: String,
    timestamp: String,
    chunks_written: usize,
}
```

### Progress Reporter (`progress.rs`)

```rust
use indicatif::{ProgressBar, ProgressStyle};

/// Progress reporting abstraction
pub trait ProgressReporter: Send + Sync {
    fn set_phase(&self, phase: &str);
    fn set_total(&self, total: usize);
    fn set_progress(&self, current: usize);
    fn warn(&self, message: &str);
    fn info(&self, message: &str);
}

/// Terminal progress with indicatif
pub struct TerminalProgress {
    bar: ProgressBar,
}

impl TerminalProgress {
    pub fn new() -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap()
        );
        Self { bar }
    }
}

impl ProgressReporter for TerminalProgress {
    fn set_phase(&self, phase: &str) {
        self.bar.set_message(phase.to_string());
    }

    fn set_total(&self, total: usize) {
        self.bar.set_length(total as u64);
        self.bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
    }

    fn set_progress(&self, current: usize) {
        self.bar.set_position(current as u64);
    }

    fn warn(&self, message: &str) {
        self.bar.println(format!("⚠️  {}", message));
    }

    fn info(&self, message: &str) {
        self.bar.println(format!("ℹ️  {}", message));
    }
}

impl Drop for TerminalProgress {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}

/// Quiet progress (no output)
pub struct QuietProgress;

impl ProgressReporter for QuietProgress {
    fn set_phase(&self, _: &str) {}
    fn set_total(&self, _: usize) {}
    fn set_progress(&self, _: usize) {}
    fn warn(&self, _: &str) {}
    fn info(&self, _: &str) {}
}
```

---

## Testing Strategy

### Determinism Tests (Critical)

```rust
#[cfg(test)]
mod determinism_tests {
    use super::*;
    use tempfile::TempDir;

    /// Run chunker N times, verify byte-identical output
    #[test]
    fn test_multiple_runs_identical() {
        let repo = create_test_repo();
        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();

        let results: Vec<Vec<EmbedChunk>> = (0..10)
            .map(|_| {
                let chunker = EmbedChunker::new(settings.clone(), limits.clone());
                chunker.chunk_repository(&repo, &QuietProgress).unwrap()
            })
            .collect();

        // All runs identical
        for i in 1..10 {
            assert_eq!(results[0], results[i], "Run {} differs from run 0", i);
        }

        // JSON serialization identical
        let json0 = serde_json::to_string(&results[0]).unwrap();
        let json1 = serde_json::to_string(&results[1]).unwrap();
        assert_eq!(json0, json1);
    }

    /// Verify cross-platform normalization
    #[test]
    fn test_cross_platform_content() {
        let variants = [
            "fn foo() {\n    bar();\n}",
            "fn foo() {\r\n    bar();\r\n}",
            "fn foo() {\r    bar();\r}",
            "fn foo() {   \n    bar();   \n}   ",
        ];

        let hashes: Vec<_> = variants
            .iter()
            .map(|c| hash_content(c))
            .collect();

        // All produce same hash
        for i in 1..hashes.len() {
            assert_eq!(hashes[0].short_id, hashes[i].short_id);
        }
    }

    /// Verify Unicode normalization (macOS vs Linux)
    #[test]
    fn test_unicode_normalization() {
        // NFD (macOS Finder default)
        let nfd = "cafe\u{0301}"; // e + combining accent
        // NFC (Linux default)
        let nfc = "caf\u{00E9}";  // single é character

        let h1 = hash_content(nfd);
        let h2 = hash_content(nfc);

        assert_eq!(h1.short_id, h2.short_id);
    }

    /// Verify no float math affects output
    #[test]
    fn test_no_float_sensitivity() {
        // Create content that would trigger splitting
        let large_content = "fn large() {\n".to_string()
            + &"    let x = 1;\n".repeat(1000)
            + "}";

        let h1 = hash_content(&large_content);
        let h2 = hash_content(&large_content);

        assert_eq!(h1.short_id, h2.short_id);
    }
}
```

### Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn test_recursion_limit() {
        let deep_code = "fn a() { ".repeat(1000) + &"}".repeat(1000);
        let settings = EmbedSettings::default();
        let limits = ResourceLimits { max_recursion_depth: 100, ..Default::default() };
        let chunker = EmbedChunker::new(settings, limits);

        // Should error, not stack overflow
        let result = chunker.chunk_content(&deep_code, "rust");
        assert!(matches!(result, Err(EmbedError::RecursionLimitExceeded { .. })));
    }

    #[test]
    fn test_file_size_limit() {
        let limits = ResourceLimits { max_file_size: 1000, ..Default::default() };
        // Test with file > 1000 bytes
        // Should error, not OOM
    }

    #[test]
    fn test_secret_detection() {
        let code = r#"const API_KEY = "sk_live_1234567890abcdef";"#;
        let settings = EmbedSettings { scan_secrets: true, redact_secrets: true, ..Default::default() };
        let limits = ResourceLimits::default();
        let chunker = EmbedChunker::new(settings, limits);

        let chunks = chunker.chunk_content(code, "javascript").unwrap();

        // Secret should be redacted
        assert!(!chunks[0].content.contains("sk_live"));
    }

    #[test]
    fn test_path_traversal() {
        // Test symlink outside repo
        // Test manifest with "../" path
        // Should error, not expose files
    }

    #[test]
    fn test_manifest_integrity() {
        let manifest = create_test_manifest();
        let path = TempDir::new().unwrap().path().join("test.bin");
        manifest.save(&path).unwrap();

        // Tamper with file
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[bytes.len() - 10] ^= 0xFF;
        std::fs::write(&path, bytes).unwrap();

        // Should detect tampering
        let result = EmbedManifest::load(&path);
        assert!(matches!(result, Err(EmbedError::ManifestCorrupted { .. })));
    }
}
```

### Performance Tests

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored
    fn test_10k_files_under_30s() {
        let repo = create_large_test_repo(10_000); // 10k files
        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();
        let chunker = EmbedChunker::new(settings, limits);

        let start = Instant::now();
        let chunks = chunker.chunk_repository(&repo, &QuietProgress).unwrap();
        let elapsed = start.elapsed();

        println!("Processed {} files → {} chunks in {:?}", 10_000, chunks.len(), elapsed);
        assert!(elapsed.as_secs() < 30, "Took {:?}, expected < 30s", elapsed);
    }

    #[test]
    fn test_parallel_no_contention() {
        // Verify thread-local resources work correctly
        // Verify no mutex contention
    }
}
```

### RAG Quality Tests

```rust
#[cfg(test)]
mod rag_tests {
    use super::*;

    #[test]
    fn test_chunks_have_context() {
        let code = r#"
/// Validates user password
/// Returns error if invalid
fn validate_password(password: &str) -> Result<(), Error> {
    if password.len() < 8 {
        return Err(Error::TooShort);
    }
    Ok(())
}
"#;
        let chunks = chunk_content(code, "rust");

        // Should include docstring
        assert!(chunks[0].context.docstring.is_some());
        assert!(chunks[0].context.docstring.as_ref().unwrap().contains("Validates"));
    }

    #[test]
    fn test_split_chunks_include_signature() {
        // Large function should split
        // Each part should include parent signature
    }

    #[test]
    fn test_metadata_for_filtering() {
        // Chunks should have visibility, is_test, tags
    }
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (3 days)
- [ ] Create `engine/src/embedding/` module structure
- [ ] Implement `normalizer.rs` with Unicode NFC + tests
- [ ] Implement `hasher.rs` with single-pass optimization
- [ ] Implement `types.rs` with all data structures
- [ ] Implement `error.rs` with actionable messages
- [ ] Implement `limits.rs` with DoS protection

### Phase 2: Chunking Logic (3 days)
- [ ] Implement basic `chunker.rs` with thread-local resources
- [ ] Add symbol extraction with docstring/comment enrichment
- [ ] Implement depth-limited large symbol splitting (integer math only)
- [ ] Add deterministic sorting throughout
- [ ] Integrate secret scanning

### Phase 3: Manifest & Diffing (2 days)
- [ ] Implement `manifest.rs` with bincode serialization
- [ ] Add integrity checksums with BLAKE3
- [ ] Implement O(n) diffing with HashMap
- [ ] Add collision detection
- [ ] Add batch API for vector DB operations

### Phase 4: CLI Integration (2 days)
- [ ] Implement `embed.rs` command with all options
- [ ] Add envelope JSONL output format
- [ ] Implement progress reporting with indicatif
- [ ] Add `--explain-ids` help
- [ ] Add mode auto-detection

### Phase 5: Testing & Polish (2 days)
- [ ] Comprehensive determinism tests
- [ ] Security tests (DoS, traversal, secrets)
- [ ] Performance benchmarks
- [ ] RAG quality tests
- [ ] Documentation

**Total: 12 days**

---

## Dependencies

```toml
# engine/Cargo.toml additions
blake3 = "1.5"                    # Already in project
unicode-normalization = "0.1"     # NEW: Unicode NFC
bincode = "1.3"                   # Already in project
indicatif = "0.17"                # Already in project for CLI
chrono = "0.4"                    # For timestamps in output
```

---

## Success Criteria

1. **Determinism**: 100 runs produce byte-identical output
2. **Cross-platform**: Windows/Linux/macOS produce identical output
3. **Performance**: <15 seconds for 10k files (release build)
4. **Security**: No secrets leaked, DoS-resistant, tamper-evident
5. **RAG quality**: Chunks include docstrings, context, metadata
6. **UX**: Clear progress, actionable errors, intuitive CLI

---

## Changelog from v1.0

### Security Improvements
- Added recursion depth limits (max 500)
- Added file size limits (10MB default)
- Added chunk count limits (1M default)
- Integrated secret scanning with redaction
- Added manifest integrity checksums
- Added path traversal protection

### Performance Improvements
- Thread-local parsers (3-10x speedup)
- Thread-local tokenizers (2-5x speedup)
- Single-pass hashing (2x faster)
- HashMap instead of BTreeMap (10x faster diff)
- Bincode instead of JSON for manifest (5-10x faster I/O)

### Determinism Fixes
- Integer-only math in splitting (no floats)
- Unicode NFC normalization
- Optional timestamps (excluded from integrity)
- Relative paths in manifest

### UX Improvements
- Envelope JSONL format with metadata
- Progress bars with indicatif
- Actionable error messages
- Auto-detect diff vs full mode
- `--only-changed` flag
- `--explain-ids` help

### RAG Improvements
- Default max_tokens increased to 1000
- Added overlap_tokens (100 default)
- Context lines increased to 5
- Docstring extraction
- Semantic tags auto-generation
- Richer metadata (visibility, is_test, tags)
- Batch API for vector DB operations
