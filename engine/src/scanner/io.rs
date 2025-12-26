//! File I/O utilities for the scanner
//!
//! This module provides smart file reading that automatically chooses
//! between regular I/O and memory-mapped I/O based on file size.

use std::path::Path;

use crate::mmap_scanner::MappedFile;

/// Threshold for using memory-mapped I/O (files >= 1MB use mmap)
pub const MMAP_THRESHOLD: u64 = 1024 * 1024;

/// Smart file reading that uses mmap for large files
///
/// Files >= MMAP_THRESHOLD (1MB) use memory-mapped I/O for better performance
/// on large files. Smaller files use regular `read_to_string`.
///
/// # Arguments
/// * `path` - Path to the file to read
/// * `size_bytes` - File size in bytes (used to decide mmap vs regular read)
///
/// # Returns
/// `Some(content)` if the file was read successfully, `None` if:
/// - File read failed (permissions, not found, etc.)
/// - File appears to be binary
/// - File is not valid UTF-8
pub fn smart_read_file(path: &Path, size_bytes: u64) -> Option<String> {
    smart_read_file_with_options(path, size_bytes, true)
}

/// Smart file reading with configurable mmap support
///
/// # Arguments
/// * `path` - Path to the file to read
/// * `size_bytes` - File size in bytes
/// * `use_mmap` - Whether to use mmap for large files
///
/// # Returns
/// `Some(content)` if successful, `None` otherwise
pub fn smart_read_file_with_options(
    path: &Path,
    size_bytes: u64,
    use_mmap: bool,
) -> Option<String> {
    if use_mmap && size_bytes >= MMAP_THRESHOLD {
        read_file_mmap(path)
    } else {
        read_file_regular(path)
    }
}

/// Read file using memory-mapped I/O
///
/// Best for large files (>= 1MB) as it avoids copying the entire file into memory.
fn read_file_mmap(path: &Path) -> Option<String> {
    let mapped = match MappedFile::open(path) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("Failed to mmap file {}: {}", path.display(), e);
            return None;
        },
    };

    if mapped.is_binary() {
        tracing::debug!("Skipping binary file: {}", path.display());
        return None;
    }

    match mapped.as_str() {
        Some(s) => Some(s.to_owned()),
        None => {
            tracing::debug!("File is not valid UTF-8: {}", path.display());
            None
        },
    }
}

/// Read file using regular std::fs::read_to_string
///
/// Best for small files (< 1MB) where the overhead of mmap setup isn't worth it.
fn read_file_regular(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Check for binary content (null bytes, etc.)
            if is_binary_string(&content) {
                tracing::debug!("Skipping binary file: {}", path.display());
                return None;
            }
            Some(content)
        },
        Err(e) => {
            tracing::debug!("Failed to read file {}: {}", path.display(), e);
            None
        },
    }
}

/// Check if a string appears to be binary content
///
/// Uses a null-byte heuristic: if we find null bytes in the first 8KB,
/// the content is likely binary.
fn is_binary_string(content: &str) -> bool {
    // Check first 8KB for binary indicators
    let check_size = content.floor_char_boundary(content.len().min(8192));
    let sample = &content[..check_size];

    // If we find null bytes, it's likely binary
    sample.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_smart_read_file_small() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("small.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let content = smart_read_file(&file_path, 13);
        assert_eq!(content, Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_smart_read_file_nonexistent() {
        let content = smart_read_file(Path::new("/nonexistent/file.txt"), 0);
        assert!(content.is_none());
    }

    #[test]
    fn test_smart_read_file_with_options_no_mmap() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        // Even with large size, mmap is disabled
        let content = smart_read_file_with_options(&file_path, MMAP_THRESHOLD + 1000, false);
        assert_eq!(content, Some("content".to_string()));
    }

    #[test]
    fn test_is_binary_string_text() {
        assert!(!is_binary_string("Hello, World!"));
        assert!(!is_binary_string("fn main() {\n    println!(\"Hello\");\n}"));
    }

    #[test]
    fn test_is_binary_string_with_null() {
        assert!(is_binary_string("Hello\0World"));
    }

    #[test]
    fn test_is_binary_string_empty() {
        // Empty string has no binary indicators
        // but division by zero protection should handle this
        let result = is_binary_string("");
        // Empty strings shouldn't crash, result can be true or false
        let _ = result;
    }

    #[test]
    fn test_is_binary_string_unicode() {
        // Normal Unicode text should not be detected as binary
        assert!(!is_binary_string("Hello 世界 🌍"));
        assert!(!is_binary_string("日本語テキスト"));
    }
}
