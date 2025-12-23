//! Infiniloom CLI - Repository context generator for LLMs
//!
//! This CLI tool generates optimized repository context for AI assistants.

// CLI tools legitimately use print macros for user output
#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use humansize::{format_size, BINARY};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;

mod scanner;

use infiniloom_engine::{
    git::GitRepo,
    index::{
        BuildOptions, ChangeType, ContextDepth, ContextExpander, ContextSnippet, DiffChange,
        IndexBuilder, IndexStorage, LazyContextBuilder,
    },
    output::{OutputFormat, OutputFormatter},
    remote::RemoteRepo,
    repomap::RepoMapGenerator,
    security::SecurityScanner,
    tokenizer::{TokenModel, Tokenizer},
    types::{CompressionLevel, TokenizerModel},
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{self, BufRead};

/// Pre-compiled regex for base64 content detection
/// Matches:
/// - Data URIs: data:image/png;base64,...
/// - Standalone base64: 200+ chars (filtered in callback for + or / to avoid matching hex)
static BASE64_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Two patterns:
    // 1. Data URIs (always match - explicit base64 marker)
    // 2. Standalone base64: 200+ chars - callback filters for + or / to distinguish from hex
    Regex::new(r"data:[^;]+;base64,[A-Za-z0-9+/]*={0,2}|[A-Za-z0-9+/]{200,}={0,2}").unwrap()
});

/// Infiniloom - Repository context generator for LLMs
#[derive(Parser)]
#[command(
    name = "infiniloom",
    version,
    about = "Generate optimized repository context for LLMs",
    long_about = "Infiniloom transforms codebases into LLM-friendly formats with intelligent\ncompression, symbol ranking, and model-specific optimizations."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack a repository into LLM-friendly format
    Pack {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format (default: xml, or from config file)
        #[arg(short, long, value_enum)]
        format: Option<Format>,

        /// Target model for optimization (default: claude, or from config file)
        #[arg(short, long, value_enum)]
        model: Option<Model>,

        /// Compression level (default: balanced, or from config file)
        #[arg(short, long, value_enum)]
        compression: Option<Compression>,

        /// Maximum output tokens (0 = no limit, default)
        #[arg(short = 't', long, alias = "budget", short_alias = 'b', default_value = "0")]
        max_tokens: u32,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include hidden files
        #[arg(long)]
        hidden: bool,

        /// Don't respect .gitignore
        #[arg(long)]
        no_gitignore: bool,

        /// Enable symbol extraction (slower, but provides better repo map)
        #[arg(long)]
        symbols: bool,

        /// Enable full analysis mode (symbols + repo map + PageRank ranking)
        #[arg(long)]
        full: bool,

        /// Exclude file contents from output (metadata only)
        #[arg(long)]
        no_content: bool,

        /// Explicitly skip symbol extraction (overrides --full)
        #[arg(long)]
        no_symbols: bool,

        /// Include test files (excluded by default)
        #[arg(long)]
        include_tests: bool,

        /// Include documentation files (excluded by default)
        #[arg(long)]
        include_docs: bool,

        /// Disable default ignore patterns (node_modules, dist, etc.)
        #[arg(long)]
        no_default_ignores: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Custom header text to include at the top
        #[arg(long)]
        header_text: Option<String>,

        /// Path to file containing custom instructions
        #[arg(long)]
        instruction_file: Option<PathBuf>,

        /// Copy output to clipboard
        #[arg(long)]
        copy_to_clipboard: bool,

        /// Show token count breakdown by file
        #[arg(long)]
        token_tree: bool,

        /// Hide directory structure from output
        #[arg(long)]
        no_directory_structure: bool,

        /// Hide file summary from output
        #[arg(long)]
        no_file_summary: bool,

        /// Remove empty lines from code
        #[arg(long)]
        remove_empty_lines: bool,

        /// Remove comments from code
        #[arg(long)]
        remove_comments: bool,

        /// Limit number of files in summary (0 = all)
        #[arg(long, default_value = "0")]
        top_files: usize,

        /// Include git commit history in output
        #[arg(long)]
        include_logs: bool,

        /// Number of git log entries to include
        #[arg(long, default_value = "50")]
        logs_count: usize,

        /// Include git diffs in output
        #[arg(long)]
        include_diffs: bool,

        /// Sort files by git change frequency
        #[arg(long)]
        sort_by_changes: bool,

        /// Read file paths from stdin (one per line)
        #[arg(long)]
        stdin: bool,

        /// Truncate base64 encoded content
        #[arg(long)]
        truncate_base64: bool,

        /// Include only files matching glob pattern (can be repeated)
        #[arg(long = "include", short = 'i')]
        include_patterns: Vec<String>,

        /// Exclude files matching glob pattern (can be repeated)
        #[arg(long = "exclude", short = 'e')]
        exclude_patterns: Vec<String>,

        /// Scan for security issues (secrets, API keys)
        #[arg(long)]
        security_check: bool,

        /// Branch to checkout for remote repositories
        #[arg(long)]
        remote_branch: Option<String>,

        /// Sparse checkout paths for remote repositories (only fetch specified directories)
        /// Can be repeated: --sparse-path src --sparse-path lib
        /// Dramatically speeds up cloning large monorepos
        #[arg(long = "sparse-path")]
        sparse_paths: Vec<String>,

        /// Enable line numbers in output (default: enabled)
        #[arg(long, conflicts_with = "no_line_numbers")]
        line_numbers: bool,

        /// Disable line numbers in output
        #[arg(long, conflicts_with = "line_numbers")]
        no_line_numbers: bool,

        /// Redact detected secrets in output (replace with [REDACTED])
        #[arg(long)]
        redact_secrets: bool,

        /// Path to config file (default: .infiniloom.yaml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Watch for file changes and regenerate output (prefer output path outside repo to avoid self-trigger loops)
        #[arg(long)]
        watch: bool,

        /// Enable incremental caching (speeds up repeated scans)
        #[arg(long)]
        cache: bool,

        /// Token budget for repository map (default: 2000)
        #[arg(long, default_value = "2000")]
        map_budget: u32,
    },

    /// Scan a repository and show statistics
    Scan {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Target model for token counting
        #[arg(short, long, value_enum, default_value = "claude")]
        model: Model,

        /// Include hidden files
        #[arg(long)]
        hidden: bool,

        /// Show detailed file list
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Scan for security issues (secrets, API keys)
        #[arg(long)]
        security_check: bool,

        /// Sample N random files instead of full scan (for large repos)
        #[arg(long, conflicts_with = "sample_percent")]
        sample: Option<usize>,

        /// Sample P percent of files instead of full scan (for large repos)
        #[arg(long, conflicts_with = "sample")]
        sample_percent: Option<f64>,
    },

    /// Generate a repository map (symbol index)
    Map {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Token budget for map
        #[arg(short, long, default_value = "2000")]
        budget: u32,

        /// Target model for token counting (default: claude, or from config file)
        #[arg(short, long, value_enum)]
        model: Option<Model>,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show version and configuration info
    Info {
        /// Optional path to show project-specific info (default: general info only)
        path: Option<PathBuf>,
    },

    /// Initialize a new configuration file
    Init {
        /// Directory to create config in (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Configuration format
        #[arg(short, long, value_enum, default_value = "yaml")]
        format: ConfigFormat,

        /// Project template (pre-configured settings for common project types)
        #[arg(short, long, value_enum, default_value = "generic")]
        template: ConfigTemplate,

        /// Output path (overrides path argument, default: .infiniloom.yaml in path)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },

    /// Build or update the symbol index for fast diff context
    Index {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Force full rebuild (ignore existing index)
        #[arg(long)]
        force: bool,

        /// Show index status without rebuilding
        #[arg(long)]
        status: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Watch for file changes and automatically re-index
        #[arg(long)]
        watch: bool,
    },

    /// Get context for a diff (changed files, dependents, tests)
    Diff {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Diff reference (e.g., HEAD~1, main..feature, commit hash)
        /// If not specified, uses unstaged changes
        #[arg()]
        reference: Option<String>,

        /// Use staged changes
        #[arg(long)]
        staged: bool,

        /// Context depth: 1=containing, 2=direct deps, 3=transitive
        #[arg(short, long, default_value = "2")]
        depth: u8,

        /// Token budget for context
        #[arg(short, long, default_value = "50000")]
        budget: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "xml")]
        format: Format,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include actual diff content (the +/- lines) in output
        #[arg(long)]
        include_diff: bool,

        /// Target model for token counting (default: claude, or from config file)
        #[arg(short, long, value_enum)]
        model: Option<Model>,

        /// Include recent commit history for each changed file
        #[arg(long)]
        include_history: bool,

        /// Number of recent commits to include per file (default: 3)
        #[arg(long, default_value = "3")]
        history_count: usize,
    },

    /// Analyze impact of changes to a file or symbol
    Impact {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// File or symbol to analyze
        target: Option<String>,

        /// Analyze a symbol instead of a file
        #[arg(long)]
        symbol: bool,

        /// Show call graph
        #[arg(long)]
        call_graph: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Split repository into chunks for multi-turn LLM conversations
    Chunk {
        /// Path to repository (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Chunking strategy
        #[arg(short, long, value_enum, default_value = "semantic")]
        strategy: ChunkingStrategy,

        /// Maximum tokens per chunk
        #[arg(short = 't', long, default_value = "8000")]
        max_tokens: u32,

        /// Overlap tokens between chunks (for context continuity)
        #[arg(long, default_value = "0")]
        overlap: u32,

        /// Target model for token counting
        #[arg(short, long, value_enum, default_value = "claude")]
        model: Model,

        /// Output format
        #[arg(short, long, value_enum, default_value = "xml")]
        format: Format,

        /// Output directory (default: stdout as numbered files)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Disable chunk summary headers
        #[arg(long)]
        no_chunk_summary: bool,

        /// Sort chunks by priority (core modules first, tests last)
        #[arg(long)]
        priority_first: bool,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ConfigFormat {
    /// YAML format
    Yaml,
    /// TOML format
    Toml,
    /// JSON format
    Json,
}

/// Configuration template for common project types
#[derive(ValueEnum, Clone, Copy)]
enum ConfigTemplate {
    /// Generic template (default)
    Generic,
    /// Rust project (Cargo.toml, *.rs)
    Rust,
    /// Python project (*.py, requirements.txt)
    Python,
    /// TypeScript/JavaScript project (*.ts, *.tsx, package.json)
    Typescript,
    /// Go project (*.go, go.mod)
    Go,
    /// Java project (*.java, pom.xml/build.gradle)
    Java,
}

#[derive(ValueEnum, Clone, Copy)]
enum ChunkingStrategy {
    /// Fixed token size chunks
    Fixed,
    /// One file per chunk
    File,
    /// Group by module/directory
    Module,
    /// Group by symbols (AST-based)
    Symbol,
    /// Group by semantic similarity (default)
    Semantic,
    /// Group by dependency order
    Dependency,
}

impl From<ChunkingStrategy> for infiniloom_engine::ChunkStrategy {
    fn from(s: ChunkingStrategy) -> Self {
        match s {
            ChunkingStrategy::Fixed => Self::Fixed { size: 8000 },
            ChunkingStrategy::File => Self::File,
            ChunkingStrategy::Module => Self::Module,
            ChunkingStrategy::Symbol => Self::Symbol,
            ChunkingStrategy::Semantic => Self::Semantic,
            ChunkingStrategy::Dependency => Self::Dependency,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Format {
    /// XML format (Claude-optimized)
    Xml,
    /// Markdown format (GPT-optimized)
    Markdown,
    /// JSON format (generic)
    Json,
    /// YAML format (Gemini-optimized)
    Yaml,
    /// TOON format (most token-efficient, 40% smaller)
    Toon,
    /// Plain text format (simple, no formatting)
    Plain,
}

impl From<Format> for OutputFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Xml => OutputFormat::Xml,
            Format::Markdown => OutputFormat::Markdown,
            Format::Json => OutputFormat::Json,
            Format::Yaml => OutputFormat::Yaml,
            Format::Toon => OutputFormat::Toon,
            Format::Plain => OutputFormat::Plain,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Model {
    // Anthropic
    Claude,
    // OpenAI o200k_base models (modern)
    #[value(name = "gpt52")]
    Gpt52,
    #[value(name = "gpt51")]
    Gpt51,
    #[value(name = "gpt5")]
    Gpt5,
    #[value(name = "o4-mini")]
    O4Mini,
    #[value(name = "o3")]
    O3,
    #[value(name = "o1")]
    O1,
    #[value(name = "gpt4o", alias = "gpt-4o")]
    Gpt4o,
    #[value(name = "gpt4o-mini", alias = "gpt-4o-mini")]
    Gpt4oMini,
    // OpenAI cl100k_base models (legacy)
    #[value(name = "gpt4", alias = "gpt-4")]
    Gpt4,
    #[value(name = "gpt35-turbo")]
    Gpt35Turbo,
    // Google
    Gemini,
    // Meta
    Llama,
    #[value(name = "codellama")]
    CodeLlama,
    // Other vendors
    Mistral,
    DeepSeek,
    Qwen,
    Cohere,
    Grok,
}

impl From<Model> for TokenizerModel {
    fn from(m: Model) -> Self {
        match m {
            // Anthropic
            Model::Claude => TokenizerModel::Claude,
            // OpenAI o200k_base models (modern)
            Model::Gpt52 => TokenizerModel::Gpt52,
            Model::Gpt51 => TokenizerModel::Gpt51,
            Model::Gpt5 => TokenizerModel::Gpt5,
            Model::O4Mini => TokenizerModel::O4Mini,
            Model::O3 => TokenizerModel::O3,
            Model::O1 => TokenizerModel::O1,
            Model::Gpt4o => TokenizerModel::Gpt4o,
            Model::Gpt4oMini => TokenizerModel::Gpt4oMini,
            // OpenAI cl100k_base models (legacy)
            Model::Gpt4 => TokenizerModel::Gpt4,
            Model::Gpt35Turbo => TokenizerModel::Gpt35Turbo,
            // Google
            Model::Gemini => TokenizerModel::Gemini,
            // Meta
            Model::Llama => TokenizerModel::Llama,
            Model::CodeLlama => TokenizerModel::CodeLlama,
            // Other vendors
            Model::Mistral => TokenizerModel::Mistral,
            Model::DeepSeek => TokenizerModel::DeepSeek,
            Model::Qwen => TokenizerModel::Qwen,
            Model::Cohere => TokenizerModel::Cohere,
            Model::Grok => TokenizerModel::Grok,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Compression {
    /// No compression
    None,
    /// Minimal: remove empty lines
    Minimal,
    /// Balanced: remove comments
    Balanced,
    /// Aggressive: signatures only
    Aggressive,
    /// Extreme: key symbols only
    Extreme,
    /// Focused: key symbols with small context
    Focused,
    /// Semantic: heuristic chunking (char-frequency, NOT neural)
    Semantic,
}

impl From<Compression> for CompressionLevel {
    fn from(c: Compression) -> Self {
        match c {
            Compression::None => CompressionLevel::None,
            Compression::Minimal => CompressionLevel::Minimal,
            Compression::Balanced => CompressionLevel::Balanced,
            Compression::Aggressive => CompressionLevel::Aggressive,
            Compression::Extreme => CompressionLevel::Extreme,
            Compression::Focused => CompressionLevel::Focused,
            Compression::Semantic => CompressionLevel::Semantic,
        }
    }
}

fn main() -> Result<()> {
    // Initialize logging
    let default_filter = if std::env::var("INFINILOOM_TIMING").is_ok() {
        "info"
    } else {
        "warn"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Pack {
            path,
            format,
            model,
            compression,
            max_tokens,
            output,
            hidden,
            no_gitignore,
            symbols,
            full,
            no_content,
            no_symbols,
            include_tests,
            include_docs,
            no_default_ignores,
            verbose,
            header_text,
            instruction_file,
            copy_to_clipboard,
            token_tree,
            no_directory_structure,
            no_file_summary,
            remove_empty_lines,
            remove_comments,
            top_files,
            include_logs,
            logs_count,
            include_diffs,
            sort_by_changes,
            stdin,
            truncate_base64,
            include_patterns,
            exclude_patterns,
            security_check,
            remote_branch,
            sparse_paths,
            line_numbers,
            no_line_numbers,
            redact_secrets,
            config,
            watch,
            cache,
            map_budget,
        } => cmd_pack(
            path,
            format,
            model,
            compression,
            max_tokens,
            output,
            hidden,
            !no_gitignore,
            (symbols || full) && !no_symbols, // Enable symbols unless --no-symbols
            full && !no_symbols,              // Full mode disabled if --no-symbols
            no_content,                       // Exclude file contents
            include_tests,
            include_docs,
            !no_default_ignores,
            verbose,
            header_text,
            instruction_file,
            copy_to_clipboard,
            token_tree,
            !no_directory_structure,
            !no_file_summary,
            remove_empty_lines,
            remove_comments,
            top_files,
            include_logs,
            logs_count,
            include_diffs,
            sort_by_changes,
            stdin,
            truncate_base64,
            include_patterns,
            exclude_patterns,
            security_check,
            remote_branch,
            sparse_paths,
            line_numbers || !no_line_numbers, // line_numbers explicit OR not disabled
            redact_secrets,
            config,
            watch,
            cache,
            map_budget,
        ),
        Commands::Scan {
            path,
            model,
            hidden,
            verbose,
            json,
            security_check,
            sample,
            sample_percent,
        } => cmd_scan(
            path,
            model.into(),
            hidden,
            verbose,
            json,
            security_check,
            sample,
            sample_percent,
        ),
        Commands::Map { path, budget, model, output } => cmd_map(path, budget, model, output),
        Commands::Info { path } => cmd_info(path),
        Commands::Init { path, format, template, output, force } => {
            cmd_init(path, format, template, output, force)
        },
        Commands::Index { path, force, status, verbose, watch } => {
            cmd_index(path, force, status, verbose, watch)
        },
        Commands::Diff {
            path,
            reference,
            staged,
            depth,
            budget,
            format,
            output,
            include_diff,
            model,
            include_history,
            history_count,
        } => cmd_diff(
            path,
            reference,
            staged,
            depth,
            budget,
            format.into(),
            output,
            include_diff,
            model,
            include_history,
            history_count,
        ),
        Commands::Impact { path, target, symbol, call_graph, json } => {
            cmd_impact(path, target, symbol, call_graph, json)
        },
        Commands::Chunk {
            path,
            strategy,
            max_tokens,
            overlap,
            model,
            format,
            output,
            verbose,
            no_chunk_summary,
            priority_first,
        } => cmd_chunk(
            path,
            strategy.into(),
            max_tokens,
            overlap,
            model.into(),
            format.into(),
            output,
            verbose,
            no_chunk_summary,
            priority_first,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_pack(
    path: PathBuf,
    cli_format: Option<Format>,
    cli_model: Option<Model>,
    cli_compression: Option<Compression>,
    max_tokens: u32,
    // Note: format, model, compression are converted from CLI defaults before this function.
    // Config file overrides are applied inside this function when CLI uses defaults.
    output: Option<PathBuf>,
    include_hidden: bool,
    respect_gitignore: bool,
    enable_symbols: bool,
    full_mode: bool,
    exclude_content: bool,
    include_tests: bool,
    include_docs: bool,
    use_default_ignores: bool,
    verbose: bool,
    header_text: Option<String>,
    instruction_file: Option<PathBuf>,
    copy_to_clipboard: bool,
    token_tree: bool,
    show_directory_structure: bool,
    show_file_summary: bool,
    remove_empty_lines: bool,
    remove_comments: bool,
    top_files: usize,
    include_logs: bool,
    logs_count: usize,
    include_diffs: bool,
    sort_by_changes: bool,
    stdin: bool,
    truncate_base64: bool,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    security_check: bool,
    remote_branch: Option<String>,
    sparse_paths: Vec<String>,
    show_line_numbers: bool,
    redact_secrets: bool,
    config_path: Option<PathBuf>,
    watch_mode: bool,
    incremental_cache: bool,
    map_budget: u32,
) -> Result<()> {
    let start = Instant::now();

    // Handle stdin mode - read file paths from stdin
    let stdin_paths: Option<Vec<String>> = if stdin {
        let stdin_handle = io::stdin();
        let paths: Vec<String> = stdin_handle
            .lock()
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    } else {
        None
    };

    if verbose {
        eprintln!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
        eprintln!();
    }

    // Create progress bar with better formatting
    let pb = if verbose {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("Scanning repository...");
        Some(pb)
    } else {
        None
    };

    // Handle remote URL - clone if needed BEFORE loading config
    // This ensures we load config from the cloned repo, not the URL
    // Use clone_with_cleanup for RAII cleanup - the temp_dir must stay alive until we're done
    let (repo_path, _temp_dir) = if RemoteRepo::is_remote_url(path.to_string_lossy().as_ref()) {
        if let Some(pb) = &pb {
            pb.set_message("Cloning remote repository...");
        }
        let mut remote = RemoteRepo::parse(path.to_string_lossy().as_ref())
            .map_err(|e| anyhow::anyhow!("Invalid remote URL: {}", e))?;

        // Override branch if specified via CLI
        if let Some(ref branch) = remote_branch {
            remote.branch = Some(branch.clone());
        }

        if verbose {
            let branch_info = remote.branch.as_deref().unwrap_or("default");
            if sparse_paths.is_empty() {
                eprintln!(
                    "  Cloning {} from {:?} (branch: {})...",
                    remote.name, remote.provider, branch_info
                );
            } else {
                eprintln!(
                    "  Sparse cloning {} from {:?} (branch: {}, paths: {:?})...",
                    remote.name, remote.provider, branch_info, sparse_paths
                );
            }
        }

        // Use sparse clone if paths specified, otherwise full clone
        let (cloned_path, temp_dir) = if !sparse_paths.is_empty() {
            // Create temp directory for sparse clone
            let temp_dir = tempfile::TempDir::with_prefix("infiniloom-sparse-")
                .map_err(|e| anyhow::anyhow!("Failed to create temp dir: {}", e))?;
            let paths_refs: Vec<&str> = sparse_paths.iter().map(|s| s.as_str()).collect();
            let cloned = remote
                .sparse_clone(&paths_refs, Some(temp_dir.path()))
                .map_err(|e| anyhow::anyhow!("Failed to sparse clone repository: {}", e))?;
            (cloned, temp_dir)
        } else {
            // Use clone_with_cleanup for automatic temp directory cleanup when _temp_dir is dropped
            remote
                .clone_with_cleanup()
                .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?
        };

        // Warn about shallow clone limitations when history features are enabled
        let uses_history_features = include_logs || sort_by_changes || include_diffs;
        if uses_history_features {
            eprintln!(
                "{} Remote repositories are cloned with --depth 1 (shallow clone).",
                "Warning:".yellow().bold()
            );
            eprintln!(
                "         History-dependent options (--logs, --sort-by-changes, --include-diffs)"
            );
            eprintln!("         may return incomplete or empty results.");
            eprintln!();
        }

        // Keep temp dir alive by returning it - will be cleaned up when dropped
        (cloned_path, Some(temp_dir))
    } else {
        (path.clone(), None)
    };

    // Load config file AFTER remote URL handling so we read from the actual repo path
    // For remote repos, this reads config from the cloned directory
    let loaded_config = load_config_file(config_path.as_ref(), &repo_path);

    // Apply loaded config values as defaults (CLI args override these)
    // These are mutable so we can apply config file defaults
    let mut include_tests = include_tests;
    let mut include_docs = include_docs;
    let mut security_check = security_check;
    let mut show_line_numbers = show_line_numbers;
    let mut show_directory_structure = show_directory_structure;
    let mut show_file_summary = show_file_summary;
    let mut remove_empty_lines = remove_empty_lines;
    let mut remove_comments = remove_comments;
    let mut max_tokens = max_tokens;
    let mut include_hidden = include_hidden;
    let mut max_file_size = 50 * 1024 * 1024u64; // 50MB default

    // Apply config file values (only if not explicitly set via CLI)
    // For booleans, we apply config if the CLI default is false (not overridden)
    if !include_tests {
        include_tests = loaded_config.include_tests.unwrap_or(false);
    }
    if !include_docs {
        include_docs = loaded_config.include_docs.unwrap_or(false);
    }
    if !security_check {
        security_check = loaded_config.security_check.unwrap_or(false);
    }
    // Apply include_hidden from config
    if !include_hidden {
        include_hidden = loaded_config.include_hidden.unwrap_or(false);
    }
    // Apply max_file_size from config
    if let Some(size) = loaded_config.max_file_size {
        max_file_size = size;
    }
    // Security config options from config file
    let fail_on_secrets = loaded_config.fail_on_secrets.unwrap_or(false);
    let security_allowlist = loaded_config.security_allowlist.clone();
    let security_custom_patterns = loaded_config.security_custom_patterns.clone();
    // Apply redact_secrets from config if CLI didn't set it
    let redact_secrets = if redact_secrets {
        true // CLI override
    } else {
        loaded_config.redact_secrets.unwrap_or(false)
    };
    // Line numbers: config can disable them (default is true)
    if let Some(ln) = loaded_config.line_numbers {
        if show_line_numbers {
            // CLI didn't explicitly disable, so use config
            show_line_numbers = ln;
        }
    }
    // Directory structure: config can disable it
    if let Some(ds) = loaded_config.show_directory_structure {
        if show_directory_structure {
            show_directory_structure = ds;
        }
    }
    // File summary: config can disable it
    if let Some(fs) = loaded_config.show_file_summary {
        if show_file_summary {
            show_file_summary = fs;
        }
    }
    if !remove_empty_lines {
        remove_empty_lines = loaded_config.remove_empty_lines.unwrap_or(false);
    }
    if !remove_comments {
        remove_comments = loaded_config.remove_comments.unwrap_or(false);
    }
    // Token budget: apply config if CLI is using default (0 = no limit)
    // Config file can override the default; explicit CLI value takes precedence
    if max_tokens == 0 {
        if let Some(budget) = loaded_config.token_budget {
            max_tokens = budget;
        }
    }

    // Apply format/model/compression: CLI takes precedence over config over defaults
    // CLI args are Option<T>: Some = explicit, None = not specified (use config or default)
    let format: OutputFormat = if let Some(f) = cli_format {
        f.into()
    } else if let Some(ref fmt_str) = loaded_config.format {
        match fmt_str.to_lowercase().as_str() {
            "markdown" | "md" => OutputFormat::Markdown,
            "json" => OutputFormat::Json,
            "yaml" | "yml" => OutputFormat::Yaml,
            "plain" | "text" | "txt" => OutputFormat::Plain,
            "toon" => OutputFormat::Toon,
            _ => OutputFormat::Xml,
        }
    } else {
        OutputFormat::Xml
    };

    let model: TokenizerModel = if let Some(m) = cli_model {
        m.into()
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let compression: CompressionLevel = if let Some(c) = cli_compression {
        c.into()
    } else if let Some(ref comp_str) = loaded_config.compression {
        CompressionLevel::from_str(comp_str).unwrap_or(CompressionLevel::Balanced)
    } else {
        CompressionLevel::Balanced
    };

    // Scan repository
    // Fast mode (default): skip symbols for speed
    // Full mode: enable symbols for better ranking and repo map
    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: true,
        max_file_size,
        skip_symbols: !enable_symbols, // Skip by default unless --symbols or --full
    };

    // Load cache if incremental caching is enabled
    let cache_path = infiniloom_engine::RepoCache::default_cache_path(&repo_path);
    let mut repo_cache = if incremental_cache {
        if let Some(pb) = &pb {
            pb.set_message("Loading cache...");
        }
        infiniloom_engine::RepoCache::load(&cache_path).ok()
    } else {
        None
    };

    // Scan repository, using cache to skip unchanged files when available
    let mut repo = if let Some(ref cache) = repo_cache {
        if let Some(pb) = &pb {
            pb.set_message("Scanning with incremental cache...");
        }
        scanner::scan_repository_with_cache(&repo_path, config, cache)
            .context("Failed to scan repository")?
    } else {
        scanner::scan_repository(&repo_path, config).context("Failed to scan repository")?
    };

    // Update cache with newly scanned files
    if incremental_cache {
        let cache = repo_cache.get_or_insert_with(|| {
            infiniloom_engine::RepoCache::new(repo_path.to_string_lossy().as_ref())
        });

        update_repo_cache(cache, &repo, enable_symbols);

        // Save updated cache
        if let Err(e) = cache.save(&cache_path) {
            if verbose {
                eprintln!("{} Failed to save cache: {}", "⚠".yellow(), e);
            }
        } else if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Cache saved ({} files)", cache.files.len()));
            }
        }
    }

    // Apply default ignores (test files, docs, node_modules, etc.)
    if use_default_ignores {
        use infiniloom_engine::default_ignores::{
            matches_any, DEFAULT_IGNORES, DOC_IGNORES, TEST_IGNORES,
        };

        let before_count = repo.files.len();
        repo.files.retain(|f| {
            // Always apply default ignores
            if matches_any(&f.relative_path, DEFAULT_IGNORES) {
                return false;
            }
            // Optionally filter tests
            if !include_tests && matches_any(&f.relative_path, TEST_IGNORES) {
                return false;
            }
            // Optionally filter docs
            if !include_docs && matches_any(&f.relative_path, DOC_IGNORES) {
                return false;
            }
            true
        });

        if verbose && repo.files.len() < before_count {
            if let Some(pb) = &pb {
                pb.set_message(format!(
                    "Filtered {} -> {} files (default ignores)",
                    before_count,
                    repo.files.len()
                ));
            }
        }
    }

    // Filter to stdin paths if provided
    if let Some(ref paths) = stdin_paths {
        repo.files.retain(|f| {
            paths
                .iter()
                .any(|p| f.relative_path == *p || f.relative_path.ends_with(p))
        });
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Filtered to {} files from stdin", repo.files.len()));
            }
        }
    }

    // Helper function to check if a pattern matches a file path
    // Matches against both the full relative path AND just the filename
    // This allows patterns like "*.rs" to match "src/main.rs"
    fn pattern_matches_file(pattern: &glob::Pattern, relative_path: &str) -> bool {
        // Try matching the full relative path
        if pattern.matches(relative_path) {
            return true;
        }
        // Also try matching just the filename
        if let Some(filename) = std::path::Path::new(relative_path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                if pattern.matches(filename_str) {
                    return true;
                }
            }
        }
        false
    }

    // Apply include patterns (combine CLI args with config file patterns)
    // Pre-compile patterns for reuse in watch mode
    let all_include_patterns: Vec<String> = include_patterns
        .into_iter()
        .chain(loaded_config.include_patterns)
        .collect();
    let compiled_include_patterns: Vec<glob::Pattern> = all_include_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    if !compiled_include_patterns.is_empty() {
        repo.files.retain(|f| {
            compiled_include_patterns
                .iter()
                .any(|p| pattern_matches_file(p, &f.relative_path))
        });
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Included {} files matching patterns", repo.files.len()));
            }
        }
    }

    // Apply exclude patterns (combine CLI args with config file patterns)
    // Pre-compile patterns for reuse in watch mode
    let all_exclude_patterns: Vec<String> = exclude_patterns
        .into_iter()
        .chain(loaded_config.exclude_patterns)
        .collect();
    let compiled_exclude_patterns: Vec<glob::Pattern> = all_exclude_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    if !compiled_exclude_patterns.is_empty() {
        repo.files.retain(|f| {
            !compiled_exclude_patterns
                .iter()
                .any(|p| pattern_matches_file(p, &f.relative_path))
        });
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("After exclusions: {} files", repo.files.len()));
            }
        }
    }

    // Strip file contents if --no-content was passed (metadata only output)
    // Must happen BEFORE recalculate_metadata so token counts reflect actual output
    if exclude_content {
        for file in &mut repo.files {
            file.content = None;
            // Zero out token counts since content is not included
            file.token_count = infiniloom_engine::types::TokenCounts::default();
        }
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message("Excluding file contents (metadata only mode)");
            }
        }
    }

    // Recalculate metadata after filtering (and after content stripping)
    recalculate_metadata(&mut repo);

    if let Some(pb) = &pb {
        pb.set_message(format!("Found {} files", repo.files.len()));
    }

    // Sort by git change frequency if requested
    if sort_by_changes {
        if let Ok(git_repo) = GitRepo::open(&repo_path) {
            // Calculate change frequency for each file (commits in last 90 days)
            let mut file_changes: Vec<(String, u32)> = repo
                .files
                .iter()
                .map(|f| {
                    let freq = git_repo
                        .file_change_frequency(&f.relative_path, 90)
                        .unwrap_or(0);
                    (f.relative_path.clone(), freq)
                })
                .collect();

            // Sort by frequency descending
            file_changes.sort_by(|a, b| b.1.cmp(&a.1));

            // Reorder files based on change frequency
            let order_map: std::collections::HashMap<String, usize> = file_changes
                .iter()
                .enumerate()
                .map(|(i, (path, _))| (path.clone(), i))
                .collect();

            repo.files.sort_by_key(|f| {
                order_map
                    .get(&f.relative_path)
                    .copied()
                    .unwrap_or(usize::MAX)
            });

            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message("Sorted files by git change frequency");
                }
            }
        }
    } else if full_mode {
        // Full mode: use PageRank-based ranking (slower, better quality)
        infiniloom_engine::rank_files(&mut repo);
        infiniloom_engine::sort_files_by_importance(&mut repo);
    } else {
        // Fast mode (default): use heuristic-based ranking
        rank_files_fast(&mut repo);
    }

    // Limit to top N files if specified (AFTER ranking, so we keep the most important files)
    if top_files > 0 && repo.files.len() > top_files {
        repo.files.truncate(top_files);
        // Recalculate metadata after truncation
        recalculate_metadata(&mut repo);
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message(format!("Limited to top {} files by importance", top_files));
            }
        }
    }

    // Apply content transformations based on compression level and flags
    let should_remove_comments = remove_comments
        || matches!(
            compression,
            CompressionLevel::Balanced | CompressionLevel::Aggressive | CompressionLevel::Extreme
        );
    let should_remove_empty = remove_empty_lines
        || matches!(
            compression,
            CompressionLevel::Minimal
                | CompressionLevel::Balanced
                | CompressionLevel::Aggressive
                | CompressionLevel::Extreme
        );

    // Create semantic compressor if needed (lazy initialization)
    let semantic_compressor = if compression == CompressionLevel::Semantic {
        Some(infiniloom_engine::HeuristicCompressor::new())
    } else {
        None
    };

    for file in &mut repo.files {
        if let Some(ref mut content) = file.content {
            // Apply compression level transformations
            match compression {
                CompressionLevel::Aggressive => {
                    // Signatures only - extract just function/class definitions
                    if let Some(lang) = &file.language {
                        *content = extract_signatures_only(content, lang, &file.symbols);
                    }
                },
                CompressionLevel::Extreme => {
                    // Key symbols only - extract only the most important definitions
                    if let Some(lang) = &file.language {
                        *content = extract_key_symbols_only(content, lang, &file.symbols);
                    }
                },
                CompressionLevel::Focused => {
                    // Focused symbols with small surrounding context
                    if let Some(lang) = &file.language {
                        *content = extract_key_symbols_focused(content, lang, &file.symbols);
                    }
                },
                CompressionLevel::Semantic => {
                    // Use semantic compression (chunk-based with intelligent sampling)
                    if let Some(ref compressor) = semantic_compressor {
                        if let Ok(compressed) = compressor.compress(content) {
                            *content = compressed;
                        }
                    }
                },
                _ => {
                    // For None, Minimal, Balanced: apply standard transformations
                    // Remove empty lines if requested
                    if should_remove_empty {
                        *content = remove_empty_lines_from_content(content, show_line_numbers);
                    }
                    // Remove comments if requested
                    if should_remove_comments {
                        if let Some(lang) = &file.language {
                            *content =
                                remove_comments_from_content(content, lang, show_line_numbers);
                        }
                    }
                },
            }
            // Truncate base64 content if requested (applies to all levels)
            if truncate_base64 {
                *content = truncate_base64_content(content);
            }
        }
    }

    // Run security scan if requested - also redact secrets from file content
    // Uses parallel processing for better performance on large repositories
    // Triggers when either --security-check or --redact-secrets is specified
    let security_issues = if security_check || redact_secrets {
        if let Some(pb) = &pb {
            pb.set_message("Scanning for security issues and redacting secrets...");
        }
        use rayon::prelude::*;

        let mut scanner = SecurityScanner::new();
        // Apply allowlist from config
        for pattern in &security_allowlist {
            scanner.allowlist(pattern);
        }
        // Apply custom patterns from config
        scanner.add_custom_patterns(&security_custom_patterns);

        // Process files in parallel, collecting issues from each
        let all_issues: Vec<_> = repo
            .files
            .par_iter_mut()
            .filter_map(|file| {
                if let Some(content) = &file.content {
                    let (redacted_content, file_issues) =
                        scanner.scan_and_redact(content, &file.relative_path);
                    // Replace content with redacted version
                    file.content = Some(redacted_content);
                    if file_issues.is_empty() {
                        None
                    } else {
                        Some(file_issues)
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        Some(all_issues)
    } else {
        None
    };

    // Check fail_on_secrets: exit with error if secrets found and config requires it
    if fail_on_secrets {
        if let Some(ref issues) = security_issues {
            if !issues.is_empty() {
                if let Some(pb) = &pb {
                    pb.finish_and_clear();
                }
                eprintln!(
                    "{}",
                    format!("❌ Security check failed: found {} potential secrets", issues.len())
                        .red()
                );
                for issue in issues.iter().take(10) {
                    eprintln!(
                        "  • {} (line {}): {} [{}]",
                        issue.file.yellow(),
                        issue.line,
                        issue.kind.name(),
                        format!("{:?}", issue.severity).red()
                    );
                }
                if issues.len() > 10 {
                    eprintln!("  ... and {} more", issues.len() - 10);
                }
                eprintln!("\nTo allow these patterns, add them to security.allowlist in your config file.");
                anyhow::bail!("Secrets detected with fail_on_secrets enabled");
            }
        }
    }

    if verbose && security_check {
        if let Some(ref issues) = security_issues {
            if issues.is_empty() {
                eprintln!("{} No security issues found", "✓".green());
            } else {
                eprintln!("{} Found {} security issues", "⚠".yellow(), issues.len());
            }
        }
    }

    // Recompute token counts after all content transformations (compression, redaction, etc.)
    // This ensures token counts reflect the actual transformed content, not the original
    {
        let tokenizer = Tokenizer::new();
        for file in &mut repo.files {
            if let Some(ref content) = file.content {
                let counts = tokenizer.count_all(content);
                // Convert from tokenizer::TokenCounts to types::TokenCounts
                file.token_count = infiniloom_engine::types::TokenCounts {
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
                };
            }
        }
        // Update metadata totals
        recalculate_metadata(&mut repo);
        if verbose {
            if let Some(pb) = &pb {
                pb.set_message("Recomputed token counts after transformations");
            }
        }
    }

    // Populate git history in Repository struct (for structured output in formatters)
    if include_logs || include_diffs {
        if let Ok(git_repo) = GitRepo::open(&repo_path) {
            use infiniloom_engine::types::{GitChangedFile, GitCommitInfo, GitHistory};

            let mut git_history = GitHistory::default();

            // Get recent commits if requested
            if include_logs {
                if let Ok(commits) = git_repo.log(logs_count) {
                    git_history.commits = commits
                        .iter()
                        .map(|c| GitCommitInfo {
                            hash: c.hash.clone(),
                            short_hash: c.short_hash.clone(),
                            author: c.author.clone(),
                            date: c.date.clone(),
                            message: c.message.clone(),
                        })
                        .collect();
                }
            }

            // Get uncommitted changes if requested
            if include_diffs {
                if let Ok(changed_files) = git_repo.status() {
                    git_history.changed_files = changed_files
                        .iter()
                        .map(|f| {
                            let status = match f.status {
                                infiniloom_engine::git::FileStatus::Added => "A",
                                infiniloom_engine::git::FileStatus::Modified => "M",
                                infiniloom_engine::git::FileStatus::Deleted => "D",
                                infiniloom_engine::git::FileStatus::Renamed => "R",
                                infiniloom_engine::git::FileStatus::Copied => "C",
                                infiniloom_engine::git::FileStatus::Unknown => "?",
                            };
                            // Get actual diff content for this file
                            let diff_content = git_repo
                                .uncommitted_diff(&f.path)
                                .ok()
                                .filter(|d| !d.is_empty());
                            GitChangedFile {
                                path: f.path.clone(),
                                status: status.to_owned(),
                                diff_content,
                            }
                        })
                        .collect();
                }
            }

            // Set git history on repo metadata
            repo.metadata.git_history = Some(git_history);

            if verbose {
                if let Some(pb) = &pb {
                    pb.set_message(format!(
                        "Loaded {} commits, {} changes",
                        repo.metadata
                            .git_history
                            .as_ref()
                            .map(|h| h.commits.len())
                            .unwrap_or(0),
                        repo.metadata
                            .git_history
                            .as_ref()
                            .map(|h| h.changed_files.len())
                            .unwrap_or(0)
                    ));
                }
            }
        } else if verbose {
            eprintln!("{} Not a git repository, skipping git history", "⚠".yellow());
        }
    }

    // Enforce token budget using smart truncation BEFORE generating output
    // This truncates file contents at semantic boundaries based on importance
    if let Some(result) = enforce_budget(&mut repo, max_tokens, model) {
        if verbose && (result.truncated_files > 0 || result.excluded_files > 0) {
            if let Some(pb) = &pb {
                pb.set_message(format!(
                    "Budget enforced: {} files truncated, {} excluded ({:.0}% of budget used)",
                    result.truncated_files, result.excluded_files, result.budget_used_pct
                ));
            }
        }
    }

    // Clear directory structure if --no-directory-structure was passed
    if !show_directory_structure {
        repo.metadata.directory_structure = None;
    }

    // Generate repo map (using selected model for token counting)
    let map = RepoMapGenerator::builder()
        .token_budget(map_budget)
        .model(model)
        .build()
        .generate(&repo);

    if let Some(pb) = &pb {
        pb.set_message("Generating output...");
    }

    // Read instruction file (if any) before formatting extras
    let instructions_text = read_instruction_file(&instruction_file)?;

    // Format output with options and apply format-aware extras
    let formatter = OutputFormatter::by_format_with_all_options_and_model(
        format,
        show_line_numbers,
        show_file_summary,
        model,
    );
    let output_text = formatter.format(&repo, &map);
    let mut output_text = apply_pack_extras(
        output_text,
        format,
        &repo,
        model,
        header_text.as_deref(),
        instructions_text.as_deref(),
        token_tree,
        if security_check {
            security_issues.as_deref()
        } else {
            None
        },
        include_logs || include_diffs,
    )?;

    // Enforce max tokens limit
    if max_tokens > 0 {
        let current_tokens = estimate_tokens(&output_text, model);
        if current_tokens > max_tokens as usize {
            if verbose {
                eprintln!(
                    "{} Output exceeds token limit ({} > {}), truncating...",
                    "⚠".yellow(),
                    current_tokens,
                    max_tokens
                );
            }
            output_text = truncate_to_tokens(&output_text, max_tokens as usize, model);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    // Copy to clipboard if requested
    if copy_to_clipboard {
        #[cfg(feature = "clipboard")]
        {
            use clipboard::{ClipboardContext, ClipboardProvider};
            if let Ok(mut ctx) = ClipboardContext::new() {
                let _ = ctx.set_contents(output_text.clone());
                if verbose {
                    eprintln!("{} Copied to clipboard", "✓".green());
                }
            }
        }
        #[cfg(not(feature = "clipboard"))]
        {
            eprintln!(
                "{} Clipboard support not enabled. Build with --features clipboard",
                "⚠".yellow()
            );
        }
    }

    // Write output
    if let Some(ref output_path) = output {
        std::fs::write(output_path, &output_text).context("Failed to write output file")?;

        if verbose {
            let elapsed = start.elapsed();
            let total_lines: usize = repo
                .files
                .iter()
                .filter_map(|f| f.content.as_ref())
                .map(|c| c.lines().count())
                .sum();

            eprintln!();
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("{} Output written to: {}", "✓".green(), output_path.display());
            eprintln!("{}", "━".repeat(50).dimmed());
            eprintln!("  {} {} files", "📁".dimmed(), repo.files.len());
            eprintln!("  {} {} lines", "📄".dimmed(), total_lines);
            eprintln!("  {} {}", "📦".dimmed(), format_size(output_text.len() as u64, BINARY));
            eprintln!(
                "  {} ~{} tokens ({})",
                "🔢".dimmed(),
                repo.total_tokens(model),
                model.name()
            );
            eprintln!("  {} {:?}", "⏱️ ".dimmed(), elapsed);

            // Show language breakdown if available
            if !repo.metadata.languages.is_empty() {
                eprintln!();
                eprintln!("  {}:", "Languages".cyan());
                for lang in repo.metadata.languages.iter().take(5) {
                    eprintln!(
                        "    {} {}: {} files ({:.1}%)",
                        "•".dimmed(),
                        lang.language,
                        lang.files,
                        lang.percentage
                    );
                }
            }
            eprintln!();
        }
    } else {
        print!("{}", output_text);
    }

    // Handle watch mode
    if watch_mode {
        if output.is_none() {
            eprintln!("{} Watch mode requires --output to be specified", "Error:".red().bold());
            std::process::exit(1);
        }

        let output_path = output.as_ref().unwrap().clone();
        eprintln!();
        eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

        use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc::channel;
        use std::time::Duration;

        let (tx, rx) = channel();

        let mut watcher = PollWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if res.is_ok() {
                    let _ = tx.send(());
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .context("Failed to create file watcher")?;

        watcher
            .watch(&repo_path, RecursiveMode::Recursive)
            .context("Failed to watch directory")?;

        // Debounce: wait for changes to settle
        let debounce_duration = Duration::from_millis(500);
        let mut last_rebuild = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => {
                    // Debounce - wait for changes to settle
                    if last_rebuild.elapsed() < debounce_duration {
                        continue;
                    }

                    eprintln!("{} Change detected, regenerating...", "🔄".yellow());

                    // Re-run the pack logic
                    let rebuild_start = Instant::now();

                    // Re-scan repository
                    let scan_config = scanner::ScanConfig {
                        include_hidden,
                        respect_gitignore,
                        read_contents: true,
                        max_file_size,
                        skip_symbols: !enable_symbols,
                    };

                    let scan_result = if incremental_cache {
                        let cache = repo_cache.get_or_insert_with(|| {
                            infiniloom_engine::RepoCache::new(repo_path.to_string_lossy().as_ref())
                        });
                        scanner::scan_repository_with_cache(&repo_path, scan_config, cache)
                    } else {
                        scanner::scan_repository(&repo_path, scan_config)
                    };

                    if let Ok(mut new_repo) = scan_result {
                        if incremental_cache {
                            if let Some(cache) = repo_cache.as_mut() {
                                update_repo_cache(cache, &new_repo, enable_symbols);
                                if let Err(e) = cache.save(&cache_path) {
                                    eprintln!("{} Failed to save cache: {}", "⚠".yellow(), e);
                                }
                            }
                        }
                        // Apply default ignores (same as initial pack)
                        if use_default_ignores {
                            use infiniloom_engine::default_ignores::{
                                matches_any, DEFAULT_IGNORES, DOC_IGNORES, TEST_IGNORES,
                            };
                            new_repo.files.retain(|f| {
                                if matches_any(&f.relative_path, DEFAULT_IGNORES) {
                                    return false;
                                }
                                if !include_tests && matches_any(&f.relative_path, TEST_IGNORES) {
                                    return false;
                                }
                                if !include_docs && matches_any(&f.relative_path, DOC_IGNORES) {
                                    return false;
                                }
                                true
                            });
                        }

                        // Apply stdin paths filter if provided
                        if let Some(ref paths) = stdin_paths {
                            new_repo.files.retain(|f| {
                                paths
                                    .iter()
                                    .any(|p| f.relative_path == *p || f.relative_path.ends_with(p))
                            });
                        }

                        // Apply include patterns
                        if !compiled_include_patterns.is_empty() {
                            new_repo.files.retain(|f| {
                                compiled_include_patterns
                                    .iter()
                                    .any(|p| pattern_matches_file(p, &f.relative_path))
                            });
                        }

                        // Apply exclude patterns
                        if !compiled_exclude_patterns.is_empty() {
                            new_repo.files.retain(|f| {
                                !compiled_exclude_patterns
                                    .iter()
                                    .any(|p| pattern_matches_file(p, &f.relative_path))
                            });
                        }

                        // Strip file contents if --no-content was passed (metadata only output)
                        // Must happen BEFORE ranking and budget enforcement so metadata reflects output
                        if exclude_content {
                            for file in &mut new_repo.files {
                                file.content = None;
                                file.token_count = infiniloom_engine::types::TokenCounts::default();
                            }
                        }

                        // Recalculate metadata after filtering (and content stripping)
                        recalculate_metadata(&mut new_repo);

                        // Re-apply ranking (same logic as initial pack)
                        if sort_by_changes {
                            if let Ok(git_repo) = GitRepo::open(&repo_path) {
                                let mut file_changes: Vec<(String, u32)> = new_repo
                                    .files
                                    .iter()
                                    .map(|f| {
                                        let freq = git_repo
                                            .file_change_frequency(&f.relative_path, 90)
                                            .unwrap_or(0);
                                        (f.relative_path.clone(), freq)
                                    })
                                    .collect();
                                file_changes.sort_by(|a, b| b.1.cmp(&a.1));
                                let order_map: std::collections::HashMap<String, usize> =
                                    file_changes
                                        .iter()
                                        .enumerate()
                                        .map(|(i, (path, _))| (path.clone(), i))
                                        .collect();
                                new_repo.files.sort_by_key(|f| {
                                    order_map
                                        .get(&f.relative_path)
                                        .copied()
                                        .unwrap_or(usize::MAX)
                                });
                            }
                        } else if full_mode {
                            infiniloom_engine::rank_files(&mut new_repo);
                            infiniloom_engine::sort_files_by_importance(&mut new_repo);
                        } else {
                            rank_files_fast(&mut new_repo);
                        }

                        // Limit to top N files if specified (AFTER ranking)
                        if top_files > 0 && new_repo.files.len() > top_files {
                            new_repo.files.truncate(top_files);
                            recalculate_metadata(&mut new_repo);
                        }

                        // Apply content transformations based on compression level and flags
                        let should_remove_comments = remove_comments
                            || matches!(
                                compression,
                                CompressionLevel::Balanced
                                    | CompressionLevel::Aggressive
                                    | CompressionLevel::Extreme
                            );
                        let should_remove_empty = remove_empty_lines
                            || matches!(
                                compression,
                                CompressionLevel::Minimal
                                    | CompressionLevel::Balanced
                                    | CompressionLevel::Aggressive
                                    | CompressionLevel::Extreme
                            );

                        // Create semantic compressor if needed
                        let watch_semantic_compressor = if compression == CompressionLevel::Semantic
                        {
                            Some(infiniloom_engine::HeuristicCompressor::new())
                        } else {
                            None
                        };

                        for file in &mut new_repo.files {
                            if let Some(ref mut content) = file.content {
                                // Apply compression level transformations
                                match compression {
                                    CompressionLevel::Aggressive => {
                                        if let Some(lang) = &file.language {
                                            *content = extract_signatures_only(
                                                content,
                                                lang,
                                                &file.symbols,
                                            );
                                        }
                                    },
                                    CompressionLevel::Extreme => {
                                        if let Some(lang) = &file.language {
                                            *content = extract_key_symbols_only(
                                                content,
                                                lang,
                                                &file.symbols,
                                            );
                                        }
                                    },
                                    CompressionLevel::Focused => {
                                        if let Some(lang) = &file.language {
                                            *content = extract_key_symbols_focused(
                                                content,
                                                lang,
                                                &file.symbols,
                                            );
                                        }
                                    },
                                    CompressionLevel::Semantic => {
                                        if let Some(ref compressor) = watch_semantic_compressor {
                                            if let Ok(compressed) = compressor.compress(content) {
                                                *content = compressed;
                                            }
                                        }
                                    },
                                    _ => {
                                        if should_remove_empty {
                                            *content = remove_empty_lines_from_content(
                                                content,
                                                show_line_numbers,
                                            );
                                        }
                                        if should_remove_comments {
                                            if let Some(lang) = &file.language {
                                                *content = remove_comments_from_content(
                                                    content,
                                                    lang,
                                                    show_line_numbers,
                                                );
                                            }
                                        }
                                    },
                                }
                                // Truncate base64 content if requested
                                if truncate_base64 {
                                    *content = truncate_base64_content(content);
                                }
                            }
                        }

                        // Run security scan if requested - scan and redact secrets, collect issues
                        let watch_security_issues = if security_check || redact_secrets {
                            use rayon::prelude::*;
                            let mut scanner = SecurityScanner::new();
                            // Apply allowlist and custom patterns from config
                            for pattern in &security_allowlist {
                                scanner.allowlist(pattern);
                            }
                            scanner.add_custom_patterns(&security_custom_patterns);
                            let all_issues: Vec<_> = new_repo
                                .files
                                .par_iter_mut()
                                .filter_map(|file| {
                                    if let Some(content) = &file.content {
                                        let (redacted_content, file_issues) =
                                            scanner.scan_and_redact(content, &file.relative_path);
                                        file.content = Some(redacted_content);
                                        if file_issues.is_empty() {
                                            None
                                        } else {
                                            Some(file_issues)
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .flatten()
                                .collect();
                            Some(all_issues)
                        } else {
                            None
                        };

                        // Check fail_on_secrets in watch mode (same as initial pack)
                        if fail_on_secrets {
                            if let Some(ref issues) = watch_security_issues {
                                if !issues.is_empty() {
                                    eprintln!(
                                        "\n{} Secrets detected with fail_on_secrets enabled:",
                                        "Error:".red().bold()
                                    );
                                    for issue in issues.iter().take(10) {
                                        eprintln!(
                                            "  - [{:?}] {} in {} (line {})",
                                            issue.severity,
                                            issue.kind.name(),
                                            issue.file,
                                            issue.line
                                        );
                                    }
                                    if issues.len() > 10 {
                                        eprintln!("  ... and {} more", issues.len() - 10);
                                    }
                                    eprintln!("\nTo allow these patterns, add them to security.allowlist in your config file.");
                                    eprintln!("Watch mode stopping due to fail_on_secrets policy.");
                                    break; // Exit watch loop
                                }
                            }
                        }

                        // Recompute token counts after all content transformations
                        // This ensures token counts reflect transformed content, not original
                        {
                            let tokenizer = Tokenizer::new();
                            for file in &mut new_repo.files {
                                if let Some(ref content) = file.content {
                                    let counts = tokenizer.count_all(content);
                                    file.token_count = infiniloom_engine::types::TokenCounts {
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
                                    };
                                }
                            }
                            recalculate_metadata(&mut new_repo);
                        }

                        // Re-populate git history (same as initial pack)
                        if include_logs || include_diffs {
                            if let Ok(git_repo) = GitRepo::open(&repo_path) {
                                use infiniloom_engine::types::{
                                    GitChangedFile, GitCommitInfo, GitHistory,
                                };

                                let mut git_history = GitHistory::default();

                                if include_logs {
                                    if let Ok(commits) = git_repo.log(logs_count) {
                                        git_history.commits = commits
                                            .iter()
                                            .map(|c| GitCommitInfo {
                                                hash: c.hash.clone(),
                                                short_hash: c.short_hash.clone(),
                                                author: c.author.clone(),
                                                date: c.date.clone(),
                                                message: c.message.clone(),
                                            })
                                            .collect();
                                    }
                                }

                                if include_diffs {
                                    if let Ok(changed_files) = git_repo.status() {
                                        git_history.changed_files = changed_files
                                            .iter()
                                            .map(|f| {
                                                let status = match f.status {
                                                    infiniloom_engine::git::FileStatus::Added => "A",
                                                    infiniloom_engine::git::FileStatus::Modified => "M",
                                                    infiniloom_engine::git::FileStatus::Deleted => "D",
                                                    infiniloom_engine::git::FileStatus::Renamed => "R",
                                                    infiniloom_engine::git::FileStatus::Copied => "C",
                                                    infiniloom_engine::git::FileStatus::Unknown => "?",
                                                };
                                                // Get actual diff content for this file
                                                let diff_content = git_repo
                                                    .uncommitted_diff(&f.path)
                                                    .ok()
                                                    .filter(|d| !d.is_empty());
                                                GitChangedFile {
                                                    path: f.path.clone(),
                                                    status: status.to_owned(),
                                                    diff_content,
                                                }
                                            })
                                            .collect();
                                    }
                                }

                                new_repo.metadata.git_history = Some(git_history);
                            }
                        }

                        // Enforce token budget before generating output
                        let _budget_result = enforce_budget(&mut new_repo, max_tokens, model);

                        // Clear directory structure if --no-directory-structure was passed
                        if !show_directory_structure {
                            new_repo.metadata.directory_structure = None;
                        }

                        let new_map = RepoMapGenerator::builder()
                            .token_budget(map_budget)
                            .model(model)
                            .build()
                            .generate(&new_repo);
                        let new_formatter = OutputFormatter::by_format_with_all_options_and_model(
                            format,
                            show_line_numbers,
                            show_file_summary,
                            model,
                        );
                        let new_output = new_formatter.format(&new_repo, &new_map);
                        let instructions_text = match read_instruction_file(&instruction_file) {
                            Ok(text) => text,
                            Err(err) => {
                                eprintln!("{} {}", "Error:".red(), err);
                                None
                            },
                        };
                        let mut new_output = match apply_pack_extras(
                            new_output,
                            format,
                            &new_repo,
                            model,
                            header_text.as_deref(),
                            instructions_text.as_deref(),
                            token_tree,
                            if security_check {
                                watch_security_issues.as_deref()
                            } else {
                                None
                            },
                            include_logs || include_diffs,
                        ) {
                            Ok(output) => output,
                            Err(err) => {
                                eprintln!("{} {}", "Error:".red(), err);
                                continue;
                            },
                        };

                        // Enforce max tokens limit (same as initial pack)
                        if max_tokens > 0 {
                            let current_tokens = estimate_tokens(&new_output, model);
                            if current_tokens > max_tokens as usize {
                                new_output =
                                    truncate_to_tokens(&new_output, max_tokens as usize, model);
                            }
                        }

                        if let Err(e) = std::fs::write(&output_path, &new_output) {
                            eprintln!("{} Failed to write output: {}", "Error:".red(), e);
                        } else {
                            eprintln!(
                                "{} Regenerated in {:?} ({} files, ~{} tokens)",
                                "✓".green(),
                                rebuild_start.elapsed(),
                                new_repo.files.len(),
                                new_repo.total_tokens(model)
                            );
                        }
                    }

                    last_rebuild = Instant::now();
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Just keep watching
                },
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                },
            }
        }
    }

    Ok(())
}

fn cmd_scan(
    path: PathBuf,
    model: TokenizerModel,
    include_hidden: bool,
    verbose: bool,
    json_output: bool,
    security_check: bool,
    sample: Option<usize>,
    sample_percent: Option<f64>,
) -> Result<()> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    let start = Instant::now();

    // Always read content to get accurate token counts (not just heuristic estimates)
    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore: true,
        read_contents: true, // Read content for accurate token counting
        max_file_size: 50 * 1024 * 1024u64,
        skip_symbols: true, // No need for symbols in scan mode
    };

    let mut repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    // Apply sampling if requested
    let (is_sampled, sample_size, original_count) = if let Some(n) = sample {
        let original_count = repo.files.len();
        if n < original_count && n > 0 {
            let mut rng = thread_rng();
            repo.files.shuffle(&mut rng);
            repo.files.truncate(n);
            repo.metadata.total_files = n as u32;
            (true, n, original_count)
        } else {
            (false, repo.files.len(), repo.files.len())
        }
    } else if let Some(p) = sample_percent {
        let original_count = repo.files.len();
        let n = ((original_count as f64) * (p / 100.0)).ceil() as usize;
        if n < original_count && n > 0 {
            let mut rng = thread_rng();
            repo.files.shuffle(&mut rng);
            repo.files.truncate(n);
            repo.metadata.total_files = n as u32;
            (true, n, original_count)
        } else {
            (false, repo.files.len(), repo.files.len())
        }
    } else {
        (false, repo.files.len(), repo.files.len())
    };

    // Extrapolation factor for estimating totals from sample
    let extrapolation_factor = if is_sampled && sample_size > 0 {
        original_count as f64 / sample_size as f64
    } else {
        1.0
    };

    // Compute accurate token counts using the real tokenizer (not heuristics)
    {
        let tokenizer = Tokenizer::new();
        for file in &mut repo.files {
            if let Some(ref content) = file.content {
                let counts = tokenizer.count_all(content);
                file.token_count = infiniloom_engine::types::TokenCounts {
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
                };
            }
        }
        // Update metadata totals
        repo.metadata.total_tokens = infiniloom_engine::types::TokenCounts {
            o200k: repo.files.iter().map(|f| f.token_count.o200k).sum(),
            cl100k: repo.files.iter().map(|f| f.token_count.cl100k).sum(),
            claude: repo.files.iter().map(|f| f.token_count.claude).sum(),
            gemini: repo.files.iter().map(|f| f.token_count.gemini).sum(),
            llama: repo.files.iter().map(|f| f.token_count.llama).sum(),
            mistral: repo.files.iter().map(|f| f.token_count.mistral).sum(),
            deepseek: repo.files.iter().map(|f| f.token_count.deepseek).sum(),
            qwen: repo.files.iter().map(|f| f.token_count.qwen).sum(),
            cohere: repo.files.iter().map(|f| f.token_count.cohere).sum(),
            grok: repo.files.iter().map(|f| f.token_count.grok).sum(),
        };
    }

    // Run security scan if requested (parallelized for performance)
    let security_issues = if security_check {
        use rayon::prelude::*;
        let scanner = SecurityScanner::new();
        let all_issues: Vec<_> = repo
            .files
            .par_iter()
            .filter_map(|file| {
                if let Some(ref content) = file.content {
                    let (_, file_issues) = scanner.scan_and_redact(content, &file.relative_path);
                    if file_issues.is_empty() {
                        None
                    } else {
                        Some(file_issues)
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        Some(all_issues)
    } else {
        None
    };

    let elapsed = start.elapsed();

    if json_output {
        // Calculate estimated totals (extrapolate if sampled)
        let sampled_bytes: u64 = repo.files.iter().map(|f| f.size_bytes).sum();
        let estimated_bytes = (sampled_bytes as f64 * extrapolation_factor) as u64;
        let estimated_claude =
            (repo.total_tokens(TokenizerModel::Claude) as f64 * extrapolation_factor) as u32;
        let estimated_gpt4o =
            (repo.total_tokens(TokenizerModel::Gpt4o) as f64 * extrapolation_factor) as u32;
        let estimated_gemini =
            (repo.total_tokens(TokenizerModel::Gemini) as f64 * extrapolation_factor) as u32;

        // JSON output
        let mut stats = serde_json::json!({
            "repository": repo.name,
            "files": if is_sampled { original_count } else { repo.files.len() },
            "total_bytes": estimated_bytes,
            "total_tokens": {
                "claude": estimated_claude,
                "gpt4o": estimated_gpt4o,
                "gemini": estimated_gemini,
            },
            "languages": repo.metadata.languages,
            "scan_time_ms": elapsed.as_millis(),
        });

        // Add sampling metadata if sampled
        if is_sampled {
            stats["sampling"] = serde_json::json!({
                "is_estimated": true,
                "sample_size": sample_size,
                "total_files": original_count,
                "sample_percent": (sample_size as f64 / original_count as f64 * 100.0),
                "extrapolation_factor": extrapolation_factor,
            });
        }

        // Add security issues if scanned
        if let Some(ref issues) = security_issues {
            let estimated_issues = (issues.len() as f64 * extrapolation_factor) as usize;
            stats["security"] = serde_json::json!({
                "issues_found": if is_sampled { estimated_issues } else { issues.len() },
                "is_estimated": is_sampled,
                "issues": issues.iter().map(|i| serde_json::json!({
                    "file": &i.file,
                    "line": i.line,
                    "kind": i.kind.name(),
                    "severity": format!("{:?}", i.severity),
                })).collect::<Vec<_>>(),
            });
        }
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        // Human-readable output
        println!();
        println!("{}", "━".repeat(50).dimmed());
        if is_sampled {
            println!("  {} {}", "Scan Results".cyan().bold(), "[ESTIMATED]".yellow().bold());
        } else {
            println!("  {}", "Scan Results".cyan().bold());
        }
        println!("{}", "━".repeat(50).dimmed());
        println!();

        println!("  Repository:   {}", repo.name.yellow());
        println!("  Path:         {}", path.display());

        // Show file count with sampling info
        if is_sampled {
            println!(
                "  Files:        {} (sampled {} of {})",
                original_count, sample_size, original_count
            );
        } else {
            println!("  Files:        {}", repo.files.len());
        }

        let sampled_bytes: u64 = repo.files.iter().map(|f| f.size_bytes).sum();
        let estimated_bytes = (sampled_bytes as f64 * extrapolation_factor) as u64;
        if is_sampled {
            println!("  Total Size:   ~{} (estimated)", format_size(estimated_bytes, BINARY));
        } else {
            println!("  Total Size:   {}", format_size(sampled_bytes, BINARY));
        }
        println!("  Scan Time:    {:?}", elapsed);
        println!();

        // Show sampling details
        if is_sampled {
            println!("  {}:", "Sampling".yellow());
            println!("    Sample size:        {} files", sample_size);
            println!(
                "    Sample percentage:  {:.1}%",
                sample_size as f64 / original_count as f64 * 100.0
            );
            println!("    Extrapolation:      {:.2}x", extrapolation_factor);
            println!();
        }

        // Language breakdown
        if !repo.metadata.languages.is_empty() {
            println!("  {}:", "Languages".cyan());
            for lang in &repo.metadata.languages {
                println!("    {}: {} files ({:.1}%)", lang.language, lang.files, lang.percentage);
            }
            println!();
        }

        // Token counts (accurate, using tiktoken for OpenAI models)
        let estimated_tokens = (repo.total_tokens(model) as f64 * extrapolation_factor) as u32;
        if is_sampled {
            println!("  {} ({}) {}:", "Token Counts".cyan(), model.name(), "[ESTIMATED]".yellow());
            println!("    Total: ~{}", estimated_tokens);
        } else {
            println!("  {} ({}):", "Token Counts".cyan(), model.name());
            println!("    Total: {}", repo.total_tokens(model));
        }
        println!();

        // Verbose file list
        if verbose {
            println!("  {}:", "Files".cyan());
            let mut files: Vec<_> = repo.files.iter().collect();
            files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

            for file in files.iter().take(20) {
                let lang = file.language.as_deref().unwrap_or("?");
                println!(
                    "    {} ({}) - {}",
                    file.relative_path,
                    lang,
                    format_size(file.size_bytes, BINARY)
                );
            }

            if files.len() > 20 {
                println!("    ... and {} more files", files.len() - 20);
            }
            println!();
        }

        // Security scan results
        if let Some(ref issues) = security_issues {
            if is_sampled {
                println!("  {} {}:", "Security Scan".cyan(), "[ESTIMATED]".yellow());
            } else {
                println!("  {}:", "Security Scan".cyan());
            }
            if issues.is_empty() {
                println!("    {} No secrets detected in sample", "✓".green());
            } else {
                let estimated_issues = (issues.len() as f64 * extrapolation_factor) as usize;
                if is_sampled {
                    println!(
                        "    {} Found ~{} potential secrets (estimated from {} in sample):",
                        "⚠".yellow(),
                        estimated_issues,
                        issues.len()
                    );
                } else {
                    println!("    {} Found {} potential secrets:", "⚠".yellow(), issues.len());
                }
                for issue in issues.iter().take(10) {
                    println!(
                        "      • [{:?}] {} in {} (line {})",
                        issue.severity,
                        issue.kind.name(),
                        issue.file,
                        issue.line
                    );
                }
                if issues.len() > 10 {
                    println!("      ... and {} more in sample", issues.len() - 10);
                }
            }
            println!();
        }
    }

    Ok(())
}

fn cmd_map(
    path: PathBuf,
    budget: u32,
    cli_model: Option<Model>,
    output: Option<PathBuf>,
) -> Result<()> {
    let config = scanner::ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024u64,
        skip_symbols: false, // Map command needs symbols for ranking
    };

    let mut repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    // Rank files by importance
    infiniloom_engine::rank_files(&mut repo);
    infiniloom_engine::sort_files_by_importance(&mut repo);

    let loaded_config = load_config_file(None, &path);
    let model: TokenizerModel = if let Some(m) = cli_model {
        m.into()
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let map = RepoMapGenerator::builder()
        .token_budget(budget)
        .model(model)
        .build()
        .generate(&repo);

    let output_text = format_repo_map(&map, budget);

    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text).context("Failed to write output file")?;
        eprintln!("Repository map written to: {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    Ok(())
}

fn cmd_info(path: Option<PathBuf>) -> Result<()> {
    println!();
    println!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
    println!("{}", "━".repeat(50).dimmed());
    println!();
    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("  Engine:       {}", infiniloom_engine::VERSION);

    // Show project-specific info if path provided
    if let Some(ref project_path) = path {
        let resolved_path = if project_path.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            project_path.clone()
        };
        println!();
        println!("  {}:", "Project".yellow());
        println!("    Path:       {}", resolved_path.display());

        // Check for config file
        let config = load_config_file(None, &resolved_path);
        if config.format.is_some() || config.model.is_some() || config.compression.is_some() {
            println!("    Config:     Found (.infiniloom.yaml/.toml/.json)");
            if let Some(fmt) = &config.format {
                println!("      Format:     {:?}", fmt);
            }
            if let Some(model) = &config.model {
                println!("      Model:      {:?}", model);
            }
            if let Some(comp) = &config.compression {
                println!("      Compression: {:?}", comp);
            }
            if let Some(budget) = config.token_budget {
                println!("      Budget:     {} tokens", budget);
            }
        } else {
            println!("    Config:     None (using defaults)");
        }
    }

    println!();
    println!("  {}:", "Supported Formats".yellow());
    println!("    xml       - Claude-optimized (with cache hints)");
    println!("    markdown  - GPT-optimized (with code blocks)");
    println!("    json      - Generic structured format");
    println!("    yaml      - Gemini-optimized (query at end)");
    println!("    toon      - Most token-efficient (~40% smaller)");
    println!("    plain     - Simple plain text (no markup)");
    println!();
    println!("  {}:", "Supported Models".yellow());
    println!("    claude      - Anthropic Claude (default)");
    println!("    gpt52       - OpenAI GPT-5.2 (o200k_base encoding)");
    println!("    gpt51       - OpenAI GPT-5.1 (o200k_base encoding)");
    println!("    gpt5        - OpenAI GPT-5 (o200k_base encoding)");
    println!("    o4-mini     - OpenAI O4-mini reasoning model");
    println!("    o3          - OpenAI O3 reasoning model");
    println!("    o1          - OpenAI O1 reasoning model");
    println!("    gpt4o       - OpenAI GPT-4o (o200k_base encoding)");
    println!("    gpt4o-mini  - OpenAI GPT-4o-mini (o200k_base encoding)");
    println!("    gpt4        - OpenAI GPT-4/GPT-4 Turbo (cl100k_base, legacy)");
    println!("    gpt35-turbo - OpenAI GPT-3.5-turbo (cl100k_base, legacy)");
    println!("    gemini      - Google Gemini");
    println!("    llama       - Meta Llama 3/4");
    println!("    codellama   - Meta CodeLlama (optimized for code)");
    println!("    mistral     - Mistral AI (Large, Medium, Codestral)");
    println!("    deepseek    - DeepSeek (V3, R1, Coder)");
    println!("    qwen        - Alibaba Qwen (Qwen3, Qwen2.5)");
    println!("    cohere      - Cohere (Command R+, Command R)");
    println!("    grok        - xAI Grok (Grok 2, Grok 3)");
    println!();
    println!("  {}:", "Compression Levels".yellow());
    println!("    none      - No compression (0%)");
    println!("    minimal   - Whitespace only (~15%)");
    println!("    balanced  - Remove comments (~35%)");
    println!("    aggressive - Signatures only (~60%)");
    println!("    extreme   - Key symbols only (~80%)");
    println!("    focused   - Key symbols with context (~75%)");
    println!("    semantic  - Heuristic chunking (~65%, NOT neural)");
    println!();

    Ok(())
}

fn cmd_init(
    path: PathBuf,
    format: ConfigFormat,
    template: ConfigTemplate,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let (ext, format_name) = match format {
        ConfigFormat::Yaml => ("yaml", "yaml"),
        ConfigFormat::Toml => ("toml", "toml"),
        ConfigFormat::Json => ("json", "json"),
    };

    // If explicit output path is given, use it; otherwise create in the given directory
    let output_path = output.unwrap_or_else(|| path.join(format!(".infiniloom.{}", ext)));

    // Check if file exists
    if output_path.exists() && !force {
        eprintln!(
            "{} Configuration file already exists: {}",
            "Error:".red().bold(),
            output_path.display()
        );
        eprintln!("Use --force to overwrite");
        std::process::exit(1);
    }

    // Generate config based on template
    let config_content = generate_template_config(format_name, template);

    // Write config file
    std::fs::write(&output_path, &config_content)
        .with_context(|| format!("Failed to write config file: {}", output_path.display()))?;

    let template_name = match template {
        ConfigTemplate::Generic => "generic",
        ConfigTemplate::Rust => "Rust",
        ConfigTemplate::Python => "Python",
        ConfigTemplate::Typescript => "TypeScript",
        ConfigTemplate::Go => "Go",
        ConfigTemplate::Java => "Java",
    };

    println!(
        "{} Created {} configuration file: {}",
        "✓".green(),
        template_name,
        output_path.display()
    );
    println!();
    println!("Edit this file to customize Infiniloom behavior.");
    println!("See https://github.com/Topos-Labs/infiniloom#configuration for options.");

    Ok(())
}

/// Generate configuration content based on template and format
fn generate_template_config(format: &str, template: ConfigTemplate) -> String {
    match template {
        ConfigTemplate::Generic => infiniloom_engine::Config::generate_default(format),
        ConfigTemplate::Rust => generate_rust_template(format),
        ConfigTemplate::Python => generate_python_template(format),
        ConfigTemplate::Typescript => generate_typescript_template(format),
        ConfigTemplate::Go => generate_go_template(format),
        ConfigTemplate::Java => generate_java_template(format),
    }
}

/// Generate Rust project configuration template
fn generate_rust_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Rust Project Template
# Documentation: https://github.com/Topos-Labs/infiniloom#configuration

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Rust source files and configuration
  include:
    - "*.rs"
    - "Cargo.toml"
    - "Cargo.lock"
    - "build.rs"
  # Exclude build artifacts and dependencies
  exclude:
    - "target/*"
    - "target/**"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Rust Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.rs", "Cargo.toml", "Cargo.lock", "build.rs"]
exclude = ["target/*", "target/**"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.rs", "Cargo.toml", "Cargo.lock", "build.rs"],
    "exclude": ["target/*", "target/**"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Python project configuration template
fn generate_python_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Python Project Template
# Documentation: https://github.com/Topos-Labs/infiniloom#configuration

output:
  format: markdown
  model: gpt4o
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Python source files and configuration
  include:
    - "*.py"
    - "*.pyi"
    - "requirements.txt"
    - "pyproject.toml"
    - "setup.py"
    - "setup.cfg"
    - "Pipfile"
  # Exclude virtual environments and cache
  exclude:
    - "venv/*"
    - ".venv/*"
    - "__pycache__/*"
    - "*.pyc"
    - ".pytest_cache/*"
    - ".mypy_cache/*"
    - "*.egg-info/*"
    - "dist/*"
    - "build/*"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#.to_owned(),
        "toml" => r#"# Infiniloom Configuration - Python Project Template

[output]
format = "markdown"
model = "gpt4o"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.py", "*.pyi", "requirements.txt", "pyproject.toml", "setup.py", "setup.cfg", "Pipfile"]
exclude = ["venv/*", ".venv/*", "__pycache__/*", "*.pyc", ".pytest_cache/*", ".mypy_cache/*", "*.egg-info/*", "dist/*", "build/*"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#.to_owned(),
        "json" => r#"{
  "output": {
    "format": "markdown",
    "model": "gpt4o",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.py", "*.pyi", "requirements.txt", "pyproject.toml", "setup.py", "setup.cfg", "Pipfile"],
    "exclude": ["venv/*", ".venv/*", "__pycache__/*", "*.pyc", ".pytest_cache/*", ".mypy_cache/*", "*.egg-info/*", "dist/*", "build/*"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#.to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate TypeScript/JavaScript project configuration template
fn generate_typescript_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - TypeScript/JavaScript Project Template
# Documentation: https://github.com/Topos-Labs/infiniloom#configuration

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # TypeScript/JavaScript source files and configuration
  include:
    - "*.ts"
    - "*.tsx"
    - "*.js"
    - "*.jsx"
    - "*.mjs"
    - "*.cjs"
    - "package.json"
    - "tsconfig.json"
    - "*.config.js"
    - "*.config.ts"
  # Exclude dependencies and build outputs
  exclude:
    - "node_modules/*"
    - "node_modules/**"
    - "dist/*"
    - "build/*"
    - ".next/*"
    - ".nuxt/*"
    - "coverage/*"
    - "*.test.ts"
    - "*.test.tsx"
    - "*.spec.ts"
    - "*.spec.tsx"
    - "*.min.js"
    - "*.bundle.js"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
    - "127.0.0.1"
"#.to_owned(),
        "toml" => r#"# Infiniloom Configuration - TypeScript/JavaScript Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.ts", "*.tsx", "*.js", "*.jsx", "*.mjs", "*.cjs", "package.json", "tsconfig.json", "*.config.js", "*.config.ts"]
exclude = ["node_modules/*", "node_modules/**", "dist/*", "build/*", ".next/*", ".nuxt/*", "coverage/*", "*.test.ts", "*.test.tsx", "*.spec.ts", "*.spec.tsx", "*.min.js", "*.bundle.js"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost", "127.0.0.1"]
"#.to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.ts", "*.tsx", "*.js", "*.jsx", "*.mjs", "*.cjs", "package.json", "tsconfig.json", "*.config.js", "*.config.ts"],
    "exclude": ["node_modules/*", "node_modules/**", "dist/*", "build/*", ".next/*", ".nuxt/*", "coverage/*", "*.test.ts", "*.test.tsx", "*.spec.ts", "*.spec.tsx", "*.min.js", "*.bundle.js"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost", "127.0.0.1"]
  }
}"#.to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Go project configuration template
fn generate_go_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Go Project Template
# Documentation: https://github.com/Topos-Labs/infiniloom#configuration

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Go source files and configuration
  include:
    - "*.go"
    - "go.mod"
    - "go.sum"
  # Exclude vendor and build outputs
  exclude:
    - "vendor/*"
    - "vendor/**"
    - "*_test.go"
    - "testdata/*"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Go Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.go", "go.mod", "go.sum"]
exclude = ["vendor/*", "vendor/**", "*_test.go", "testdata/*"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.go", "go.mod", "go.sum"],
    "exclude": ["vendor/*", "vendor/**", "*_test.go", "testdata/*"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Java project configuration template
fn generate_java_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Java Project Template
# Documentation: https://github.com/Topos-Labs/infiniloom#configuration

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Java source files and build configuration
  include:
    - "*.java"
    - "pom.xml"
    - "build.gradle"
    - "build.gradle.kts"
    - "settings.gradle"
    - "settings.gradle.kts"
    - "gradle.properties"
  # Exclude build outputs and IDE files
  exclude:
    - "target/*"
    - "target/**"
    - "build/*"
    - "build/**"
    - ".gradle/*"
    - ".idea/*"
    - "*.class"
    - "*Test.java"
    - "*Tests.java"
    - "*IT.java"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#.to_owned(),
        "toml" => r#"# Infiniloom Configuration - Java Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.java", "pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts", "gradle.properties"]
exclude = ["target/*", "target/**", "build/*", "build/**", ".gradle/*", ".idea/*", "*.class", "*Test.java", "*Tests.java", "*IT.java"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#.to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.java", "pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts", "gradle.properties"],
    "exclude": ["target/*", "target/**", "build/*", "build/**", ".gradle/*", ".idea/*", "*.class", "*Test.java", "*Tests.java", "*IT.java"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#.to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Truncate base64 encoded content in a string
/// This helps reduce token count when files contain embedded binary data
fn truncate_base64_content(content: &str) -> String {
    // Use pre-compiled static regex for base64 patterns
    BASE64_PATTERN
        .replace_all(content, |caps: &regex::Captures<'_>| {
            let matched = caps.get(0).map_or("", |m| m.as_str());
            if matched.starts_with("data:") {
                // Data URI - keep prefix, truncate data
                if let Some(comma_idx) = matched.find(',') {
                    let prefix = &matched[..comma_idx + 1];
                    format!("{}[BASE64_TRUNCATED]", prefix)
                } else {
                    "[BASE64_TRUNCATED]".to_owned()
                }
            } else if matched.len() > 100 {
                // Standalone base64 - must contain + or / to distinguish from hex/alphanumeric
                // This check replaces the lookahead that regex crate doesn't support
                if matched.contains('+') || matched.contains('/') {
                    format!("{}...[BASE64_TRUNCATED]", &matched[..50])
                } else {
                    // Not base64 (likely hex or alphanumeric ID), keep as-is
                    matched.to_owned()
                }
            } else {
                matched.to_owned()
            }
        })
        .to_string()
}

/// Remove empty lines from content while optionally preserving line numbers
/// If preserve_line_numbers is true, output format is "line_num:content" for each kept line
/// Otherwise, just outputs the non-empty lines
/// Detects if content already has embedded line numbers and preserves them
fn remove_empty_lines_from_content(content: &str, preserve_line_numbers: bool) -> String {
    // Detect if content already has embedded line numbers
    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    if has_embedded_nums {
        // Content already has line numbers - just filter empty lines
        if preserve_line_numbers {
            // Keep the line numbers
            content
                .lines()
                .filter(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        !rest.trim().is_empty()
                    } else {
                        !line.trim().is_empty()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Strip the line numbers, keep just content
            content
                .lines()
                .filter_map(|line| {
                    if let Some((_num, rest)) = line.split_once(':') {
                        if !rest.trim().is_empty() {
                            Some(rest.to_owned())
                        } else {
                            None
                        }
                    } else if !line.trim().is_empty() {
                        Some(line.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        // No embedded line numbers
        if preserve_line_numbers {
            // Add line numbers
            content
                .lines()
                .enumerate()
                .filter(|(_, line)| !line.trim().is_empty())
                .map(|(i, line)| format!("{}:{}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Just filter empty lines, don't add line numbers
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Check if text ends inside an unclosed string literal
/// Handles escaped quotes (\" and \') properly
fn is_inside_string(text: &str) -> bool {
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_backslash = false;

    for c in text.chars() {
        if prev_backslash {
            // This character is escaped, skip it
            prev_backslash = false;
            continue;
        }

        match c {
            '\\' => prev_backslash = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            _ => {},
        }
    }

    in_double || in_single
}

/// Remove comments from code based on language while optionally preserving line numbers
/// If preserve_line_numbers is true, output format is "line_num:content" for each kept line
/// Otherwise, just outputs the cleaned lines
/// Detects if content already has embedded line numbers and handles them appropriately
fn remove_comments_from_content(
    content: &str,
    language: &str,
    preserve_line_numbers: bool,
) -> String {
    let (line_comment, block_start, block_end) = match language.to_lowercase().as_str() {
        "python" | "ruby" | "shell" | "bash" | "sh" | "yaml" | "yml" => ("#", "", ""),
        "javascript" | "typescript" | "java" | "c" | "cpp" | "c++" | "rust" | "go" | "swift"
        | "kotlin" | "scala" => ("//", "/*", "*/"),
        "html" | "xml" => ("", "<!--", "-->"),
        "css" | "scss" | "sass" => ("", "/*", "*/"),
        "sql" => ("--", "/*", "*/"),
        "lua" => ("--", "--[[", "]]"),
        _ => ("//", "/*", "*/"), // Default to C-style
    };

    // Helper to format output line based on line number preference
    let format_line = |line_num: u32, content: &str| -> String {
        if preserve_line_numbers {
            format!("{}:{}\n", line_num, content)
        } else {
            format!("{}\n", content)
        }
    };

    // Detect if content already has embedded line numbers
    let first_line = content.lines().next().unwrap_or("");
    let has_embedded_nums = first_line.contains(':')
        && first_line
            .split(':')
            .next()
            .map(|s| s.parse::<u32>().is_ok())
            .unwrap_or(false);

    let mut result = String::new();
    let mut in_block_comment = false;

    for (line_num, raw_line) in content.lines().enumerate() {
        // Parse existing line number if present
        let (original_line_num, line) = if has_embedded_nums {
            if let Some((num_str, rest)) = raw_line.split_once(':') {
                if let Ok(n) = num_str.parse::<u32>() {
                    (n, rest)
                } else {
                    (line_num as u32 + 1, raw_line)
                }
            } else {
                (line_num as u32 + 1, raw_line)
            }
        } else {
            (line_num as u32 + 1, raw_line)
        };

        let trimmed = line.trim();

        // Handle block comments
        if !block_start.is_empty() && !block_end.is_empty() {
            if in_block_comment {
                if let Some(idx) = line.find(block_end) {
                    in_block_comment = false;
                    let after_block = &line[idx + block_end.len()..];
                    if !after_block.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, after_block));
                    }
                }
                continue;
            }

            if let Some(idx) = line.find(block_start) {
                // Check if block comment ends on same line
                if let Some(end_idx) = line[idx + block_start.len()..].find(block_end) {
                    let before = &line[..idx];
                    let after = &line[idx + block_start.len() + end_idx + block_end.len()..];
                    let combined = format!("{}{}", before.trim_end(), after);
                    if !combined.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, &combined));
                    }
                    continue;
                } else {
                    in_block_comment = true;
                    let before = &line[..idx];
                    if !before.trim().is_empty() {
                        result.push_str(&format_line(original_line_num, before.trim_end()));
                    }
                    continue;
                }
            }
        }

        // Handle line comments (simple approach - may not handle strings perfectly)
        if !line_comment.is_empty() && trimmed.starts_with(line_comment) {
            continue;
        }

        // Try to remove trailing line comments
        if !line_comment.is_empty() {
            if let Some(idx) = line.find(line_comment) {
                // Heuristic: check if the comment marker is inside a string literal
                // This handles escaped quotes better than simple counting
                let before = &line[..idx];
                if !is_inside_string(before) {
                    let cleaned = before.trim_end();
                    if !cleaned.is_empty() {
                        result.push_str(&format_line(original_line_num, cleaned));
                    }
                    continue;
                }
            }
        }

        result.push_str(&format_line(original_line_num, line));
    }

    result
}

/// Extract signatures only from code (for aggressive compression)
/// Keeps function/method/class signatures but removes bodies
fn extract_signatures_only(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    if symbols.is_empty() {
        // Fallback: just keep first line of each definition-like pattern
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut included_lines: std::collections::HashSet<u32> = std::collections::HashSet::new();

    // Include signature lines for each symbol
    for symbol in symbols {
        // Include the signature line(s)
        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            // Fallback: include the first line of the symbol
            let line_idx = (symbol.start_line - 1) as usize;
            if !included_lines.contains(&symbol.start_line) {
                result.push_str(lines[line_idx]);
                result.push('\n');
                included_lines.insert(symbol.start_line);
            }
        }

        // Include docstring if present
        if let Some(ref doc) = symbol.docstring {
            if !doc.is_empty() {
                result.push_str("  // ");
                result.push_str(doc);
                result.push('\n');
            }
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

/// Heuristic-based signature extraction when symbols aren't available
fn extract_signatures_heuristic(content: &str, language: &str) -> String {
    let mut result = String::new();
    let signature_patterns: &[&str] = match language.to_lowercase().as_str() {
        "python" => &["def ", "class ", "async def "],
        "javascript" | "typescript" | "jsx" | "tsx" => {
            &["function ", "class ", "const ", "let ", "export ", "async "]
        },
        "rust" => &["fn ", "pub fn ", "struct ", "enum ", "trait ", "impl ", "type ", "const "],
        "go" => &["func ", "type ", "const ", "var "],
        "java" | "kotlin" => {
            &["public ", "private ", "protected ", "class ", "interface ", "enum "]
        },
        "c" | "cpp" | "c++" => &["void ", "int ", "char ", "bool ", "class ", "struct ", "enum "],
        _ => &["def ", "fn ", "func ", "function ", "class ", "struct "],
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if signature_patterns.iter().any(|p| trimmed.starts_with(p)) {
            result.push_str(line);
            result.push('\n');
        }
    }

    if result.is_empty() {
        // If nothing found, return a truncated version
        content.lines().take(50).collect::<Vec<_>>().join("\n")
    } else {
        result
    }
}

/// Extract key symbols only (for extreme compression)
/// Keeps only the most important symbol definitions
fn extract_key_symbols_only(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    use infiniloom_engine::SymbolKind;

    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();

    // Filter to only key symbols: public functions, classes, structs, traits, enums
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
            ) && s.visibility != infiniloom_engine::Visibility::Private
        })
        .collect();

    // If no key symbols, fall back to all non-import symbols
    let symbols_to_use: Vec<_> = if key_symbols.is_empty() {
        symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .take(20) // Limit to top 20
            .collect()
    } else {
        key_symbols.into_iter().take(30).collect() // Limit to top 30
    };

    for symbol in symbols_to_use {
        // Add symbol name and kind as header
        result.push_str(&format!("// {}: {}\n", symbol.kind.name(), symbol.name));

        // Add signature
        if let Some(ref sig) = symbol.signature {
            result.push_str(sig);
            result.push('\n');
        } else if symbol.start_line > 0 && (symbol.start_line as usize) <= lines.len() {
            let line_idx = (symbol.start_line - 1) as usize;
            result.push_str(lines[line_idx]);
            result.push('\n');
        }
    }

    if result.is_empty() {
        extract_signatures_heuristic(content, language)
    } else {
        result
    }
}

/// Extract key symbols with small surrounding context (for focused compression)
fn extract_key_symbols_focused(
    content: &str,
    language: &str,
    symbols: &[infiniloom_engine::Symbol],
) -> String {
    use infiniloom_engine::SymbolKind;

    const CONTEXT_LINES: u32 = 2;

    if symbols.is_empty() {
        return extract_signatures_heuristic(content, language);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return String::new();
    }

    // Filter to only key symbols: public functions, classes, structs, traits, enums
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
            ) && s.visibility != infiniloom_engine::Visibility::Private
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

    let mut ranges: Vec<SymbolRange> = Vec::new();
    let mut fallback_snippets: Vec<String> = Vec::new();

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
            ranges.push(SymbolRange { start, end, labels: vec![label] });
        } else if let Some(ref sig) = symbol.signature {
            let snippet = format!("// {}\n{}", label, sig.trim());
            fallback_snippets.push(snippet);
        }
    }

    if ranges.is_empty() && fallback_snippets.is_empty() {
        return extract_signatures_heuristic(content, language);
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
        let header = format!("// Focused symbols: {}\n", range.labels.join(", "));
        result.push_str(&header);

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

/// Convert TokenizerModel to the tokenizer's TokenModel
fn to_token_model(model: TokenizerModel) -> TokenModel {
    use infiniloom_engine::tokenizer::TokenModel;
    match model {
        // OpenAI o200k_base models
        TokenizerModel::Gpt52 => TokenModel::Gpt52,
        TokenizerModel::Gpt52Pro => TokenModel::Gpt52Pro,
        TokenizerModel::Gpt51 => TokenModel::Gpt51,
        TokenizerModel::Gpt51Mini => TokenModel::Gpt51Mini,
        TokenizerModel::Gpt51Codex => TokenModel::Gpt51Codex,
        TokenizerModel::Gpt5 => TokenModel::Gpt5,
        TokenizerModel::Gpt5Mini => TokenModel::Gpt5Mini,
        TokenizerModel::Gpt5Nano => TokenModel::Gpt5Nano,
        TokenizerModel::O4Mini => TokenModel::O4Mini,
        TokenizerModel::O3 => TokenModel::O3,
        TokenizerModel::O3Mini => TokenModel::O3Mini,
        TokenizerModel::O1 => TokenModel::O1,
        TokenizerModel::O1Mini => TokenModel::O1Mini,
        TokenizerModel::O1Preview => TokenModel::O1Preview,
        TokenizerModel::Gpt4o => TokenModel::Gpt4o,
        TokenizerModel::Gpt4oMini => TokenModel::Gpt4oMini,
        // OpenAI cl100k_base models (legacy)
        TokenizerModel::Gpt4 => TokenModel::Gpt4,
        TokenizerModel::Gpt35Turbo => TokenModel::Gpt35Turbo,
        // Other vendors
        TokenizerModel::Claude => TokenModel::Claude,
        TokenizerModel::Gemini => TokenModel::Gemini,
        TokenizerModel::Llama => TokenModel::Llama,
        TokenizerModel::CodeLlama => TokenModel::CodeLlama,
        TokenizerModel::Mistral => TokenModel::Mistral,
        TokenizerModel::DeepSeek => TokenModel::DeepSeek,
        TokenizerModel::Qwen => TokenModel::Qwen,
        TokenizerModel::Cohere => TokenModel::Cohere,
        TokenizerModel::Grok => TokenModel::Grok,
    }
}

/// Estimate token count for text using the engine's accurate tokenizer
fn estimate_tokens(text: &str, model: TokenizerModel) -> usize {
    let tokenizer = Tokenizer::new();
    tokenizer.count(text, to_token_model(model)) as usize
}

/// Truncate text to fit within token limit using accurate tokenization
fn truncate_to_tokens(text: &str, max_tokens: usize, model: TokenizerModel) -> String {
    let tokenizer = Tokenizer::new();
    let token_model = to_token_model(model);
    let current = tokenizer.count(text, token_model) as usize;

    if current <= max_tokens {
        return text.to_owned();
    }

    // Use the engine's accurate truncation
    let truncated = tokenizer.truncate_to_budget(text, token_model, max_tokens as u32);

    // Try to truncate at a sensible boundary (file boundary in output)
    let markers = ["</file>", "```\n\n", "----------------------------------------\n", "\n---\n"];
    let mut best_end = truncated.len();

    for marker in markers {
        if let Some(pos) = truncated.rfind(marker) {
            let end_pos = pos + marker.len();
            if end_pos > truncated.len() / 2 {
                best_end = end_pos;
                break;
            }
        }
    }

    let mut result = truncated[..best_end].to_string();
    result.push_str("\n\n<!-- Output truncated to fit token limit -->\n");
    result
}

/// Fast heuristic-based file ranking (no symbol extraction needed)
/// This is the default mode - much faster than PageRank-based ranking
fn rank_files_fast(repo: &mut infiniloom_engine::Repository) {
    repo.files.sort_by_key(|f| {
        let path = &f.relative_path;
        let mut score: i32 = 1000; // Base score

        // === CRITICAL: Entry points (highest priority) ===
        let entry_point_patterns = [
            "main.rs",
            "main.go",
            "main.py",
            "main.ts",
            "main.js",
            "main.c",
            "main.cpp",
            "index.ts",
            "index.js",
            "index.tsx",
            "index.jsx",
            "index.py",
            "app.py",
            "app.ts",
            "app.js",
            "app.tsx",
            "app.jsx",
            "app.go",
            "server.py",
            "server.ts",
            "server.js",
            "server.go",
            "mod.rs",
            "lib.rs",
            "lib.py",
            "__main__.py",
            "__init__.py",
        ];
        if entry_point_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 5000;
        }

        // === HIGH: Config and manifest files ===
        let config_patterns = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tsconfig.json",
            "webpack.config",
            "vite.config",
            "next.config",
            "Makefile",
            "CMakeLists.txt",
            "Dockerfile",
            "docker-compose",
            ".env.example",
        ];
        if config_patterns.iter().any(|p| path.contains(p)) {
            score -= 3000;
        }

        // === MEDIUM-HIGH: Source directories ===
        if path.starts_with("src/") || path.starts_with("lib/") || path.starts_with("pkg/") {
            score -= 1000;
        }

        // === MEDIUM: API/Routes/Models ===
        let important_patterns =
            ["api/", "routes/", "models/", "controllers/", "services/", "handlers/"];
        if important_patterns.iter().any(|p| path.contains(p)) {
            score -= 500;
        }

        // === LOW: Tests (if included) ===
        let test_patterns = ["/test", "_test.", ".test.", ".spec.", "tests/", "__tests__/"];
        if test_patterns.iter().any(|p| path.contains(p)) {
            score += 2000;
        }

        // === LOWER: Examples, benchmarks, scripts ===
        let auxiliary_patterns =
            ["examples/", "example/", "benchmarks/", "bench/", "scripts/", "tools/"];
        if auxiliary_patterns.iter().any(|p| path.contains(p)) {
            score += 1500;
        }

        // === LOWEST: Vendored, generated, docs ===
        let low_priority_patterns = ["vendor/", "third_party/", "generated/", "docs/", "doc/"];
        if low_priority_patterns.iter().any(|p| path.contains(p)) {
            score += 3000;
        }

        // Prefer shallower paths (fewer slashes = more important)
        score += (path.matches('/').count() as i32) * 50;

        // Prefer shorter filenames
        if let Some(name) = path.rsplit('/').next() {
            score += (name.len() as i32) / 5;
        }

        score
    });

    // Update importance field based on new order
    let total = repo.files.len() as f32;
    for (i, file) in repo.files.iter_mut().enumerate() {
        file.importance = 1.0 - (i as f32 / total);
    }
}

/// Loaded configuration from file
#[derive(Default)]
struct LoadedConfig {
    /// Additional exclude patterns from config
    pub exclude_patterns: Vec<String>,
    /// Additional include patterns from config
    pub include_patterns: Vec<String>,
    /// Output format from config
    pub format: Option<String>,
    /// Target model from config
    pub model: Option<String>,
    /// Token budget from config
    pub token_budget: Option<u32>,
    /// Compression level from config
    pub compression: Option<String>,
    /// Include tests from config
    pub include_tests: Option<bool>,
    /// Include docs from config
    pub include_docs: Option<bool>,
    /// Security check from config
    pub security_check: Option<bool>,
    /// Fail if secrets are detected
    pub fail_on_secrets: Option<bool>,
    /// Security allowlist patterns
    pub security_allowlist: Vec<String>,
    /// Custom security patterns
    pub security_custom_patterns: Vec<String>,
    /// Redact secrets in output
    pub redact_secrets: Option<bool>,
    /// Show line numbers
    pub line_numbers: Option<bool>,
    /// Show directory structure
    pub show_directory_structure: Option<bool>,
    /// Show file summary
    pub show_file_summary: Option<bool>,
    /// Remove empty lines
    pub remove_empty_lines: Option<bool>,
    /// Remove comments
    pub remove_comments: Option<bool>,
    /// Include hidden files
    pub include_hidden: Option<bool>,
    /// Maximum file size in bytes
    pub max_file_size: Option<u64>,
}

/// Output section of config file (supports both flat and nested formats)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct OutputConfigSection {
    format: Option<String>,
    model: Option<String>,
    compression: Option<String>,
    #[serde(alias = "token_budget")]
    token_budget: Option<u32>,
    line_numbers: Option<bool>,
    show_directory_structure: Option<bool>,
    show_file_summary: Option<bool>,
    remove_empty_lines: Option<bool>,
    remove_comments: Option<bool>,
}

/// Budget section of config file (legacy flat format)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct BudgetConfig {
    #[serde(alias = "max_tokens", alias = "token_budget")]
    tokens: Option<u32>,
}

/// Scan section (from engine's Config structure)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ScanConfigSection {
    /// Include patterns (from engine's nested format)
    include: Vec<String>,
    /// Exclude patterns (from engine's nested format)
    exclude: Vec<String>,
    /// Include hidden files
    include_hidden: Option<bool>,
    /// Maximum file size (supports "10MB", "500KB", etc. or raw bytes)
    max_file_size: Option<String>,
}

/// Security section (from engine's Config structure)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct SecurityConfigSection {
    /// Enable secret scanning
    scan_secrets: Option<bool>,
    /// Fail with error if secrets are detected
    fail_on_secrets: Option<bool>,
    /// Patterns to allowlist (won't be flagged)
    #[serde(default)]
    allowlist: Vec<String>,
    /// Additional secret patterns (regex)
    #[serde(default)]
    custom_patterns: Vec<String>,
    /// Redact secrets in output
    redact_secrets: Option<bool>,
}

/// Partial config structure for serde deserialization
/// Supports multiple naming conventions used in config files:
/// - Flat format: include/exclude at top level
/// - Nested format: scan.include/scan.exclude (from engine's Config)
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct PartialConfig {
    /// Exclude patterns (flat format - legacy)
    #[serde(alias = "ignore", alias = "excludes")]
    exclude: Vec<String>,
    /// Include patterns (flat format - legacy)
    #[serde(alias = "includes")]
    include: Vec<String>,
    /// Scan configuration (nested format from engine's Config)
    scan: ScanConfigSection,
    /// Output configuration
    output: OutputConfigSection,
    /// Budget configuration (legacy flat format)
    budget: BudgetConfig,
    /// Security configuration (nested format)
    security: SecurityConfigSection,
    /// Include tests (flat format)
    include_tests: Option<bool>,
    /// Include docs (flat format)
    include_docs: Option<bool>,
    /// Security check (flat format, legacy)
    security_check: Option<bool>,
}

/// Load config file (.infiniloom.yaml, .infiniloom.toml, .infiniloom.json)
fn load_config_file(config_path: Option<&PathBuf>, repo_path: &std::path::Path) -> LoadedConfig {
    let mut config = LoadedConfig::default();

    // Try to load specified config file OR look for default config files
    if let Some(path) = config_path {
        // Use specified config file
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                parse_config_content(&content, path, &mut config);
            }
        }
        // Note: Don't return early - still need to load .infiniloomignore below
    } else {
        // Look for default config files (including .infiniloomrc for engine compatibility)
        let config_files = [
            ".infiniloomrc",
            ".infiniloom.yaml",
            ".infiniloom.yml",
            ".infiniloom.toml",
            ".infiniloom.json",
        ];
        for name in config_files {
            let path = repo_path.join(name);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // For .infiniloomrc, detect format from content
                    let effective_path = if name == ".infiniloomrc" {
                        // Detect format: JSON starts with {, YAML has :, else TOML
                        let trimmed = content.trim_start();
                        if trimmed.starts_with('{') {
                            path.with_extension("json")
                        } else if content.contains(':') {
                            path.with_extension("yaml")
                        } else {
                            path.with_extension("toml")
                        }
                    } else {
                        path.clone()
                    };
                    parse_config_content(&content, &effective_path, &mut config);
                    break;
                }
            }
        }
    }

    // Always load .infiniloomignore patterns (regardless of config file source)
    let ignore_path = repo_path.join(".infiniloomignore");
    if ignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ignore_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    config.exclude_patterns.push(line.to_owned());
                }
            }
        }
    }

    config
}

/// Parse config content using proper serde deserialization
/// Supports both flat format (legacy) and nested format (from engine's Config)
fn parse_config_content(content: &str, path: &std::path::Path, config: &mut LoadedConfig) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Try to parse and capture errors for warning
    let (partial, parse_error): (Option<PartialConfig>, Option<String>) = match ext {
        "yaml" | "yml" => match serde_yaml::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("YAML parse error: {}", e))),
        },
        "json" => match serde_json::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("JSON parse error: {}", e))),
        },
        "toml" => match toml::from_str(content) {
            Ok(parsed) => (Some(parsed), None),
            Err(e) => (None, Some(format!("TOML parse error: {}", e))),
        },
        _ => (None, Some(format!("Unknown config file extension: {}", ext))),
    };

    // Print warning if parsing failed
    if let Some(error) = parse_error {
        eprintln!(
            "{} Failed to parse config file '{}': {}",
            "warning:".yellow().bold(),
            path.display(),
            error
        );
        eprintln!("         Using default configuration.");
        return;
    }

    if let Some(parsed) = partial {
        // Merge exclude patterns from both flat and nested formats
        config.exclude_patterns.extend(parsed.exclude);
        config.exclude_patterns.extend(parsed.scan.exclude);

        // Merge include patterns from both flat and nested formats
        config.include_patterns.extend(parsed.include);
        config.include_patterns.extend(parsed.scan.include);

        // Merge output settings (config provides defaults, CLI overrides)
        if config.format.is_none() {
            config.format = parsed.output.format;
        }
        if config.model.is_none() {
            config.model = parsed.output.model;
        }
        if config.compression.is_none() {
            config.compression = parsed.output.compression;
        }
        // Token budget: check both output.token_budget and budget.tokens
        if config.token_budget.is_none() {
            config.token_budget = parsed.output.token_budget.or(parsed.budget.tokens);
        }
        if config.include_tests.is_none() {
            config.include_tests = parsed.include_tests;
        }
        if config.include_docs.is_none() {
            config.include_docs = parsed.include_docs;
        }
        // Security check: check both flat and nested formats
        if config.security_check.is_none() {
            config.security_check = parsed.security_check.or(parsed.security.scan_secrets);
        }
        // Security options from nested format
        if config.fail_on_secrets.is_none() {
            config.fail_on_secrets = parsed.security.fail_on_secrets;
        }
        if config.redact_secrets.is_none() {
            config.redact_secrets = parsed.security.redact_secrets;
        }
        if config.security_allowlist.is_empty() {
            config.security_allowlist = parsed.security.allowlist;
        }
        if config.security_custom_patterns.is_empty() {
            config.security_custom_patterns = parsed.security.custom_patterns;
        }
        // Additional output options
        if config.line_numbers.is_none() {
            config.line_numbers = parsed.output.line_numbers;
        }
        if config.show_directory_structure.is_none() {
            config.show_directory_structure = parsed.output.show_directory_structure;
        }
        if config.show_file_summary.is_none() {
            config.show_file_summary = parsed.output.show_file_summary;
        }
        if config.remove_empty_lines.is_none() {
            config.remove_empty_lines = parsed.output.remove_empty_lines;
        }
        if config.remove_comments.is_none() {
            config.remove_comments = parsed.output.remove_comments;
        }
        // Scan options from nested format
        if config.include_hidden.is_none() {
            config.include_hidden = parsed.scan.include_hidden;
        }
        if config.max_file_size.is_none() {
            if let Some(ref size_str) = parsed.scan.max_file_size {
                config.max_file_size = Some(parse_size_string(size_str));
            }
        }
    }
}

/// Parse a size string like "10MB", "500KB", "1GB" into bytes
fn parse_size_string(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    let (num_part, suffix) = if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024u64)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024 * 1024u64)
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024u64)
    } else if s.ends_with('B') {
        (&s[..s.len() - 1], 1u64)
    } else {
        (s.as_str(), 1u64)
    };
    num_part.trim().parse::<u64>().unwrap_or(50 * 1024 * 1024) * suffix
}

// =============================================================================
// Git Context Commands (Index, Diff, Impact)
// =============================================================================

/// Build or update the symbol index for fast diff context
fn cmd_index(
    path: PathBuf,
    force: bool,
    status: bool,
    verbose: bool,
    watch_mode: bool,
) -> Result<()> {
    let storage = IndexStorage::new(&path);

    // Just show status
    if status {
        if storage.exists() {
            match storage.load_meta() {
                Ok(meta) => {
                    println!("{} Index found", "✓".green());
                    println!("  Repository: {}", meta.repo_name);
                    println!("  Files indexed: {}", meta.file_count);
                    println!("  Symbols indexed: {}", meta.symbol_count);
                    println!("  Index size: {}", format_size(meta.index_size_bytes, BINARY));
                    if let Some(ref commit) = meta.commit_hash {
                        println!("  Git commit: {}", &commit[..7.min(commit.len())]);
                    }
                    println!("  Created: {}", chrono_humanize(meta.created_at));
                },
                Err(e) => {
                    eprintln!("{} Failed to read index metadata: {}", "✗".red(), e);
                },
            }
        } else {
            println!("{} No index found at {}", "✗".yellow(), path.display());
            println!("  Run 'infiniloom index' to create one.");
        }
        return Ok(());
    }

    // Helper function to build and save index
    let build_index =
        |storage: &IndexStorage, path: &std::path::Path, verbose: bool| -> Result<()> {
            if verbose {
                println!("{}", "Building symbol index...".cyan());
            }

            let start = Instant::now();

            // Build index
            let builder = IndexBuilder::new(path)
                .with_options(BuildOptions { respect_gitignore: true, ..Default::default() });

            let (index, graph) = builder.build().context("Failed to build index")?;

            // Save index
            let meta = storage
                .save_all(&index, &graph)
                .context("Failed to save index")?;

            let elapsed = start.elapsed();

            println!("{} Index built successfully", "✓".green());
            println!("  Files: {}", meta.file_count);
            println!("  Symbols: {}", meta.symbol_count);
            println!("  Size: {}", format_size(meta.index_size_bytes, BINARY));
            println!("  Time: {:.2}s", elapsed.as_secs_f64());
            println!();
            println!("Index saved to {}", storage.index_dir().display());

            Ok(())
        };

    // Check if we need to rebuild (skip check in watch mode - always build initially)
    if !watch_mode && !force && storage.exists() {
        if let Ok(meta) = storage.load_meta() {
            // Check if index is recent
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if now - meta.created_at < 300 {
                // Less than 5 minutes old
                if verbose {
                    println!("Index is recent (< 5 minutes). Use --force to rebuild.");
                }
                return Ok(());
            }
        }
    }

    // Initial build
    build_index(&storage, &path, verbose)?;

    // If watch mode, start watching for changes
    if watch_mode {
        use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc::channel;
        use std::time::Duration;

        println!();
        eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

        let (tx, rx) = channel();

        let mut watcher = PollWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if res.is_ok() {
                    let _ = tx.send(());
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .context("Failed to create file watcher")?;

        watcher
            .watch(&path, RecursiveMode::Recursive)
            .context("Failed to watch directory")?;

        // Debounce: wait for changes to settle
        let debounce_duration = Duration::from_millis(500);
        let mut last_rebuild = Instant::now();
        let mut pending_rebuild = false;

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) => {
                    pending_rebuild = true;
                    last_rebuild = Instant::now();
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check if we should rebuild (debounce elapsed)
                    if pending_rebuild && last_rebuild.elapsed() >= debounce_duration {
                        pending_rebuild = false;
                        println!();
                        eprintln!("{} File changes detected, rebuilding index...", "🔄".yellow());
                        if let Err(e) = build_index(&storage, &path, verbose) {
                            eprintln!("{} Failed to rebuild index: {}", "✗".red(), e);
                        }
                        eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());
                    }
                },
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                },
            }
        }
    }

    Ok(())
}

/// Get context for a diff
#[allow(clippy::too_many_arguments)]
fn cmd_diff(
    mut path: PathBuf,
    mut reference: Option<String>,
    staged: bool,
    depth: u8,
    budget: u32,
    format: OutputFormat,
    output: Option<PathBuf>,
    include_diff: bool,
    cli_model: Option<Model>,
    include_history: bool,
    history_count: usize,
) -> Result<()> {
    // Check git is available
    check_git_available()?;

    if reference.is_none() && !path.exists() {
        reference = Some(path.to_string_lossy().to_string());
        path = PathBuf::from(".");
    }

    let storage = IndexStorage::new(&path);
    let loaded_config = load_config_file(None, &path);

    let model: TokenizerModel = if let Some(m) = cli_model {
        m.into()
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let token_model = to_token_model(model);
    let base_ref = resolve_base_ref(reference.as_deref(), &path);

    // Always load diff content for accurate change classification
    let changes = get_diff_changes(&path, reference.as_deref(), staged, true)?;

    if changes.is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    // Convert depth
    let context_depth = match depth {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expand_budget = u32::MAX / 2;

    // Try to use pre-built index, fall back to in-memory rebuild or lazy indexing
    let mut context = if storage.exists() {
        let mut use_prebuilt = true;
        if let Ok(meta) = storage.load_meta() {
            if !is_index_fresh(&path, &meta) {
                use_prebuilt = false;
            }
        }

        if use_prebuilt {
            let (index, graph) = storage.load_all().context("Failed to load index")?;
            let expander = ContextExpander::new(&index, &graph);
            expander.expand(&changes, context_depth, expand_budget)
        } else {
            eprintln!(
                "{} Index is stale; rebuilding in memory for accurate context...",
                "→".yellow()
            );
            let builder = IndexBuilder::new(&path)
                .with_options(BuildOptions { respect_gitignore: true, ..Default::default() });
            match builder.build() {
                Ok((index, graph)) => {
                    let expander = ContextExpander::new(&index, &graph);
                    expander.expand(&changes, context_depth, expand_budget)
                },
                Err(e) => {
                    eprintln!(
                        "{} Index rebuild failed ({}). Falling back to lazy indexing...",
                        "⚠".yellow(),
                        e
                    );
                    let mut builder = LazyContextBuilder::new(&path);
                    builder
                        .generate_context(&changes, context_depth, expand_budget)
                        .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
                },
            }
        }
    } else {
        // Lazy path: build minimal index on-the-fly
        eprintln!("{} No pre-built index found, using lazy indexing...", "→".yellow());
        let mut builder = LazyContextBuilder::new(&path);
        builder
            .generate_context(&changes, context_depth, expand_budget)
            .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
    };

    if !include_diff {
        for file in &mut context.changed_files {
            file.diff_content = None;
        }
    }

    enrich_diff_context(&path, &changes, base_ref.as_deref(), &mut context, token_model)?;
    apply_diff_budget(&mut context, budget, token_model);

    // Fetch commit history for changed files if requested
    let file_history: std::collections::HashMap<String, Vec<infiniloom_engine::git::Commit>> =
        if include_history && history_count > 0 {
            use infiniloom_engine::git::GitRepo;
            let mut history_map = std::collections::HashMap::new();
            if let Ok(repo) = GitRepo::open(&path) {
                for file in &context.changed_files {
                    if let Ok(commits) = repo.file_log(&file.path, history_count) {
                        if !commits.is_empty() {
                            history_map.insert(file.path.clone(), commits);
                        }
                    }
                }
            }
            history_map
        } else {
            std::collections::HashMap::new()
        };

    // Format output
    let output_text = format_diff_context(&context, format, &file_history);

    // Write output
    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        println!("Context written to {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    // Print summary
    eprintln!();
    eprintln!(
        "{} Impact: {} ({} files, {} symbols, {} tests)",
        "→".cyan(),
        context.impact_summary.level.name(),
        context.impact_summary.direct_files + context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    );
    eprintln!("  Tokens: ~{}", context.total_tokens);

    Ok(())
}

/// Analyze impact of changes to a file or symbol
fn cmd_impact(
    path: PathBuf,
    target: Option<String>,
    is_symbol: bool,
    show_call_graph: bool,
    json_output: bool,
) -> Result<()> {
    let (path, target) = match target {
        Some(value) => (path, value),
        None => {
            if path.is_dir() {
                anyhow::bail!("Target is required. Use: infiniloom impact <target>");
            }
            (PathBuf::from("."), path.to_string_lossy().to_string())
        },
    };

    let storage = IndexStorage::new(&path);

    // Check if index exists
    if !storage.exists() {
        eprintln!("{} No index found. Run 'infiniloom index' first.", "Error:".red());
        std::process::exit(1);
    }

    // Load index
    let (index, graph) = storage.load_all().context("Failed to load index")?;

    if is_symbol {
        // Find symbol
        let symbols = index.find_symbols(&target);
        if symbols.is_empty() {
            eprintln!("{} Symbol '{}' not found in index.", "Error:".red(), target);
            std::process::exit(1);
        }

        for symbol in &symbols {
            let file = index.get_file_by_id(symbol.file_id.as_u32());
            let file_path = file.map(|f| f.path.as_str()).unwrap_or("unknown");

            if json_output {
                let callers = graph.get_callers(symbol.id.as_u32());
                let callees = graph.get_callees(symbol.id.as_u32());

                let output = serde_json::json!({
                    "symbol": {
                        "name": symbol.name,
                        "kind": symbol.kind.name(),
                        "file": file_path,
                        "line": symbol.span.start_line,
                    },
                    "callers": callers.len(),
                    "callees": callees.len(),
                    "impact": callers.len() + callees.len(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "{} {} ({}) in {}:{}",
                    "→".cyan(),
                    symbol.name,
                    symbol.kind.name(),
                    file_path,
                    symbol.span.start_line
                );

                let callers = graph.get_callers(symbol.id.as_u32());
                let callees = graph.get_callees(symbol.id.as_u32());

                if !callers.is_empty() {
                    println!("  Called by ({}):", callers.len());
                    for &caller_id in callers.iter().take(10) {
                        if let Some(caller) = index.get_symbol(caller_id) {
                            let caller_file = index.get_file_by_id(caller.file_id.as_u32());
                            let caller_path =
                                caller_file.map(|f| f.path.as_str()).unwrap_or("unknown");
                            println!("    • {} ({})", caller.name, caller_path);
                        }
                    }
                    if callers.len() > 10 {
                        println!("    ... and {} more", callers.len() - 10);
                    }
                }

                if show_call_graph && !callees.is_empty() {
                    println!("  Calls ({}):", callees.len());
                    for &callee_id in callees.iter().take(10) {
                        if let Some(callee) = index.get_symbol(callee_id) {
                            let callee_file = index.get_file_by_id(callee.file_id.as_u32());
                            let callee_path =
                                callee_file.map(|f| f.path.as_str()).unwrap_or("unknown");
                            println!("    • {} ({})", callee.name, callee_path);
                        }
                    }
                    if callees.len() > 10 {
                        println!("    ... and {} more", callees.len() - 10);
                    }
                }
            }
        }
    } else {
        // Find file
        let file = index.get_file(&target);
        if file.is_none() {
            eprintln!("{} File '{}' not found in index.", "Error:".red(), target);
            std::process::exit(1);
        }

        let file = file.unwrap();
        let importers = graph.get_importers(file.id.as_u32());

        if json_output {
            let output = serde_json::json!({
                "file": {
                    "path": file.path,
                    "language": file.language.name(),
                    "lines": file.lines,
                    "tokens": file.tokens,
                },
                "imported_by": importers.len(),
                "symbols": file.symbols.len(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!(
                "{} {} ({}, {} lines, ~{} tokens)",
                "→".cyan(),
                file.path,
                file.language.name(),
                file.lines,
                file.tokens
            );

            let symbols = index.get_file_symbols(file.id);
            if !symbols.is_empty() {
                println!("  Symbols ({}):", symbols.len());
                for symbol in symbols.iter().take(15) {
                    println!(
                        "    • {} ({}) L{}",
                        symbol.name,
                        symbol.kind.name(),
                        symbol.span.start_line
                    );
                }
                if symbols.len() > 15 {
                    println!("    ... and {} more", symbols.len() - 15);
                }
            }

            if !importers.is_empty() {
                println!("  Imported by ({}):", importers.len());
                for &importer_id in importers.iter().take(10) {
                    if let Some(importer) = index.get_file_by_id(importer_id) {
                        println!("    • {}", importer.path);
                    }
                }
                if importers.len() > 10 {
                    println!("    ... and {} more", importers.len() - 10);
                }
            }
        }
    }

    Ok(())
}

/// Get diff changes from git
fn get_diff_changes(
    repo_path: &PathBuf,
    reference: Option<&str>,
    staged: bool,
    include_diff_content: bool,
) -> Result<Vec<DiffChange>> {
    use std::process::Command;

    let mut changes = Vec::new();

    // Build git diff command
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--name-status");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    let output = cmd.output().context("Failed to run git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let status = parts[0];

            // For renames (R###), git outputs: R###\told_path\tnew_path
            let (file_path, old_path) = if status.starts_with('R') && parts.len() >= 3 {
                // Rename: parts[1] is old path, parts[2] is new path
                (parts[2].to_owned(), Some(parts[1].to_owned()))
            } else {
                (parts[1].to_owned(), None)
            };

            let change_type = match status.chars().next() {
                Some('A') => ChangeType::Added,
                Some('M') => ChangeType::Modified,
                Some('D') => ChangeType::Deleted,
                Some('R') => ChangeType::Renamed,
                _ => ChangeType::Modified,
            };

            // For modified files, get the actual changed lines
            // For renames, use the new path
            let line_ranges = match change_type {
                ChangeType::Modified => {
                    get_changed_lines(repo_path, &file_path, reference, staged)?
                },
                ChangeType::Added | ChangeType::Renamed => {
                    // Get actual line count to avoid iterating 4+ billion lines
                    let full_path = repo_path.join(&file_path);
                    let line_count = std::fs::read_to_string(&full_path)
                        .map(|content| content.lines().count() as u32)
                        .unwrap_or(1)
                        .max(1);
                    vec![(1, line_count)]
                },
                ChangeType::Deleted => {
                    // Deleted files have no lines to iterate - use empty range
                    vec![]
                },
            };

            // Optionally get the raw diff content
            // For renames, we need to check both old and new paths for diff content
            let diff_content = if include_diff_content {
                get_diff_content(repo_path, &file_path, reference, staged).ok()
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path,
                line_ranges,
                change_type,
                diff_content,
            });
        }
    }

    // Also include untracked files when looking at working tree changes
    // (not staged and no reference specified)
    if !staged && reference.is_none() {
        let untracked = get_untracked_files(repo_path)?;
        for file_path in untracked {
            // Get line count for untracked files
            let full_path = repo_path.join(&file_path);
            let line_count = std::fs::read_to_string(&full_path)
                .map(|content| content.lines().count() as u32)
                .unwrap_or(1)
                .max(1);

            // Read file content if requested
            let diff_content = if include_diff_content {
                std::fs::read_to_string(&full_path).ok().map(|content| {
                    format!(
                        "@@ -0,0 +1,{} @@\n{}",
                        line_count,
                        content
                            .lines()
                            .map(|l| format!("+{}", l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
            } else {
                None
            };

            changes.push(DiffChange {
                file_path,
                old_path: None,
                line_ranges: vec![(1, line_count)],
                change_type: ChangeType::Added,
                diff_content,
            });
        }
    }

    Ok(changes)
}

/// Get untracked files from git status
fn get_untracked_files(repo_path: &PathBuf) -> Result<Vec<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        return Ok(vec![]); // Silently ignore errors
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let untracked: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Untracked files start with "?? "
            line.strip_prefix("?? ").map(|stripped| stripped.to_owned())
        })
        .collect();

    Ok(untracked)
}

/// Get raw diff content for a file (the actual +/- lines)
fn get_diff_content(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<String> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout.to_string())
}

/// Get changed line ranges for a file
fn get_changed_lines(
    repo_path: &PathBuf,
    file_path: &str,
    reference: Option<&str>,
    staged: bool,
) -> Result<Vec<(u32, u32)>> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path);
    cmd.arg("diff");
    cmd.arg("--unified=0");

    if staged {
        cmd.arg("--cached");
    }

    if let Some(ref_spec) = reference {
        cmd.arg(ref_spec);
    }

    cmd.arg("--");
    cmd.arg(file_path);

    let output = cmd.output().context("Failed to run git diff")?;

    let mut ranges = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse @@ -start,count +start,count @@ lines
    for line in stdout.lines() {
        if line.starts_with("@@") {
            // Extract the new file range
            if let Some(plus_idx) = line.find('+') {
                let rest = &line[plus_idx + 1..];
                if let Some(space_idx) = rest.find(' ') {
                    let range_str = &rest[..space_idx];
                    let parts: Vec<&str> = range_str.split(',').collect();
                    if !parts.is_empty() {
                        let start: u32 = parts[0].parse().unwrap_or(1);
                        let count: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                        ranges.push((start, start + count.saturating_sub(1)));
                    }
                }
            }
        }
    }

    if ranges.is_empty() {
        // Fallback: get actual line count from file instead of arbitrary 100
        let full_path = repo_path.join(file_path);
        let line_count = std::fs::read_to_string(&full_path)
            .map(|content| content.lines().count() as u32)
            .unwrap_or(1)
            .max(1); // Ensure at least 1 line
        ranges.push((1, line_count));
    }

    Ok(ranges)
}

/// Resolve the base git reference from a reference string
/// For "main..feature" returns "main", for "HEAD~1" returns "HEAD~1"
fn resolve_base_ref(reference: Option<&str>, repo_path: &std::path::Path) -> Option<String> {
    let ref_str = match reference {
        Some(r) => r,
        None => "HEAD",
    };

    // Handle range format: "base..head" or "base...head"
    if let Some(base) = ref_str.split("..").next() {
        if !base.is_empty() && base != ref_str {
            return Some(base.to_owned());
        }
    }

    // For single refs like "HEAD~1", "main", etc., use as-is
    // But verify it's a valid ref first
    use std::process::Command;
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "--verify", ref_str])
        .output()
        .ok()?;

    if output.status.success() {
        Some(ref_str.to_owned())
    } else {
        None
    }
}

/// Read file content from a git reference
/// Uses `git show ref:path` to retrieve file content from a specific commit/ref
fn read_file_from_git(
    repo_path: &std::path::Path,
    git_ref: &str,
    file_path: &str,
) -> Option<String> {
    use std::process::Command;

    // Git show format: ref:path
    let ref_path = format!("{}:{}", git_ref, file_path);

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["show", &ref_path])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn diff_preamble(context: &infiniloom_engine::index::ExpandedContext) -> String {
    format!(
        "Use this diff context to understand changes. Start with changed file snippets, then dependent symbols/files/tests. Impact: {}.",
        context.impact_summary.level.name()
    )
}

/// Type alias for file history map (file path -> list of commits)
type FileHistory = std::collections::HashMap<String, Vec<infiniloom_engine::git::Commit>>;

/// Format diff context for output
fn format_diff_context(
    context: &infiniloom_engine::index::ExpandedContext,
    format: OutputFormat,
    history: &FileHistory,
) -> String {
    match format {
        OutputFormat::Xml => format_diff_context_xml(context, history),
        OutputFormat::Json => format_diff_context_json(context, history),
        OutputFormat::Markdown => format_diff_context_markdown(context, history),
        OutputFormat::Yaml => format_diff_context_yaml(context, history),
        OutputFormat::Toon => format_diff_context_toon(context, history),
        OutputFormat::Plain => format_diff_context_plain(context, history),
    }
}

fn format_diff_context_json(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "preamble": diff_preamble(context),
        "changed_files": context.changed_files.iter().map(|f| {
            let file_history = history.get(&f.path).map(|commits| {
                commits.iter().map(|c| serde_json::json!({
                    "hash": &c.short_hash,
                    "author": &c.author,
                    "date": &c.date,
                    "message": &c.message,
                })).collect::<Vec<_>>()
            });
            serde_json::json!({
                "path": &f.path,
                "language": &f.language,
                "tokens": f.tokens,
                "diff_content": &f.diff_content,
                "history": file_history,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "dependent_files": context.dependent_files.iter().map(|f| {
            serde_json::json!({
                "path": &f.path,
                "reason": &f.relevance_reason,
                "relevance": f.relevance_score,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "changed_symbols": context.changed_symbols.iter().map(|s| {
            serde_json::json!({
                "name": &s.name,
                "kind": &s.kind,
                "file": &s.file_path,
                "line": s.start_line,
            })
        }).collect::<Vec<_>>(),
        "dependent_symbols": context.dependent_symbols.iter().map(|s| {
            serde_json::json!({
                "name": &s.name,
                "kind": &s.kind,
                "file": &s.file_path,
                "line": s.start_line,
                "reason": &s.relevance_reason,
                "relevance": s.relevance_score,
            })
        }).collect::<Vec<_>>(),
        "related_tests": context.related_tests.iter().map(|f| {
            serde_json::json!({
                "path": &f.path,
                "snippets": f.snippets.iter().map(|s| {
                    serde_json::json!({
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "reason": &s.reason,
                        "content": &s.content,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "call_chains": context.call_chains.iter().map(|c| c.symbols.join(" → ")).collect::<Vec<_>>(),
        "impact": {
            "level": context.impact_summary.level.name(),
            "description": &context.impact_summary.description,
            "direct_files": context.impact_summary.direct_files,
            "transitive_files": context.impact_summary.transitive_files,
            "affected_symbols": context.impact_summary.affected_symbols,
            "affected_tests": context.impact_summary.affected_tests,
        },
        "tokens": context.total_tokens,
    })).unwrap_or_else(|_| "{}".to_owned())
}

fn format_diff_context_markdown(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut md = String::new();

    // Header
    md.push_str("# Diff Context\n\n");
    md.push_str(&format!("> {}\n\n", diff_preamble(context)));

    // Impact summary
    md.push_str("## Impact Summary\n\n");
    md.push_str(&format!("**Level:** {}\n\n", context.impact_summary.level.name()));
    md.push_str(&format!("{}\n\n", context.impact_summary.description));
    md.push_str(&format!(
        "- Direct files: {}\n- Transitive files: {}\n- Affected symbols: {}\n- Affected tests: {}\n- Total tokens: {}\n\n",
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests,
        context.total_tokens
    ));

    // Changed files
    md.push_str("## Changed Files\n\n");
    for file in &context.changed_files {
        md.push_str(&format!("### `{}`\n\n", file.path));
        md.push_str(&format!("- Language: {}\n- Tokens: {}\n\n", file.language, file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                md.push_str("**Recent History**\n\n");
                md.push_str("| Commit | Author | Date | Message |\n");
                md.push_str("|--------|--------|------|--------|\n");
                for commit in commits {
                    md.push_str(&format!(
                        "| `{}` | {} | {} | {} |\n",
                        commit.short_hash, commit.author, commit.date, commit.message
                    ));
                }
                md.push('\n');
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            md.push_str("```diff\n");
            md.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                md.push('\n');
            }
            md.push_str("```\n\n");
        }
        if !file.snippets.is_empty() {
            md.push_str("**Snippets**\n\n");
            for snippet in &file.snippets {
                md.push_str(&format!(
                    "- {} (lines {}-{})\n\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                md.push_str("```text\n");
                md.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
        }
    }

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        md.push_str("## Changed Symbols\n\n");
        md.push_str("| Symbol | Kind | File | Line |\n");
        md.push_str("|--------|------|------|------|\n");
        for sym in &context.changed_symbols {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
        md.push('\n');
    }

    // Dependent symbols
    if !context.dependent_symbols.is_empty() {
        md.push_str("## Dependent Symbols\n\n");
        md.push_str("| Symbol | Kind | File | Line | Relevance |\n");
        md.push_str("|--------|------|------|------|-----------|\n");
        for sym in &context.dependent_symbols {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {:.2} |\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
        md.push('\n');
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        md.push_str("## Dependent Files\n\n");
        md.push_str("| File | Reason | Relevance |\n");
        md.push_str("|------|--------|----------|\n");
        for file in &context.dependent_files {
            md.push_str(&format!(
                "| `{}` | {} | {:.2} |\n",
                file.path, file.relevance_reason, file.relevance_score
            ));
        }
        md.push('\n');

        for file in &context.dependent_files {
            if !file.snippets.is_empty() {
                md.push_str(&format!("### `{}` Snippets\n\n", file.path));
                for snippet in &file.snippets {
                    md.push_str(&format!(
                        "- {} (lines {}-{})\n\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    md.push_str("```text\n");
                    md.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        md.push('\n');
                    }
                    md.push_str("```\n\n");
                }
            }
        }
    }

    // Related tests
    if !context.related_tests.is_empty() {
        md.push_str("## Related Tests\n\n");
        for test in &context.related_tests {
            md.push_str(&format!("### `{}`\n\n", test.path));
            if test.snippets.is_empty() {
                md.push_str("- No focused snippets selected\n\n");
                continue;
            }
            for snippet in &test.snippets {
                md.push_str(&format!(
                    "- {} (lines {}-{})\n\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                md.push_str("```text\n");
                md.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
        }
    }

    // Call chains
    if !context.call_chains.is_empty() {
        md.push_str("## Call Graph\n\n");
        for chain in &context.call_chains {
            md.push_str(&format!("- {}\n", chain.symbols.join(" → ")));
        }
        md.push('\n');
    }

    md
}

fn format_diff_context_yaml(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut yaml = String::new();

    yaml.push_str("# Diff Context\n\n");
    yaml.push_str("preamble: |\n");
    for line in diff_preamble(context).lines() {
        yaml.push_str(&format!("  {}\n", line));
    }
    yaml.push('\n');

    // Impact
    yaml.push_str("impact:\n");
    yaml.push_str(&format!("  level: {}\n", context.impact_summary.level.name()));
    yaml.push_str(&format!(
        "  description: \"{}\"\n",
        context.impact_summary.description.replace('"', "\\\"")
    ));
    yaml.push_str(&format!("  direct_files: {}\n", context.impact_summary.direct_files));
    yaml.push_str(&format!("  transitive_files: {}\n", context.impact_summary.transitive_files));
    yaml.push_str(&format!("  affected_symbols: {}\n", context.impact_summary.affected_symbols));
    yaml.push_str(&format!("  affected_tests: {}\n", context.impact_summary.affected_tests));
    yaml.push_str(&format!("total_tokens: {}\n\n", context.total_tokens));

    // Changed files
    yaml.push_str("changed_files:\n");
    for file in &context.changed_files {
        yaml.push_str(&format!("  - path: \"{}\"\n", file.path));
        yaml.push_str(&format!("    language: {}\n", file.language));
        yaml.push_str(&format!("    tokens: {}\n", file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                yaml.push_str("    history:\n");
                for commit in commits {
                    yaml.push_str(&format!("      - hash: \"{}\"\n", commit.short_hash));
                    yaml.push_str(&format!("        author: \"{}\"\n", commit.author));
                    yaml.push_str(&format!("        date: \"{}\"\n", commit.date));
                    yaml.push_str(&format!(
                        "        message: \"{}\"\n",
                        commit.message.replace('"', "\\\"")
                    ));
                }
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            yaml.push_str("    diff: |\n");
            for line in diff_content.lines() {
                yaml.push_str(&format!("      {}\n", line));
            }
        }
        if !file.snippets.is_empty() {
            yaml.push_str("    snippets:\n");
            for snippet in &file.snippets {
                yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                yaml.push_str(&format!(
                    "        reason: \"{}\"\n",
                    snippet.reason.replace('"', "\\\"")
                ));
                yaml.push_str("        content: |\n");
                for line in snippet.content.lines() {
                    yaml.push_str(&format!("          {}\n", line));
                }
            }
        }
    }

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        yaml.push_str("\nchanged_symbols:\n");
        for sym in &context.changed_symbols {
            yaml.push_str(&format!("  - name: \"{}\"\n", sym.name));
            yaml.push_str(&format!("    kind: {}\n", sym.kind));
            yaml.push_str(&format!("    file: \"{}\"\n", sym.file_path));
            yaml.push_str(&format!("    line: {}\n", sym.start_line));
        }
    }

    if !context.dependent_symbols.is_empty() {
        yaml.push_str("\ndependent_symbols:\n");
        for sym in &context.dependent_symbols {
            yaml.push_str(&format!("  - name: \"{}\"\n", sym.name));
            yaml.push_str(&format!("    kind: {}\n", sym.kind));
            yaml.push_str(&format!("    file: \"{}\"\n", sym.file_path));
            yaml.push_str(&format!("    line: {}\n", sym.start_line));
            yaml.push_str(&format!(
                "    reason: \"{}\"\n",
                sym.relevance_reason.replace('"', "\\\"")
            ));
            yaml.push_str(&format!("    relevance: {:.2}\n", sym.relevance_score));
        }
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        yaml.push_str("\ndependent_files:\n");
        for file in &context.dependent_files {
            yaml.push_str(&format!("  - path: \"{}\"\n", file.path));
            yaml.push_str(&format!("    reason: \"{}\"\n", file.relevance_reason));
            yaml.push_str(&format!("    relevance: {:.2}\n", file.relevance_score));
            if !file.snippets.is_empty() {
                yaml.push_str("    snippets:\n");
                for snippet in &file.snippets {
                    yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                    yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                    yaml.push_str(&format!(
                        "        reason: \"{}\"\n",
                        snippet.reason.replace('"', "\\\"")
                    ));
                    yaml.push_str("        content: |\n");
                    for line in snippet.content.lines() {
                        yaml.push_str(&format!("          {}\n", line));
                    }
                }
            }
        }
    }

    // Related tests
    if !context.related_tests.is_empty() {
        yaml.push_str("\nrelated_tests:\n");
        for test in &context.related_tests {
            yaml.push_str(&format!("  - path: \"{}\"\n", test.path));
            if !test.snippets.is_empty() {
                yaml.push_str("    snippets:\n");
                for snippet in &test.snippets {
                    yaml.push_str(&format!("      - start_line: {}\n", snippet.start_line));
                    yaml.push_str(&format!("        end_line: {}\n", snippet.end_line));
                    yaml.push_str(&format!(
                        "        reason: \"{}\"\n",
                        snippet.reason.replace('"', "\\\"")
                    ));
                    yaml.push_str("        content: |\n");
                    for line in snippet.content.lines() {
                        yaml.push_str(&format!("          {}\n", line));
                    }
                }
            }
        }
    }

    // Call chains
    if !context.call_chains.is_empty() {
        yaml.push_str("\ncall_chains:\n");
        for chain in &context.call_chains {
            yaml.push_str(&format!("  - \"{}\"\n", chain.symbols.join(" → ")));
        }
    }

    yaml
}

fn format_diff_context_toon(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    // TOON = Token-Optimized Output Notation - minimal delimiters
    let mut toon = String::new();

    // Header
    toon.push_str(&format!(
        "DIFF|{}|d{}t{}s{}T{}\n",
        context.impact_summary.level.name(),
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.total_tokens
    ));
    toon.push_str(&format!("PRE|{}\n", diff_preamble(context)));

    // Changed files
    toon.push_str("FILES:\n");
    for file in &context.changed_files {
        toon.push_str(&format!("F|{}|{}|{}\n", file.path, file.language, file.tokens));

        // Include file history if available (compact format)
        if let Some(commits) = history.get(&file.path) {
            for commit in commits {
                toon.push_str(&format!(
                    "H|{}|{}|{}|{}\n",
                    commit.short_hash, commit.author, commit.date, commit.message
                ));
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            toon.push_str("D{\n");
            toon.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                toon.push('\n');
            }
            toon.push_str("}D\n");
        }
        for snippet in &file.snippets {
            toon.push_str(&format!(
                "N|{}|{}|{}\n",
                snippet.start_line, snippet.end_line, snippet.reason
            ));
            toon.push_str("C{\n");
            toon.push_str(&snippet.content);
            if !snippet.content.ends_with('\n') {
                toon.push('\n');
            }
            toon.push_str("}C\n");
        }
    }

    // Symbols
    if !context.changed_symbols.is_empty() {
        toon.push_str("SYMS:\n");
        for sym in &context.changed_symbols {
            toon.push_str(&format!(
                "S|{}|{}|{}|{}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
    }

    if !context.dependent_symbols.is_empty() {
        toon.push_str("DEPSYMS:\n");
        for sym in &context.dependent_symbols {
            toon.push_str(&format!(
                "S|{}|{}|{}|{}|{:.1}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
    }

    // Dependents
    if !context.dependent_files.is_empty() {
        toon.push_str("DEPS:\n");
        for file in &context.dependent_files {
            toon.push_str(&format!("P|{}|{:.1}\n", file.path, file.relevance_score));
            for snippet in &file.snippets {
                toon.push_str(&format!(
                    "N|{}|{}|{}\n",
                    snippet.start_line, snippet.end_line, snippet.reason
                ));
                toon.push_str("C{\n");
                toon.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    toon.push('\n');
                }
                toon.push_str("}C\n");
            }
        }
    }

    // Tests
    if !context.related_tests.is_empty() {
        toon.push_str("TESTS:\n");
        for test in &context.related_tests {
            toon.push_str(&format!("T|{}\n", test.path));
            for snippet in &test.snippets {
                toon.push_str(&format!(
                    "N|{}|{}|{}\n",
                    snippet.start_line, snippet.end_line, snippet.reason
                ));
                toon.push_str("C{\n");
                toon.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    toon.push('\n');
                }
                toon.push_str("}C\n");
            }
        }
    }

    toon
}

fn format_diff_context_plain(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut plain = String::new();

    // Header
    plain.push_str("=== DIFF CONTEXT ===\n\n");
    plain.push_str(&format!("{}\n\n", diff_preamble(context)));
    plain.push_str(&format!(
        "Impact: {} - {}\n",
        context.impact_summary.level.name(),
        context.impact_summary.description
    ));
    plain.push_str(&format!(
        "Stats: {} direct files, {} transitive, {} symbols, {} tests\n",
        context.impact_summary.direct_files,
        context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    ));
    plain.push_str(&format!("Total tokens: {}\n\n", context.total_tokens));

    // Changed files
    plain.push_str("--- CHANGED FILES ---\n");
    for file in &context.changed_files {
        plain.push_str(&format!("\n{} ({}, {} tokens)\n", file.path, file.language, file.tokens));

        // Include file history if available
        if let Some(commits) = history.get(&file.path) {
            if !commits.is_empty() {
                plain.push_str("Recent history:\n");
                for commit in commits {
                    plain.push_str(&format!(
                        "  {} ({}, {}) {}\n",
                        commit.short_hash, commit.author, commit.date, commit.message
                    ));
                }
            }
        }

        if let Some(ref diff_content) = file.diff_content {
            plain.push_str(diff_content);
            if !diff_content.ends_with('\n') {
                plain.push('\n');
            }
        }
        if !file.snippets.is_empty() {
            plain.push_str("Snippets:\n");
            for snippet in &file.snippets {
                plain.push_str(&format!(
                    "- {} (lines {}-{})\n",
                    snippet.reason, snippet.start_line, snippet.end_line
                ));
                plain.push_str(&snippet.content);
                if !snippet.content.ends_with('\n') {
                    plain.push('\n');
                }
            }
        }
    }

    // Symbols
    if !context.changed_symbols.is_empty() {
        plain.push_str("\n--- CHANGED SYMBOLS ---\n");
        for sym in &context.changed_symbols {
            plain.push_str(&format!(
                "{} ({}) in {} line {}\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
    }

    if !context.dependent_symbols.is_empty() {
        plain.push_str("\n--- DEPENDENT SYMBOLS ---\n");
        for sym in &context.dependent_symbols {
            plain.push_str(&format!(
                "{} ({}) in {} line {} (relevance: {:.2})\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score
            ));
        }
    }

    // Dependents
    if !context.dependent_files.is_empty() {
        plain.push_str("\n--- DEPENDENT FILES ---\n");
        for file in &context.dependent_files {
            plain.push_str(&format!(
                "{} - {} (relevance: {:.2})\n",
                file.path, file.relevance_reason, file.relevance_score
            ));
            if !file.snippets.is_empty() {
                plain.push_str("Snippets:\n");
                for snippet in &file.snippets {
                    plain.push_str(&format!(
                        "- {} (lines {}-{})\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    plain.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        plain.push('\n');
                    }
                }
            }
        }
    }

    // Tests
    if !context.related_tests.is_empty() {
        plain.push_str("\n--- RELATED TESTS ---\n");
        for test in &context.related_tests {
            plain.push_str(&format!("{}\n", test.path));
            if !test.snippets.is_empty() {
                plain.push_str("Snippets:\n");
                for snippet in &test.snippets {
                    plain.push_str(&format!(
                        "- {} (lines {}-{})\n",
                        snippet.reason, snippet.start_line, snippet.end_line
                    ));
                    plain.push_str(&snippet.content);
                    if !snippet.content.ends_with('\n') {
                        plain.push('\n');
                    }
                }
            }
        }
    }

    plain
}

fn format_diff_context_xml(
    context: &infiniloom_engine::index::ExpandedContext,
    history: &FileHistory,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<diff_context>\n");

    // Summary
    xml.push_str("  <summary>\n");
    xml.push_str(&format!(
        "    <preamble>{}</preamble>\n",
        escape_xml_text(&diff_preamble(context))
    ));
    xml.push_str(&format!(
        "    <impact level=\"{}\">{}</impact>\n",
        context.impact_summary.level.name(),
        context.impact_summary.description
    ));
    xml.push_str(&format!(
        "    <stats files=\"{}\" symbols=\"{}\" tests=\"{}\"/>\n",
        context.impact_summary.direct_files + context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    ));
    xml.push_str("  </summary>\n");

    // Changed files
    xml.push_str("  <changed_files>\n");
    for file in &context.changed_files {
        let has_snippets = !file.snippets.is_empty();
        let has_history = history.get(&file.path).is_some_and(|h| !h.is_empty());
        let needs_full_element = file.diff_content.is_some() || has_snippets || has_history;

        if needs_full_element {
            xml.push_str(&format!(
                "    <file path=\"{}\" language=\"{}\" tokens=\"{}\">\n",
                file.path, file.language, file.tokens
            ));

            // Include file history if available
            if let Some(commits) = history.get(&file.path) {
                if !commits.is_empty() {
                    xml.push_str("      <history>\n");
                    for commit in commits {
                        xml.push_str(&format!(
                            "        <commit hash=\"{}\" date=\"{}\" author=\"{}\">\n          {}\n        </commit>\n",
                            escape_xml_attr(&commit.short_hash),
                            escape_xml_attr(&commit.date),
                            escape_xml_attr(&commit.author),
                            escape_xml_text(&commit.message)
                        ));
                    }
                    xml.push_str("      </history>\n");
                }
            }

            if let Some(ref diff_content) = file.diff_content {
                xml.push_str("      <diff>\n<![CDATA[\n");
                xml.push_str(diff_content);
                xml.push_str("]]>\n      </diff>\n");
            }
            if has_snippets {
                xml.push_str("      <snippets>\n");
                for snippet in &file.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
            }
            xml.push_str("    </file>\n");
        } else {
            xml.push_str(&format!(
                "    <file path=\"{}\" language=\"{}\" tokens=\"{}\"/>\n",
                file.path, file.language, file.tokens
            ));
        }
    }
    xml.push_str("  </changed_files>\n");

    // Changed symbols
    if !context.changed_symbols.is_empty() {
        xml.push_str("  <changed_symbols>\n");
        for sym in &context.changed_symbols {
            xml.push_str(&format!(
                "    <symbol name=\"{}\" kind=\"{}\" file=\"{}\" line=\"{}\"/>\n",
                sym.name, sym.kind, sym.file_path, sym.start_line
            ));
        }
        xml.push_str("  </changed_symbols>\n");
    }

    if !context.dependent_symbols.is_empty() {
        xml.push_str("  <dependent_symbols>\n");
        for sym in &context.dependent_symbols {
            let reason = sym.relevance_reason.replace('"', "&quot;");
            xml.push_str(&format!(
                "    <symbol name=\"{}\" kind=\"{}\" file=\"{}\" line=\"{}\" relevance=\"{:.2}\" reason=\"{}\"/>\n",
                sym.name, sym.kind, sym.file_path, sym.start_line, sym.relevance_score, reason
            ));
        }
        xml.push_str("  </dependent_symbols>\n");
    }

    // Dependent files
    if !context.dependent_files.is_empty() {
        xml.push_str("  <dependent_files>\n");
        for file in &context.dependent_files {
            if file.snippets.is_empty() {
                xml.push_str(&format!(
                    "    <file path=\"{}\" reason=\"{}\" relevance=\"{:.2}\"/>\n",
                    file.path, file.relevance_reason, file.relevance_score
                ));
            } else {
                xml.push_str(&format!(
                    "    <file path=\"{}\" reason=\"{}\" relevance=\"{:.2}\">\n",
                    file.path, file.relevance_reason, file.relevance_score
                ));
                xml.push_str("      <snippets>\n");
                for snippet in &file.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
                xml.push_str("    </file>\n");
            }
        }
        xml.push_str("  </dependent_files>\n");
    }

    // Related tests
    if !context.related_tests.is_empty() {
        xml.push_str("  <related_tests>\n");
        for test in &context.related_tests {
            if test.snippets.is_empty() {
                xml.push_str(&format!("    <test path=\"{}\"/>\n", test.path));
            } else {
                xml.push_str(&format!("    <test path=\"{}\">\n", test.path));
                xml.push_str("      <snippets>\n");
                for snippet in &test.snippets {
                    let reason = snippet.reason.replace('"', "&quot;");
                    xml.push_str(&format!(
                        "        <snippet start=\"{}\" end=\"{}\" reason=\"{}\">\n<![CDATA[\n",
                        snippet.start_line, snippet.end_line, reason
                    ));
                    xml.push_str(&snippet.content);
                    xml.push_str("]]>\n        </snippet>\n");
                }
                xml.push_str("      </snippets>\n");
                xml.push_str("    </test>\n");
            }
        }
        xml.push_str("  </related_tests>\n");
    }

    // Call chains
    if !context.call_chains.is_empty() {
        xml.push_str("  <call_graph>\n");
        for chain in &context.call_chains {
            xml.push_str(&format!("    <chain>{}</chain>\n", chain.symbols.join(" → ")));
        }
        xml.push_str("  </call_graph>\n");
    }

    xml.push_str("</diff_context>\n");
    xml
}

fn is_index_fresh(repo_path: &PathBuf, meta: &infiniloom_engine::index::IndexMeta) -> bool {
    let repo = match GitRepo::open(repo_path) {
        Ok(repo) => repo,
        Err(_) => return true,
    };

    let status = match repo.status() {
        Ok(status) => status,
        Err(_) => return false,
    };
    if !status.is_empty() {
        return false;
    }

    if let Ok(head) = repo.current_commit() {
        if let Some(ref index_commit) = meta.commit_hash {
            if index_commit != &head {
                return false;
            }
        }
    }

    true
}

#[derive(Clone)]
struct SnippetRange {
    start: u32,
    end: u32,
    reasons: Vec<String>,
}

fn merge_snippet_ranges(mut ranges: Vec<SnippetRange>) -> Vec<SnippetRange> {
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<SnippetRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                for reason in range.reasons {
                    if !last.reasons.contains(&reason) {
                        last.reasons.push(reason);
                    }
                }
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    merged
}

fn line_contains_symbol_name(line: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut offset = 0;
    while let Some(pos) = line[offset..].find(name) {
        let start = offset + pos;
        let end = start + name.len();

        let before = line[..start].chars().rev().next();
        let after = line[end..].chars().next();

        let before_ok = before.map(|c| !is_word_char(c)).unwrap_or(true);
        let after_ok = after.map(|c| !is_word_char(c)).unwrap_or(true);

        if before_ok && after_ok {
            return true;
        }

        offset = end;
    }

    false
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn enrich_diff_context(
    repo_path: &PathBuf,
    changes: &[DiffChange],
    base_ref: Option<&str>,
    context: &mut infiniloom_engine::index::ExpandedContext,
    token_model: TokenModel,
) -> Result<()> {
    use std::collections::HashMap;

    const CONTEXT_LINES: u32 = 3;
    let tokenizer = Tokenizer::new();
    let mut change_by_path: HashMap<String, &DiffChange> = HashMap::new();
    for change in changes {
        change_by_path.insert(change.file_path.clone(), change);
        if let Some(old_path) = &change.old_path {
            change_by_path.insert(old_path.clone(), change);
        }
    }

    let mut changed_symbols_by_file: HashMap<
        String,
        Vec<&infiniloom_engine::index::ContextSymbol>,
    > = HashMap::new();
    for sym in &context.changed_symbols {
        changed_symbols_by_file
            .entry(sym.file_path.clone())
            .or_default()
            .push(sym);
    }

    let mut dependent_symbols_by_file: HashMap<
        String,
        Vec<&infiniloom_engine::index::ContextSymbol>,
    > = HashMap::new();
    for sym in &context.dependent_symbols {
        dependent_symbols_by_file
            .entry(sym.file_path.clone())
            .or_default()
            .push(sym);
    }

    let mut file_lines_cache: HashMap<String, Vec<String>> = HashMap::new();

    let changed_symbol_names: Vec<String> = context
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let mut process_file = |file: &mut infiniloom_engine::index::ContextFile, is_test: bool| {
        let lines = if let Some(lines) = file_lines_cache.get(&file.path) {
            lines.clone()
        } else {
            let full_path = repo_path.join(&file.path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    let lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();
                    file_lines_cache.insert(file.path.clone(), lines.clone());
                    lines
                },
                Err(_) => {
                    let fallback_content = if let Some(change) = change_by_path.get(&file.path) {
                        if let Some(ref_path) = base_ref {
                            if let Some(old_path) = &change.old_path {
                                read_file_from_git(repo_path, ref_path, old_path)
                                    .or_else(|| read_file_from_git(repo_path, ref_path, &file.path))
                            } else {
                                read_file_from_git(repo_path, ref_path, &file.path)
                            }
                        } else {
                            None
                        }
                    } else if let Some(ref_path) = base_ref {
                        read_file_from_git(repo_path, ref_path, &file.path)
                    } else {
                        None
                    };

                    if let Some(content) = fallback_content {
                        let lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();
                        file_lines_cache.insert(file.path.clone(), lines.clone());
                        lines
                    } else {
                        file.snippets = Vec::new();
                        file.tokens = file
                            .diff_content
                            .as_deref()
                            .map(|d| tokenizer.count(d, token_model))
                            .unwrap_or(0);
                        return;
                    }
                },
            }
        };

        let total_lines = lines.len() as u32;
        if total_lines == 0 {
            return;
        }

        let mut ranges: Vec<SnippetRange> = Vec::new();

        for (start, end) in &file.relevant_sections {
            let start = start.saturating_sub(CONTEXT_LINES).max(1);
            let end = end.saturating_add(CONTEXT_LINES).min(total_lines);
            if start <= end {
                ranges.push(SnippetRange { start, end, reasons: vec!["diff hunk".to_owned()] });
            }
        }

        if let Some(symbols) = changed_symbols_by_file.get(&file.path) {
            for sym in symbols {
                let start = sym.start_line.max(1);
                let end = sym.end_line.max(start).min(total_lines);
                ranges.push(SnippetRange {
                    start,
                    end,
                    reasons: vec![format!("changed symbol: {}", sym.name)],
                });
            }
        }

        if let Some(symbols) = dependent_symbols_by_file.get(&file.path) {
            for sym in symbols {
                let start = sym.start_line.max(1);
                let end = sym.end_line.max(start).min(total_lines);
                ranges.push(SnippetRange {
                    start,
                    end,
                    reasons: vec![format!("dependent symbol: {}", sym.name)],
                });
            }
        }

        if is_test && !changed_symbol_names.is_empty() {
            for (idx, line) in lines.iter().enumerate() {
                let line_no = idx as u32 + 1;
                for name in &changed_symbol_names {
                    if line_contains_symbol_name(line, name) {
                        let start = line_no.saturating_sub(CONTEXT_LINES).max(1);
                        let end = line_no.saturating_add(CONTEXT_LINES).min(total_lines);
                        ranges.push(SnippetRange {
                            start,
                            end,
                            reasons: vec![format!("references changed symbol: {}", name)],
                        });
                    }
                }
            }
        }

        if ranges.is_empty() {
            if let Some(change) = change_by_path.get(&file.path) {
                if change.change_type == ChangeType::Deleted {
                    let end = total_lines.min(200);
                    ranges.push(SnippetRange {
                        start: 1,
                        end,
                        reasons: vec!["file removed".to_owned()],
                    });
                }
            }
        }

        let merged = merge_snippet_ranges(ranges);
        let mut snippets = Vec::new();
        let mut tokens = file
            .diff_content
            .as_deref()
            .map(|d| tokenizer.count(d, token_model))
            .unwrap_or(0);

        for range in merged {
            let start_idx = range.start.saturating_sub(1) as usize;
            let end_idx = range.end.saturating_sub(1) as usize;
            if start_idx >= lines.len() || end_idx >= lines.len() || start_idx > end_idx {
                continue;
            }
            let content = lines[start_idx..=end_idx].join("\n");
            tokens += tokenizer.count(&content, token_model);
            snippets.push(ContextSnippet {
                start_line: range.start,
                end_line: range.end,
                reason: range.reasons.join("; "),
                content,
            });
        }

        file.snippets = snippets;
        file.tokens = tokens;
    };

    for file in context.changed_files.iter_mut() {
        process_file(file, false);
    }

    for file in context.dependent_files.iter_mut() {
        process_file(file, false);
    }

    for file in context.related_tests.iter_mut() {
        process_file(file, true);
    }

    Ok(())
}

fn apply_diff_budget(
    context: &mut infiniloom_engine::index::ExpandedContext,
    budget: u32,
    token_model: TokenModel,
) {
    let tokenizer = Tokenizer::new();
    let mut running_tokens: u32 = context.changed_files.iter().map(|f| f.tokens).sum();

    if budget > 0 {
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        enum SnippetOwner {
            Dependent,
            Test,
        }

        struct SnippetCandidate {
            owner: SnippetOwner,
            file_index: usize,
            snippet_index: usize,
            tokens: u32,
            score: f32,
        }

        let snippet_score =
            |file: &infiniloom_engine::index::ContextFile, snippet: &ContextSnippet| -> f32 {
                let mut score = file.relevance_score;
                let reason = snippet.reason.as_str();
                if reason.contains("changed symbol") {
                    score += 0.3;
                } else if reason.contains("dependent symbol") {
                    score += 0.2;
                } else if reason.contains("diff hunk") {
                    score += 0.1;
                } else if reason.contains("file removed") {
                    score += 0.25;
                }
                score
            };

        let mut candidates: Vec<SnippetCandidate> = Vec::new();

        for (file_index, file) in context.dependent_files.iter().enumerate() {
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                let tokens = tokenizer.count(&snippet.content, token_model);
                candidates.push(SnippetCandidate {
                    owner: SnippetOwner::Dependent,
                    file_index,
                    snippet_index,
                    tokens,
                    score: snippet_score(file, snippet),
                });
            }
        }

        for (file_index, file) in context.related_tests.iter().enumerate() {
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                let tokens = tokenizer.count(&snippet.content, token_model);
                candidates.push(SnippetCandidate {
                    owner: SnippetOwner::Test,
                    file_index,
                    snippet_index,
                    tokens,
                    score: snippet_score(file, snippet),
                });
            }
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.tokens.cmp(&b.tokens))
        });

        let mut keep: std::collections::HashSet<(SnippetOwner, usize, usize)> =
            std::collections::HashSet::new();

        for candidate in candidates {
            if running_tokens.saturating_add(candidate.tokens) <= budget {
                running_tokens = running_tokens.saturating_add(candidate.tokens);
                keep.insert((candidate.owner, candidate.file_index, candidate.snippet_index));
            }
        }

        for (file_index, file) in context.dependent_files.iter_mut().enumerate() {
            let mut tokens: u32 = 0;
            let mut kept = Vec::new();
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                if keep.contains(&(SnippetOwner::Dependent, file_index, snippet_index)) {
                    tokens = tokens.saturating_add(tokenizer.count(&snippet.content, token_model));
                    kept.push(snippet.clone());
                }
            }
            file.snippets = kept;
            file.tokens = tokens;
        }

        for (file_index, file) in context.related_tests.iter_mut().enumerate() {
            let mut tokens: u32 = 0;
            let mut kept = Vec::new();
            for (snippet_index, snippet) in file.snippets.iter().enumerate() {
                if keep.contains(&(SnippetOwner::Test, file_index, snippet_index)) {
                    tokens = tokens.saturating_add(tokenizer.count(&snippet.content, token_model));
                    kept.push(snippet.clone());
                }
            }
            file.snippets = kept;
            file.tokens = tokens;
        }

        context.dependent_files.retain(|f| !f.snippets.is_empty());
        context.related_tests.retain(|f| !f.snippets.is_empty());
    }

    let allowed_paths: std::collections::HashSet<&str> = context
        .changed_files
        .iter()
        .map(|f| f.path.as_str())
        .chain(context.dependent_files.iter().map(|f| f.path.as_str()))
        .chain(context.related_tests.iter().map(|f| f.path.as_str()))
        .collect();

    context
        .dependent_symbols
        .retain(|sym| allowed_paths.contains(sym.file_path.as_str()));

    context.total_tokens = context
        .changed_files
        .iter()
        .chain(context.dependent_files.iter())
        .chain(context.related_tests.iter())
        .map(|f| f.tokens)
        .sum();
}

/// Check if git is available on the system
fn check_git_available() -> Result<()> {
    use std::process::Command;

    let output = Command::new("git")
        .arg("--version")
        .output()
        .context("Git is not installed or not found in PATH. Please install git and ensure it's available in your PATH.")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git is installed but returned an error. Please check your git installation."
        );
    }

    Ok(())
}

/// Recalculate repository metadata after filtering files
/// This ensures metadata (total_files, total_lines, total_tokens, languages)
/// accurately reflects the current set of files
fn recalculate_metadata(repo: &mut infiniloom_engine::types::Repository) {
    use infiniloom_engine::types::{LanguageStats, TokenCounts};
    use std::collections::HashMap;

    // Recalculate total files
    repo.metadata.total_files = repo.files.len() as u32;

    // Recalculate total lines
    repo.metadata.total_lines = repo
        .files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| f.size_bytes / 40) // Estimate ~40 chars/line
        })
        .sum();

    // Recalculate total tokens
    repo.metadata.total_tokens = TokenCounts {
        o200k: repo.files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: repo.files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: repo.files.iter().map(|f| f.token_count.claude).sum(),
        gemini: repo.files.iter().map(|f| f.token_count.gemini).sum(),
        llama: repo.files.iter().map(|f| f.token_count.llama).sum(),
        mistral: repo.files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: repo.files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: repo.files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: repo.files.iter().map(|f| f.token_count.cohere).sum(),
        grok: repo.files.iter().map(|f| f.token_count.grok).sum(),
    };

    // Recalculate language statistics
    let mut language_counts: HashMap<String, u32> = HashMap::new();
    let mut language_lines: HashMap<String, u64> = HashMap::new();

    for file in &repo.files {
        if let Some(ref lang) = file.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
            let lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| file.size_bytes / 40);
            *language_lines.entry(lang.clone()).or_insert(0) += lines;
        }
    }

    let total_files = repo.metadata.total_files;
    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let lines = language_lines.get(&lang).copied().unwrap_or(0);
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    // Sort by file count descending so primary language is first
    languages.sort_by(|a, b| b.files.cmp(&a.files));

    repo.metadata.languages = languages;

    // Regenerate directory structure from filtered files
    let mut paths: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    paths.sort();
    repo.metadata.directory_structure = Some(paths.join("\n"));
}

fn update_repo_cache(
    cache: &mut infiniloom_engine::RepoCache,
    repo: &infiniloom_engine::Repository,
    symbols_extracted: bool,
) {
    use infiniloom_engine::incremental::hash_content;

    for file in &repo.files {
        let mtime = std::fs::metadata(&file.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let content_hash = file
            .content
            .as_ref()
            .map(|c| hash_content(c.as_bytes()))
            .unwrap_or(0);

        let cached = cache.get(&file.relative_path);
        let changed = cached.map_or(true, |_| {
            if content_hash != 0 {
                cache.needs_rescan_with_hash(
                    &file.relative_path,
                    mtime,
                    file.size_bytes,
                    content_hash,
                )
            } else {
                cache.needs_rescan(&file.relative_path, mtime, file.size_bytes)
            }
        });

        let symbols_extracted_for_file = if symbols_extracted {
            true
        } else if !changed {
            cached.map(|c| c.symbols_extracted).unwrap_or(false)
        } else {
            false
        };

        cache.update_file(infiniloom_engine::CachedFile {
            path: file.relative_path.clone(),
            mtime,
            size: file.size_bytes,
            hash: content_hash,
            tokens: infiniloom_engine::AccurateTokenCounts {
                o200k: file.token_count.o200k,
                cl100k: file.token_count.cl100k,
                claude: file.token_count.claude,
                gemini: file.token_count.gemini,
                llama: file.token_count.llama,
                mistral: file.token_count.mistral,
                deepseek: file.token_count.deepseek,
                qwen: file.token_count.qwen,
                cohere: file.token_count.cohere,
                grok: file.token_count.grok,
            },
            symbols: file
                .symbols
                .iter()
                .map(infiniloom_engine::CachedSymbol::from)
                .collect(),
            symbols_extracted: symbols_extracted_for_file,
            language: file.language.clone(),
            lines: file
                .content
                .as_ref()
                .map(|c| c.lines().count())
                .unwrap_or(0),
        });
    }

    let current_files: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    for deleted in cache.find_deleted_files(&current_files) {
        cache.remove_file(&deleted);
    }

    cache.recalculate_totals();
}

/// Human-readable time ago
fn chrono_humanize(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        format!("{} seconds ago", diff)
    } else if diff < 3600 {
        format!("{} minutes ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

/// Calculate priority score for a file based on its path
/// Higher scores = more important (entry points, core modules)
/// Lower scores = less important (tests, utilities)
fn file_priority_score(path: &str) -> f64 {
    let path_lower = path.to_lowercase();

    // Entry points (highest priority: 100)
    if path_lower.ends_with("main.rs")
        || path_lower.ends_with("main.py")
        || path_lower.ends_with("__main__.py")
        || path_lower.ends_with("index.ts")
        || path_lower.ends_with("index.js")
        || path_lower.ends_with("app.ts")
        || path_lower.ends_with("app.js")
        || path_lower.ends_with("main.go")
        || path_lower.ends_with("main.java")
    {
        return 100.0;
    }

    // Configuration files (high priority: 90)
    if path_lower.ends_with("cargo.toml")
        || path_lower.ends_with("package.json")
        || path_lower.ends_with("pyproject.toml")
        || path_lower.ends_with("go.mod")
        || path_lower.ends_with("pom.xml")
        || path_lower.ends_with("build.gradle")
    {
        return 90.0;
    }

    // Core library modules (high priority: 80)
    if path_lower.contains("/lib/")
        || path_lower.contains("/core/")
        || path_lower.contains("/src/lib")
        || path_lower.ends_with("lib.rs")
        || path_lower.ends_with("mod.rs")
    {
        return 80.0;
    }

    // API/handlers (high priority: 75)
    if path_lower.contains("/api/")
        || path_lower.contains("/handlers/")
        || path_lower.contains("/routes/")
        || path_lower.contains("/controllers/")
        || path_lower.contains("/endpoints/")
    {
        return 75.0;
    }

    // Source code (medium-high priority: 60)
    if path_lower.starts_with("src/") || path_lower.contains("/src/") {
        return 60.0;
    }

    // Tests (low priority: 20)
    if path_lower.contains("/test")
        || path_lower.contains("test_")
        || path_lower.contains("_test.")
        || path_lower.contains(".test.")
        || path_lower.contains(".spec.")
        || path_lower.ends_with("_test.rs")
        || path_lower.ends_with("_test.py")
        || path_lower.ends_with("_test.go")
    {
        return 20.0;
    }

    // Utilities/helpers (low priority: 30)
    if path_lower.contains("/utils/")
        || path_lower.contains("/helpers/")
        || path_lower.contains("/util/")
        || path_lower.contains("/common/")
        || path_lower.contains("/shared/")
    {
        return 30.0;
    }

    // Examples/docs (lowest priority: 10)
    if path_lower.contains("/examples/")
        || path_lower.contains("/docs/")
        || path_lower.contains("/example/")
        || path_lower.ends_with(".md")
    {
        return 10.0;
    }

    // Default priority for other source files
    50.0
}

/// Generate a comprehensive summary header for a chunk
fn generate_chunk_summary(chunk: &infiniloom_engine::Chunk) -> String {
    // Extract file names (just the filename, not full path)
    let file_names: Vec<&str> = chunk
        .files
        .iter()
        .take(5) // Limit to first 5 files
        .map(|f| {
            f.path
                .rsplit('/')
                .next()
                .unwrap_or(&f.path)
        })
        .collect();

    let files_str = if file_names.is_empty() {
        "no files".to_owned()
    } else if chunk.files.len() > 5 {
        format!("{}, ... +{} more", file_names.join(", "), chunk.files.len() - 5)
    } else {
        file_names.join(", ")
    };

    let refs_info = if chunk.context.cross_references.is_empty() {
        String::new()
    } else {
        format!(" | Refs: {}", chunk.context.cross_references.len())
    };

    // Get overlap info if available
    let overlap_info = if chunk.context.overlap_content.is_some() {
        " [has overlap from previous]"
    } else {
        ""
    };

    format!(
        "Chunk {}/{}: {} | Files: {} | ~{} tokens{}{}",
        chunk.index + 1,
        chunk.total,
        chunk.focus,
        files_str,
        chunk.tokens,
        refs_info,
        overlap_info
    )
}

/// Split repository into chunks for multi-turn LLM conversations
#[allow(clippy::too_many_arguments)]
fn cmd_chunk(
    path: PathBuf,
    strategy: infiniloom_engine::ChunkStrategy,
    max_tokens: u32,
    overlap: u32,
    model: TokenizerModel,
    format: OutputFormat,
    output: Option<PathBuf>,
    verbose: bool,
    no_chunk_summary: bool,
    priority_first: bool,
) -> Result<()> {
    use infiniloom_engine::Chunker;

    if verbose {
        eprintln!("{}", "Infiniloom - Repository Chunker".cyan().bold());
        eprintln!();
    }

    // Dependency strategy requires symbols for import resolution
    let needs_symbols = matches!(
        strategy,
        infiniloom_engine::ChunkStrategy::Dependency | infiniloom_engine::ChunkStrategy::Symbol
    );

    // Scan repository (need content for chunking)
    let config = scanner::ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024u64,
        skip_symbols: !needs_symbols, // Enable symbols for dependency chunking
    };

    let repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    if verbose {
        eprintln!("  Scanned {} files", repo.files.len());
        if overlap > 0 {
            eprintln!("  Overlap: {} tokens", overlap);
        }
        if priority_first {
            eprintln!("  Priority sorting: enabled (core modules first)");
        }
    }

    // Create chunker with the specified strategy and max_tokens
    // For Fixed strategy, use max_tokens as the chunk size
    let effective_strategy = match strategy {
        infiniloom_engine::ChunkStrategy::Fixed { .. } => {
            infiniloom_engine::ChunkStrategy::Fixed { size: max_tokens }
        },
        other => other,
    };

    let chunker = Chunker::new(effective_strategy, max_tokens)
        .with_model(model)
        .with_overlap(overlap);

    // Generate chunks
    let mut chunks = chunker.chunk(&repo);

    // Apply priority sorting if requested
    if priority_first && chunks.len() > 1 {
        // Calculate average priority for each chunk
        let mut chunk_priorities: Vec<(usize, f64)> = chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                let avg_priority = if chunk.files.is_empty() {
                    0.0
                } else {
                    let total: f64 = chunk
                        .files
                        .iter()
                        .map(|f| file_priority_score(&f.path))
                        .sum();
                    total / chunk.files.len() as f64
                };
                (i, avg_priority)
            })
            .collect();

        // Sort by priority descending (higher priority first)
        chunk_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Reorder chunks
        let original_chunks = std::mem::take(&mut chunks);
        for (idx, (orig_idx, _)) in chunk_priorities.iter().enumerate() {
            let mut chunk = original_chunks[*orig_idx].clone();
            chunk.index = idx;
            chunks.push(chunk);
        }

        // Update total in all chunks
        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.total = total;
        }
    }

    if verbose {
        eprintln!("  Generated {} chunks", chunks.len());
        eprintln!();
    }

    // Format and output chunks
    let formatter = OutputFormatter::by_format_with_model(format, model);
    let map_generator = RepoMapGenerator::builder()
        .token_budget(500)
        .model(model)
        .build();

    #[derive(serde::Serialize)]
    struct ChunkMetadata {
        repository: String,
        total_chunks: u32,
        current_chunk: u32,
        focus: String,
        tokens: u32,
        strategy: String,
        max_tokens: u32,
        model: String,
        context: infiniloom_engine::chunking::ChunkContext,
    }

    #[derive(serde::Serialize)]
    struct ChunkEnvelope<T> {
        chunk_metadata: ChunkMetadata,
        chunk_content: T,
    }

    #[derive(serde::Serialize)]
    struct ChunkSequence<T> {
        repository: String,
        total_chunks: u32,
        chunks: Vec<ChunkEnvelope<T>>,
    }

    let strategy_label = match strategy {
        infiniloom_engine::ChunkStrategy::Fixed { .. } => "fixed",
        infiniloom_engine::ChunkStrategy::File => "file",
        infiniloom_engine::ChunkStrategy::Module => "module",
        infiniloom_engine::ChunkStrategy::Symbol => "symbol",
        infiniloom_engine::ChunkStrategy::Semantic => "semantic",
        infiniloom_engine::ChunkStrategy::Dependency => "dependency",
    };

    let build_metadata = |chunk: &infiniloom_engine::Chunk| ChunkMetadata {
        repository: repo.name.clone(),
        total_chunks: chunk.total as u32,
        current_chunk: (chunk.index + 1) as u32,
        focus: chunk.focus.clone(),
        tokens: chunk.tokens,
        strategy: strategy_label.to_owned(),
        max_tokens,
        model: model.name().to_owned(),
        context: chunk.context.clone(),
    };

    if let Some(output_dir) = output {
        // Create output directory
        std::fs::create_dir_all(&output_dir).with_context(|| {
            format!("Failed to create output directory: {}", output_dir.display())
        })?;

        // Write each chunk to a separate file
        for chunk in &chunks {
            let filename = format!(
                "chunk_{:03}_of_{:03}.{}",
                chunk.index + 1,
                chunk.total,
                format_extension(format)
            );
            let chunk_path = output_dir.join(&filename);

            // Create a mini repository for this chunk
            let chunk_repo = create_chunk_repo(&repo, chunk, model);
            let chunk_map = map_generator.generate(&chunk_repo);
            let chunk_output = formatter.format(&chunk_repo, &chunk_map);

            let full_output = match format {
                OutputFormat::Json => {
                    let content: serde_json::Value = serde_json::from_str(&chunk_output)
                        .with_context(|| "Failed to parse JSON chunk output".to_owned())?;
                    let envelope = ChunkEnvelope {
                        chunk_metadata: build_metadata(chunk),
                        chunk_content: content,
                    };
                    serde_json::to_string_pretty(&envelope)
                        .with_context(|| "Failed to serialize JSON chunk output".to_owned())?
                },
                OutputFormat::Yaml => {
                    let content: serde_yaml::Value = serde_yaml::from_str(&chunk_output)
                        .with_context(|| "Failed to parse YAML chunk output".to_owned())?;
                    let envelope = ChunkEnvelope {
                        chunk_metadata: build_metadata(chunk),
                        chunk_content: content,
                    };
                    serde_yaml::to_string(&envelope)
                        .with_context(|| "Failed to serialize YAML chunk output".to_owned())?
                },
                _ => {
                    if no_chunk_summary {
                        chunk_output
                    } else {
                        format!("<!-- {} -->\n\n{}", generate_chunk_summary(chunk), chunk_output)
                    }
                },
            };

            std::fs::write(&chunk_path, &full_output)
                .with_context(|| format!("Failed to write chunk file: {}", chunk_path.display()))?;

            if verbose {
                eprintln!("  Written: {} ({} tokens)", filename, chunk.tokens);
            }
        }

        eprintln!("{} {} chunks written to {}", "✓".green(), chunks.len(), output_dir.display());
    } else {
        match format {
            OutputFormat::Json => {
                let mut envelopes = Vec::with_capacity(chunks.len());
                for chunk in &chunks {
                    let chunk_repo = create_chunk_repo(&repo, chunk, model);
                    let chunk_map = map_generator.generate(&chunk_repo);
                    let chunk_output = formatter.format(&chunk_repo, &chunk_map);
                    let content: serde_json::Value = serde_json::from_str(&chunk_output)
                        .with_context(|| "Failed to parse JSON chunk output".to_owned())?;
                    envelopes.push(ChunkEnvelope {
                        chunk_metadata: build_metadata(chunk),
                        chunk_content: content,
                    });
                }
                let sequence = ChunkSequence {
                    repository: repo.name.clone(),
                    total_chunks: chunks.len() as u32,
                    chunks: envelopes,
                };
                let output = serde_json::to_string_pretty(&sequence)
                    .with_context(|| "Failed to serialize JSON chunk output".to_owned())?;
                println!("{}", output);
            },
            OutputFormat::Yaml => {
                let mut envelopes = Vec::with_capacity(chunks.len());
                for chunk in &chunks {
                    let chunk_repo = create_chunk_repo(&repo, chunk, model);
                    let chunk_map = map_generator.generate(&chunk_repo);
                    let chunk_output = formatter.format(&chunk_repo, &chunk_map);
                    let content: serde_yaml::Value = serde_yaml::from_str(&chunk_output)
                        .with_context(|| "Failed to parse YAML chunk output".to_owned())?;
                    envelopes.push(ChunkEnvelope {
                        chunk_metadata: build_metadata(chunk),
                        chunk_content: content,
                    });
                }
                let sequence = ChunkSequence {
                    repository: repo.name.clone(),
                    total_chunks: chunks.len() as u32,
                    chunks: envelopes,
                };
                let output = serde_yaml::to_string(&sequence)
                    .with_context(|| "Failed to serialize YAML chunk output".to_owned())?;
                println!("{}", output);
            },
            _ => {
                // Output to stdout with separators
                for chunk in &chunks {
                    if !no_chunk_summary {
                        println!(
                            "<!-- ======================================================== -->"
                        );
                        println!("<!-- {} -->", generate_chunk_summary(chunk));
                        println!(
                            "<!-- ======================================================== -->"
                        );
                        println!();
                    }

                    // Create a mini repository for this chunk
                    let chunk_repo = create_chunk_repo(&repo, chunk, model);
                    let chunk_map = map_generator.generate(&chunk_repo);
                    let chunk_output = formatter.format(&chunk_repo, &chunk_map);

                    println!("{}", chunk_output);
                    println!();
                }
            },
        }
    }

    // Print summary
    if verbose {
        let total_tokens: u32 = chunks.iter().map(|c| c.tokens).sum();
        eprintln!();
        eprintln!("{}", "━".repeat(50).dimmed());
        eprintln!("  {} {} chunks generated", "✓".green(), chunks.len());
        eprintln!("  {} ~{} total tokens ({})", "🔢".dimmed(), total_tokens, model.name());
        eprintln!(
            "  {} ~{} tokens per chunk (avg)",
            "📊".dimmed(),
            total_tokens / chunks.len().max(1) as u32
        );
        eprintln!("{}", "━".repeat(50).dimmed());
    }

    Ok(())
}

/// Get file extension for output format
fn format_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Xml => "xml",
        OutputFormat::Markdown => "md",
        OutputFormat::Json => "json",
        OutputFormat::Yaml => "yaml",
        OutputFormat::Toon => "toon",
        OutputFormat::Plain => "txt",
    }
}

/// Create a mini repository from a chunk for formatting
fn create_chunk_repo(
    repo: &infiniloom_engine::Repository,
    chunk: &infiniloom_engine::Chunk,
    model: TokenizerModel,
) -> infiniloom_engine::Repository {
    use infiniloom_engine::types::{RepoFile, RepoMetadata, TokenCounts};
    use infiniloom_engine::Tokenizer;

    let mut chunk_repo = infiniloom_engine::Repository::new(
        format!("{} (Chunk {}/{})", repo.name, chunk.index + 1, chunk.total),
        repo.path.clone(),
    );

    let tokenizer = Tokenizer::new();
    let mut total_tokens = 0u32;

    // Convert chunk files back to RepoFiles
    for chunk_file in &chunk.files {
        let tokens = tokenizer.count(&chunk_file.content, model);
        total_tokens = total_tokens.saturating_add(tokens);

        // Find the original file to get full metadata
        if let Some(orig_file) = repo
            .files
            .iter()
            .find(|f| f.relative_path == chunk_file.path)
        {
            let mut file = orig_file.clone();
            file.content = Some(chunk_file.content.clone());
            file.token_count.set(model, tokens);
            chunk_repo.files.push(file);
        } else {
            // Fallback: create minimal RepoFile
            let mut token_count = TokenCounts::default();
            token_count.set(model, tokens);
            chunk_repo.files.push(RepoFile {
                path: repo.path.join(&chunk_file.path),
                relative_path: chunk_file.path.clone(),
                language: None,
                size_bytes: chunk_file.content.len() as u64,
                token_count,
                symbols: Vec::new(),
                importance: 0.5,
                content: Some(chunk_file.content.clone()),
            });
        }
    }

    // Set basic metadata
    chunk_repo.metadata = RepoMetadata {
        total_files: chunk_repo.files.len() as u32,
        description: Some(format!("Chunk {}/{}: {}", chunk.index + 1, chunk.total, chunk.focus)),
        total_tokens: {
            let mut counts = TokenCounts::default();
            counts.set(model, total_tokens);
            counts
        },
        ..Default::default()
    };

    chunk_repo
}

fn read_instruction_file(instruction_file: &Option<PathBuf>) -> Result<Option<String>> {
    let path = match instruction_file {
        Some(path) => path,
        None => return Ok(None),
    };
    let instructions = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read instruction file: {}", path.display()))?;
    Ok(Some(instructions))
}

fn budget_token_model_for(model: TokenizerModel) -> TokenModel {
    use infiniloom_engine::tokenizer::TokenModel as BudgetTokenModel;

    // All OpenAI o200k_base models map to Gpt4o, cl100k_base to Gpt4
    match model {
        // Anthropic
        TokenizerModel::Claude => BudgetTokenModel::Claude,
        // OpenAI o200k_base models (all share same encoding)
        TokenizerModel::Gpt52
        | TokenizerModel::Gpt52Pro
        | TokenizerModel::Gpt51
        | TokenizerModel::Gpt51Mini
        | TokenizerModel::Gpt51Codex
        | TokenizerModel::Gpt5
        | TokenizerModel::Gpt5Mini
        | TokenizerModel::Gpt5Nano
        | TokenizerModel::O4Mini
        | TokenizerModel::O3
        | TokenizerModel::O3Mini
        | TokenizerModel::O1
        | TokenizerModel::O1Mini
        | TokenizerModel::O1Preview
        | TokenizerModel::Gpt4o
        | TokenizerModel::Gpt4oMini => BudgetTokenModel::Gpt4o,
        // OpenAI cl100k_base models (legacy)
        TokenizerModel::Gpt4 | TokenizerModel::Gpt35Turbo => BudgetTokenModel::Gpt4,
        // Other vendors
        TokenizerModel::Gemini => BudgetTokenModel::Gemini,
        TokenizerModel::Llama | TokenizerModel::CodeLlama => BudgetTokenModel::Llama,
        TokenizerModel::Mistral => BudgetTokenModel::Mistral,
        TokenizerModel::DeepSeek => BudgetTokenModel::DeepSeek,
        TokenizerModel::Qwen => BudgetTokenModel::Qwen,
        TokenizerModel::Cohere => BudgetTokenModel::Cohere,
        TokenizerModel::Grok => BudgetTokenModel::Grok,
    }
}

fn enforce_budget(
    repo: &mut infiniloom_engine::Repository,
    max_tokens: u32,
    model: TokenizerModel,
) -> Option<infiniloom_engine::budget::EnforcementResult> {
    if max_tokens == 0 {
        return None;
    }

    use infiniloom_engine::budget::{BudgetConfig, BudgetEnforcer, TruncationStrategy};

    let config = BudgetConfig {
        budget: max_tokens,
        model: budget_token_model_for(model),
        strategy: TruncationStrategy::Line,
        overhead_reserve: 2000, // Reserve for headers, repo map, etc.
    };
    let enforcer = BudgetEnforcer::new(config);
    let result = enforcer.enforce(repo);

    // Recompute metadata after budget enforcement
    recalculate_metadata(repo);

    Some(result)
}

fn format_repo_map(map: &infiniloom_engine::repomap::RepoMap, budget: u32) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    let _ = writeln!(
        &mut output,
        "Repository Map (budget: {}, estimated tokens: {})",
        budget, map.token_count
    );
    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Summary");
    let _ = writeln!(&mut output, "-------");
    output.push_str(&map.summary);
    output.push('\n');

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Key Symbols");
    let _ = writeln!(&mut output, "-----------");
    if map.key_symbols.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for sym in &map.key_symbols {
            let _ = writeln!(
                &mut output,
                "{:>2}. {} {} - {}:{} (refs: {}, importance: {:.2})",
                sym.rank, sym.kind, sym.name, sym.file, sym.line, sym.references, sym.importance
            );
            if let Some(ref summary) = sym.summary {
                let _ = writeln!(&mut output, "    summary: {}", summary);
            }
            if let Some(ref sig) = sym.signature {
                if let Some(first_line) = sig.lines().next() {
                    let _ = writeln!(&mut output, "    signature: {}", first_line.trim());
                }
            }
        }
    }

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Module Graph");
    let _ = writeln!(&mut output, "------------");
    if map.module_graph.nodes.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for node in &map.module_graph.nodes {
            let _ = writeln!(
                &mut output,
                "- {} (files: {}, tokens: {})",
                node.name, node.files, node.tokens
            );
        }
        if !map.module_graph.edges.is_empty() {
            let _ = writeln!(&mut output, "Dependencies:");
            for edge in &map.module_graph.edges {
                let _ = writeln!(
                    &mut output,
                    "  {} -> {} (weight: {})",
                    edge.from, edge.to, edge.weight
                );
            }
        }
    }

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "File Index");
    let _ = writeln!(&mut output, "----------");
    if map.file_index.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for entry in &map.file_index {
            let _ = writeln!(
                &mut output,
                "- {} (tokens: {}, importance: {})",
                entry.path, entry.tokens, entry.importance
            );
        }
    }

    output
}

#[derive(serde::Serialize)]
struct TokenTreeEntry {
    path: String,
    tokens: u32,
}

#[derive(serde::Serialize)]
struct SecurityIssueEntry {
    file: String,
    line: u32,
    kind: String,
    severity: String,
}

fn token_tree_entries(
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
) -> Vec<TokenTreeEntry> {
    repo.files
        .iter()
        .map(|file| TokenTreeEntry {
            path: file.relative_path.clone(),
            tokens: file.token_count.get(model),
        })
        .collect()
}

fn security_issue_entries(
    issues: &[infiniloom_engine::security::SecretFinding],
) -> Vec<SecurityIssueEntry> {
    issues
        .iter()
        .map(|issue| SecurityIssueEntry {
            file: issue.file.clone(),
            line: issue.line,
            kind: issue.kind.name().to_owned(),
            severity: format!("{:?}", issue.severity),
        })
        .collect()
}

fn escape_xml_text(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Escape a string for use in XML attribute values (same as text escaping)
fn escape_xml_attr(input: &str) -> String {
    escape_xml_text(input)
}

fn escape_yaml_string(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn append_yaml_block(output: &mut String, key: &str, value: &str) {
    output.push_str(&format!("\n{}: |\n", key));
    for line in value.lines() {
        output.push_str(&format!("  {}\n", line));
    }
}

fn append_git_context_markdown(
    output: &mut String,
    history: &infiniloom_engine::types::GitHistory,
) {
    if history.commits.is_empty() && history.changed_files.is_empty() {
        return;
    }

    output.push_str("\n\n## Git Context\n\n");

    if !history.commits.is_empty() {
        output.push_str(
            "| Commit | Date | Author | Message |\n|--------|------|--------|---------|\n",
        );
        for commit in &history.commits {
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                commit.short_hash, commit.date, commit.author, commit.message
            ));
        }
        output.push('\n');
    }

    if !history.changed_files.is_empty() {
        output.push_str("**Uncommitted changes:**\n");
        for file in &history.changed_files {
            output.push_str(&format!("- `{}` ({})\n", file.path, file.status));
            if let Some(ref diff) = file.diff_content {
                output.push_str("```diff\n");
                output.push_str(diff);
                if !diff.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str("```\n");
            }
        }
    }
}

fn append_git_context_plain(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if history.commits.is_empty() && history.changed_files.is_empty() {
        return;
    }

    output.push_str("\n\nGIT CONTEXT\n");
    output.push_str("-----------\n");

    if !history.commits.is_empty() {
        output.push_str("Commits:\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "- {} {} {}: {}\n",
                commit.short_hash, commit.date, commit.author, commit.message
            ));
        }
    }

    if !history.changed_files.is_empty() {
        output.push_str("Uncommitted changes:\n");
        for file in &history.changed_files {
            output.push_str(&format!("- {} {}\n", file.status, file.path));
            if let Some(ref diff) = file.diff_content {
                output.push_str("  diff:\n");
                for line in diff.lines() {
                    output.push_str(&format!("    {}\n", line));
                }
            }
        }
    }
}

fn append_git_context_toon(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if history.commits.is_empty() && history.changed_files.is_empty() {
        return;
    }

    output.push_str("\n\ngit:\n");

    if !history.commits.is_empty() {
        output.push_str(&format!(
            "  commits[{}]{{hash,date,author,message}}:\n",
            history.commits.len()
        ));
        for commit in &history.commits {
            output.push_str(&format!(
                "    {},{},{},{}\n",
                commit.short_hash, commit.date, commit.author, commit.message
            ));
        }
    }

    if !history.changed_files.is_empty() {
        output.push_str(&format!("  changes[{}]{{status,path}}:\n", history.changed_files.len()));
        for file in &history.changed_files {
            output.push_str(&format!("    {},{}\n", file.status, file.path));
            if let Some(ref diff) = file.diff_content {
                output.push_str("    D{\n");
                output.push_str(diff);
                if !diff.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str("    }D\n");
            }
        }
    }
}

fn append_git_context_yaml(output: &mut String, history: &infiniloom_engine::types::GitHistory) {
    if history.commits.is_empty() && history.changed_files.is_empty() {
        return;
    }

    output.push_str("\ngit:\n");

    if !history.commits.is_empty() {
        output.push_str("  commits:\n");
        for commit in &history.commits {
            output.push_str(&format!(
                "    - hash: {}\n      short_hash: {}\n      author: {}\n      date: {}\n      message: {}\n",
                escape_yaml_string(&commit.hash),
                escape_yaml_string(&commit.short_hash),
                escape_yaml_string(&commit.author),
                escape_yaml_string(&commit.date),
                escape_yaml_string(&commit.message)
            ));
        }
    }

    if !history.changed_files.is_empty() {
        output.push_str("  changes:\n");
        for file in &history.changed_files {
            output.push_str(&format!(
                "    - path: {}\n      status: {}\n",
                escape_yaml_string(&file.path),
                escape_yaml_string(&file.status)
            ));
            if let Some(ref diff) = file.diff_content {
                output.push_str("      diff: |\n");
                for line in diff.lines() {
                    output.push_str(&format!("        {}\n", line));
                }
            }
        }
    }
}

fn apply_pack_extras(
    output_text: String,
    format: OutputFormat,
    repo: &infiniloom_engine::Repository,
    model: TokenizerModel,
    header_text: Option<&str>,
    instructions: Option<&str>,
    token_tree: bool,
    security_issues: Option<&[infiniloom_engine::security::SecretFinding]>,
    include_git_context: bool,
) -> Result<String> {
    let token_tree_entries = if token_tree {
        Some(token_tree_entries(repo, model))
    } else {
        None
    };
    let security_entries = security_issues.map(security_issue_entries);
    let git_history = if include_git_context {
        repo.metadata.git_history.as_ref()
    } else {
        None
    };

    match format {
        OutputFormat::Json => {
            let mut root: serde_json::Value =
                serde_json::from_str(&output_text).context("Failed to parse JSON output")?;
            let obj = root
                .as_object_mut()
                .context("JSON output is not an object")?;

            if let Some(header) = header_text {
                obj.insert("header_text".to_owned(), serde_json::Value::String(header.to_owned()));
            }
            if let Some(instructions) = instructions {
                obj.insert(
                    "instructions".to_owned(),
                    serde_json::Value::String(instructions.to_owned()),
                );
            }
            if let Some(entries) = token_tree_entries {
                obj.insert(
                    "token_tree".to_owned(),
                    serde_json::json!({
                        "model": model.name(),
                        "files": entries,
                    }),
                );
            }
            if let Some(entries) = security_entries {
                obj.insert(
                    "security_scan".to_owned(),
                    serde_json::json!({
                        "issues_found": entries.len(),
                        "issues": entries,
                    }),
                );
            }

            serde_json::to_string_pretty(&root)
                .context("Failed to serialize JSON output with extras")
        },
        OutputFormat::Yaml => {
            let mut output = output_text;
            if !output.ends_with('\n') {
                output.push('\n');
            }

            if let Some(header) = header_text {
                append_yaml_block(&mut output, "header_text", header);
            }
            if let Some(instructions) = instructions {
                append_yaml_block(&mut output, "instructions", instructions);
            }
            if let Some(entries) = token_tree_entries {
                output.push_str("\ntoken_tree:\n");
                output.push_str(&format!("  model: {}\n", escape_yaml_string(model.name())));
                output.push_str("  files:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - path: {}\n      tokens: {}\n",
                        escape_yaml_string(&entry.path),
                        entry.tokens
                    ));
                }
            }
            if let Some(entries) = security_entries {
                output.push_str("\nsecurity_scan:\n");
                output.push_str(&format!("  issues_found: {}\n", entries.len()));
                output.push_str("  issues:\n");
                for entry in entries {
                    output.push_str(&format!(
                        "    - file: {}\n      line: {}\n      kind: {}\n      severity: {}\n",
                        escape_yaml_string(&entry.file),
                        entry.line,
                        escape_yaml_string(&entry.kind),
                        escape_yaml_string(&entry.severity)
                    ));
                }
            }
            if let Some(history) = git_history {
                append_git_context_yaml(&mut output, history);
            }

            Ok(output)
        },
        OutputFormat::Xml => {
            let mut extras = String::new();
            if header_text.is_some()
                || instructions.is_some()
                || token_tree_entries.is_some()
                || security_entries.is_some()
            {
                extras.push_str("  <extras>\n");
                if let Some(header) = header_text {
                    extras.push_str(&format!(
                        "    <header_text>{}</header_text>\n",
                        escape_xml_text(header)
                    ));
                }
                if let Some(instructions) = instructions {
                    extras.push_str(&format!(
                        "    <instructions>{}</instructions>\n",
                        escape_xml_text(instructions)
                    ));
                }
                if let Some(entries) = token_tree_entries {
                    extras.push_str(&format!(
                        "    <token_tree model=\"{}\">\n",
                        escape_xml_text(model.name())
                    ));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <file path=\"{}\" tokens=\"{}\"/>\n",
                            escape_xml_text(&entry.path),
                            entry.tokens
                        ));
                    }
                    extras.push_str("    </token_tree>\n");
                }
                if let Some(entries) = security_entries {
                    extras.push_str(&format!("    <security_scan issues=\"{}\">\n", entries.len()));
                    for entry in entries {
                        extras.push_str(&format!(
                            "      <issue file=\"{}\" line=\"{}\" kind=\"{}\" severity=\"{}\"/>\n",
                            escape_xml_text(&entry.file),
                            entry.line,
                            escape_xml_text(&entry.kind),
                            escape_xml_text(&entry.severity)
                        ));
                    }
                    extras.push_str("    </security_scan>\n");
                }
                extras.push_str("  </extras>\n");
            }

            if extras.is_empty() {
                return Ok(output_text);
            }

            if let Some(pos) = output_text.rfind("</repository>") {
                let mut output = String::with_capacity(output_text.len() + extras.len() + 2);
                output.push_str(&output_text[..pos]);
                output.push('\n');
                output.push_str(&extras);
                output.push_str(&output_text[pos..]);
                Ok(output)
            } else {
                Ok(format!("{}\n{}", output_text, extras))
            }
        },
        OutputFormat::Markdown => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_markdown(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\n## Security Scan Results\n\n");
                output.push_str(&format!("Found {} potential security issues.\n\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!(
                    "\n\n## Token Tree\n\n| File | Tokens ({}) |\n|------|--------|\n",
                    model.name()
                ));
                for entry in entries {
                    output.push_str(&format!("| {} | {} |\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\n## Instructions\n\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Plain => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str(header);
                output.push_str("\n\n");
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_plain(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str("\n\nSECURITY SCAN RESULTS\n");
                output.push_str("----------------------\n");
                output.push_str(&format!("Found {} potential security issues.\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!(
                        "- [{}] {} in {} (line {})\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\nTOKEN TREE ({})\n", model.name()));
                output.push_str("----------------------\n");
                for entry in entries {
                    output.push_str(&format!("- {}: {}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\nINSTRUCTIONS\n");
                output.push_str("------------\n");
                output.push_str(instructions);
            }

            Ok(output)
        },
        OutputFormat::Toon => {
            let mut output = String::new();
            if let Some(header) = header_text {
                output.push_str("header_text: |\n");
                for line in header.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
                output.push('\n');
            }
            output.push_str(&output_text);

            if let Some(history) = git_history {
                append_git_context_toon(&mut output, history);
            }
            if let Some(entries) = security_entries {
                output.push_str(&format!(
                    "\n\nsecurity_scan[{}]{{severity,kind,file,line}}:\n",
                    entries.len()
                ));
                for entry in entries {
                    output.push_str(&format!(
                        "  {},{},{},{}\n",
                        entry.severity, entry.kind, entry.file, entry.line
                    ));
                }
            }
            if let Some(entries) = token_tree_entries {
                output.push_str(&format!("\n\ntoken_tree_model: {}\n", model.name()));
                output.push_str(&format!("token_tree[{}]{{path,tokens}}:\n", entries.len()));
                for entry in entries {
                    output.push_str(&format!("  {},{}\n", entry.path, entry.tokens));
                }
            }
            if let Some(instructions) = instructions {
                output.push_str("\n\ninstructions: |\n");
                for line in instructions.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }

            Ok(output)
        },
    }
}
