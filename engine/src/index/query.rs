//! Call graph query API
//!
//! High-level functions for querying call relationships between symbols.
//! Used by both Python and Node.js bindings.

use super::types::{DepGraph, IndexSymbol, IndexSymbolKind, SymbolIndex, Visibility};
use serde::Serialize;

/// Information about a symbol, returned from call graph queries
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    /// Symbol ID
    pub id: u32,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// File path containing the symbol
    pub file: String,
    /// Start line number
    pub line: u32,
    /// End line number
    pub end_line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Visibility (public, private, etc.)
    pub visibility: String,
}

/// A reference location in the codebase
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceInfo {
    /// Symbol making the reference
    pub symbol: SymbolInfo,
    /// Reference kind (call, import, inherit, implement)
    pub kind: String,
}

/// An edge in the call graph
#[derive(Debug, Clone, Serialize)]
pub struct CallGraphEdge {
    /// Caller symbol ID
    pub caller_id: u32,
    /// Callee symbol ID
    pub callee_id: u32,
    /// Caller symbol name
    pub caller: String,
    /// Callee symbol name
    pub callee: String,
    /// File containing the call site
    pub file: String,
    /// Line number of the call
    pub line: u32,
}

/// Complete call graph with nodes and edges
#[derive(Debug, Clone, Serialize)]
pub struct CallGraph {
    /// All symbols (nodes)
    pub nodes: Vec<SymbolInfo>,
    /// Call relationships (edges)
    pub edges: Vec<CallGraphEdge>,
    /// Summary statistics
    pub stats: CallGraphStats,
}

/// Call graph statistics
#[derive(Debug, Clone, Serialize)]
pub struct CallGraphStats {
    /// Total number of symbols
    pub total_symbols: usize,
    /// Total number of call edges
    pub total_calls: usize,
    /// Number of functions/methods
    pub functions: usize,
    /// Number of classes/structs
    pub classes: usize,
}

impl SymbolInfo {
    /// Create SymbolInfo from an IndexSymbol
    pub fn from_index_symbol(sym: &IndexSymbol, index: &SymbolIndex) -> Self {
        let file_path = index
            .get_file_by_id(sym.file_id.as_u32())
            .map(|f| f.path.clone())
            .unwrap_or_else(|| "<unknown>".to_owned());

        Self {
            id: sym.id.as_u32(),
            name: sym.name.clone(),
            kind: format_symbol_kind(sym.kind),
            file: file_path,
            line: sym.span.start_line,
            end_line: sym.span.end_line,
            signature: sym.signature.clone(),
            visibility: format_visibility(sym.visibility),
        }
    }
}

/// Find a symbol by name and return its info
///
/// Deduplicates results by file path and line number to avoid returning
/// the same symbol multiple times (e.g., export + declaration).
pub fn find_symbol(index: &SymbolIndex, name: &str) -> Vec<SymbolInfo> {
    let mut results: Vec<SymbolInfo> = index
        .find_symbols(name)
        .into_iter()
        .map(|sym| SymbolInfo::from_index_symbol(sym, index))
        .collect();

    // Deduplicate by (file, line) to avoid returning export+declaration as separate entries
    results.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    results.dedup_by(|a, b| a.file == b.file && a.line == b.line);

    results
}

/// Get all callers of a symbol by name
///
/// Returns symbols that call any symbol with the given name.
pub fn get_callers_by_name(index: &SymbolIndex, graph: &DepGraph, name: &str) -> Vec<SymbolInfo> {
    let mut callers = Vec::new();

    // Find all symbols with this name
    for sym in index.find_symbols(name) {
        let symbol_id = sym.id.as_u32();

        // Get callers from the dependency graph
        for caller_id in graph.get_callers(symbol_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                callers.push(SymbolInfo::from_index_symbol(caller_sym, index));
            }
        }
    }

    // Deduplicate by symbol ID
    callers.sort_by_key(|s| s.id);
    callers.dedup_by_key(|s| s.id);

    callers
}

/// Get all callees of a symbol by name
///
/// Returns symbols that are called by any symbol with the given name.
pub fn get_callees_by_name(index: &SymbolIndex, graph: &DepGraph, name: &str) -> Vec<SymbolInfo> {
    let mut callees = Vec::new();

    // Find all symbols with this name
    for sym in index.find_symbols(name) {
        let symbol_id = sym.id.as_u32();

        // Get callees from the dependency graph
        for callee_id in graph.get_callees(symbol_id) {
            if let Some(callee_sym) = index.get_symbol(callee_id) {
                callees.push(SymbolInfo::from_index_symbol(callee_sym, index));
            }
        }
    }

    // Deduplicate by symbol ID
    callees.sort_by_key(|s| s.id);
    callees.dedup_by_key(|s| s.id);

    callees
}

/// Get all references to a symbol by name
///
/// Returns symbols that reference any symbol with the given name
/// (includes calls, imports, inheritance, and implementations).
pub fn get_references_by_name(
    index: &SymbolIndex,
    graph: &DepGraph,
    name: &str,
) -> Vec<ReferenceInfo> {
    let mut references = Vec::new();

    // Find all symbols with this name
    for sym in index.find_symbols(name) {
        let symbol_id = sym.id.as_u32();

        // Get callers (call references)
        for caller_id in graph.get_callers(symbol_id) {
            if let Some(caller_sym) = index.get_symbol(caller_id) {
                references.push(ReferenceInfo {
                    symbol: SymbolInfo::from_index_symbol(caller_sym, index),
                    kind: "call".to_owned(),
                });
            }
        }

        // Get referencers (symbol_ref - may include imports/inheritance)
        for ref_id in graph.get_referencers(symbol_id) {
            if let Some(ref_sym) = index.get_symbol(ref_id) {
                // Avoid duplicates with callers
                if !graph.get_callers(symbol_id).contains(&ref_id) {
                    references.push(ReferenceInfo {
                        symbol: SymbolInfo::from_index_symbol(ref_sym, index),
                        kind: "reference".to_owned(),
                    });
                }
            }
        }
    }

    // Deduplicate by symbol ID
    references.sort_by_key(|r| r.symbol.id);
    references.dedup_by_key(|r| r.symbol.id);

    references
}

/// Get the complete call graph
///
/// Returns all symbols (nodes) and call relationships (edges).
/// For large codebases, consider using `get_call_graph_filtered` with limits.
pub fn get_call_graph(index: &SymbolIndex, graph: &DepGraph) -> CallGraph {
    get_call_graph_filtered(index, graph, None, None)
}

/// Get a filtered call graph
///
/// Args:
///   - `max_nodes`: Optional limit on number of symbols returned
///   - `max_edges`: Optional limit on number of edges returned
pub fn get_call_graph_filtered(
    index: &SymbolIndex,
    graph: &DepGraph,
    max_nodes: Option<usize>,
    max_edges: Option<usize>,
) -> CallGraph {
    // Bug #5 fix: When only max_edges is specified, limit nodes to those that appear in edges
    // This ensures users get a small, focused graph rather than all nodes with limited edges

    // First, collect all edges and apply edge limit
    let mut edges: Vec<CallGraphEdge> = graph
        .calls
        .iter()
        .filter_map(|&(caller_id, callee_id)| {
            let caller_sym = index.get_symbol(caller_id)?;
            let callee_sym = index.get_symbol(callee_id)?;

            let file_path = index
                .get_file_by_id(caller_sym.file_id.as_u32())
                .map(|f| f.path.clone())
                .unwrap_or_else(|| "<unknown>".to_owned());

            Some(CallGraphEdge {
                caller_id,
                callee_id,
                caller: caller_sym.name.clone(),
                callee: callee_sym.name.clone(),
                file: file_path,
                line: caller_sym.span.start_line,
            })
        })
        .collect();

    // Apply edge limit first (before node filtering for more intuitive behavior)
    if let Some(limit) = max_edges {
        edges.truncate(limit);
    }

    // Collect node IDs that appear in the (possibly limited) edges
    let edge_node_ids: std::collections::HashSet<u32> = edges
        .iter()
        .flat_map(|e| [e.caller_id, e.callee_id])
        .collect();

    // Collect nodes - when max_edges is specified without max_nodes, only include nodes from edges
    let mut nodes: Vec<SymbolInfo> = if max_edges.is_some() && max_nodes.is_none() {
        // Only include nodes that appear in the limited edges
        index
            .symbols
            .iter()
            .filter(|sym| edge_node_ids.contains(&sym.id.as_u32()))
            .map(|sym| SymbolInfo::from_index_symbol(sym, index))
            .collect()
    } else {
        // Include all nodes, then optionally truncate
        index
            .symbols
            .iter()
            .map(|sym| SymbolInfo::from_index_symbol(sym, index))
            .collect()
    };

    // Apply node limit if specified
    if let Some(limit) = max_nodes {
        nodes.truncate(limit);

        // When max_nodes is applied, also filter edges to only include those between limited nodes
        let node_ids: std::collections::HashSet<u32> = nodes.iter().map(|n| n.id).collect();
        edges.retain(|e| node_ids.contains(&e.caller_id) && node_ids.contains(&e.callee_id));
    }

    // Calculate statistics
    let functions = nodes
        .iter()
        .filter(|n| n.kind == "function" || n.kind == "method")
        .count();
    let classes = nodes
        .iter()
        .filter(|n| n.kind == "class" || n.kind == "struct")
        .count();

    let stats =
        CallGraphStats { total_symbols: nodes.len(), total_calls: edges.len(), functions, classes };

    CallGraph { nodes, edges, stats }
}

/// Get callers of a symbol by its ID
pub fn get_callers_by_id(index: &SymbolIndex, graph: &DepGraph, symbol_id: u32) -> Vec<SymbolInfo> {
    graph
        .get_callers(symbol_id)
        .into_iter()
        .filter_map(|id| index.get_symbol(id))
        .map(|sym| SymbolInfo::from_index_symbol(sym, index))
        .collect()
}

/// Get callees of a symbol by its ID
pub fn get_callees_by_id(index: &SymbolIndex, graph: &DepGraph, symbol_id: u32) -> Vec<SymbolInfo> {
    graph
        .get_callees(symbol_id)
        .into_iter()
        .filter_map(|id| index.get_symbol(id))
        .map(|sym| SymbolInfo::from_index_symbol(sym, index))
        .collect()
}

// Helper functions

fn format_symbol_kind(kind: IndexSymbolKind) -> String {
    match kind {
        IndexSymbolKind::Function => "function",
        IndexSymbolKind::Method => "method",
        IndexSymbolKind::Class => "class",
        IndexSymbolKind::Struct => "struct",
        IndexSymbolKind::Interface => "interface",
        IndexSymbolKind::Trait => "trait",
        IndexSymbolKind::Enum => "enum",
        IndexSymbolKind::Constant => "constant",
        IndexSymbolKind::Variable => "variable",
        IndexSymbolKind::Module => "module",
        IndexSymbolKind::Import => "import",
        IndexSymbolKind::Export => "export",
        IndexSymbolKind::TypeAlias => "type_alias",
        IndexSymbolKind::Macro => "macro",
    }
    .to_owned()
}

fn format_visibility(vis: Visibility) -> String {
    match vis {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Protected => "protected",
        Visibility::Internal => "internal",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{FileEntry, FileId, Language, Span, SymbolId};

    fn create_test_index() -> (SymbolIndex, DepGraph) {
        let mut index = SymbolIndex::default();

        // Add test file
        index.files.push(FileEntry {
            id: FileId::new(0),
            path: "test.py".to_string(),
            language: Language::Python,
            symbols: 0..2,
            imports: vec![],
            content_hash: [0u8; 32],
            lines: 25,
            tokens: 100,
        });

        // Add test symbols
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(0),
            name: "main".to_string(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span { start_line: 1, start_col: 0, end_line: 10, end_col: 0 },
            signature: Some("def main()".to_string()),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });

        index.symbols.push(IndexSymbol {
            id: SymbolId::new(1),
            name: "helper".to_string(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span { start_line: 12, start_col: 0, end_line: 20, end_col: 0 },
            signature: Some("def helper()".to_string()),
            parent: None,
            visibility: Visibility::Private,
            docstring: None,
        });

        // Build name index
        index.symbols_by_name.insert("main".to_string(), vec![0]);
        index.symbols_by_name.insert("helper".to_string(), vec![1]);

        // Create dependency graph with call edge: main -> helper
        let mut graph = DepGraph::new();
        graph.add_call(0, 1); // main calls helper

        (index, graph)
    }

    #[test]
    fn test_find_symbol() {
        let (index, _graph) = create_test_index();

        let results = find_symbol(&index, "main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "main");
        assert_eq!(results[0].kind, "function");
        assert_eq!(results[0].file, "test.py");
    }

    #[test]
    fn test_get_callers() {
        let (index, graph) = create_test_index();

        // helper is called by main
        let callers = get_callers_by_name(&index, &graph, "helper");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "main");
    }

    #[test]
    fn test_get_callees() {
        let (index, graph) = create_test_index();

        // main calls helper
        let callees = get_callees_by_name(&index, &graph, "main");
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "helper");
    }

    #[test]
    fn test_get_call_graph() {
        let (index, graph) = create_test_index();

        let call_graph = get_call_graph(&index, &graph);
        assert_eq!(call_graph.nodes.len(), 2);
        assert_eq!(call_graph.edges.len(), 1);
        assert_eq!(call_graph.stats.functions, 2);

        // Check edge
        assert_eq!(call_graph.edges[0].caller, "main");
        assert_eq!(call_graph.edges[0].callee, "helper");
    }
}
