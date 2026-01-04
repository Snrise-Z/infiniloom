//! Batch embedding API for processing multiple repositories
//!
//! This module provides APIs for embedding multiple repositories in a single
//! operation, with parallel processing and unified output.
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::{BatchEmbedder, BatchRepoConfig, EmbedSettings};
//!
//! let embedder = BatchEmbedder::new(EmbedSettings::default());
//!
//! let repos = vec![
//!     BatchRepoConfig::new("/path/to/repo1")
//!         .with_namespace("github.com/org")
//!         .with_name("auth-service"),
//!     BatchRepoConfig::new("/path/to/repo2")
//!         .with_namespace("github.com/org")
//!         .with_name("user-service"),
//! ];
//!
//! let result = embedder.embed_batch(&repos)?;
//! println!("Total chunks: {}", result.total_chunks);
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::chunker::EmbedChunker;
use super::error::EmbedError;
use super::progress::{ProgressReporter, QuietProgress};
use super::types::{EmbedChunk, EmbedSettings, RepoIdentifier};
use super::ResourceLimits;

/// Configuration for a single repository in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRepoConfig {
    /// Path to the repository (local path or URL for remote)
    pub path: PathBuf,

    /// Optional namespace override (e.g., "github.com/org")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Optional repository name override (defaults to directory name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional version tag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Optional branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,

    /// Override include patterns for this repo only
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_patterns: Vec<String>,

    /// Override exclude patterns for this repo only
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_patterns: Vec<String>,
}

impl BatchRepoConfig {
    /// Create a new batch repo config from a path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            namespace: None,
            name: None,
            version: None,
            branch: None,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    /// Set the namespace
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set the repository name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the version tag
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the branch name
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Set include patterns (overrides global settings for this repo)
    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }

    /// Set exclude patterns (overrides global settings for this repo)
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Build the RepoIdentifier from this config
    pub fn to_repo_id(&self) -> RepoIdentifier {
        let name = self
            .name
            .clone()
            .or_else(|| {
                self.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "unknown".to_owned());

        RepoIdentifier {
            namespace: self.namespace.clone().unwrap_or_default(),
            name,
            version: self.version.clone(),
            branch: self.branch.clone(),
            commit: None, // Would need git integration to get this
        }
    }
}

/// Result for a single repository in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRepoResult {
    /// Repository identifier
    pub repo_id: RepoIdentifier,

    /// Path that was processed
    pub path: PathBuf,

    /// Generated chunks
    pub chunks: Vec<EmbedChunk>,

    /// Processing time for this repository
    pub elapsed: Duration,

    /// Error if processing failed (chunks will be empty)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary of a batch embedding operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    /// Results per repository
    pub repos: Vec<BatchRepoResult>,

    /// Total chunks across all repositories
    pub total_chunks: usize,

    /// Total tokens across all chunks
    pub total_tokens: u64,

    /// Number of successfully processed repositories
    pub successful_repos: usize,

    /// Number of failed repositories
    pub failed_repos: usize,

    /// Total elapsed time
    pub elapsed: Duration,
}

impl BatchResult {
    /// Get all chunks from all successful repositories
    pub fn all_chunks(&self) -> impl Iterator<Item = &EmbedChunk> {
        self.repos.iter().flat_map(|r| r.chunks.iter())
    }

    /// Get all chunks as owned vector
    pub fn into_chunks(self) -> Vec<EmbedChunk> {
        self.repos.into_iter().flat_map(|r| r.chunks).collect()
    }

    /// Check if there were any failures
    pub fn has_failures(&self) -> bool {
        self.failed_repos > 0
    }

    /// Get failed repository paths and errors
    pub fn failures(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.repos
            .iter()
            .filter(|r| r.error.is_some())
            .map(|r| (r.path.as_path(), r.error.as_deref().unwrap_or("unknown")))
    }
}

/// Batch embedder for processing multiple repositories
pub struct BatchEmbedder {
    settings: EmbedSettings,
    limits: ResourceLimits,
    /// Maximum number of parallel repository processing
    max_parallel: usize,
}

impl BatchEmbedder {
    /// Create a new batch embedder with default limits
    pub fn new(settings: EmbedSettings) -> Self {
        Self {
            settings,
            limits: ResourceLimits::default(),
            max_parallel: std::thread::available_parallelism().map_or(4, |n| n.get()),
        }
    }

    /// Create with custom resource limits
    pub fn with_limits(settings: EmbedSettings, limits: ResourceLimits) -> Self {
        Self {
            settings,
            limits,
            max_parallel: std::thread::available_parallelism().map_or(4, |n| n.get()),
        }
    }

    /// Set maximum parallel repository processing
    pub fn with_max_parallel(mut self, max: usize) -> Self {
        self.max_parallel = max.max(1);
        self
    }

    /// Process a batch of repositories
    ///
    /// Repositories are processed in parallel up to `max_parallel` at a time.
    /// Each repository gets its own chunker instance for thread safety.
    pub fn embed_batch(&self, repos: &[BatchRepoConfig]) -> Result<BatchResult, EmbedError> {
        self.embed_batch_with_progress(repos, &QuietProgress)
    }

    /// Process a batch of repositories with progress reporting
    pub fn embed_batch_with_progress(
        &self,
        repos: &[BatchRepoConfig],
        progress: &dyn ProgressReporter,
    ) -> Result<BatchResult, EmbedError> {
        let start = Instant::now();

        if repos.is_empty() {
            return Ok(BatchResult {
                repos: Vec::new(),
                total_chunks: 0,
                total_tokens: 0,
                successful_repos: 0,
                failed_repos: 0,
                elapsed: start.elapsed(),
            });
        }

        progress.set_phase(&format!("Processing {} repositories...", repos.len()));
        progress.set_total(repos.len());

        let processed = AtomicUsize::new(0);

        // Configure rayon for controlled parallelism
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_parallel)
            .build()
            .map_err(|e| EmbedError::SerializationError {
                reason: format!("Failed to create thread pool: {}", e),
            })?;

        let results: Vec<BatchRepoResult> = pool.install(|| {
            repos
                .par_iter()
                .map(|config| {
                    let result = self.process_single_repo(config);

                    let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
                    progress.set_progress(done);

                    result
                })
                .collect()
        });

        // Calculate totals
        let total_chunks: usize = results.iter().map(|r| r.chunks.len()).sum();
        let total_tokens: u64 = results
            .iter()
            .flat_map(|r| r.chunks.iter())
            .map(|c| c.tokens as u64)
            .sum();
        let successful_repos = results.iter().filter(|r| r.error.is_none()).count();
        let failed_repos = results.iter().filter(|r| r.error.is_some()).count();

        progress.set_phase("Batch complete");

        Ok(BatchResult {
            repos: results,
            total_chunks,
            total_tokens,
            successful_repos,
            failed_repos,
            elapsed: start.elapsed(),
        })
    }

    /// Process a single repository
    fn process_single_repo(&self, config: &BatchRepoConfig) -> BatchRepoResult {
        let start = Instant::now();
        let repo_id = config.to_repo_id();

        // Build settings with per-repo overrides
        let mut settings = self.settings.clone();
        if !config.include_patterns.is_empty() {
            settings.include_patterns = config.include_patterns.clone();
        }
        if !config.exclude_patterns.is_empty() {
            settings.exclude_patterns = config.exclude_patterns.clone();
        }

        // Create chunker for this repo
        let chunker =
            EmbedChunker::new(settings, self.limits.clone()).with_repo_id(repo_id.clone());

        // Process the repository
        let quiet = QuietProgress;
        match chunker.chunk_repository(&config.path, &quiet) {
            Ok(chunks) => BatchRepoResult {
                repo_id,
                path: config.path.clone(),
                chunks,
                elapsed: start.elapsed(),
                error: None,
            },
            Err(e) => BatchRepoResult {
                repo_id,
                path: config.path.clone(),
                chunks: Vec::new(),
                elapsed: start.elapsed(),
                error: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_batch_repo_config_builder() {
        let config = BatchRepoConfig::new("/path/to/repo")
            .with_namespace("github.com/org")
            .with_name("my-repo")
            .with_version("v1.0.0")
            .with_branch("main");

        assert_eq!(config.path, PathBuf::from("/path/to/repo"));
        assert_eq!(config.namespace.as_deref(), Some("github.com/org"));
        assert_eq!(config.name.as_deref(), Some("my-repo"));
        assert_eq!(config.version.as_deref(), Some("v1.0.0"));
        assert_eq!(config.branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_batch_repo_config_to_repo_id() {
        let config = BatchRepoConfig::new("/path/to/my-repo")
            .with_namespace("github.com/org")
            .with_name("custom-name");

        let repo_id = config.to_repo_id();
        assert_eq!(repo_id.namespace, "github.com/org");
        assert_eq!(repo_id.name, "custom-name");
    }

    #[test]
    fn test_batch_repo_config_infer_name() {
        let config = BatchRepoConfig::new("/path/to/my-repo");

        let repo_id = config.to_repo_id();
        assert_eq!(repo_id.name, "my-repo");
    }

    #[test]
    fn test_batch_embedder_empty_batch() {
        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&[]).unwrap();

        assert_eq!(result.total_chunks, 0);
        assert_eq!(result.successful_repos, 0);
        assert_eq!(result.failed_repos, 0);
        assert!(!result.has_failures());
    }

    #[test]
    fn test_batch_embedder_single_repo() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "lib.rs",
            "/// A test function\npub fn hello() { println!(\"hello\"); }\n",
        );

        let repos = vec![BatchRepoConfig::new(temp_dir.path()).with_name("test-repo")];

        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&repos).unwrap();

        assert_eq!(result.successful_repos, 1);
        assert_eq!(result.failed_repos, 0);
        assert!(!result.has_failures());
        assert!(result.total_chunks > 0);
    }

    #[test]
    fn test_batch_embedder_multiple_repos() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        create_test_file(temp_dir1.path(), "a.rs", "pub fn foo() { println!(\"foo\"); }\n");
        create_test_file(temp_dir2.path(), "b.rs", "pub fn bar() { println!(\"bar\"); }\n");

        let repos = vec![
            BatchRepoConfig::new(temp_dir1.path())
                .with_namespace("org")
                .with_name("repo1"),
            BatchRepoConfig::new(temp_dir2.path())
                .with_namespace("org")
                .with_name("repo2"),
        ];

        let embedder = BatchEmbedder::new(EmbedSettings::default()).with_max_parallel(2);
        let result = embedder.embed_batch(&repos).unwrap();

        assert_eq!(result.successful_repos, 2);
        assert_eq!(result.failed_repos, 0);

        // Verify repo IDs are different
        let repo_ids: Vec<_> = result.repos.iter().map(|r| &r.repo_id).collect();
        assert_ne!(repo_ids[0].name, repo_ids[1].name);
    }

    #[test]
    fn test_batch_embedder_handles_failure() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "lib.rs", "pub fn ok() {}\n");

        let repos = vec![
            BatchRepoConfig::new(temp_dir.path()).with_name("good-repo"),
            BatchRepoConfig::new("/nonexistent/path").with_name("bad-repo"),
        ];

        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&repos).unwrap();

        assert_eq!(result.successful_repos, 1);
        assert_eq!(result.failed_repos, 1);
        assert!(result.has_failures());

        let failures: Vec<_> = result.failures().collect();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].0.to_str().unwrap().contains("nonexistent"));
    }

    #[test]
    fn test_batch_result_all_chunks() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        create_test_file(temp_dir1.path(), "a.rs", "pub fn func1() {}\npub fn func2() {}\n");
        create_test_file(temp_dir2.path(), "b.rs", "pub fn func3() {}\n");

        let repos = vec![
            BatchRepoConfig::new(temp_dir1.path()).with_name("repo1"),
            BatchRepoConfig::new(temp_dir2.path()).with_name("repo2"),
        ];

        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&repos).unwrap();

        // Verify all_chunks returns chunks from all repos
        let all_chunks: Vec<_> = result.all_chunks().collect();
        assert_eq!(all_chunks.len(), result.total_chunks);
    }

    #[test]
    fn test_batch_result_into_chunks() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "lib.rs", "pub fn test() {}\n");

        let repos = vec![BatchRepoConfig::new(temp_dir.path())];

        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&repos).unwrap();

        let total = result.total_chunks;
        let chunks = result.into_chunks();
        assert_eq!(chunks.len(), total);
    }

    #[test]
    fn test_per_repo_pattern_override() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "src/lib.rs", "pub fn included() {}\n");
        create_test_file(temp_dir.path(), "tests/test.rs", "pub fn excluded() {}\n");

        // Test with include pattern override
        let repos = vec![BatchRepoConfig::new(temp_dir.path())
            .with_name("filtered-repo")
            .with_include_patterns(vec!["src/**/*.rs".to_owned()])];

        let embedder = BatchEmbedder::new(EmbedSettings::default());
        let result = embedder.embed_batch(&repos).unwrap();

        // Should only have chunks from src/
        for chunk in result.all_chunks() {
            assert!(
                chunk.source.file.starts_with("src/"),
                "Expected src/ prefix, got: {}",
                chunk.source.file
            );
        }
    }
}
