//! Call graph query operations
//!
//! This module provides comprehensive call graph querying functionality:
//! - Finding symbols by name
//! - Querying callers and callees
//! - Getting all references to symbols
//! - Retrieving the complete call graph
//! - Filtering results by symbol kind
//! - Async versions of all operations

use crate::types::{CallGraph, CallGraphOptions, QueryFilter, ReferenceInfo, SymbolInfo};
use crate::validation::{validate_path_option, validate_symbol_name_option};
use infiniloom_engine::index::{
    find_symbol as engine_find_symbol, get_call_graph as engine_get_call_graph,
    get_call_graph_filtered, get_callees_by_name, get_callers_by_name, get_references_by_name,
    CallGraph as EngineCallGraph, CallGraphStats as EngineCallGraphStats, IndexStorage,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::HashSet;
use std::path::PathBuf;

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper function to check if a symbol matches the query filter
fn matches_query_filter(symbol: &SymbolInfo, filter: &Option<QueryFilter>) -> bool {
    if let Some(ref f) = filter {
        let kind_lower = symbol.kind.to_lowercase();

        // Check if symbol kind is in the allowed list
        if let Some(ref allowed) = f.kinds {
            let allowed_lower: HashSet<String> = allowed.iter().map(|s| s.to_lowercase()).collect();
            if !allowed_lower.contains(&kind_lower) {
                return false;
            }
        }

        // Check if symbol kind is in the excluded list
        if let Some(ref excluded) = f.exclude_kinds {
            let excluded_lower: HashSet<String> =
                excluded.iter().map(|s| s.to_lowercase()).collect();
            if excluded_lower.contains(&kind_lower) {
                return false;
            }
        }
    }
    true
}

// ============================================================================
// Basic Query Functions
// ============================================================================

/// Find a symbol by name
///
/// Searches the index for all symbols matching the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
/// * `name` - Symbol name to search for (null/undefined returns error)
///
/// # Returns
/// Array of matching symbols
///
/// # Example
/// ```javascript
/// const { findSymbol, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const symbols = findSymbol('./my-repo', 'processRequest');
/// console.log(`Found ${symbols.length} symbols named processRequest`);
/// ```
#[napi]
pub fn find_symbol(path: Option<String>, name: Option<String>) -> Result<Vec<SymbolInfo>> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;
    let name = validate_symbol_name_option(name.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    let results = engine_find_symbol(&index, &name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all callers of a symbol
///
/// Returns symbols that call any symbol with the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
/// * `symbol_name` - Name of the symbol to find callers for (null/undefined returns error)
///
/// # Returns
/// Array of symbols that call the target symbol
///
/// # Example
/// ```javascript
/// const { getCallers, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callers = getCallers('./my-repo', 'authenticate');
/// console.log(`authenticate is called by ${callers.length} functions`);
/// for (const c of callers) {
///   console.log(`  ${c.name} at ${c.file}:${c.line}`);
/// }
/// ```
#[napi]
pub fn get_callers(path: Option<String>, symbol_name: Option<String>) -> Result<Vec<SymbolInfo>> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;
    let symbol_name = validate_symbol_name_option(symbol_name.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_callers_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all callees of a symbol
///
/// Returns symbols that are called by any symbol with the given name.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root (null/undefined returns error)
/// * `symbol_name` - Name of the symbol to find callees for (null/undefined returns error)
///
/// # Returns
/// Array of symbols that the target symbol calls
///
/// # Example
/// ```javascript
/// const { getCallees, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callees = getCallees('./my-repo', 'main');
/// console.log(`main calls ${callees.length} functions`);
/// for (const c of callees) {
///   console.log(`  ${c.name} at ${c.file}:${c.line}`);
/// }
/// ```
#[napi]
pub fn get_callees(path: Option<String>, symbol_name: Option<String>) -> Result<Vec<SymbolInfo>> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;
    let symbol_name = validate_symbol_name_option(symbol_name.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_callees_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Get all references to a symbol
///
/// Returns all locations where a symbol is referenced (calls, imports, inheritance).
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find references for
///
/// # Returns
/// Array of reference information including the referencing symbol and kind
///
/// # Example
/// ```javascript
/// const { getReferences, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const refs = getReferences('./my-repo', 'UserService');
/// console.log(`UserService is referenced ${refs.length} times`);
/// for (const r of refs) {
///   console.log(`  ${r.kind}: ${r.symbol.name} at ${r.symbol.file}:${r.symbol.line}`);
/// }
/// ```
#[napi]
pub fn get_references(
    path: Option<String>,
    symbol_name: Option<String>,
) -> Result<Vec<ReferenceInfo>> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;
    let symbol_name = validate_symbol_name_option(symbol_name.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results = get_references_by_name(&index, &graph, &symbol_name);
    Ok(results.into_iter().map(Into::into).collect())
}

// ============================================================================
// Filtered Query Functions
// ============================================================================

/// Find symbols by name with filtering
///
/// Like `findSymbol`, but allows filtering results by symbol kind.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `name` - Symbol name to search for
/// * `filter` - Optional filter for symbol kinds
///
/// # Returns
/// Array of matching symbols that pass the filter
///
/// # Example
/// ```javascript
/// const { findSymbolFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// // Find only functions named "process"
/// const funcs = findSymbolFiltered('./my-repo', 'process', {
///   kinds: ['function', 'method']
/// });
/// // Find all symbols except imports
/// const noImports = findSymbolFiltered('./my-repo', 'User', {
///   excludeKinds: ['import']
/// });
/// ```
#[napi]
pub fn find_symbol_filtered(
    path: String,
    name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    let results: Vec<SymbolInfo> = engine_find_symbol(&index, &name)
        .into_iter()
        .map(Into::into)
        .filter(|s| matches_query_filter(s, &filter))
        .collect();

    Ok(results)
}

/// Get callers of a symbol with filtering
///
/// Like `getCallers`, but allows filtering results by symbol kind.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callers for
/// * `filter` - Optional filter for symbol kinds
///
/// # Returns
/// Array of filtered calling symbols
///
/// # Example
/// ```javascript
/// const { getCallersFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// // Get only function callers (not class methods)
/// const callers = getCallersFiltered('./my-repo', 'authenticate', {
///   kinds: ['function']
/// });
/// ```
#[napi]
pub fn get_callers_filtered(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results: Vec<SymbolInfo> = get_callers_by_name(&index, &graph, &symbol_name)
        .into_iter()
        .map(Into::into)
        .filter(|s| matches_query_filter(s, &filter))
        .collect();

    Ok(results)
}

/// Get callees of a symbol with filtering
///
/// Like `getCallees`, but allows filtering results by symbol kind.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callees for
/// * `filter` - Optional filter for symbol kinds
///
/// # Returns
/// Array of filtered called symbols
///
/// # Example
/// ```javascript
/// const { getCalleesFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// // Get only function calls (not method calls)
/// const callees = getCalleesFiltered('./my-repo', 'main', {
///   kinds: ['function']
/// });
/// ```
#[napi]
pub fn get_callees_filtered(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results: Vec<SymbolInfo> = get_callees_by_name(&index, &graph, &symbol_name)
        .into_iter()
        .map(Into::into)
        .filter(|s| matches_query_filter(s, &filter))
        .collect();

    Ok(results)
}

/// Get references to a symbol with filtering
///
/// Like `getReferences`, but allows filtering results by symbol kind.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find references for
/// * `filter` - Optional filter for referencing symbol kinds
///
/// # Returns
/// Array of filtered reference information
///
/// # Example
/// ```javascript
/// const { getReferencesFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// // Get only call references from functions
/// const refs = getReferencesFiltered('./my-repo', 'UserService', {
///   kinds: ['function', 'method'],
///   excludeKinds: ['import']
/// });
/// ```
#[napi]
pub fn get_references_filtered(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<ReferenceInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let results: Vec<ReferenceInfo> = get_references_by_name(&index, &graph, &symbol_name)
        .into_iter()
        .map(Into::into)
        .filter(|r: &ReferenceInfo| matches_query_filter(&r.symbol, &filter))
        .collect();

    Ok(results)
}

// ============================================================================
// Async Filtered Query Functions
// ============================================================================

/// Async version of findSymbolFiltered
#[napi]
pub async fn find_symbol_filtered_async(
    path: String,
    name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || find_symbol_filtered(path, name, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallersFiltered
#[napi]
pub async fn get_callers_filtered_async(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callers_filtered(path, symbol_name, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCalleesFiltered
#[napi]
pub async fn get_callees_filtered_async(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callees_filtered(path, symbol_name, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getReferencesFiltered
#[napi]
pub async fn get_references_filtered_async(
    path: String,
    symbol_name: String,
    filter: Option<QueryFilter>,
) -> Result<Vec<ReferenceInfo>> {
    tokio::task::spawn_blocking(move || get_references_filtered(path, symbol_name, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

// ============================================================================
// Call Graph Operations
// ============================================================================

/// Get the complete call graph
///
/// Returns all symbols and their call relationships.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Optional filtering options
///
/// # Returns
/// Call graph with nodes (symbols), edges (calls), and statistics
///
/// # Example
/// ```javascript
/// const { getCallGraph, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const graph = getCallGraph('./my-repo');
/// console.log(`Call graph: ${graph.stats.totalSymbols} symbols, ${graph.stats.totalCalls} calls`);
///
/// // Find most called functions
/// const callCounts = new Map();
/// for (const edge of graph.edges) {
///   callCounts.set(edge.callee, (callCounts.get(edge.callee) || 0) + 1);
/// }
/// const sorted = [...callCounts.entries()].sort((a, b) => b[1] - a[1]);
/// console.log('Most called functions:', sorted.slice(0, 10));
/// ```
#[napi]
pub fn get_call_graph(
    path: Option<String>,
    options: Option<CallGraphOptions>,
) -> Result<CallGraph> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let result = if let Some(opts) = options {
        // Bug fix: maxNodes=0 or maxEdges=0 should return empty graph
        let max_nodes = opts.max_nodes.map(|n| n as usize);
        let max_edges = opts.max_edges.map(|n| n as usize);

        // If either limit is explicitly set to 0, return empty graph
        if max_nodes == Some(0) || max_edges == Some(0) {
            EngineCallGraph {
                nodes: vec![],
                edges: vec![],
                stats: EngineCallGraphStats {
                    total_symbols: 0,
                    total_calls: 0,
                    functions: 0,
                    classes: 0,
                },
            }
        } else {
            get_call_graph_filtered(&index, &graph, max_nodes, max_edges)
        }
    } else {
        engine_get_call_graph(&index, &graph)
    };

    Ok(result.into())
}

// ============================================================================
// Async Basic Query Functions
// ============================================================================

/// Async version of findSymbol
#[napi]
pub async fn find_symbol_async(
    path: Option<String>,
    name: Option<String>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || find_symbol(path, name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallers
#[napi]
pub async fn get_callers_async(
    path: Option<String>,
    symbol_name: Option<String>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callers(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallees
#[napi]
pub async fn get_callees_async(
    path: Option<String>,
    symbol_name: Option<String>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_callees(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getReferences
#[napi]
pub async fn get_references_async(
    path: Option<String>,
    symbol_name: Option<String>,
) -> Result<Vec<ReferenceInfo>> {
    tokio::task::spawn_blocking(move || get_references(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallGraph
#[napi]
pub async fn get_call_graph_async(
    path: Option<String>,
    options: Option<CallGraphOptions>,
) -> Result<CallGraph> {
    tokio::task::spawn_blocking(move || get_call_graph(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}
