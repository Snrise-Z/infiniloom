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
    use tempfile::NamedTempFile;

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
    fn test_binary_detection() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&[0x00, 0x01, 0x02, 0x03]).unwrap();

        let mapped = MappedFile::open(temp.path()).unwrap();
        assert!(mapped.is_binary());
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
    fn test_detect_language() {
        assert_eq!(detect_language(Path::new("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(Path::new("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(Path::new("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language(Path::new("test.unknown")), None);
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
}
