//! Utility functions for parsing, scanning, and formatting
//!
//! This module contains helper functions used across the Node.js bindings:
//! - Parsing: Convert string options to engine types
//! - Scanning: Repository scanning with various options
//! - Formatting: Convert engine types to JavaScript-friendly strings

use infiniloom_bindings_common::{
    format_file_status as common_format_file_status,
    parse_compression,
    parse_format,
    parse_model,
    parse_security_threshold as common_parse_security_threshold,
    scan_repository as do_scan,
    ScanConfig,
};
use infiniloom_engine::{
    git::FileStatus as EngineFileStatus,
    parser::{Language, Parser},
    security::Severity,
    CompressionLevel,
    OutputFormat,
    Repository,
    TokenizerModel,
};
use napi::{Error, Result, Status};
use rayon::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;

// ============================================================================
// Parsing Helpers
// ============================================================================

/// Parse output format string to OutputFormat enum
pub fn napi_parse_format(format: Option<&str>) -> Result<OutputFormat> {
    parse_format(format).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

/// Parse model string to TokenizerModel enum
pub fn napi_parse_model(model: Option<&str>) -> Result<TokenizerModel> {
    parse_model(model).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

/// Parse compression level string to CompressionLevel enum
pub fn napi_parse_compression(compression: Option<&str>) -> Result<CompressionLevel> {
    parse_compression(compression).map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

/// Parse security severity threshold (Bug #5 fix)
pub fn parse_security_threshold(threshold: Option<&str>) -> Result<Severity> {
    common_parse_security_threshold(threshold)
        .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
}

// ============================================================================
// Scanning Helpers
// ============================================================================

/// Scan repository with default options
pub fn scan_repository(path: &str, read_contents: bool) -> Result<Repository> {
    scan_repository_with_options(path, read_contents, false)
}

/// Scan repository with custom options
pub fn scan_repository_with_options(
    path: &str,
    read_contents: bool,
    skip_symbols: bool,
) -> Result<Repository> {
    let path_buf = PathBuf::from(path);

    if !path_buf.exists() {
        return Err(Error::new(Status::InvalidArg, format!("Path does not exist: {}", path)));
    }

    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents,
        max_file_size: 50 * 1024 * 1024, // 50MB
        skip_symbols,
    };

    do_scan(&path_buf, config).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}

/// Read file contents in parallel for already-scanned files
///
/// This is used for the filter-first optimization pattern:
/// 1. Scan without reading content (fast)
/// 2. Apply filters
/// 3. Read content only for filtered files (this function)
pub fn read_contents_parallel(repo: &mut Repository) {
    repo.files.par_iter_mut().for_each(|file| {
        if let Ok(content) = std::fs::read_to_string(&file.path) {
            file.content = Some(content);
        }
    });
}

/// Read file contents and optionally extract symbols in parallel
///
/// When extract_symbols is true, uses thread-local Parser for symbol extraction.
pub fn read_contents_and_symbols_parallel(repo: &mut Repository, extract_symbols: bool) {
    if extract_symbols {
        thread_local! {
            static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
        }

        repo.files.par_iter_mut().for_each(|file| {
            if let Ok(content) = std::fs::read_to_string(&file.path) {
                // Extract symbols if we have a supported language
                if let Some(ref lang_str) = file.language {
                    if let Ok(lang) = lang_str.parse::<Language>() {
                        THREAD_PARSER.with(|parser| {
                            if let Ok(symbols) = parser.borrow_mut().parse(&content, lang) {
                                file.symbols = symbols;
                            }
                        });
                    }
                }
                file.content = Some(content);
            }
        });
    } else {
        read_contents_parallel(repo);
    }
}

// ============================================================================
// Formatting Helpers
// ============================================================================

/// Format FileStatus as string
pub fn format_file_status(status: EngineFileStatus) -> String {
    common_format_file_status(status).to_string()
}
