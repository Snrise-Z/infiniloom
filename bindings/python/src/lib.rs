//! Python bindings for Infiniloom
//!
//! This module provides Python bindings using PyO3 for the Infiniloom engine.

#![allow(non_local_definitions)]

use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;

// Import from infiniloom-engine
use infiniloom_engine::{
    git::{GitRepo as EngineGitRepo, FileStatus as EngineFileStatus},
    tokenizer::TokenModel, CompressionLevel, HeuristicCompressor, OutputFormat, OutputFormatter,
    RepoMapGenerator, Repository, SecurityScanner, SemanticCompressor, SemanticConfig, Tokenizer,
    TokenizerModel, Symbol, SymbolKind, Visibility,
};

mod scanner;
use scanner::{scan_repository, ScanConfig};

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
    // Parse format
    let output_format = match format.to_lowercase().as_str() {
        "xml" => OutputFormat::Xml,
        "markdown" | "md" => OutputFormat::Markdown,
        "json" => OutputFormat::Json,
        "yaml" | "yml" => OutputFormat::Yaml,
        "toon" => OutputFormat::Toon,
        "plain" | "text" | "txt" => OutputFormat::Plain,
        _ => return Err(PyValueError::new_err(format!("Invalid format: {}. Use 'xml', 'markdown', 'json', 'yaml', 'toon', or 'plain'", format))),
    };

    // Parse model
    let tokenizer_model = match model.to_lowercase().as_str() {
        "claude" => TokenizerModel::Claude,
        // GPT-5.x series (latest)
        "gpt-5.2" | "gpt5.2" | "gpt52" => TokenizerModel::Gpt52,
        "gpt-5.2-pro" | "gpt52-pro" => TokenizerModel::Gpt52Pro,
        "gpt-5.1" | "gpt5.1" | "gpt51" => TokenizerModel::Gpt51,
        "gpt-5.1-mini" | "gpt51-mini" => TokenizerModel::Gpt51Mini,
        "gpt-5.1-codex" | "gpt51-codex" => TokenizerModel::Gpt51Codex,
        "gpt-5" | "gpt5" => TokenizerModel::Gpt5,
        "gpt-5-mini" | "gpt5-mini" => TokenizerModel::Gpt5Mini,
        "gpt-5-nano" | "gpt5-nano" => TokenizerModel::Gpt5Nano,
        // O-series reasoning models
        "o4-mini" => TokenizerModel::O4Mini,
        "o3" => TokenizerModel::O3,
        "o3-mini" => TokenizerModel::O3Mini,
        "o1" => TokenizerModel::O1,
        "o1-mini" => TokenizerModel::O1Mini,
        "o1-preview" => TokenizerModel::O1Preview,
        // GPT-4 series
        "gpt-4o" | "gpt4o" => TokenizerModel::Gpt4o,
        "gpt-4o-mini" | "gpt4o-mini" => TokenizerModel::Gpt4oMini,
        "gpt" | "gpt-4" | "gpt4" => TokenizerModel::Gpt4,
        "gpt-3.5-turbo" | "gpt35-turbo" => TokenizerModel::Gpt35Turbo,
        // Other vendors
        "gemini" => TokenizerModel::Gemini,
        "llama" => TokenizerModel::Llama,
        "codellama" => TokenizerModel::CodeLlama,
        "mistral" => TokenizerModel::Mistral,
        "deepseek" => TokenizerModel::DeepSeek,
        "qwen" => TokenizerModel::Qwen,
        "cohere" => TokenizerModel::Cohere,
        "grok" => TokenizerModel::Grok,
        _ => return Err(PyValueError::new_err(format!("Invalid model: {}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, mistral, deepseek, qwen, cohere, grok", model))),
    };

    // Parse compression level
    let compression_level = match compression.to_lowercase().as_str() {
        "none" => CompressionLevel::None,
        "minimal" => CompressionLevel::Minimal,
        "balanced" => CompressionLevel::Balanced,
        "aggressive" => CompressionLevel::Aggressive,
        "extreme" => CompressionLevel::Extreme,
        "focused" => CompressionLevel::Focused,
        "semantic" => CompressionLevel::Semantic,
        _ => return Err(PyValueError::new_err(format!("Invalid compression: {}", compression))),
    };

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

    // Redact secrets from file content if enabled
    if redact_secrets {
        let scanner = SecurityScanner::new();
        for file in &mut repo.files {
            if let Some(ref content) = file.content {
                let redacted = scanner.redact_content(content, &file.relative_path);
                file.content = Some(redacted);
            }
        }
    }

    // Apply compression to file contents based on compression level
    match compression_level {
        CompressionLevel::None => {
            // No compression - keep content as-is
        }
        CompressionLevel::Minimal => {
            // Remove empty lines
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        }
        CompressionLevel::Balanced => {
            // Remove empty lines and comments (basic heuristic)
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| {
                            let trimmed = line.trim();
                            !trimmed.is_empty()
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with('#')
                                && !trimmed.starts_with("/*")
                                && !trimmed.starts_with('*')
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        }
        CompressionLevel::Aggressive | CompressionLevel::Extreme => {
            // Extract signatures only - keep function/class definitions
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    file.content = Some(signature_lines(content));
                }
            }
        }
        CompressionLevel::Focused => {
            // Key symbols with small surrounding context
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let focused = focused_symbol_context(content, &file.symbols);
                    file.content = Some(focused);
                }
            }
        }
        CompressionLevel::Semantic => {
            // Use heuristic-based semantic compression
            let compressor = HeuristicCompressor::new();
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    if let Ok(compressed) = compressor.compress(content) {
                        file.content = Some(compressed);
                    }
                }
            }
        }
    }

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
    let config = ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: false,
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
    let token_model = match model.to_lowercase().as_str() {
        "claude" => TokenModel::Claude,
        // GPT-5.x series (latest)
        "gpt-5.2" | "gpt5.2" | "gpt52" => TokenModel::Gpt52,
        "gpt-5.2-pro" | "gpt52-pro" => TokenModel::Gpt52Pro,
        "gpt-5.1" | "gpt5.1" | "gpt51" => TokenModel::Gpt51,
        "gpt-5.1-mini" | "gpt51-mini" => TokenModel::Gpt51Mini,
        "gpt-5.1-codex" | "gpt51-codex" => TokenModel::Gpt51Codex,
        "gpt-5" | "gpt5" => TokenModel::Gpt5,
        "gpt-5-mini" | "gpt5-mini" => TokenModel::Gpt5Mini,
        "gpt-5-nano" | "gpt5-nano" => TokenModel::Gpt5Nano,
        // O-series reasoning models
        "o4-mini" => TokenModel::O4Mini,
        "o3" => TokenModel::O3,
        "o3-mini" => TokenModel::O3Mini,
        "o1" => TokenModel::O1,
        "o1-mini" => TokenModel::O1Mini,
        "o1-preview" => TokenModel::O1Preview,
        // GPT-4 series
        "gpt-4o" | "gpt4o" => TokenModel::Gpt4o,
        "gpt-4o-mini" | "gpt4o-mini" => TokenModel::Gpt4oMini,
        "gpt" | "gpt-4" | "gpt4" => TokenModel::Gpt4,
        "gpt-3.5-turbo" | "gpt35-turbo" | "gpt35turbo" => TokenModel::Gpt35Turbo,
        // Other vendors
        "gemini" => TokenModel::Gemini,
        "llama" => TokenModel::Llama,
        "codellama" => TokenModel::CodeLlama,
        "mistral" => TokenModel::Mistral,
        "deepseek" => TokenModel::DeepSeek,
        "qwen" => TokenModel::Qwen,
        "cohere" => TokenModel::Cohere,
        "grok" => TokenModel::Grok,
        _ => return Err(PyValueError::new_err(format!("Invalid model: {}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, codellama, mistral, deepseek, qwen, cohere, grok", model))),
    };

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
        skip_symbols: true, // Fast mode for security scan
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
            dict.set_item("severity", format!("{:?}", finding.severity)).unwrap();
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
fn semantic_compress(
    text: &str,
    similarity_threshold: f32,
    budget_ratio: f32,
) -> PyResult<String> {
    let config = SemanticConfig {
        similarity_threshold,
        budget_ratio,
        min_chunk_size: 100,
        max_chunk_size: 2000,
    };

    let compressor = SemanticCompressor::with_config(config);
    compressor.compress(text).map_err(|e| {
        PyValueError::new_err(format!("Compression failed: {}", e))
    })
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

        Ok(Infiniloom {
            path: path_buf,
            repo: None,
        })
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

        // Parse format
        let output_format = match format.to_lowercase().as_str() {
            "xml" => OutputFormat::Xml,
            "markdown" | "md" => OutputFormat::Markdown,
            "json" => OutputFormat::Json,
            "yaml" | "yml" => OutputFormat::Yaml,
            "toon" => OutputFormat::Toon,
            "plain" | "text" | "txt" => OutputFormat::Plain,
            _ => return Err(PyValueError::new_err(format!("Invalid format: {}. Use 'xml', 'markdown', 'json', 'yaml', 'toon', or 'plain'", format))),
        };

        // Parse model
        let tokenizer_model = match model.to_lowercase().as_str() {
            "claude" => TokenizerModel::Claude,
            // GPT-5.x series (latest)
            "gpt-5.2" | "gpt5.2" | "gpt52" => TokenizerModel::Gpt52,
            "gpt-5.2-pro" | "gpt52-pro" => TokenizerModel::Gpt52Pro,
            "gpt-5.1" | "gpt5.1" | "gpt51" => TokenizerModel::Gpt51,
            "gpt-5.1-mini" | "gpt51-mini" => TokenizerModel::Gpt51Mini,
            "gpt-5.1-codex" | "gpt51-codex" => TokenizerModel::Gpt51Codex,
            "gpt-5" | "gpt5" => TokenizerModel::Gpt5,
            "gpt-5-mini" | "gpt5-mini" => TokenizerModel::Gpt5Mini,
            "gpt-5-nano" | "gpt5-nano" => TokenizerModel::Gpt5Nano,
            // O-series reasoning models
            "o4-mini" => TokenizerModel::O4Mini,
            "o3" => TokenizerModel::O3,
            "o3-mini" => TokenizerModel::O3Mini,
            "o1" => TokenizerModel::O1,
            "o1-mini" => TokenizerModel::O1Mini,
            "o1-preview" => TokenizerModel::O1Preview,
            // GPT-4 series
            "gpt-4o" | "gpt4o" => TokenizerModel::Gpt4o,
            "gpt-4o-mini" | "gpt4o-mini" => TokenizerModel::Gpt4oMini,
            "gpt" | "gpt-4" | "gpt4" => TokenizerModel::Gpt4,
            "gpt-3.5-turbo" | "gpt35-turbo" => TokenizerModel::Gpt35Turbo,
            // Other vendors
            "gemini" => TokenizerModel::Gemini,
            "llama" => TokenizerModel::Llama,
            "codellama" => TokenizerModel::CodeLlama,
            "mistral" => TokenizerModel::Mistral,
            "deepseek" => TokenizerModel::DeepSeek,
            "qwen" => TokenizerModel::Qwen,
            "cohere" => TokenizerModel::Cohere,
            "grok" => TokenizerModel::Grok,
            _ => return Err(PyValueError::new_err(format!("Invalid model: {}. Supported: gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4, claude, gemini, llama, mistral, deepseek, qwen, cohere, grok", model))),
        };

        // Parse and apply compression level
        let compression_level = match compression.to_lowercase().as_str() {
            "none" => CompressionLevel::None,
            "minimal" => CompressionLevel::Minimal,
            "balanced" => CompressionLevel::Balanced,
            "aggressive" => CompressionLevel::Aggressive,
            "extreme" => CompressionLevel::Extreme,
            "focused" => CompressionLevel::Focused,
            "semantic" => CompressionLevel::Semantic,
            _ => return Err(PyValueError::new_err(format!("Invalid compression: {}", compression))),
        };

        // Apply compression to file contents
        match compression_level {
            CompressionLevel::None => {}
            CompressionLevel::Minimal => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let compressed: String = content
                            .lines()
                            .filter(|line| !line.trim().is_empty())
                            .collect::<Vec<_>>()
                            .join("\n");
                        file.content = Some(compressed);
                    }
                }
            }
            CompressionLevel::Balanced => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let compressed: String = content
                            .lines()
                            .filter(|line| {
                                let trimmed = line.trim();
                                !trimmed.is_empty()
                                    && !trimmed.starts_with("//")
                                    && !trimmed.starts_with('#')
                                    && !trimmed.starts_with("/*")
                                    && !trimmed.starts_with('*')
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        file.content = Some(compressed);
                    }
                }
            }
            CompressionLevel::Aggressive | CompressionLevel::Extreme => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        file.content = Some(signature_lines(content));
                    }
                }
            }
            CompressionLevel::Focused => {
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        let focused = focused_symbol_context(content, &file.symbols);
                        file.content = Some(focused);
                    }
                }
            }
            CompressionLevel::Semantic => {
                let compressor = HeuristicCompressor::new();
                for file in &mut repo.files {
                    if let Some(ref content) = file.content {
                        if let Ok(compressed) = compressor.compress(content) {
                            file.content = Some(compressed);
                        }
                    }
                }
            }
        }

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

        let repo = self.repo.as_ref().unwrap();
        let generator = RepoMapGenerator::builder()
            .token_budget(map_budget)
            .max_symbols(max_symbols)
            .build();
        let map = generator.generate(repo);

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
                dict.set_item("severity", format!("{:?}", finding.severity)).unwrap();
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
                dict.set_item("status", format_file_status(f.status)).unwrap();
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
                dict.set_item("status", format_file_status(f.status)).unwrap();
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
        self.inner.diff_content(from_ref, to_ref, path).map_err(to_py_err)
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
        self.inner.file_change_frequency(path, days).map_err(to_py_err)
    }

    fn __repr__(&self) -> String {
        "GitRepo(<git repository>)".to_string()
    }
}

fn signature_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("export ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("type ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn focused_symbol_context(content: &str, symbols: &[Symbol]) -> String {
    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return signature_lines(content);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return String::new();
    }

    let key_symbols: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Function
                    | SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::Interface
            ) && s.visibility != Visibility::Private
        })
        .collect();

    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20)
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect()
    };

    #[derive(Clone)]
    struct SymbolRange {
        start: u32,
        end: u32,
        labels: Vec<String>,
    }

    let mut ranges = Vec::new();
    let mut fallback_snippets = Vec::new();

    for symbol in symbols_to_use {
        let label = format!("{}: {}", symbol.kind.name(), symbol.name);
        if symbol.start_line > 0
            && symbol.end_line >= symbol.start_line
            && symbol.start_line <= total_lines
        {
            let start = symbol.start_line.saturating_sub(CONTEXT_LINES).max(1);
            let end = symbol
                .end_line
                .max(symbol.start_line)
                .saturating_add(CONTEXT_LINES)
                .min(total_lines);
            ranges.push(SymbolRange {
                start,
                end,
                labels: vec![label],
            });
        } else if let Some(ref sig) = symbol.signature {
            fallback_snippets.push(format!("// {}\n{}", label, sig.trim()));
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return signature_lines(content);
    }

    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SymbolRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                for label in range.labels {
                    if !last.labels.contains(&label) {
                        last.labels.push(label);
                    }
                }
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    let mut result = String::new();
    for range in merged {
        result.push_str(&format!(
            "// Focused symbols: {}\n",
            range.labels.join(", ")
        ));
        let start_idx = range.start.saturating_sub(1) as usize;
        let end_idx = range.end.saturating_sub(1) as usize;
        if start_idx <= end_idx && end_idx < lines.len() {
            result.push_str(&lines[start_idx..=end_idx].join("\n"));
            result.push('\n');
        }
        result.push('\n');
    }

    if !fallback_snippets.is_empty() {
        result.push_str("// Additional signatures\n");
        for snippet in fallback_snippets {
            result.push_str(&snippet);
            result.push('\n');
        }
    }

    result
}

/// Format FileStatus as string
fn format_file_status(status: EngineFileStatus) -> &'static str {
    match status {
        EngineFileStatus::Added => "Added",
        EngineFileStatus::Modified => "Modified",
        EngineFileStatus::Deleted => "Deleted",
        EngineFileStatus::Renamed => "Renamed",
        EngineFileStatus::Copied => "Copied",
        EngineFileStatus::Unknown => "Unknown",
    }
}

/// Python module definition
#[pymodule]
fn _infiniloom(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Functions
    m.add_function(wrap_pyfunction!(pack, m)?)?;
    m.add_function(wrap_pyfunction!(scan, m)?)?;
    m.add_function(wrap_pyfunction!(count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(scan_security, m)?)?;
    m.add_function(wrap_pyfunction!(semantic_compress, m)?)?;
    m.add_function(wrap_pyfunction!(is_git_repo, m)?)?;

    // Classes
    m.add_class::<Infiniloom>()?;
    m.add_class::<GitRepo>()?;

    // Exceptions
    m.add("InfiniloomError", _py.get_type::<InfiniloomError>())?;

    Ok(())
}
