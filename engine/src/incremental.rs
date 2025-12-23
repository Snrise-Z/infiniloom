//! Incremental scanning with file watching and caching
//!
//! Provides efficient re-scanning by caching results and only processing changed files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;

use crate::tokenizer::TokenCounts;
use crate::types::Symbol;

/// Cache entry for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    /// Relative path
    pub path: String,
    /// Last modified time (Unix timestamp)
    pub mtime: u64,
    /// File size in bytes
    pub size: u64,
    /// Content hash (for change detection)
    pub hash: u64,
    /// Token counts
    pub tokens: TokenCounts,
    /// Extracted symbols
    pub symbols: Vec<CachedSymbol>,
    /// Whether symbols were extracted for this file
    pub symbols_extracted: bool,
    /// Detected language
    pub language: Option<String>,
    /// Line count
    pub lines: usize,
}

/// Cached symbol (simplified for storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
}

impl From<&Symbol> for CachedSymbol {
    fn from(s: &Symbol) -> Self {
        Self {
            name: s.name.clone(),
            kind: s.kind.name().to_owned(),
            start_line: s.start_line,
            end_line: s.end_line,
            signature: s.signature.clone(),
        }
    }
}

impl From<&CachedSymbol> for Symbol {
    fn from(s: &CachedSymbol) -> Self {
        use crate::types::{SymbolKind, Visibility};
        Self {
            name: s.name.clone(),
            kind: SymbolKind::from_str(&s.kind).unwrap_or(SymbolKind::Variable),
            start_line: s.start_line,
            end_line: s.end_line,
            signature: s.signature.clone(),
            docstring: None,
            visibility: Visibility::Public,
            references: 0,
            importance: 0.5,
            parent: None,
            calls: Vec::new(),
            extends: None,
            implements: Vec::new(),
        }
    }
}

/// Repository cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCache {
    /// Cache version (for compatibility)
    pub version: u32,
    /// Repository root path
    pub root_path: String,
    /// Cache creation time
    pub created_at: u64,
    /// Last update time
    pub updated_at: u64,
    /// Cached files
    pub files: HashMap<String, CachedFile>,
    /// Total token count
    pub total_tokens: TokenCounts,
    /// External dependencies detected
    pub external_deps: Vec<String>,
}

impl RepoCache {
    /// Current cache version
    pub const VERSION: u32 = 2;

    /// Create a new empty cache
    pub fn new(root_path: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: Self::VERSION,
            root_path: root_path.to_owned(),
            created_at: now,
            updated_at: now,
            files: HashMap::new(),
            total_tokens: TokenCounts::default(),
            external_deps: Vec::new(),
        }
    }

    /// Load cache from file
    pub fn load(cache_path: &Path) -> Result<Self, CacheError> {
        let content = fs::read(cache_path).map_err(|e| CacheError::IoError(e.to_string()))?;

        let cache: Self = bincode::deserialize(&content)
            .map_err(|e| CacheError::DeserializeError(e.to_string()))?;

        // Check version compatibility
        if cache.version != Self::VERSION {
            return Err(CacheError::VersionMismatch {
                expected: Self::VERSION,
                found: cache.version,
            });
        }

        Ok(cache)
    }

    /// Save cache to file
    pub fn save(&self, cache_path: &Path) -> Result<(), CacheError> {
        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|e| CacheError::IoError(e.to_string()))?;
        }

        let content =
            bincode::serialize(self).map_err(|e| CacheError::SerializeError(e.to_string()))?;

        fs::write(cache_path, content).map_err(|e| CacheError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Get default cache path for a repository
    pub fn default_cache_path(repo_path: &Path) -> PathBuf {
        repo_path.join(".infiniloom/cache/repo.cache")
    }

    /// Check if a file needs rescanning based on mtime and size
    pub fn needs_rescan(&self, path: &str, current_mtime: u64, current_size: u64) -> bool {
        match self.files.get(path) {
            Some(cached) => cached.mtime != current_mtime || cached.size != current_size,
            None => true,
        }
    }

    /// Check if a file needs rescanning, including content hash comparison
    /// This catches changes that don't modify mtime/size (e.g., touch followed by edit)
    pub fn needs_rescan_with_hash(
        &self,
        path: &str,
        current_mtime: u64,
        current_size: u64,
        current_hash: u64,
    ) -> bool {
        match self.files.get(path) {
            Some(cached) => {
                cached.mtime != current_mtime
                    || cached.size != current_size
                    || (cached.hash != 0 && current_hash != 0 && cached.hash != current_hash)
            },
            None => true,
        }
    }

    /// Get a cached file by path
    pub fn get(&self, path: &str) -> Option<&CachedFile> {
        self.files.get(path)
    }

    /// Add or update a file in the cache
    pub fn update_file(&mut self, file: CachedFile) {
        self.files.insert(file.path.clone(), file);
        self.updated_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    /// Remove a file from the cache
    pub fn remove_file(&mut self, path: &str) {
        self.files.remove(path);
    }

    /// Get files that no longer exist
    pub fn find_deleted_files(&self, current_files: &[&str]) -> Vec<String> {
        let current_set: std::collections::HashSet<&str> = current_files.iter().copied().collect();
        self.files
            .keys()
            .filter(|p| !current_set.contains(p.as_str()))
            .cloned()
            .collect()
    }

    /// Recalculate total tokens
    pub fn recalculate_totals(&mut self) {
        self.total_tokens = self.files.values().map(|f| f.tokens).sum();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            file_count: self.files.len(),
            total_tokens: self.total_tokens,
            total_bytes: self.files.values().map(|f| f.size).sum(),
            age_seconds: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_sub(self.updated_at),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub file_count: usize,
    pub total_tokens: TokenCounts,
    pub total_bytes: u64,
    pub age_seconds: u64,
}

/// Cache errors
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializeError(String),
    #[error("Deserialization error: {0}")]
    DeserializeError(String),
    #[error("Cache version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
}

/// Incremental scanner that uses caching
pub struct IncrementalScanner {
    cache: RepoCache,
    cache_path: PathBuf,
    dirty: bool,
}

impl IncrementalScanner {
    /// Create or load an incremental scanner for a repository
    pub fn new(repo_path: &Path) -> Self {
        let cache_path = RepoCache::default_cache_path(repo_path);

        let cache = RepoCache::load(&cache_path)
            .unwrap_or_else(|_| RepoCache::new(&repo_path.to_string_lossy()));

        Self { cache, cache_path, dirty: false }
    }

    /// Create with custom cache path
    pub fn with_cache_path(repo_path: &Path, cache_path: PathBuf) -> Self {
        let cache = RepoCache::load(&cache_path)
            .unwrap_or_else(|_| RepoCache::new(&repo_path.to_string_lossy()));

        Self { cache, cache_path, dirty: false }
    }

    /// Check if a file needs to be rescanned (fast check using mtime/size only)
    pub fn needs_rescan(&self, path: &Path) -> bool {
        let metadata = match path.metadata() {
            Ok(m) => m,
            Err(_) => return true,
        };

        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let relative_path = path.to_string_lossy();
        self.cache
            .needs_rescan(&relative_path, mtime, metadata.len())
    }

    /// Check if a file needs to be rescanned, including content hash check
    /// This is more accurate but requires reading the file content
    pub fn needs_rescan_with_content(&self, path: &Path, content: &[u8]) -> bool {
        let metadata = match path.metadata() {
            Ok(m) => m,
            Err(_) => return true,
        };

        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let content_hash = hash_content(content);
        let relative_path = path.to_string_lossy();
        self.cache
            .needs_rescan_with_hash(&relative_path, mtime, metadata.len(), content_hash)
    }

    /// Get cached file if available and up-to-date
    pub fn get_cached(&self, path: &str) -> Option<&CachedFile> {
        self.cache.files.get(path)
    }

    /// Update cache with new file data
    pub fn update(&mut self, file: CachedFile) {
        self.cache.update_file(file);
        self.dirty = true;
    }

    /// Remove a deleted file from cache
    pub fn remove(&mut self, path: &str) {
        self.cache.remove_file(path);
        self.dirty = true;
    }

    /// Save cache if modified
    pub fn save(&mut self) -> Result<(), CacheError> {
        if self.dirty {
            self.cache.recalculate_totals();
            self.cache.save(&self.cache_path)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Force save cache
    pub fn force_save(&mut self) -> Result<(), CacheError> {
        self.cache.recalculate_totals();
        self.cache.save(&self.cache_path)?;
        self.dirty = false;
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache = RepoCache::new(&self.cache.root_path);
        self.dirty = true;
    }

    /// Get list of changed files compared to current state
    pub fn get_changed_files<'a>(
        &self,
        current_files: &'a [(PathBuf, u64, u64)],
    ) -> Vec<&'a PathBuf> {
        current_files
            .iter()
            .filter(|(path, mtime, size)| {
                let relative = path.to_string_lossy();
                self.cache.needs_rescan(&relative, *mtime, *size)
            })
            .map(|(path, _, _)| path)
            .collect()
    }
}

/// File change event for watching
#[derive(Debug, Clone)]
pub enum FileChange {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// File watcher using notify crate (when 'watch' feature enabled)
#[cfg(feature = "watch")]
pub mod watcher {
    use super::*;
    use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::{channel, Receiver};

    /// File system watcher for incremental updates
    pub struct FileWatcher {
        watcher: RecommendedWatcher,
        receiver: Receiver<Result<Event, notify::Error>>,
        root_path: PathBuf,
    }

    impl FileWatcher {
        /// Create a new file watcher for a directory
        pub fn new(path: &Path) -> Result<Self, notify::Error> {
            let (tx, rx) = channel();

            let watcher = RecommendedWatcher::new(
                move |res| {
                    let _ = tx.send(res);
                },
                Config::default(),
            )?;

            let mut fw = Self { watcher, receiver: rx, root_path: path.to_path_buf() };

            fw.watcher.watch(path, RecursiveMode::Recursive)?;

            Ok(fw)
        }

        /// Get next file change event (non-blocking)
        pub fn try_next(&self) -> Option<FileChange> {
            match self.receiver.try_recv() {
                Ok(Ok(event)) => self.event_to_change(event),
                _ => None,
            }
        }

        /// Wait for next file change event (blocking)
        pub fn next(&self) -> Option<FileChange> {
            match self.receiver.recv() {
                Ok(Ok(event)) => self.event_to_change(event),
                _ => None,
            }
        }

        /// Convert notify event to FileChange
        fn event_to_change(&self, event: Event) -> Option<FileChange> {
            let path = event.paths.first()?.clone();

            match event.kind {
                EventKind::Create(_) => Some(FileChange::Created(path)),
                EventKind::Modify(_) => Some(FileChange::Modified(path)),
                EventKind::Remove(_) => Some(FileChange::Deleted(path)),
                _ => None,
            }
        }

        /// Stop watching
        pub fn stop(mut self) -> Result<(), notify::Error> {
            self.watcher.unwatch(&self.root_path)
        }
    }
}

/// Compute a simple hash for change detection
pub fn hash_content(content: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Get file modification time as Unix timestamp
pub fn get_mtime(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_create_save_load() {
        let temp = TempDir::new().unwrap();
        let cache_path = temp.path().join("test.cache");

        let mut cache = RepoCache::new("/test/repo");
        cache.files.insert(
            "test.py".to_string(),
            CachedFile {
                path: "test.py".to_string(),
                mtime: 12345,
                size: 100,
                hash: 0,
                tokens: TokenCounts {
                    o200k: 45,
                    cl100k: 48,
                    claude: 50,
                    gemini: 46,
                    llama: 50,
                    mistral: 50,
                    deepseek: 50,
                    qwen: 50,
                    cohere: 48,
                    grok: 50,
                },
                symbols: vec![],
                symbols_extracted: false,
                language: Some("python".to_string()),
                lines: 10,
            },
        );

        cache.save(&cache_path).unwrap();

        let loaded = RepoCache::load(&cache_path).unwrap();
        assert_eq!(loaded.files.len(), 1);
        assert!(loaded.files.contains_key("test.py"));
    }

    #[test]
    fn test_needs_rescan() {
        let cache = RepoCache::new("/test");
        assert!(cache.needs_rescan("new_file.py", 0, 0));

        let mut cache = RepoCache::new("/test");
        cache.files.insert(
            "existing.py".to_string(),
            CachedFile {
                path: "existing.py".to_string(),
                mtime: 1000,
                size: 500,
                hash: 0,
                tokens: TokenCounts::default(),
                symbols: vec![],
                symbols_extracted: false,
                language: None,
                lines: 0,
            },
        );

        assert!(!cache.needs_rescan("existing.py", 1000, 500));
        assert!(cache.needs_rescan("existing.py", 2000, 500)); // mtime changed
        assert!(cache.needs_rescan("existing.py", 1000, 600)); // size changed
    }

    #[test]
    fn test_incremental_scanner() {
        let temp = TempDir::new().unwrap();

        let mut scanner = IncrementalScanner::new(temp.path());
        assert!(scanner.needs_rescan(&temp.path().join("test.py")));

        scanner.update(CachedFile {
            path: "test.py".to_string(),
            mtime: 1000,
            size: 100,
            hash: 0,
            tokens: TokenCounts::default(),
            symbols: vec![],
            symbols_extracted: false,
            language: Some("python".to_string()),
            lines: 5,
        });

        assert!(scanner.get_cached("test.py").is_some());
    }

    #[test]
    fn test_hash_content() {
        let h1 = hash_content(b"hello world");
        let h2 = hash_content(b"hello world");
        let h3 = hash_content(b"different");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_needs_rescan_with_hash() {
        let mut cache = RepoCache::new("/test");
        let original_hash = hash_content(b"original content");
        let modified_hash = hash_content(b"modified content");

        cache.files.insert(
            "file.py".to_string(),
            CachedFile {
                path: "file.py".to_string(),
                mtime: 1000,
                size: 500,
                hash: original_hash,
                tokens: TokenCounts::default(),
                symbols: vec![],
                symbols_extracted: false,
                language: None,
                lines: 0,
            },
        );

        // Same mtime/size/hash - no rescan needed
        assert!(!cache.needs_rescan_with_hash("file.py", 1000, 500, original_hash));

        // Same mtime/size but different hash - rescan needed
        assert!(cache.needs_rescan_with_hash("file.py", 1000, 500, modified_hash));

        // Different mtime - rescan needed regardless of hash
        assert!(cache.needs_rescan_with_hash("file.py", 2000, 500, original_hash));

        // Hash of 0 is ignored (backwards compatibility)
        assert!(!cache.needs_rescan_with_hash("file.py", 1000, 500, 0));
    }
}
