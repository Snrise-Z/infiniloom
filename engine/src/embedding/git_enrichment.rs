//! Git metadata enrichment for embedding chunks
//!
//! Collects per-file git metadata (change frequency, authors, last modified)
//! and caches it so multiple chunks from the same file share one git query.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::GitMetadata;

/// Collects and caches git metadata per file for embedding enrichment.
///
/// Each file is queried at most once; subsequent lookups return the cached result.
pub struct GitMetadataCollector {
    /// Repository root path (where .git lives)
    repo_root: PathBuf,
    /// Per-file cache: relative path -> metadata
    cache: HashMap<String, GitMetadata>,
}

impl GitMetadataCollector {
    /// Create a new collector for the given repository root.
    ///
    /// Returns `None` if the path is not a git repository.
    pub fn new(repo_root: &Path) -> Option<Self> {
        if !repo_root.join(".git").exists() {
            return None;
        }
        Some(Self { repo_root: repo_root.to_path_buf(), cache: HashMap::new() })
    }

    /// Get git metadata for a file, using cache if available.
    ///
    /// `relative_path` should be the file path relative to the repo root.
    pub fn get_metadata(&mut self, relative_path: &str) -> GitMetadata {
        if let Some(cached) = self.cache.get(relative_path) {
            return cached.clone();
        }

        let metadata = self.collect_file_metadata(relative_path);
        self.cache
            .insert(relative_path.to_owned(), metadata.clone());
        metadata
    }

    /// Collect all git metadata for a single file.
    fn collect_file_metadata(&self, relative_path: &str) -> GitMetadata {
        let last_modified = self.get_last_modified(relative_path);
        let total_commits = self.get_total_commits(relative_path);
        let change_frequency = self.get_change_frequency(relative_path, 90);
        let authors = self.get_authors(relative_path);
        let age_days = self.get_age_days(relative_path);

        GitMetadata { last_modified, change_frequency, total_commits, authors, age_days }
    }

    /// Get the ISO 8601 date of the most recent commit touching this file.
    fn get_last_modified(&self, relative_path: &str) -> Option<String> {
        let output = self.run_git(&["log", "-1", "--format=%aI", "--", relative_path])?;
        let trimmed = output.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    }

    /// Get total number of commits ever touching this file.
    fn get_total_commits(&self, relative_path: &str) -> Option<u32> {
        let output = self.run_git(&["log", "--oneline", "--follow", "--", relative_path])?;
        let count = output.lines().filter(|l| !l.is_empty()).count();
        Some(count as u32)
    }

    /// Get number of commits touching this file within the lookback period.
    fn get_change_frequency(&self, relative_path: &str, days: u32) -> Option<u32> {
        let since_arg = format!("--since={} days ago", days);
        let output =
            self.run_git(&["log", &since_arg, "--oneline", "--follow", "--", relative_path])?;
        let count = output.lines().filter(|l| !l.is_empty()).count();
        Some(count as u32)
    }

    /// Get sorted, deduplicated list of authors who have touched this file.
    fn get_authors(&self, relative_path: &str) -> Vec<String> {
        let output = match self.run_git(&["log", "--format=%an", "--", relative_path]) {
            Some(o) => o,
            None => return Vec::new(),
        };

        let mut authors: Vec<String> = output
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_owned())
            .collect();

        authors.sort();
        authors.dedup();
        authors
    }

    /// Compute age in days since the first commit touching this file.
    fn get_age_days(&self, relative_path: &str) -> Option<u32> {
        // Get the date of the first (oldest) commit
        let output = self.run_git(&["log", "--reverse", "--format=%at", "--", relative_path])?;

        let first_line = output.lines().find(|l| !l.is_empty())?;
        let first_timestamp: i64 = first_line.trim().parse().ok()?;

        // Get current time as Unix timestamp
        let now_output = self.run_git(&["log", "-1", "--format=%at", "HEAD"])?;
        let now_timestamp: i64 = now_output.trim().parse().ok()?;

        let diff_secs = now_timestamp.saturating_sub(first_timestamp);
        let diff_days = diff_secs / 86400;
        Some(diff_days as u32)
    }

    /// Run a git command and return stdout on success, None on failure.
    fn run_git(&self, args: &[&str]) -> Option<String> {
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .args(args)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        String::from_utf8(output.stdout).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_metadata_default() {
        let meta = GitMetadata::default();
        assert!(meta.last_modified.is_none());
        assert!(meta.change_frequency.is_none());
        assert!(meta.total_commits.is_none());
        assert!(meta.authors.is_empty());
        assert!(meta.age_days.is_none());
    }

    #[test]
    fn test_git_metadata_serialization() {
        let meta = GitMetadata {
            last_modified: Some("2025-01-15T10:30:00+00:00".to_owned()),
            change_frequency: Some(5),
            total_commits: Some(42),
            authors: vec!["Alice".to_owned(), "Bob".to_owned()],
            age_days: Some(365),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: GitMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, deserialized);
    }

    #[test]
    fn test_git_metadata_skips_empty_fields() {
        let meta = GitMetadata::default();
        let json = serde_json::to_string(&meta).unwrap();
        // All fields should be skipped when empty/None
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_git_metadata_deserialize_missing_fields() {
        // Ensure backward compatibility: missing fields deserialize to defaults
        let json = r#"{"last_modified": "2025-01-01"}"#;
        let meta: GitMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.last_modified, Some("2025-01-01".to_owned()));
        assert!(meta.change_frequency.is_none());
        assert!(meta.authors.is_empty());
    }

    #[test]
    fn test_collector_not_a_git_repo() {
        let temp = tempfile::TempDir::new().unwrap();
        let collector = GitMetadataCollector::new(temp.path());
        assert!(collector.is_none());
    }

    #[test]
    fn test_collector_caching() {
        let temp = tempfile::TempDir::new().unwrap();

        // Initialize a git repo
        Command::new("git")
            .current_dir(temp.path())
            .args(["init"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["config", "user.name", "TestAuthor"])
            .output()
            .unwrap();

        std::fs::write(temp.path().join("hello.rs"), "fn main() {}").unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();

        let mut collector = GitMetadataCollector::new(temp.path()).unwrap();

        let meta1 = collector.get_metadata("hello.rs");
        assert!(meta1.last_modified.is_some());
        assert_eq!(meta1.total_commits, Some(1));
        assert_eq!(meta1.change_frequency, Some(1));
        assert_eq!(meta1.authors, vec!["TestAuthor".to_owned()]);
        assert!(meta1.age_days.is_some());

        // Second call should return cached result
        let meta2 = collector.get_metadata("hello.rs");
        assert_eq!(meta1, meta2);
    }

    #[test]
    fn test_collector_untracked_file() {
        let temp = tempfile::TempDir::new().unwrap();

        // Initialize a git repo with one commit
        Command::new("git")
            .current_dir(temp.path())
            .args(["init"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();
        std::fs::write(temp.path().join("tracked.rs"), "fn main() {}").unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();

        let mut collector = GitMetadataCollector::new(temp.path()).unwrap();

        // Query an untracked file -- should return empty metadata gracefully
        let meta = collector.get_metadata("nonexistent.rs");
        assert!(meta.last_modified.is_none());
        assert_eq!(meta.total_commits, Some(0));
        assert!(meta.authors.is_empty());
    }
}
