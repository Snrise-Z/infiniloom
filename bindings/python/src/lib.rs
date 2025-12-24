//! Python bindings for Infiniloom
//!
//! This module provides Python bindings using PyO3 for the Infiniloom engine.

#![allow(non_local_definitions)]

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;

// Import from infiniloom-bindings-common
use infiniloom_bindings_common::{
    // Repository operations
    apply_compression,
    apply_default_ignores,
    file_priority_score,
    format_file_status,
    parse_compression,
    parse_format,
    parse_model,
    prepare_repository,
    // Scanner from common crate
    scan_repository,
    ScanConfig,
};

// Import from infiniloom-engine
use infiniloom_engine::{
    git::{
        ChangedFile, DiffHunk as EngineGitDiffHunk, FileStatus as EngineFileStatus,
        GitRepo as EngineGitRepo,
    },
    // Index module
    index::{
        // Call graph query API
        find_symbol as engine_find_symbol,
        get_call_graph as engine_get_call_graph,
        get_call_graph_filtered,
        get_callees_by_name,
        get_callers_by_name,
        get_references_by_name,
        BuildOptions,
        CallGraph as EngineCallGraph,
        ChangeType,
        ContextDepth,
        ContextExpander,
        DiffChange,
        IndexBuilder,
        IndexStorage,
        ReferenceInfo as EngineReferenceInfo,
        SymbolInfo as EngineSymbolInfo,
    },
    tokenizer::TokenModel,
    ChunkStrategy,
    // Chunking module
    Chunker,
    OutputFormatter,
    RepoMapGenerator,
    Repository,
    SecurityScanner,
    SemanticCompressor,
    SemanticConfig,
    Tokenizer,
};

// Python exception for Infiniloom errors
pyo3::create_exception!(infiniloom, InfiniloomError, pyo3::exceptions::PyException);

/// Convert Rust errors to Python exceptions
fn to_py_err(err: impl std::fmt::Display) -> PyErr {
    InfiniloomError::new_err(format!("{}", err))
}

/// Pack a repository into an LLM-optimized format
///
/// Args:
///     path: Path to the repository
///     format: Output format ("xml", "markdown", "json", "yaml", "toon", "plain")
///     model: Target LLM model ("gpt-5.2", "gpt-5.1", "gpt-5", "o3", "gpt-4o", "claude", "gemini", "llama", etc.)
///     compression: Compression level ("none", "minimal", "balanced", "aggressive", "extreme", "focused", "semantic")
///     map_budget: Token budget for repository map (default: 2000)
///     max_symbols: Maximum number of symbols to include (default: 50)
///     redact_secrets: Redact detected secrets in output (default: True)
///     skip_symbols: Skip symbol extraction for faster scanning (default: False)
///
/// Returns:
///     Formatted repository context as a string
///
/// Example:
///     >>> import infiniloom
///     >>> context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
///     >>> print(context)
#[pyfunction]
#[pyo3(signature = (path, format="xml", model="claude", compression="balanced", map_budget=2000, max_symbols=50, redact_secrets=true, skip_symbols=false))]
fn pack(
    path: &str,
    format: &str,
    model: &str,
    compression: &str,
    map_budget: u32,
    max_symbols: usize,
    redact_secrets: bool,
    skip_symbols: bool,
) -> PyResult<String> {
    // Parse format using common crate
    let output_format =
        parse_format(Some(format)).map_err(|e| PyValueError::new_err(e.to_string()))?;

    // Parse model using common crate
    let tokenizer_model =
        parse_model(Some(model)).map_err(|e| PyValueError::new_err(e.to_string()))?;

    // Parse compression level using common crate
    let compression_level =
        parse_compression(Some(compression)).map_err(|e| PyValueError::new_err(e.to_string()))?;

    // Scan repository
    let path_buf = PathBuf::from(path);
    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024, // 50MB
        skip_symbols,
    };

    let mut repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    // Apply default ignores to filter out build outputs, dependencies, test fixtures, etc.
    apply_default_ignores(&mut repo);

    // Prepare repository (count references, rank files, sort by importance)
    prepare_repository(&mut repo);

    // Redact secrets from file content if enabled
    if redact_secrets {
        infiniloom_bindings_common::redact_secrets(&mut repo);
    }

    // Apply compression to file contents based on compression level
    apply_compression(&mut repo, compression_level);

    // Generate repository map using builder pattern
    let generator = RepoMapGenerator::builder()
        .token_budget(map_budget)
        .max_symbols(max_symbols)
        .model(tokenizer_model)
        .build();
    let map = generator.generate(&repo);

    // Format output
    let formatter = OutputFormatter::by_format_with_model(output_format, tokenizer_model);
    let output = formatter.format(&repo, &map);

    Ok(output)
}

/// Scan a repository and return statistics
///
/// Args:
///     path: Path to the repository
///     include_hidden: Include hidden files (default: False)
///     respect_gitignore: Respect .gitignore files (default: True)
///
/// Returns:
///     Dictionary with repository statistics
///
/// Example:
///     >>> import infiniloom
///     >>> stats = infiniloom.scan("/path/to/repo")
///     >>> print(stats["total_files"])
#[pyfunction]
#[pyo3(signature = (path, include_hidden=false, respect_gitignore=true))]
fn scan(
    py: Python,
    path: &str,
    include_hidden: bool,
    respect_gitignore: bool,
) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);

    // Check if path exists
    if !path_buf.exists() {
        return Err(InfiniloomError::new_err(format!("Path does not exist: {}", path)));
    }

    let config = ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: true, // Must be true to get line counts and language stats
        max_file_size: 50 * 1024 * 1024,
        skip_symbols: true, // Fast mode for scan stats
    };

    let repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    // Convert to Python dict
    let dict = PyDict::new(py);
    dict.set_item("name", repo.name)?;
    dict.set_item("path", repo.path.to_string_lossy().to_string())?;
    dict.set_item("total_files", repo.metadata.total_files)?;
    dict.set_item("total_lines", repo.metadata.total_lines)?;

    // Token counts
    let tokens = PyDict::new(py);
    tokens.set_item("o200k", repo.metadata.total_tokens.o200k)?;
    tokens.set_item("cl100k", repo.metadata.total_tokens.cl100k)?;
    tokens.set_item("claude", repo.metadata.total_tokens.claude)?;
    tokens.set_item("gemini", repo.metadata.total_tokens.gemini)?;
    tokens.set_item("llama", repo.metadata.total_tokens.llama)?;
    tokens.set_item("mistral", repo.metadata.total_tokens.mistral)?;
    tokens.set_item("deepseek", repo.metadata.total_tokens.deepseek)?;
    tokens.set_item("qwen", repo.metadata.total_tokens.qwen)?;
    tokens.set_item("cohere", repo.metadata.total_tokens.cohere)?;
    tokens.set_item("grok", repo.metadata.total_tokens.grok)?;
    dict.set_item("total_tokens", tokens)?;

    // Languages
    let languages = PyList::new(
        py,
        repo.metadata.languages.iter().map(|lang| {
            let lang_dict = PyDict::new(py);
            lang_dict.set_item("language", &lang.language).unwrap();
            lang_dict.set_item("files", lang.files).unwrap();
            lang_dict.set_item("lines", lang.lines).unwrap();
            lang_dict.set_item("percentage", lang.percentage).unwrap();
            lang_dict
        }),
    );
    dict.set_item("languages", languages)?;

    // Optional metadata
    if let Some(branch) = repo.metadata.branch {
        dict.set_item("branch", branch)?;
    }
    if let Some(commit) = repo.metadata.commit {
        dict.set_item("commit", commit)?;
    }
    if let Some(framework) = repo.metadata.framework {
        dict.set_item("framework", framework)?;
    }

    Ok(dict.into())
}

/// Count tokens in text for a specific model
///
/// Args:
///     text: Text to count tokens for
///     model: Target LLM model ("claude", "gpt", "gpt4o", "gemini", "llama", etc.)
///
/// Returns:
///     Number of tokens (exact for OpenAI models via tiktoken, calibrated estimates for others)
///
/// Example:
///     >>> import infiniloom
///     >>> tokens = infiniloom.count_tokens("Hello, world!", model="claude")
///     >>> print(tokens)
#[pyfunction]
#[pyo3(signature = (text, model="claude"))]
fn count_tokens(text: &str, model: &str) -> PyResult<u32> {
    // Use the engine's accurate tokenizer (tiktoken for OpenAI, calibrated estimates for others)
    let token_model = parse_model(Some(model)).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let tokenizer = Tokenizer::new();
    Ok(tokenizer.count(text, token_model))
}

/// Scan repository for security issues
///
/// Args:
///     path: Path to the repository
///
/// Returns:
///     List of security findings
///
/// Example:
///     >>> import infiniloom
///     >>> findings = infiniloom.scan_security("/path/to/repo")
///     >>> for finding in findings:
///     ...     print(finding["severity"], finding["message"])
#[pyfunction]
fn scan_security(py: Python, path: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 10 * 1024 * 1024, // 10MB for security scan
        skip_symbols: true,              // Fast mode for security scan
    };

    let repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    let scanner = SecurityScanner::new();
    let mut all_findings = Vec::new();

    // Scan each file's content
    for file in &repo.files {
        if let Some(content) = &file.content {
            let findings = scanner.scan(content, &file.relative_path);
            all_findings.extend(findings);
        }
    }

    // Convert findings to Python list
    let results = PyList::new(
        py,
        all_findings.iter().map(|finding| {
            let dict = PyDict::new(py);
            dict.set_item("file", &finding.file).unwrap();
            dict.set_item("line", finding.line).unwrap();
            dict.set_item("severity", format!("{:?}", finding.severity))
                .unwrap();
            dict.set_item("kind", finding.kind.name()).unwrap();
            dict.set_item("pattern", &finding.pattern).unwrap();
            dict
        }),
    );

    Ok(results.into())
}

/// Compress text using semantic compression
///
/// Uses heuristic-based compression to reduce content while preserving meaning.
/// When built with the "embeddings" feature, uses neural networks for clustering.
///
/// Args:
///     text: Text to compress
///     similarity_threshold: Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
///     budget_ratio: Target size as ratio of original (0.0-1.0, default: 0.5)
///
/// Returns:
///     Compressed text
///
/// Example:
///     >>> import infiniloom
///     >>> compressed = infiniloom.semantic_compress(long_text, budget_ratio=0.3)
#[pyfunction]
#[pyo3(signature = (text, similarity_threshold=0.7, budget_ratio=0.5))]
fn semantic_compress(text: &str, similarity_threshold: f32, budget_ratio: f32) -> PyResult<String> {
    let config = SemanticConfig {
        similarity_threshold,
        budget_ratio,
        min_chunk_size: 100,
        max_chunk_size: 2000,
    };

    let compressor = SemanticCompressor::with_config(config);
    compressor
        .compress(text)
        .map_err(|e| PyValueError::new_err(format!("Compression failed: {}", e)))
}

/// Infiniloom class for object-oriented interface
///
/// Example:
///     >>> from infiniloom import Infiniloom
///     >>> loom = Infiniloom("/path/to/repo")
///     >>> stats = loom.stats()
///     >>> context = loom.pack(format="xml", model="claude")
#[pyclass]
struct Infiniloom {
    path: PathBuf,
    repo: Option<Repository>,
}

#[pymethods]
impl Infiniloom {
    /// Create a new Infiniloom instance
    ///
    /// Args:
    ///     path: Path to the repository
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err(PyIOError::new_err(format!("Path does not exist: {}", path)));
        }

        Ok(Infiniloom { path: path_buf, repo: None })
    }

    /// Scan the repository and load it into memory
    fn load(&mut self, include_hidden: bool, respect_gitignore: bool) -> PyResult<()> {
        let config = ScanConfig {
            include_hidden,
            respect_gitignore,
            read_contents: true,
            max_file_size: 50 * 1024 * 1024,
            skip_symbols: false, // Extract symbols by default
        };

        let repo = scan_repository(&self.path, config).map_err(to_py_err)?;
        self.repo = Some(repo);
        Ok(())
    }

    /// Get repository statistics
    fn stats(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();

        let dict = PyDict::new(py);
        dict.set_item("name", &repo.name)?;
        dict.set_item("path", repo.path.to_string_lossy().to_string())?;
        dict.set_item("total_files", repo.metadata.total_files)?;
        dict.set_item("total_lines", repo.metadata.total_lines)?;

        let tokens = PyDict::new(py);
        tokens.set_item("o200k", repo.metadata.total_tokens.o200k)?;
        tokens.set_item("cl100k", repo.metadata.total_tokens.cl100k)?;
        tokens.set_item("claude", repo.metadata.total_tokens.claude)?;
        tokens.set_item("gemini", repo.metadata.total_tokens.gemini)?;
        tokens.set_item("llama", repo.metadata.total_tokens.llama)?;
        tokens.set_item("mistral", repo.metadata.total_tokens.mistral)?;
        tokens.set_item("deepseek", repo.metadata.total_tokens.deepseek)?;
        tokens.set_item("qwen", repo.metadata.total_tokens.qwen)?;
        tokens.set_item("cohere", repo.metadata.total_tokens.cohere)?;
        tokens.set_item("grok", repo.metadata.total_tokens.grok)?;
        dict.set_item("tokens", tokens)?;

        Ok(dict.into())
    }

    /// Pack the repository into an LLM-optimized format
    #[pyo3(signature = (format="xml", model="claude", compression="balanced", map_budget=2000, max_symbols=50))]
    fn pack(
        &mut self,
        format: &str,
        model: &str,
        compression: &str,
        map_budget: u32,
        max_symbols: usize,
    ) -> PyResult<String> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        // Clone repo so we can modify it for compression
        let mut repo = self.repo.as_ref().unwrap().clone();

        // Apply default ignores to filter out build outputs, dependencies, test fixtures, etc.
        apply_default_ignores(&mut repo);

        // Prepare repository (count references, rank files, sort by importance)
        prepare_repository(&mut repo);

        // Parse format, model, and compression using common crate
        let output_format =
            parse_format(Some(format)).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let tokenizer_model =
            parse_model(Some(model)).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let compression_level = parse_compression(Some(compression))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        // Apply compression to file contents
        apply_compression(&mut repo, compression_level);

        // Generate repository map using builder pattern
        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols)
            .model(tokenizer_model)
            .build();
        let map = generator.generate(&repo);

        // Format output
        let formatter = OutputFormatter::by_format_with_model(output_format, tokenizer_model);
        let output = formatter.format(&repo, &map);

        Ok(output)
    }

    /// Get the repository map
    #[pyo3(signature = (map_budget=2000, max_symbols=50))]
    fn map(&mut self, py: Python, map_budget: u32, max_symbols: usize) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        // Clone and process repo
        let mut repo = self.repo.as_ref().unwrap().clone();

        // Apply default ignores and prepare repository
        apply_default_ignores(&mut repo);
        prepare_repository(&mut repo);

        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols)
            .build();
        let map = generator.generate(&repo);

        // Convert to Python dict
        let dict = PyDict::new(py);
        dict.set_item("summary", &map.summary)?;
        dict.set_item("token_count", map.token_count)?;

        // Key symbols
        let symbols = PyList::new(
            py,
            map.key_symbols.iter().map(|sym| {
                let sym_dict = PyDict::new(py);
                sym_dict.set_item("name", &sym.name).unwrap();
                sym_dict.set_item("kind", &sym.kind).unwrap();
                sym_dict.set_item("file", &sym.file).unwrap();
                sym_dict.set_item("line", sym.line).unwrap();
                sym_dict.set_item("rank", sym.rank).unwrap();
                sym_dict.set_item("importance", sym.importance).unwrap();
                if let Some(sig) = &sym.signature {
                    sym_dict.set_item("signature", sig).unwrap();
                }
                sym_dict
            }),
        );
        dict.set_item("key_symbols", symbols)?;

        Ok(dict.into())
    }

    /// Scan for security issues
    fn scan_security(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();
        let scanner = SecurityScanner::new();
        let mut all_findings = Vec::new();

        // Scan each file's content
        for file in &repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                all_findings.extend(findings);
            }
        }

        let results = PyList::new(
            py,
            all_findings.iter().map(|finding| {
                let dict = PyDict::new(py);
                dict.set_item("file", &finding.file).unwrap();
                dict.set_item("line", finding.line).unwrap();
                dict.set_item("severity", format!("{:?}", finding.severity))
                    .unwrap();
                dict.set_item("kind", finding.kind.name()).unwrap();
                dict.set_item("pattern", &finding.pattern).unwrap();
                dict
            }),
        );

        Ok(results.into())
    }

    /// Get list of files in the repository
    fn files(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();

        let files = PyList::new(
            py,
            repo.files.iter().map(|file| {
                let dict = PyDict::new(py);
                dict.set_item("path", &file.relative_path).unwrap();
                if let Some(lang) = &file.language {
                    dict.set_item("language", lang).unwrap();
                }
                dict.set_item("size_bytes", file.size_bytes).unwrap();
                dict.set_item("tokens", file.token_count.claude).unwrap();
                dict.set_item("importance", file.importance).unwrap();
                dict
            }),
        );

        Ok(files.into())
    }

    fn __repr__(&self) -> String {
        format!("Infiniloom('{}')", self.path.display())
    }

    fn __str__(&self) -> String {
        format!("Infiniloom repository at {}", self.path.display())
    }
}

// ============================================================================
// Git Operations
// ============================================================================

/// Check if a path is a git repository
///
/// Args:
///     path: Path to check
///
/// Returns:
///     True if path is a git repository, False otherwise
///
/// Example:
///     >>> import infiniloom
///     >>> is_repo = infiniloom.is_git_repo("/path/to/repo")
#[pyfunction]
fn is_git_repo(path: &str) -> bool {
    let path_buf = PathBuf::from(path);
    EngineGitRepo::is_git_repo(&path_buf)
}

/// Git repository wrapper for Python
///
/// Provides access to git operations like status, diff, log, and blame.
///
/// Example:
///     >>> from infiniloom import GitRepo
///     >>> repo = GitRepo("/path/to/repo")
///     >>> print(repo.current_branch())
///     >>> for file in repo.status():
///     ...     print(file["path"], file["status"])
#[pyclass]
struct GitRepo {
    inner: EngineGitRepo,
}

#[pymethods]
impl GitRepo {
    /// Open a git repository
    ///
    /// Args:
    ///     path: Path to the repository
    ///
    /// Raises:
    ///     InfiniloomError: If path is not a git repository
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let path_buf = PathBuf::from(path);
        let inner = EngineGitRepo::open(&path_buf)
            .map_err(|e| InfiniloomError::new_err(format!("Failed to open git repo: {}", e)))?;
        Ok(GitRepo { inner })
    }

    /// Get the current branch name
    ///
    /// Returns:
    ///     Current branch name (e.g., "main", "feature/xyz")
    fn current_branch(&self) -> PyResult<String> {
        self.inner.current_branch().map_err(to_py_err)
    }

    /// Get the current commit hash
    ///
    /// Returns:
    ///     Full SHA-1 hash of HEAD commit
    fn current_commit(&self) -> PyResult<String> {
        self.inner.current_commit().map_err(to_py_err)
    }

    /// Get working tree status
    ///
    /// Returns both staged and unstaged changes.
    ///
    /// Returns:
    ///     List of dicts with keys: path, old_path (for renames), status
    ///     Status is one of: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
    fn status(&self, py: Python) -> PyResult<PyObject> {
        let files = self.inner.status().map_err(to_py_err)?;

        let result = PyList::new(
            py,
            files.iter().map(|f| {
                let dict = PyDict::new(py);
                dict.set_item("path", &f.path).unwrap();
                if let Some(old) = &f.old_path {
                    dict.set_item("old_path", old).unwrap();
                }
                dict.set_item("status", format_file_status(f.status))
                    .unwrap();
                dict
            }),
        );

        Ok(result.into())
    }

    /// Get files changed between two commits
    ///
    /// Args:
    ///     from_ref: Starting commit/branch/tag
    ///     to_ref: Ending commit/branch/tag
    ///
    /// Returns:
    ///     List of dicts with: path, old_path, status, additions, deletions
    #[pyo3(signature = (from_ref, to_ref))]
    fn diff_files(&self, py: Python, from_ref: &str, to_ref: &str) -> PyResult<PyObject> {
        let files = self.inner.diff_files(from_ref, to_ref).map_err(to_py_err)?;

        let result = PyList::new(
            py,
            files.iter().map(|f| {
                let dict = PyDict::new(py);
                dict.set_item("path", &f.path).unwrap();
                if let Some(old) = &f.old_path {
                    dict.set_item("old_path", old).unwrap();
                }
                dict.set_item("status", format_file_status(f.status))
                    .unwrap();
                dict.set_item("additions", f.additions).unwrap();
                dict.set_item("deletions", f.deletions).unwrap();
                dict
            }),
        );

        Ok(result.into())
    }

    /// Get recent commits
    ///
    /// Args:
    ///     count: Maximum number of commits to return (default: 10)
    ///
    /// Returns:
    ///     List of dicts with: hash, short_hash, author, email, date, message
    #[pyo3(signature = (count=10))]
    fn log(&self, py: Python, count: usize) -> PyResult<PyObject> {
        let commits = self.inner.log(count).map_err(to_py_err)?;

        let result = PyList::new(
            py,
            commits.iter().map(|c| {
                let dict = PyDict::new(py);
                dict.set_item("hash", &c.hash).unwrap();
                dict.set_item("short_hash", &c.short_hash).unwrap();
                dict.set_item("author", &c.author).unwrap();
                dict.set_item("email", &c.email).unwrap();
                dict.set_item("date", &c.date).unwrap();
                dict.set_item("message", &c.message).unwrap();
                dict
            }),
        );

        Ok(result.into())
    }

    /// Get commits that modified a specific file
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///     count: Maximum number of commits to return (default: 10)
    ///
    /// Returns:
    ///     List of commits that modified the file
    #[pyo3(signature = (path, count=10))]
    fn file_log(&self, py: Python, path: &str, count: usize) -> PyResult<PyObject> {
        let commits = self.inner.file_log(path, count).map_err(to_py_err)?;

        let result = PyList::new(
            py,
            commits.iter().map(|c| {
                let dict = PyDict::new(py);
                dict.set_item("hash", &c.hash).unwrap();
                dict.set_item("short_hash", &c.short_hash).unwrap();
                dict.set_item("author", &c.author).unwrap();
                dict.set_item("email", &c.email).unwrap();
                dict.set_item("date", &c.date).unwrap();
                dict.set_item("message", &c.message).unwrap();
                dict
            }),
        );

        Ok(result.into())
    }

    /// Get blame information for a file
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///
    /// Returns:
    ///     List of dicts with: commit, author, date, line_number
    fn blame(&self, py: Python, path: &str) -> PyResult<PyObject> {
        let lines = self.inner.blame(path).map_err(to_py_err)?;

        let result = PyList::new(
            py,
            lines.iter().map(|l| {
                let dict = PyDict::new(py);
                dict.set_item("commit", &l.commit).unwrap();
                dict.set_item("author", &l.author).unwrap();
                dict.set_item("date", &l.date).unwrap();
                dict.set_item("line_number", l.line_number).unwrap();
                dict
            }),
        );

        Ok(result.into())
    }

    /// Get list of files tracked by git
    ///
    /// Returns:
    ///     List of file paths tracked by git
    fn ls_files(&self) -> PyResult<Vec<String>> {
        self.inner.ls_files().map_err(to_py_err)
    }

    /// Get diff content between two commits for a file
    ///
    /// Args:
    ///     from_ref: Starting commit/branch/tag
    ///     to_ref: Ending commit/branch/tag
    ///     path: File path (relative to repo root)
    ///
    /// Returns:
    ///     Unified diff content as string
    #[pyo3(signature = (from_ref, to_ref, path))]
    fn diff_content(&self, from_ref: &str, to_ref: &str, path: &str) -> PyResult<String> {
        self.inner
            .diff_content(from_ref, to_ref, path)
            .map_err(to_py_err)
    }

    /// Get diff content for uncommitted changes in a file
    ///
    /// Includes both staged and unstaged changes compared to HEAD.
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///
    /// Returns:
    ///     Unified diff content as string
    fn uncommitted_diff(&self, path: &str) -> PyResult<String> {
        self.inner.uncommitted_diff(path).map_err(to_py_err)
    }

    /// Get diff for all uncommitted changes
    ///
    /// Returns combined diff for all changed files.
    ///
    /// Returns:
    ///     Unified diff content as string
    fn all_uncommitted_diffs(&self) -> PyResult<String> {
        self.inner.all_uncommitted_diffs().map_err(to_py_err)
    }

    /// Check if a file has uncommitted changes
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///
    /// Returns:
    ///     True if file has changes, False otherwise
    fn has_changes(&self, path: &str) -> PyResult<bool> {
        self.inner.has_changes(path).map_err(to_py_err)
    }

    /// Get the last commit that modified a file
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///
    /// Returns:
    ///     Dict with commit information
    fn last_modified_commit(&self, py: Python, path: &str) -> PyResult<PyObject> {
        let commit = self.inner.last_modified_commit(path).map_err(to_py_err)?;

        let dict = PyDict::new(py);
        dict.set_item("hash", &commit.hash)?;
        dict.set_item("short_hash", &commit.short_hash)?;
        dict.set_item("author", &commit.author)?;
        dict.set_item("email", &commit.email)?;
        dict.set_item("date", &commit.date)?;
        dict.set_item("message", &commit.message)?;

        Ok(dict.into())
    }

    /// Get file change frequency in recent days
    ///
    /// Useful for determining file importance based on recent activity.
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///     days: Number of days to look back (default: 30)
    ///
    /// Returns:
    ///     Number of commits that modified the file in the period
    #[pyo3(signature = (path, days=30))]
    fn file_change_frequency(&self, path: &str, days: u32) -> PyResult<u32> {
        self.inner
            .file_change_frequency(path, days)
            .map_err(to_py_err)
    }

    /// Get file content at a specific git ref (commit, branch, tag)
    ///
    /// Uses `git show <ref>:<path>` to retrieve file content at that revision.
    ///
    /// Args:
    ///     path: File path (relative to repo root)
    ///     git_ref: Git ref (commit hash, branch name, tag, HEAD~n, etc.)
    ///
    /// Returns:
    ///     File content as string
    ///
    /// Example:
    ///     >>> repo = GitRepo("/path/to/repo")
    ///     >>> old_version = repo.file_at_ref("src/main.py", "HEAD~5")
    ///     >>> main_version = repo.file_at_ref("src/main.py", "main")
    #[pyo3(signature = (path, git_ref))]
    fn file_at_ref(&self, path: &str, git_ref: &str) -> PyResult<String> {
        self.inner.file_at_ref(path, git_ref).map_err(to_py_err)
    }

    /// Parse diff between two refs into structured hunks
    ///
    /// Returns detailed hunk information including line numbers for each change.
    /// Useful for PR review tools that need to post comments at specific lines.
    ///
    /// Args:
    ///     from_ref: Starting ref (e.g., "main", "HEAD~5", commit hash)
    ///     to_ref: Ending ref (e.g., "HEAD", "feature-branch")
    ///     path: Optional file path to filter to a single file
    ///
    /// Returns:
    ///     List of dicts with: old_start, old_count, new_start, new_count, header, lines
    ///     Each line has: change_type ("add"/"remove"/"context"), old_line, new_line, content
    ///
    /// Example:
    ///     >>> repo = GitRepo("/path/to/repo")
    ///     >>> hunks = repo.diff_hunks("main", "HEAD", "src/index.py")
    ///     >>> for hunk in hunks:
    ///     ...     print(f"Hunk at old:{hunk['old_start']} new:{hunk['new_start']}")
    ///     ...     for line in hunk['lines']:
    ///     ...         print(f"{line['change_type']}: {line['content']}")
    #[pyo3(signature = (from_ref, to_ref, path=None))]
    fn diff_hunks(
        &self,
        py: Python,
        from_ref: &str,
        to_ref: &str,
        path: Option<&str>,
    ) -> PyResult<PyObject> {
        let hunks = self
            .inner
            .diff_hunks(from_ref, to_ref, path)
            .map_err(to_py_err)?;

        let result = PyList::new(py, hunks.iter().map(|h| convert_hunk_to_py(py, h)));

        Ok(result.into())
    }

    /// Parse uncommitted changes (working tree vs HEAD) into structured hunks
    ///
    /// Args:
    ///     path: Optional file path to filter to a single file
    ///
    /// Returns:
    ///     List of diff hunks for uncommitted changes
    ///
    /// Example:
    ///     >>> repo = GitRepo("/path/to/repo")
    ///     >>> hunks = repo.uncommitted_hunks("src/index.py")
    ///     >>> print(f"{len(hunks)} hunks with uncommitted changes")
    #[pyo3(signature = (path=None))]
    fn uncommitted_hunks(&self, py: Python, path: Option<&str>) -> PyResult<PyObject> {
        let hunks = self.inner.uncommitted_hunks(path).map_err(to_py_err)?;

        let result = PyList::new(py, hunks.iter().map(|h| convert_hunk_to_py(py, h)));

        Ok(result.into())
    }

    /// Parse staged changes into structured hunks
    ///
    /// Args:
    ///     path: Optional file path to filter to a single file
    ///
    /// Returns:
    ///     List of diff hunks for staged changes only
    ///
    /// Example:
    ///     >>> repo = GitRepo("/path/to/repo")
    ///     >>> hunks = repo.staged_hunks("src/index.py")
    ///     >>> print(f"{len(hunks)} hunks staged for commit")
    #[pyo3(signature = (path=None))]
    fn staged_hunks(&self, py: Python, path: Option<&str>) -> PyResult<PyObject> {
        let hunks = self.inner.staged_hunks(path).map_err(to_py_err)?;

        let result = PyList::new(py, hunks.iter().map(|h| convert_hunk_to_py(py, h)));

        Ok(result.into())
    }

    fn __repr__(&self) -> String {
        "GitRepo(<git repository>)".to_string()
    }
}

/// Convert an engine DiffHunk to a Python dict
fn convert_hunk_to_py<'py>(py: Python<'py>, hunk: &EngineGitDiffHunk) -> &'py pyo3::types::PyDict {
    let dict = PyDict::new(py);
    dict.set_item("old_start", hunk.old_start).unwrap();
    dict.set_item("old_count", hunk.old_count).unwrap();
    dict.set_item("new_start", hunk.new_start).unwrap();
    dict.set_item("new_count", hunk.new_count).unwrap();
    dict.set_item("header", &hunk.header).unwrap();

    let lines = PyList::new(
        py,
        hunk.lines.iter().map(|l| {
            let line_dict = PyDict::new(py);
            line_dict
                .set_item("change_type", l.change_type.as_str())
                .unwrap();
            if let Some(old_line) = l.old_line {
                line_dict.set_item("old_line", old_line).unwrap();
            }
            if let Some(new_line) = l.new_line {
                line_dict.set_item("new_line", new_line).unwrap();
            }
            line_dict.set_item("content", &l.content).unwrap();
            line_dict
        }),
    );
    dict.set_item("lines", lines).unwrap();

    dict
}

// ============================================================================
// Index API - Build and query symbol indexes
// ============================================================================

/// Build or update the symbol index for a repository
///
/// The index enables fast diff-to-context lookups and impact analysis.
///
/// Args:
///     path: Path to repository root
///     force: Force full rebuild even if index exists (default: False)
///     include_tests: Include test files in index (default: False)
///     max_file_size: Maximum file size to index in bytes (default: 10MB)
///
/// Returns:
///     Dictionary with index status: exists, file_count, symbol_count, last_built, version
///
/// Example:
///     >>> import infiniloom
///     >>> status = infiniloom.build_index("/path/to/repo")
///     >>> print(f"Indexed {status['symbol_count']} symbols")
#[pyfunction]
#[pyo3(signature = (path, force=false, include_tests=false, max_file_size=None))]
fn build_index(
    py: Python,
    path: &str,
    force: bool,
    include_tests: bool,
    max_file_size: Option<u64>,
) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    if !force {
        // Check if index exists and is valid
        if let Ok(meta) = storage.load_meta() {
            if let (Ok(index), Ok(_graph)) = (storage.load_index(), storage.load_graph()) {
                let dict = PyDict::new(py);
                dict.set_item("exists", true)?;
                dict.set_item("file_count", index.files.len())?;
                dict.set_item("symbol_count", index.symbols.len())?;
                dict.set_item("last_built", meta.created_at)?;
                dict.set_item("version", format!("v{}", meta.version))?;
                return Ok(dict.into());
            }
        }
    }

    // Build new index
    let mut exclude_dirs = vec![
        "node_modules".to_string(),
        "target".to_string(),
        ".git".to_string(),
        "dist".to_string(),
        "build".to_string(),
    ];

    if !include_tests {
        exclude_dirs.extend(vec![
            "test".to_string(),
            "tests".to_string(),
            "__tests__".to_string(),
            "spec".to_string(),
        ]);
    }

    let build_opts = BuildOptions {
        max_file_size: max_file_size.unwrap_or(10 * 1024 * 1024),
        exclude_dirs,
        ..Default::default()
    };

    let builder = IndexBuilder::new(&path_buf).with_options(build_opts);
    let (index, graph) = builder.build().map_err(to_py_err)?;

    // Save index
    storage.save_all(&index, &graph).map_err(to_py_err)?;

    let meta = storage.load_meta().map_err(to_py_err)?;

    let dict = PyDict::new(py);
    dict.set_item("exists", true)?;
    dict.set_item("file_count", index.files.len())?;
    dict.set_item("symbol_count", index.symbols.len())?;
    dict.set_item("last_built", meta.created_at)?;
    dict.set_item("version", format!("v{}", meta.version))?;

    Ok(dict.into())
}

/// Get the status of an existing index
///
/// Args:
///     path: Path to repository root
///
/// Returns:
///     Dictionary with index status information
///
/// Example:
///     >>> import infiniloom
///     >>> status = infiniloom.index_status("/path/to/repo")
///     >>> if status["exists"]:
///     ...     print(f"Index has {status['symbol_count']} symbols")
#[pyfunction]
fn index_status(py: Python, path: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let dict = PyDict::new(py);

    match (storage.load_meta(), storage.load_index()) {
        (Ok(meta), Ok(index)) => {
            dict.set_item("exists", true)?;
            dict.set_item("file_count", index.files.len())?;
            dict.set_item("symbol_count", index.symbols.len())?;
            dict.set_item("last_built", meta.created_at)?;
            dict.set_item("version", format!("v{}", meta.version))?;
        },
        _ => {
            dict.set_item("exists", false)?;
            dict.set_item("file_count", 0)?;
            dict.set_item("symbol_count", 0)?;
            dict.set_item("last_built", py.None())?;
            dict.set_item("version", py.None())?;
        },
    }

    Ok(dict.into())
}

// ============================================================================
// Call Graph API - Query symbol relationships
// ============================================================================

/// Convert an engine SymbolInfo to a Python dict
fn symbol_info_to_py<'py>(py: Python<'py>, s: &EngineSymbolInfo) -> &'py pyo3::types::PyDict {
    let dict = PyDict::new(py);
    dict.set_item("id", s.id).unwrap();
    dict.set_item("name", &s.name).unwrap();
    dict.set_item("kind", &s.kind).unwrap();
    dict.set_item("file", &s.file).unwrap();
    dict.set_item("line", s.line).unwrap();
    dict.set_item("end_line", s.end_line).unwrap();
    if let Some(ref sig) = s.signature {
        dict.set_item("signature", sig).unwrap();
    }
    dict.set_item("visibility", &s.visibility).unwrap();
    dict
}

/// Convert an engine ReferenceInfo to a Python dict
fn reference_info_to_py<'py>(py: Python<'py>, r: &EngineReferenceInfo) -> &'py pyo3::types::PyDict {
    let dict = PyDict::new(py);
    dict.set_item("symbol", symbol_info_to_py(py, &r.symbol))
        .unwrap();
    dict.set_item("kind", &r.kind).unwrap();
    dict
}

/// Convert an engine CallGraph to a Python dict
fn call_graph_to_py<'py>(py: Python<'py>, g: &EngineCallGraph) -> &'py pyo3::types::PyDict {
    let dict = PyDict::new(py);

    // Convert nodes
    let nodes = PyList::new(py, g.nodes.iter().map(|n| symbol_info_to_py(py, n)));
    dict.set_item("nodes", nodes).unwrap();

    // Convert edges
    let edges = PyList::new(
        py,
        g.edges.iter().map(|e| {
            let edge_dict = PyDict::new(py);
            edge_dict.set_item("caller_id", e.caller_id).unwrap();
            edge_dict.set_item("callee_id", e.callee_id).unwrap();
            edge_dict.set_item("caller", &e.caller).unwrap();
            edge_dict.set_item("callee", &e.callee).unwrap();
            edge_dict.set_item("file", &e.file).unwrap();
            edge_dict.set_item("line", e.line).unwrap();
            edge_dict
        }),
    );
    dict.set_item("edges", edges).unwrap();

    // Convert stats
    let stats = PyDict::new(py);
    stats
        .set_item("total_symbols", g.stats.total_symbols)
        .unwrap();
    stats.set_item("total_calls", g.stats.total_calls).unwrap();
    stats.set_item("functions", g.stats.functions).unwrap();
    stats.set_item("classes", g.stats.classes).unwrap();
    dict.set_item("stats", stats).unwrap();

    dict
}

/// Find a symbol by name
///
/// Searches the index for all symbols matching the given name.
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     name: Symbol name to search for
///
/// Returns:
///     List of dicts with: id, name, kind, file, line, end_line, signature, visibility
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> symbols = infiniloom.find_symbol("/path/to/repo", "process_request")
///     >>> print(f"Found {len(symbols)} symbols named process_request")
#[pyfunction]
fn find_symbol(py: Python, path: &str, name: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| PyIOError::new_err(format!("Failed to load index: {}", e)))?;

    let results = engine_find_symbol(&index, name);

    let list = PyList::new(py, results.iter().map(|s| symbol_info_to_py(py, s)));

    Ok(list.into())
}

/// Get all callers of a symbol
///
/// Returns symbols that call any symbol with the given name.
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     symbol_name: Name of the symbol to find callers for
///
/// Returns:
///     List of symbols that call the target symbol
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> callers = infiniloom.get_callers("/path/to/repo", "authenticate")
///     >>> print(f"authenticate is called by {len(callers)} functions")
///     >>> for c in callers:
///     ...     print(f"  {c['name']} at {c['file']}:{c['line']}")
#[pyfunction]
fn get_callers(py: Python, path: &str, symbol_name: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| PyIOError::new_err(format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| PyIOError::new_err(format!("Failed to load graph: {}", e)))?;

    let results = get_callers_by_name(&index, &graph, symbol_name);

    let list = PyList::new(py, results.iter().map(|s| symbol_info_to_py(py, s)));

    Ok(list.into())
}

/// Get all callees of a symbol
///
/// Returns symbols that are called by any symbol with the given name.
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     symbol_name: Name of the symbol to find callees for
///
/// Returns:
///     List of symbols that the target symbol calls
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> callees = infiniloom.get_callees("/path/to/repo", "main")
///     >>> print(f"main calls {len(callees)} functions")
///     >>> for c in callees:
///     ...     print(f"  {c['name']} at {c['file']}:{c['line']}")
#[pyfunction]
fn get_callees(py: Python, path: &str, symbol_name: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| PyIOError::new_err(format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| PyIOError::new_err(format!("Failed to load graph: {}", e)))?;

    let results = get_callees_by_name(&index, &graph, symbol_name);

    let list = PyList::new(py, results.iter().map(|s| symbol_info_to_py(py, s)));

    Ok(list.into())
}

/// Get all references to a symbol
///
/// Returns all locations where a symbol is referenced (calls, imports, inheritance).
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     symbol_name: Name of the symbol to find references for
///
/// Returns:
///     List of dicts with: symbol (SymbolInfo dict), kind (reference type)
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> refs = infiniloom.get_references("/path/to/repo", "UserService")
///     >>> print(f"UserService is referenced {len(refs)} times")
///     >>> for r in refs:
///     ...     print(f"  {r['kind']}: {r['symbol']['name']} at {r['symbol']['file']}:{r['symbol']['line']}")
#[pyfunction]
fn get_references(py: Python, path: &str, symbol_name: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| PyIOError::new_err(format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| PyIOError::new_err(format!("Failed to load graph: {}", e)))?;

    let results = get_references_by_name(&index, &graph, symbol_name);

    let list = PyList::new(py, results.iter().map(|r| reference_info_to_py(py, r)));

    Ok(list.into())
}

/// Get the complete call graph
///
/// Returns all symbols and their call relationships.
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     max_nodes: Maximum number of nodes to return (default: unlimited)
///     max_edges: Maximum number of edges to return (default: unlimited)
///
/// Returns:
///     Dict with: nodes (list of symbols), edges (list of call edges), stats (summary)
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> graph = infiniloom.get_call_graph("/path/to/repo")
///     >>> print(f"Call graph: {graph['stats']['total_symbols']} symbols, {graph['stats']['total_calls']} calls")
///     >>> # Find most called functions
///     >>> from collections import Counter
///     >>> call_counts = Counter(edge['callee'] for edge in graph['edges'])
///     >>> print("Most called:", call_counts.most_common(10))
#[pyfunction]
#[pyo3(signature = (path, max_nodes=None, max_edges=None))]
fn get_call_graph(
    py: Python,
    path: &str,
    max_nodes: Option<usize>,
    max_edges: Option<usize>,
) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| PyIOError::new_err(format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| PyIOError::new_err(format!("Failed to load graph: {}", e)))?;

    let result = if max_nodes.is_some() || max_edges.is_some() {
        get_call_graph_filtered(&index, &graph, max_nodes, max_edges)
    } else {
        engine_get_call_graph(&index, &graph)
    };

    Ok(call_graph_to_py(py, &result).into())
}

// ============================================================================
// Chunk API - Split repositories into manageable pieces
// ============================================================================

/// Split a repository into chunks for incremental processing
///
/// Useful for processing large repositories that exceed LLM context limits.
///
/// Args:
///     path: Path to repository root
///     strategy: Chunking strategy - "fixed", "file", "module", "symbol", "semantic", "dependency" (default: "module")
///     max_tokens: Maximum tokens per chunk (default: 8000)
///     overlap: Token overlap between chunks (default: 0)
///     model: Target model for token counting (default: "claude")
///     priority_first: Sort chunks by priority, core modules first (default: False)
///
/// Returns:
///     List of chunk dictionaries with: index, total, focus, tokens, files, content
///
/// Example:
///     >>> import infiniloom
///     >>> chunks = infiniloom.chunk("/path/to/large-repo", strategy="module", max_tokens=50000)
///     >>> for c in chunks:
///     ...     print(f"Chunk {c['index']}/{c['total']}: {c['focus']} ({c['tokens']} tokens)")
#[pyfunction]
#[pyo3(signature = (path, strategy="module", max_tokens=8000, overlap=0, model="claude", priority_first=false))]
fn chunk(
    py: Python,
    path: &str,
    strategy: &str,
    max_tokens: u32,
    overlap: u32,
    model: &str,
    priority_first: bool,
) -> PyResult<PyObject> {
    // Parse strategy
    let chunk_strategy = match strategy.to_lowercase().as_str() {
        "fixed" => ChunkStrategy::Fixed { size: max_tokens },
        "file" => ChunkStrategy::File,
        "module" => ChunkStrategy::Module,
        "symbol" => ChunkStrategy::Symbol,
        "semantic" => ChunkStrategy::Semantic,
        "dependency" => ChunkStrategy::Dependency,
        _ => return Err(PyValueError::new_err(format!(
            "Invalid strategy: {}. Use 'fixed', 'file', 'module', 'symbol', 'semantic', or 'dependency'",
            strategy
        ))),
    };

    // Parse model using common crate
    let tokenizer_model =
        parse_model(Some(model)).map_err(|e| PyValueError::new_err(e.to_string()))?;

    // Scan repository
    let path_buf = PathBuf::from(path);
    let needs_symbols = matches!(chunk_strategy, ChunkStrategy::Dependency | ChunkStrategy::Symbol);
    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024,
        skip_symbols: !needs_symbols,
    };

    let mut repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    // Apply default ignores
    apply_default_ignores(&mut repo);

    // Create chunker
    let chunker = Chunker::new(chunk_strategy, max_tokens)
        .with_model(tokenizer_model)
        .with_overlap(overlap);

    let mut chunks = chunker.chunk(&repo);

    // Apply priority sorting if requested
    if priority_first && chunks.len() > 1 {
        let mut chunk_priorities: Vec<(usize, f64)> = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let avg_priority = if c.files.is_empty() {
                    0.0
                } else {
                    let total: f64 = c.files.iter().map(|f| file_priority_score(&f.path)).sum();
                    total / c.files.len() as f64
                };
                (i, avg_priority)
            })
            .collect();

        chunk_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let original_chunks = std::mem::take(&mut chunks);
        for (idx, (orig_idx, _)) in chunk_priorities.iter().enumerate() {
            let mut c = original_chunks[*orig_idx].clone();
            c.index = idx;
            chunks.push(c);
        }

        let total = chunks.len();
        for c in &mut chunks {
            c.total = total;
        }
    }

    // Convert to Python list
    let results = PyList::new(
        py,
        chunks.iter().map(|c| {
            let dict = PyDict::new(py);
            dict.set_item("index", c.index).unwrap();
            dict.set_item("total", c.total).unwrap();
            dict.set_item("focus", &c.focus).unwrap();
            dict.set_item("tokens", c.tokens).unwrap();
            dict.set_item("files", c.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>())
                .unwrap();
            // Format content
            let content: String = c
                .files
                .iter()
                .map(|f| format!("// {}\n{}", f.path, f.content))
                .collect::<Vec<_>>()
                .join("\n\n");
            dict.set_item("content", content).unwrap();
            dict
        }),
    );

    Ok(results.into())
}

// ============================================================================
// Impact API - Analyze change impact
// ============================================================================

/// Analyze the impact of changes to files or symbols
///
/// Requires an index to be built first (use build_index).
///
/// Args:
///     path: Path to repository root
///     files: List of files to analyze
///     depth: Depth of dependency traversal (1-3, default: 2)
///     include_tests: Include test files in analysis (default: False)
///
/// Returns:
///     Dictionary with: changed_files, dependent_files, test_files, affected_symbols, impact_level, summary
///
/// Example:
///     >>> import infiniloom
///     >>> infiniloom.build_index("/path/to/repo")
///     >>> impact = infiniloom.analyze_impact("/path/to/repo", ["src/auth.py"])
///     >>> print(f"Impact level: {impact['impact_level']}")
#[pyfunction]
#[pyo3(signature = (path, files, depth=2, include_tests=false))]
fn analyze_impact(
    py: Python,
    path: &str,
    files: Vec<String>,
    depth: u32,
    include_tests: bool,
) -> PyResult<PyObject> {
    let _ = include_tests; // Reserved for future use

    let path_buf = PathBuf::from(path);
    let storage = IndexStorage::new(&path_buf);

    // Load index
    let index = storage.load_index().map_err(|e| {
        PyIOError::new_err(format!("Failed to load index (run build_index first): {}", e))
    })?;
    let graph = storage
        .load_graph()
        .map_err(|e| PyIOError::new_err(format!("Failed to load dependency graph: {}", e)))?;

    // Create context expander
    let context_depth = match depth {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expander = ContextExpander::new(&index, &graph);

    // Convert files to diff changes
    let changes: Vec<DiffChange> = files
        .iter()
        .map(|f| DiffChange {
            file_path: f.clone(),
            old_path: None,
            line_ranges: vec![],
            change_type: ChangeType::Modified,
            diff_content: None,
        })
        .collect();

    // Expand context
    let token_budget = 50000;
    let context = expander.expand(&changes, context_depth, token_budget);

    // Collect results
    let changed_files: Vec<String> = changes.iter().map(|c| c.file_path.clone()).collect();

    let dependent_files: Vec<String> = context
        .dependent_files
        .iter()
        .map(|f| f.path.clone())
        .collect();

    let test_files: Vec<String> = context
        .related_tests
        .iter()
        .map(|f| f.path.clone())
        .collect();

    // Combine changed and dependent symbols
    let affected_symbols: Vec<_> = context
        .changed_symbols
        .iter()
        .chain(context.dependent_symbols.iter())
        .collect();

    // Determine impact level
    let impact_level = if dependent_files.len() > 20 || affected_symbols.len() > 50 {
        "critical"
    } else if dependent_files.len() > 10 || affected_symbols.len() > 20 {
        "high"
    } else if dependent_files.len() > 5 || affected_symbols.len() > 10 {
        "medium"
    } else {
        "low"
    };

    let summary = format!(
        "{} files changed, {} dependents affected, {} symbols impacted, {} tests related",
        changed_files.len(),
        dependent_files.len(),
        affected_symbols.len(),
        test_files.len()
    );

    // Build result dict
    let dict = PyDict::new(py);
    dict.set_item("changed_files", changed_files)?;
    dict.set_item("dependent_files", dependent_files)?;
    dict.set_item("test_files", test_files)?;

    // Affected symbols as list of dicts
    let symbols_list = PyList::new(
        py,
        affected_symbols.iter().map(|s| {
            let sym_dict = PyDict::new(py);
            sym_dict.set_item("name", &s.name).unwrap();
            sym_dict.set_item("kind", &s.kind).unwrap();
            sym_dict.set_item("file", &s.file_path).unwrap();
            sym_dict.set_item("line", s.start_line).unwrap();
            sym_dict
                .set_item("impact_type", &s.relevance_reason)
                .unwrap();
            sym_dict
        }),
    );
    dict.set_item("affected_symbols", symbols_list)?;
    dict.set_item("impact_level", impact_level)?;
    dict.set_item("summary", summary)?;

    Ok(dict.into())
}

// ============================================================================
// Diff Context API - Get context-aware diffs
// ============================================================================

/// Get context-aware diff with surrounding symbols and dependencies
///
/// Unlike basic git diff, this provides semantic context around changes.
/// Requires an index for full functionality (will work with limited context without one).
///
/// Args:
///     path: Path to repository root
///     from_ref: Starting commit/branch (use "" for unstaged changes)
///     to_ref: Ending commit/branch (use "HEAD" for staged, "" for working tree)
///     depth: Depth of context expansion (1-3, default: 2)
///     budget: Token budget for context (default: 50000)
///     include_diff: Include the actual diff content (default: False)
///
/// Returns:
///     Dictionary with: changed_files, context_symbols, related_tests, total_tokens
///
/// Example:
///     >>> import infiniloom
///     >>> # Get context for last commit
///     >>> context = infiniloom.get_diff_context("/path/to/repo", "HEAD~1", "HEAD")
///     >>> print(f"Changed: {len(context['changed_files'])} files")
///     >>> print(f"Related symbols: {len(context['context_symbols'])}")
#[pyfunction]
#[pyo3(signature = (path, from_ref="", to_ref="HEAD", depth=2, budget=50000, include_diff=false))]
fn get_diff_context(
    py: Python,
    path: &str,
    from_ref: &str,
    to_ref: &str,
    depth: u32,
    budget: u32,
    include_diff: bool,
) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf).map_err(to_py_err)?;

    // Get changed files
    let changed: Vec<ChangedFile> = if from_ref.is_empty() && to_ref.is_empty() {
        // Uncommitted changes
        git_repo
            .status()
            .map_err(to_py_err)?
            .iter()
            .map(|f| ChangedFile {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: f.status,
                additions: 0,
                deletions: 0,
            })
            .collect()
    } else {
        let from = if from_ref.is_empty() {
            "HEAD"
        } else {
            from_ref
        };
        let to = if to_ref.is_empty() { "HEAD" } else { to_ref };
        git_repo.diff_files(from, to).map_err(to_py_err)?
    };

    // Try to load existing index
    let storage = IndexStorage::new(&path_buf);

    // Build file contexts
    let mut changed_files_result: Vec<_> = Vec::new();
    for file in &changed {
        let diff_content = if include_diff {
            let from = if from_ref.is_empty() {
                "HEAD"
            } else {
                from_ref
            };
            let to = if to_ref.is_empty() { "HEAD" } else { to_ref };
            git_repo.diff_content(from, to, &file.path).ok()
        } else {
            None
        };

        let file_dict = PyDict::new(py);
        file_dict.set_item("path", &file.path)?;
        file_dict.set_item("change_type", format_file_status(file.status))?;
        file_dict.set_item("additions", file.additions)?;
        file_dict.set_item("deletions", file.deletions)?;
        if let Some(ref diff) = diff_content {
            file_dict.set_item("diff", diff)?;
        }
        changed_files_result.push(file_dict);
    }

    // Try to expand context if index exists
    let mut context_symbols: Vec<PyObject> = Vec::new();
    let mut related_tests: Vec<String> = Vec::new();

    if let (Ok(index), Ok(graph)) = (storage.load_index(), storage.load_graph()) {
        let context_depth = match depth {
            1 => ContextDepth::L1,
            2 => ContextDepth::L2,
            _ => ContextDepth::L3,
        };

        let expander = ContextExpander::new(&index, &graph);
        let changes: Vec<DiffChange> = changed
            .iter()
            .map(|f| DiffChange {
                file_path: f.path.clone(),
                old_path: f.old_path.clone(),
                line_ranges: vec![],
                change_type: match f.status {
                    EngineFileStatus::Added => ChangeType::Added,
                    EngineFileStatus::Deleted => ChangeType::Deleted,
                    _ => ChangeType::Modified,
                },
                diff_content: None,
            })
            .collect();

        let context = expander.expand(&changes, context_depth, budget);

        // Combine changed and dependent symbols
        for s in context
            .changed_symbols
            .iter()
            .chain(context.dependent_symbols.iter())
        {
            let sym_dict = PyDict::new(py);
            sym_dict.set_item("name", &s.name)?;
            sym_dict.set_item("kind", &s.kind)?;
            sym_dict.set_item("file", &s.file_path)?;
            sym_dict.set_item("line", s.start_line)?;
            sym_dict.set_item("reason", &s.relevance_reason)?;
            if let Some(ref sig) = s.signature {
                sym_dict.set_item("signature", sig)?;
            }
            context_symbols.push(sym_dict.into());
        }

        related_tests = context
            .related_tests
            .iter()
            .map(|f| f.path.clone())
            .collect();
    }

    // Calculate tokens
    let tokenizer = Tokenizer::new();
    let total_content: String = changed_files_result
        .iter()
        .filter_map(|d| d.get_item("diff").ok().flatten())
        .filter_map(|item| item.extract::<String>().ok())
        .collect::<Vec<_>>()
        .join("\n");
    let total_tokens = tokenizer.count(&total_content, TokenModel::Claude);

    // Build result dict
    let dict = PyDict::new(py);
    dict.set_item("changed_files", changed_files_result)?;
    dict.set_item("context_symbols", context_symbols)?;
    dict.set_item("related_tests", related_tests)?;
    dict.set_item("total_tokens", total_tokens)?;

    Ok(dict.into())
}

/// Python module definition
#[pymodule]
fn _infiniloom(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Core Functions
    m.add_function(wrap_pyfunction!(pack, m)?)?;
    m.add_function(wrap_pyfunction!(scan, m)?)?;
    m.add_function(wrap_pyfunction!(count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(scan_security, m)?)?;
    m.add_function(wrap_pyfunction!(semantic_compress, m)?)?;
    m.add_function(wrap_pyfunction!(is_git_repo, m)?)?;

    // Index API
    m.add_function(wrap_pyfunction!(build_index, m)?)?;
    m.add_function(wrap_pyfunction!(index_status, m)?)?;

    // Call Graph API
    m.add_function(wrap_pyfunction!(find_symbol, m)?)?;
    m.add_function(wrap_pyfunction!(get_callers, m)?)?;
    m.add_function(wrap_pyfunction!(get_callees, m)?)?;
    m.add_function(wrap_pyfunction!(get_references, m)?)?;
    m.add_function(wrap_pyfunction!(get_call_graph, m)?)?;

    // Chunk API
    m.add_function(wrap_pyfunction!(chunk, m)?)?;

    // Impact & Diff Context API
    m.add_function(wrap_pyfunction!(analyze_impact, m)?)?;
    m.add_function(wrap_pyfunction!(get_diff_context, m)?)?;

    // Classes
    m.add_class::<Infiniloom>()?;
    m.add_class::<GitRepo>()?;

    // Exceptions
    m.add("InfiniloomError", _py.get_type::<InfiniloomError>())?;

    Ok(())
}
