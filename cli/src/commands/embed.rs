//! Embed command handler
//!
//! Generates deterministic, content-addressable code chunks for vector databases.
//! Supports incremental updates via manifest diffing and git-diff-driven file filtering.

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use infiniloom_engine::embedding::{
    generate_graph_export, DiffSummary, EmbedChunk, EmbedChunker, EmbedDiff, EmbedManifest,
    EmbedSettings, ManifestEntry, ModifiedChunk, QuietProgress, RemovedChunk, ResourceLimits,
    StreamingStats, TerminalProgress,
};
use infiniloom_engine::git::GitRepo;

/// Output format for embed command
#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum EmbedOutputFormat {
    /// JSONL envelope format (header, chunks, footer)
    #[default]
    Jsonl,
    /// Single JSON array
    Json,
}

/// Embed command configuration
pub(crate) struct EmbedConfig {
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
    /// Only process files changed since this commit
    pub since: Option<String>,
    /// Use the commit hash stored in the manifest for --since
    pub since_manifest: bool,
    /// Repository namespace for cross-repo identity
    pub repo_namespace: Option<String>,
    /// Repository name override
    pub repo_name: Option<String>,
    /// Generate signature-only chunks alongside code chunks
    pub include_signatures: bool,
    /// Enable hierarchical chunking
    pub enable_hierarchy: bool,
    /// Minimum children for hierarchy summary
    pub hierarchy_min_children: usize,
    /// Enrich chunks with git metadata
    pub git_metadata: bool,
    /// Enable streaming output mode (lower memory usage)
    pub streaming: bool,
    /// Files per batch in streaming mode
    pub batch_size: usize,
    /// Verbose output
    pub verbose: bool,
    /// Quiet mode (suppress non-error output)
    pub quiet: bool,
    /// JSON output (for statistics)
    pub json_stats: bool,
    /// Generate database schema and exit (e.g., "pgvector")
    pub generate_schema: Option<String>,
    /// Embedding vector dimensions for schema generation
    pub embedding_dims: u32,
    /// Use SQLite for manifest storage instead of bincode
    pub sqlite_manifest: bool,
    /// Generate Neptune-compatible graph files
    pub graph_export: bool,
    /// Output directory for graph files
    pub graph_dir: PathBuf,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            output_format: EmbedOutputFormat::Jsonl,
            output_file: None,
            manifest_path: PathBuf::from(".infiniloom-embed.bin"),
            diff_only: false,
            max_tokens: 1000,
            min_tokens: 50,
            context_lines: 5,
            token_model: "claude".to_owned(),
            include_imports: true,
            include_top_level: true,
            security_scan: true,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            include_tests: false,
            since: None,
            since_manifest: false,
            repo_namespace: None,
            repo_name: None,
            include_signatures: false,
            enable_hierarchy: false,
            hierarchy_min_children: 2,
            git_metadata: false,
            streaming: false,
            batch_size: 500,
            verbose: false,
            quiet: false,
            json_stats: false,
            generate_schema: None,
            embedding_dims: infiniloom_engine::embedding::pgvector_schema::DEFAULT_EMBEDDING_DIMS,
            sqlite_manifest: false,
            graph_export: false,
            graph_dir: PathBuf::from("./graph/"),
        }
    }
}

/// Get files changed between a commit and HEAD using git diff.
///
/// Returns (changed_files, deleted_files) as relative paths from the repo root.
/// - changed_files: files that were added, modified, copied, or renamed (need re-chunking)
/// - deleted_files: files that were deleted (chunks should be removed)
fn get_changed_files_since(repo_path: &Path, since: &str) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let git_repo = GitRepo::open(repo_path)
        .map_err(|e| anyhow::anyhow!("Not a git repository (required for --since): {}", e))?;

    // Get changed/added/modified/renamed files
    let changed_files = git_repo.diff_files(since, "HEAD").map_err(|e| {
        anyhow::anyhow!("Failed to get git diff (commit '{}' may not exist): {}", since, e)
    })?;

    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for file in &changed_files {
        match file.status {
            infiniloom_engine::git::FileStatus::Deleted => {
                deleted.push(PathBuf::from(&file.path));
            },
            _ => {
                // Added, Modified, Renamed, Copied, Unknown - all need re-chunking
                modified.push(PathBuf::from(&file.path));
            },
        }
    }

    Ok((modified, deleted))
}

/// Get current HEAD commit hash
fn get_head_commit(repo_path: &Path) -> Result<String> {
    let git_repo =
        GitRepo::open(repo_path).map_err(|e| anyhow::anyhow!("Not a git repository: {}", e))?;
    git_repo
        .current_commit()
        .map_err(|e| anyhow::anyhow!("Failed to get HEAD commit: {}", e))
}

/// Run the embed command
pub(crate) fn cmd_embed(config: EmbedConfig) -> Result<()> {
    // Handle --generate-schema: print schema and exit without running embedding
    if let Some(ref schema_type) = config.generate_schema {
        return generate_schema(schema_type, config.embedding_dims);
    }

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
        repo_namespace: config.repo_namespace.clone(),
        repo_name: config.repo_name.clone(),
        include_signatures: config.include_signatures,
        enable_hierarchy: config.enable_hierarchy,
        hierarchy_min_children: config.hierarchy_min_children,
        git_metadata: config.git_metadata,
        streaming: config.streaming,
        batch_size: config.batch_size,
        ..Default::default()
    };

    // Validate settings
    settings.validate().context("Invalid embed settings")?;

    // Create chunker
    let limits = ResourceLimits::default();
    let mut chunker = EmbedChunker::new(settings.clone(), limits);

    // Create progress reporter
    // verbose: show progress, quiet: suppress all, default: show minimal
    let progress: Box<dyn infiniloom_engine::ProgressReporter> = if config.quiet {
        Box::new(QuietProgress)
    } else if config.verbose {
        Box::new(TerminalProgress::new())
    } else {
        Box::new(QuietProgress) // Default to quiet for clean stdout piping
    };

    // Streaming mode: write chunks as they are generated (JSONL only)
    if config.streaming {
        return cmd_embed_streaming(config, settings, chunker, progress.as_ref(), start);
    }

    // Non-streaming mode: collect all chunks, then output

    // Load existing manifest for diff (needed for both --since-manifest and standard diff)
    // Generate chunks
    let chunks = chunker
        .chunk_repository(&config.path, progress.as_ref())
        .context("Failed to generate chunks")?;

    let elapsed = start.elapsed();

    // Determine manifest path based on storage backend
    #[cfg(feature = "sqlite-manifest")]
    let use_sqlite = config.sqlite_manifest;
    #[cfg(not(feature = "sqlite-manifest"))]
    let use_sqlite = {
        if config.sqlite_manifest {
            anyhow::bail!(
                "SQLite manifest requires the 'sqlite-manifest' feature.\n\
                 Rebuild with: cargo build --features sqlite-manifest"
            );
        }
        false
    };

    let manifest_path = if use_sqlite {
        // Use .db extension for SQLite manifests
        let db_name = PathBuf::from(".infiniloom-embed.db");
        if config.manifest_path.is_absolute() {
            config.manifest_path.with_extension("db")
        } else {
            config.path.join(db_name)
        }
    } else if config.manifest_path.is_absolute() {
        config.manifest_path.clone()
    } else {
        config.path.join(&config.manifest_path)
    };

    // Branch: SQLite manifest or bincode manifest
    #[cfg(feature = "sqlite-manifest")]
    if use_sqlite {
        return cmd_embed_sqlite(config, settings, chunks, elapsed, &manifest_path);
    }

    let existing_manifest =
        EmbedManifest::load_if_exists(&manifest_path).context("Failed to load manifest")?;

    // Resolve the --since commit ref
    let since_ref = resolve_since_ref(&config, existing_manifest.as_ref());

    // Generate chunks (full or filtered by git diff)
    let (chunks, deleted_files) = if let Some(ref since) = since_ref {
        if !config.quiet {
            eprintln!(
                "  {} Incremental mode: processing files changed since {}",
                "->".cyan(),
                since
            );
        }

        match get_changed_files_since(&config.path, since) {
            Ok((changed, deleted)) => {
                if !config.quiet {
                    eprintln!(
                        "  {} {} changed file(s), {} deleted file(s)",
                        "->".cyan(),
                        changed.len(),
                        deleted.len()
                    );
                }

                if changed.is_empty() && deleted.is_empty() {
                    if !config.quiet {
                        eprintln!("  {} No changes detected, nothing to do.", "->".cyan());
                    }
                    // Return existing chunks from manifest if available
                    let chunks = Vec::new();
                    let diff = existing_manifest.as_ref().map(|m| m.diff(&chunks));
                    output_and_save(
                        &config,
                        &chunks,
                        diff.as_ref(),
                        &settings,
                        start.elapsed(),
                        &manifest_path,
                        existing_manifest,
                    )?;
                    return Ok(());
                }

                let only_files: HashSet<PathBuf> = changed.into_iter().collect();

                // Use filtered chunking - only process changed files
                let new_chunks = if only_files.is_empty() {
                    Vec::new()
                } else {
                    chunker
                        .chunk_repository_filtered(&config.path, &only_files, progress.as_ref())
                        .unwrap_or_else(|e| {
                            // NoChunksGenerated is expected when changed files have no symbols
                            if matches!(
                                e,
                                infiniloom_engine::embedding::EmbedError::NoChunksGenerated { .. }
                            ) {
                                Vec::new()
                            } else {
                                eprintln!("Warning: chunk generation failed: {}", e);
                                Vec::new()
                            }
                        })
                };

                (new_chunks, deleted)
            },
            Err(e) => {
                // Commit not found or other git error - fall back to full re-index
                eprintln!(
                    "  {} Git diff failed ({}), falling back to full re-index",
                    "Warning:".yellow(),
                    e
                );
                let chunks = chunker
                    .chunk_repository(&config.path, progress.as_ref())
                    .context("Failed to generate chunks")?;
                (chunks, Vec::new())
            },
        }
    } else {
        // Standard full chunking
        let chunks = chunker
            .chunk_repository(&config.path, progress.as_ref())
            .context("Failed to generate chunks")?;
        (chunks, Vec::new())
    };

    let elapsed = start.elapsed();

    // For incremental mode with --since: merge new chunks with unchanged chunks from manifest
    let (final_chunks, diff) = if since_ref.is_some() {
        if let Some(ref manifest) = existing_manifest {
            // Build a set of files that were changed or deleted
            let changed_files: HashSet<&str> =
                chunks.iter().map(|c| c.source.file.as_str()).collect();
            let deleted_file_strs: HashSet<String> = deleted_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();

            // Reconstruct full chunk list: unchanged from manifest + new from changed files
            // We need to rebuild unchanged chunks from manifest entries, but we don't have
            // the full chunk data in the manifest (only IDs and metadata).
            // Instead, compute the diff against a virtual "current" that includes:
            // - New chunks for changed files
            // - Removal markers for deleted files
            let diff = compute_incremental_diff(manifest, &chunks, &deleted_file_strs);

            // The "final chunks" for manifest update = new chunks from changed files
            // + chunks from manifest for unchanged files (keep manifest as-is for those)
            // For output: only the new chunks + diff information
            (chunks, Some(diff))
        } else {
            // No existing manifest - treat as fresh run
            let diff = None;
            (chunks, diff)
        }
    } else {
        // Standard diff against manifest
        let diff = existing_manifest.as_ref().map(|m| m.diff(&chunks));
        (chunks, diff)
    };

    // Output chunks or diff
    match config.output_format {
        EmbedOutputFormat::Jsonl => {
            output_jsonl(&config, &final_chunks, diff.as_ref(), &settings, elapsed)?;
        },
        EmbedOutputFormat::Json => {
            output_json(&config, &final_chunks, diff.as_ref(), &settings, elapsed)?;
        },
    }

    // Update and save manifest
    let mut manifest = if since_ref.is_some() {
        if let Some(mut m) = existing_manifest {
            // Incremental update: remove deleted file chunks, update changed file chunks
            let deleted_file_strs: HashSet<String> = deleted_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();

            // Remove entries for deleted files
            m.chunks.retain(|key, _| {
                // Location key format: "file::symbol::kind"
                let file_part = key.split("::").next().unwrap_or("");
                !deleted_file_strs.contains(file_part)
            });

            // Remove entries for changed files (will be replaced by new chunks)
            let changed_files: HashSet<&str> = final_chunks
                .iter()
                .map(|c| c.source.file.as_str())
                .collect();
            m.chunks.retain(|key, _| {
                let file_part = key.split("::").next().unwrap_or("");
                !changed_files.contains(file_part)
            });

            // Add new chunks for changed files
            for chunk in &final_chunks {
                let key = EmbedManifest::location_key(
                    &chunk.source.file,
                    &chunk.source.symbol,
                    chunk.kind,
                );
                m.chunks.insert(
                    key,
                    ManifestEntry {
                        chunk_id: chunk.id.clone(),
                        full_hash: chunk.full_hash.clone(),
                        tokens: chunk.tokens,
                        lines: chunk.source.lines,
                    },
                );
            }

            m.settings = settings.clone();
            m
        } else {
            let mut m =
                EmbedManifest::new(config.path.to_string_lossy().to_string(), settings.clone());
            m.update(&final_chunks)
                .context("Failed to update manifest")?;
            m
        }
    } else {
        let mut m = existing_manifest.unwrap_or_else(|| {
            EmbedManifest::new(config.path.to_string_lossy().to_string(), settings.clone())
        });
        m.update(&final_chunks)
            .context("Failed to update manifest")?;
        m
    };

    // Store current HEAD commit in manifest (for --since-manifest on next run)
    if let Ok(head) = get_head_commit(&config.path) {
        manifest.commit_hash = Some(head);
    }

    manifest
        .save(&manifest_path)
        .context("Failed to save manifest")?;

    // Generate graph export if requested
    if config.graph_export {
        write_graph_export(&final_chunks, &config.graph_dir, config.quiet)?;
    }

    // Print statistics if not quiet and (outputting to file or verbose mode)
    if !config.quiet && (config.output_file.is_some() || config.verbose) {
        print_statistics(&final_chunks, diff.as_ref(), elapsed, config.json_stats);
    }

    Ok(())
}

/// Resolve the --since commit ref from CLI args or manifest
fn resolve_since_ref(config: &EmbedConfig, manifest: Option<&EmbedManifest>) -> Option<String> {
    if let Some(ref since) = config.since {
        return Some(since.clone());
    }

    if config.since_manifest {
        if let Some(m) = manifest {
            if let Some(ref commit) = m.commit_hash {
                return Some(commit.clone());
            }
            eprintln!(
                "  {} No commit hash in manifest, falling back to full re-index",
                "Warning:".yellow()
            );
        } else {
            eprintln!("  {} No manifest found, falling back to full re-index", "Warning:".yellow());
        }
    }

    None
}

/// Compute diff for incremental mode (--since)
///
/// Compares new chunks from changed files against the manifest, and marks
/// chunks from deleted files as removed.
fn compute_incremental_diff(
    manifest: &EmbedManifest,
    new_chunks: &[EmbedChunk],
    deleted_files: &HashSet<String>,
) -> EmbedDiff {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    let mut unchanged_count = 0usize;

    // Files that were changed (have new chunks)
    let changed_files: HashSet<&str> = new_chunks.iter().map(|c| c.source.file.as_str()).collect();

    // Check each manifest entry
    for (key, entry) in &manifest.chunks {
        let file_part = key.split("::").next().unwrap_or("");

        if deleted_files.contains(file_part) {
            // File was deleted - mark chunk as removed
            removed.push(RemovedChunk { id: entry.chunk_id.clone(), location_key: key.clone() });
        } else if changed_files.contains(file_part) {
            // File was changed - will be handled below when comparing new chunks
            // (don't count as unchanged)
        } else {
            // File was not changed - chunk is unchanged
            unchanged_count += 1;
        }
    }

    // Compare new chunks against manifest entries for changed files
    for chunk in new_chunks {
        let key = EmbedManifest::location_key(&chunk.source.file, &chunk.source.symbol, chunk.kind);

        if let Some(entry) = manifest.chunks.get(&key) {
            if chunk.id == entry.chunk_id {
                unchanged_count += 1;
            } else {
                modified.push(ModifiedChunk {
                    old_id: entry.chunk_id.clone(),
                    new_id: chunk.id.clone(),
                    chunk: chunk.clone(),
                });
            }
        } else {
            added.push(chunk.clone());
        }
    }

    // Also find manifest entries for changed files that no longer exist in new chunks
    // (e.g., a function was removed from a modified file)
    for (key, entry) in &manifest.chunks {
        let file_part = key.split("::").next().unwrap_or("");
        if changed_files.contains(file_part) && !deleted_files.contains(file_part) {
            // Check if this manifest entry has a corresponding new chunk
            let has_match = new_chunks.iter().any(|c| {
                let new_key = EmbedManifest::location_key(&c.source.file, &c.source.symbol, c.kind);
                new_key == *key
            });
            if !has_match {
                removed
                    .push(RemovedChunk { id: entry.chunk_id.clone(), location_key: key.clone() });
            }
        }
    }

    let total_chunks = new_chunks.len() + unchanged_count;
    let summary = DiffSummary {
        added: added.len(),
        modified: modified.len(),
        removed: removed.len(),
        unchanged: unchanged_count,
        total_chunks,
    };

    EmbedDiff {
        summary,
        added,
        modified,
        removed,
        unchanged: Vec::new(), // We don't track individual unchanged IDs in incremental mode
    }
}

/// Helper to output and save when there are no changes (early return path)
fn output_and_save(
    config: &EmbedConfig,
    chunks: &[EmbedChunk],
    diff: Option<&EmbedDiff>,
    settings: &EmbedSettings,
    elapsed: std::time::Duration,
    manifest_path: &Path,
    existing_manifest: Option<EmbedManifest>,
) -> Result<()> {
    match config.output_format {
        EmbedOutputFormat::Jsonl => {
            output_jsonl(config, chunks, diff, settings, elapsed)?;
        },
        EmbedOutputFormat::Json => {
            output_json(config, chunks, diff, settings, elapsed)?;
        },
    }

    // Re-save manifest with updated HEAD commit
    if let Some(mut manifest) = existing_manifest {
        if let Ok(head) = get_head_commit(&config.path) {
            manifest.commit_hash = Some(head);
        }
        manifest
            .save(manifest_path)
            .context("Failed to save manifest")?;
    }

    Ok(())
}

/// Run embed command with SQLite manifest backend
#[cfg(feature = "sqlite-manifest")]
fn cmd_embed_sqlite(
    config: EmbedConfig,
    settings: EmbedSettings,
    chunks: Vec<EmbedChunk>,
    elapsed: std::time::Duration,
    manifest_path: &std::path::Path,
) -> Result<()> {
    use infiniloom_engine::embedding::SqliteManifest;

    // Open or create SQLite manifest
    let mut sqlite_manifest =
        SqliteManifest::new(manifest_path).context("Failed to open SQLite manifest")?;

    // Initialize if needed (first run)
    sqlite_manifest
        .init(config.path.to_string_lossy().to_string(), &settings)
        .context("Failed to initialize SQLite manifest")?;

    // Compute diff against existing state
    let diff = sqlite_manifest
        .diff(&chunks)
        .context("Failed to compute diff")?;
    let diff_ref = Some(&diff);

    // Output chunks or diff
    match config.output_format {
        EmbedOutputFormat::Jsonl => {
            output_jsonl(&config, &chunks, diff_ref, &settings, elapsed)?;
        },
        EmbedOutputFormat::Json => {
            output_json(&config, &chunks, diff_ref, &settings, elapsed)?;
        },
    }

    // Update manifest with current chunks
    sqlite_manifest
        .save_chunks(&chunks)
        .context("Failed to save chunks to SQLite manifest")?;

    // Print statistics if not quiet and (outputting to file or verbose mode)
    if !config.quiet && (config.output_file.is_some() || config.verbose) {
        print_statistics(&chunks, diff_ref, elapsed, config.json_stats);
    }

    Ok(())
}

/// Run embed command with SQLite manifest backend
#[cfg(feature = "sqlite-manifest")]
fn cmd_embed_sqlite(
    config: EmbedConfig,
    settings: EmbedSettings,
    chunks: Vec<EmbedChunk>,
    elapsed: std::time::Duration,
    manifest_path: &std::path::Path,
) -> Result<()> {
    use infiniloom_engine::embedding::SqliteManifest;

    // Open or create SQLite manifest
    let sqlite_manifest =
        SqliteManifest::new(manifest_path).context("Failed to open SQLite manifest")?;

    // Save settings so subsequent runs can detect config changes
    sqlite_manifest
        .save_settings(&settings)
        .context("Failed to save settings to SQLite manifest")?;

    // Compute diff against existing state
    let diff = sqlite_manifest
        .diff(&chunks)
        .context("Failed to compute diff")?;
    let diff_ref = Some(&diff);

    // Output chunks or diff
    match config.output_format {
        EmbedOutputFormat::Jsonl => {
            output_jsonl(&config, &chunks, diff_ref, &settings, elapsed)?;
        },
        EmbedOutputFormat::Json => {
            output_json(&config, &chunks, diff_ref, &settings, elapsed)?;
        },
    }

    // Update manifest with current chunks
    sqlite_manifest
        .save_chunks(&chunks)
        .context("Failed to save chunks to SQLite manifest")?;

    // Print statistics if not quiet and (outputting to file or verbose mode)
    if !config.quiet && (config.output_file.is_some() || config.verbose) {
        print_statistics(&chunks, diff_ref, elapsed, config.json_stats);
    }

    Ok(())
}

/// Run the embed command in streaming mode.
///
/// Writes header, streams chunk batches directly to the writer via
/// `chunk_repository_streaming`, then writes a footer with totals.
/// Manifest updates are skipped in streaming mode because we never hold
/// all chunks in memory simultaneously.
fn cmd_embed_streaming(
    config: EmbedConfig,
    settings: EmbedSettings,
    chunker: EmbedChunker,
    progress: &dyn infiniloom_engine::ProgressReporter,
    start: Instant,
) -> Result<()> {
    // Streaming only supports JSONL output
    if !matches!(config.output_format, EmbedOutputFormat::Jsonl) {
        anyhow::bail!("Streaming mode only supports JSONL output format. Remove --streaming or use --format jsonl.");
    }

    let mut writer: Box<dyn Write> = if let Some(ref path) = config.output_file {
        Box::new(BufWriter::new(
            std::fs::File::create(path).context("Failed to create output file")?,
        ))
    } else {
        Box::new(BufWriter::new(std::io::stdout()))
    };

    // Write header line
    let header = serde_json::json!({
        "type": "header",
        "streaming": true,
        "batch_size": settings.batch_size,
        "version": infiniloom_engine::embedding::MANIFEST_VERSION,
        "settings": settings,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    writeln!(writer, "{}", serde_json::to_string(&header)?)?;

    // Stream chunks -- the chunker writes {"type":"chunk","data":...} lines directly
    let streaming_stats = chunker
        .chunk_repository_streaming(&config.path, &mut writer, progress)
        .context("Failed to generate chunks in streaming mode")?;

    let elapsed = start.elapsed();

    // Write footer line
    let footer = serde_json::json!({
        "type": "footer",
        "total_chunks": streaming_stats.total_chunks,
        "total_files": streaming_stats.total_files,
        "files_processed": streaming_stats.files_processed,
        "files_skipped": streaming_stats.files_skipped,
        "batches_processed": streaming_stats.batches_processed,
        "elapsed_ms": elapsed.as_millis(),
    });
    writeln!(writer, "{}", serde_json::to_string(&footer)?)?;

    // Flush the writer
    writer.flush()?;

    // Print statistics to stderr if appropriate
    if !config.quiet && (config.output_file.is_some() || config.verbose) {
        print_streaming_statistics(&streaming_stats, elapsed, config.json_stats);
    }

    Ok(())
}

/// Print streaming statistics to stderr
fn print_streaming_statistics(
    stats: &StreamingStats,
    elapsed: std::time::Duration,
    json_output: bool,
) {
    if json_output {
        let json = serde_json::json!({
            "total_chunks": stats.total_chunks,
            "total_files": stats.total_files,
            "files_processed": stats.files_processed,
            "files_skipped": stats.files_skipped,
            "batches_processed": stats.batches_processed,
            "elapsed_ms": elapsed.as_millis(),
            "streaming": true,
        });
        eprintln!("{}", serde_json::to_string(&json).unwrap_or_default());
    } else {
        eprintln!();
        eprintln!("{}", "━".repeat(50).dimmed());
        eprintln!("  {}", "Streaming Embedding Statistics".cyan().bold());
        eprintln!("{}", "━".repeat(50).dimmed());
        eprintln!();
        eprintln!("  Total Chunks:   {}", stats.total_chunks);
        eprintln!("  Total Files:    {}", stats.total_files);
        eprintln!("  Processed:      {}", stats.files_processed);
        eprintln!("  Skipped:        {}", stats.files_skipped);
        eprintln!("  Batches:        {}", stats.batches_processed);
        eprintln!("  Elapsed:        {:?}", elapsed);
        eprintln!();
    }
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

/// Write Neptune-compatible graph files (vertices.jsonl + edges.jsonl)
fn write_graph_export(
    chunks: &[EmbedChunk],
    graph_dir: &std::path::Path,
    quiet: bool,
) -> Result<()> {
    let graph = generate_graph_export(chunks);

    // Create output directory
    std::fs::create_dir_all(graph_dir)
        .with_context(|| format!("Failed to create graph directory: {}", graph_dir.display()))?;

    // Write vertices.jsonl
    let vertices_path = graph_dir.join("vertices.jsonl");
    let mut vfile =
        std::fs::File::create(&vertices_path).context("Failed to create vertices.jsonl")?;
    for vertex in &graph.vertices {
        writeln!(vfile, "{}", serde_json::to_string(vertex)?)?;
    }

    // Write edges.jsonl
    let edges_path = graph_dir.join("edges.jsonl");
    let mut efile = std::fs::File::create(&edges_path).context("Failed to create edges.jsonl")?;
    for edge in &graph.edges {
        writeln!(efile, "{}", serde_json::to_string(edge)?)?;
    }

    if !quiet {
        eprintln!(
            "  Graph export: {} vertices, {} edges -> {}",
            graph.vertices.len(),
            graph.edges.len(),
            graph_dir.display()
        );
    }

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

/// Generate a database schema and print it to stdout.
///
/// Currently supports: "pgvector"
fn generate_schema(schema_type: &str, embedding_dims: u32) -> Result<()> {
    match schema_type.to_lowercase().as_str() {
        "pgvector" | "pg_vector" | "pg" => {
            use infiniloom_engine::embedding::pgvector_schema;

            if embedding_dims < pgvector_schema::MIN_EMBEDDING_DIMS
                || embedding_dims > pgvector_schema::MAX_EMBEDDING_DIMS
            {
                anyhow::bail!(
                    "Embedding dimensions must be between {} and {}, got {}",
                    pgvector_schema::MIN_EMBEDDING_DIMS,
                    pgvector_schema::MAX_EMBEDDING_DIMS,
                    embedding_dims,
                );
            }

            let schema = pgvector_schema::generate_pgvector_schema(embedding_dims);
            print!("{schema}");
            Ok(())
        },
        other => {
            anyhow::bail!("Unsupported schema type: '{}'. Supported types: pgvector", other);
        },
    }
}
