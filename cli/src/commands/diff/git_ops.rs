//! Git operations for diff command
//!
//! This module contains all git-related operations used by the diff command:
//! - Git availability checking
//! - Diff change detection and parsing
//! - File content retrieval from git
//! - Change line range extraction
//! - Reference resolution

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use infiniloom_engine::git::GitRepo;
use infiniloom_engine::index::{ChangeType, DiffChange};

/// Check if git is available in PATH
pub(crate) fn check_git_available() -> Result<()> {
    use std::process::Command;

    let output = Command::new("git")
        .arg("--version")
        .output()
        .context("Git is not installed or not found in PATH. Please install git and ensure it's available in your PATH.")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git is installed but returned an error. Please check your git installation."
        );
    }

    Ok(())
}

/// Get diff changes from git
pub(crate) fn get_diff_changes(
    repo_path: &PathBuf,
    reference: Option<&str>,
    staged: bool,
    include_diff_content: bool,
) -> Result<Vec<DiffChange>> {
    use std::process::Command;

    let mut changes = Vec::new();

    // Build git diff command
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--name-status");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    let output = cmd.output().context("Failed to run git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let status = parts[0];

            // For renames (R###), git outputs: R###\told_path\tnew_path
            let (file_path, old_path) = if status.starts_with('R') && parts.len() >= 3 {
                // Rename: parts[1] is old path, parts[2] is new path
                (parts[2].to_owned(), Some(parts[1].to_owned()))
            } else {
                (parts[1].to_owned(), None)
            };

            let change_type = match status.chars().next() {
                Some('A') => ChangeType::Added,
                Some('M') => ChangeType::Modified,
                Some('D') => ChangeType::Deleted,
                Some('R') => ChangeType::Renamed,
                _ => ChangeType::Modified,
            };

            // For modified files, get the actual changed lines
            // For renames, use the new path
            let line_ranges = match change_type {
                ChangeType::Modified => {
                    get_changed_lines(repo_path, &file_path, reference, staged)?
                },
                ChangeType::Added | ChangeType::Renamed => {
                    // Get actual line count to avoid iterating 4+ billion lines
                    let full_path = repo_path.join(&file_path);
                    let line_count = std::fs::read_to_string(&full_path)
                        .map(|content| content.lines().count() as u32)
                        .unwrap_or(1)
                        .max(1);
                    vec![(1, line_count)]
                },
                ChangeType::Deleted => {
                    // Deleted files have no lines to iterate - use empty range
                    vec![]
                },
            };

            // Optionally get the raw diff content
            // For renames, we need to check both old and new paths for diff content
            let diff_content = if include_diff_content {
                get_diff_content(repo_path, &file_path, reference, staged).ok()
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path,
                line_ranges,
                change_type,
                diff_content,
            });
        }
    }

    // Also include untracked files when looking at working tree changes
    // (not staged and no reference specified)
    if !staged && reference.is_none() {
        let untracked = get_untracked_files(repo_path)?;
        for file_path in untracked {
            // Get line count for untracked files
            let full_path = repo_path.join(&file_path);
            let line_count = std::fs::read_to_string(&full_path)
                .map(|content| content.lines().count() as u32)
                .unwrap_or(1)
                .max(1);

            // Read file content if requested
            let diff_content = if include_diff_content {
                std::fs::read_to_string(&full_path).ok().map(|content| {
                    format!(
                        "@@ -0,0 +1,{} @@\n{}",
                        line_count,
                        content
                            .lines()
                            .map(|l| format!("+{}", l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path: None,
                line_ranges: vec![(1, line_count)],
                change_type: ChangeType::Added,
                diff_content,
            });
        }
    }

    Ok(changes)
}

/// Get untracked files from git status
pub(crate) fn get_untracked_files(repo_path: &PathBuf) -> Result<Vec<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        return Ok(vec![]); // Silently ignore errors
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let untracked: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Untracked files start with "?? "
            line.strip_prefix("?? ").map(|stripped| stripped.to_owned())
        })
        .collect();

    Ok(untracked)
}

/// Get raw diff content for a file (the actual +/- lines)
pub(crate) fn get_diff_content(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<String> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout.to_string())
}

/// Get changed line ranges for a file
pub(crate) fn get_changed_lines(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<Vec<(u32, u32)>> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--unified=0");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;

    let mut ranges = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse @@ -start,count +start,count @@ lines
    for line in stdout.lines() {
        if line.starts_with("@@") {
            // Extract the new file range
            if let Some(plus_idx) = line.find('+') {
                let rest = &line[plus_idx + 1..];
                if let Some(space_idx) = rest.find(' ') {
                    let range_str = &rest[..space_idx];
                    let parts: Vec<&str> = range_str.split(',').collect();
                    if !parts.is_empty() {
                        let start: u32 = parts[0].parse().unwrap_or(1);
                        let count: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                        ranges.push((start, start + count.saturating_sub(1)));
                    }
                }
            }
        }
    }

    if ranges.is_empty() {
        // Fallback: get actual line count from file instead of arbitrary 100
        let full_path = repo_path.join(file_path);
        let line_count = std::fs::read_to_string(&full_path)
            .map(|content| content.lines().count() as u32)
            .unwrap_or(1)
            .max(1); // Ensure at least 1 line
        ranges.push((1, line_count));
    }

    Ok(ranges)
}

/// Resolve the base git reference from a reference string
///
/// For "main..feature" returns "main", for "HEAD~1" returns "HEAD~1"
pub(crate) fn resolve_base_ref(reference: Option<&str>, repo_path: &Path) -> Option<String> {
    let ref_str = reference.unwrap_or("HEAD");

    // Handle range format: "base..head" or "base...head"
    if let Some(base) = ref_str.split("..").next() {
        if !base.is_empty() && base != ref_str {
            return Some(base.to_owned());
        }
    }

    // For single refs like "HEAD~1", "main", etc., use as-is
    // But verify it's a valid ref first
    use std::process::Command;
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "--verify", ref_str])
        .output()
        .ok()?;

    if output.status.success() {
        Some(ref_str.to_owned())
    } else {
        None
    }
}

/// Read file content from a git reference
///
/// Uses `git show ref:path` to retrieve file content from a specific commit/ref
pub(crate) fn read_file_from_git(
    repo_path: &Path,
    git_ref: &str,
    file_path: &str,
) -> Option<String> {
    use std::process::Command;

    // Git show format: ref:path
    let ref_path = format!("{}:{}", git_ref, file_path);

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["show", &ref_path])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

/// Check if the symbol index is fresh (no changes since last build)
pub(crate) fn is_index_fresh(repo_path: &Path, meta: &infiniloom_engine::index::IndexMeta) -> bool {
    let repo = match GitRepo::open(repo_path) {
        Ok(repo) => repo,
        Err(_) => return true,
    };

    let status = match repo.status() {
        Ok(status) => status,
        Err(_) => return false,
    };
    if !status.is_empty() {
        return false;
    }

    if let Ok(head) = repo.current_commit() {
        if let Some(ref index_commit) = meta.commit_hash {
            if index_commit != &head {
                return false;
            }
        }
    }

    true
}
