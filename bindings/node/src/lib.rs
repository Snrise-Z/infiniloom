#![deny(clippy::all)]

//! Infiniloom Node.js bindings
//!
//! This library provides high-performance Node.js bindings for the Infiniloom
//! repository context generator. It enables LLM-optimized codebase indexing,
//! symbol extraction, and diff analysis directly from Node.js/TypeScript.

// Module declarations
mod types;
mod validation;
mod utils;
mod security;
mod scan;
mod chunk;
mod pack;
mod git;
mod index;
mod call_graph;
mod symbols;
mod diff;
mod impact;

// Re-export all public types
pub use types::*;

// Re-export public functions from modules
pub use call_graph::{
    find_symbol, find_symbol_async, find_symbol_filtered, find_symbol_filtered_async,
    get_call_graph, get_call_graph_async, get_callees, get_callees_async, get_callees_filtered,
    get_callees_filtered_async, get_callers, get_callers_async, get_callers_filtered,
    get_callers_filtered_async, get_references, get_references_async, get_references_filtered,
    get_references_filtered_async,
};
pub use chunk::chunk;
pub use diff::get_diff_context;
pub use git::{is_git_repo, GitRepo};
pub use impact::analyze_impact;
pub use index::{build_index, index_status};
pub use pack::pack;
pub use scan::scan;
pub use security::scan_security;
pub use symbols::{
    get_call_sites, get_call_sites_async, get_call_sites_with_context,
    get_call_sites_with_context_async, get_changed_symbols, get_changed_symbols_async,
    get_changed_symbols_filtered, get_changed_symbols_filtered_async, get_symbol_source,
    get_symbol_source_async, get_symbols_in_file, get_symbols_in_file_async, get_tests_for_file,
    get_tests_for_file_async, get_transitive_callers, get_transitive_callers_async,
};

use napi_derive::napi;

// ============================================================================
// Package Version
// ============================================================================

/// Get the package version
///
/// # Returns
/// The version string of the infiniloom-node package
///
/// # Example
/// ```javascript
/// const { version } = require('infiniloom-node');
///
/// console.log(`infiniloom-node v${version()}`);
/// ```
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
