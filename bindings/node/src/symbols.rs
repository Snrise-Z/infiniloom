//! Symbol operations and queries
//!
//! This module provides comprehensive symbol analysis:
//! - Getting symbols in files with filtering
//! - Extracting symbol source code
//! - Finding changed symbols in diffs
//! - Discovering test files
//! - Locating call sites with context
//! - Transitive caller analysis
//! - Async versions of all operations

use crate::types::{
    CallSite, CallSiteWithContext, CallSitesContextOptions, ChangedSymbolInfo,
    ChangedSymbolsFilter, SymbolFilter, SymbolInfo, SymbolSourceResult, TransitiveCallerInfo,
    TransitiveCallersOptions,
};
use crate::validation::{validate_path, validate_path_option, validate_symbol_name_option};
use infiniloom_bindings_common::{find_call_site_in_body as common_find_call_site_in_body, get_line_context as common_get_line_context};
use infiniloom_engine::{
    git::{FileStatus as EngineFileStatus, GitRepo as EngineGitRepo},
    index::IndexStorage,
};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Get all symbols in a specific file
///
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `file_path` - Relative path to the file within the repository
/// * `filter` - Optional filter for symbol kind/visibility
///
/// # Returns
/// Array of symbols defined in the file
///
/// # Example
/// ```javascript
/// const { getSymbolsInFile, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const symbols = getSymbolsInFile('./my-repo', 'src/auth.ts');
/// console.log(`Found ${symbols.length} symbols in auth.ts`);
/// for (const s of symbols) {
///   console.log(`  ${s.kind}: ${s.name} at line ${s.line}`);
/// }
/// ```
#[napi]
pub fn get_symbols_in_file(
    path: String,
    file_path: String,
    filter: Option<SymbolFilter>,
) -> Result<Vec<SymbolInfo>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Get file entry
    let file = index.get_file(&file_path).ok_or_else(|| {
        Error::new(Status::GenericFailure, format!("File not found in index: {}", file_path))
    })?;

    // Get all symbols in this file
    let symbols = index.get_file_symbols(file.id);

    // Filter and convert to SymbolInfo
    let mut results: Vec<SymbolInfo> = symbols
        .iter()
        .filter(|s| {
            if let Some(ref f) = filter {
                // Filter by kind
                if let Some(ref kind) = f.kind {
                    if s.kind.name() != kind.as_str() {
                        return false;
                    }
                }
                // Filter by visibility
                if let Some(ref vis) = f.visibility {
                    let sym_vis = match s.visibility {
                        infiniloom_engine::index::Visibility::Public => "public",
                        infiniloom_engine::index::Visibility::Private => "private",
                        infiniloom_engine::index::Visibility::Protected => "protected",
                        infiniloom_engine::index::Visibility::Internal => "internal",
                    };
                    if sym_vis != vis.as_str() {
                        return false;
                    }
                }
            }
            true
        })
        .map(|sym| {
            use infiniloom_engine::index::query::SymbolInfo as EngineSymbolInfo;
            EngineSymbolInfo::from_index_symbol(sym, &index).into()
        })
        .collect();

    // Sort by line number
    results.sort_by_key(|s| s.line);
    Ok(results)
}

/// Get the source code of a symbol
///
/// Reads the file and extracts the source code for the specified symbol.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to get source for
/// * `file_path` - Optional file path to disambiguate when multiple symbols have the same name
///
/// # Returns
/// Source code of the symbol (or the first matching symbol if multiple exist)
///
/// # Example
/// ```javascript
/// const { getSymbolSource, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const result = getSymbolSource('./my-repo', 'authenticate', 'src/auth.ts');
/// console.log(`Source at ${result.path}:${result.startLine}`);
/// console.log(result.source);
/// ```
#[napi]
pub fn get_symbol_source(
    path: Option<String>,
    symbol_name: Option<String>,
    file_path: Option<String>,
) -> Result<SymbolSourceResult> {
    // Input validation - handle null/undefined gracefully
    let path = validate_path_option(path.as_deref())?;
    let symbol_name = validate_symbol_name_option(symbol_name.as_deref())?;

    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Find the symbol
    let symbols = index.find_symbols(&symbol_name);
    if symbols.is_empty() {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Symbol not found: {}", symbol_name),
        ));
    }

    // Filter by file path if specified
    let symbol = if let Some(ref fp) = file_path {
        symbols
            .iter()
            .find(|s| {
                index
                    .get_file_by_id(s.file_id.as_u32())
                    .is_some_and(|f| f.path == *fp)
            })
            .or_else(|| symbols.first())
    } else {
        symbols.first()
    };

    let symbol = symbol.ok_or_else(|| {
        Error::new(Status::GenericFailure, format!("Symbol not found: {}", symbol_name))
    })?;

    // Get file path
    let file = index
        .get_file_by_id(symbol.file_id.as_u32())
        .ok_or_else(|| Error::new(Status::GenericFailure, "File not found in index"))?;

    // Read file content
    let full_path = path_buf.join(&file.path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to read file: {}", e)))?;

    // Extract the symbol source (lines are 1-indexed)
    let lines: Vec<&str> = content.lines().collect();
    let start = (symbol.span.start_line as usize).saturating_sub(1);
    let end = (symbol.span.end_line as usize).min(lines.len());

    if start >= lines.len() {
        return Err(Error::new(Status::GenericFailure, "Symbol line numbers out of range"));
    }

    let source = lines[start..end].join("\n");

    // Format symbol kind
    use infiniloom_engine::index::types::IndexSymbolKind;
    let kind = match symbol.kind {
        IndexSymbolKind::Function => "function",
        IndexSymbolKind::Method => "method",
        IndexSymbolKind::Class => "class",
        IndexSymbolKind::Struct => "struct",
        IndexSymbolKind::Enum => "enum",
        IndexSymbolKind::Interface => "interface",
        IndexSymbolKind::Trait => "trait",
        IndexSymbolKind::Constant => "constant",
        IndexSymbolKind::Variable => "variable",
        IndexSymbolKind::Module => "module",
        IndexSymbolKind::Import => "import",
        IndexSymbolKind::Export => "export",
        IndexSymbolKind::TypeAlias => "type_alias",
        IndexSymbolKind::Macro => "macro",
    };

    Ok(SymbolSourceResult {
        source,
        path: file.path.clone(),
        start_line: symbol.span.start_line,
        end_line: symbol.span.end_line,
        name: symbol.name.clone(),
        kind: kind.to_string(),
    })
}

/// Get symbols that were changed in a diff
///
/// Parses the diff between two refs and identifies which symbols were modified.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch (e.g., "main", "HEAD~1")
/// * `to_ref` - Ending commit/branch (e.g., "HEAD", "feature-branch")
///
/// # Returns
/// Array of symbols that were modified in the diff
///
/// # Example
/// ```javascript
/// const { getChangedSymbols, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const changed = getChangedSymbols('./my-repo', 'main', 'HEAD');
/// console.log(`${changed.length} symbols were modified`);
/// for (const s of changed) {
///   console.log(`  ${s.kind}: ${s.name} in ${s.file}`);
/// }
/// ```
#[napi]
pub fn get_changed_symbols(
    path: String,
    from_ref: String,
    to_ref: String,
) -> Result<Vec<SymbolInfo>> {
    // Input validation
    validate_path(&path)?;

    let path_buf = PathBuf::from(&path);

    // Open git repo
    let git_repo = EngineGitRepo::open(&path_buf).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e))
    })?;

    // Load index
    let storage = IndexStorage::new(&path_buf);
    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    // Get refs
    let from = if from_ref.is_empty() {
        "HEAD"
    } else {
        &from_ref
    };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    // OPTIMIZATION: Get all hunks in one git call
    let all_hunks = git_repo.diff_hunks(from, to, None).unwrap_or_default();

    // Group hunks by file path
    let mut hunks_by_file: HashMap<&str, Vec<_>> = HashMap::new();
    for hunk in &all_hunks {
        hunks_by_file.entry(&hunk.file).or_default().push(hunk);
    }

    let mut changed_symbols: Vec<SymbolInfo> = Vec::new();
    let mut seen_ids: HashSet<u32> = HashSet::new();

    // Process each file that has hunks
    for (file_path, hunks) in &hunks_by_file {
        let file_entry = match index.get_file(file_path) {
            Some(f) => f,
            None => continue,
        };

        // Find symbols that overlap with changed lines
        for hunk in hunks {
            if hunk.new_count == 0 {
                continue;
            }

            let start_line = hunk.new_start;
            let end_line = hunk.new_start + hunk.new_count;

            for sym in index.get_file_symbols(file_entry.id) {
                let sym_overlaps =
                    sym.span.start_line <= end_line && sym.span.end_line >= start_line;

                if sym_overlaps && !seen_ids.contains(&sym.id.as_u32()) {
                    seen_ids.insert(sym.id.as_u32());
                    use infiniloom_engine::index::query::SymbolInfo as EngineSymbolInfo;
                    changed_symbols.push(EngineSymbolInfo::from_index_symbol(sym, &index).into());
                }
            }
        }
    }

    // Sort by file and line
    changed_symbols.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(changed_symbols)
}

/// Get test files related to a source file
///
/// Finds test files that:
/// 1. Import the specified file
/// 2. Match common test naming conventions
///
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `file_path` - Relative path to the source file
///
/// # Returns
/// Array of test file paths related to the source file
///
/// # Example
/// ```javascript
/// const { getTestsForFile, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const tests = getTestsForFile('./my-repo', 'src/auth.ts');
/// console.log(`Found ${tests.length} test files for auth.ts`);
/// ```
#[napi]
pub fn get_tests_for_file(path: String, file_path: String) -> Result<Vec<String>> {
    let path_buf = PathBuf::from(&path);
    let storage = IndexStorage::new(&path_buf);

    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;
    let graph = storage
        .load_graph()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load graph: {}", e)))?;

    let file = index.get_file(&file_path);
    let file_id = file.map(|f| f.id.as_u32());

    let mut test_files: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let is_test_file = |path: &str| -> bool {
        let path_lower = path.to_lowercase();
        path_lower.contains("test")
            || path_lower.contains("spec")
            || path_lower.contains("__tests__")
            || path_lower.ends_with("_test.rs")
            || path_lower.ends_with("_test.go")
            || path_lower.ends_with("_test.py")
            || path_lower.ends_with(".test.ts")
            || path_lower.ends_with(".test.js")
            || path_lower.ends_with(".spec.ts")
            || path_lower.ends_with(".spec.js")
    };

    // Method 1: Find test files that import this file
    if let Some(fid) = file_id {
        let importers = graph.get_importers(fid);
        for importer_id in importers {
            if let Some(importer_file) = index.get_file_by_id(importer_id) {
                if is_test_file(&importer_file.path) && seen.insert(importer_file.path.clone()) {
                    test_files.push(importer_file.path.clone());
                }
            }
        }
    }

    // Method 2: Find test files by naming convention
    let path_lower = file_path.to_lowercase();
    let base_name = std::path::Path::new(&path_lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if !base_name.is_empty() {
        let test_patterns = [
            format!("{}_test.", base_name),
            format!("test_{}", base_name),
            format!("{}.test.", base_name),
            format!("{}.spec.", base_name),
            format!("test/{}", base_name),
            format!("tests/{}", base_name),
            format!("__tests__/{}", base_name),
        ];

        for indexed_file in &index.files {
            if is_test_file(&indexed_file.path) {
                let file_lower = indexed_file.path.to_lowercase();
                for pattern in &test_patterns {
                    if file_lower.contains(pattern) && seen.insert(indexed_file.path.clone()) {
                        test_files.push(indexed_file.path.clone());
                        break;
                    }
                }
            }
        }
    }

    Ok(test_files)
}

/// Get call sites where a symbol is called
///
/// Returns the locations where a function/method is called, with exact line numbers.
/// Requires an index to be built first (use `buildIndex`).
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find call sites for
///
/// # Returns
/// Array of call sites with caller information and line numbers
///
/// # Example
/// ```javascript
/// const { getCallSites, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callSites = getCallSites('./my-repo', 'authenticate');
/// console.log(`authenticate is called from ${callSites.length} locations`);
/// ```
#[napi]
pub fn get_call_sites(path: Option<String>, symbol_name: Option<String>) -> Result<Vec<CallSite>> {
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

    let mut call_sites: Vec<CallSite> = Vec::new();
    let mut seen_sites: HashSet<(String, u32, u32, u32)> = HashSet::new();
    let mut file_cache: HashMap<String, Vec<String>> = HashMap::new();

    for sym in index.find_symbols(&symbol_name) {
        let callee_id = sym.id.as_u32();

        for caller_id in graph.get_callers(callee_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                let file_path = index
                    .get_file_by_id(caller_sym.file_id.as_u32())
                    .map(|f| f.path.clone())
                    .unwrap_or_else(|| "<unknown>".to_owned());

                let (call_line, call_col) = common_find_call_site_in_body(
                    &path_buf,
                    &file_path,
                    caller_sym.span.start_line,
                    caller_sym.span.end_line,
                    &symbol_name,
                    &mut file_cache,
                );

                let site_key = (file_path.clone(), call_line, caller_id, callee_id);
                if seen_sites.insert(site_key) {
                    call_sites.push(CallSite {
                        caller: caller_sym.name.clone(),
                        callee: sym.name.clone(),
                        file: file_path,
                        line: call_line,
                        column: call_col,
                        caller_id,
                        callee_id,
                    });
                }
            }
        }
    }

    call_sites.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(call_sites)
}

/// Get symbols that were changed in a diff with filtering and change type
///
/// Enhanced version with filtering by symbol kind and change type detection.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `from_ref` - Starting commit/branch
/// * `to_ref` - Ending commit/branch
/// * `filter` - Optional filter for symbol kinds
///
/// # Returns
/// Array of symbols with change type
///
/// # Example
/// ```javascript
/// const { getChangedSymbolsFiltered, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const changed = getChangedSymbolsFiltered('./my-repo', 'main', 'HEAD', {
///   kinds: ['function', 'method']
/// });
/// ```
#[napi]
pub fn get_changed_symbols_filtered(
    path: String,
    from_ref: String,
    to_ref: String,
    filter: Option<ChangedSymbolsFilter>,
) -> Result<Vec<ChangedSymbolInfo>> {
    let path_buf = PathBuf::from(&path);

    let git_repo = EngineGitRepo::open(&path_buf).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Failed to open git repo: {}", e))
    })?;

    let storage = IndexStorage::new(&path_buf);
    let index = storage
        .load_index()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load index: {}", e)))?;

    let from = if from_ref.is_empty() { "HEAD" } else { &from_ref };
    let to = if to_ref.is_empty() { "HEAD" } else { &to_ref };

    let changed_files = git_repo
        .diff_files(from, to)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let file_status_map: HashMap<String, EngineFileStatus> =
        changed_files.into_iter().map(|f| (f.path, f.status)).collect();

    let all_hunks = git_repo.diff_hunks(from, to, None).unwrap_or_default();
    let mut hunks_by_file: HashMap<&str, Vec<_>> = HashMap::new();
    for hunk in &all_hunks {
        hunks_by_file.entry(&hunk.file).or_default().push(hunk);
    }

    let mut changed_symbols: Vec<ChangedSymbolInfo> = Vec::new();
    let mut seen_ids: HashSet<u32> = HashSet::new();

    let kinds: Option<HashSet<String>> = filter
        .as_ref()
        .and_then(|f| f.kinds.as_ref())
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect::<HashSet<String>>());
    let exclude_kinds: Option<HashSet<String>> = filter
        .as_ref()
        .and_then(|f| f.exclude_kinds.as_ref())
        .map(|v| v.iter().map(|s| s.to_lowercase()).collect::<HashSet<String>>());

    let passes_filter = |kind_name: &str| -> bool {
        if let Some(ref allowed_kinds) = kinds {
            if !allowed_kinds.contains(&kind_name.to_string()) {
                return false;
            }
        }
        if let Some(ref excluded) = exclude_kinds {
            if excluded.contains(&kind_name.to_string()) {
                return false;
            }
        }
        true
    };

    let mut all_files: HashSet<&str> = HashSet::new();
    for path in file_status_map.keys() {
        all_files.insert(path.as_str());
    }
    for path in hunks_by_file.keys() {
        all_files.insert(path);
    }

    for file_path in all_files {
        let status = file_status_map
            .get(file_path)
            .copied()
            .unwrap_or(EngineFileStatus::Modified);
        let file_change_type = match status {
            EngineFileStatus::Added => "added",
            EngineFileStatus::Deleted => "deleted",
            _ => "modified",
        };

        let file_entry = match index.get_file(file_path) {
            Some(f) => f,
            None => continue,
        };

        if status == EngineFileStatus::Added || status == EngineFileStatus::Deleted {
            for sym in index.get_file_symbols(file_entry.id) {
                let kind_name = sym.kind.name().to_lowercase();
                if !passes_filter(&kind_name) || seen_ids.contains(&sym.id.as_u32()) {
                    continue;
                }
                seen_ids.insert(sym.id.as_u32());
                changed_symbols.push(ChangedSymbolInfo {
                    id: sym.id.as_u32(),
                    name: sym.name.clone(),
                    kind: kind_name,
                    file: file_path.to_string(),
                    line: sym.span.start_line,
                    end_line: sym.span.end_line,
                    signature: sym.signature.clone(),
                    visibility: format!("{:?}", sym.visibility).to_lowercase(),
                    change_type: file_change_type.to_string(),
                });
            }
            continue;
        }

        if let Some(hunks) = hunks_by_file.get(file_path) {
            for hunk in hunks {
                if hunk.new_count == 0 {
                    continue;
                }
                let start_line = hunk.new_start;
                let end_line = hunk.new_start + hunk.new_count;

                for sym in index.get_file_symbols(file_entry.id) {
                    let sym_overlaps =
                        sym.span.start_line <= end_line && sym.span.end_line >= start_line;
                    if !sym_overlaps || seen_ids.contains(&sym.id.as_u32()) {
                        continue;
                    }
                    let kind_name = sym.kind.name().to_lowercase();
                    if !passes_filter(&kind_name) {
                        continue;
                    }
                    seen_ids.insert(sym.id.as_u32());
                    changed_symbols.push(ChangedSymbolInfo {
                        id: sym.id.as_u32(),
                        name: sym.name.clone(),
                        kind: kind_name,
                        file: file_path.to_string(),
                        line: sym.span.start_line,
                        end_line: sym.span.end_line,
                        signature: sym.signature.clone(),
                        visibility: format!("{:?}", sym.visibility).to_lowercase(),
                        change_type: "modified".to_string(),
                    });
                }
            }
        }
    }

    changed_symbols.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(changed_symbols)
}

/// Get all functions that eventually call a symbol
///
/// Traverses the call graph to find all direct and indirect callers.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find callers for
/// * `options` - Optional query options
///
/// # Returns
/// Array of callers with their depth and call path
///
/// # Example
/// ```javascript
/// const { getTransitiveCallers, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const callers = getTransitiveCallers('./my-repo', 'validateInput', { maxDepth: 3 });
/// ```
#[napi]
pub fn get_transitive_callers(
    path: Option<String>,
    symbol_name: Option<String>,
    options: Option<TransitiveCallersOptions>,
) -> Result<Vec<TransitiveCallerInfo>> {
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

    let max_depth = options.as_ref().and_then(|o| o.max_depth).unwrap_or(3);
    let max_results = options.as_ref().and_then(|o| o.max_results).unwrap_or(100) as usize;

    if max_depth == 0 {
        return Ok(vec![]);
    }

    let mut results: Vec<TransitiveCallerInfo> = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();

    let target_symbols: Vec<_> = index.find_symbols(&symbol_name);
    if target_symbols.is_empty() {
        return Ok(vec![]);
    }

    let mut queue: std::collections::VecDeque<(u32, u32, Vec<String>)> =
        std::collections::VecDeque::new();

    for target in &target_symbols {
        visited.insert(target.id.as_u32());
        queue.push_back((target.id.as_u32(), 0, vec![target.name.clone()]));
    }

    while let Some((current_id, current_depth, call_path)) = queue.pop_front() {
        if results.len() >= max_results {
            break;
        }

        for caller_id in graph.get_callers(current_id) {
            if visited.insert(caller_id) {
                if let Some(caller) = index.get_symbol(caller_id) {
                    let mut new_path = call_path.clone();
                    new_path.insert(0, caller.name.clone());

                    let file_path = index
                        .get_file_by_id(caller.file_id.as_u32())
                        .map(|f| f.path.clone())
                        .unwrap_or_else(|| "<unknown>".to_string());

                    results.push(TransitiveCallerInfo {
                        name: caller.name.clone(),
                        kind: caller.kind.name().to_string(),
                        file: file_path,
                        line: caller.span.start_line,
                        depth: current_depth + 1,
                        call_path: new_path.clone(),
                    });

                    if current_depth + 1 < max_depth {
                        queue.push_back((caller_id, current_depth + 1, new_path));
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| (a.depth, &a.name).cmp(&(b.depth, &b.name)));
    Ok(results)
}

/// Get call sites with surrounding code context
///
/// Enhanced version that includes the surrounding code for each call site.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `symbol_name` - Name of the symbol to find call sites for
/// * `options` - Optional context options
///
/// # Returns
/// Array of call sites with code context
///
/// # Example
/// ```javascript
/// const { getCallSitesWithContext, buildIndex } = require('infiniloom-node');
///
/// buildIndex('./my-repo');
/// const sites = getCallSitesWithContext('./my-repo', 'authenticate', {
///   linesBefore: 5,
///   linesAfter: 5
/// });
/// ```
#[napi]
pub fn get_call_sites_with_context(
    path: Option<String>,
    symbol_name: Option<String>,
    options: Option<CallSitesContextOptions>,
) -> Result<Vec<CallSiteWithContext>> {
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

    let lines_before = options.as_ref().and_then(|o| o.lines_before).unwrap_or(3) as usize;
    let lines_after = options.as_ref().and_then(|o| o.lines_after).unwrap_or(3) as usize;

    let mut call_sites: Vec<CallSiteWithContext> = Vec::new();
    let mut seen_sites: HashSet<(String, u32, u32, u32)> = HashSet::new();
    let mut file_cache: HashMap<String, Vec<String>> = HashMap::new();

    for sym in index.find_symbols(&symbol_name) {
        let callee_id = sym.id.as_u32();

        for caller_id in graph.get_callers(callee_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                let file_path = index
                    .get_file_by_id(caller_sym.file_id.as_u32())
                    .map(|f| f.path.clone())
                    .unwrap_or_else(|| "<unknown>".to_owned());

                let (call_line, call_col) = common_find_call_site_in_body(
                    &path_buf,
                    &file_path,
                    caller_sym.span.start_line,
                    caller_sym.span.end_line,
                    &symbol_name,
                    &mut file_cache,
                );

                let site_key = (file_path.clone(), call_line, caller_id, callee_id);
                if !seen_sites.insert(site_key) {
                    continue;
                }

                let (context, context_start, context_end) = common_get_line_context(
                    &path_buf,
                    &file_path,
                    call_line,
                    lines_before,
                    lines_after,
                    &mut file_cache,
                );

                call_sites.push(CallSiteWithContext {
                    caller: caller_sym.name.clone(),
                    callee: sym.name.clone(),
                    file: file_path,
                    line: call_line,
                    column: call_col,
                    caller_id,
                    callee_id,
                    context,
                    context_start_line: context_start,
                    context_end_line: context_end,
                });
            }
        }
    }

    call_sites.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(call_sites)
}

// ============================================================================
// Async Versions
// ============================================================================

/// Async version of getSymbolsInFile
#[napi]
pub async fn get_symbols_in_file_async(
    path: String,
    file_path: String,
    filter: Option<SymbolFilter>,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_symbols_in_file(path, file_path, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getSymbolSource
#[napi]
pub async fn get_symbol_source_async(
    path: Option<String>,
    symbol_name: Option<String>,
    file_path: Option<String>,
) -> Result<SymbolSourceResult> {
    tokio::task::spawn_blocking(move || get_symbol_source(path, symbol_name, file_path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getChangedSymbols
#[napi]
pub async fn get_changed_symbols_async(
    path: String,
    from_ref: String,
    to_ref: String,
) -> Result<Vec<SymbolInfo>> {
    tokio::task::spawn_blocking(move || get_changed_symbols(path, from_ref, to_ref))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getTestsForFile
#[napi]
pub async fn get_tests_for_file_async(path: String, file_path: String) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || get_tests_for_file(path, file_path))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallSites
#[napi]
pub async fn get_call_sites_async(
    path: Option<String>,
    symbol_name: Option<String>,
) -> Result<Vec<CallSite>> {
    tokio::task::spawn_blocking(move || get_call_sites(path, symbol_name))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getChangedSymbolsFiltered
#[napi]
pub async fn get_changed_symbols_filtered_async(
    path: String,
    from_ref: String,
    to_ref: String,
    filter: Option<ChangedSymbolsFilter>,
) -> Result<Vec<ChangedSymbolInfo>> {
    tokio::task::spawn_blocking(move || get_changed_symbols_filtered(path, from_ref, to_ref, filter))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getTransitiveCallers
#[napi]
pub async fn get_transitive_callers_async(
    path: Option<String>,
    symbol_name: Option<String>,
    options: Option<TransitiveCallersOptions>,
) -> Result<Vec<TransitiveCallerInfo>> {
    tokio::task::spawn_blocking(move || get_transitive_callers(path, symbol_name, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Async version of getCallSitesWithContext
#[napi]
pub async fn get_call_sites_with_context_async(
    path: Option<String>,
    symbol_name: Option<String>,
    options: Option<CallSitesContextOptions>,
) -> Result<Vec<CallSiteWithContext>> {
    tokio::task::spawn_blocking(move || get_call_sites_with_context(path, symbol_name, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}
