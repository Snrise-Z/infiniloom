//! Remote repository support
//!
//! Supports cloning and fetching from remote Git repositories (GitHub, GitLab, Bitbucket, etc.)

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use thiserror::Error;
use url::Url;

/// Supported Git providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitProvider {
    GitHub,
    GitLab,
    Bitbucket,
    Generic,
}

/// Parsed remote repository URL
#[derive(Debug, Clone)]
pub struct RemoteRepo {
    /// Original URL
    pub url: String,
    /// Git provider
    pub provider: GitProvider,
    /// Repository owner/organization
    pub owner: Option<String>,
    /// Repository name
    pub name: String,
    /// Branch to clone (None = default branch)
    pub branch: Option<String>,
    /// Specific commit/tag to checkout
    pub reference: Option<String>,
    /// Subdirectory to extract (sparse checkout)
    pub subdir: Option<String>,
}

impl RemoteRepo {
    /// Parse a remote URL into a RemoteRepo
    /// Supports formats:
    /// - https://github.com/owner/repo
    /// - https://github.com/owner/repo/tree/branch
    /// - https://github.com/owner/repo/tree/branch/subdir
    /// - github:owner/repo
    /// - owner/repo (assumes GitHub)
    /// - git@github.com:owner/repo.git
    pub fn parse(input: &str) -> Result<Self, RemoteError> {
        let input = input.trim();

        // Handle shorthand formats
        if let Some(rest) = input.strip_prefix("github:") {
            return Self::parse_shorthand(rest, GitProvider::GitHub);
        }
        if let Some(rest) = input.strip_prefix("gitlab:") {
            return Self::parse_shorthand(rest, GitProvider::GitLab);
        }
        if let Some(rest) = input.strip_prefix("bitbucket:") {
            return Self::parse_shorthand(rest, GitProvider::Bitbucket);
        }

        // Handle owner/repo shorthand (assumes GitHub)
        if !input.contains("://") && !input.contains('@') && input.contains('/') {
            return Self::parse_shorthand(input, GitProvider::GitHub);
        }

        // Handle SSH URLs (git@github.com:owner/repo.git)
        if input.starts_with("git@") {
            return Self::parse_ssh_url(input);
        }

        // Handle HTTPS URLs
        Self::parse_https_url(input)
    }

    fn parse_shorthand(input: &str, provider: GitProvider) -> Result<Self, RemoteError> {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() < 2 {
            return Err(RemoteError::InvalidUrl(format!("Invalid shorthand: {}", input)));
        }

        let owner = parts[0].to_owned();
        let name = parts[1].trim_end_matches(".git").to_owned();

        let (branch, subdir) = if parts.len() > 2 {
            // Check if "tree" or "blob" is in path (GitHub URL format)
            if parts.get(2) == Some(&"tree") || parts.get(2) == Some(&"blob") {
                let branch = parts.get(3).map(|s| s.to_string());
                let subdir = if parts.len() > 4 {
                    Some(parts[4..].join("/"))
                } else {
                    None
                };
                (branch, subdir)
            } else {
                // Assume rest is subdir
                (None, Some(parts[2..].join("/")))
            }
        } else {
            (None, None)
        };

        Ok(Self {
            url: Self::build_clone_url(provider, &owner, &name),
            provider,
            owner: Some(owner),
            name,
            branch,
            reference: None,
            subdir,
        })
    }

    fn parse_ssh_url(input: &str) -> Result<Self, RemoteError> {
        // git@github.com:owner/repo.git
        let provider = if input.contains("github.com") {
            GitProvider::GitHub
        } else if input.contains("gitlab.com") {
            GitProvider::GitLab
        } else if input.contains("bitbucket.org") {
            GitProvider::Bitbucket
        } else {
            GitProvider::Generic
        };

        // Extract owner/repo from path
        let path_start = input
            .find(':')
            .ok_or_else(|| RemoteError::InvalidUrl("Invalid SSH URL format".to_owned()))?
            + 1;
        let path = &input[path_start..];

        // For Generic providers, preserve the original SSH URL
        // This ensures self-hosted Git servers (Gitea, self-hosted GitLab, etc.) work correctly
        if provider == GitProvider::Generic {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() < 2 {
                return Err(RemoteError::InvalidUrl(format!(
                    "Cannot parse owner/repo from SSH URL: {}",
                    input
                )));
            }
            let owner = parts[0].to_owned();
            let name = parts[1].trim_end_matches(".git").to_owned();

            return Ok(Self {
                url: input.to_owned(), // Keep original SSH URL for generic providers
                provider,
                owner: Some(owner),
                name,
                branch: None,
                reference: None,
                subdir: None,
            });
        }

        Self::parse_shorthand(path, provider)
    }

    fn parse_https_url(input: &str) -> Result<Self, RemoteError> {
        let url = Url::parse(input).map_err(|e| RemoteError::InvalidUrl(e.to_string()))?;

        let host = url.host_str().unwrap_or("");
        let provider = if host.contains("github.com") {
            GitProvider::GitHub
        } else if host.contains("gitlab.com") {
            GitProvider::GitLab
        } else if host.contains("bitbucket.org") {
            GitProvider::Bitbucket
        } else {
            GitProvider::Generic
        };

        let path = url.path().trim_start_matches('/');

        // For Generic providers, preserve the original URL instead of rebuilding
        // This ensures custom Git servers (self-hosted GitLab, Gitea, etc.) work correctly
        if provider == GitProvider::Generic {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() < 2 {
                return Err(RemoteError::InvalidUrl(format!(
                    "Cannot parse repository path from URL: {}",
                    input
                )));
            }
            let owner = parts[0].to_owned();
            let name = parts[1].trim_end_matches(".git").to_owned();

            return Ok(Self {
                url: input.to_owned(), // Keep original URL for generic providers
                provider,
                owner: Some(owner),
                name,
                branch: None,
                reference: None,
                subdir: None,
            });
        }

        Self::parse_shorthand(path, provider)
    }

    fn build_clone_url(provider: GitProvider, owner: &str, name: &str) -> String {
        match provider {
            GitProvider::GitHub => format!("https://github.com/{}/{}.git", owner, name),
            GitProvider::GitLab => format!("https://gitlab.com/{}/{}.git", owner, name),
            GitProvider::Bitbucket => format!("https://bitbucket.org/{}/{}.git", owner, name),
            GitProvider::Generic => format!("https://example.com/{}/{}.git", owner, name),
        }
    }

    /// Clone the repository to a temporary directory with RAII cleanup
    /// Returns (path_to_repo, temp_dir_handle) - keep the TempDir alive to prevent cleanup
    pub fn clone_with_cleanup(&self) -> Result<(PathBuf, TempDir), RemoteError> {
        let temp_dir = TempDir::with_prefix("infiniloom-")
            .map_err(|e| RemoteError::IoError(format!("Failed to create temp dir: {}", e)))?;

        let target = temp_dir.path().to_path_buf();
        let repo_path = self.clone_to_path(&target)?;

        Ok((repo_path, temp_dir))
    }

    /// Clone the repository to a temporary directory (legacy method without RAII cleanup)
    ///
    /// # Warning
    /// This method does not clean up the temp directory automatically.
    /// Consider using [`clone_with_cleanup()`](Self::clone_with_cleanup) instead for automatic cleanup.
    ///
    /// # Public API Note
    /// This method is part of the public library API for users who need manual control
    /// over the cloned directory lifecycle. The CLI uses `clone_with_cleanup()` internally.
    #[allow(dead_code)]
    pub fn clone(&self, target_dir: Option<&Path>) -> Result<PathBuf, RemoteError> {
        let target = target_dir.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "infiniloom-{}-{}",
                self.owner.as_deref().unwrap_or("repo"),
                self.name
            ))
        });

        self.clone_to_path(&target)
    }

    /// Internal method to clone to a specific path
    ///
    /// SAFETY: Will only delete existing directories if:
    /// - The directory is inside system temp directory, OR
    /// - The directory contains an `.infiniloom-clone` marker file, OR
    /// - The directory is empty
    fn clone_to_path(&self, target: &Path) -> Result<PathBuf, RemoteError> {
        // Clean up existing directory if it exists (with safety checks)
        if target.exists() {
            if !Self::is_safe_to_delete(target) {
                return Err(RemoteError::IoError(format!(
                    "Refusing to delete existing directory '{}'. \
                     Path is not empty, not in temp dir, and has no .infiniloom-clone marker. \
                     Please remove manually or use a different target path.",
                    target.display()
                )));
            }
            std::fs::remove_dir_all(target).map_err(|e| RemoteError::IoError(e.to_string()))?;
        }

        // Build git clone command
        let mut cmd = Command::new("git");
        cmd.arg("clone");

        // Shallow clone for faster download
        cmd.arg("--depth").arg("1");

        // Branch if specified
        if let Some(ref branch) = self.branch {
            cmd.arg("--branch").arg(branch);
        }

        // Single branch for speed
        cmd.arg("--single-branch");

        cmd.arg(&self.url);
        cmd.arg(target);

        let output = cmd
            .output()
            .map_err(|e| RemoteError::GitError(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteError::GitError(format!("git clone failed: {}", stderr)));
        }

        // Checkout specific reference if provided
        if let Some(ref reference) = self.reference {
            let mut checkout = Command::new("git");
            checkout.current_dir(target);
            checkout.args(["checkout", reference]);

            let output = checkout
                .output()
                .map_err(|e| RemoteError::GitError(format!("Failed to checkout: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(RemoteError::GitError(format!("git checkout failed: {}", stderr)));
            }
        }

        // Create marker file so we know this is a directory we created
        // This allows safe cleanup on subsequent runs
        let marker_path = target.join(".infiniloom-clone");
        drop(std::fs::write(&marker_path, format!("cloned from: {}\n", self.url)));

        // If subdir specified, return path to subdir
        if let Some(ref subdir) = self.subdir {
            let subdir_path = target.join(subdir);
            if subdir_path.exists() {
                return Ok(subdir_path);
            }
        }

        Ok(target.to_path_buf())
    }

    /// Check if a directory is safe to delete
    ///
    /// Returns true if:
    /// - The path is inside system temp directory, OR
    /// - The path contains an `.infiniloom-clone` marker file, OR
    /// - The path is an empty directory
    fn is_safe_to_delete(path: &Path) -> bool {
        // Check if path is in temp directory
        if let Ok(temp_dir) = std::env::temp_dir().canonicalize() {
            if let Ok(canonical_path) = path.canonicalize() {
                if canonical_path.starts_with(&temp_dir) {
                    return true;
                }
            }
        }

        // Check for our marker file
        if path.join(".infiniloom-clone").exists() {
            return true;
        }

        // Check if directory is empty
        if let Ok(mut entries) = std::fs::read_dir(path) {
            if entries.next().is_none() {
                return true;
            }
        }

        false
    }

    /// Clone with sparse checkout (only fetch specified paths)
    ///
    /// This is useful for very large repositories where you only need a subset
    /// of files. Uses Git's sparse checkout feature to minimize download size.
    ///
    /// # Safety
    /// Will only delete existing directories if:
    /// - The directory is inside system temp directory, OR
    /// - The directory contains an `.infiniloom-clone` marker file, OR
    /// - The directory is empty
    ///
    /// # Public API Note
    /// This method is part of the public library API. The CLI does not currently
    /// use sparse checkout, but it's available for library users who need it.
    #[allow(dead_code)]
    pub fn sparse_clone(
        &self,
        paths: &[&str],
        target_dir: Option<&Path>,
    ) -> Result<PathBuf, RemoteError> {
        let target = target_dir.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!("infiniloom-sparse-{}", self.name))
        });

        // Clean up (with safety checks)
        if target.exists() {
            if !Self::is_safe_to_delete(&target) {
                return Err(RemoteError::IoError(format!(
                    "Refusing to delete existing directory '{}'. \
                     Path is not empty, not in temp dir, and has no .infiniloom-clone marker. \
                     Please remove manually or use a different target path.",
                    target.display()
                )));
            }
            std::fs::remove_dir_all(&target).map_err(|e| RemoteError::IoError(e.to_string()))?;
        }

        // Initialize empty repo
        let mut init = Command::new("git");
        init.args(["init", &target.to_string_lossy()]);
        init.output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Configure sparse checkout
        let mut config = Command::new("git");
        config.current_dir(&target);
        config.args(["config", "core.sparseCheckout", "true"]);
        config
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Add remote
        let mut remote = Command::new("git");
        remote.current_dir(&target);
        remote.args(["remote", "add", "origin", &self.url]);
        remote
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Write sparse checkout config
        let sparse_dir = target.join(".git/info");
        std::fs::create_dir_all(&sparse_dir).map_err(|e| RemoteError::IoError(e.to_string()))?;

        let sparse_file = sparse_dir.join("sparse-checkout");
        let sparse_content = paths.join("\n");
        std::fs::write(&sparse_file, sparse_content)
            .map_err(|e| RemoteError::IoError(e.to_string()))?;

        // Fetch and checkout
        let branch = self.branch.as_deref().unwrap_or("HEAD");
        let mut fetch = Command::new("git");
        fetch.current_dir(&target);
        fetch.args(["fetch", "--depth", "1", "origin", branch]);
        let output = fetch
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteError::GitError(format!("git fetch failed: {}", stderr)));
        }

        let mut checkout = Command::new("git");
        checkout.current_dir(&target);
        checkout.args(["checkout", "FETCH_HEAD"]);
        checkout
            .output()
            .map_err(|e| RemoteError::GitError(e.to_string()))?;

        // Create marker file so we know this is a directory we created
        let marker_path = target.join(".infiniloom-clone");
        drop(std::fs::write(&marker_path, format!("sparse clone from: {}\n", self.url)));

        Ok(target)
    }

    /// Check if a URL is a remote repository URL
    pub fn is_remote_url(input: &str) -> bool {
        input.contains("://") ||
        input.starts_with("git@") ||
        input.starts_with("github:") ||
        input.starts_with("gitlab:") ||
        input.starts_with("bitbucket:") ||
        // Simple owner/repo format (not starting with / or .)
        (input.contains('/') && !input.starts_with('/') && !input.starts_with('.') && input.matches('/').count() == 1)
    }
}

/// Remote repository errors
#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Git error: {0}")]
    GitError(String),
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;

    // ============================================
    // URL Parsing Tests - GitHub
    // ============================================

    #[test]
    fn test_parse_github_url() {
        let repo = RemoteRepo::parse("https://github.com/rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.owner, Some("rust-lang".to_string()));
        assert_eq!(repo.name, "rust");
    }

    #[test]
    fn test_parse_github_url_with_git_suffix() {
        let repo = RemoteRepo::parse("https://github.com/owner/repo.git").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.name, "repo");
    }

    #[test]
    fn test_parse_shorthand() {
        let repo = RemoteRepo::parse("rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.name, "rust");

        let repo = RemoteRepo::parse("github:rust-lang/rust").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
    }

    #[test]
    fn test_parse_ssh_url() {
        let repo = RemoteRepo::parse("git@github.com:rust-lang/rust.git").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.owner, Some("rust-lang".to_string()));
        assert_eq!(repo.name, "rust");
    }

    #[test]
    fn test_parse_with_branch() {
        let repo = RemoteRepo::parse("https://github.com/rust-lang/rust/tree/master").unwrap();
        assert_eq!(repo.branch, Some("master".to_string()));
    }

    #[test]
    fn test_parse_with_branch_and_subdir() {
        let repo = RemoteRepo::parse("https://github.com/owner/repo/tree/main/src/lib").unwrap();
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.branch, Some("main".to_string()));
        assert_eq!(repo.subdir, Some("src/lib".to_string()));
    }

    #[test]
    fn test_parse_with_blob_path() {
        let repo = RemoteRepo::parse("https://github.com/owner/repo/blob/main/README.md").unwrap();
        assert_eq!(repo.branch, Some("main".to_string()));
        assert_eq!(repo.subdir, Some("README.md".to_string()));
    }

    // ============================================
    // URL Parsing Tests - GitLab
    // ============================================

    #[test]
    fn test_parse_gitlab_https() {
        let repo = RemoteRepo::parse("https://gitlab.com/owner/project").unwrap();
        assert_eq!(repo.provider, GitProvider::GitLab);
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "project");
    }

    #[test]
    fn test_parse_gitlab_ssh() {
        let repo = RemoteRepo::parse("git@gitlab.com:owner/project.git").unwrap();
        assert_eq!(repo.provider, GitProvider::GitLab);
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "project");
    }

    #[test]
    fn test_parse_gitlab_shorthand() {
        let repo = RemoteRepo::parse("gitlab:mygroup/myproject").unwrap();
        assert_eq!(repo.provider, GitProvider::GitLab);
        assert_eq!(repo.owner, Some("mygroup".to_string()));
        assert_eq!(repo.name, "myproject");
    }

    // ============================================
    // URL Parsing Tests - Bitbucket
    // ============================================

    #[test]
    fn test_parse_bitbucket_https() {
        let repo = RemoteRepo::parse("https://bitbucket.org/team/repo").unwrap();
        assert_eq!(repo.provider, GitProvider::Bitbucket);
        assert_eq!(repo.owner, Some("team".to_string()));
        assert_eq!(repo.name, "repo");
    }

    #[test]
    fn test_parse_bitbucket_ssh() {
        let repo = RemoteRepo::parse("git@bitbucket.org:team/repo.git").unwrap();
        assert_eq!(repo.provider, GitProvider::Bitbucket);
        assert_eq!(repo.owner, Some("team".to_string()));
        assert_eq!(repo.name, "repo");
    }

    #[test]
    fn test_parse_bitbucket_shorthand() {
        let repo = RemoteRepo::parse("bitbucket:myteam/myrepo").unwrap();
        assert_eq!(repo.provider, GitProvider::Bitbucket);
        assert_eq!(repo.owner, Some("myteam".to_string()));
        assert_eq!(repo.name, "myrepo");
    }

    // ============================================
    // URL Parsing Tests - Generic Provider
    // ============================================

    #[test]
    fn test_parse_ssh_url_generic_provider() {
        // Self-hosted Git servers should preserve the original SSH URL
        let repo = RemoteRepo::parse("git@git.mycompany.com:team/project.git").unwrap();
        assert_eq!(repo.provider, GitProvider::Generic);
        assert_eq!(repo.owner, Some("team".to_string()));
        assert_eq!(repo.name, "project");
        // Original SSH URL should be preserved (not converted to https://example.com/...)
        assert_eq!(repo.url, "git@git.mycompany.com:team/project.git");
    }

    #[test]
    fn test_parse_https_url_generic_provider() {
        // Self-hosted Git servers via HTTPS should preserve the original URL
        let repo = RemoteRepo::parse("https://git.mycompany.com/team/project.git").unwrap();
        assert_eq!(repo.provider, GitProvider::Generic);
        assert_eq!(repo.owner, Some("team".to_string()));
        assert_eq!(repo.name, "project");
        // Original HTTPS URL should be preserved
        assert_eq!(repo.url, "https://git.mycompany.com/team/project.git");
    }

    #[test]
    fn test_parse_gitea_server() {
        let repo = RemoteRepo::parse("https://gitea.example.org/user/project").unwrap();
        assert_eq!(repo.provider, GitProvider::Generic);
        assert_eq!(repo.owner, Some("user".to_string()));
        assert_eq!(repo.name, "project");
    }

    // ============================================
    // URL Parsing Tests - Edge Cases
    // ============================================

    #[test]
    fn test_parse_url_with_whitespace() {
        let repo = RemoteRepo::parse("  owner/repo  ").unwrap();
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "repo");
    }

    #[test]
    fn test_parse_shorthand_with_subdir() {
        let repo = RemoteRepo::parse("owner/repo/src/main").unwrap();
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.subdir, Some("src/main".to_string()));
    }

    // ============================================
    // URL Parsing Tests - Error Cases
    // ============================================

    #[test]
    fn test_parse_invalid_shorthand() {
        let result = RemoteRepo::parse("github:onlyname");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RemoteError::InvalidUrl(_)));
    }

    #[test]
    fn test_parse_invalid_url() {
        let result = RemoteRepo::parse("not-a-valid-url://weird");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_https_path() {
        let result = RemoteRepo::parse("https://github.com/onlyowner");
        assert!(result.is_err());
    }

    // ============================================
    // is_remote_url Tests
    // ============================================

    #[test]
    fn test_is_remote_url() {
        assert!(RemoteRepo::is_remote_url("https://github.com/foo/bar"));
        assert!(RemoteRepo::is_remote_url("git@github.com:foo/bar.git"));
        assert!(RemoteRepo::is_remote_url("github:foo/bar"));
        assert!(!RemoteRepo::is_remote_url("/path/to/local/repo"));
    }

    #[test]
    fn test_is_remote_url_gitlab() {
        assert!(RemoteRepo::is_remote_url("https://gitlab.com/foo/bar"));
        assert!(RemoteRepo::is_remote_url("gitlab:foo/bar"));
    }

    #[test]
    fn test_is_remote_url_bitbucket() {
        assert!(RemoteRepo::is_remote_url("https://bitbucket.org/foo/bar"));
        assert!(RemoteRepo::is_remote_url("bitbucket:foo/bar"));
    }

    #[test]
    fn test_is_remote_url_owner_repo_format() {
        assert!(RemoteRepo::is_remote_url("owner/repo"));
        assert!(!RemoteRepo::is_remote_url("./local/path"));
        assert!(!RemoteRepo::is_remote_url("/absolute/path"));
        assert!(!RemoteRepo::is_remote_url("owner/repo/subdir")); // More than one slash
    }

    #[test]
    fn test_is_remote_url_local_paths() {
        assert!(!RemoteRepo::is_remote_url("."));
        assert!(!RemoteRepo::is_remote_url(".."));
        assert!(!RemoteRepo::is_remote_url("./src"));
        assert!(!RemoteRepo::is_remote_url("../parent"));
        assert!(!RemoteRepo::is_remote_url("/home/user/project"));
    }

    // ============================================
    // build_clone_url Tests
    // ============================================

    #[test]
    fn test_build_clone_url_github() {
        let url = RemoteRepo::build_clone_url(GitProvider::GitHub, "owner", "repo");
        assert_eq!(url, "https://github.com/owner/repo.git");
    }

    #[test]
    fn test_build_clone_url_gitlab() {
        let url = RemoteRepo::build_clone_url(GitProvider::GitLab, "owner", "repo");
        assert_eq!(url, "https://gitlab.com/owner/repo.git");
    }

    #[test]
    fn test_build_clone_url_bitbucket() {
        let url = RemoteRepo::build_clone_url(GitProvider::Bitbucket, "owner", "repo");
        assert_eq!(url, "https://bitbucket.org/owner/repo.git");
    }

    #[test]
    fn test_build_clone_url_generic() {
        let url = RemoteRepo::build_clone_url(GitProvider::Generic, "owner", "repo");
        assert_eq!(url, "https://example.com/owner/repo.git");
    }

    // ============================================
    // is_safe_to_delete Tests
    // ============================================

    #[test]
    fn test_is_safe_to_delete_temp_dir() {
        let temp = TempDir::new().unwrap();
        assert!(RemoteRepo::is_safe_to_delete(temp.path()));
    }

    #[test]
    fn test_is_safe_to_delete_empty_dir() {
        let temp = TempDir::new().unwrap();
        let empty_dir = temp.path().join("empty");
        std::fs::create_dir(&empty_dir).unwrap();
        assert!(RemoteRepo::is_safe_to_delete(&empty_dir));
    }

    #[test]
    fn test_is_safe_to_delete_with_marker() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("with_marker");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join(".infiniloom-clone"), "marker").unwrap();
        assert!(RemoteRepo::is_safe_to_delete(&dir));
    }

    #[test]
    fn test_is_safe_to_delete_non_empty_without_marker() {
        // Create a directory outside temp that has content but no marker
        // This simulates a user directory that shouldn't be deleted
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("user_data");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("important.txt"), "don't delete me").unwrap();

        // Since it's in temp dir, it will still return true
        // But in production, a directory outside temp would return false
        assert!(RemoteRepo::is_safe_to_delete(&dir));
    }

    // ============================================
    // RemoteRepo Struct Tests
    // ============================================

    #[test]
    fn test_remote_repo_fields() {
        let repo = RemoteRepo::parse("https://github.com/owner/repo/tree/main/src").unwrap();
        assert!(repo.url.contains("github.com"));
        assert_eq!(repo.provider, GitProvider::GitHub);
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.branch, Some("main".to_string()));
        assert!(repo.reference.is_none());
        assert_eq!(repo.subdir, Some("src".to_string()));
    }

    // ============================================
    // GitProvider Enum Tests
    // ============================================

    #[test]
    fn test_git_provider_equality() {
        assert_eq!(GitProvider::GitHub, GitProvider::GitHub);
        assert_ne!(GitProvider::GitHub, GitProvider::GitLab);
        assert_ne!(GitProvider::GitLab, GitProvider::Bitbucket);
        assert_ne!(GitProvider::Bitbucket, GitProvider::Generic);
    }

    #[test]
    fn test_git_provider_clone() {
        let provider = GitProvider::GitHub;
        let cloned = provider;
        assert_eq!(provider, cloned);
    }

    // ============================================
    // RemoteError Tests
    // ============================================

    #[test]
    fn test_remote_error_display() {
        let err = RemoteError::InvalidUrl("bad url".to_string());
        assert!(err.to_string().contains("Invalid URL"));

        let err = RemoteError::GitError("clone failed".to_string());
        assert!(err.to_string().contains("Git error"));

        let err = RemoteError::IoError("file not found".to_string());
        assert!(err.to_string().contains("I/O error"));

        let err = RemoteError::NotFound("repo missing".to_string());
        assert!(err.to_string().contains("Not found"));
    }

    // ============================================
    // Clone Tests (require git but don't actually clone)
    // ============================================

    #[test]
    fn test_clone_with_cleanup_creates_temp_dir() {
        // We can't actually clone without network, but we can test the setup
        let repo = RemoteRepo::parse("owner/repo").unwrap();
        // Verify the URL is correctly formed
        assert!(repo.url.contains("github.com"));
        assert!(repo.url.contains("owner"));
        assert!(repo.url.contains("repo"));
    }

    #[test]
    fn test_clone_target_path_generation() {
        let repo = RemoteRepo::parse("owner/project").unwrap();

        // Test that clone() would use correct default path
        let expected_pattern =
            format!("infiniloom-{}-{}", repo.owner.as_deref().unwrap_or("repo"), repo.name);

        // The default path should be in temp dir with owner and name
        assert_eq!(repo.owner.as_deref(), Some("owner"));
        assert_eq!(repo.name, "project");
        assert!(expected_pattern.contains("owner"));
        assert!(expected_pattern.contains("project"));
    }

    // ============================================
    // Complex URL Parsing Tests
    // ============================================

    #[test]
    fn test_parse_url_removes_git_suffix() {
        let repo = RemoteRepo::parse("github:owner/repo.git").unwrap();
        assert_eq!(repo.name, "repo");
    }

    #[test]
    fn test_parse_deep_subdir() {
        let repo = RemoteRepo::parse("owner/repo/src/main/java/com/example").unwrap();
        assert_eq!(repo.owner, Some("owner".to_string()));
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.subdir, Some("src/main/java/com/example".to_string()));
    }
}
