//! Repository scanner for Node.js bindings
//!
//! This is a pure Rust scanner similar to the CLI's scanner, adapted for Node.js bindings.

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::tokenizer::Tokenizer;
use infiniloom_engine::types::{LanguageStats, RepoFile, RepoMetadata, Repository, TokenCounts};

// Thread-local parser for each rayon worker
// This avoids mutex contention by giving each thread its own parser
thread_local! {
    static THREAD_PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
    static THREAD_TOKENIZER: Tokenizer = Tokenizer::new();
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

/// Count tokens using thread-local tokenizer (accurate via tiktoken)
fn count_tokens_accurate(content: &str) -> TokenCounts {
    THREAD_TOKENIZER.with(|tokenizer| {
        let counts = tokenizer.count_all(content);
        TokenCounts {
            o200k: counts.o200k,
            cl100k: counts.cl100k,
            claude: counts.claude,
            gemini: counts.gemini,
            llama: counts.llama,
            mistral: counts.mistral,
            deepseek: counts.deepseek,
            qwen: counts.qwen,
            cohere: counts.cohere,
            grok: counts.grok,
        }
    })
}

/// Configuration for repository scanning
pub struct ScanConfig {
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
    path: std::path::PathBuf,
    relative_path: String,
    size_bytes: u64,
    language: Option<String>,
}

/// Scan a repository and return a Repository struct
pub fn scan_repository(path: &Path, config: ScanConfig) -> Result<Repository> {
    let path = path.canonicalize().context("Invalid repository path")?;

    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();

    // Phase 1: Collect file info (single-threaded walk)
    let mut file_infos: Vec<FileInfo> = Vec::new();

    let walker = WalkBuilder::new(&path)
        .hidden(!config.include_hidden)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        // Skip directories
        if !entry_path.is_file() {
            continue;
        }

        // Check file size
        let metadata = entry_path.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        if size_bytes > config.max_file_size {
            continue;
        }

        // Skip binary files
        if is_binary_extension(entry_path) {
            continue;
        }

        // Get relative path
        let relative_path = entry_path
            .strip_prefix(&path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        // Detect language
        let language = detect_language(entry_path);

        file_infos.push(FileInfo {
            path: entry_path.to_path_buf(),
            relative_path,
            size_bytes,
            language,
        });
    }

    // Phase 2: Process files in parallel (read content, parse symbols, count tokens)
    let skip_symbols = config.skip_symbols;
    let read_contents = config.read_contents;

    let files: Vec<RepoFile> = file_infos
        .into_par_iter()
        .filter_map(|info| {
            // Read content if requested
            let content = if read_contents {
                std::fs::read_to_string(&info.path).ok()
            } else {
                None
            };

            // Count tokens accurately if we have content, otherwise estimate
            let token_count = if let Some(ref text) = content {
                count_tokens_accurate(text)
            } else {
                estimate_tokens(info.size_bytes, None)
            };

            // Extract symbols if enabled and we have content
            let symbols = if !skip_symbols {
                if let Some(ref text) = content {
                    parse_with_thread_local(text, &info.path)
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            Some(RepoFile {
                path: info.path,
                relative_path: info.relative_path,
                language: info.language,
                size_bytes: info.size_bytes,
                token_count,
                symbols,
                importance: 0.5, // Default importance
                content,
            })
        })
        .collect();

    // Calculate language counts and total lines from processed files
    let mut language_counts: HashMap<String, u32> = HashMap::new();
    let mut total_lines: u64 = 0;

    for file in &files {
        if let Some(ref lang) = file.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
        }
        let lines = file
            .content
            .as_ref()
            .map(|c| c.lines().count() as u64)
            .unwrap_or_else(|| estimate_lines(file.size_bytes));
        total_lines += lines;
    }

    // Calculate language statistics
    let total_files = files.len() as u32;
    let languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats {
                language: lang,
                files: count,
                lines: 0, // Would need per-language line counting
                percentage,
            }
        })
        .collect();

    // Calculate total tokens
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
            directory_structure: None,
            external_dependencies: Vec::new(),
            git_history: None,
        },
    })
}

/// Estimate tokens from file size
fn estimate_tokens(size_bytes: u64, content: Option<&str>) -> TokenCounts {
    let size = size_bytes as f32;

    // If we have content, count more accurately
    if let Some(text) = content {
        let len = text.len() as f32;
        return TokenCounts {
            o200k: (len / 4.0) as u32,      // OpenAI modern (GPT-4o, O1, etc.)
            cl100k: (len / 3.7) as u32,     // OpenAI legacy (GPT-4)
            claude: (len / 3.5) as u32,
            gemini: (len / 3.8) as u32,
            llama: (len / 3.5) as u32,
            mistral: (len / 3.5) as u32,
            deepseek: (len / 3.5) as u32,
            qwen: (len / 3.5) as u32,
            cohere: (len / 3.5) as u32,
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
        cohere: (size / 3.5) as u32,
        grok: (size / 3.5) as u32,
    }
}

/// Estimate lines from file size
fn estimate_lines(size_bytes: u64) -> u64 {
    // Average ~40 characters per line
    size_bytes / 40
}

/// Detect programming language from file extension
fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;

    let lang = match ext.to_lowercase().as_str() {
        "py" | "pyi" | "pyx" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" | "rake" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "sh" | "bash" => "bash",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "md" | "markdown" => "markdown",
        "sql" => "sql",
        "zig" => "zig",
        _ => return None,
    };

    Some(lang.to_string())
}

/// Check if file has a binary extension
fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "lib" |
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" |
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" |
        "mp3" | "mp4" | "avi" | "mov" | "wav" |
        "pdf" | "doc" | "docx" | "xls" | "xlsx" |
        "woff" | "woff2" | "ttf" | "eot" |
        "db" | "sqlite" | "sqlite3"
    )
}

/// Detect current git branch
fn detect_git_branch(path: &Path) -> Option<String> {
    let head_path = path.join(".git/HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;

    if content.starts_with("ref: refs/heads/") {
        Some(content.trim_start_matches("ref: refs/heads/").trim().to_owned())
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
        std::fs::read_to_string(full_path)
            .ok()
            .map(|s| s.trim().chars().take(7).collect())
    } else {
        // Detached HEAD - safely take first 7 characters
        Some(content.trim().chars().take(7).collect())
    }
}
