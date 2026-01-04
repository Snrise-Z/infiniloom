//! Tamper-evident audit logging with hash chain
//!
//! This module provides cryptographic audit logging for embedding operations.
//! Each log entry is linked to the previous via a hash chain, making tampering
//! detectable.
//!
//! # Features
//!
//! - **Hash chain**: Each entry's hash includes the previous entry's hash
//! - **Tamper detection**: Any modification breaks the chain
//! - **Append-only**: Logs can only be extended, not modified
//! - **Portable**: JSON-based format with optional binary storage
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::{AuditLog, AuditOperation};
//!
//! // Create a new audit log
//! let mut log = AuditLog::new();
//!
//! // Record an embedding operation
//! log.record(AuditOperation::EmbedStart {
//!     repo_path: "/path/to/repo".to_string(),
//!     settings_hash: "abc123".to_string(),
//! });
//!
//! // Record completion
//! log.record(AuditOperation::EmbedComplete {
//!     chunks_count: 150,
//!     total_tokens: 75000,
//!     manifest_hash: "def456".to_string(),
//! });
//!
//! // Verify integrity
//! assert!(log.verify_integrity());
//!
//! // Save to file
//! log.save(Path::new("audit.log"))?;
//! ```

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use blake3::Hasher;
use serde::{Deserialize, Serialize};

use super::error::EmbedError;

/// A single audit log entry with hash chain link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequential entry number (0-indexed)
    pub sequence: u64,

    /// Unix timestamp (seconds since epoch)
    pub timestamp: u64,

    /// Hash of the previous entry (empty string for first entry)
    pub prev_hash: String,

    /// Hash of this entry (includes prev_hash for chain)
    pub hash: String,

    /// The operation being logged
    pub operation: AuditOperation,
}

/// Operations that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuditOperation {
    /// Log file created
    LogCreated { version: u32, created_by: String },

    /// Embedding operation started
    EmbedStart { repo_path: String, settings_hash: String },

    /// Embedding operation completed successfully
    EmbedComplete { chunks_count: usize, total_tokens: u64, manifest_hash: String },

    /// Embedding operation failed
    EmbedFailed { error_code: String, error_message: String },

    /// Manifest loaded from disk
    ManifestLoaded { path: String, manifest_hash: String, chunks_count: usize },

    /// Manifest saved to disk
    ManifestSaved { path: String, manifest_hash: String },

    /// Diff computed between manifest and current state
    DiffComputed { added: usize, modified: usize, removed: usize },

    /// Batch embedding started
    BatchStart { repo_count: usize, total_settings_hash: String },

    /// Single repo in batch completed
    BatchRepoComplete { repo_index: usize, repo_path: String, chunks_count: usize, success: bool },

    /// Batch embedding completed
    BatchComplete { successful: usize, failed: usize, total_chunks: usize },

    /// Security scan performed
    SecurityScan { findings_count: usize, secrets_redacted: bool },

    /// Checkpoint created for resume
    CheckpointCreated { checkpoint_hash: String, files_processed: usize, chunks_generated: usize },

    /// Resume from checkpoint
    ResumeFromCheckpoint { checkpoint_hash: String, files_remaining: usize },

    /// Custom user-defined operation
    Custom { name: String, data: String },
}

/// Current audit log format version
pub const AUDIT_LOG_VERSION: u32 = 1;

/// Tamper-evident audit log with hash chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    /// Log format version
    pub version: u32,

    /// All entries in chronological order
    pub entries: Vec<AuditEntry>,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    /// Create a new empty audit log
    pub fn new() -> Self {
        let mut log = Self { version: AUDIT_LOG_VERSION, entries: Vec::new() };

        // Add initial entry
        log.record(AuditOperation::LogCreated {
            version: AUDIT_LOG_VERSION,
            created_by: format!("infiniloom-engine/{}", env!("CARGO_PKG_VERSION")),
        });

        log
    }

    /// Record a new operation in the audit log
    ///
    /// Returns the hash of the new entry.
    pub fn record(&mut self, operation: AuditOperation) -> String {
        let sequence = self.entries.len() as u64;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let prev_hash = self
            .entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_default();

        // Compute hash of this entry
        let hash = compute_entry_hash(sequence, timestamp, &prev_hash, &operation);

        let entry = AuditEntry { sequence, timestamp, prev_hash, hash: hash.clone(), operation };

        self.entries.push(entry);
        hash
    }

    /// Verify the integrity of the entire hash chain
    ///
    /// Returns `true` if all hashes are valid and the chain is intact.
    pub fn verify_integrity(&self) -> bool {
        let mut prev_hash = String::new();

        for entry in &self.entries {
            // Check prev_hash link
            if entry.prev_hash != prev_hash {
                return false;
            }

            // Recompute and verify hash
            let expected_hash =
                compute_entry_hash(entry.sequence, entry.timestamp, &prev_hash, &entry.operation);

            if entry.hash != expected_hash {
                return false;
            }

            prev_hash = entry.hash.clone();
        }

        true
    }

    /// Verify integrity and return detailed results
    pub fn verify_integrity_detailed(&self) -> IntegrityReport {
        let mut errors = Vec::new();
        let mut prev_hash = String::new();

        for (index, entry) in self.entries.iter().enumerate() {
            // Check prev_hash link
            if entry.prev_hash != prev_hash {
                errors.push(IntegrityError::ChainBroken {
                    entry_index: index,
                    expected_prev: prev_hash.clone(),
                    actual_prev: entry.prev_hash.clone(),
                });
            }

            // Recompute and verify hash
            let expected_hash =
                compute_entry_hash(entry.sequence, entry.timestamp, &prev_hash, &entry.operation);

            if entry.hash != expected_hash {
                errors.push(IntegrityError::HashMismatch {
                    entry_index: index,
                    expected: expected_hash,
                    actual: entry.hash.clone(),
                });
            }

            prev_hash = entry.hash.clone();
        }

        IntegrityReport { is_valid: errors.is_empty(), entries_checked: self.entries.len(), errors }
    }

    /// Get the number of entries in the log
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the log is empty (has only the initial entry)
    pub fn is_empty(&self) -> bool {
        self.entries.len() <= 1
    }

    /// Get the hash of the latest entry (chain head)
    pub fn head_hash(&self) -> Option<&str> {
        self.entries.last().map(|e| e.hash.as_str())
    }

    /// Get entries filtered by operation type
    pub fn filter_by_type<F>(&self, predicate: F) -> Vec<&AuditEntry>
    where
        F: Fn(&AuditOperation) -> bool,
    {
        self.entries
            .iter()
            .filter(|e| predicate(&e.operation))
            .collect()
    }

    /// Get entries in a time range (inclusive)
    pub fn filter_by_time(&self, start: u64, end: u64) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Save the audit log to a file (JSON format)
    pub fn save(&self, path: &Path) -> Result<(), EmbedError> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| EmbedError::SerializationError {
                reason: format!("Failed to serialize audit log: {}", e),
            })?;

        std::fs::write(path, json)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        Ok(())
    }

    /// Save as newline-delimited JSON (each entry on one line)
    ///
    /// This format is more suitable for streaming and appending.
    pub fn save_jsonl(&self, path: &Path) -> Result<(), EmbedError> {
        use std::io::Write;

        let file = std::fs::File::create(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        let mut writer = std::io::BufWriter::new(file);

        // Write header line with version
        let header = serde_json::json!({
            "audit_log_version": self.version,
            "entry_count": self.entries.len()
        });
        writeln!(writer, "{}", header)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Write each entry
        for entry in &self.entries {
            let line =
                serde_json::to_string(entry).map_err(|e| EmbedError::SerializationError {
                    reason: format!("Failed to serialize audit entry: {}", e),
                })?;
            writeln!(writer, "{}", line)
                .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;
        }

        writer
            .flush()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        Ok(())
    }

    /// Load an audit log from a file (JSON format)
    pub fn load(path: &Path) -> Result<Self, EmbedError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        let log: Self =
            serde_json::from_str(&content).map_err(|e| EmbedError::DeserializationError {
                reason: format!("Failed to deserialize audit log: {}", e),
            })?;

        // Verify integrity on load
        if !log.verify_integrity() {
            return Err(EmbedError::ManifestCorrupted {
                path: path.to_path_buf(),
                expected: "valid hash chain".to_owned(),
                actual: "hash chain broken".to_owned(),
            });
        }

        Ok(log)
    }

    /// Append a single entry to an existing JSONL file
    ///
    /// This is more efficient than rewriting the entire file.
    pub fn append_entry_to_file(path: &Path, entry: &AuditEntry) -> Result<(), EmbedError> {
        use std::io::Write;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        let mut writer = std::io::BufWriter::new(file);
        let line = serde_json::to_string(entry).map_err(|e| EmbedError::SerializationError {
            reason: format!("Failed to serialize audit entry: {}", e),
        })?;
        writeln!(writer, "{}", line)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        writer
            .flush()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        Ok(())
    }
}

/// Compute the hash of an entry for the chain
fn compute_entry_hash(
    sequence: u64,
    timestamp: u64,
    prev_hash: &str,
    operation: &AuditOperation,
) -> String {
    let mut hasher = Hasher::new();

    // Include sequence number
    hasher.update(&sequence.to_le_bytes());

    // Include timestamp
    hasher.update(&timestamp.to_le_bytes());

    // Include previous hash (chain link)
    hasher.update(prev_hash.as_bytes());

    // Include operation as JSON
    let op_json = serde_json::to_string(operation).unwrap_or_default();
    hasher.update(op_json.as_bytes());

    // Return hex-encoded hash
    hasher.finalize().to_hex().to_string()
}

/// Result of integrity verification
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    /// Whether the entire log is valid
    pub is_valid: bool,

    /// Number of entries checked
    pub entries_checked: usize,

    /// List of errors found
    pub errors: Vec<IntegrityError>,
}

/// Types of integrity errors
#[derive(Debug, Clone)]
pub enum IntegrityError {
    /// Hash chain is broken (prev_hash doesn't match)
    ChainBroken { entry_index: usize, expected_prev: String, actual_prev: String },

    /// Entry hash doesn't match computed value
    HashMismatch { entry_index: usize, expected: String, actual: String },
}

impl std::fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChainBroken { entry_index, expected_prev, actual_prev } => {
                write!(
                    f,
                    "Chain broken at entry {}: expected prev_hash '{}', got '{}'",
                    entry_index,
                    &expected_prev[..8.min(expected_prev.len())],
                    &actual_prev[..8.min(actual_prev.len())]
                )
            },
            Self::HashMismatch { entry_index, expected, actual } => {
                write!(
                    f,
                    "Hash mismatch at entry {}: expected '{}', got '{}'",
                    entry_index,
                    &expected[..8.min(expected.len())],
                    &actual[..8.min(actual.len())]
                )
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_audit_log() {
        let log = AuditLog::new();
        assert_eq!(log.version, AUDIT_LOG_VERSION);
        assert_eq!(log.entries.len(), 1); // Initial LogCreated entry
        assert!(log.verify_integrity());
    }

    #[test]
    fn test_record_operations() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::EmbedStart {
            repo_path: "/test/repo".to_owned(),
            settings_hash: "abc123".to_owned(),
        });

        log.record(AuditOperation::EmbedComplete {
            chunks_count: 100,
            total_tokens: 50000,
            manifest_hash: "def456".to_owned(),
        });

        assert_eq!(log.entries.len(), 3);
        assert!(log.verify_integrity());
    }

    #[test]
    fn test_hash_chain_integrity() {
        let mut log = AuditLog::new();

        for i in 0..10 {
            log.record(AuditOperation::Custom {
                name: format!("test_{}", i),
                data: format!("data_{}", i),
            });
        }

        assert!(log.verify_integrity());

        // Tamper with an entry's data
        if let AuditOperation::Custom { ref mut data, .. } = log.entries[5].operation {
            *data = "tampered".to_owned();
        }

        // Integrity should now fail
        assert!(!log.verify_integrity());
    }

    #[test]
    fn test_verify_integrity_detailed() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::EmbedStart {
            repo_path: "/test".to_owned(),
            settings_hash: "hash".to_owned(),
        });

        let report = log.verify_integrity_detailed();
        assert!(report.is_valid);
        assert_eq!(report.entries_checked, 2);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_tamper_detection_chain_broken() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::Custom { name: "op1".to_owned(), data: "data1".to_owned() });
        log.record(AuditOperation::Custom { name: "op2".to_owned(), data: "data2".to_owned() });

        // Break the chain by modifying prev_hash
        log.entries[2].prev_hash = "fake_hash".to_owned();

        let report = log.verify_integrity_detailed();
        assert!(!report.is_valid);
        assert!(!report.errors.is_empty());
        assert!(matches!(report.errors[0], IntegrityError::ChainBroken { .. }));
    }

    #[test]
    fn test_tamper_detection_hash_mismatch() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::Custom { name: "op1".to_owned(), data: "data1".to_owned() });

        // Modify the entry's own hash
        log.entries[1].hash = "fake_hash".to_owned();

        let report = log.verify_integrity_detailed();
        assert!(!report.is_valid);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, IntegrityError::HashMismatch { .. })));
    }

    #[test]
    fn test_head_hash() {
        let mut log = AuditLog::new();

        let initial_head = log.head_hash().map(String::from);
        assert!(initial_head.is_some());

        let new_hash =
            log.record(AuditOperation::Custom { name: "test".to_owned(), data: "data".to_owned() });

        assert_eq!(log.head_hash(), Some(new_hash.as_str()));
        assert_ne!(log.head_hash().map(String::from), initial_head);
    }

    #[test]
    fn test_filter_by_type() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::EmbedStart {
            repo_path: "/repo1".to_owned(),
            settings_hash: "h1".to_owned(),
        });
        log.record(AuditOperation::EmbedComplete {
            chunks_count: 100,
            total_tokens: 50000,
            manifest_hash: "m1".to_owned(),
        });
        log.record(AuditOperation::EmbedStart {
            repo_path: "/repo2".to_owned(),
            settings_hash: "h2".to_owned(),
        });

        let starts = log.filter_by_type(|op| matches!(op, AuditOperation::EmbedStart { .. }));
        assert_eq!(starts.len(), 2);

        let completes = log.filter_by_type(|op| matches!(op, AuditOperation::EmbedComplete { .. }));
        assert_eq!(completes.len(), 1);
    }

    #[test]
    fn test_filter_by_time() {
        let mut log = AuditLog::new();

        // All entries will have the same timestamp (or very close)
        log.record(AuditOperation::Custom { name: "test".to_owned(), data: "data".to_owned() });

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entries = log.filter_by_time(now - 60, now + 60);
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.json");

        let mut log = AuditLog::new();
        log.record(AuditOperation::EmbedStart {
            repo_path: "/test/repo".to_owned(),
            settings_hash: "abc123".to_owned(),
        });
        log.record(AuditOperation::EmbedComplete {
            chunks_count: 100,
            total_tokens: 50000,
            manifest_hash: "def456".to_owned(),
        });

        // Save
        log.save(&log_path).unwrap();

        // Load and verify
        let loaded = AuditLog::load(&log_path).unwrap();
        assert_eq!(loaded.entries.len(), log.entries.len());
        assert!(loaded.verify_integrity());

        // Compare hashes
        for (orig, loaded) in log.entries.iter().zip(loaded.entries.iter()) {
            assert_eq!(orig.hash, loaded.hash);
            assert_eq!(orig.prev_hash, loaded.prev_hash);
        }
    }

    #[test]
    fn test_save_jsonl() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.jsonl");

        let mut log = AuditLog::new();
        log.record(AuditOperation::Custom { name: "test".to_owned(), data: "data".to_owned() });

        log.save_jsonl(&log_path).unwrap();

        // Verify file exists and has content
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(!content.is_empty());

        // Count lines (header + entries)
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 entries
    }

    #[test]
    fn test_security_scan_operation() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::SecurityScan { findings_count: 5, secrets_redacted: true });

        assert!(log.verify_integrity());

        let scans = log.filter_by_type(|op| matches!(op, AuditOperation::SecurityScan { .. }));
        assert_eq!(scans.len(), 1);

        if let AuditOperation::SecurityScan { findings_count, secrets_redacted } =
            &scans[0].operation
        {
            assert_eq!(*findings_count, 5);
            assert!(*secrets_redacted);
        }
    }

    #[test]
    fn test_batch_operations() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::BatchStart {
            repo_count: 3,
            total_settings_hash: "settings_hash".to_owned(),
        });

        for i in 0..3 {
            log.record(AuditOperation::BatchRepoComplete {
                repo_index: i,
                repo_path: format!("/repo{}", i),
                chunks_count: 100 * (i + 1),
                success: true,
            });
        }

        log.record(AuditOperation::BatchComplete { successful: 3, failed: 0, total_chunks: 600 });

        assert!(log.verify_integrity());
        assert_eq!(log.entries.len(), 6); // LogCreated + BatchStart + 3 repos + BatchComplete
    }

    #[test]
    fn test_checkpoint_operations() {
        let mut log = AuditLog::new();

        log.record(AuditOperation::CheckpointCreated {
            checkpoint_hash: "ckpt_abc123".to_owned(),
            files_processed: 50,
            chunks_generated: 200,
        });

        log.record(AuditOperation::ResumeFromCheckpoint {
            checkpoint_hash: "ckpt_abc123".to_owned(),
            files_remaining: 100,
        });

        assert!(log.verify_integrity());
    }
}
