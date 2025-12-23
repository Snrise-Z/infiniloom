//! Index storage and serialization.
//!
//! Handles saving and loading the symbol index and dependency graph
//! using bincode for fast binary serialization.

use super::types::{DepGraph, SymbolIndex};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Index storage directory name
pub const INDEX_DIR: &str = ".infiniloom";

/// Index file names
pub const INDEX_FILE: &str = "index.bin";
pub const GRAPH_FILE: &str = "graph.bin";
pub const META_FILE: &str = "meta.json";
pub const CONFIG_FILE: &str = "config.toml";

/// Errors that can occur during index storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Index not found at {0}")]
    NotFound(PathBuf),

    #[error("Index version mismatch: found {found}, expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },

    #[error("Invalid index directory: {0}")]
    InvalidDirectory(String),
}

/// Metadata about the index (human-readable JSON)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexMeta {
    /// Index version
    pub version: u32,
    /// Repository name
    pub repo_name: String,
    /// Git commit hash when index was built
    pub commit_hash: Option<String>,
    /// Timestamp of index creation (Unix epoch seconds)
    pub created_at: u64,
    /// Number of files indexed
    pub file_count: usize,
    /// Number of symbols indexed
    pub symbol_count: usize,
    /// Total size of index files in bytes
    pub index_size_bytes: u64,
}

/// Index storage manager
pub struct IndexStorage {
    /// Path to the index directory (.infiniloom)
    index_dir: PathBuf,
}

impl IndexStorage {
    /// Create a new storage manager for a repository
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self { index_dir: repo_root.as_ref().join(INDEX_DIR) }
    }

    /// Get path to the index directory
    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    /// Check if index exists
    pub fn exists(&self) -> bool {
        self.index_dir.join(INDEX_FILE).exists() && self.index_dir.join(GRAPH_FILE).exists()
    }

    /// Initialize the index directory structure
    pub fn init(&self) -> Result<(), StorageError> {
        // Create .infiniloom directory
        fs::create_dir_all(&self.index_dir)?;

        // Create .gitignore for temporary files only
        let gitignore_path = self.index_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "*.tmp\n*.lock\n")?;
        }

        Ok(())
    }

    /// Save the symbol index to disk
    pub fn save_index(&self, index: &SymbolIndex) -> Result<(), StorageError> {
        self.init()?;

        let path = self.index_dir.join(INDEX_FILE);
        let tmp_path = self.index_dir.join(format!("{}.tmp", INDEX_FILE));

        // Write to temp file first for atomicity
        let file = File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);
        bincode::serialize_into(&mut writer, index)?;
        writer.flush()?;

        // Atomic rename
        fs::rename(&tmp_path, &path)?;

        Ok(())
    }

    /// Load the symbol index from disk
    pub fn load_index(&self) -> Result<SymbolIndex, StorageError> {
        let path = self.index_dir.join(INDEX_FILE);

        if !path.exists() {
            return Err(StorageError::NotFound(path));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut index: SymbolIndex = bincode::deserialize_from(reader)?;

        // Check version compatibility
        if index.version != SymbolIndex::CURRENT_VERSION {
            return Err(StorageError::VersionMismatch {
                found: index.version,
                expected: SymbolIndex::CURRENT_VERSION,
            });
        }

        // Rebuild lookup tables
        index.rebuild_lookups();

        Ok(index)
    }

    /// Save the dependency graph to disk
    pub fn save_graph(&self, graph: &DepGraph) -> Result<(), StorageError> {
        self.init()?;

        let path = self.index_dir.join(GRAPH_FILE);
        let tmp_path = self.index_dir.join(format!("{}.tmp", GRAPH_FILE));

        let file = File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);
        bincode::serialize_into(&mut writer, graph)?;
        writer.flush()?;

        fs::rename(&tmp_path, &path)?;

        Ok(())
    }

    /// Load the dependency graph from disk
    pub fn load_graph(&self) -> Result<DepGraph, StorageError> {
        let path = self.index_dir.join(GRAPH_FILE);

        if !path.exists() {
            return Err(StorageError::NotFound(path));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let graph: DepGraph = bincode::deserialize_from(reader)?;

        Ok(graph)
    }

    /// Save index metadata (human-readable JSON)
    pub fn save_meta(&self, meta: &IndexMeta) -> Result<(), StorageError> {
        self.init()?;

        let path = self.index_dir.join(META_FILE);
        let json = serde_json::to_string_pretty(meta)?;
        fs::write(&path, json)?;

        Ok(())
    }

    /// Load index metadata
    pub fn load_meta(&self) -> Result<IndexMeta, StorageError> {
        let path = self.index_dir.join(META_FILE);

        if !path.exists() {
            return Err(StorageError::NotFound(path));
        }

        let content = fs::read_to_string(&path)?;
        let meta: IndexMeta = serde_json::from_str(&content)?;

        Ok(meta)
    }

    /// Save everything (index, graph, meta) atomically
    pub fn save_all(
        &self,
        index: &SymbolIndex,
        graph: &DepGraph,
    ) -> Result<IndexMeta, StorageError> {
        // Save index and graph
        self.save_index(index)?;
        self.save_graph(graph)?;

        // Calculate sizes
        let index_size = fs::metadata(self.index_dir.join(INDEX_FILE))?.len();
        let graph_size = fs::metadata(self.index_dir.join(GRAPH_FILE))?.len();

        // Create and save metadata
        let meta = IndexMeta {
            version: index.version,
            repo_name: index.repo_name.clone(),
            commit_hash: index.commit_hash.clone(),
            created_at: index.created_at,
            file_count: index.files.len(),
            symbol_count: index.symbols.len(),
            index_size_bytes: index_size + graph_size,
        };

        self.save_meta(&meta)?;

        Ok(meta)
    }

    /// Load everything (index, graph)
    pub fn load_all(&self) -> Result<(SymbolIndex, DepGraph), StorageError> {
        let index = self.load_index()?;
        let graph = self.load_graph()?;
        Ok((index, graph))
    }

    /// Get size of stored index files
    pub fn storage_size(&self) -> u64 {
        let mut total = 0u64;

        for name in [INDEX_FILE, GRAPH_FILE, META_FILE] {
            if let Ok(metadata) = fs::metadata(self.index_dir.join(name)) {
                total += metadata.len();
            }
        }

        total
    }

    /// Delete the index
    pub fn delete(&self) -> Result<(), StorageError> {
        if self.index_dir.exists() {
            fs::remove_dir_all(&self.index_dir)?;
        }
        Ok(())
    }
}

// Note: Memory-mapped index loader can be added as a future optimization
// for very large repositories. For now, the standard file-based loader
// is sufficient and provides good performance.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{
        FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId, Visibility,
    };
    use tempfile::TempDir;

    #[test]
    fn test_storage_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path());

        // Create test index
        let mut index = SymbolIndex::new();
        index.repo_name = "test-repo".to_owned();
        index.created_at = 12345;
        index.files.push(FileEntry {
            id: FileId::new(0),
            path: "src/main.rs".to_owned(),
            language: Language::Rust,
            content_hash: [1; 32],
            symbols: 0..1,
            imports: vec![],
            lines: 100,
            tokens: 500,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(0),
            name: "main".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(1, 0, 10, 0),
            signature: Some("fn main()".to_owned()),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });

        // Create test graph
        let mut graph = DepGraph::new();
        graph.add_file_import(0, 1);

        // Save
        storage.save_all(&index, &graph).unwrap();

        // Verify files exist
        assert!(storage.exists());
        assert!(storage.storage_size() > 0);

        // Load and verify
        let (loaded_index, loaded_graph) = storage.load_all().unwrap();
        assert_eq!(loaded_index.repo_name, "test-repo");
        assert_eq!(loaded_index.files.len(), 1);
        assert_eq!(loaded_index.symbols.len(), 1);
        assert_eq!(loaded_graph.file_imports.len(), 1);

        // Verify lookups work
        assert!(loaded_index.get_file("src/main.rs").is_some());
    }

    #[test]
    fn test_meta_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path());
        storage.init().unwrap();

        let meta = IndexMeta {
            version: 1,
            repo_name: "test".to_owned(),
            commit_hash: Some("abc123".to_owned()),
            created_at: 12345,
            file_count: 10,
            symbol_count: 100,
            index_size_bytes: 1024,
        };

        storage.save_meta(&meta).unwrap();
        let loaded = storage.load_meta().unwrap();

        assert_eq!(loaded.repo_name, "test");
        assert_eq!(loaded.file_count, 10);
    }

    #[test]
    fn test_not_found() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path());

        assert!(!storage.exists());
        assert!(matches!(storage.load_index(), Err(StorageError::NotFound(_))));
    }
}
