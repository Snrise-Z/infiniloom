//! Git operations and repository wrapper
//!
//! This module provides a comprehensive Git API for Node.js, including:
//! - Repository status and diffs
//! - Commit history and blame
//! - File tracking and change detection
//! - Structured diff parsing with hunks

use crate::types::{GitBlameLine, GitChangedFile, GitCommit, GitDiffHunk, GitDiffLine, GitFileStatus};
use crate::utils::format_file_status;
use crate::validation::validate_path_option;
use infiniloom_engine::git::{DiffHunk as EngineGitDiffHunk, GitRepo as EngineGitRepo};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::path::PathBuf;

/// Check if a path is a git repository
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// True if path is a git repository, false otherwise
///
/// # Example
/// ```javascript
/// const { isGitRepo } = require('infiniloom-node');
///
/// if (isGitRepo('./my-project')) {
///   console.log('This is a git repository');
/// }
/// ```
#[napi]
pub fn is_git_repo(path: String) -> bool {
    let path_buf = PathBuf::from(path);
    EngineGitRepo::is_git_repo(&path_buf)
}

/// Git repository wrapper for Node.js
///
/// Provides access to git operations like status, diff, log, and blame.
///
/// # Example
/// ```javascript
/// const { GitRepo } = require('infiniloom-node');
///
/// const repo = new GitRepo('./my-project');
/// console.log(`Branch: ${repo.currentBranch()}`);
/// console.log(`Commit: ${repo.currentCommit()}`);
///
/// for (const file of repo.status()) {
///   console.log(`${file.status}: ${file.path}`);
/// }
/// ```
#[napi]
pub struct GitRepo {
    inner: EngineGitRepo,
}

#[napi]
impl GitRepo {
    /// Open a git repository
    ///
    /// # Arguments
    /// * `path` - Path to the repository
    ///
    /// # Throws
    /// Error if path is null/undefined or not a git repository
    #[napi(constructor)]
    pub fn new(path: Option<String>) -> Result<Self> {
        let path = validate_path_option(path.as_deref())?;
        let path_buf = PathBuf::from(path);
        let inner = EngineGitRepo::open(&path_buf).map_err(|e| {
            Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e))
        })?;
        Ok(GitRepo { inner })
    }

    /// Get the current branch name
    ///
    /// # Returns
    /// Current branch name (e.g., "main", "feature/xyz")
    #[napi]
    pub fn current_branch(&self) -> Result<String> {
        self.inner
            .current_branch()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get the current commit hash
    ///
    /// # Returns
    /// Full SHA-1 hash of HEAD commit
    #[napi]
    pub fn current_commit(&self) -> Result<String> {
        self.inner
            .current_commit()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get working tree status
    ///
    /// Returns both staged and unstaged changes.
    ///
    /// # Returns
    /// Array of file status objects
    #[napi]
    pub fn status(&self) -> Result<Vec<GitFileStatus>> {
        let files = self
            .inner
            .status()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(files
            .iter()
            .map(|f| GitFileStatus {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: format_file_status(f.status),
            })
            .collect())
    }

    /// Get files changed between two commits
    ///
    /// # Arguments
    /// * `from_ref` - Starting commit/branch/tag
    /// * `to_ref` - Ending commit/branch/tag
    ///
    /// # Returns
    /// Array of changed files with diff stats
    #[napi]
    pub fn diff_files(&self, from_ref: String, to_ref: String) -> Result<Vec<GitChangedFile>> {
        let files = self
            .inner
            .diff_files(&from_ref, &to_ref)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(files
            .iter()
            .map(|f| GitChangedFile {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: format_file_status(f.status),
                additions: f.additions,
                deletions: f.deletions,
            })
            .collect())
    }

    /// Get recent commits
    ///
    /// # Arguments
    /// * `count` - Maximum number of commits to return (default: 10)
    ///
    /// # Returns
    /// Array of commit objects
    #[napi]
    pub fn log(&self, count: Option<u32>) -> Result<Vec<GitCommit>> {
        let commits = self
            .inner
            .log(count.unwrap_or(10) as usize)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(commits
            .iter()
            .map(|c| GitCommit {
                hash: c.hash.clone(),
                short_hash: c.short_hash.clone(),
                author: c.author.clone(),
                email: c.email.clone(),
                date: c.date.clone(),
                message: c.message.clone(),
            })
            .collect())
    }

    /// Get commits that modified a specific file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `count` - Maximum number of commits to return (default: 10)
    ///
    /// # Returns
    /// Array of commits that modified the file
    #[napi]
    pub fn file_log(&self, path: String, count: Option<u32>) -> Result<Vec<GitCommit>> {
        let commits = self
            .inner
            .file_log(&path, count.unwrap_or(10) as usize)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(commits
            .iter()
            .map(|c| GitCommit {
                hash: c.hash.clone(),
                short_hash: c.short_hash.clone(),
                author: c.author.clone(),
                email: c.email.clone(),
                date: c.date.clone(),
                message: c.message.clone(),
            })
            .collect())
    }

    /// Get blame information for a file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Array of blame line objects
    #[napi]
    pub fn blame(&self, path: String) -> Result<Vec<GitBlameLine>> {
        let lines = self
            .inner
            .blame(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(lines
            .iter()
            .map(|l| GitBlameLine {
                commit: l.commit.clone(),
                author: l.author.clone(),
                date: l.date.clone(),
                line_number: l.line_number,
            })
            .collect())
    }

    /// Get list of files tracked by git
    ///
    /// # Returns
    /// Array of file paths tracked by git
    #[napi]
    pub fn ls_files(&self) -> Result<Vec<String>> {
        self.inner
            .ls_files()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff content between two commits for a file
    ///
    /// # Arguments
    /// * `from_ref` - Starting commit/branch/tag
    /// * `to_ref` - Ending commit/branch/tag
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn diff_content(&self, from_ref: String, to_ref: String, path: String) -> Result<String> {
        self.inner
            .diff_content(&from_ref, &to_ref, &path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff content for uncommitted changes in a file
    ///
    /// Includes both staged and unstaged changes compared to HEAD.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn uncommitted_diff(&self, path: String) -> Result<String> {
        self.inner
            .uncommitted_diff(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get diff for all uncommitted changes
    ///
    /// Returns combined diff for all changed files.
    ///
    /// # Returns
    /// Unified diff content as string
    #[napi]
    pub fn all_uncommitted_diffs(&self) -> Result<String> {
        self.inner
            .all_uncommitted_diffs()
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Check if a file has uncommitted changes
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// True if file has changes, false otherwise
    #[napi]
    pub fn has_changes(&self, path: String) -> Result<bool> {
        self.inner
            .has_changes(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get the last commit that modified a file
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    ///
    /// # Returns
    /// Commit information object
    #[napi]
    pub fn last_modified_commit(&self, path: String) -> Result<GitCommit> {
        let commit = self
            .inner
            .last_modified_commit(&path)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(GitCommit {
            hash: commit.hash,
            short_hash: commit.short_hash,
            author: commit.author,
            email: commit.email,
            date: commit.date,
            message: commit.message,
        })
    }

    /// Get file change frequency in recent days
    ///
    /// Useful for determining file importance based on recent activity.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `days` - Number of days to look back (default: 30)
    ///
    /// # Returns
    /// Number of commits that modified the file in the period
    #[napi]
    pub fn file_change_frequency(&self, path: String, days: Option<u32>) -> Result<u32> {
        self.inner
            .file_change_frequency(&path, days.unwrap_or(30))
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Get file content at a specific git ref (commit, branch, tag)
    ///
    /// Uses `git show <ref>:<path>` to retrieve file content at that revision.
    ///
    /// # Arguments
    /// * `path` - File path (relative to repo root)
    /// * `git_ref` - Git ref (commit hash, branch name, tag, HEAD~n, etc.)
    ///
    /// # Returns
    /// File content as string
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const oldVersion = repo.fileAtRef('src/main.ts', 'HEAD~5');
    /// const mainVersion = repo.fileAtRef('src/main.ts', 'main');
    /// ```
    #[napi]
    pub fn file_at_ref(&self, path: String, git_ref: String) -> Result<String> {
        self.inner
            .file_at_ref(&path, &git_ref)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Parse diff between two refs into structured hunks
    ///
    /// Returns detailed hunk information including line numbers for each change.
    /// Useful for PR review tools that need to post comments at specific lines.
    ///
    /// # Arguments
    /// * `from_ref` - Starting ref (e.g., "main", "HEAD~5", commit hash)
    /// * `to_ref` - Ending ref (e.g., "HEAD", "feature-branch")
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks with line-level information
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.diffHunks('main', 'HEAD', 'src/index.ts');
    /// for (const hunk of hunks) {
    ///   console.log(`Hunk at old:${hunk.oldStart} new:${hunk.newStart}`);
    ///   for (const line of hunk.lines) {
    ///     console.log(`${line.changeType}: ${line.content}`);
    ///   }
    /// }
    /// ```
    #[napi]
    pub fn diff_hunks(
        &self,
        from_ref: String,
        to_ref: String,
        path: Option<String>,
    ) -> Result<Vec<GitDiffHunk>> {
        let hunks = self
            .inner
            .diff_hunks(&from_ref, &to_ref, path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }

    /// Parse uncommitted changes (working tree vs HEAD) into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks for uncommitted changes
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.uncommittedHunks('src/index.ts');
    /// console.log(`${hunks.length} hunks with uncommitted changes`);
    /// ```
    #[napi]
    pub fn uncommitted_hunks(&self, path: Option<String>) -> Result<Vec<GitDiffHunk>> {
        let hunks = self
            .inner
            .uncommitted_hunks(path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }

    /// Parse staged changes into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Array of diff hunks for staged changes only
    ///
    /// # Example
    /// ```javascript
    /// const { GitRepo } = require('infiniloom-node');
    ///
    /// const repo = new GitRepo('./my-project');
    /// const hunks = repo.stagedHunks('src/index.ts');
    /// console.log(`${hunks.length} hunks staged for commit`);
    /// ```
    #[napi]
    pub fn staged_hunks(&self, path: Option<String>) -> Result<Vec<GitDiffHunk>> {
        let hunks = self
            .inner
            .staged_hunks(path.as_deref())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(hunks.into_iter().map(convert_hunk).collect())
    }
}

/// Convert engine DiffHunk to JS GitDiffHunk
fn convert_hunk(hunk: EngineGitDiffHunk) -> GitDiffHunk {
    GitDiffHunk {
        old_start: hunk.old_start,
        old_count: hunk.old_count,
        new_start: hunk.new_start,
        new_count: hunk.new_count,
        header: hunk.header,
        lines: hunk
            .lines
            .into_iter()
            .map(|l| GitDiffLine {
                change_type: l.change_type.as_str().to_owned(),
                old_line: l.old_line,
                new_line: l.new_line,
                content: l.content,
            })
            .collect(),
    }
}
