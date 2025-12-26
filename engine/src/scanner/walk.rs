//! File collection utilities
//!
//! This module provides file path collection using the `ignore` crate
//! for fast, gitignore-respecting directory traversal.

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::path::Path;

use super::common::is_binary_extension;
use super::{FileInfo, ScannerConfig};
use crate::parser::detect_file_language;

/// Collect file information (paths, sizes) without reading content
///
/// Uses the `ignore` crate for fast traversal that respects:
/// - .gitignore patterns
/// - Global gitignore (~/.gitignore)
/// - .git/info/exclude
///
/// # Arguments
/// * `base_path` - Root directory to scan
/// * `config` - Scanner configuration
///
/// # Returns
/// Vector of FileInfo structs for each file to process
pub fn collect_file_infos(base_path: &Path, config: &ScannerConfig) -> Result<Vec<FileInfo>> {
    let mut file_infos = Vec::new();

    let walker = WalkBuilder::new(base_path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .filter_entry(|entry| {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name == ".git" {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        let metadata = entry_path.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        if size_bytes > config.max_file_size {
            continue;
        }

        if is_binary_extension(entry_path) {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(base_path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        let language = detect_file_language(entry_path);

        file_infos.push(FileInfo {
            path: entry_path.to_path_buf(),
            relative_path,
            size_bytes: Some(size_bytes),
            language,
        });
    }

    Ok(file_infos)
}

/// Collect file paths from a repository, returning only path information
///
/// This is a lighter version that doesn't detect languages.
/// Useful when language detection will happen during content processing.
pub fn collect_file_paths(base_path: &Path, config: &ScannerConfig) -> Result<Vec<FileInfo>> {
    let base_path = base_path
        .canonicalize()
        .context("Invalid repository path")?;
    let mut file_infos = Vec::new();

    let walker = WalkBuilder::new(&base_path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .filter_entry(|entry| {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name == ".git" {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        if is_binary_extension(entry_path) {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(&base_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry_path.to_string_lossy().to_string());

        file_infos.push(FileInfo {
            path: entry_path.to_path_buf(),
            relative_path,
            size_bytes: None,
            language: None,
        });
    }

    Ok(file_infos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_collect_file_infos_empty_dir() {
        let dir = tempdir().unwrap();
        let config = ScannerConfig::default();
        let infos = collect_file_infos(dir.path(), &config).unwrap();
        assert!(infos.is_empty());
    }

    #[test]
    fn test_collect_file_infos_single_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let config = ScannerConfig::default();
        let infos = collect_file_infos(dir.path(), &config).unwrap();

        assert_eq!(infos.len(), 1);
        assert!(infos[0].relative_path.contains("test.rs"));
        assert_eq!(infos[0].language, Some("rust".to_string()));
    }

    #[test]
    fn test_collect_file_infos_skips_binary() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("binary.exe"), "not really binary").unwrap();
        fs::write(dir.path().join("source.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig::default();
        let infos = collect_file_infos(dir.path(), &config).unwrap();

        assert_eq!(infos.len(), 1);
        assert!(infos[0].relative_path.contains("source.rs"));
    }

    #[test]
    fn test_collect_file_infos_respects_size_limit() {
        let dir = tempdir().unwrap();
        // Create a file larger than limit
        let large_content = "x".repeat(1000);
        fs::write(dir.path().join("large.rs"), &large_content).unwrap();
        fs::write(dir.path().join("small.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig {
            max_file_size: 500, // Very small limit
            ..Default::default()
        };
        let infos = collect_file_infos(dir.path(), &config).unwrap();

        assert_eq!(infos.len(), 1);
        assert!(infos[0].relative_path.contains("small.rs"));
    }

    #[test]
    fn test_collect_file_paths_no_language() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let config = ScannerConfig::default();
        let infos = collect_file_paths(dir.path(), &config).unwrap();

        assert_eq!(infos.len(), 1);
        // Language is not detected in paths-only mode
        assert!(infos[0].language.is_none());
    }
}
