//! Pack command module
//!
//! This module contains the pack command implementation and configuration using a
//! modern builder pattern architecture that replaced the original 78-parameter function.
//!
//! # Module Structure
//!
//! The pack module is organized into focused submodules for better maintainability:
//!
//! - **`config`**: Configuration types and builder pattern implementation
//!   - `PackConfig`: Main configuration struct
//!   - `PackConfigBuilder`: Fluent builder for constructing configurations
//!   - `OutputOptions`, `ScanOptions`, `GitOptions`, `SecurityOptions`, `WatchOptions`:
//!     Grouped option structs
//!
//! - **`filters`**: File filtering logic
//!   - Pattern matching (`pattern_matches_file`)
//!   - Default ignores, include/exclude patterns
//!
//! - **`compression`**: Content transformation and compression
//!   - Remove empty lines and comments
//!   - Extract signatures (aggressive, extreme, focused)
//!   - Heuristic-based extraction
//!
//! - **`budget`**: Token budget management and ranking
//!   - Token estimation and truncation
//!   - Fast file ranking by importance
//!   - Repository metadata recalculation
//!   - Incremental cache management
//!
//! - **`output`**: Output formatting and extras
//!   - `apply_pack_extras()`: Enrich output with header, instructions, token tree, security results
//!   - Format-specific helpers (XML, JSON, Markdown, YAML, TOON, Plain)
//!
//! - **`impl`**: Core implementation of the pack command
//!   - `cmd_pack()`: Main entry point (reduced from 78 parameters to 1)
//!
//! # Public API
//!
//! The module re-exports all public types and functions for convenient access:
//!
//! ```rust
//! use infiniloom_cli::commands::pack::{
//!     PackConfig,           // Main config struct
//!     PackConfigBuilder,    // Builder type
//!     OutputOptions,        // Output settings group
//!     ScanOptions,          // Scan settings group
//!     GitOptions,           // Git settings group
//!     SecurityOptions,      // Security settings group
//!     WatchOptions,         // Watch mode settings group
//!     cmd_pack,             // Main command function
//! };
//! ```
//!
//! # Refactoring History
//!
//! This module underwent two major refactorings to improve maintainability:
//!
//! **Before (Phase 0)**:
//! ```ignore
//! pub fn cmd_pack(
//!     path: PathBuf,
//!     cli_format: Option<OutputFormat>,
//!     cli_model: Option<TokenizerModel>,
//!     // ... 75 more parameters
//! ) -> Result<()>
//! ```
//!
//! **After Phase 1** (Item 1 - Parameter Reduction):
//! ```no_run
//! use infiniloom_cli::commands::pack::{PackConfig, cmd_pack};
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("/path/to/repo"))
//!     .build()
//!     .expect("Failed to build config");
//!
//! cmd_pack(config).expect("Pack failed");
//! ```
//!
//! **After Phase 2** (Item 8 - File Size Reduction):
//! Split impl.rs (3104 lines) into 5 focused modules:
//! - filters.rs (170 lines)
//! - compression.rs (515 lines)
//! - budget.rs (363 lines)
//! - output.rs (522 lines)
//! - tests.rs (429 lines)
//!
//! Removed old run_watch_mode (497 lines) - replaced by cli/src/watch.rs
//!
//! # Benefits of Refactoring
//!
//! 1. **Maintainability**: Easy to find and modify specific functionality
//! 2. **Type Safety**: Compile-time validation of required parameters
//! 3. **Testability**: Focused unit tests per module
//! 4. **Discoverability**: Clear module boundaries and responsibilities
//! 5. **Code Size**: Average file size reduced from 3104 to ~350 lines (89% reduction)
//!
//! # Usage Examples
//!
//! See [`config`] module documentation for comprehensive usage examples.
//!
//! ## Quick Start
//!
//! ```no_run
//! use infiniloom_cli::commands::pack::{PackConfig, cmd_pack};
//! use std::path::PathBuf;
//!
//! // Minimal configuration
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("."))
//!     .build()?;
//!
//! cmd_pack(config)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ## Advanced Configuration
//!
//! ```no_run
//! use infiniloom_cli::commands::pack::{
//!     PackConfig, OutputOptions, ScanOptions, cmd_pack
//! };
//! use infiniloom_engine::output::OutputFormat;
//! use infiniloom_engine::types::CompressionLevel;
//! use std::path::PathBuf;
//!
//! let config = PackConfig::builder()
//!     .path(PathBuf::from("."))
//!     .output(OutputOptions {
//!         format: Some(OutputFormat::Xml),
//!         compression: Some(CompressionLevel::Balanced),
//!         max_tokens: 100000,
//!         ..Default::default()
//!     })
//!     .scan(ScanOptions {
//!         enable_symbols: true,
//!         remove_comments: true,
//!         ..Default::default()
//!     })
//!     .verbose(true)
//!     .build()?;
//!
//! cmd_pack(config)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

// Configuration types
pub(crate) mod config;

// Submodules (Phase 2 refactoring - Item 8)
pub(crate) mod budget;
pub(crate) mod compression;
pub(crate) mod filters;
pub(crate) mod output;

// Core implementation
pub(crate) mod r#impl;

// Tests module (private)
#[cfg(test)]
mod tests;

// Unused from Phase 1 (kept for backward compatibility)
mod core;

// ============================================================================
// Re-exports for public API
// ============================================================================

// Configuration types
pub(crate) use config::{
    GitOptions, OutputOptions, PackConfig, ScanOptions, SecurityOptions, WatchOptions,
};

// Main command function
pub(crate) use r#impl::cmd_pack;

// File filtering functions (from filters module)

// Content transformation functions (from compression module)
pub(crate) use compression::{
    extract_key_symbols_focused, extract_key_symbols_only, extract_signatures_only,
    remove_comments_from_content, remove_empty_lines_from_content,
};

// Token budget functions (from budget module)
pub(crate) use budget::{
    enforce_budget, estimate_tokens, rank_files_fast, recalculate_metadata, truncate_to_tokens,
    update_repo_cache,
};

// Output formatting functions (from output module)
pub(crate) use output::{apply_pack_extras, read_instruction_file};
