//! Chunk command handler
//!
//! Splits repositories into chunks for multi-turn LLM conversations.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use infiniloom_engine::output::OutputFormat;
use infiniloom_engine::repomap::RepoMapGenerator;
use infiniloom_engine::types::TokenizerModel;
use infiniloom_engine::OutputFormatter;

use crate::scanner;

/// Split repository into chunks for multi-turn LLM conversations
#[allow(clippy::too_many_arguments)]
pub fn cmd_chunk(
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
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
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

    let mut repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    // Apply exclude patterns if provided
    if !exclude.is_empty() {
        repo.files.retain(|f| {
            !exclude.iter().any(|pattern| {
                f.relative_path.contains(pattern)
                    || f.relative_path.starts_with(pattern)
                    || f.relative_path.split('/').any(|part| part == pattern)
            })
        });
    }

    // Apply include patterns (only keep matching files)
    if !include_patterns.is_empty() {
        repo.files.retain(|f| {
            include_patterns.iter().any(|pattern| {
                if pattern.contains('*') {
                    glob::Pattern::new(pattern).is_ok_and(|p| p.matches(&f.relative_path))
                } else {
                    f.relative_path.contains(pattern) || f.relative_path.ends_with(pattern)
                }
            })
        });
    }

    // Exclude test files unless include_tests is true
    if !include_tests {
        use infiniloom_engine::default_ignores::{matches_any, TEST_IGNORES};
        repo.files
            .retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    repo.metadata.total_files = repo.files.len() as u32;

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
        .map(|f| f.path.rsplit('/').next().unwrap_or(&f.path))
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
