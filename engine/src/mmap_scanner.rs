//! Memory-mapped file scanner for high-performance large repository scanning
//!
//! Uses memory-mapped I/O to avoid copying file contents into memory,
//! enabling efficient scanning of very large files and repositories.

use memmap2::{Mmap, MmapOptions};
use rayon::prelude::*;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::tokenizer::{TokenCounts, TokenModel, Tokenizer};

/// A memory-mapped file for efficient reading
pub struct MappedFile {
    mmap: Mmap,
    path: String,
}

impl MappedFile {
    /// Open a file with memory mapping
    #[allow(unsafe_code)]
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        // SAFETY: mapping is read-only and the file remains open for the mmap lifetime.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(Self { mmap, path: path.to_string_lossy().to_string() })
    }

    /// Get the file contents as a byte slice
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Get the file contents as a string (if valid UTF-8)
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.mmap).ok()
    }

    /// Get file size
    #[inline]
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Get the file path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Check if content appears to be binary
    pub fn is_binary(&self) -> bool {
        // Check first 8KB for binary indicators
        let check_len = self.mmap.len().min(8192);
        let sample = &self.mmap[..check_len];

        // Null bytes indicate binary
        if sample.contains(&0) {
            return true;
        }

        // High ratio of non-printable characters
        let non_printable = sample
            .iter()
            .filter(|&&b| b < 32 && b != b'\t' && b != b'\n' && b != b'\r')
            .count();

        non_printable * 10 > check_len
    }

    /// Count lines efficiently using SIMD-friendly iteration
    pub fn count_lines(&self) -> usize {
        self.mmap.iter().filter(|&&b| b == b'\n').count()
    }
}

/// High-performance scanner using memory-mapped files
pub struct MmapScanner {
    /// Minimum file size to use mmap (smaller files use regular read)
    mmap_threshold: u64,
    /// Maximum file size to process
    max_file_size: u64,
    /// Tokenizer for counting
    tokenizer: Tokenizer,
    /// Statistics
    stats: ScanStats,
}

/// Scanning statistics
#[derive(Debug, Default)]
pub struct ScanStats {
    pub files_scanned: AtomicU64,
    pub bytes_read: AtomicU64,
    pub files_skipped_binary: AtomicU64,
    pub files_skipped_size: AtomicU64,
    pub mmap_used: AtomicU64,
    pub regular_read_used: AtomicU64,
}

impl ScanStats {
    pub fn summary(&self) -> String {
        format!(
            "Scanned {} files ({} bytes), skipped {} binary + {} oversized, mmap: {}, regular: {}",
            self.files_scanned.load(Ordering::Relaxed),
            self.bytes_read.load(Ordering::Relaxed),
            self.files_skipped_binary.load(Ordering::Relaxed),
            self.files_skipped_size.load(Ordering::Relaxed),
            self.mmap_used.load(Ordering::Relaxed),
            self.regular_read_used.load(Ordering::Relaxed),
        )
    }
}

/// Result of scanning a single file
#[derive(Debug)]
pub struct ScannedFile {
    pub path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub lines: usize,
    pub token_counts: TokenCounts,
    pub language: Option<String>,
    pub content: Option<String>,
    pub is_binary: bool,
}

impl MmapScanner {
    /// Create a new scanner with default settings
    pub fn new() -> Self {
        Self {
            mmap_threshold: 64 * 1024,       // 64KB
            max_file_size: 50 * 1024 * 1024, // 50MB
            tokenizer: Tokenizer::new(),
            stats: ScanStats::default(),
        }
    }

    /// Set minimum file size for memory mapping
    pub fn with_mmap_threshold(mut self, bytes: u64) -> Self {
        self.mmap_threshold = bytes;
        self
    }

    /// Set maximum file size
    pub fn with_max_file_size(mut self, bytes: u64) -> Self {
        self.max_file_size = bytes;
        self
    }

    /// Scan a single file
    pub fn scan_file(&self, path: &Path, base_path: &Path) -> io::Result<Option<ScannedFile>> {
        let metadata = path.metadata()?;
        let size = metadata.len();

        // Skip files over max size
        if size > self.max_file_size {
            self.stats
                .files_skipped_size
                .fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }

        let relative_path = path
            .strip_prefix(base_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Choose reading strategy based on file size
        let (content_bytes, _use_mmap) = if size >= self.mmap_threshold {
            self.stats.mmap_used.fetch_add(1, Ordering::Relaxed);
            let mapped = MappedFile::open(path)?;

            // Check for binary
            if mapped.is_binary() {
                self.stats
                    .files_skipped_binary
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }

            (mapped.as_bytes().to_vec(), true)
        } else {
            self.stats.regular_read_used.fetch_add(1, Ordering::Relaxed);
            let content = std::fs::read(path)?;

            // Check for binary
            if is_binary_content(&content) {
                self.stats
                    .files_skipped_binary
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }

            (content, false)
        };

        // Convert to string
        let content_str = match String::from_utf8(content_bytes) {
            Ok(s) => s,
            Err(_) => {
                self.stats
                    .files_skipped_binary
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            },
        };

        // Count tokens
        let token_counts = self.tokenizer.count_all(&content_str);

        // Count lines
        let lines = content_str.lines().count();

        // Detect language
        let language = detect_language(path);

        self.stats.files_scanned.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_read.fetch_add(size, Ordering::Relaxed);

        Ok(Some(ScannedFile {
            path: path.to_string_lossy().to_string(),
            relative_path,
            size_bytes: size,
            lines,
            token_counts,
            language,
            content: Some(content_str),
            is_binary: false,
        }))
    }

    /// Scan multiple files in parallel
    pub fn scan_files_parallel(&self, paths: &[&Path], base_path: &Path) -> Vec<ScannedFile> {
        paths
            .par_iter()
            .filter_map(|path| match self.scan_file(path, base_path) {
                Ok(Some(file)) => Some(file),
                Ok(None) => None,
                Err(e) => {
                    tracing::debug!("Error scanning {:?}: {}", path, e);
                    None
                },
            })
            .collect()
    }

    /// Get scanning statistics
    pub fn stats(&self) -> &ScanStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.files_scanned.store(0, Ordering::Relaxed);
        self.stats.bytes_read.store(0, Ordering::Relaxed);
        self.stats.files_skipped_binary.store(0, Ordering::Relaxed);
        self.stats.files_skipped_size.store(0, Ordering::Relaxed);
        self.stats.mmap_used.store(0, Ordering::Relaxed);
        self.stats.regular_read_used.store(0, Ordering::Relaxed);
    }
}

impl Default for MmapScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick binary check for content
fn is_binary_content(content: &[u8]) -> bool {
    let check_len = content.len().min(8192);
    let sample = &content[..check_len];

    if sample.contains(&0) {
        return true;
    }

    let non_printable = sample
        .iter()
        .filter(|&&b| b < 32 && b != b'\t' && b != b'\n' && b != b'\r')
        .count();

    non_printable * 10 > check_len
}

/// Detect language from file extension
fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;

    let lang = match ext.to_lowercase().as_str() {
        "py" | "pyw" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "sh" | "bash" => "bash",
        "lua" => "lua",
        "zig" => "zig",
        "md" | "markdown" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "sql" => "sql",
        _ => return None,
    };

    Some(lang.to_owned())
}

/// Streaming content processor for very large files
pub struct StreamingProcessor {
    chunk_size: usize,
    tokenizer: Tokenizer,
}

impl StreamingProcessor {
    /// Create a new streaming processor
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size, tokenizer: Tokenizer::new() }
    }

    /// Process a file in chunks, yielding partial results
    pub fn process_file<F>(&self, path: &Path, mut callback: F) -> io::Result<()>
    where
        F: FnMut(&str, usize, TokenCounts),
    {
        let mapped = MappedFile::open(path)?;

        if mapped.is_binary() {
            return Ok(());
        }

        let content = mapped
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;

        let mut offset = 0;
        while offset < content.len() {
            let end = (offset + self.chunk_size).min(content.len());

            // Find line boundary
            let chunk_end = if end < content.len() {
                content[offset..end]
                    .rfind('\n')
                    .map(|i| offset + i + 1)
                    .unwrap_or(end)
            } else {
                end
            };

            let chunk = &content[offset..chunk_end];
            let tokens = self.tokenizer.count_all(chunk);

            callback(chunk, offset, tokens);

            offset = chunk_end;
        }

        Ok(())
    }

    /// Estimate total tokens without loading full content
    pub fn estimate_tokens(&self, path: &Path, model: TokenModel) -> io::Result<u32> {
        let metadata = path.metadata()?;
        let size = metadata.len();

        // Quick estimation based on file size
        let chars_per_token = model.chars_per_token();
        Ok((size as f32 / chars_per_token).ceil() as u32)
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_mapped_file() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Hello, World!").unwrap();
        writeln!(temp, "Second line").unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();

        assert!(!mapped.is_empty());
        assert!(!mapped.is_binary());
        assert_eq!(mapped.count_lines(), 2);
    }

    #[test]
    fn test_mapped_file_as_str() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Valid UTF-8 content").unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        let content = mapped.as_str();
        assert!(content.is_some());
        assert!(content.unwrap().contains("Valid UTF-8"));
    }

    #[test]
    fn test_mapped_file_len_and_path() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Test content").unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(mapped.len() > 0);
        assert!(!mapped.path().is_empty());
        assert!(mapped
            .path()
            .contains(temp.path().file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    fn test_mapped_file_as_bytes() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"Raw bytes").unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        let bytes = mapped.as_bytes();
        assert_eq!(&bytes[..9], b"Raw bytes");
    }

    #[test]
    fn test_mapped_file_empty() {
        let temp = NamedTempFile::new().unwrap();
        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(mapped.is_empty());
        assert_eq!(mapped.len(), 0);
        assert_eq!(mapped.count_lines(), 0);
    }

    #[test]
    fn test_mapped_file_invalid_utf8() {
        let mut temp = NamedTempFile::new().unwrap();
        // Write invalid UTF-8 sequence
        temp.write_all(&[0xFF, 0xFE, 0x41, 0x42]).unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        // as_str returns None for invalid UTF-8
        assert!(mapped.as_str().is_none());
    }

    #[test]
    fn test_binary_detection() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00, 0x01, 0x02, 0x03]).unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(mapped.is_binary());
    }

    #[test]
    fn test_binary_detection_high_non_printable() {
        let mut temp = NamedTempFile::new().unwrap();
        // Write many non-printable chars (not null)
        let mut content = vec![0x01u8; 100];
        content.extend(b"some text"); // Add some text to avoid null detection
        temp.write_all(&content).unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(mapped.is_binary());
    }

    #[test]
    fn test_binary_detection_text_with_tabs() {
        let mut temp = NamedTempFile::new().unwrap();
        // Text with tabs and newlines should NOT be binary
        writeln!(temp, "Line 1\twith\ttabs").unwrap();
        writeln!(temp, "Line 2\twith\ttabs").unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(!mapped.is_binary());
    }

    #[test]
    fn test_scanner() {
        let mut temp = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(temp, "def hello():").unwrap();
        writeln!(temp, "    print('hello')").unwrap();

        let scanner = MmapScanner::new();
        let result = scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(result.is_some());
        let file = result.unwrap();
        assert_eq!(file.language, Some("python".to_string()));
        assert!(file.token_counts.claude > 0);
    }

    #[test]
    fn test_scanner_default() {
        let scanner = MmapScanner::default();
        // Should have same settings as new()
        assert_eq!(scanner.mmap_threshold, 64 * 1024);
        assert_eq!(scanner.max_file_size, 50 * 1024 * 1024);
    }

    #[test]
    fn test_scanner_with_thresholds() {
        let scanner = MmapScanner::new()
            .with_mmap_threshold(1024)
            .with_max_file_size(1024 * 1024);
        assert_eq!(scanner.mmap_threshold, 1024);
        assert_eq!(scanner.max_file_size, 1024 * 1024);
    }

    #[test]
    fn test_scanner_skips_large_files() {
        let mut temp = NamedTempFile::new().unwrap();
        // Write content that would be under the threshold
        writeln!(temp, "Small content").unwrap();

        // Set max file size very small
        let scanner = MmapScanner::new().with_max_file_size(5);
        let result = scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(result.is_none());
        assert_eq!(scanner.stats().files_skipped_size.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_scanner_skips_binary_files() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00, 0x01, 0x02, 0x03]).unwrap();

        let scanner = MmapScanner::new();
        let result = scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(result.is_none());
        assert_eq!(scanner.stats().files_skipped_binary.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_scanner_uses_mmap_for_large_files() {
        let mut temp = NamedTempFile::with_suffix(".rs").unwrap();
        // Write content larger than default mmap threshold (64KB)
        let content = "fn test() {}\n".repeat(10000);
        temp.write_all(content.as_bytes()).unwrap();

        let scanner = MmapScanner::new().with_mmap_threshold(1024); // Set low threshold
        let result = scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(result.is_some());
        assert!(scanner.stats().mmap_used.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_scanner_uses_regular_read_for_small_files() {
        let mut temp = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(temp, "x = 1").unwrap();

        let scanner = MmapScanner::new().with_mmap_threshold(1024 * 1024); // High threshold
        let result = scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(result.is_some());
        assert_eq!(scanner.stats().regular_read_used.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_scanner_reset_stats() {
        let mut temp = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(temp, "x = 1").unwrap();

        let scanner = MmapScanner::new();
        scanner
            .scan_file(temp.path(), temp.path().parent().unwrap())
            .unwrap();

        assert!(scanner.stats().files_scanned.load(Ordering::Relaxed) >= 1);

        scanner.reset_stats();

        assert_eq!(scanner.stats().files_scanned.load(Ordering::Relaxed), 0);
        assert_eq!(scanner.stats().bytes_read.load(Ordering::Relaxed), 0);
        assert_eq!(scanner.stats().files_skipped_binary.load(Ordering::Relaxed), 0);
        assert_eq!(scanner.stats().files_skipped_size.load(Ordering::Relaxed), 0);
        assert_eq!(scanner.stats().mmap_used.load(Ordering::Relaxed), 0);
        assert_eq!(scanner.stats().regular_read_used.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_scan_stats_summary() {
        let stats = ScanStats::default();
        stats.files_scanned.store(10, Ordering::Relaxed);
        stats.bytes_read.store(5000, Ordering::Relaxed);
        stats.files_skipped_binary.store(2, Ordering::Relaxed);
        stats.files_skipped_size.store(1, Ordering::Relaxed);
        stats.mmap_used.store(5, Ordering::Relaxed);
        stats.regular_read_used.store(5, Ordering::Relaxed);

        let summary = stats.summary();
        assert!(summary.contains("10 files"));
        assert!(summary.contains("5000 bytes"));
        assert!(summary.contains("2 binary"));
        assert!(summary.contains("1 oversized"));
        assert!(summary.contains("mmap: 5"));
        assert!(summary.contains("regular: 5"));
    }

    #[test]
    fn test_scan_files_parallel() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("test1.py");
        let file2 = dir.path().join("test2.rs");
        let file3 = dir.path().join("binary.bin");

        std::fs::write(&file1, "def foo(): pass\n").unwrap();
        std::fs::write(&file2, "fn main() {}\n").unwrap();
        std::fs::write(&file3, &[0x00, 0x01, 0x02]).unwrap(); // Binary

        let scanner = MmapScanner::new();
        let paths: Vec<&Path> = vec![file1.as_path(), file2.as_path(), file3.as_path()];
        let results = scanner.scan_files_parallel(&paths, dir.path());

        // Should get 2 files (binary skipped)
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .any(|f| f.language == Some("python".to_string())));
        assert!(results
            .iter()
            .any(|f| f.language == Some("rust".to_string())));
    }

    #[test]
    fn test_scan_files_parallel_with_errors() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("test.py");
        std::fs::write(&file1, "x = 1\n").unwrap();

        let scanner = MmapScanner::new();
        let nonexistent = Path::new("/nonexistent/file.py");
        let paths: Vec<&Path> = vec![file1.as_path(), nonexistent];
        let results = scanner.scan_files_parallel(&paths, dir.path());

        // Should get 1 file (nonexistent skipped with error)
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(Path::new("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(Path::new("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("test.unknown")), None);
    }

    #[test]
    fn test_detect_language_all_extensions() {
        // Python
        assert_eq!(detect_language(Path::new("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("test.pyw")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("test.pyi")), Some("python".to_string()));

        // JavaScript
        assert_eq!(detect_language(Path::new("test.js")), Some("javascript".to_string()));
        assert_eq!(detect_language(Path::new("test.mjs")), Some("javascript".to_string()));
        assert_eq!(detect_language(Path::new("test.cjs")), Some("javascript".to_string()));
        assert_eq!(detect_language(Path::new("test.jsx")), Some("jsx".to_string()));

        // TypeScript
        assert_eq!(detect_language(Path::new("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("test.mts")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("test.cts")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("test.tsx")), Some("tsx".to_string()));

        // Systems languages
        assert_eq!(detect_language(Path::new("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(Path::new("test.go")), Some("go".to_string()));
        assert_eq!(detect_language(Path::new("test.c")), Some("c".to_string()));
        assert_eq!(detect_language(Path::new("test.h")), Some("c".to_string()));
        assert_eq!(detect_language(Path::new("test.cpp")), Some("cpp".to_string()));
        assert_eq!(detect_language(Path::new("test.hpp")), Some("cpp".to_string()));
        assert_eq!(detect_language(Path::new("test.cc")), Some("cpp".to_string()));
        assert_eq!(detect_language(Path::new("test.cxx")), Some("cpp".to_string()));
        assert_eq!(detect_language(Path::new("test.zig")), Some("zig".to_string()));

        // JVM languages
        assert_eq!(detect_language(Path::new("test.java")), Some("java".to_string()));
        assert_eq!(detect_language(Path::new("test.kt")), Some("kotlin".to_string()));
        assert_eq!(detect_language(Path::new("test.kts")), Some("kotlin".to_string()));
        assert_eq!(detect_language(Path::new("test.scala")), Some("scala".to_string()));

        // Other languages
        assert_eq!(detect_language(Path::new("test.cs")), Some("csharp".to_string()));
        assert_eq!(detect_language(Path::new("test.rb")), Some("ruby".to_string()));
        assert_eq!(detect_language(Path::new("test.php")), Some("php".to_string()));
        assert_eq!(detect_language(Path::new("test.swift")), Some("swift".to_string()));
        assert_eq!(detect_language(Path::new("test.lua")), Some("lua".to_string()));

        // Shell
        assert_eq!(detect_language(Path::new("test.sh")), Some("bash".to_string()));
        assert_eq!(detect_language(Path::new("test.bash")), Some("bash".to_string()));

        // Markup/Data
        assert_eq!(detect_language(Path::new("test.md")), Some("markdown".to_string()));
        assert_eq!(detect_language(Path::new("test.markdown")), Some("markdown".to_string()));
        assert_eq!(detect_language(Path::new("test.json")), Some("json".to_string()));
        assert_eq!(detect_language(Path::new("test.yaml")), Some("yaml".to_string()));
        assert_eq!(detect_language(Path::new("test.yml")), Some("yaml".to_string()));
        assert_eq!(detect_language(Path::new("test.toml")), Some("toml".to_string()));
        assert_eq!(detect_language(Path::new("test.xml")), Some("xml".to_string()));
        assert_eq!(detect_language(Path::new("test.html")), Some("html".to_string()));
        assert_eq!(detect_language(Path::new("test.htm")), Some("html".to_string()));
        assert_eq!(detect_language(Path::new("test.css")), Some("css".to_string()));
        assert_eq!(detect_language(Path::new("test.scss")), Some("scss".to_string()));
        assert_eq!(detect_language(Path::new("test.sass")), Some("scss".to_string()));
        assert_eq!(detect_language(Path::new("test.sql")), Some("sql".to_string()));

        // No extension
        assert_eq!(detect_language(Path::new("Makefile")), None);
        assert_eq!(detect_language(Path::new("README")), None);
    }

    #[test]
    fn test_detect_language_case_insensitive() {
        // Extensions should be case-insensitive
        assert_eq!(detect_language(Path::new("test.PY")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("test.RS")), Some("rust".to_string()));
        assert_eq!(detect_language(Path::new("test.Js")), Some("javascript".to_string()));
    }

    #[test]
    fn test_is_binary_content() {
        // Text content should not be binary
        assert!(!is_binary_content(b"Hello, world!\n"));
        assert!(!is_binary_content(b"Line 1\nLine 2\nLine 3\n"));
        assert!(!is_binary_content(b"Tab\tseparated\tvalues\n"));

        // Null bytes indicate binary
        assert!(is_binary_content(&[0x00, 0x01, 0x02]));
        assert!(is_binary_content(b"text\x00with\x00nulls"));

        // High non-printable ratio indicates binary
        let mostly_binary: Vec<u8> = (0u8..100).collect();
        assert!(is_binary_content(&mostly_binary));
    }

    #[test]
    fn test_streaming_processor() {
        let mut temp = NamedTempFile::new().unwrap();
        for i in 0..100 {
            writeln!(temp, "Line {}: Some content here", i).unwrap();
        }

        let processor = StreamingProcessor::new(256);
        let mut chunks = 0;

        processor
            .process_file(temp.path(), |_chunk, _offset, _tokens| {
                chunks += 1;
            })
            .unwrap();

        assert!(chunks > 1);
    }

    #[test]
    fn test_streaming_processor_single_chunk() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Short content").unwrap();

        let processor = StreamingProcessor::new(1024 * 1024); // Large chunk size
        let mut chunks = 0;
        let mut total_offset = 0;

        processor
            .process_file(temp.path(), |_chunk, offset, _tokens| {
                chunks += 1;
                total_offset = offset;
            })
            .unwrap();

        assert_eq!(chunks, 1);
        assert_eq!(total_offset, 0);
    }

    #[test]
    fn test_streaming_processor_binary_file() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00, 0x01, 0x02]).unwrap();

        let processor = StreamingProcessor::new(256);
        let mut chunks = 0;

        // Should return Ok(()) but not call callback for binary files
        processor
            .process_file(temp.path(), |_chunk, _offset, _tokens| {
                chunks += 1;
            })
            .unwrap();

        assert_eq!(chunks, 0);
    }

    #[test]
    fn test_streaming_processor_estimate_tokens() {
        let mut temp = NamedTempFile::new().unwrap();
        let content = "x".repeat(1000);
        temp.write_all(content.as_bytes()).unwrap();

        let processor = StreamingProcessor::new(256);
        let estimate = processor
            .estimate_tokens(temp.path(), TokenModel::Claude)
            .unwrap();

        // Claude has ~4 chars per token, so 1000 chars should be ~250 tokens
        assert!(estimate > 0);
        assert!(estimate < 500);
    }

    #[test]
    fn test_scanned_file_struct() {
        let file = ScannedFile {
            path: "/tmp/test.py".to_string(),
            relative_path: "test.py".to_string(),
            size_bytes: 100,
            lines: 10,
            token_counts: TokenCounts::default(),
            language: Some("python".to_string()),
            content: Some("x = 1".to_string()),
            is_binary: false,
        };

        assert_eq!(file.path, "/tmp/test.py");
        assert_eq!(file.relative_path, "test.py");
        assert_eq!(file.size_bytes, 100);
        assert_eq!(file.lines, 10);
        assert!(!file.is_binary);
    }

    #[test]
    fn test_mapped_file_open_nonexistent() {
        let result = MappedFile::open(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_nonexistent_file() {
        let scanner = MmapScanner::new();
        let result =
            scanner.scan_file(Path::new("/nonexistent/file.py"), Path::new("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_streaming_processor_invalid_utf8() {
        let mut temp = NamedTempFile::new().unwrap();
        // Write invalid UTF-8 that's not binary (no nulls, not high non-printable ratio)
        // This is tricky - we need bytes that:
        // 1. Don't contain null (0x00)
        // 2. Don't have high non-printable ratio
        // 3. Are invalid UTF-8
        // Write text with an invalid UTF-8 sequence embedded
        temp.write_all(b"Hello ").unwrap();
        temp.write_all(&[0xFF, 0xFE]).unwrap(); // Invalid UTF-8
        temp.write_all(b" World").unwrap();

        let processor = StreamingProcessor::new(256);
        let result = processor.process_file(temp.path(), |_, _, _| {});

        // Should return an error for invalid UTF-8
        assert!(result.is_err());
    }
}
