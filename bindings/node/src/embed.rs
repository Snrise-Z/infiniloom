//! Embedding chunk generation for Node.js bindings
//!
//! Provides deterministic, content-addressable code chunks for vector databases.

use crate::types::{
    EmbedChunk, EmbedChunkContext, EmbedChunkPart, EmbedChunkSource, EmbedDiffSummary,
    EmbedManifestStatus, EmbedOptions, EmbedResult,
};
use crate::validation::validate_path_option;
use infiniloom_engine::embedding::{
    EmbedChunk as EngineChunk, EmbedChunker, EmbedManifest, EmbedSettings, QuietProgress,
    ResourceLimits, MANIFEST_VERSION,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::path::PathBuf;
use std::time::Instant;

/// Convert engine chunk to NAPI-compatible chunk
fn engine_chunk_to_napi(chunk: &EngineChunk) -> EmbedChunk {
    EmbedChunk {
        id: chunk.id.clone(),
        full_hash: chunk.full_hash.clone(),
        content: chunk.content.clone(),
        tokens: chunk.tokens,
        kind: chunk.kind.name().to_string(),
        source: EmbedChunkSource {
            file: chunk.source.file.clone(),
            lines_start: chunk.source.lines.0,
            lines_end: chunk.source.lines.1,
            symbol: chunk.source.symbol.clone(),
            fqn: chunk.source.fqn.clone(),
            language: chunk.source.language.clone(),
            parent: chunk.source.parent.clone(),
            visibility: format!("{:?}", chunk.source.visibility).to_lowercase(),
            is_test: chunk.source.is_test,
        },
        context: EmbedChunkContext {
            docstring: chunk.context.docstring.clone(),
            comments: chunk.context.comments.clone(),
            signature: chunk.context.signature.clone(),
            calls: chunk.context.calls.clone(),
            called_by: chunk.context.called_by.clone(),
            imports: chunk.context.imports.clone(),
            tags: chunk.context.tags.clone(),
            qualified_calls: chunk.context.qualified_calls.clone(),
            unresolved_calls: chunk.context.unresolved_calls.clone(),

            type_signature: chunk.context.type_signature.clone(),
            parameter_types: chunk.context.parameter_types.clone(),
            return_type: chunk.context.return_type.clone(),
            error_types: chunk.context.error_types.clone(),
            lines_of_code: chunk.context.lines_of_code,
            max_nesting_depth: chunk.context.max_nesting_depth,
        },
        part: chunk.part.as_ref().map(|p| EmbedChunkPart {
            part: p.part,
            of: p.of,
            parent_id: p.parent_id.clone(),
            parent_signature: Some(p.parent_signature.clone()),
        }),
    }
}

/// Generate embedding chunks for a repository
///
/// Creates deterministic, content-addressable code chunks optimized for vector databases.
/// Uses manifest-based diffing for incremental updates.
///
/// # Arguments
/// * `path` - Path to repository root (default: current directory)
/// * `options` - Optional embedding options
///
/// # Returns
/// Embedding result with chunks and optional diff summary
///
/// # Example
/// ```javascript
/// const { embed } = require('infiniloom-node');
///
/// const result = embed('./my-repo', {
///   maxTokens: 1000,
///   securityScan: true,
///   diffOnly: false
/// });
///
/// console.log(`Generated ${result.chunks.length} chunks`);
/// for (const chunk of result.chunks) {
///   console.log(`${chunk.id}: ${chunk.source.symbol} (${chunk.tokens} tokens)`);
/// }
/// ```
#[napi]
pub fn embed(path: Option<String>, options: Option<EmbedOptions>) -> Result<EmbedResult> {
    let start = Instant::now();

    // Input validation - handle null/undefined gracefully (defaults to ".")
    let path_str = validate_path_option(path.as_deref().or(Some(".")))?;
    let repo_path = PathBuf::from(path_str);
    let opts = options.unwrap_or(EmbedOptions {
        max_tokens: None,
        min_tokens: None,
        context_lines: None,
        include_imports: None,
        include_top_level: None,
        include_tests: None,
        security_scan: None,
        include_patterns: None,
        exclude_patterns: None,
        manifest_path: None,
        diff_only: None,
    });

    // Build settings
    let settings = EmbedSettings {
        max_tokens: opts.max_tokens.unwrap_or(1000),
        min_tokens: opts.min_tokens.unwrap_or(50),
        context_lines: opts.context_lines.unwrap_or(5),
        token_model: "claude".to_string(),
        include_imports: opts.include_imports.unwrap_or(true),
        include_top_level: opts.include_top_level.unwrap_or(true),
        scan_secrets: opts.security_scan.unwrap_or(true),
        fail_on_secrets: false,
        redact_secrets: opts.security_scan.unwrap_or(true),
        include_patterns: opts.include_patterns.unwrap_or_default(),
        exclude_patterns: opts.exclude_patterns.unwrap_or_default(),
        include_tests: opts.include_tests.unwrap_or(false),
        ..Default::default()
    };

    // Validate settings
    settings
        .validate()
        .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;

    // Create chunker
    let limits = ResourceLimits::default();
    let mut chunker = EmbedChunker::new(settings.clone(), limits);

    // Generate chunks
    let progress = QuietProgress;
    let chunks = chunker
        .chunk_repository(&repo_path, &progress)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    // Handle manifest for diffing
    let manifest_path = if let Some(ref mp) = opts.manifest_path {
        if PathBuf::from(mp).is_absolute() {
            PathBuf::from(mp)
        } else {
            repo_path.join(mp)
        }
    } else {
        repo_path.join(".infiniloom-embed.bin")
    };

    let existing_manifest = EmbedManifest::load_if_exists(&manifest_path)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    // Compute diff if manifest exists
    let diff = existing_manifest.as_ref().map(|m| {
        let d = m.diff(&chunks);
        EmbedDiffSummary {
            added: d.summary.added as u32,
            modified: d.summary.modified as u32,
            removed: d.summary.removed as u32,
            unchanged: d.summary.unchanged as u32,
            total_chunks: d.summary.total_chunks as u32,
        }
    });

    // Update and save manifest
    let mut manifest = existing_manifest.unwrap_or_else(|| {
        EmbedManifest::new(repo_path.to_string_lossy().to_string(), settings)
    });
    manifest
        .update(&chunks)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
    manifest
        .save(&manifest_path)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    // Convert chunks to NAPI types
    let napi_chunks: Vec<EmbedChunk> = chunks.iter().map(engine_chunk_to_napi).collect();

    let elapsed = start.elapsed();

    Ok(EmbedResult {
        chunks: napi_chunks,
        diff,
        manifest_version: MANIFEST_VERSION,
        elapsed_ms: elapsed.as_millis() as f64,
    })
}

/// Async version of embed
///
/// Generate embedding chunks asynchronously, useful for Node.js applications
/// that want to avoid blocking the event loop.
///
/// # Example
/// ```javascript
/// const { embedAsync } = require('infiniloom-node');
///
/// const result = await embedAsync('./my-repo', {
///   maxTokens: 1000,
///   diffOnly: true
/// });
/// ```
#[napi]
pub async fn embed_async(path: Option<String>, options: Option<EmbedOptions>) -> Result<EmbedResult> {
    tokio::task::spawn_blocking(move || embed(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Load and inspect an embedding manifest
///
/// Returns status information about an existing manifest file.
///
/// # Arguments
/// * `path` - Path to repository or manifest file
///
/// # Returns
/// Manifest status information
///
/// # Example
/// ```javascript
/// const { loadEmbedManifest } = require('infiniloom-node');
///
/// const status = loadEmbedManifest('./my-repo');
/// if (status.exists) {
///   console.log(`Manifest has ${status.chunkCount} chunks`);
/// }
/// ```
#[napi]
pub fn load_embed_manifest(path: Option<String>) -> Result<EmbedManifestStatus> {
    let path_str = path.unwrap_or_else(|| ".".to_string());
    let repo_path = PathBuf::from(&path_str);

    // Determine manifest path
    let manifest_path = if repo_path.extension().is_some_and(|ext| ext == "bin") {
        // Path is directly to manifest file
        repo_path
    } else {
        // Path is to repository, use default manifest location
        repo_path.join(".infiniloom-embed.bin")
    };

    if !manifest_path.exists() {
        return Ok(EmbedManifestStatus {
            exists: false,
            chunk_count: None,
            repo_path: None,
            updated_at: None,
            version: None,
        });
    }

    let manifest = EmbedManifest::load(&manifest_path)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    Ok(EmbedManifestStatus {
        exists: true,
        chunk_count: Some(manifest.chunk_count() as u32),
        repo_path: Some(manifest.repo_path),
        updated_at: manifest.updated_at.map(|t| t as f64),
        version: Some(manifest.version),
    })
}

/// Async version of loadEmbedManifest
#[napi]
pub async fn load_embed_manifest_async(path: Option<String>) -> Result<EmbedManifestStatus> {
    tokio::task::spawn_blocking(move || load_embed_manifest(path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Delete an embedding manifest
///
/// Removes the manifest file to force a full rebuild on next embed.
///
/// # Arguments
/// * `path` - Path to repository or manifest file
///
/// # Returns
/// true if manifest was deleted, false if it didn't exist
///
/// # Example
/// ```javascript
/// const { deleteEmbedManifest } = require('infiniloom-node');
///
/// const deleted = deleteEmbedManifest('./my-repo');
/// console.log(deleted ? 'Manifest deleted' : 'No manifest found');
/// ```
#[napi]
pub fn delete_embed_manifest(path: Option<String>) -> Result<bool> {
    let path_str = path.unwrap_or_else(|| ".".to_string());
    let repo_path = PathBuf::from(&path_str);

    // Determine manifest path
    let manifest_path = if repo_path.extension().is_some_and(|ext| ext == "bin") {
        repo_path
    } else {
        repo_path.join(".infiniloom-embed.bin")
    };

    if !manifest_path.exists() {
        return Ok(false);
    }

    std::fs::remove_file(&manifest_path)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    Ok(true)
}

/// Async version of deleteEmbedManifest
#[napi]
pub async fn delete_embed_manifest_async(path: Option<String>) -> Result<bool> {
    tokio::task::spawn_blocking(move || delete_embed_manifest(path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}
