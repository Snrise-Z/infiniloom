//! Repository chunking operations
//!
//! This module provides functionality for splitting large repositories into
//! smaller chunks for incremental LLM processing.

use crate::types::{ChunkOptions, RepoChunk};
use crate::utils::{napi_parse_format, napi_parse_model, read_contents_and_symbols_parallel, scan_repository_with_options};
use crate::validation::validate_path_option;
use infiniloom_bindings_common::{apply_default_ignores, file_priority_score};
use infiniloom_engine::{ChunkStrategy, Chunker};
use napi::{Error, Result, Status};
use napi_derive::napi;

/// Split a repository into chunks for incremental processing
///
/// Useful for processing large repositories that exceed LLM context limits.
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
/// * `options` - Optional chunking options
///
/// # Returns
/// Array of repository chunks
///
/// # Example
/// ```javascript
/// const { chunk } = require('infiniloom-node');
///
/// const chunks = chunk('./large-repo', {
///   strategy: 'module',
///   maxTokens: 50000,
///   model: 'claude'
/// });
///
/// for (const c of chunks) {
///   console.log(`Chunk ${c.index}/${c.total}: ${c.focus} (${c.tokens} tokens)`);
///   // Process c.content with LLM
/// }
/// ```
#[napi]
pub fn chunk(path: Option<String>, options: Option<ChunkOptions>) -> Result<Vec<RepoChunk>> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;

    let opts = options.unwrap_or(ChunkOptions {
        strategy: None,
        max_tokens: None,
        overlap: None,
        model: None,
        format: None,
        priority_first: None,
        exclude: None,
    });

    let strategy = match opts.strategy.as_deref().unwrap_or("module") {
        "fixed" => ChunkStrategy::Fixed { size: opts.max_tokens.unwrap_or(8000) },
        "file" => ChunkStrategy::File,
        "module" => ChunkStrategy::Module,
        "symbol" => ChunkStrategy::Symbol,
        "semantic" => ChunkStrategy::Semantic,
        "dependency" => ChunkStrategy::Dependency,
        other => return Err(Error::new(
            Status::InvalidArg,
            format!("Unknown chunk strategy: {}. Use 'fixed', 'file', 'module', 'symbol', 'semantic', or 'dependency'", other),
        )),
    };

    // Bug fix: Validate maxTokens - values below minimum are rejected
    // maxTokens=0 is ambiguous (could mean "no limit" or "return nothing")
    // maxTokens < 100 is impractical for any meaningful chunking
    let max_tokens = opts.max_tokens.unwrap_or(8000);
    if max_tokens == 0 {
        return Err(Error::new(
            Status::InvalidArg,
            "maxTokens cannot be 0. Use a value >= 100 for meaningful chunks, or omit to use default (8000)".to_string(),
        ));
    }
    if max_tokens > 0 && max_tokens < 100 {
        return Err(Error::new(
            Status::InvalidArg,
            format!("maxTokens {} is too small. Minimum is 100 tokens for meaningful chunks.", max_tokens),
        ));
    }

    let overlap = opts.overlap.unwrap_or(0);
    let model = napi_parse_model(opts.model.as_deref())?;
    let format = napi_parse_format(opts.format.as_deref())?;
    let priority_first = opts.priority_first.unwrap_or(false);

    // STEP 1: Fast file list without reading content (filter-first optimization)
    let needs_symbols = matches!(strategy, ChunkStrategy::Dependency | ChunkStrategy::Symbol);
    let mut repo = scan_repository_with_options(&path, false, true)?;

    // STEP 2: Apply all filters BEFORE reading content
    // Apply default ignores
    apply_default_ignores(&mut repo);

    // Apply exclude patterns if provided
    if let Some(ref patterns) = opts.exclude {
        if !patterns.is_empty() {
            repo.files.retain(|f| {
                !patterns.iter().any(|pattern| {
                    f.relative_path.contains(pattern)
                        || f.relative_path.starts_with(pattern)
                        || f.relative_path.split('/').any(|part| part == pattern)
                })
            });
        }
    }

    // STEP 3: Read content and optionally extract symbols for filtered files
    read_contents_and_symbols_parallel(&mut repo, needs_symbols);

    // Create chunker
    let chunker = Chunker::new(strategy, max_tokens)
        .with_model(model)
        .with_overlap(overlap);

    let mut chunks = chunker.chunk(&repo);

    // Apply priority sorting if requested
    if priority_first && chunks.len() > 1 {
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

        chunk_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let original_chunks = std::mem::take(&mut chunks);
        for (idx, (orig_idx, _)) in chunk_priorities.iter().enumerate() {
            let mut chunk = original_chunks[*orig_idx].clone();
            chunk.index = idx;
            chunks.push(chunk);
        }

        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.total = total;
        }
    }

    // Format each chunk
    // Note: formatter and map_generator are available if we want to format chunks
    // For now, we return raw content and let the caller format
    let _ = format; // Mark format as used (could use for chunk formatting later)

    let result: Vec<RepoChunk> = chunks
        .iter()
        .map(|c| {
            // Format chunk content manually since ChunkFile doesn't match RepoFile
            let content = c
                .files
                .iter()
                .map(|f| format!("// {}\n{}", f.path, f.content))
                .collect::<Vec<_>>()
                .join("\n\n");

            RepoChunk {
                index: c.index as u32,
                total: c.total as u32,
                focus: c.focus.clone(),
                tokens: c.tokens,
                files: c.files.iter().map(|f| f.path.clone()).collect(),
                content,
            }
        })
        .collect();

    Ok(result)
}
