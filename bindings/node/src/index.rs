//! Symbol index building and management
//!
//! This module provides functionality for building and querying symbol indexes.
//! The index enables fast diff-to-context lookups and impact analysis.

use crate::types::{IndexOptions, IndexStatus};
use crate::validation::validate_path_option;
use infiniloom_bindings_common::format_timestamp;
use infiniloom_engine::index::{BuildOptions, IndexBuilder, IndexStorage};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::path::PathBuf;

/// Build or update the symbol index for a repository
///
/// The index enables fast diff-to-context lookups and impact analysis.
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
/// * `options` - Optional index build options
///
/// # Returns
/// Index status after building
///
/// # Example
/// ```javascript
/// const { buildIndex } = require('infiniloom-node');
///
/// const status = buildIndex('./my-repo');
/// console.log(`Indexed ${status.symbolCount} symbols`);
///
/// // Force rebuild
/// const status2 = buildIndex('./my-repo', { force: true });
/// ```
#[napi]
pub fn build_index(path: Option<String>, options: Option<IndexOptions>) -> Result<IndexStatus> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;

    let opts = options.unwrap_or(IndexOptions {
        force: None,
        include_tests: None,
        max_file_size: None,
        exclude: None,
        incremental: None,
    });

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    // Check if we need to rebuild
    let force = opts.force.unwrap_or(false);
    let incremental = opts.incremental.unwrap_or(false);

    if !force && !incremental {
        // Check if index exists and is valid (return early if not forcing rebuild)
        if let Ok(meta) = storage.load_meta() {
            if let (Ok(index), Ok(_graph)) = (storage.load_index(), storage.load_graph()) {
                return Ok(IndexStatus {
                    exists: true,
                    file_count: index.files.len() as u32,
                    symbol_count: index.symbols.len() as u32,
                    last_built: Some(format_timestamp(meta.created_at)),
                    version: Some(format!("v{}", meta.version)),
                    files_updated: None,
                    incremental: Some(false),
                });
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

    // Exclude test directories if not including tests
    if !opts.include_tests.unwrap_or(false) {
        exclude_dirs.extend(vec![
            "test".to_string(),
            "tests".to_string(),
            "__tests__".to_string(),
            "spec".to_string(),
        ]);
    }

    // Feature #1: Add custom exclude patterns from user options
    if let Some(ref custom_excludes) = opts.exclude {
        exclude_dirs.extend(custom_excludes.iter().cloned());
    }

    let build_opts = BuildOptions {
        max_file_size: opts
            .max_file_size
            .map(|s| s as u64)
            .unwrap_or(10 * 1024 * 1024),
        exclude_dirs,
        ..Default::default()
    };

    // Feature #4: Incremental update support
    let (index, graph, files_updated) = if incremental && !force {
        // Try to load existing index for incremental update
        if let (Ok(existing_index), Ok(_existing_graph)) =
            (storage.load_index(), storage.load_graph())
        {
            // Build a set of existing file hashes for comparison
            let existing_hashes: std::collections::HashMap<String, [u8; 32]> = existing_index
                .files
                .iter()
                .map(|f| (f.path.clone(), f.content_hash))
                .collect();

            // Build new index
            let builder = IndexBuilder::new(&path_buf).with_options(build_opts);
            let (new_index, new_graph) = builder.build().map_err(|e| {
                Error::new(Status::GenericFailure, format!("Failed to build index: {}", e))
            })?;

            // Count how many files were updated (new or changed hash)
            let mut updated_count = 0u32;
            for file in &new_index.files {
                match existing_hashes.get(&file.path) {
                    Some(old_hash) if old_hash == &file.content_hash => {
                        // File unchanged
                    },
                    _ => {
                        // File is new or changed
                        updated_count += 1;
                    },
                }
            }

            (new_index, new_graph, Some(updated_count))
        } else {
            // No existing index, do full build
            let builder = IndexBuilder::new(&path_buf).with_options(build_opts);
            let (index, graph) = builder.build().map_err(|e| {
                Error::new(Status::GenericFailure, format!("Failed to build index: {}", e))
            })?;
            (index, graph, None)
        }
    } else {
        // Full rebuild
        let builder = IndexBuilder::new(&path_buf).with_options(build_opts);
        let (index, graph) = builder.build().map_err(|e| {
            Error::new(Status::GenericFailure, format!("Failed to build index: {}", e))
        })?;
        (index, graph, None)
    };

    // Save index
    storage
        .save_all(&index, &graph)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to save index: {}", e)))?;

    let meta = storage
        .load_meta()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load meta: {}", e)))?;

    Ok(IndexStatus {
        exists: true,
        file_count: index.files.len() as u32,
        symbol_count: index.symbols.len() as u32,
        last_built: Some(format_timestamp(meta.created_at)),
        version: Some(format!("v{}", meta.version)),
        files_updated,
        incremental: Some(incremental),
    })
}

/// Get the status of an existing index
///
/// # Arguments
/// * `path` - Path to repository root
///
/// # Returns
/// Index status information
///
/// # Example
/// ```javascript
/// const { indexStatus } = require('infiniloom-node');
///
/// const status = indexStatus('./my-repo');
/// if (status.exists) {
///   console.log(`Index has ${status.symbolCount} symbols`);
/// } else {
///   console.log('No index found, run buildIndex first');
/// }
/// ```
#[napi]
pub fn index_status(path: String) -> Result<IndexStatus> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    match (storage.load_meta(), storage.load_index()) {
        (Ok(meta), Ok(index)) => Ok(IndexStatus {
            exists: true,
            file_count: index.files.len() as u32,
            symbol_count: index.symbols.len() as u32,
            last_built: Some(format_timestamp(meta.created_at)),
            version: Some(format!("v{}", meta.version)),
            files_updated: None,
            incremental: None,
        }),
        _ => Ok(IndexStatus {
            exists: false,
            file_count: 0,
            symbol_count: 0,
            last_built: None,
            version: None,
            files_updated: None,
            incremental: None,
        }),
    }
}
