//! Manifest storage and diffing for incremental updates
//!
//! The manifest tracks all chunks generated for a repository, enabling:
//! - Incremental updates (only re-embed changed chunks)
//! - Change detection (added, modified, removed)
//! - Integrity verification (detect tampering)
//!
//! # Storage Format
//!
//! Manifests are stored in bincode format (5-10x faster than JSON) with:
//! - BLAKE3 integrity checksum
//! - Version compatibility checking
//! - Settings validation

use std::collections::BTreeMap;
use std::path::Path;

use bincode::Options;
use serde::{Deserialize, Serialize};

use super::error::EmbedError;
use super::hasher::IncrementalHasher;
use super::types::{ChunkKind, EmbedChunk, EmbedSettings};
use crate::bincode_safe::deserialize_with_limit;

/// Current manifest format version
pub const MANIFEST_VERSION: u32 = 2;

/// Manifest tracking all chunks for incremental updates
///
/// # Determinism Note
///
/// The manifest binary file is **not byte-deterministic** across saves due to the
/// `updated_at` timestamp. However, the **checksum is deterministic** because it
/// excludes the timestamp from its calculation.
///
/// For comparing manifests:
/// - **Wrong**: Compare raw binary files (will differ due to timestamp)
/// - **Right**: Compare checksums via `manifest.checksum` (deterministic)
///
/// This design allows incremental updates while still detecting actual content changes.
///
/// # CI/CD Integration
///
/// If you need byte-deterministic manifests (e.g., for Docker layer caching):
/// - Compare checksums instead of file hashes
/// - Or set `updated_at = None` before saving in test environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedManifest {
    /// Manifest format version
    pub version: u32,

    /// Relative repository path (from git root or CWD)
    pub repo_path: String,

    /// Git commit hash when manifest was created (for reference only)
    /// Note: We always serialize Option fields for bincode compatibility
    #[serde(default)]
    pub commit_hash: Option<String>,

    /// Timestamp of last update (Unix seconds)
    ///
    /// **Important**: This field is excluded from the integrity checksum calculation
    /// to allow the checksum to remain stable across re-saves of unchanged content.
    /// The binary file will differ byte-for-byte on each save, but the checksum will
    /// only change if actual chunk content changes.
    #[serde(default)]
    pub updated_at: Option<u64>,

    /// Settings used to generate chunks (part of integrity)
    pub settings: EmbedSettings,

    /// All chunks indexed by location key
    /// Using BTreeMap for deterministic iteration order (critical for cross-platform consistency)
    pub chunks: BTreeMap<String, ManifestEntry>,

    /// Integrity checksum (BLAKE3 of settings + sorted chunk entries)
    /// Excluded from serialization, computed on save, verified on load
    #[serde(default)]
    pub checksum: Option<String>,
}

/// Entry in the manifest for a single chunk
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Content-addressable chunk ID (128-bit)
    pub chunk_id: String,

    /// Full content hash for collision detection (256-bit)
    pub full_hash: String,

    /// Token count
    pub tokens: u32,

    /// Line range (1-indexed, inclusive)
    pub lines: (u32, u32),
}

impl EmbedManifest {
    /// Create a new empty manifest
    pub fn new(repo_path: String, settings: EmbedSettings) -> Self {
        Self {
            version: MANIFEST_VERSION,
            repo_path,
            commit_hash: None,
            updated_at: None,
            settings,
            chunks: BTreeMap::new(),
            checksum: None,
        }
    }

    /// Generate deterministic location key for a chunk
    ///
    /// Format: `file::symbol::kind`
    /// Uses `::` as separator (unlikely in paths/symbols)
    pub fn location_key(file: &str, symbol: &str, kind: ChunkKind) -> String {
        format!("{}::{}::{}", file, symbol, kind.name())
    }

    /// Compute integrity checksum over settings and chunk entries
    fn compute_checksum(&self) -> String {
        let mut hasher = IncrementalHasher::new();

        // Hash manifest version
        hasher.update_u32(self.version);

        // Hash settings (affects chunk generation)
        let settings_json = serde_json::to_string(&self.settings).unwrap_or_default();
        hasher.update_str(&settings_json);

        // Hash chunks in deterministic order (sorted by key)
        let mut keys: Vec<_> = self.chunks.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(entry) = self.chunks.get(key) {
                hasher.update_str(key);
                hasher.update_str(&entry.chunk_id);
                hasher.update_str(&entry.full_hash);
                hasher.update_u32(entry.tokens);
                hasher.update_u32(entry.lines.0);
                hasher.update_u32(entry.lines.1);
            }
        }

        hasher.finalize_hex()
    }

    /// Save manifest to file with integrity checksum
    ///
    /// # Behavior
    ///
    /// This method:
    /// 1. Updates `updated_at` to the current timestamp
    /// 2. Computes a new checksum (excluding timestamp)
    /// 3. Serializes to bincode format
    ///
    /// # Determinism
    ///
    /// The resulting binary file is **not byte-deterministic** because the timestamp
    /// changes on every save. However, the checksum **is deterministic** - it only
    /// changes when actual chunk content or settings change.
    ///
    /// For deterministic testing, set `self.updated_at = None` before saving.
    ///
    /// # Note
    ///
    /// This method mutates `self` to set checksum and timestamp.
    /// This avoids cloning the entire manifest (which can be large).
    pub fn save(&mut self, path: &Path) -> Result<(), EmbedError> {
        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;
        }

        // Update timestamp
        self.updated_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );

        // Compute checksum (excludes timestamp for deterministic checksums across saves)
        self.checksum = Some(self.compute_checksum());

        // Use bincode for faster I/O (5-10x faster than JSON for large manifests)
        // Note: Must use bincode::options() to match deserialize_with_limit() in load()
        let bytes = bincode::options()
            .serialize(self)
            .map_err(|e| EmbedError::SerializationError { reason: e.to_string() })?;

        std::fs::write(path, bytes)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        Ok(())
    }

    /// Load manifest from file with integrity verification
    pub fn load(path: &Path) -> Result<Self, EmbedError> {
        let bytes = std::fs::read(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        let mut manifest: Self = deserialize_with_limit(&bytes)
            .map_err(|e| EmbedError::DeserializationError { reason: e.to_string() })?;

        // Version check
        if manifest.version > MANIFEST_VERSION {
            return Err(EmbedError::ManifestVersionTooNew {
                found: manifest.version,
                max_supported: MANIFEST_VERSION,
            });
        }

        // Integrity verification using constant-time comparison to prevent timing attacks
        if let Some(stored_checksum) = manifest.checksum.take() {
            let computed = manifest.compute_checksum();
            if !constant_time_eq(stored_checksum.as_bytes(), computed.as_bytes()) {
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

    /// Load manifest if it exists, otherwise return None
    pub fn load_if_exists(path: &Path) -> Result<Option<Self>, EmbedError> {
        if path.exists() {
            Ok(Some(Self::load(path)?))
        } else {
            Ok(None)
        }
    }

    /// Update manifest with current chunks, detecting collisions
    pub fn update(&mut self, chunks: &[EmbedChunk]) -> Result<(), EmbedError> {
        // Collision detection: track id -> full_hash mappings
        // Using BTreeMap for deterministic iteration (critical for cross-platform consistency)
        let mut id_to_hash: BTreeMap<&str, &str> = BTreeMap::new();

        self.chunks.clear();

        for chunk in chunks {
            // Check for hash collision
            if let Some(&existing_hash) = id_to_hash.get(chunk.id.as_str()) {
                if existing_hash != chunk.full_hash.as_str() {
                    return Err(EmbedError::HashCollision {
                        id: chunk.id.clone(),
                        hash1: existing_hash.to_owned(),
                        hash2: chunk.full_hash.clone(),
                    });
                }
            }
            id_to_hash.insert(&chunk.id, &chunk.full_hash);

            let key = Self::location_key(&chunk.source.file, &chunk.source.symbol, chunk.kind);

            self.chunks.insert(
                key,
                ManifestEntry {
                    chunk_id: chunk.id.clone(),
                    full_hash: chunk.full_hash.clone(),
                    tokens: chunk.tokens,
                    lines: chunk.source.lines,
                },
            );
        }

        Ok(())
    }

    /// Compute diff between current chunks and manifest
    pub fn diff(&self, current_chunks: &[EmbedChunk]) -> EmbedDiff {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut removed = Vec::new();
        let mut unchanged = Vec::new();

        // Build map of current chunks by location key
        // Using BTreeMap for deterministic iteration in "added" detection
        let current_map: BTreeMap<String, &EmbedChunk> = current_chunks
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
                removed
                    .push(RemovedChunk { id: entry.chunk_id.clone(), location_key: key.clone() });
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

    /// Check if settings match the manifest settings
    pub fn settings_match(&self, settings: &EmbedSettings) -> bool {
        &self.settings == settings
    }

    /// Get the number of chunks in the manifest
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

/// Result of diffing current state against manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedDiff {
    /// Summary statistics
    pub summary: DiffSummary,

    /// New chunks (not in manifest)
    pub added: Vec<EmbedChunk>,

    /// Changed chunks (different content)
    pub modified: Vec<ModifiedChunk>,

    /// Deleted chunks (in manifest but not current)
    pub removed: Vec<RemovedChunk>,

    /// Unchanged chunk IDs (same content)
    pub unchanged: Vec<String>,
}

impl EmbedDiff {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        self.summary.added > 0 || self.summary.modified > 0 || self.summary.removed > 0
    }

    /// Get all chunks that need to be upserted (added + modified)
    pub fn chunks_to_upsert(&self) -> Vec<&EmbedChunk> {
        let mut chunks: Vec<&EmbedChunk> = self.added.iter().collect();
        chunks.extend(self.modified.iter().map(|m| &m.chunk));
        chunks
    }

    /// Get all IDs that need to be deleted
    pub fn ids_to_delete(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.removed.iter().map(|r| r.id.as_str()).collect();
        // Also delete old IDs for modified chunks
        ids.extend(self.modified.iter().map(|m| m.old_id.as_str()));
        ids
    }

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

/// Summary of changes between manifest and current state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Number of new chunks
    pub added: usize,

    /// Number of modified chunks
    pub modified: usize,

    /// Number of removed chunks
    pub removed: usize,

    /// Number of unchanged chunks
    pub unchanged: usize,

    /// Total chunks in current state
    pub total_chunks: usize,
}

/// A chunk that was modified (content changed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedChunk {
    /// Previous chunk ID
    pub old_id: String,

    /// New chunk ID
    pub new_id: String,

    /// The updated chunk
    pub chunk: EmbedChunk,
}

/// A chunk that was removed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedChunk {
    /// Chunk ID that was removed
    pub id: String,

    /// Location key for reference
    pub location_key: String,
}

/// Batch of operations for vector DB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffBatch {
    /// Batch number (0-indexed)
    pub batch_number: usize,

    /// Operation type
    pub operation: BatchOperation,

    /// Chunks to upsert (for Upsert operation)
    pub chunks: Vec<EmbedChunk>,

    /// IDs to delete (for Delete operation, or old IDs for Upsert)
    pub ids: Vec<String>,
}

/// Type of batch operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchOperation {
    /// Insert or update chunks
    Upsert,
    /// Delete chunks by ID
    Delete,
}

/// Constant-time byte comparison to prevent timing attacks
///
/// Returns true if both slices are equal, using constant-time comparison
/// that doesn't short-circuit on first difference.
#[inline]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // XOR all bytes and accumulate - runs in constant time regardless of content
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::types::{ChunkContext, ChunkSource, RepoIdentifier, Visibility};
    use tempfile::TempDir;

    fn create_test_chunk(id: &str, file: &str, symbol: &str) -> EmbedChunk {
        EmbedChunk {
            id: id.to_owned(),
            full_hash: format!("{}_full", id),
            content: "fn test() {}".to_owned(),
            tokens: 10,
            kind: ChunkKind::Function,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: file.to_owned(),
                lines: (1, 5),
                symbol: symbol.to_owned(),
                fqn: None,
                language: "rust".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
            },
            context: ChunkContext::default(),
            part: None,
        }
    }

    #[test]
    fn test_new_manifest() {
        let manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        assert_eq!(manifest.version, MANIFEST_VERSION);
        assert_eq!(manifest.repo_path, "my-repo");
        assert!(manifest.chunks.is_empty());
    }

    #[test]
    fn test_location_key() {
        let key = EmbedManifest::location_key("src/auth.rs", "validate", ChunkKind::Function);
        assert_eq!(key, "src/auth.rs::validate::function");
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("test.bin");

        // Create and save manifest
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let chunks = vec![
            create_test_chunk("ec_123", "src/foo.rs", "foo"),
            create_test_chunk("ec_456", "src/bar.rs", "bar"),
        ];
        manifest.update(&chunks).unwrap();
        manifest.save(&manifest_path).unwrap();

        // Load and verify
        let loaded = EmbedManifest::load(&manifest_path).unwrap();
        assert_eq!(loaded.repo_path, "my-repo");
        assert_eq!(loaded.chunks.len(), 2);
    }

    #[test]
    fn test_integrity_verification() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("test.bin");

        // Create and save manifest
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());
        manifest.save(&manifest_path).unwrap();

        // Tamper with file
        let mut bytes = std::fs::read(&manifest_path).unwrap();
        if bytes.len() >= 10 {
            let idx = bytes.len() - 10;
            bytes[idx] ^= 0xFF;
            std::fs::write(&manifest_path, bytes).unwrap();
        }

        // Should detect tampering
        let result = EmbedManifest::load(&manifest_path);
        assert!(matches!(
            result,
            Err(EmbedError::ManifestCorrupted { .. })
                | Err(EmbedError::DeserializationError { .. })
        ));
    }

    #[test]
    fn test_diff_added() {
        let manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let chunks = vec![create_test_chunk("ec_123", "src/foo.rs", "foo")];

        let diff = manifest.diff(&chunks);
        assert_eq!(diff.summary.added, 1);
        assert_eq!(diff.summary.modified, 0);
        assert_eq!(diff.summary.removed, 0);
    }

    #[test]
    fn test_diff_modified() {
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let old_chunks = vec![create_test_chunk("ec_old", "src/foo.rs", "foo")];
        manifest.update(&old_chunks).unwrap();

        // Same location, different ID = modified
        let new_chunks = vec![create_test_chunk("ec_new", "src/foo.rs", "foo")];

        let diff = manifest.diff(&new_chunks);
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.modified, 1);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.modified[0].old_id, "ec_old");
        assert_eq!(diff.modified[0].new_id, "ec_new");
    }

    #[test]
    fn test_diff_removed() {
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let old_chunks = vec![create_test_chunk("ec_123", "src/foo.rs", "foo")];
        manifest.update(&old_chunks).unwrap();

        // Empty current = all removed
        let diff = manifest.diff(&[]);
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.modified, 0);
        assert_eq!(diff.summary.removed, 1);
    }

    #[test]
    fn test_diff_unchanged() {
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let chunks = vec![create_test_chunk("ec_123", "src/foo.rs", "foo")];
        manifest.update(&chunks).unwrap();

        // Same chunks = unchanged
        let diff = manifest.diff(&chunks);
        assert_eq!(diff.summary.unchanged, 1);
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_batches() {
        let manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        let chunks: Vec<_> = (0..5)
            .map(|i| {
                create_test_chunk(&format!("ec_{i}"), &format!("src/f{i}.rs"), &format!("f{i}"))
            })
            .collect();

        let diff = manifest.diff(&chunks);
        let batches = diff.batches(2);

        // 5 chunks / batch size 2 = 3 batches
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].chunks.len(), 2);
        assert_eq!(batches[1].chunks.len(), 2);
        assert_eq!(batches[2].chunks.len(), 1);
    }

    #[test]
    fn test_load_if_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("nonexistent.bin");

        // Non-existent returns None
        let result = EmbedManifest::load_if_exists(&manifest_path).unwrap();
        assert!(result.is_none());

        // Existing returns Some
        let mut manifest = EmbedManifest::new("test".to_owned(), EmbedSettings::default());
        manifest.save(&manifest_path).unwrap();

        let result = EmbedManifest::load_if_exists(&manifest_path).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_collision_detection() {
        let mut manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        // Create two chunks with same ID but different hashes
        let mut chunk1 = create_test_chunk("ec_same", "src/foo.rs", "foo");
        let mut chunk2 = create_test_chunk("ec_same", "src/bar.rs", "bar");
        chunk1.full_hash = "hash1".to_owned();
        chunk2.full_hash = "hash2".to_owned();

        let result = manifest.update(&[chunk1, chunk2]);
        assert!(matches!(result, Err(EmbedError::HashCollision { .. })));
    }

    #[test]
    fn test_settings_match() {
        let manifest = EmbedManifest::new("my-repo".to_owned(), EmbedSettings::default());

        assert!(manifest.settings_match(&EmbedSettings::default()));

        let mut different = EmbedSettings::default();
        different.max_tokens = 2000;
        assert!(!manifest.settings_match(&different));
    }
}
