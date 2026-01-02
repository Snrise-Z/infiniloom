//! Embed command handler
//!
//! Generates deterministic, content-addressable code chunks for vector databases.
//! Supports incremental updates via manifest diffing.

use anyhow::{Context, Result};
use colored::Colorize;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use infiniloom_engine::embedding::{
    EmbedChunk, EmbedChunker, EmbedDiff, EmbedManifest, EmbedSettings, QuietProgress,
    ResourceLimits, TerminalProgress,
};

/// Output format for embed command
#[derive(Debug, Clone, Copy, Default)]
pub enum EmbedOutputFormat {
    /// JSONL envelope format (header, chunks, footer)
    #[default]
    Jsonl,
    /// Single JSON array
    Json,
}

/// Embed command configuration
pub struct EmbedConfig {
    /// Path to repository
    pub path: PathBuf,
    /// Output format
    pub output_format: EmbedOutputFormat,
    /// Output file (None = stdout)
    pub output_file: Option<PathBuf>,
    /// Manifest file path
    pub manifest_path: PathBuf,
    /// Only output diff (changed chunks)
    pub diff_only: bool,
    /// Token limit per chunk (default: 1000, matching engine defaults)
    pub max_tokens: u32,
    /// Minimum tokens for a chunk
    pub min_tokens: u32,
    /// Context lines around symbols
    pub context_lines: u32,
    /// Token counting model
    pub token_model: String,
    /// Include imports
    pub include_imports: bool,
    /// Include top-level code
    pub include_top_level: bool,
    /// Enable secret scanning
    pub security_scan: bool,
    /// Include patterns
    pub include_patterns: Vec<String>,
    /// Exclude patterns
    pub exclude_patterns: Vec<String>,
    /// Include test files
    pub include_tests: bool,
    /// Enable hierarchical chunking
    pub enable_hierarchy: bool,
    /// Minimum children for hierarchy summary
    pub hierarchy_min_children: usize,
    /// Verbose output
    pub verbose: bool,
    /// Quiet mode (suppress non-error output)
    pub quiet: bool,
    /// JSON output (for statistics)
    pub json_stats: bool,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            output_format: EmbedOutputFormat::Jsonl,
            output_file: None,
            manifest_path: PathBuf::from(".infiniloom-embed.bin"),
            diff_only: false,
            // Defaults match EmbedSettings in engine for consistency
            max_tokens: 1000, // Matches EmbedSettings::default().max_tokens
            min_tokens: 50,   // Matches EmbedSettings::default().min_tokens
            context_lines: 5, // Matches EmbedSettings::default().context_lines
            token_model: "claude".to_string(),
            include_imports: true, // Include imports by default for dependency tracking
            include_top_level: true,
            security_scan: true, // Enable security scanning by default for safety
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            include_tests: false,
            enable_hierarchy: false,
            hierarchy_min_children: 2,
            verbose: false,
            quiet: false,
            json_stats: false,
        }
    }
}

/// Run the embed command
pub fn cmd_embed(config: EmbedConfig) -> Result<()> {
    let start = Instant::now();

    // Build settings
    let settings = EmbedSettings {
        max_tokens: config.max_tokens,
        min_tokens: config.min_tokens,
        context_lines: config.context_lines,
        token_model: config.token_model.clone(),
        include_imports: config.include_imports,
        include_top_level: config.include_top_level,
        scan_secrets: config.security_scan,
        fail_on_secrets: false, // Default: scan and redact, don't fail
        redact_secrets: config.security_scan, // Redact if scanning
        include_patterns: config.include_patterns.clone(),
        exclude_patterns: config.exclude_patterns.clone(),
        include_tests: config.include_tests,
        enable_hierarchy: config.enable_hierarchy,
        hierarchy_min_children: config.hierarchy_min_children,
        ..Default::default()
    };

    // Validate settings
    settings.validate().context("Invalid embed settings")?;

    // Create chunker
    let limits = ResourceLimits::default();
    let chunker = EmbedChunker::new(settings.clone(), limits);

    // Create progress reporter
    // verbose: show progress, quiet: suppress all, default: show minimal
    let progress: Box<dyn infiniloom_engine::ProgressReporter> = if config.quiet {
        Box::new(QuietProgress)
    } else if config.verbose {
        Box::new(TerminalProgress::new())
    } else {
        Box::new(QuietProgress) // Default to quiet for clean stdout piping
    };

    // Generate chunks
    let chunks = chunker
        .chunk_repository(&config.path, progress.as_ref())
        .context("Failed to generate chunks")?;

    let elapsed = start.elapsed();

    // Load existing manifest for diff
    let manifest_path = if config.manifest_path.is_absolute() {
        config.manifest_path.clone()
    } else {
        config.path.join(&config.manifest_path)
    };

    let existing_manifest =
        EmbedManifest::load_if_exists(&manifest_path).context("Failed to load manifest")?;

    // Compute diff if we have an existing manifest
    let diff = existing_manifest.as_ref().map(|m| m.diff(&chunks));

    // Output chunks or diff
    match config.output_format {
        EmbedOutputFormat::Jsonl => {
            output_jsonl(&config, &chunks, diff.as_ref(), &settings, elapsed)?;
        },
        EmbedOutputFormat::Json => {
            output_json(&config, &chunks, diff.as_ref(), &settings, elapsed)?;
        },
    }

    // Update and save manifest
    let mut manifest = existing_manifest.unwrap_or_else(|| {
        EmbedManifest::new(config.path.to_string_lossy().to_string(), settings.clone())
    });
    manifest
        .update(&chunks)
        .context("Failed to update manifest")?;
    manifest
        .save(&manifest_path)
        .context("Failed to save manifest")?;

    // Print statistics if not quiet and (outputting to file or verbose mode)
    if !config.quiet && (config.output_file.is_some() || config.verbose) {
        print_statistics(&chunks, diff.as_ref(), elapsed, config.json_stats);
    }

    Ok(())
}

/// Output chunks in JSONL envelope format
fn output_jsonl(
    config: &EmbedConfig,
    chunks: &[EmbedChunk],
    diff: Option<&EmbedDiff>,
    settings: &EmbedSettings,
    elapsed: std::time::Duration,
) -> Result<()> {
    let mut writer: Box<dyn Write> = if let Some(ref path) = config.output_file {
        Box::new(std::fs::File::create(path).context("Failed to create output file")?)
    } else {
        Box::new(std::io::stdout())
    };

    // Header
    let header = serde_json::json!({
        "type": "header",
        "version": infiniloom_engine::embedding::MANIFEST_VERSION,
        "settings": settings,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    writeln!(writer, "{}", serde_json::to_string(&header)?)?;

    // Chunks (or diff)
    if config.diff_only {
        if let Some(d) = diff {
            // Output added chunks
            for chunk in &d.added {
                let chunk_json = serde_json::json!({
                    "type": "chunk",
                    "action": "add",
                    "data": chunk,
                });
                writeln!(writer, "{}", serde_json::to_string(&chunk_json)?)?;
            }
            // Output modified chunks
            for modified in &d.modified {
                let chunk_json = serde_json::json!({
                    "type": "chunk",
                    "action": "modify",
                    "old_id": modified.old_id,
                    "data": modified.chunk,
                });
                writeln!(writer, "{}", serde_json::to_string(&chunk_json)?)?;
            }
            // Output removed chunk IDs
            for removed in &d.removed {
                let chunk_json = serde_json::json!({
                    "type": "chunk",
                    "action": "remove",
                    "id": removed.id,
                    "location_key": removed.location_key,
                });
                writeln!(writer, "{}", serde_json::to_string(&chunk_json)?)?;
            }
        }
    } else {
        for chunk in chunks {
            let chunk_json = serde_json::json!({
                "type": "chunk",
                "data": chunk,
            });
            writeln!(writer, "{}", serde_json::to_string(&chunk_json)?)?;
        }
    }

    // Footer/Summary
    let summary = if let Some(d) = diff {
        serde_json::json!({
            "type": "summary",
            "total_chunks": chunks.len(),
            "diff": {
                "added": d.summary.added,
                "modified": d.summary.modified,
                "removed": d.summary.removed,
                "unchanged": d.summary.unchanged,
            },
            "elapsed_ms": elapsed.as_millis(),
        })
    } else {
        serde_json::json!({
            "type": "summary",
            "total_chunks": chunks.len(),
            "elapsed_ms": elapsed.as_millis(),
        })
    };
    writeln!(writer, "{}", serde_json::to_string(&summary)?)?;

    Ok(())
}

/// Output chunks as a single JSON object
fn output_json(
    config: &EmbedConfig,
    chunks: &[EmbedChunk],
    diff: Option<&EmbedDiff>,
    settings: &EmbedSettings,
    elapsed: std::time::Duration,
) -> Result<()> {
    let mut writer: Box<dyn Write> = if let Some(ref path) = config.output_file {
        Box::new(std::fs::File::create(path).context("Failed to create output file")?)
    } else {
        Box::new(std::io::stdout())
    };

    let output = if config.diff_only {
        if let Some(d) = diff {
            serde_json::json!({
                "version": infiniloom_engine::embedding::MANIFEST_VERSION,
                "settings": settings,
                "diff": {
                    "added": d.added,
                    "modified": d.modified,
                    "removed": d.removed,
                    "unchanged_count": d.unchanged.len(),
                },
                "summary": d.summary,
                "elapsed_ms": elapsed.as_millis(),
            })
        } else {
            serde_json::json!({
                "version": infiniloom_engine::embedding::MANIFEST_VERSION,
                "settings": settings,
                "chunks": chunks,
                "elapsed_ms": elapsed.as_millis(),
            })
        }
    } else {
        serde_json::json!({
            "version": infiniloom_engine::embedding::MANIFEST_VERSION,
            "settings": settings,
            "chunks": chunks,
            "summary": diff.map(|d| &d.summary),
            "elapsed_ms": elapsed.as_millis(),
        })
    };

    writeln!(writer, "{}", serde_json::to_string_pretty(&output)?)?;

    Ok(())
}

/// Print statistics to stderr
fn print_statistics(
    chunks: &[EmbedChunk],
    diff: Option<&EmbedDiff>,
    elapsed: std::time::Duration,
    json_output: bool,
) {
    if json_output {
        let stats = if let Some(d) = diff {
            serde_json::json!({
                "total_chunks": chunks.len(),
                "total_tokens": chunks.iter().map(|c| c.tokens as u64).sum::<u64>(),
                "diff": {
                    "added": d.summary.added,
                    "modified": d.summary.modified,
                    "removed": d.summary.removed,
                    "unchanged": d.summary.unchanged,
                },
                "elapsed_ms": elapsed.as_millis(),
            })
        } else {
            serde_json::json!({
                "total_chunks": chunks.len(),
                "total_tokens": chunks.iter().map(|c| c.tokens as u64).sum::<u64>(),
                "elapsed_ms": elapsed.as_millis(),
            })
        };
        eprintln!("{}", serde_json::to_string(&stats).unwrap_or_default());
    } else {
        eprintln!();
        eprintln!("{}", "━".repeat(50).dimmed());
        eprintln!("  {}", "Embedding Statistics".cyan().bold());
        eprintln!("{}", "━".repeat(50).dimmed());
        eprintln!();
        eprintln!("  Total Chunks:  {}", chunks.len());
        eprintln!("  Total Tokens:  {}", chunks.iter().map(|c| c.tokens as u64).sum::<u64>());

        if let Some(d) = diff {
            eprintln!();
            eprintln!("  {}:", "Changes".yellow());
            eprintln!("    Added:     {}", d.summary.added);
            eprintln!("    Modified:  {}", d.summary.modified);
            eprintln!("    Removed:   {}", d.summary.removed);
            eprintln!("    Unchanged: {}", d.summary.unchanged);
        }

        eprintln!();
        eprintln!("  Elapsed:       {:?}", elapsed);
        eprintln!();
    }
}
