//! Git integration for diff/log analysis
//!
//! Provides integration with Git for:
//! - Getting changed files between commits
//! - Extracting commit history
//! - Blame information for file importance

use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Git repository wrapper
pub struct GitRepo {
    path: String,
}

/// A git commit entry
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub date: String,
    pub message: String,
}

/// A file changed in a commit
#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// Current path (or new path for renames)
    pub path: String,
    /// Original path for renamed/copied files (None for add/modify/delete)
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
}

/// File change status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}

impl FileStatus {
    fn from_char(c: char) -> Self {
        match c {
            'A' => Self::Added,
            'M' => Self::Modified,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            _ => Self::Unknown,
        }
    }
}

/// Blame entry for a line
#[derive(Debug, Clone)]
pub struct BlameLine {
    pub commit: String,
    pub author: String,
    pub date: String,
    pub line_number: u32,
}

/// Type of line change in a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    /// Line was added
    Add,
    /// Line was removed
    Remove,
    /// Context line (unchanged)
    Context,
}

impl DiffLineType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Context => "context",
        }
    }
}

/// A single line change within a diff hunk
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Type of change: add, remove, or context
    pub change_type: DiffLineType,
    /// Line number in the old file (None for additions)
    pub old_line: Option<u32>,
    /// Line number in the new file (None for deletions)
    pub new_line: Option<u32>,
    /// The actual line content (without +/- prefix)
    pub content: String,
}

/// A diff hunk representing a contiguous block of changes
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// File path this hunk belongs to (relative to repo root)
    pub file: String,
    /// Starting line in the old file
    pub old_start: u32,
    /// Number of lines in the old file section
    pub old_count: u32,
    /// Starting line in the new file
    pub new_start: u32,
    /// Number of lines in the new file section
    pub new_count: u32,
    /// Header line (e.g., "@@ -1,5 +1,7 @@ function name")
    pub header: String,
    /// Individual line changes within this hunk
    pub lines: Vec<DiffLine>,
}

/// Git errors
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Not a git repository")]
    NotAGitRepo,
    #[error("Git command failed: {0}")]
    CommandFailed(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl GitRepo {
    /// Open a git repository
    pub fn open(path: &Path) -> Result<Self, GitError> {
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return Err(GitError::NotAGitRepo);
        }

        Ok(Self { path: path.to_string_lossy().to_string() })
    }

    /// Check if path is a git repository
    pub fn is_git_repo(path: &Path) -> bool {
        path.join(".git").exists()
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        let output = self.run_git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        Ok(output.trim().to_owned())
    }

    /// Get current commit hash
    pub fn current_commit(&self) -> Result<String, GitError> {
        let output = self.run_git(&["rev-parse", "HEAD"])?;
        Ok(output.trim().to_owned())
    }

    /// Get short commit hash
    pub fn short_hash(&self, commit: &str) -> Result<String, GitError> {
        let output = self.run_git(&["rev-parse", "--short", commit])?;
        Ok(output.trim().to_owned())
    }

    /// Get files changed between two commits
    pub fn diff_files(&self, from: &str, to: &str) -> Result<Vec<ChangedFile>, GitError> {
        // First get file status with --name-status (shows A/M/D/R/C status)
        let status_output = self.run_git(&["diff", "--name-status", from, to])?;

        // Then get line counts with --numstat (shows additions/deletions)
        let numstat_output = self.run_git(&["diff", "--numstat", from, to])?;

        // Build a map of path -> (additions, deletions) from numstat
        let mut stats: std::collections::HashMap<String, (u32, u32)> =
            std::collections::HashMap::new();
        for line in numstat_output.lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                // numstat format: additions<TAB>deletions<TAB>path
                // Binary files show "-" for additions/deletions
                let add = parts[0].parse::<u32>().unwrap_or(0);
                let del = parts[1].parse::<u32>().unwrap_or(0);
                let path = parts[2..].join("\t");
                stats.insert(path, (add, del));
            }
        }

        let mut files = Vec::new();

        // Parse name-status output
        for line in status_output.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status_str = parts[0];
            let first_char = status_str.chars().next().unwrap_or(' ');
            let status = FileStatus::from_char(first_char);

            // Handle renamed/copied files: R100 or C100 followed by old_path and new_path
            let (path, old_path) = if (first_char == 'R' || first_char == 'C') && parts.len() >= 3 {
                // For renames: parts[1] = old_path, parts[2] = new_path
                (parts[2].to_owned(), Some(parts[1].to_owned()))
            } else if parts.len() >= 2 {
                // For other statuses: parts[1] is the path
                (parts[1].to_owned(), None)
            } else {
                continue;
            };

            // Look up line statistics
            let (additions, deletions) = stats.get(&path).copied().unwrap_or((0, 0));

            files.push(ChangedFile { path, old_path, status, additions, deletions });
        }

        Ok(files)
    }

    /// Get files changed in working tree
    ///
    /// Returns both staged and unstaged changes. For renames, the `old_path`
    /// field contains the original filename.
    pub fn status(&self) -> Result<Vec<ChangedFile>, GitError> {
        let output = self.run_git(&["status", "--porcelain"])?;

        let mut files = Vec::new();

        for line in output.lines() {
            if line.len() < 3 {
                continue;
            }

            // Git status --porcelain format: XY filename
            // X = staged status, Y = unstaged status
            let staged_char = line.chars().next().unwrap_or(' ');
            let unstaged_char = line.chars().nth(1).unwrap_or(' ');
            let path_part = &line[3..];

            // Determine the effective status (prefer staged, then unstaged)
            let (status, status_char) = if staged_char != ' ' && staged_char != '?' {
                // Has staged changes
                (
                    match staged_char {
                        'A' => FileStatus::Added,
                        'M' => FileStatus::Modified,
                        'D' => FileStatus::Deleted,
                        'R' => FileStatus::Renamed,
                        'C' => FileStatus::Copied,
                        _ => FileStatus::Unknown,
                    },
                    staged_char,
                )
            } else {
                // Only unstaged changes
                (
                    match unstaged_char {
                        '?' | 'A' => FileStatus::Added,
                        'M' => FileStatus::Modified,
                        'D' => FileStatus::Deleted,
                        'R' => FileStatus::Renamed,
                        _ => FileStatus::Unknown,
                    },
                    unstaged_char,
                )
            };

            // Handle renames: format is "old_path -> new_path"
            let (path, old_path) = if status_char == 'R' || status_char == 'C' {
                if let Some(arrow_pos) = path_part.find(" -> ") {
                    let old = path_part[..arrow_pos].to_owned();
                    let new = path_part[arrow_pos + 4..].to_owned();
                    (new, Some(old))
                } else {
                    (path_part.to_owned(), None)
                }
            } else {
                (path_part.to_owned(), None)
            };

            files.push(ChangedFile { path, old_path, status, additions: 0, deletions: 0 });
        }

        Ok(files)
    }

    /// Get recent commits
    pub fn log(&self, count: usize) -> Result<Vec<Commit>, GitError> {
        let output = self.run_git(&[
            "log",
            &format!("-{}", count),
            "--format=%H%n%h%n%an%n%ae%n%ad%n%s%n---COMMIT---",
            "--date=short",
        ])?;

        let mut commits = Vec::new();
        let mut lines = output.lines().peekable();

        while lines.peek().is_some() {
            let hash = lines.next().unwrap_or("").to_owned();
            if hash.is_empty() {
                continue;
            }

            let short_hash = lines.next().unwrap_or("").to_owned();
            let author = lines.next().unwrap_or("").to_owned();
            let email = lines.next().unwrap_or("").to_owned();
            let date = lines.next().unwrap_or("").to_owned();
            let message = lines.next().unwrap_or("").to_owned();

            // Skip separator
            while lines.peek().map(|l| *l != "---COMMIT---").unwrap_or(false) {
                lines.next();
            }
            lines.next(); // Skip the separator

            commits.push(Commit { hash, short_hash, author, email, date, message });
        }

        Ok(commits)
    }

    /// Get commits that modified a specific file
    pub fn file_log(&self, path: &str, count: usize) -> Result<Vec<Commit>, GitError> {
        let output = self.run_git(&[
            "log",
            &format!("-{}", count),
            "--format=%H%n%h%n%an%n%ae%n%ad%n%s%n---COMMIT---",
            "--date=short",
            "--follow",
            "--",
            path,
        ])?;

        let mut commits = Vec::new();
        let commit_blocks: Vec<&str> = output.split("---COMMIT---").collect();

        for block in commit_blocks {
            let lines: Vec<&str> = block.lines().filter(|l| !l.is_empty()).collect();
            if lines.len() < 6 {
                continue;
            }

            commits.push(Commit {
                hash: lines[0].to_owned(),
                short_hash: lines[1].to_owned(),
                author: lines[2].to_owned(),
                email: lines[3].to_owned(),
                date: lines[4].to_owned(),
                message: lines[5].to_owned(),
            });
        }

        Ok(commits)
    }

    /// Get blame information for a file
    pub fn blame(&self, path: &str) -> Result<Vec<BlameLine>, GitError> {
        let output = self.run_git(&["blame", "--porcelain", path])?;

        let mut lines = Vec::new();
        let mut current_commit = String::new();
        let mut current_author = String::new();
        let mut current_date = String::new();
        let mut line_number = 0u32;

        for line in output.lines() {
            if line.starts_with('\t') {
                // This is the actual line content, create blame entry
                lines.push(BlameLine {
                    commit: current_commit.clone(),
                    author: current_author.clone(),
                    date: current_date.clone(),
                    line_number,
                });
            } else if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
                // New commit hash line
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    current_commit = parts[0][..8.min(parts[0].len())].to_string();
                    if parts.len() >= 3 {
                        line_number = parts[2].parse().unwrap_or(0);
                    }
                }
            } else if let Some(author) = line.strip_prefix("author ") {
                current_author = author.to_owned();
            } else if let Some(time) = line.strip_prefix("author-time ") {
                // Convert Unix timestamp to date
                if let Ok(ts) = time.parse::<i64>() {
                    current_date = format_timestamp(ts);
                }
            }
        }

        Ok(lines)
    }

    /// Get list of files tracked by git
    pub fn ls_files(&self) -> Result<Vec<String>, GitError> {
        let output = self.run_git(&["ls-files"])?;
        Ok(output.lines().map(String::from).collect())
    }

    /// Get diff content between two commits for a file
    pub fn diff_content(&self, from: &str, to: &str, path: &str) -> Result<String, GitError> {
        self.run_git(&["diff", from, to, "--", path])
    }

    /// Get diff content for uncommitted changes (working tree vs HEAD)
    /// Includes both staged and unstaged changes.
    pub fn uncommitted_diff(&self, path: &str) -> Result<String, GitError> {
        // Get both staged and unstaged changes combined
        self.run_git(&["diff", "HEAD", "--", path])
    }

    /// Get diff content for all uncommitted changes
    /// Returns combined diff for all changed files.
    pub fn all_uncommitted_diffs(&self) -> Result<String, GitError> {
        self.run_git(&["diff", "HEAD"])
    }

    /// Check if a file has uncommitted changes
    pub fn has_changes(&self, path: &str) -> Result<bool, GitError> {
        let output = self.run_git(&["status", "--porcelain", "--", path])?;
        Ok(!output.trim().is_empty())
    }

    /// Get the commit where a file was last modified
    pub fn last_modified_commit(&self, path: &str) -> Result<Commit, GitError> {
        let commits = self.file_log(path, 1)?;
        commits
            .into_iter()
            .next()
            .ok_or_else(|| GitError::ParseError("No commits found".to_owned()))
    }

    /// Calculate file importance based on recent changes
    pub fn file_change_frequency(&self, path: &str, days: u32) -> Result<u32, GitError> {
        let output = self.run_git(&[
            "log",
            &format!("--since={} days ago", days),
            "--oneline",
            "--follow",
            "--",
            path,
        ])?;

        Ok(output.lines().count() as u32)
    }

    /// Get file content at a specific git ref (commit, branch, tag)
    ///
    /// Uses `git show <ref>:<path>` to retrieve file content at that revision.
    ///
    /// # Arguments
    /// * `path` - File path relative to repository root
    /// * `git_ref` - Git ref (commit hash, branch name, tag, HEAD~n, etc.)
    ///
    /// # Returns
    /// File content as string, or error if file doesn't exist at that ref
    ///
    /// # Example
    /// ```ignore
    /// let repo = GitRepo::open(Path::new("."))?;
    /// let content = repo.file_at_ref("src/main.rs", "HEAD~5")?;
    /// ```
    pub fn file_at_ref(&self, path: &str, git_ref: &str) -> Result<String, GitError> {
        self.run_git(&["show", &format!("{}:{}", git_ref, path)])
    }

    /// Parse diff between two refs into structured hunks
    ///
    /// Returns detailed hunk information including line numbers for each change.
    ///
    /// # Arguments
    /// * `from_ref` - Starting ref (e.g., "main", "HEAD~5", commit hash)
    /// * `to_ref` - Ending ref (e.g., "HEAD", "feature-branch")
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Vec of DiffHunk with structured line-level information
    pub fn diff_hunks(
        &self,
        from_ref: &str,
        to_ref: &str,
        path: Option<&str>,
    ) -> Result<Vec<DiffHunk>, GitError> {
        let output = match path {
            Some(p) => self.run_git(&["diff", "-U3", from_ref, to_ref, "--", p])?,
            None => self.run_git(&["diff", "-U3", from_ref, to_ref])?,
        };

        parse_diff_hunks(&output)
    }

    /// Parse uncommitted changes (working tree vs HEAD) into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Vec of DiffHunk for uncommitted changes
    pub fn uncommitted_hunks(&self, path: Option<&str>) -> Result<Vec<DiffHunk>, GitError> {
        let output = match path {
            Some(p) => self.run_git(&["diff", "-U3", "HEAD", "--", p])?,
            None => self.run_git(&["diff", "-U3", "HEAD"])?,
        };

        parse_diff_hunks(&output)
    }

    /// Parse staged changes into structured hunks
    ///
    /// # Arguments
    /// * `path` - Optional file path to filter to a single file
    ///
    /// # Returns
    /// Vec of DiffHunk for staged changes only
    pub fn staged_hunks(&self, path: Option<&str>) -> Result<Vec<DiffHunk>, GitError> {
        let output = match path {
            Some(p) => self.run_git(&["diff", "-U3", "--staged", "--", p])?,
            None => self.run_git(&["diff", "-U3", "--staged"])?,
        };

        parse_diff_hunks(&output)
    }

    /// Run a git command and return output
    fn run_git(&self, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        String::from_utf8(output.stdout).map_err(|e| GitError::ParseError(e.to_string()))
    }
}

/// Format Unix timestamp as YYYY-MM-DD
fn format_timestamp(ts: i64) -> String {
    // Simple formatting without chrono
    let secs_per_day = 86400;
    let days_since_epoch = ts / secs_per_day;

    // Approximate calculation (doesn't account for leap seconds)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days in days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }

    let day = remaining_days + 1;

    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Parse unified diff output into structured hunks
///
/// Handles the standard unified diff format with hunk headers like:
/// `@@ -start,count +start,count @@ optional context`
fn parse_diff_hunks(diff_output: &str) -> Result<Vec<DiffHunk>, GitError> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut current_file = String::new();
    let mut old_line = 0u32;
    let mut new_line = 0u32;

    for line in diff_output.lines() {
        // Reset file tracking when we see a new diff header
        if line.starts_with("diff --git") {
            // Save previous hunk if exists before starting new file
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            current_file = String::new();
            continue;
        }
        // Track file from "--- a/path" lines (old file path)
        if let Some(path) = line.strip_prefix("--- a/") {
            current_file = path.to_owned();
            continue;
        }
        // Track file from "+++ b/path" lines (new file path - prefer this)
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = path.to_owned();
            continue;
        }
        // Handle /dev/null for new or deleted files
        if line.starts_with("--- /dev/null") || line.starts_with("+++ /dev/null") {
            continue;
        }

        // Check for hunk header: @@ -old_start,old_count +new_start,new_count @@ context
        if line.starts_with("@@") {
            // Save previous hunk if exists
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }

            // Parse hunk header
            if let Some((old_start, old_count, new_start, new_count)) = parse_hunk_header(line) {
                old_line = old_start;
                new_line = new_start;

                current_hunk = Some(DiffHunk {
                    file: current_file.clone(),
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                    header: line.to_owned(),
                    lines: Vec::new(),
                });
            }
        } else if let Some(ref mut hunk) = current_hunk {
            // Parse line within a hunk
            if let Some(first_char) = line.chars().next() {
                let (change_type, content) = match first_char {
                    '+' => (DiffLineType::Add, line[1..].to_owned()),
                    '-' => (DiffLineType::Remove, line[1..].to_owned()),
                    ' ' => (DiffLineType::Context, line[1..].to_owned()),
                    '\\' => continue, // "\ No newline at end of file"
                    _ => continue,    // Skip diff headers (diff --git, index, ---, +++)
                };

                let (old_ln, new_ln) = match change_type {
                    DiffLineType::Add => {
                        let nl = new_line;
                        new_line += 1;
                        (None, Some(nl))
                    },
                    DiffLineType::Remove => {
                        let ol = old_line;
                        old_line += 1;
                        (Some(ol), None)
                    },
                    DiffLineType::Context => {
                        let ol = old_line;
                        let nl = new_line;
                        old_line += 1;
                        new_line += 1;
                        (Some(ol), Some(nl))
                    },
                };

                hunk.lines.push(DiffLine {
                    change_type,
                    old_line: old_ln,
                    new_line: new_ln,
                    content,
                });
            }
        }
    }

    // Push final hunk
    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    Ok(hunks)
}

/// Parse a hunk header line into (old_start, old_count, new_start, new_count)
///
/// Format: @@ -old_start,old_count +new_start,new_count @@ optional_context
/// Note: count defaults to 1 if omitted (e.g., @@ -5 +5,2 @@)
fn parse_hunk_header(header: &str) -> Option<(u32, u32, u32, u32)> {
    // Find the range specifications between @@ markers
    let header = header.strip_prefix("@@")?;
    let end_idx = header.find("@@")?;
    let range_part = header[..end_idx].trim();

    let parts: Vec<&str> = range_part.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // Parse old range: -start,count or -start
    let old_part = parts[0].strip_prefix('-')?;
    let (old_start, old_count) = parse_range(old_part)?;

    // Parse new range: +start,count or +start
    let new_part = parts[1].strip_prefix('+')?;
    let (new_start, new_count) = parse_range(new_part)?;

    Some((old_start, old_count, new_start, new_count))
}

/// Parse a range specification like "5,3" or "5" into (start, count)
fn parse_range(range: &str) -> Option<(u32, u32)> {
    if let Some((start_str, count_str)) = range.split_once(',') {
        let start = start_str.parse().ok()?;
        let count = count_str.parse().ok()?;
        Some((start, count))
    } else {
        let start = range.parse().ok()?;
        Some((start, 1)) // Default count is 1
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_test_repo() -> TempDir {
        let temp = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .current_dir(temp.path())
            .args(["init"])
            .output()
            .unwrap();

        // Configure git
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

        // Create a file and commit
        std::fs::write(temp.path().join("test.txt"), "hello").unwrap();

        Command::new("git")
            .current_dir(temp.path())
            .args(["add", "."])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(temp.path())
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        temp
    }

    #[test]
    fn test_open_repo() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path());
        assert!(repo.is_ok());
    }

    #[test]
    fn test_not_a_repo() {
        let temp = TempDir::new().unwrap();
        let repo = GitRepo::open(temp.path());
        assert!(matches!(repo, Err(GitError::NotAGitRepo)));
    }

    #[test]
    fn test_current_branch() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();
        let branch = repo.current_branch().unwrap();
        // Branch could be "main" or "master" depending on git config
        assert!(!branch.is_empty());
    }

    #[test]
    fn test_log() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();
        let commits = repo.log(10).unwrap();
        assert!(!commits.is_empty());
        assert_eq!(commits[0].message, "Initial commit");
    }

    #[test]
    fn test_ls_files() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();
        let files = repo.ls_files().unwrap();
        assert!(files.contains(&"test.txt".to_string()));
    }

    #[test]
    fn test_format_timestamp() {
        // 2024-01-01 00:00:00 UTC
        let ts = 1704067200;
        let date = format_timestamp(ts);
        assert_eq!(date, "2024-01-01");
    }

    #[test]
    fn test_file_at_ref() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();

        // Get file content at HEAD
        let content = repo.file_at_ref("test.txt", "HEAD").unwrap();
        assert_eq!(content.trim(), "hello");

        // Modify the file and commit
        std::fs::write(temp.path().join("test.txt"), "world").unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["commit", "-m", "Update"])
            .output()
            .unwrap();

        // Check current HEAD has new content
        let new_content = repo.file_at_ref("test.txt", "HEAD").unwrap();
        assert_eq!(new_content.trim(), "world");

        // Check HEAD~1 still has old content
        let old_content = repo.file_at_ref("test.txt", "HEAD~1").unwrap();
        assert_eq!(old_content.trim(), "hello");
    }

    #[test]
    fn test_parse_hunk_header() {
        // Standard case
        let result = parse_hunk_header("@@ -1,5 +1,7 @@ fn main()");
        assert_eq!(result, Some((1, 5, 1, 7)));

        // No count (defaults to 1)
        let result = parse_hunk_header("@@ -1 +1 @@");
        assert_eq!(result, Some((1, 1, 1, 1)));

        // Mixed
        let result = parse_hunk_header("@@ -10,3 +15 @@");
        assert_eq!(result, Some((10, 3, 15, 1)));

        // Invalid
        let result = parse_hunk_header("not a header");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_diff_hunks() {
        let diff = r#"diff --git a/test.txt b/test.txt
index abc123..def456 100644
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
-old line 2
+new line 2
+added line
 line 3
"#;

        let hunks = parse_diff_hunks(diff).unwrap();
        assert_eq!(hunks.len(), 1);

        let hunk = &hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 4);
        assert_eq!(hunk.lines.len(), 5);

        // Check line types
        assert_eq!(hunk.lines[0].change_type, DiffLineType::Context);
        assert_eq!(hunk.lines[1].change_type, DiffLineType::Remove);
        assert_eq!(hunk.lines[2].change_type, DiffLineType::Add);
        assert_eq!(hunk.lines[3].change_type, DiffLineType::Add);
        assert_eq!(hunk.lines[4].change_type, DiffLineType::Context);

        // Check line numbers
        assert_eq!(hunk.lines[0].old_line, Some(1));
        assert_eq!(hunk.lines[0].new_line, Some(1));
        assert_eq!(hunk.lines[1].old_line, Some(2));
        assert_eq!(hunk.lines[1].new_line, None);
        assert_eq!(hunk.lines[2].old_line, None);
        assert_eq!(hunk.lines[2].new_line, Some(2));
    }

    #[test]
    fn test_diff_hunks() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();

        // Modify file and commit
        std::fs::write(temp.path().join("test.txt"), "hello\nworld\n").unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp.path())
            .args(["commit", "-m", "Add world"])
            .output()
            .unwrap();

        // Get hunks between commits
        let hunks = repo.diff_hunks("HEAD~1", "HEAD", Some("test.txt")).unwrap();
        assert!(!hunks.is_empty());

        // Verify we got structured data
        let hunk = &hunks[0];
        assert!(hunk.old_start > 0);
        assert!(!hunk.header.is_empty());
    }

    #[test]
    fn test_uncommitted_hunks() {
        let temp = init_test_repo();
        let repo = GitRepo::open(temp.path()).unwrap();

        // Make uncommitted change
        std::fs::write(temp.path().join("test.txt"), "modified content").unwrap();

        let hunks = repo.uncommitted_hunks(Some("test.txt")).unwrap();
        assert!(!hunks.is_empty());

        // Should have some changes
        let total_changes: usize = hunks.iter().map(|h| h.lines.len()).sum();
        assert!(total_changes > 0);
    }
}
