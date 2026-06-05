//! SQLite-backed manifest storage for enterprise-scale incremental updates
//!
//! This module provides a SQLite alternative to the bincode-based `EmbedManifest`,
//! designed for enterprise-scale deployments processing thousands of repositories
//! with millions of chunks.
//!
//! # Advantages over bincode manifest
//!
//! - **Per-file updates**: Only re-process changed files (vs full manifest load/save)
//! - **Concurrent reads**: WAL journal mode supports parallel consumers
//! - **Query flexibility**: Filter by file, kind, language without loading all chunks
//! - **Scalability**: Handles millions of chunks without memory pressure
//!
//! # Usage
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::sqlite_manifest::SqliteManifest;
//!
//! let manifest = SqliteManifest::new(Path::new("repo.infiniloom.db"))?;
//! manifest.save_chunks(&chunks)?;
//! let diff = manifest.diff(&new_chunks)?;
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, Connection, Transaction};

use super::error::EmbedError;
use super::manifest::{DiffSummary, EmbedDiff, ManifestEntry, ModifiedChunk, RemovedChunk};
use super::types::{ChunkKind, EmbedChunk, EmbedSettings};

/// SQLite-backed manifest for enterprise-scale incremental updates
pub struct SqliteManifest {
    conn: Connection,
}

impl SqliteManifest {
    /// Create or open a SQLite manifest database
    ///
    /// Initializes the schema on first use. Uses WAL journal mode
    /// for concurrent read access.
    pub fn new(path: &Path) -> Result<Self, EmbedError> {
        let conn = Connection::open(path).map_err(|e| EmbedError::SqliteError {
            reason: format!("Failed to open database: {e}"),
        })?;

        // WAL mode for concurrent reads
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to set WAL mode: {e}"),
            })?;

        // Reasonable busy timeout for concurrent access
        conn.pragma_update(None, "busy_timeout", 5000)
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to set busy_timeout: {e}"),
            })?;

        let manifest = Self { conn };
        manifest.init_schema()?;

        Ok(manifest)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<(), EmbedError> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS manifest_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                location_key TEXT PRIMARY KEY,
                chunk_id TEXT NOT NULL,
                full_hash TEXT NOT NULL,
                tokens INTEGER NOT NULL,
                lines_start INTEGER NOT NULL,
                lines_end INTEGER NOT NULL,
                file TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file);
            CREATE INDEX IF NOT EXISTS idx_chunks_chunk_id ON chunks(chunk_id);
        ",
            )
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to initialize schema: {e}"),
            })?;

        Ok(())
    }

    /// Save chunks into the database, replacing all existing data
    ///
    /// Uses a single transaction for atomicity and performance.
    pub fn save_chunks(&self, chunks: &[EmbedChunk]) -> Result<(), EmbedError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to begin transaction: {e}"),
            })?;

        // Clear existing chunks
        tx.execute("DELETE FROM chunks", [])
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to clear chunks: {e}"),
            })?;

        Self::insert_chunks_batch(&tx, chunks)?;

        tx.commit().map_err(|e| EmbedError::SqliteError {
            reason: format!("Failed to commit transaction: {e}"),
        })?;

        Ok(())
    }

    /// Upsert chunks for specific files (incremental update)
    ///
    /// Only replaces chunks for files that appear in the input.
    /// Chunks from other files are left untouched.
    pub fn upsert_chunks_for_files(&self, chunks: &[EmbedChunk]) -> Result<(), EmbedError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to begin transaction: {e}"),
            })?;

        // Collect unique files from new chunks
        let mut files: Vec<&str> = chunks.iter().map(|c| c.source.file.as_str()).collect();
        files.sort_unstable();
        files.dedup();

        // Remove existing chunks for those files
        for file in &files {
            tx.execute("DELETE FROM chunks WHERE file = ?1", params![file])
                .map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to remove chunks for file {file}: {e}"),
                })?;
        }

        Self::insert_chunks_batch(&tx, chunks)?;

        tx.commit().map_err(|e| EmbedError::SqliteError {
            reason: format!("Failed to commit transaction: {e}"),
        })?;

        Ok(())
    }

    /// Insert chunks in a batch within an existing transaction
    fn insert_chunks_batch(tx: &Transaction<'_>, chunks: &[EmbedChunk]) -> Result<(), EmbedError> {
        let mut stmt = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO chunks (location_key, chunk_id, full_hash, tokens, lines_start, lines_end, file)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to prepare insert statement: {e}"),
            })?;

        for chunk in chunks {
            let location_key = super::manifest::EmbedManifest::location_key(chunk);

            stmt.execute(params![
                location_key,
                chunk.id,
                chunk.full_hash,
                chunk.tokens,
                chunk.source.lines.0,
                chunk.source.lines.1,
                chunk.source.file,
            ])
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to insert chunk: {e}"),
            })?;
        }

        Ok(())
    }

    /// Load manifest entries, optionally filtered by file path
    pub fn load_chunks(&self, file_filter: Option<&str>) -> Result<Vec<ManifestEntry>, EmbedError> {
        let mut entries = Vec::new();

        if let Some(file) = file_filter {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT chunk_id, full_hash, tokens, lines_start, lines_end FROM chunks WHERE file = ?1",
                )
                .map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to prepare query: {e}"),
                })?;

            let rows = stmt
                .query_map(params![file], |row| {
                    Ok(ManifestEntry {
                        chunk_id: row.get(0)?,
                        full_hash: row.get(1)?,
                        tokens: row.get(2)?,
                        lines: (row.get(3)?, row.get(4)?),
                    })
                })
                .map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to query chunks: {e}"),
                })?;

            for row in rows {
                entries.push(row.map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to read row: {e}"),
                })?);
            }
        } else {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT chunk_id, full_hash, tokens, lines_start, lines_end FROM chunks",
                )
                .map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to prepare query: {e}"),
                })?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(ManifestEntry {
                        chunk_id: row.get(0)?,
                        full_hash: row.get(1)?,
                        tokens: row.get(2)?,
                        lines: (row.get(3)?, row.get(4)?),
                    })
                })
                .map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to query chunks: {e}"),
                })?;

            for row in rows {
                entries.push(row.map_err(|e| EmbedError::SqliteError {
                    reason: format!("Failed to read row: {e}"),
                })?);
            }
        }

        Ok(entries)
    }

    /// Compute diff between current chunks and stored state
    ///
    /// Has the same semantics as `EmbedManifest::diff()`.
    pub fn diff(&self, current_chunks: &[EmbedChunk]) -> Result<EmbedDiff, EmbedError> {
        // Load all stored entries keyed by location_key
        let mut stored: BTreeMap<String, (String, String, u32, (u32, u32))> = BTreeMap::new();

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT location_key, chunk_id, full_hash, tokens, lines_start, lines_end FROM chunks",
            )
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to prepare query: {e}"),
            })?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, u32>(5)?,
                ))
            })
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to query chunks: {e}"),
            })?;

        for row in rows {
            let (key, chunk_id, full_hash, tokens, start, end) = row.map_err(|e| {
                EmbedError::SqliteError { reason: format!("Failed to read row: {e}") }
            })?;
            stored.insert(key, (chunk_id, full_hash, tokens, (start, end)));
        }

        // Build map of current chunks
        let current_map: BTreeMap<String, &EmbedChunk> = current_chunks
            .iter()
            .map(|c| (super::manifest::EmbedManifest::location_key(c), c))
            .collect();

        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut removed = Vec::new();
        let mut unchanged = Vec::new();

        // Find modified and unchanged
        for (key, (chunk_id, _full_hash, _tokens, _lines)) in &stored {
            if let Some(current) = current_map.get(key) {
                if current.id == *chunk_id {
                    unchanged.push(current.id.clone());
                } else {
                    modified.push(ModifiedChunk {
                        old_id: chunk_id.clone(),
                        new_id: current.id.clone(),
                        chunk: (*current).clone(),
                    });
                }
            } else {
                removed.push(RemovedChunk { id: chunk_id.clone(), location_key: key.clone() });
            }
        }

        // Find added
        for (key, chunk) in &current_map {
            if !stored.contains_key(key) {
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

        Ok(EmbedDiff { summary, added, modified, removed, unchanged })
    }

    /// Remove all chunks for a given file
    pub fn remove_by_file(&self, file: &str) -> Result<usize, EmbedError> {
        let count = self
            .conn
            .execute("DELETE FROM chunks WHERE file = ?1", params![file])
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to remove chunks for file: {e}"),
            })?;

        Ok(count)
    }

    /// Get total number of chunks stored
    pub fn chunk_count(&self) -> Result<usize, EmbedError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to count chunks: {e}"),
            })?;

        Ok(count as usize)
    }

    /// Store settings in metadata
    pub fn save_settings(&self, settings: &EmbedSettings) -> Result<(), EmbedError> {
        let json = serde_json::to_string(settings).map_err(|e| EmbedError::SqliteError {
            reason: format!("Failed to serialize settings: {e}"),
        })?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO manifest_meta (key, value) VALUES ('settings', ?1)",
                params![json],
            )
            .map_err(|e| EmbedError::SqliteError {
                reason: format!("Failed to save settings: {e}"),
            })?;

        Ok(())
    }

    /// Check if stored settings match the given settings
    pub fn settings_match(&self, settings: &EmbedSettings) -> Result<bool, EmbedError> {
        let stored: Option<String> = self
            .conn
            .query_row("SELECT value FROM manifest_meta WHERE key = 'settings'", [], |row| {
                row.get(0)
            })
            .ok();

        match stored {
            Some(json) => {
                let stored_settings: EmbedSettings =
                    serde_json::from_str(&json).map_err(|e| EmbedError::SqliteError {
                        reason: format!("Failed to deserialize stored settings: {e}"),
                    })?;
                Ok(stored_settings == *settings)
            },
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::types::{
        ChunkContext, ChunkKind, ChunkSource, EmbedChunk, EmbedSettings, RepoIdentifier, Visibility,
    };
    use tempfile::TempDir;

    fn test_chunk(id: &str, file: &str, symbol: &str) -> EmbedChunk {
        EmbedChunk {
            id: id.to_owned(),
            full_hash: format!("{id}_full"),
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
                module_path: None,
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext::default(),
            children_ids: Vec::new(),
            repr: "code".to_owned(),
            code_chunk_id: None,
            part: None,
        }
    }

    #[test]
    fn test_new_db_is_empty() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();
        assert_eq!(manifest.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_save_and_load_chunks() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let chunks =
            vec![test_chunk("ec_1", "src/foo.rs", "foo"), test_chunk("ec_2", "src/bar.rs", "bar")];

        manifest.save_chunks(&chunks).unwrap();
        assert_eq!(manifest.chunk_count().unwrap(), 2);

        let loaded = manifest.load_chunks(None).unwrap();
        assert_eq!(loaded.len(), 2);

        let filtered = manifest.load_chunks(Some("src/foo.rs")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].chunk_id, "ec_1");
    }

    #[test]
    fn test_diff_added() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let chunks = vec![test_chunk("ec_1", "src/foo.rs", "foo")];
        let diff = manifest.diff(&chunks).unwrap();

        assert_eq!(diff.summary.added, 1);
        assert_eq!(diff.summary.modified, 0);
        assert_eq!(diff.summary.removed, 0);
    }

    #[test]
    fn test_diff_modified() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let old_chunks = vec![test_chunk("ec_old", "src/foo.rs", "foo")];
        manifest.save_chunks(&old_chunks).unwrap();

        let new_chunks = vec![test_chunk("ec_new", "src/foo.rs", "foo")];
        let diff = manifest.diff(&new_chunks).unwrap();

        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.modified, 1);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.modified[0].old_id, "ec_old");
        assert_eq!(diff.modified[0].new_id, "ec_new");
    }

    #[test]
    fn test_diff_removed() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let chunks = vec![test_chunk("ec_1", "src/foo.rs", "foo")];
        manifest.save_chunks(&chunks).unwrap();

        let diff = manifest.diff(&[]).unwrap();
        assert_eq!(diff.summary.removed, 1);
        assert_eq!(diff.summary.added, 0);
    }

    #[test]
    fn test_remove_by_file() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let chunks = vec![
            test_chunk("ec_1", "src/foo.rs", "foo"),
            test_chunk("ec_2", "src/foo.rs", "bar"),
            test_chunk("ec_3", "src/baz.rs", "baz"),
        ];
        manifest.save_chunks(&chunks).unwrap();
        assert_eq!(manifest.chunk_count().unwrap(), 3);

        let removed = manifest.remove_by_file("src/foo.rs").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(manifest.chunk_count().unwrap(), 1);
    }

    #[test]
    fn test_settings_match() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let settings = EmbedSettings::default();
        assert!(!manifest.settings_match(&settings).unwrap());

        manifest.save_settings(&settings).unwrap();
        assert!(manifest.settings_match(&settings).unwrap());

        let mut different = EmbedSettings::default();
        different.max_tokens = 2000;
        assert!(!manifest.settings_match(&different).unwrap());
    }

    #[test]
    fn test_upsert_for_files() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let manifest = SqliteManifest::new(&db_path).unwrap();

        let initial =
            vec![test_chunk("ec_1", "src/foo.rs", "foo"), test_chunk("ec_2", "src/bar.rs", "bar")];
        manifest.save_chunks(&initial).unwrap();
        assert_eq!(manifest.chunk_count().unwrap(), 2);

        // Update only foo.rs with new chunk
        let update = vec![test_chunk("ec_new", "src/foo.rs", "foo_updated")];
        manifest.upsert_chunks_for_files(&update).unwrap();

        assert_eq!(manifest.chunk_count().unwrap(), 2);
        let foo_chunks = manifest.load_chunks(Some("src/foo.rs")).unwrap();
        assert_eq!(foo_chunks.len(), 1);
        assert_eq!(foo_chunks[0].chunk_id, "ec_new");

        // bar.rs unchanged
        let bar_chunks = manifest.load_chunks(Some("src/bar.rs")).unwrap();
        assert_eq!(bar_chunks.len(), 1);
        assert_eq!(bar_chunks[0].chunk_id, "ec_2");
    }
}
