//! Repository scanner for Infiniloom CLI
//!
//! Performance notes:
//! - Uses `ignore` crate for fast gitignore-respecting file walking
//! - File reading and parsing are parallelized with rayon
//! - Thread-local parsers enable lock-free parallel tree-sitter parsing
//! - Optional pipelined mode overlaps I/O with CPU using crossbeam channels
//! - Use --skip-symbols for 80x speedup on large repos

use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread;

use infiniloom_engine::dependencies::DependencyGraph;
use infiniloom_engine::mmap_scanner::MappedFile;
use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::{LanguageStats, RepoFile, RepoMetadata, Repository, TokenCounts};

/// Threshold for using memory-mapped I/O (files >= 1MB use mmap)
const MMAP_THRESHOLD: u64 = 1024 * 1024;

// Thread-local parser for each rayon worker
// This avoids mutex contention by giving each thread its own parser
thread_local! {
    static THREAD_PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
}

/// Parse content using thread-local parser (lock-free)
fn parse_with_thread_local(content: &str, path: &Path) -> Vec<infiniloom_engine::types::Symbol> {
    THREAD_PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = Language::from_extension(ext) {
                parser.parse(content, lang).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    })
}

/// Configuration for repository scanning
pub(crate) struct ScanConfig {
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Read file contents
    pub read_contents: bool,
    /// Maximum file size to include (bytes)
    pub max_file_size: u64,
    /// Skip symbol extraction (faster for large repos)
    pub skip_symbols: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: true,
            read_contents: false,
            max_file_size: 50 * 1024 * 1024, // 50MB
            skip_symbols: false,
        }
    }
}

/// File info collected during initial walk
#[derive(Clone)]
struct FileInfo {
    path: PathBuf,
    relative_path: String,
    size_bytes: u64,
    language: Option<String>,
}

/// File content ready for parsing (used in pipelined mode)
struct FileContent {
    info: FileInfo,
    content: String,
}

/// Minimum number of files to trigger pipelined mode
const PIPELINE_THRESHOLD: usize = 100;

/// Scan a repository with cache support, skipping unchanged files
///
/// This function checks each file against the cache and only re-processes files
/// that have changed (different mtime or size). Unchanged files use cached data.
pub(crate) fn scan_repository_with_cache(
    path: &Path,
    config: ScanConfig,
    cache: &infiniloom_engine::RepoCache,
) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Phase 1: Collect file paths (fast, sequential walk with ignore filtering)
    let file_infos = collect_file_infos(&path, &config)?;

    // Phase 2: Partition files into cached (unchanged) and needs-rescan
    let mut cached_files: Vec<RepoFile> = Vec::new();
    let mut files_to_scan: Vec<FileInfo> = Vec::new();

    for info in file_infos {
        // Get file mtime
        let mtime = std::fs::metadata(&info.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let cached = cache.get(&info.relative_path);
        let mut needs_rescan = cache.needs_rescan(&info.relative_path, mtime, info.size_bytes);

        if !needs_rescan {
            if let Some(cached) = cached {
                // If symbols are required but were never extracted, force rescan
                if !config.skip_symbols && !cached.symbols_extracted {
                    needs_rescan = true;
                }

                let mut content = None;
                if !needs_rescan && config.read_contents {
                    content = smart_read_file(&info.path, cached.size);
                    if let Some(ref content_str) = content {
                        if cached.hash != 0 {
                            let content_hash = infiniloom_engine::incremental::hash_content(
                                content_str.as_bytes(),
                            );
                            if cache.needs_rescan_with_hash(
                                &info.relative_path,
                                mtime,
                                info.size_bytes,
                                content_hash,
                            ) {
                                needs_rescan = true;
                                content = None;
                            }
                        }
                    }
                }

                if needs_rescan {
                    files_to_scan.push(info);
                } else {
                    cached_files.push(RepoFile {
                        path: info.path,
                        relative_path: info.relative_path,
                        language: cached.language.clone(),
                        size_bytes: cached.size,
                        token_count: TokenCounts {
                            o200k: cached.tokens.o200k,
                            cl100k: cached.tokens.cl100k,
                            claude: cached.tokens.claude,
                            gemini: cached.tokens.gemini,
                            llama: cached.tokens.llama,
                            mistral: cached.tokens.mistral,
                            deepseek: cached.tokens.deepseek,
                            qwen: cached.tokens.qwen,
                            cohere: cached.tokens.cohere,
                            grok: cached.tokens.grok,
                        },
                        symbols: cached.symbols.iter().map(|s| s.into()).collect(),
                        importance: 0.5,
                        content,
                    });
                }
            } else {
                // File in walk but not in cache - need to scan
                files_to_scan.push(info);
            }
        } else {
            files_to_scan.push(info);
        }
    }

    // Phase 3: Process only changed files
    let mut scanned_files: Vec<RepoFile> = if config.read_contents {
        if config.skip_symbols {
            files_to_scan
                .into_par_iter()
                .filter_map(process_file_content_only)
                .collect()
        } else if files_to_scan.len() >= PIPELINE_THRESHOLD {
            scan_files_pipelined(files_to_scan)?
        } else {
            files_to_scan
                .into_par_iter()
                .filter_map(process_file_with_content)
                .collect()
        }
    } else {
        files_to_scan
            .into_iter()
            .map(process_file_without_content)
            .collect()
    };

    // Merge cached and scanned files
    scanned_files.extend(cached_files);
    let files = scanned_files;

    // Phase 4: Aggregate statistics (same as regular scan)
    let total_files = files.len() as u32;
    let total_lines: u64 = files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(f.size_bytes))
        })
        .sum();

    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new();
    for file in &files {
        if let Some(ref lang) = file.language {
            let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1;
            let file_lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(file.size_bytes));
            entry.1 += file_lines;
        }
    }

    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, (count, lines))| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    languages.sort_by(|a, b| b.files.cmp(&a.files));

    let total_tokens = TokenCounts {
        o200k: files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
        mistral: files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: files.iter().map(|f| f.token_count.cohere).sum(),
        grok: files.iter().map(|f| f.token_count.grok).sum(),
    };

    let branch = detect_git_branch(&path);
    let commit = detect_git_commit(&path);
    let directory_structure = generate_directory_structure(&files);

    let temp_repo = Repository {
        name: repo_name.clone(),
        path: path.clone(),
        files: files.clone(),
        metadata: RepoMetadata::default(),
    };
    let dep_graph = DependencyGraph::build(&temp_repo);
    let mut external_dependencies: Vec<String> =
        dep_graph.get_external_deps().iter().cloned().collect();
    external_dependencies.sort();

    Ok(Repository {
        name: repo_name,
        path,
        files,
        metadata: RepoMetadata {
            total_files,
            total_lines,
            total_tokens,
            languages,
            framework: None,
            description: None,
            branch,
            commit,
            directory_structure: Some(directory_structure),
            external_dependencies,
            git_history: None,
        },
    })
}

/// Scan a repository and return a Repository struct
/// Uses parallel processing for improved performance on large repositories
///
/// For large repositories (>100 files), uses a pipelined architecture with channels
/// to overlap I/O with CPU-intensive parsing work.
pub(crate) fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned();

    // Phase 1: Collect file paths (fast, sequential walk with ignore filtering)
    let file_infos = collect_file_infos(&path, &config)?;

    // Phase 2: Process files
    // For large repos with symbol extraction, use pipelined architecture
    // For small repos or skip_symbols mode, use simpler parallel processing
    let files: Vec<RepoFile> = if config.read_contents {
        if config.skip_symbols {
            // Without symbols, parallelize freely (no parser needed)
            file_infos
                .into_par_iter()
                .filter_map(process_file_content_only)
                .collect()
        } else if file_infos.len() >= PIPELINE_THRESHOLD {
            // Large repo with symbols: use pipelined architecture
            scan_files_pipelined(file_infos)?
        } else {
            // Small repo with symbols: use thread-local parsers
            file_infos
                .into_par_iter()
                .filter_map(process_file_with_content)
                .collect()
        }
    } else {
        // Sequential is fine when just collecting metadata (CPU bound, fast)
        file_infos
            .into_iter()
            .map(process_file_without_content)
            .collect()
    };

    // Phase 3: Aggregate statistics
    let total_files = files.len() as u32;
    let total_lines: u64 = files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(f.size_bytes))
        })
        .sum();

    // Track both file counts and line counts per language
    let mut language_counts: HashMap<String, (u32, u64)> = HashMap::new();
    for file in &files {
        if let Some(ref lang) = file.language {
            let entry = language_counts.entry(lang.clone()).or_insert((0, 0));
            entry.0 += 1; // file count
                          // Calculate lines from content if available, otherwise estimate
            let file_lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| estimate_lines(file.size_bytes));
            entry.1 += file_lines; // line count
        }
    }

    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, (count, lines))| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    // Sort by file count descending so primary language (first) is deterministic
    languages.sort_by(|a, b| b.files.cmp(&a.files));

    let total_tokens = TokenCounts {
        o200k: files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: files.iter().map(|f| f.token_count.claude).sum(),
        gemini: files.iter().map(|f| f.token_count.gemini).sum(),
        llama: files.iter().map(|f| f.token_count.llama).sum(),
        mistral: files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: files.iter().map(|f| f.token_count.cohere).sum(),
        grok: files.iter().map(|f| f.token_count.grok).sum(),
    };

    let branch = detect_git_branch(&path);
    let commit = detect_git_commit(&path);
    let directory_structure = generate_directory_structure(&files);

    // Build dependency graph and extract external dependencies
    let temp_repo = Repository {
        name: repo_name.clone(),
        path: path.clone(),
        files: files.clone(),
        metadata: RepoMetadata::default(),
    };
    let dep_graph = DependencyGraph::build(&temp_repo);
    let mut external_dependencies: Vec<String> =
        dep_graph.get_external_deps().iter().cloned().collect();
    external_dependencies.sort();

    Ok(Repository {
        name: repo_name,
        path,
        files,
        metadata: RepoMetadata {
            total_files,
            total_lines,
            total_tokens,
            languages,
            framework: None,
            description: None,
            branch,
            commit,
            directory_structure: Some(directory_structure),
            external_dependencies,
            git_history: None,
        },
    })
}

/// Collect file information (paths, sizes) without reading content
fn collect_file_infos(base_path: &Path, config: &ScanConfig) -> Result<Vec<FileInfo>> {
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

        let language = detect_language(entry_path);

        file_infos.push(FileInfo {
            path: entry_path.to_path_buf(),
            relative_path,
            size_bytes,
            language,
        });
    }

    Ok(file_infos)
}

/// Pipelined file scanning with overlapped I/O and parsing
///
/// Architecture:
/// - Reader threads: Read file contents from disk, send to channel
/// - Parser threads: Receive content from channel, parse symbols, send results
/// - Aggregator: Collect results into final Vec<RepoFile>
///
/// This overlaps I/O wait time with CPU-intensive parsing for better throughput
/// on large repositories.
fn scan_files_pipelined(file_infos: Vec<FileInfo>) -> Result<Vec<RepoFile>> {
    // Channel capacity balances memory usage vs throughput
    // Too small = pipeline stalls, too large = memory bloat
    let channel_capacity = 64;

    // Channel from reader -> parsers (file content)
    let (content_tx, content_rx): (Sender<FileContent>, Receiver<FileContent>) =
        bounded(channel_capacity);

    // Channel from parsers -> aggregator (parsed files)
    let (result_tx, result_rx): (Sender<RepoFile>, Receiver<RepoFile>) = bounded(channel_capacity);

    let file_count = file_infos.len();

    // Spawn reader threads (I/O bound - use more threads)
    let num_readers = 4.min(file_count.saturating_sub(1).div_ceil(25) + 1);
    let chunk_size = file_count.div_ceil(num_readers);

    // Track errors across threads
    let error_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // Collect failed file paths for better error reporting (limit to first 10)
    let failed_files = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    let reader_handles: Vec<_> = file_infos
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .map(|chunk| {
            let tx = content_tx.clone();
            let files = chunk.to_vec();
            let errors = std::sync::Arc::clone(&error_count);
            let failed = std::sync::Arc::clone(&failed_files);
            thread::spawn(move || {
                for info in files {
                    // Smart read: uses mmap for large files (>= 1MB)
                    match smart_read_file(&info.path, info.size_bytes) {
                        Some(content) => {
                            // Send to parser, track send errors
                            if tx.send(FileContent { info, content }).is_err() {
                                errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            }
                        },
                        None => {
                            // File read failed (permissions, encoding, binary, etc.)
                            errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            // Collect failed file paths (limit to 10)
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
    let num_parsers = rayon::current_num_threads().min(8);
    let parser_error_count = std::sync::Arc::clone(&error_count);
    let parser_handles: Vec<_> = (0..num_parsers)
        .map(|_| {
            let rx = content_rx.clone();
            let tx = result_tx.clone();
            let errors = std::sync::Arc::clone(&parser_error_count);
            thread::spawn(move || {
                // Each parser thread has its own parser instance
                let mut parser = Parser::new();

                while let Ok(file_content) = rx.recv() {
                    let FileContent { info, content } = file_content;

                    // Estimate tokens
                    let token_count = estimate_tokens(info.size_bytes, Some(&content));

                    // Parse symbols
                    let symbols = if let Some(ext) = info.path.extension().and_then(|e| e.to_str())
                    {
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
                        size_bytes: info.size_bytes,
                        token_count,
                        symbols,
                        importance: 0.5,
                        content: Some(content),
                    };

                    // Send result, track errors
                    if tx.send(repo_file).is_err() {
                        errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    let total_errors = error_count.load(std::sync::atomic::Ordering::Relaxed);
    if total_errors > 0 || thread_panics > 0 {
        eprintln!(
            "Warning: {} file(s) could not be processed, {} thread(s) panicked",
            total_errors, thread_panics
        );
        // Show details of failed files
        if let Ok(guard) = failed_files.lock() {
            if !guard.is_empty() {
                eprintln!("Failed files:");
                for path in guard.iter() {
                    eprintln!("  - {}", path);
                }
                if total_errors > guard.len() {
                    eprintln!("  ... and {} more", total_errors - guard.len());
                }
            }
        }
    }

    Ok(files)
}

/// Smart file reading that uses mmap for large files
/// Files >= MMAP_THRESHOLD (1MB) use memory-mapped I/O for better performance
fn smart_read_file(path: &Path, size_bytes: u64) -> Option<String> {
    if size_bytes >= MMAP_THRESHOLD {
        // Use memory-mapped I/O for large files
        let mapped = match MappedFile::open(path) {
            Ok(m) => m,
            Err(e) => {
                log::debug!("Failed to mmap file {}: {}", path.display(), e);
                return None;
            },
        };
        if mapped.is_binary() {
            log::debug!("Skipping binary file: {}", path.display());
            return None;
        }
        match mapped.as_str() {
            Some(s) => Some(s.to_owned()),
            None => {
                log::debug!("File is not valid UTF-8: {}", path.display());
                None
            },
        }
    } else {
        // Use regular read for small files
        match std::fs::read_to_string(path) {
            Ok(content) => Some(content),
            Err(e) => {
                log::debug!("Failed to read file {}: {}", path.display(), e);
                None
            },
        }
    }
}

/// Process a file with content reading only (no parsing - fast path)
fn process_file_content_only(info: FileInfo) -> Option<RepoFile> {
    let content = smart_read_file(&info.path, info.size_bytes)?;
    let token_count = estimate_tokens(info.size_bytes, Some(&content));

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file with content reading and parsing (used in parallel)
/// Uses thread-local parser for lock-free parallel parsing
/// Uses memory-mapped I/O for files >= 1MB
fn process_file_with_content(info: FileInfo) -> Option<RepoFile> {
    // Smart read: uses mmap for large files
    let content = smart_read_file(&info.path, info.size_bytes)?;

    // Estimate tokens from actual content
    let token_count = estimate_tokens(info.size_bytes, Some(&content));

    // Parse symbols using thread-local parser (lock-free)
    let symbols = parse_with_thread_local(&content, &info.path);

    Some(RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols,
        importance: 0.5,
        content: Some(content),
    })
}

/// Process a file without reading content (fast path)
fn process_file_without_content(info: FileInfo) -> RepoFile {
    let token_count = estimate_tokens(info.size_bytes, None);

    RepoFile {
        path: info.path,
        relative_path: info.relative_path,
        language: info.language,
        size_bytes: info.size_bytes,
        token_count,
        symbols: Vec::new(),
        importance: 0.5,
        content: None,
    }
}

/// Estimate tokens from file size
fn estimate_tokens(size_bytes: u64, content: Option<&str>) -> TokenCounts {
    let size = size_bytes as f32;

    // If we have content, count more accurately
    if let Some(text) = content {
        let len = text.len() as f32;
        return TokenCounts {
            o200k: (len / 4.0) as u32,  // OpenAI modern (GPT-5.x, GPT-4o, O-series)
            cl100k: (len / 3.7) as u32, // OpenAI legacy (GPT-4, GPT-3.5)
            claude: (len / 3.5) as u32,
            gemini: (len / 3.8) as u32,
            llama: (len / 3.5) as u32,
            mistral: (len / 3.5) as u32,
            deepseek: (len / 3.5) as u32,
            qwen: (len / 3.5) as u32,
            cohere: (len / 3.6) as u32,
            grok: (len / 3.5) as u32,
        };
    }

    // Otherwise estimate from file size
    TokenCounts {
        o200k: (size / 4.0) as u32,
        cl100k: (size / 3.7) as u32,
        claude: (size / 3.5) as u32,
        gemini: (size / 3.8) as u32,
        llama: (size / 3.5) as u32,
        mistral: (size / 3.5) as u32,
        deepseek: (size / 3.5) as u32,
        qwen: (size / 3.5) as u32,
        cohere: (size / 3.6) as u32,
        grok: (size / 3.5) as u32,
    }
}

/// Estimate lines from file size
fn estimate_lines(size_bytes: u64) -> u64 {
    // Average ~40 characters per line
    size_bytes / 40
}

/// Detect programming language from file extension or filename
fn detect_language(path: &Path) -> Option<String> {
    // First, check for well-known filenames without extensions
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        match filename.to_lowercase().as_str() {
            // Docker
            "dockerfile" | "dockerfile.dev" | "dockerfile.prod" | "dockerfile.test" => {
                return Some("dockerfile".to_owned())
            },
            // Make
            "makefile" | "gnumakefile" | "bsdmakefile" => return Some("make".to_owned()),
            // Ruby
            "gemfile" | "rakefile" | "guardfile" | "vagrantfile" | "berksfile" | "podfile"
            | "fastfile" | "appfile" | "matchfile" | "deliverfile" | "snapfile" => {
                return Some("ruby".to_owned())
            },
            // Shell
            ".bashrc" | ".bash_profile" | ".zshrc" | ".zprofile" | ".profile" | ".bash_aliases" => {
                return Some("shell".to_owned())
            },
            // Git
            ".gitignore" | ".gitattributes" | ".gitmodules" => return Some("gitignore".to_owned()),
            // Editor config
            ".editorconfig" => return Some("editorconfig".to_owned()),
            // Procfile (Heroku)
            "procfile" => return Some("procfile".to_owned()),
            // Justfile
            "justfile" => return Some("just".to_owned()),
            // Caddyfile
            "caddyfile" => return Some("caddyfile".to_owned()),
            // Brewfile
            "brewfile" => return Some("ruby".to_owned()),
            _ => {},
        };
        // Check for patterns like Dockerfile.something
        if filename.to_lowercase().starts_with("dockerfile") {
            return Some("dockerfile".to_owned());
        }
        if filename.to_lowercase().starts_with("makefile") {
            return Some("make".to_owned());
        }
    }

    // Then check extensions
    let ext = path.extension()?.to_str()?;

    let lang = match ext.to_lowercase().as_str() {
        // Python
        "py" | "pyi" | "pyx" => "python",

        // JavaScript/TypeScript
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",

        // Rust
        "rs" => "rust",

        // Go
        "go" => "go",

        // Java/JVM
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "groovy" => "groovy",
        "clj" | "cljs" | "cljc" => "clojure",

        // C/C++
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",

        // C#
        "cs" => "csharp",

        // Ruby
        "rb" | "rake" | "gemspec" => "ruby",

        // PHP
        "php" => "php",

        // Swift
        "swift" => "swift",

        // Shell
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "ps1" | "psm1" => "powershell",

        // Web
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",

        // Data/Config
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "ini" | "cfg" => "ini",

        // Documentation
        "md" | "markdown" => "markdown",
        "mdx" => "mdx",
        "rst" => "rst",
        "txt" => "text",

        // Zig
        "zig" => "zig",

        // Lua
        "lua" => "lua",

        // SQL
        "sql" => "sql",

        // Elixir/Erlang
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",

        // Haskell
        "hs" | "lhs" => "haskell",

        // OCaml/F#
        "ml" | "mli" => "ocaml",
        "fs" | "fsi" | "fsx" => "fsharp",

        // Vue/Svelte
        "vue" => "vue",
        "svelte" => "svelte",

        // Docker
        "dockerfile" => "dockerfile",

        // Terraform
        "tf" | "tfvars" => "terraform",

        // Makefile-like
        "makefile" | "mk" => "make",
        "cmake" => "cmake",

        // Nix
        "nix" => "nix",

        // Julia
        "jl" => "julia",

        // R
        "r" | "rmd" => "r",

        // Dart
        "dart" => "dart",

        // Nim
        "nim" => "nim",

        // V
        "v" => "vlang",

        // Crystal
        "cr" => "crystal",

        _ => return None,
    };

    Some(lang.to_owned())
}

/// Check if file has a binary extension
fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        // Executables
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "lib" |
        // Compiled
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" |
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" | "tiff" | "psd" |
        // Audio/Video
        "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac" | "ogg" | "webm" | "mkv" |
        // Documents
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" |
        // Fonts
        "woff" | "woff2" | "ttf" | "eot" | "otf" |
        // Database
        "db" | "sqlite" | "sqlite3" |
        // Misc binary
        "bin" | "dat" | "cache"
    )
}

/// Detect current git branch
fn detect_git_branch(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: refs/heads/") {
        Some(
            content
                .trim_start_matches("ref: refs/heads/")
                .trim()
                .to_owned(),
        )
    } else {
        // Detached HEAD - safely take first 7 characters
        Some(content.trim().chars().take(7).collect())
    }
}

/// Detect current git commit
fn detect_git_commit(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: ") {
        // Follow ref
        let ref_path = content.trim_start_matches("ref: ").trim();
        let full_path = path.join(".git").join(ref_path);
        std::fs::read_to_string(full_path).ok().map(|s| {
            // Safely take first 7 characters without panicking on short strings
            s.trim().chars().take(7).collect()
        })
    } else {
        // Detached HEAD - content is the commit hash
        // Safely take first 7 characters without panicking on short strings
        Some(content.trim().chars().take(7).collect())
    }
}

/// Generate a tree-like directory structure from file paths
fn generate_directory_structure(files: &[RepoFile]) -> String {
    use std::collections::BTreeSet;

    // Collect all unique directory paths
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let mut file_set: BTreeSet<&str> = BTreeSet::new();

    for file in files {
        file_set.insert(&file.relative_path);

        // Add all parent directories
        let mut current = file.relative_path.as_str();
        while let Some(idx) = current.rfind('/') {
            current = &current[..idx];
            if !current.is_empty() {
                dirs.insert(current.to_owned());
            }
        }
    }

    // Build tree structure
    let mut output = String::new();
    let mut printed: BTreeSet<String> = BTreeSet::new();

    // Sort all paths (dirs first, then files at each level)
    let mut all_paths: Vec<(&str, bool)> = Vec::new();
    for dir in &dirs {
        all_paths.push((dir, true));
    }
    for file in files {
        all_paths.push((&file.relative_path, false));
    }
    all_paths.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.0.split('/').collect();
        let b_parts: Vec<&str> = b.0.split('/').collect();
        a_parts.cmp(&b_parts)
    });

    for (path, is_dir) in all_paths {
        let parts: Vec<&str> = path.split('/').collect();
        let depth = parts.len() - 1;

        // Print parent directories if not printed
        let mut parent_path = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i < parts.len() - 1 {
                if !parent_path.is_empty() {
                    parent_path.push('/');
                }
                parent_path.push_str(part);

                if !printed.contains(&parent_path) {
                    let indent = "  ".repeat(i);
                    output.push_str(&format!("{}{}/\n", indent, part));
                    printed.insert(parent_path.clone());
                }
            }
        }

        // Print the item itself
        if !is_dir {
            let name = parts.last().unwrap_or(&"");
            let indent = "  ".repeat(depth);
            output.push_str(&format!("{}{}\n", indent, name));
        }
    }

    // Limit size for very large repos
    if output.len() > 50000 {
        let truncated: String = output.chars().take(49000).collect();
        format!("{}...\n[Directory structure truncated - {} files total]", truncated, files.len())
    } else {
        output
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(&PathBuf::from("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.ts")), Some("typescript".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test")), None);
    }

    #[test]
    fn test_is_binary_extension() {
        assert!(is_binary_extension(&PathBuf::from("test.exe")));
        assert!(is_binary_extension(&PathBuf::from("test.png")));
        assert!(!is_binary_extension(&PathBuf::from("test.rs")));
        assert!(!is_binary_extension(&PathBuf::from("test.py")));
    }

    #[test]
    fn test_estimate_tokens() {
        let tokens = estimate_tokens(1000, None);
        assert!(tokens.claude > 0);
        assert!(tokens.o200k > 0);
    }
}
