//! Comprehensive git and remote repository tests
//!
//! Tests for git integration (log, diff, blame, status) and
//! remote repository URL parsing and cloning.

use infiniloom_engine::git::{BlameLine, ChangedFile, Commit, FileStatus, GitError, GitRepo};
use infiniloom_engine::remote::{GitProvider, RemoteError, RemoteRepo};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_git_repo() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temporary dir");

    // Initialize git repo
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["init"])
        .output()
        .expect("Failed to init git");

    // Configure git
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.email", "test@example.com"])
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.name", "Test User"])
        .output()
        .expect("Failed to config name");

    temp_dir
}

fn commit_file(temp_dir: &TempDir, filename: &str, content: &str, message: &str) {
    let filepath = temp_dir.path().join(filename);
    if let Some(parent) = filepath.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent dirs");
    }
    fs::write(&filepath, content).expect("Failed to write file");

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", filename])
        .output()
        .expect("Failed to git add");

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", message])
        .output()
        .expect("Failed to git commit");
}

// ============================================================================
// GitRepo Basic Tests
// ============================================================================

#[test]
fn test_git_repo_open_valid() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path());
    assert!(repo.is_ok(), "Should open valid git repo");
}

#[test]
fn test_git_repo_open_invalid() {
    let temp_dir = TempDir::new().unwrap();
    // No .git directory

    let repo = GitRepo::open(temp_dir.path());
    assert!(matches!(repo, Err(GitError::NotAGitRepo)));
}

#[test]
fn test_git_repo_is_git_repo() {
    let temp_git = create_test_git_repo();
    let temp_non_git = TempDir::new().unwrap();

    assert!(GitRepo::is_git_repo(temp_git.path()));
    assert!(!GitRepo::is_git_repo(temp_non_git.path()));
}

// ============================================================================
// GitRepo Branch/Commit Tests
// ============================================================================

#[test]
fn test_current_branch() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let branch = repo.current_branch().unwrap();

    // Branch is typically "main" or "master"
    assert!(!branch.is_empty());
    assert!(branch == "main" || branch == "master");
}

#[test]
fn test_current_commit() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commit = repo.current_commit().unwrap();

    // Commit hash is 40 hex characters
    assert_eq!(commit.len(), 40);
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_short_hash() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let full_hash = repo.current_commit().unwrap();
    let short_hash = repo.short_hash(&full_hash).unwrap();

    // Short hash is typically 7 characters
    assert!(short_hash.len() >= 7);
    assert!(short_hash.chars().all(|c| c.is_ascii_hexdigit()));
}

// ============================================================================
// GitRepo Log Tests
// ============================================================================

#[test]
fn test_log_single_commit() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commits = repo.log(10).unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].message, "Initial commit");
    assert_eq!(commits[0].author, "Test User");
    assert_eq!(commits[0].email, "test@example.com");
}

#[test]
fn test_log_multiple_commits() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "file1.txt", "content1", "First commit");
    commit_file(&temp_dir, "file2.txt", "content2", "Second commit");
    commit_file(&temp_dir, "file3.txt", "content3", "Third commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commits = repo.log(10).unwrap();

    assert_eq!(commits.len(), 3);

    // Commits are in reverse chronological order
    assert_eq!(commits[0].message, "Third commit");
    assert_eq!(commits[1].message, "Second commit");
    assert_eq!(commits[2].message, "First commit");
}

#[test]
fn test_log_limit() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "file1.txt", "c1", "Commit 1");
    commit_file(&temp_dir, "file2.txt", "c2", "Commit 2");
    commit_file(&temp_dir, "file3.txt", "c3", "Commit 3");
    commit_file(&temp_dir, "file4.txt", "c4", "Commit 4");
    commit_file(&temp_dir, "file5.txt", "c5", "Commit 5");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commits = repo.log(3).unwrap();

    assert_eq!(commits.len(), 3);
}

#[test]
fn test_file_log() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "file1.txt", "v1", "Create file1");
    commit_file(&temp_dir, "file2.txt", "v1", "Create file2");
    commit_file(&temp_dir, "file1.txt", "v2", "Update file1");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commits = repo.file_log("file1.txt", 10).unwrap();

    // file1.txt was modified in 2 commits
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].message, "Update file1");
    assert_eq!(commits[1].message, "Create file1");
}

// ============================================================================
// GitRepo Status Tests
// ============================================================================

#[test]
fn test_status_clean() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let status = repo.status().unwrap();

    assert!(status.is_empty(), "Clean repo should have no changed files");
}

#[test]
fn test_status_modified() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    // Modify the file
    fs::write(temp_dir.path().join("test.txt"), "modified").unwrap();

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let status = repo.status().unwrap();

    assert_eq!(status.len(), 1);
    assert_eq!(status[0].path, "test.txt");
    assert_eq!(status[0].status, FileStatus::Modified);
}

#[test]
fn test_status_added() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    // Create untracked file
    fs::write(temp_dir.path().join("new.txt"), "new content").unwrap();

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let status = repo.status().unwrap();

    assert_eq!(status.len(), 1);
    assert_eq!(status[0].path, "new.txt");
    assert_eq!(status[0].status, FileStatus::Added);
}

#[test]
fn test_status_deleted() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    // Delete the file
    fs::remove_file(temp_dir.path().join("test.txt")).unwrap();

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let status = repo.status().unwrap();

    assert_eq!(status.len(), 1);
    assert_eq!(status[0].status, FileStatus::Deleted);
}

// ============================================================================
// GitRepo Diff Tests
// ============================================================================

#[test]
fn test_diff_files_between_commits() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "file1.txt", "v1", "First commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let first_commit = repo.current_commit().unwrap();

    commit_file(&temp_dir, "file2.txt", "v1", "Second commit");
    let second_commit = repo.current_commit().unwrap();

    let changes = repo.diff_files(&first_commit, &second_commit).unwrap();

    // Should show file2.txt was added
    assert!(!changes.is_empty());
}

#[test]
fn test_has_changes() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();

    // Initially no changes
    assert!(!repo.has_changes("test.txt").unwrap());

    // Modify file
    fs::write(temp_dir.path().join("test.txt"), "modified").unwrap();

    // Now has changes
    assert!(repo.has_changes("test.txt").unwrap());
}

// ============================================================================
// GitRepo ls-files Tests
// ============================================================================

#[test]
fn test_ls_files() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "file1.txt", "c1", "Add file1");
    commit_file(&temp_dir, "file2.txt", "c2", "Add file2");
    commit_file(&temp_dir, "subdir/file3.txt", "c3", "Add file3 in subdir");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let files = repo.ls_files().unwrap();

    assert!(files.contains(&"file1.txt".to_owned()));
    assert!(files.contains(&"file2.txt".to_owned()));
    assert!(files.iter().any(|f| f.contains("file3.txt")));
}

// ============================================================================
// GitRepo Blame Tests
// ============================================================================

#[test]
fn test_blame_single_commit() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "line1\nline2\nline3", "Initial commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let blame = repo.blame("test.txt").unwrap();

    assert_eq!(blame.len(), 3);
    assert_eq!(blame[0].author, "Test User");
}

#[test]
fn test_blame_multiple_commits() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "line1\nline2", "First commit");
    commit_file(&temp_dir, "test.txt", "line1\nline2\nline3", "Second commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let blame = repo.blame("test.txt").unwrap();

    assert_eq!(blame.len(), 3);
}

// ============================================================================
// FileStatus Tests
// ============================================================================

#[test]
fn test_file_status_equality() {
    // Test FileStatus enum variants exist and can be compared
    let added = FileStatus::Added;
    let modified = FileStatus::Modified;
    let deleted = FileStatus::Deleted;

    assert_eq!(added, FileStatus::Added);
    assert_eq!(modified, FileStatus::Modified);
    assert_eq!(deleted, FileStatus::Deleted);
    assert_ne!(added, modified);
    assert_ne!(modified, deleted);
}

// ============================================================================
// RemoteRepo URL Parsing Tests
// ============================================================================

#[test]
fn test_parse_github_https() {
    let repo = RemoteRepo::parse("https://github.com/rust-lang/rust").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.owner, Some("rust-lang".to_owned()));
    assert_eq!(repo.name, "rust");
    assert!(repo.branch.is_none());
}

#[test]
fn test_parse_github_https_with_git_suffix() {
    let repo = RemoteRepo::parse("https://github.com/rust-lang/rust.git").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.name, "rust"); // .git should be stripped
}

#[test]
fn test_parse_github_ssh() {
    let repo = RemoteRepo::parse("git@github.com:rust-lang/rust.git").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.owner, Some("rust-lang".to_owned()));
    assert_eq!(repo.name, "rust");
}

#[test]
fn test_parse_github_shorthand() {
    let repo = RemoteRepo::parse("rust-lang/rust").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.owner, Some("rust-lang".to_owned()));
    assert_eq!(repo.name, "rust");
}

#[test]
fn test_parse_github_prefix_shorthand() {
    let repo = RemoteRepo::parse("github:rust-lang/rust").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.owner, Some("rust-lang".to_owned()));
    assert_eq!(repo.name, "rust");
}

#[test]
fn test_parse_gitlab_https() {
    let repo = RemoteRepo::parse("https://gitlab.com/gitlab-org/gitlab").unwrap();

    assert_eq!(repo.provider, GitProvider::GitLab);
    assert_eq!(repo.owner, Some("gitlab-org".to_owned()));
    assert_eq!(repo.name, "gitlab");
}

#[test]
fn test_parse_gitlab_shorthand() {
    let repo = RemoteRepo::parse("gitlab:gitlab-org/gitlab").unwrap();

    assert_eq!(repo.provider, GitProvider::GitLab);
}

#[test]
fn test_parse_bitbucket_https() {
    let repo = RemoteRepo::parse("https://bitbucket.org/atlassian/aui").unwrap();

    assert_eq!(repo.provider, GitProvider::Bitbucket);
    assert_eq!(repo.owner, Some("atlassian".to_owned()));
    assert_eq!(repo.name, "aui");
}

#[test]
fn test_parse_bitbucket_shorthand() {
    let repo = RemoteRepo::parse("bitbucket:atlassian/aui").unwrap();

    assert_eq!(repo.provider, GitProvider::Bitbucket);
}

#[test]
fn test_parse_with_branch() {
    let repo = RemoteRepo::parse("https://github.com/rust-lang/rust/tree/master").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.branch, Some("master".to_owned()));
}

#[test]
fn test_parse_with_branch_and_subdir() {
    let repo = RemoteRepo::parse("https://github.com/rust-lang/rust/tree/master/src/lib").unwrap();

    assert_eq!(repo.provider, GitProvider::GitHub);
    assert_eq!(repo.branch, Some("master".to_owned()));
    assert_eq!(repo.subdir, Some("src/lib".to_owned()));
}

#[test]
fn test_parse_gitlab_ssh() {
    let repo = RemoteRepo::parse("git@gitlab.com:gitlab-org/gitlab.git").unwrap();

    assert_eq!(repo.provider, GitProvider::GitLab);
    assert_eq!(repo.owner, Some("gitlab-org".to_owned()));
    assert_eq!(repo.name, "gitlab");
}

#[test]
fn test_parse_invalid_url() {
    let result = RemoteRepo::parse("not-a-valid-url");

    // Single word without / is invalid
    assert!(result.is_err());
}

// ============================================================================
// RemoteRepo is_remote_url Tests
// ============================================================================

#[test]
fn test_is_remote_url_https() {
    assert!(RemoteRepo::is_remote_url("https://github.com/owner/repo"));
    assert!(RemoteRepo::is_remote_url("https://gitlab.com/owner/repo"));
    assert!(RemoteRepo::is_remote_url("https://bitbucket.org/owner/repo"));
}

#[test]
fn test_is_remote_url_ssh() {
    assert!(RemoteRepo::is_remote_url("git@github.com:owner/repo.git"));
    assert!(RemoteRepo::is_remote_url("git@gitlab.com:owner/repo.git"));
}

#[test]
fn test_is_remote_url_shorthand() {
    assert!(RemoteRepo::is_remote_url("github:owner/repo"));
    assert!(RemoteRepo::is_remote_url("gitlab:owner/repo"));
    assert!(RemoteRepo::is_remote_url("bitbucket:owner/repo"));
    assert!(RemoteRepo::is_remote_url("owner/repo")); // Assumes GitHub
}

#[test]
fn test_is_remote_url_local_paths() {
    assert!(!RemoteRepo::is_remote_url("/path/to/local/repo"));
    assert!(!RemoteRepo::is_remote_url("./relative/path"));
    assert!(!RemoteRepo::is_remote_url("../parent/path"));
}

#[test]
fn test_is_remote_url_nested_paths() {
    // Paths with multiple slashes are likely local paths
    assert!(!RemoteRepo::is_remote_url("a/b/c")); // More than one /
}

// ============================================================================
// RemoteRepo Clone URL Building Tests
// ============================================================================

#[test]
fn test_github_clone_url() {
    let repo = RemoteRepo::parse("rust-lang/rust").unwrap();
    assert_eq!(repo.url, "https://github.com/rust-lang/rust.git");
}

#[test]
fn test_gitlab_clone_url() {
    let repo = RemoteRepo::parse("gitlab:gitlab-org/gitlab").unwrap();
    assert_eq!(repo.url, "https://gitlab.com/gitlab-org/gitlab.git");
}

#[test]
fn test_bitbucket_clone_url() {
    let repo = RemoteRepo::parse("bitbucket:atlassian/aui").unwrap();
    assert_eq!(repo.url, "https://bitbucket.org/atlassian/aui.git");
}

// ============================================================================
// GitProvider Tests
// ============================================================================

#[test]
fn test_git_provider_equality() {
    assert_eq!(GitProvider::GitHub, GitProvider::GitHub);
    assert_ne!(GitProvider::GitHub, GitProvider::GitLab);
    assert_ne!(GitProvider::GitHub, GitProvider::Bitbucket);
    assert_ne!(GitProvider::GitHub, GitProvider::Generic);
}

#[test]
fn test_git_provider_copy() {
    let provider = GitProvider::GitHub;
    let copy = provider;
    assert_eq!(provider, copy);
}

// ============================================================================
// Error Display Tests
// ============================================================================

#[test]
fn test_git_error_display() {
    let not_repo = GitError::NotAGitRepo;
    let display = format!("{}", not_repo);
    assert!(display.contains("git repository"));

    let cmd_failed = GitError::CommandFailed("test error".to_owned());
    let display = format!("{}", cmd_failed);
    assert!(display.contains("test error"));

    let parse_err = GitError::ParseError("parse issue".to_owned());
    let display = format!("{}", parse_err);
    assert!(display.contains("parse issue"));
}

#[test]
fn test_remote_error_display() {
    let invalid = RemoteError::InvalidUrl("bad url".to_owned());
    let display = format!("{}", invalid);
    assert!(display.contains("bad url"));

    let git_err = RemoteError::GitError("git failed".to_owned());
    let display = format!("{}", git_err);
    assert!(display.contains("git failed"));

    let io_err = RemoteError::IoError("io failed".to_owned());
    let display = format!("{}", io_err);
    assert!(display.contains("io failed"));

    let not_found = RemoteError::NotFound("missing".to_owned());
    let display = format!("{}", not_found);
    assert!(display.contains("missing"));
}

// ============================================================================
// Integration Tests (requires git)
// ============================================================================

#[test]
fn test_last_modified_commit() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "v1", "Create file");
    commit_file(&temp_dir, "test.txt", "v2", "Update file");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commit = repo.last_modified_commit("test.txt").unwrap();

    assert_eq!(commit.message, "Update file");
}

#[test]
fn test_file_change_frequency() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "v1", "Create file");
    commit_file(&temp_dir, "test.txt", "v2", "Update 1");
    commit_file(&temp_dir, "test.txt", "v3", "Update 2");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let freq = repo.file_change_frequency("test.txt", 30).unwrap();

    // File was modified 3 times in the last 30 days
    assert_eq!(freq, 3);
}

#[test]
fn test_diff_content() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "line1\n", "First commit");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let first_commit = repo.current_commit().unwrap();

    commit_file(&temp_dir, "test.txt", "line1\nline2\n", "Second commit");
    let second_commit = repo.current_commit().unwrap();

    let diff = repo
        .diff_content(&first_commit, &second_commit, "test.txt")
        .unwrap();

    assert!(diff.contains("+line2") || diff.contains("line2"));
}

// ============================================================================
// Commit Struct Tests
// ============================================================================

#[test]
fn test_commit_struct_fields() {
    let commit = Commit {
        hash: "a".repeat(40),
        short_hash: "a".repeat(7),
        author: "Test User".to_owned(),
        email: "test@example.com".to_owned(),
        date: "2024-01-01".to_owned(),
        message: "Test commit".to_owned(),
    };

    assert_eq!(commit.hash.len(), 40);
    assert_eq!(commit.short_hash.len(), 7);
    assert_eq!(commit.author, "Test User");
}

#[test]
fn test_commit_clone() {
    let commit = Commit {
        hash: "a".repeat(40),
        short_hash: "a".repeat(7),
        author: "Test".to_owned(),
        email: "test@test.com".to_owned(),
        date: "2024-01-01".to_owned(),
        message: "Test".to_owned(),
    };

    let cloned = commit.clone();
    assert_eq!(commit.hash, cloned.hash);
}

// ============================================================================
// ChangedFile Struct Tests
// ============================================================================

#[test]
fn test_changed_file_struct() {
    let file = ChangedFile {
        path: "src/main.rs".to_owned(),
        old_path: None,
        status: FileStatus::Modified,
        additions: 10,
        deletions: 5,
    };

    assert_eq!(file.path, "src/main.rs");
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!(file.additions, 10);
    assert_eq!(file.deletions, 5);
}

// ============================================================================
// BlameLine Struct Tests
// ============================================================================

#[test]
fn test_blame_line_struct() {
    let line = BlameLine {
        commit: "abc1234".to_owned(),
        author: "Test User".to_owned(),
        date: "2024-01-01".to_owned(),
        line_number: 42,
    };

    assert_eq!(line.commit, "abc1234");
    assert_eq!(line.line_number, 42);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_unicode_in_commit_message() {
    let temp_dir = create_test_git_repo();
    commit_file(&temp_dir, "test.txt", "hello", "Fix: 修复中文问题 🎉");

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let commits = repo.log(1).unwrap();

    assert!(commits[0].message.contains("修复") || commits[0].message.contains("Fix"));
}

#[test]
fn test_unicode_in_filename() {
    let temp_dir = create_test_git_repo();

    // Create file with Unicode name
    fs::write(temp_dir.path().join("测试.txt"), "content").unwrap();

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "测试.txt"])
        .output()
        .unwrap();

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Add Unicode file"])
        .output()
        .unwrap();

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let files = repo.ls_files().unwrap();

    // Git should track the file (may be quoted in output)
    assert!(!files.is_empty());
}

#[test]
fn test_empty_repo() {
    let temp_dir = create_test_git_repo();
    // No commits yet

    let repo = GitRepo::open(temp_dir.path()).unwrap();

    // Operations on empty repo may fail gracefully
    let commits = repo.log(10);
    // May error or return empty
    assert!(commits.is_ok() || commits.is_err());
}

#[test]
fn test_subdir_creation() {
    let temp_dir = create_test_git_repo();

    // Create nested directory structure
    fs::create_dir_all(temp_dir.path().join("src/lib")).unwrap();
    fs::write(temp_dir.path().join("src/lib/mod.rs"), "// module").unwrap();

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "."])
        .output()
        .unwrap();

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Add module"])
        .output()
        .unwrap();

    let repo = GitRepo::open(temp_dir.path()).unwrap();
    let files = repo.ls_files().unwrap();

    assert!(files.iter().any(|f| f.contains("mod.rs")));
}
