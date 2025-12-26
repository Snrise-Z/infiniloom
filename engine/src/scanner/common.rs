//! Common scanner utilities
//!
//! This module provides shared utilities for detecting binary files,
//! used by both CLI and bindings scanners.

use std::path::Path;

/// List of known binary file extensions
///
/// Comprehensive list including executables, compiled code, archives,
/// media files, documents, fonts, and databases.
pub const BINARY_EXTENSIONS: &[&str] = &[
    // Executables and libraries
    "exe", "dll", "so", "dylib", "a", "o", "obj", "lib", // Compiled bytecode
    "pyc", "pyo", "class", "jar", "war", "ear", // Archives
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "tgz", // Images
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg", "tiff", "psd",
    // Audio/Video
    "mp3", "mp4", "avi", "mov", "wav", "flac", "ogg", "webm", "mkv", // Documents
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", // Fonts
    "woff", "woff2", "ttf", "eot", "otf", // Database
    "db", "sqlite", "sqlite3", // Misc binary
    "bin", "dat", "cache",
];

/// Check if a file path has a known binary extension
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// `true` if the file extension is in the known binary list
///
/// # Example
/// ```
/// use infiniloom_engine::scanner::is_binary_extension;
/// use std::path::Path;
///
/// assert!(is_binary_extension(Path::new("image.png")));
/// assert!(is_binary_extension(Path::new("archive.zip")));
/// assert!(!is_binary_extension(Path::new("code.rs")));
/// ```
pub fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    BINARY_EXTENSIONS.contains(&ext.as_str())
}

/// Check if content appears to be binary by examining bytes
///
/// Uses a heuristic: if more than 10% of the first 8KB contains
/// null bytes or other control characters, the file is considered binary.
///
/// # Arguments
/// * `content` - Byte slice to check (typically first 8KB of file)
///
/// # Returns
/// `true` if the content appears to be binary
///
/// # Example
/// ```
/// use infiniloom_engine::scanner::is_binary_content;
///
/// // Text content
/// assert!(!is_binary_content(b"fn main() { println!(\"hello\"); }"));
///
/// // Binary content (has null bytes)
/// assert!(is_binary_content(&[0x00, 0x01, 0x02, 0x00, 0x00]));
/// ```
pub fn is_binary_content(content: &[u8]) -> bool {
    // Check first 8KB for binary indicators
    let check_len = content.len().min(8192);
    let sample = &content[..check_len];

    if sample.is_empty() {
        return false;
    }

    // Count null bytes and non-printable characters
    let binary_chars = sample
        .iter()
        .filter(|&&b| {
            // Null byte or control char (except common whitespace)
            b == 0 || (b < 32 && b != b'\n' && b != b'\r' && b != b'\t')
        })
        .count();

    // If more than 10% are binary characters, consider it binary
    let threshold = sample.len() / 10;
    binary_chars > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_extension_executables() {
        assert!(is_binary_extension(Path::new("program.exe")));
        assert!(is_binary_extension(Path::new("lib.dll")));
        assert!(is_binary_extension(Path::new("library.so")));
        assert!(is_binary_extension(Path::new("framework.dylib")));
    }

    #[test]
    fn test_binary_extension_archives() {
        assert!(is_binary_extension(Path::new("archive.zip")));
        assert!(is_binary_extension(Path::new("backup.tar")));
        assert!(is_binary_extension(Path::new("compressed.gz")));
        assert!(is_binary_extension(Path::new("package.7z")));
    }

    #[test]
    fn test_binary_extension_images() {
        assert!(is_binary_extension(Path::new("photo.jpg")));
        assert!(is_binary_extension(Path::new("logo.png")));
        assert!(is_binary_extension(Path::new("icon.gif")));
        assert!(is_binary_extension(Path::new("image.webp")));
    }

    #[test]
    fn test_binary_extension_media() {
        assert!(is_binary_extension(Path::new("song.mp3")));
        assert!(is_binary_extension(Path::new("video.mp4")));
        assert!(is_binary_extension(Path::new("movie.mkv")));
    }

    #[test]
    fn test_binary_extension_documents() {
        assert!(is_binary_extension(Path::new("doc.pdf")));
        assert!(is_binary_extension(Path::new("spreadsheet.xlsx")));
        assert!(is_binary_extension(Path::new("presentation.pptx")));
    }

    #[test]
    fn test_binary_extension_fonts() {
        assert!(is_binary_extension(Path::new("font.woff")));
        assert!(is_binary_extension(Path::new("font.woff2")));
        assert!(is_binary_extension(Path::new("font.ttf")));
    }

    #[test]
    fn test_binary_extension_database() {
        assert!(is_binary_extension(Path::new("data.db")));
        assert!(is_binary_extension(Path::new("store.sqlite")));
        assert!(is_binary_extension(Path::new("cache.sqlite3")));
    }

    #[test]
    fn test_non_binary_extensions() {
        assert!(!is_binary_extension(Path::new("code.rs")));
        assert!(!is_binary_extension(Path::new("script.py")));
        assert!(!is_binary_extension(Path::new("module.ts")));
        assert!(!is_binary_extension(Path::new("style.css")));
        assert!(!is_binary_extension(Path::new("data.json")));
        assert!(!is_binary_extension(Path::new("config.yaml")));
        assert!(!is_binary_extension(Path::new("readme.md")));
    }

    #[test]
    fn test_no_extension() {
        assert!(!is_binary_extension(Path::new("Makefile")));
        assert!(!is_binary_extension(Path::new("Dockerfile")));
        assert!(!is_binary_extension(Path::new(".gitignore")));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(is_binary_extension(Path::new("FILE.PNG")));
        assert!(is_binary_extension(Path::new("Archive.ZIP")));
        assert!(is_binary_extension(Path::new("Video.MP4")));
    }

    #[test]
    fn test_binary_content_text() {
        // Normal text content
        assert!(!is_binary_content(b"fn main() {\n    println!(\"hello\");\n}"));
        assert!(!is_binary_content(b"Hello, World!\n"));
        assert!(!is_binary_content(b"def foo():\n    return 42\n"));
    }

    #[test]
    fn test_binary_content_with_nulls() {
        // Content with null bytes (clearly binary)
        let binary = vec![0u8; 100];
        assert!(is_binary_content(&binary));

        // Mixed content with many nulls
        let mut mixed = b"some text".to_vec();
        mixed.extend(vec![0u8; 100]);
        assert!(is_binary_content(&mixed));
    }

    #[test]
    fn test_binary_content_control_chars() {
        // Content with control characters
        let control: Vec<u8> = (0..32)
            .filter(|&b| b != b'\n' && b != b'\r' && b != b'\t')
            .collect();
        let mut content = control.repeat(10);
        content.extend(b"some text");
        // This should be detected as binary due to many control chars
        assert!(is_binary_content(&content));
    }

    #[test]
    fn test_binary_content_empty() {
        assert!(!is_binary_content(b""));
    }

    #[test]
    fn test_binary_content_whitespace_ok() {
        // Content with tabs, newlines, carriage returns should be fine
        assert!(!is_binary_content(b"line1\nline2\r\nline3\ttabbed"));
    }
}
