//! Pipelined file scanning for large repositories
//!
//! This module implements a pipelined architecture that overlaps I/O with CPU work:
//!
//! ```text
//! [Reader Threads] --> [Channel] --> [Parser Threads] --> [Channel] --> [Aggregator]
//!     (I/O bound)                       (CPU bound)
//! ```
//!
//! Benefits:
//! - Overlaps disk I/O wait time with CPU-intensive parsing
//! - Scales to many CPU cores
//! - Better throughput on large repositories (100+ files)

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::parser::{Language, Parser};
use crate::tokenizer::Tokenizer;
use crate::types::RepoFile;

use super::io::smart_read_file_with_options;
use super::process::estimate_tokens;
use super::{FileInfo, ScannerConfig};

/// Channel capacity - balances memory usage vs throughput
const CHANNEL_CAPACITY: usize = 64;

/// Maximum reader threads (I/O bound - can use more)
const MAX_READERS: usize = 4;

/// Maximum parser threads (CPU bound - limited by cores)
const MAX_PARSERS: usize = 8;

/// File content ready for parsing
struct FileContent {
    info: FileInfo,
    content: String,
}

/// Pipelined file scanning with overlapped I/O and parsing
///
/// Architecture:
/// - Reader threads: Read file contents from disk, send to channel
/// - Parser threads: Receive content from channel, parse symbols, send results
/// - Aggregator: Collect results into final Vec<RepoFile>
///
/// # Arguments
/// * `file_infos` - Files to process
/// * `config` - Scanner configuration
///
/// # Returns
/// Vector of processed RepoFile structs
pub fn scan_files_pipelined(
    file_infos: Vec<FileInfo>,
    config: &ScannerConfig,
) -> Result<Vec<RepoFile>> {
    // Handle empty input
    if file_infos.is_empty() {
        return Ok(Vec::new());
    }

    // Channel from reader -> parsers (file content)
    let (content_tx, content_rx): (Sender<FileContent>, Receiver<FileContent>) =
        bounded(CHANNEL_CAPACITY);

    // Channel from parsers -> aggregator (parsed files)
    let (result_tx, result_rx): (Sender<RepoFile>, Receiver<RepoFile>) = bounded(CHANNEL_CAPACITY);

    let file_count = file_infos.len();

    // Calculate number of reader threads (I/O bound - use more threads)
    let num_readers = MAX_READERS.min(file_count.saturating_sub(1).div_ceil(25) + 1);
    let chunk_size = file_count.div_ceil(num_readers);

    // Track errors across threads
    let error_count = Arc::new(AtomicUsize::new(0));
    // Collect failed file paths for better error reporting (limit to first 10)
    let failed_files = Arc::new(Mutex::new(Vec::<String>::new()));

    // Clone config values needed by threads
    let use_mmap = config.use_mmap;
    let accurate_tokens = config.accurate_tokens;
    let skip_symbols = config.skip_symbols;

    // Spawn reader threads
    let reader_handles: Vec<_> = file_infos
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .map(|chunk| {
            let tx = content_tx.clone();
            let files = chunk.to_vec();
            let errors = Arc::clone(&error_count);
            let failed = Arc::clone(&failed_files);
            thread::spawn(move || {
                for info in files {
                    let size_bytes = info.size_bytes.unwrap_or(0);
                    match smart_read_file_with_options(&info.path, size_bytes, use_mmap) {
                        Some(content) => {
                            if tx.send(FileContent { info, content }).is_err() {
                                errors.fetch_add(1, Ordering::Relaxed);
                            }
                        },
                        None => {
                            errors.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut guard) = failed.lock() {
                                if guard.len() < 10 {
                                    guard.push(info.relative_path);
                                }
                            }
                        },
                    }
                }
            })
        })
        .collect();

    // Drop original sender so channel closes when readers finish
    drop(content_tx);

    // Spawn parser threads (CPU bound - use rayon thread count)
    let num_parsers = rayon::current_num_threads().min(MAX_PARSERS);
    let parser_handles: Vec<_> = (0..num_parsers)
        .map(|_| {
            let rx = content_rx.clone();
            let tx = result_tx.clone();
            let errors = Arc::clone(&error_count);
            thread::spawn(move || {
                // Each parser thread has its own parser instance
                let mut parser = Parser::new();
                let tokenizer = Tokenizer::new();

                while let Ok(file_content) = rx.recv() {
                    let FileContent { info, content } = file_content;
                    let size_bytes = info.size_bytes.unwrap_or(0);

                    // Count tokens using configured method
                    let token_count = if accurate_tokens {
                        tokenizer.count_all(&content)
                    } else {
                        estimate_tokens(size_bytes, Some(&content))
                    };

                    // Parse symbols unless skipped
                    let symbols = if skip_symbols {
                        Vec::new()
                    } else if let Some(ext) = info.path.extension().and_then(|e| e.to_str()) {
                        if let Some(lang) = Language::from_extension(ext) {
                            parser.parse(&content, lang).unwrap_or_default()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    let repo_file = RepoFile {
                        path: info.path,
                        relative_path: info.relative_path,
                        language: info.language,
                        size_bytes,
                        token_count,
                        symbols,
                        importance: 0.5,
                        content: Some(content),
                    };

                    if tx.send(repo_file).is_err() {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    // Drop cloned receivers/senders
    drop(content_rx);
    drop(result_tx);

    // Aggregator: collect all results
    let files: Vec<RepoFile> = result_rx.iter().collect();

    // Wait for all threads to finish and track any panics
    let mut thread_panics = 0;
    for handle in reader_handles {
        if handle.join().is_err() {
            thread_panics += 1;
        }
    }
    for handle in parser_handles {
        if handle.join().is_err() {
            thread_panics += 1;
        }
    }

    // Report any errors that occurred during scanning
    let total_errors = error_count.load(Ordering::Relaxed);
    if total_errors > 0 || thread_panics > 0 {
        tracing::warn!(
            "{} file(s) could not be processed, {} thread(s) panicked",
            total_errors,
            thread_panics
        );
        if let Ok(guard) = failed_files.lock() {
            for path in guard.iter() {
                tracing::debug!("Failed to process: {}", path);
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_scan_files_pipelined_empty() {
        let config = ScannerConfig::default();
        let files = scan_files_pipelined(vec![], &config).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_files_pipelined_single_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let infos = vec![FileInfo {
            path: file_path.clone(),
            relative_path: "test.rs".to_string(),
            size_bytes: Some(12),
            language: Some("rust".to_string()),
        }];

        let config = ScannerConfig::default();
        let files = scan_files_pipelined(infos, &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].content.is_some());
    }

    #[test]
    fn test_scan_files_pipelined_multiple_files() {
        let dir = tempdir().unwrap();

        // Create multiple files
        for i in 0..10 {
            let file_path = dir.path().join(format!("test{}.rs", i));
            fs::write(&file_path, format!("fn func{}() {{}}", i)).unwrap();
        }

        let infos: Vec<FileInfo> = (0..10)
            .map(|i| FileInfo {
                path: dir.path().join(format!("test{}.rs", i)),
                relative_path: format!("test{}.rs", i),
                size_bytes: Some(15),
                language: Some("rust".to_string()),
            })
            .collect();

        let config = ScannerConfig::default();
        let files = scan_files_pipelined(infos, &config).unwrap();

        assert_eq!(files.len(), 10);
    }

    #[test]
    fn test_scan_files_pipelined_skip_symbols() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let infos = vec![FileInfo {
            path: file_path.clone(),
            relative_path: "test.rs".to_string(),
            size_bytes: Some(12),
            language: Some("rust".to_string()),
        }];

        let config = ScannerConfig { skip_symbols: true, ..Default::default() };
        let files = scan_files_pipelined(infos, &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].symbols.is_empty());
    }

    #[test]
    fn test_scan_files_pipelined_accurate_tokens() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let infos = vec![FileInfo {
            path: file_path.clone(),
            relative_path: "test.rs".to_string(),
            size_bytes: Some(12),
            language: Some("rust".to_string()),
        }];

        let config = ScannerConfig { accurate_tokens: true, ..Default::default() };
        let files = scan_files_pipelined(infos, &config).unwrap();

        assert_eq!(files.len(), 1);
        // Token count should be populated
        assert!(files[0].token_count.o200k > 0);
    }
}
