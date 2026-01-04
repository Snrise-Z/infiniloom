//! Checkpoint and resume support for large repository embedding
//!
//! This module provides checkpoint/resume functionality for embedding operations
//! on large repositories. It allows long-running operations to be interrupted
//! and resumed without losing progress.
//!
//! # Features
//!
//! - **Atomic checkpoints**: Progress saved atomically to prevent corruption
//! - **Integrity verification**: BLAKE3 hash ensures checkpoint validity
//! - **Incremental progress**: Resume from exactly where processing stopped
//! - **Space efficient**: Only stores metadata, not full chunk content
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::{CheckpointManager, EmbedChunker, EmbedSettings};
//!
//! // Create checkpoint manager
//! let checkpoint_path = Path::new(".infiniloom-checkpoint.bin");
//! let mut manager = CheckpointManager::new(checkpoint_path);
//!
//! // Try to resume from existing checkpoint
//! if let Some(checkpoint) = manager.load()? {
//!     println!("Resuming from checkpoint: {} files processed", checkpoint.files_processed);
//!     // Continue from checkpoint
//! } else {
//!     // Start fresh
//! }
//!
//! // Save checkpoint periodically during processing
//! manager.save(&checkpoint)?;
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bincode::Options;
use blake3::Hasher;
use serde::{Deserialize, Serialize};

use super::error::EmbedError;
use super::types::{EmbedChunk, EmbedSettings, RepoIdentifier};
use crate::bincode_safe::deserialize_with_limit;

/// Current checkpoint format version
pub const CHECKPOINT_VERSION: u32 = 1;

/// Bincode-compatible repository identifier for checkpoint serialization
///
/// This is a separate struct from `RepoIdentifier` because the main type uses
/// `#[serde(skip_serializing_if)]` which is incompatible with bincode's
/// fixed-format serialization.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CheckpointRepoId {
    /// Namespace/organization
    pub namespace: String,
    /// Repository name
    pub name: String,
    /// Semantic version or tag
    pub version: String,
    /// Branch name
    pub branch: String,
    /// Git commit hash
    pub commit: String,
}

impl From<&RepoIdentifier> for CheckpointRepoId {
    fn from(repo: &RepoIdentifier) -> Self {
        Self {
            namespace: repo.namespace.clone(),
            name: repo.name.clone(),
            version: repo.version.clone().unwrap_or_default(),
            branch: repo.branch.clone().unwrap_or_default(),
            commit: repo.commit.clone().unwrap_or_default(),
        }
    }
}

impl From<RepoIdentifier> for CheckpointRepoId {
    fn from(repo: RepoIdentifier) -> Self {
        Self::from(&repo)
    }
}

impl From<&CheckpointRepoId> for RepoIdentifier {
    fn from(cp: &CheckpointRepoId) -> Self {
        Self {
            namespace: cp.namespace.clone(),
            name: cp.name.clone(),
            version: if cp.version.is_empty() {
                None
            } else {
                Some(cp.version.clone())
            },
            branch: if cp.branch.is_empty() {
                None
            } else {
                Some(cp.branch.clone())
            },
            commit: if cp.commit.is_empty() {
                None
            } else {
                Some(cp.commit.clone())
            },
        }
    }
}

/// Checkpoint state for embedding operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedCheckpoint {
    /// Format version for forward compatibility
    pub version: u32,

    /// When this checkpoint was created (Unix timestamp)
    pub created_at: u64,

    /// When this checkpoint was last updated
    pub updated_at: u64,

    /// Repository identifier (bincode-compatible version)
    pub repo_id: CheckpointRepoId,

    /// Repository root path (for validation)
    pub repo_path: String,

    /// Hash of the settings used (for validation)
    pub settings_hash: String,

    /// Files that have been fully processed
    pub processed_files: BTreeSet<String>,

    /// Files remaining to process (for progress tracking)
    pub remaining_files: Vec<String>,

    /// Chunks generated so far (stored by file for efficient resume)
    pub chunks_by_file: BTreeMap<String, Vec<ChunkReference>>,

    /// Total chunks generated
    pub total_chunks: usize,

    /// Total tokens across all chunks
    pub total_tokens: u64,

    /// Files that failed processing (with error messages)
    pub failed_files: BTreeMap<String, String>,

    /// Current phase of processing
    pub phase: CheckpointPhase,

    /// Integrity hash of checkpoint content
    pub integrity_hash: String,
}

/// Lightweight reference to a chunk (without full content)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkReference {
    /// Content-addressable chunk ID
    pub id: String,

    /// Full BLAKE3 hash
    pub full_hash: String,

    /// Token count
    pub tokens: u32,

    /// Line range in source file
    pub lines: (u32, u32),

    /// Symbol name
    pub symbol: String,
}

impl From<&EmbedChunk> for ChunkReference {
    fn from(chunk: &EmbedChunk) -> Self {
        Self {
            id: chunk.id.clone(),
            full_hash: chunk.full_hash.clone(),
            tokens: chunk.tokens,
            lines: chunk.source.lines,
            symbol: chunk.source.symbol.clone(),
        }
    }
}

/// Processing phases for checkpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CheckpointPhase {
    /// Initial file discovery
    #[default]
    Discovery,
    /// Parsing and chunking files
    Chunking,
    /// Building call graph
    CallGraph,
    /// Building hierarchy
    Hierarchy,
    /// Final sorting
    Sorting,
    /// Processing complete
    Complete,
}

impl EmbedCheckpoint {
    /// Create a new checkpoint for a repository
    pub fn new(repo_path: &Path, repo_id: RepoIdentifier, settings: &EmbedSettings) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let settings_hash = compute_settings_hash(settings);

        Self {
            version: CHECKPOINT_VERSION,
            created_at: now,
            updated_at: now,
            repo_id: CheckpointRepoId::from(repo_id),
            repo_path: repo_path.to_string_lossy().to_string(),
            settings_hash,
            processed_files: BTreeSet::new(),
            remaining_files: Vec::new(),
            chunks_by_file: BTreeMap::new(),
            total_chunks: 0,
            total_tokens: 0,
            failed_files: BTreeMap::new(),
            phase: CheckpointPhase::Discovery,
            integrity_hash: String::new(),
        }
    }

    /// Set the list of files to process
    pub fn set_files(&mut self, files: Vec<String>) {
        self.remaining_files = files;
        self.phase = CheckpointPhase::Chunking;
        self.update_timestamp();
    }

    /// Record that a file has been processed
    pub fn mark_file_processed(&mut self, file: &str, chunks: &[EmbedChunk]) {
        // Move from remaining to processed
        self.remaining_files.retain(|f| f != file);
        self.processed_files.insert(file.to_owned());

        // Store chunk references
        let refs: Vec<ChunkReference> = chunks.iter().map(ChunkReference::from).collect();
        let tokens: u64 = chunks.iter().map(|c| c.tokens as u64).sum();

        self.total_chunks += chunks.len();
        self.total_tokens += tokens;
        self.chunks_by_file.insert(file.to_owned(), refs);

        self.update_timestamp();
    }

    /// Record that a file failed processing
    pub fn mark_file_failed(&mut self, file: &str, error: &str) {
        self.remaining_files.retain(|f| f != file);
        self.failed_files.insert(file.to_owned(), error.to_owned());
        self.update_timestamp();
    }

    /// Set the current processing phase
    pub fn set_phase(&mut self, phase: CheckpointPhase) {
        self.phase = phase;
        self.update_timestamp();
    }

    /// Check if all files have been processed
    pub fn is_chunking_complete(&self) -> bool {
        self.remaining_files.is_empty()
            && (self.phase == CheckpointPhase::Chunking
                || self.phase == CheckpointPhase::CallGraph
                || self.phase == CheckpointPhase::Hierarchy
                || self.phase == CheckpointPhase::Sorting
                || self.phase == CheckpointPhase::Complete)
    }

    /// Get progress as a percentage (0-100)
    pub fn progress_percent(&self) -> u32 {
        let total =
            self.processed_files.len() + self.remaining_files.len() + self.failed_files.len();
        if total == 0 {
            return 0;
        }

        let processed = self.processed_files.len() + self.failed_files.len();
        ((processed * 100) / total) as u32
    }

    /// Get number of files processed
    pub fn files_processed(&self) -> usize {
        self.processed_files.len()
    }

    /// Get number of files remaining
    pub fn files_remaining(&self) -> usize {
        self.remaining_files.len()
    }

    /// Get number of files that failed
    pub fn files_failed(&self) -> usize {
        self.failed_files.len()
    }

    /// Validate that this checkpoint matches the given settings and repo
    pub fn validate(
        &self,
        repo_path: &Path,
        settings: &EmbedSettings,
    ) -> Result<(), CheckpointError> {
        // Check version compatibility
        if self.version > CHECKPOINT_VERSION {
            return Err(CheckpointError::VersionMismatch {
                checkpoint_version: self.version,
                current_version: CHECKPOINT_VERSION,
            });
        }

        // Check repo path matches
        let current_path = repo_path.to_string_lossy().to_string();
        if self.repo_path != current_path {
            return Err(CheckpointError::RepoMismatch {
                checkpoint_repo: self.repo_path.clone(),
                current_repo: current_path,
            });
        }

        // Check settings match
        let current_hash = compute_settings_hash(settings);
        if self.settings_hash != current_hash {
            return Err(CheckpointError::SettingsMismatch {
                checkpoint_hash: self.settings_hash.clone(),
                current_hash,
            });
        }

        Ok(())
    }

    /// Update the timestamp
    fn update_timestamp(&mut self) {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Compute integrity hash for the checkpoint
    pub fn compute_integrity(&mut self) {
        let mut hasher = Hasher::new();

        // Hash key fields
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.created_at.to_le_bytes());
        hasher.update(self.repo_path.as_bytes());
        hasher.update(self.settings_hash.as_bytes());
        hasher.update(&(self.processed_files.len() as u64).to_le_bytes());
        hasher.update(&(self.total_chunks as u64).to_le_bytes());
        hasher.update(&self.total_tokens.to_le_bytes());

        // Hash processed files (sorted for determinism)
        for file in &self.processed_files {
            hasher.update(file.as_bytes());
        }

        self.integrity_hash = hasher.finalize().to_hex().to_string();
    }

    /// Verify integrity hash
    pub fn verify_integrity(&self) -> bool {
        let mut copy = self.clone();
        copy.compute_integrity();
        copy.integrity_hash == self.integrity_hash
    }
}

/// Errors that can occur during checkpoint operations
#[derive(Debug, Clone)]
pub enum CheckpointError {
    /// Checkpoint version is newer than current
    VersionMismatch { checkpoint_version: u32, current_version: u32 },
    /// Repository path doesn't match
    RepoMismatch { checkpoint_repo: String, current_repo: String },
    /// Settings hash doesn't match
    SettingsMismatch { checkpoint_hash: String, current_hash: String },
    /// Checkpoint integrity verification failed
    IntegrityFailed,
    /// Checkpoint file corrupted
    Corrupted(String),
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VersionMismatch { checkpoint_version, current_version } => {
                write!(
                    f,
                    "Checkpoint version {} is newer than current version {}",
                    checkpoint_version, current_version
                )
            },
            Self::RepoMismatch { checkpoint_repo, current_repo } => {
                write!(
                    f,
                    "Checkpoint repo '{}' doesn't match current repo '{}'",
                    checkpoint_repo, current_repo
                )
            },
            Self::SettingsMismatch { .. } => {
                write!(f, "Checkpoint settings don't match current settings")
            },
            Self::IntegrityFailed => {
                write!(f, "Checkpoint integrity verification failed")
            },
            Self::Corrupted(reason) => {
                write!(f, "Checkpoint corrupted: {}", reason)
            },
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Manager for checkpoint save/load operations
pub struct CheckpointManager {
    /// Path to the checkpoint file
    path: PathBuf,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the checkpoint path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if a checkpoint file exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Load checkpoint from disk
    pub fn load(&self) -> Result<Option<EmbedCheckpoint>, EmbedError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let bytes = std::fs::read(&self.path)
            .map_err(|e| EmbedError::IoError { path: self.path.clone(), source: e })?;

        let checkpoint: EmbedCheckpoint =
            deserialize_with_limit(&bytes).map_err(|e| EmbedError::DeserializationError {
                reason: format!("Failed to deserialize checkpoint: {}", e),
            })?;

        // Verify integrity
        if !checkpoint.verify_integrity() {
            return Err(EmbedError::ManifestCorrupted {
                path: self.path.clone(),
                expected: checkpoint.integrity_hash,
                actual: "integrity check failed".to_owned(),
            });
        }

        Ok(Some(checkpoint))
    }

    /// Save checkpoint to disk (atomic write)
    pub fn save(&self, checkpoint: &mut EmbedCheckpoint) -> Result<(), EmbedError> {
        // Compute integrity hash before saving
        checkpoint.compute_integrity();

        let bytes = bincode::options().serialize(checkpoint).map_err(|e| {
            EmbedError::SerializationError {
                reason: format!("Failed to serialize checkpoint: {}", e),
            }
        })?;

        // Write atomically via temp file
        let temp_path = self.path.with_extension("tmp");

        std::fs::write(&temp_path, &bytes)
            .map_err(|e| EmbedError::IoError { path: temp_path.clone(), source: e })?;

        std::fs::rename(&temp_path, &self.path)
            .map_err(|e| EmbedError::IoError { path: self.path.clone(), source: e })?;

        Ok(())
    }

    /// Delete the checkpoint file
    pub fn delete(&self) -> Result<(), EmbedError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|e| EmbedError::IoError { path: self.path.clone(), source: e })?;
        }
        Ok(())
    }

    /// Load and validate checkpoint against current settings
    pub fn load_validated(
        &self,
        repo_path: &Path,
        settings: &EmbedSettings,
    ) -> Result<Option<EmbedCheckpoint>, EmbedError> {
        let checkpoint = match self.load()? {
            Some(cp) => cp,
            None => return Ok(None),
        };

        // Validate checkpoint matches current settings
        match checkpoint.validate(repo_path, settings) {
            Ok(()) => Ok(Some(checkpoint)),
            Err(CheckpointError::SettingsMismatch { .. }) => {
                // Settings changed, can't resume - return None so fresh start happens
                Ok(None)
            },
            Err(CheckpointError::RepoMismatch { .. }) => {
                // Different repo, can't resume
                Ok(None)
            },
            Err(e) => Err(EmbedError::DeserializationError { reason: e.to_string() }),
        }
    }
}

/// Compute hash of settings for change detection
fn compute_settings_hash(settings: &EmbedSettings) -> String {
    let mut hasher = Hasher::new();

    // Hash settings that affect output
    hasher.update(&settings.max_tokens.to_le_bytes());
    hasher.update(&settings.min_tokens.to_le_bytes());
    hasher.update(&settings.overlap_tokens.to_le_bytes());
    hasher.update(&settings.context_lines.to_le_bytes());
    hasher.update(settings.token_model.as_bytes());
    hasher.update(&[settings.include_imports as u8]);
    hasher.update(&[settings.include_tests as u8]);
    hasher.update(&[settings.include_top_level as u8]);
    hasher.update(&[settings.scan_secrets as u8]);
    hasher.update(&[settings.redact_secrets as u8]);
    hasher.update(&[settings.fail_on_secrets as u8]);
    hasher.update(&[settings.enable_hierarchy as u8]);
    hasher.update(&settings.hierarchy_min_children.to_le_bytes());

    // Hash patterns
    for pattern in &settings.include_patterns {
        hasher.update(pattern.as_bytes());
    }
    for pattern in &settings.exclude_patterns {
        hasher.update(pattern.as_bytes());
    }

    hasher.finalize().to_hex().to_string()
}

/// Statistics from a checkpoint
#[derive(Debug, Clone)]
pub struct CheckpointStats {
    /// When checkpoint was created
    pub created_at: u64,
    /// When checkpoint was last updated
    pub updated_at: u64,
    /// Processing phase
    pub phase: CheckpointPhase,
    /// Files processed
    pub files_processed: usize,
    /// Files remaining
    pub files_remaining: usize,
    /// Files failed
    pub files_failed: usize,
    /// Total chunks
    pub total_chunks: usize,
    /// Total tokens
    pub total_tokens: u64,
    /// Progress percentage
    pub progress_percent: u32,
}

impl From<&EmbedCheckpoint> for CheckpointStats {
    fn from(cp: &EmbedCheckpoint) -> Self {
        Self {
            created_at: cp.created_at,
            updated_at: cp.updated_at,
            phase: cp.phase,
            files_processed: cp.files_processed(),
            files_remaining: cp.files_remaining(),
            files_failed: cp.files_failed(),
            total_chunks: cp.total_chunks,
            total_tokens: cp.total_tokens,
            progress_percent: cp.progress_percent(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> EmbedSettings {
        EmbedSettings::default()
    }

    #[test]
    fn test_checkpoint_creation() {
        let settings = test_settings();
        let cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        assert_eq!(cp.version, CHECKPOINT_VERSION);
        assert_eq!(cp.phase, CheckpointPhase::Discovery);
        assert!(cp.processed_files.is_empty());
        assert!(cp.remaining_files.is_empty());
        assert_eq!(cp.total_chunks, 0);
    }

    #[test]
    fn test_checkpoint_file_tracking() {
        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        // Set files to process
        cp.set_files(vec!["a.rs".to_owned(), "b.rs".to_owned(), "c.rs".to_owned()]);

        assert_eq!(cp.files_remaining(), 3);
        assert_eq!(cp.files_processed(), 0);
        assert_eq!(cp.phase, CheckpointPhase::Chunking);

        // Process one file
        cp.mark_file_processed("a.rs", &[]);
        assert_eq!(cp.files_remaining(), 2);
        assert_eq!(cp.files_processed(), 1);

        // Fail one file
        cp.mark_file_failed("b.rs", "Parse error");
        assert_eq!(cp.files_remaining(), 1);
        assert_eq!(cp.files_processed(), 1);
        assert_eq!(cp.files_failed(), 1);

        // Progress should be 66% (2 of 3 done)
        assert_eq!(cp.progress_percent(), 66);
    }

    #[test]
    fn test_checkpoint_integrity() {
        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);
        cp.set_files(vec!["test.rs".to_owned()]);
        cp.mark_file_processed("test.rs", &[]);

        // Compute and verify integrity
        cp.compute_integrity();
        assert!(!cp.integrity_hash.is_empty());
        assert!(cp.verify_integrity());

        // Tamper with data
        cp.total_chunks = 999;
        assert!(!cp.verify_integrity());
    }

    #[test]
    fn test_checkpoint_validation() {
        let settings = test_settings();
        let cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        // Should validate against same settings and path
        assert!(cp.validate(Path::new("/test/repo"), &settings).is_ok());

        // Should fail for different repo
        assert!(cp.validate(Path::new("/other/repo"), &settings).is_err());

        // Should fail for different settings
        let mut different_settings = settings;
        different_settings.max_tokens = 9999;
        assert!(cp
            .validate(Path::new("/test/repo"), &different_settings)
            .is_err());
    }

    #[test]
    fn test_checkpoint_save_load() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let checkpoint_path = temp_dir.path().join("checkpoint.bin");

        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);
        cp.set_files(vec!["test.rs".to_owned()]);
        cp.mark_file_processed("test.rs", &[]);

        // Save
        let manager = CheckpointManager::new(&checkpoint_path);
        manager.save(&mut cp).unwrap();

        // Load
        let loaded = manager.load().unwrap().unwrap();
        assert_eq!(loaded.files_processed(), 1);
        assert!(loaded.verify_integrity());
    }

    #[test]
    fn test_checkpoint_manager_validated() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let checkpoint_path = temp_dir.path().join("checkpoint.bin");
        let repo_path = Path::new("/test/repo");

        let settings = test_settings();
        let mut cp = EmbedCheckpoint::new(repo_path, RepoIdentifier::default(), &settings);
        cp.set_files(vec!["test.rs".to_owned()]);

        let manager = CheckpointManager::new(&checkpoint_path);
        manager.save(&mut cp).unwrap();

        // Should load with matching settings
        let loaded = manager.load_validated(repo_path, &settings).unwrap();
        assert!(loaded.is_some());

        // Should return None for different settings (not error)
        let mut different = settings;
        different.max_tokens = 9999;
        let loaded = manager.load_validated(repo_path, &different).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_checkpoint_phases() {
        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        assert_eq!(cp.phase, CheckpointPhase::Discovery);

        cp.set_phase(CheckpointPhase::Chunking);
        assert_eq!(cp.phase, CheckpointPhase::Chunking);

        cp.set_phase(CheckpointPhase::CallGraph);
        assert_eq!(cp.phase, CheckpointPhase::CallGraph);

        cp.set_phase(CheckpointPhase::Complete);
        assert_eq!(cp.phase, CheckpointPhase::Complete);
    }

    #[test]
    fn test_checkpoint_stats() {
        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);
        cp.set_files(vec!["a.rs".to_owned(), "b.rs".to_owned()]);
        cp.mark_file_processed("a.rs", &[]);

        let stats = CheckpointStats::from(&cp);
        assert_eq!(stats.files_processed, 1);
        assert_eq!(stats.files_remaining, 1);
        assert_eq!(stats.progress_percent, 50);
    }

    #[test]
    fn test_settings_hash_determinism() {
        let settings = test_settings();
        let hash1 = compute_settings_hash(&settings);
        let hash2 = compute_settings_hash(&settings);
        assert_eq!(hash1, hash2);

        // Different settings = different hash
        let mut different = settings;
        different.max_tokens = 2000; // Use different value from default (1000)
        let hash3 = compute_settings_hash(&different);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_checkpoint_delete() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let checkpoint_path = temp_dir.path().join("checkpoint.bin");

        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        let manager = CheckpointManager::new(&checkpoint_path);
        manager.save(&mut cp).unwrap();
        assert!(manager.exists());

        manager.delete().unwrap();
        assert!(!manager.exists());
    }

    #[test]
    fn test_is_chunking_complete() {
        let settings = test_settings();
        let mut cp =
            EmbedCheckpoint::new(Path::new("/test/repo"), RepoIdentifier::default(), &settings);

        // Not complete in discovery phase
        assert!(!cp.is_chunking_complete());

        // Not complete with remaining files
        cp.set_files(vec!["a.rs".to_owned()]);
        assert!(!cp.is_chunking_complete());

        // Complete when all files processed
        cp.mark_file_processed("a.rs", &[]);
        assert!(cp.is_chunking_complete());
    }

    #[test]
    fn test_chunk_reference_from_embed_chunk() {
        use super::super::types::{ChunkContext, ChunkKind, ChunkSource};

        let chunk = EmbedChunk {
            id: "ec_abc123".to_owned(),
            full_hash: "deadbeef".repeat(8),
            content: "fn test() {}".to_owned(),
            tokens: 10,
            kind: ChunkKind::Function,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "test.rs".to_owned(),
                lines: (1, 5),
                symbol: "test".to_owned(),
                fqn: None,
                language: "Rust".to_owned(),
                parent: None,
                visibility: super::super::types::Visibility::Public,
                is_test: false,
            },
            context: ChunkContext::default(),
            part: None,
        };

        let reference = ChunkReference::from(&chunk);
        assert_eq!(reference.id, "ec_abc123");
        assert_eq!(reference.tokens, 10);
        assert_eq!(reference.lines, (1, 5));
        assert_eq!(reference.symbol, "test");
    }
}
