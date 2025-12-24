//! Infiniloom CLI - Repository context generator for LLMs
//!
//! This CLI tool generates optimized repository context for AI assistants.

// CLI tools legitimately use print macros for user output
#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use infiniloom_engine::{
    output::OutputFormat,
    types::{CompressionLevel, TokenizerModel},
};

mod commands;
mod config;
mod scanner;

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
        format: commands::ConfigFormat,

        /// Project template (pre-configured settings for common project types)
        #[arg(short, long, value_enum, default_value = "generic")]
        template: commands::ConfigTemplate,

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
        } => commands::cmd_pack(
            path,
            format.map(|f| f.into()),
            model.map(|m| m.into()),
            compression.map(|c| c.into()),
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
        } => commands::cmd_scan(
            path,
            model.into(),
            hidden,
            verbose,
            json,
            security_check,
            sample,
            sample_percent,
        ),
        Commands::Map { path, budget, model, output } => {
            commands::cmd_map(path, budget, model.map(|m| m.into()), output)
        },
        Commands::Info { path } => commands::cmd_info(path),
        Commands::Init { path, format, template, output, force } => {
            commands::cmd_init(path, format, template, output, force)
        },
        Commands::Index { path, force, status, verbose, watch } => {
            commands::cmd_index(path, force, status, verbose, watch)
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
        } => commands::cmd_diff(
            path,
            reference,
            staged,
            depth,
            budget,
            format.into(),
            output,
            include_diff,
            model.map(|m| m.into()),
            include_history,
            history_count,
        ),
        Commands::Impact { path, target, symbol, call_graph, json } => {
            commands::cmd_impact(path, target, symbol, call_graph, json)
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
        } => commands::cmd_chunk(
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
