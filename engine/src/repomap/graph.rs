//! Symbol graph with PageRank computation

use crate::types::{RepoFile, Symbol};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use tracing::trace;

/// A node in the symbol graph
#[derive(Debug, Clone)]
pub(super) struct SymbolNode {
    /// The symbol
    pub symbol: Symbol,
    /// File containing this symbol
    pub file_path: String,
}

/// Type of edge between symbols
#[derive(Debug, Clone, Copy)]
pub(super) enum EdgeType {
    /// Function calls another function
    Calls,
    /// File imports symbol
    Imports,
    /// Class inherits from another class (extends)
    Inherits,
    /// Class implements an interface/trait
    Implements,
}

/// Graph of symbols with reference relationships
pub(super) struct SymbolGraph {
    /// The underlying directed graph
    graph: DiGraph<SymbolNode, EdgeType>,
    /// Map from symbol key ("file:name") to node index
    symbol_indices: HashMap<String, NodeIndex>,
    /// Reverse index: symbol name -> list of full keys (for O(1) name lookups)
    name_to_keys: HashMap<String, Vec<String>>,
}

impl SymbolGraph {
    /// Create a new empty graph
    pub(super) fn new() -> Self {
        Self { graph: DiGraph::new(), symbol_indices: HashMap::new(), name_to_keys: HashMap::new() }
    }

    /// Add all symbols from a file
    pub(super) fn add_file(&mut self, file: &RepoFile) {
        for symbol in &file.symbols {
            let node = SymbolNode { symbol: symbol.clone(), file_path: file.relative_path.clone() };

            let idx = self.graph.add_node(node);
            let key = format!("{}:{}", file.relative_path, symbol.name);
            self.symbol_indices.insert(key.clone(), idx);

            // Build reverse index for O(1) name lookups
            self.name_to_keys
                .entry(symbol.name.clone())
                .or_default()
                .push(key);
        }
    }

    /// Add a reference edge between symbols
    pub(super) fn add_reference(&mut self, from: &str, to: &str, edge_type: EdgeType) {
        let from_idx = self.symbol_indices.get(from);
        let to_idx = self.symbol_indices.get(to);

        match (from_idx, to_idx) {
            (Some(&from_idx), Some(&to_idx)) => {
                self.graph.add_edge(from_idx, to_idx, edge_type);
            },
            (None, _) => {
                trace!("Reference skipped: source symbol '{}' not found", from);
            },
            (_, None) => {
                trace!("Reference skipped: target symbol '{}' not found", to);
            },
        }
    }

    /// Add an edge from a symbol key to a symbol found by name
    /// Useful for inheritance where we only know the target class name, not the full key
    pub(super) fn add_reference_by_name(
        &mut self,
        from_key: &str,
        to_name: &str,
        edge_type: EdgeType,
    ) {
        let from_idx = match self.symbol_indices.get(from_key) {
            Some(&idx) => idx,
            None => {
                trace!("Reference by name skipped: source '{}' not found", from_key);
                return;
            },
        };

        // O(1) lookup via reverse index (was O(n) linear search)
        let to_idx = self
            .name_to_keys
            .get(to_name)
            .and_then(|keys| keys.first())
            .and_then(|key| self.symbol_indices.get(key))
            .copied();

        match to_idx {
            Some(to_idx) => {
                self.graph.add_edge(from_idx, to_idx, edge_type);
            },
            None => {
                trace!("Reference by name skipped: target '{}' not found", to_name);
            },
        }
    }

    /// Compute PageRank scores for all symbols with early convergence detection
    /// Uses parallel computation for large graphs (>100 nodes)
    pub(super) fn compute_pagerank(
        &self,
        damping: f64,
        max_iterations: usize,
    ) -> HashMap<String, f64> {
        use rayon::prelude::*;

        const CONVERGENCE_THRESHOLD: f64 = 1e-6;
        const PARALLEL_THRESHOLD: usize = 100;

        let node_count = self.graph.node_count();
        if node_count == 0 {
            return HashMap::new();
        }

        // Initialize ranks
        let initial_rank = 1.0 / node_count as f64;
        let mut ranks: Vec<f64> = vec![initial_rank; node_count];
        let mut new_ranks: Vec<f64> = vec![0.0; node_count];

        // Pre-compute out-degrees and cache neighbor lists for parallel access
        let out_degrees: Vec<usize> = self
            .graph
            .node_indices()
            .map(|idx| self.graph.neighbors(idx).count())
            .collect();

        // Pre-compute incoming edges for gather-based parallel update
        // incoming_edges[target] = vec![(source, out_degree)]
        let mut incoming_edges: Vec<Vec<(usize, usize)>> = vec![Vec::new(); node_count];
        for node_idx in self.graph.node_indices() {
            let source = node_idx.index();
            let out_deg = out_degrees[source];
            if out_deg > 0 {
                for neighbor in self.graph.neighbors(node_idx) {
                    incoming_edges[neighbor.index()].push((source, out_deg));
                }
            }
        }

        // Use parallel or sequential based on graph size
        let use_parallel = node_count >= PARALLEL_THRESHOLD;

        // Iterative PageRank computation
        for _ in 0..max_iterations {
            let teleport = (1.0 - damping) / node_count as f64;

            // First pass: compute dangling node sum (parallel for large graphs)
            let dangling_sum: f64 = if use_parallel {
                (0..node_count)
                    .into_par_iter()
                    .filter(|&i| out_degrees[i] == 0)
                    .map(|i| ranks[i])
                    .sum()
            } else {
                (0..node_count)
                    .filter(|&i| out_degrees[i] == 0)
                    .map(|i| ranks[i])
                    .sum()
            };

            let dangling_contribution = damping * dangling_sum / node_count as f64;
            let base_rank = teleport + dangling_contribution;

            // Second pass: gather-based rank computation (parallel for large graphs)
            // Each node computes its new rank from incoming edges
            if use_parallel {
                new_ranks = (0..node_count)
                    .into_par_iter()
                    .map(|target| {
                        let incoming_contrib: f64 = incoming_edges[target]
                            .iter()
                            .map(|&(source, out_deg)| damping * ranks[source] / out_deg as f64)
                            .sum();
                        base_rank + incoming_contrib
                    })
                    .collect();
            } else {
                for target in 0..node_count {
                    let incoming_contrib: f64 = incoming_edges[target]
                        .iter()
                        .map(|&(source, out_deg)| damping * ranks[source] / out_deg as f64)
                        .sum();
                    new_ranks[target] = base_rank + incoming_contrib;
                }
            }

            // Check for convergence (parallel for large graphs)
            let diff: f64 = if use_parallel {
                ranks
                    .par_iter()
                    .zip(new_ranks.par_iter())
                    .map(|(old, new)| (old - new).abs())
                    .sum()
            } else {
                ranks
                    .iter()
                    .zip(new_ranks.iter())
                    .map(|(old, new)| (old - new).abs())
                    .sum()
            };

            // Swap ranks
            std::mem::swap(&mut ranks, &mut new_ranks);

            // Early exit if converged
            if diff < CONVERGENCE_THRESHOLD {
                break;
            }
        }

        // Build result map
        let mut result = HashMap::new();
        for (key, &idx) in &self.symbol_indices {
            result.insert(key.clone(), ranks[idx.index()]);
        }
        result
    }

    /// Get top N symbols using pre-computed ranks
    /// Filters out imports and generic accessors to prioritize meaningful symbols
    pub(super) fn get_top_symbols_with_ranks(
        &self,
        ranks: &HashMap<String, f64>,
        n: usize,
    ) -> Vec<&SymbolNode> {
        use crate::types::SymbolKind;

        // Generic accessor/utility methods that don't help understand architecture
        // These are frequently called but not architecturally significant
        // Covers patterns from: Rust, Python, JavaScript/TypeScript, Java, Go, C++, Ruby, C#, etc.
        const GENERIC_ACCESSORS: &[&str] = &[
            // === Size/length (all languages) ===
            "len",
            "length",
            "size",
            "count",
            "is_empty",
            "isempty",
            "capacity",
            // === Getters/setters (Rust, Java, C#, JS) ===
            "get",
            "set",
            "get_mut",
            "get_ref",
            "as_ref",
            "as_mut",
            "as_str",
            "as_bytes",
            "as_slice",
            "as_ptr",
            "getvalue",
            "setvalue",
            "getname",
            "setname",
            // === Property accessors (generic names) ===
            "path",
            "name",
            "id",
            "key",
            "value",
            "data",
            "type",
            "kind",
            "index",
            "parent",
            "children",
            "child",
            "root",
            "node",
            // === Iteration (all languages) ===
            "iter",
            "iter_mut",
            "into_iter",
            "next",
            "peek",
            "has_next",
            "hasnext",
            "each",
            "foreach",
            "for_each", // Ruby, JS, Rust
            "enumerate",
            "zip",
            "chain",
            "cycle",
            // === Collection operations (JS, Python, Ruby, Java) ===
            "push",
            "pop",
            "shift",
            "unshift",
            "append",
            "extend",
            "insert",
            "remove",
            "delete",
            "clear",
            "add",
            "put",
            "first",
            "last",
            "front",
            "back",
            "head",
            "tail",
            "keys",
            "values",
            "items",
            "entries",
            "pairs",
            // === Conversion (all languages) ===
            "into",
            "from",
            "clone",
            "copy",
            "dup",
            "to_string",
            "tostring",
            "to_str",
            "str", // Rust, Python, JS, Go
            "to_owned",
            "to_vec",
            "to_bytes",
            "to_path",
            "to_array",
            "toarray",
            "to_list",
            "tolist",
            "to_dict",
            "todict",
            "to_map",
            "to_json",
            "tojson",
            "from_json",
            "fromjson",
            "to_a",
            "to_s",
            "to_h",
            "to_i",
            "to_f", // Ruby
            "valueof",
            "parse_int",
            "parseint",
            "parse_float",
            "parsefloat",
            // === Comparison/equality (all languages) ===
            "eq",
            "ne",
            "cmp",
            "partial_cmp",
            "hash",
            "hashcode",
            "gethashcode",
            "equals",
            "compare",
            "compareto",
            "compare_to",
            "min",
            "max",
            "clamp",
            "abs",
            // === Common traits/interfaces ===
            "default",
            "drop",
            "deref",
            "deref_mut",
            "finalize",
            "dispose",
            "close",
            "open",
            "read",
            "write",
            "flush",
            "seek", // IO (Go, Java, etc.)
            // === Arithmetic operators ===
            "add",
            "sub",
            "mul",
            "div",
            "rem",
            "neg",
            "sum",
            "product",
            // === Option/Result/Nullable (Rust, Kotlin, Swift, Scala) ===
            "unwrap",
            "expect",
            "ok",
            "err",
            "is_some",
            "is_none",
            "is_ok",
            "is_err",
            "map",
            "and_then",
            "or_else",
            "unwrap_or",
            "unwrap_or_else",
            "unwrap_or_default",
            "filter",
            "filter_map",
            "flatten",
            "take",
            "skip",
            "collect",
            "getordefault",
            "orelse",
            "orelsethrow",
            // === Display/Debug/String representation ===
            "fmt",
            "display",
            "debug",
            "repr",
            "inspect",
            "tostring",
            "string",
            "description", // Go, Swift
            // === Boolean checks ===
            "exists",
            "is_valid",
            "isvalid",
            "is_dir",
            "isdir",
            "is_file",
            "isfile",
            "contains",
            "has",
            "include",
            "includes",
            "member",
            "startswith",
            "starts_with",
            "endswith",
            "ends_with",
            "isnil",
            "is_nil",
            "isnull",
            "is_null",
            "isundefined",
            // === Python dunder methods ===
            "__init__",
            "__str__",
            "__repr__",
            "__len__",
            "__hash__",
            "__eq__",
            "__getitem__",
            "__setitem__",
            "__delitem__",
            "__contains__",
            "__iter__",
            "__next__",
            "__enter__",
            "__exit__",
            "__add__",
            "__sub__",
            "__mul__",
            "__div__",
            "__call__",
            "__bool__",
            "__int__",
            "__float__",
            // === JavaScript/TypeScript specific ===
            "constructor",
            "prototype",
            "apply",
            "call",
            "bind",
            "then",
            "catch",
            "finally",
            "resolve",
            "reject", // Promises
            "getprototypeof",
            "setprototypeof",
            // === Go interfaces ===
            "error",
            "string",
            "len",
            "less",
            "swap", // sort.Interface
            "servehttp",
            "readfrom",
            "writeto",
            // === C++/STL ===
            "begin",
            "end",
            "cbegin",
            "cend",
            "rbegin",
            "rend",
            "empty",
            "reserve",
            "resize",
            "shrink_to_fit",
            "emplace",
            "emplace_back",
            "emplace_front",
            // === Java/Kotlin specific ===
            "getclass",
            "notify",
            "notifyall",
            "wait",
            "iterator",
            "listiterator",
            "spliterator",
            "stream",
            "parallelstream",
            // === Ruby specific ===
            "initialize",
            "method_missing",
            "respond_to",
            "attr_reader",
            "attr_writer",
            "attr_accessor",
            // === Scala specific ===
            "apply",
            "unapply",
            "copy",
            "canequal",
            "productarity",
            "productelement",
            "productprefix",
        ];

        // Entry point patterns get a rank boost (1.5x)
        // Covers entry files and main functions across all 21 supported languages
        const ENTRY_POINT_BOOST: f64 = 1.5;
        let is_entry_point = |path: &str, name: &str| -> bool {
            let path_lower = path.to_lowercase();
            let is_entry_file =
                // Rust
                path_lower.ends_with("main.rs") || path_lower.ends_with("lib.rs") || path_lower.ends_with("mod.rs")
                // Python
                || path_lower.ends_with("main.py") || path_lower.ends_with("__init__.py")
                || path_lower.ends_with("__main__.py") || path_lower.ends_with("app.py")
                || path_lower.ends_with("wsgi.py") || path_lower.ends_with("asgi.py")
                // JavaScript/TypeScript
                || path_lower.ends_with("index.ts") || path_lower.ends_with("index.js")
                || path_lower.ends_with("index.tsx") || path_lower.ends_with("index.jsx")
                || path_lower.ends_with("main.ts") || path_lower.ends_with("main.js")
                || path_lower.ends_with("app.ts") || path_lower.ends_with("app.js")
                || path_lower.ends_with("server.ts") || path_lower.ends_with("server.js")
                // Go
                || path_lower.ends_with("main.go") || path_lower.contains("cmd/")
                // Java/Kotlin
                || path_lower.ends_with("main.java") || path_lower.ends_with("application.java")
                || path_lower.ends_with("main.kt") || path_lower.ends_with("application.kt")
                // C/C++
                || path_lower.ends_with("main.c") || path_lower.ends_with("main.cpp")
                || path_lower.ends_with("main.cc") || path_lower.ends_with("main.cxx")
                // C#
                || path_lower.ends_with("program.cs") || path_lower.ends_with("startup.cs")
                // Ruby
                || path_lower.ends_with("application.rb") || path_lower.ends_with("config.ru")
                // PHP
                || path_lower.ends_with("index.php") || path_lower.ends_with("app.php")
                // Swift
                || path_lower.ends_with("main.swift") || path_lower.ends_with("appdelegate.swift")
                // Scala
                || path_lower.ends_with("main.scala") || path_lower.ends_with("application.scala")
                // Elixir
                || path_lower.ends_with("application.ex") || path_lower.ends_with("router.ex")
                // Haskell
                || path_lower.ends_with("main.hs")
                // Clojure
                || path_lower.ends_with("core.clj")
                // Shell
                || path_lower.ends_with("main.sh") || path_lower.ends_with("entrypoint.sh");

            let name_lower = name.to_lowercase();
            let is_entry_name = matches!(
                name_lower.as_str(),
                "main"
                    | "run"
                    | "start"
                    | "init"
                    | "execute"
                    | "launch"
                    | "boot"
                    | "app"
                    | "application"
                    | "server"
                    | "program"
                    | "entrypoint"
                    | "entry_point"
                    | "entry"
                    | "setup"
                    | "configure"
                    | "bootstrap"
            );

            is_entry_file || is_entry_name
        };

        let mut ranked: Vec<_> = self
            .graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                let name = node.symbol.name.as_str();

                // Filter out imports - they're not useful as key symbols
                if node.symbol.kind == SymbolKind::Import {
                    return None;
                }

                // Filter out generic accessor methods (case-insensitive)
                let name_lower = name.to_lowercase();
                if GENERIC_ACCESSORS.iter().any(|&acc| name_lower == acc) {
                    return None;
                }

                let key = format!("{}:{}", node.file_path, node.symbol.name);
                let mut rank = ranks.get(&key).copied().unwrap_or(0.0);

                // Boost entry points
                if is_entry_point(&node.file_path, name) {
                    rank *= ENTRY_POINT_BOOST;
                }

                Some((node, rank))
            })
            .collect();

        // Sort descending by rank. NaN treated as equal (cannot occur in practice)
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ranked.into_iter().take(n).map(|(node, _)| node).collect()
    }

    /// Get top N symbols by PageRank (computes ranks internally)
    #[cfg(test)]
    pub(super) fn get_top_symbols(&self, n: usize) -> Vec<&SymbolNode> {
        let ranks = self.compute_pagerank(0.85, 100);
        self.get_top_symbols_with_ranks(&ranks, n)
    }

    /// Get number of nodes
    #[cfg(test)]
    pub(super) fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

impl Default for SymbolGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolKind;

    #[test]
    fn test_empty_graph() {
        let graph = SymbolGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert!(graph.get_top_symbols(10).is_empty());
    }

    #[test]
    fn test_add_symbols() {
        let mut graph = SymbolGraph::new();

        let file = RepoFile {
            path: "/test/main.py".into(),
            relative_path: "main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![
                Symbol::new("main", SymbolKind::Function),
                Symbol::new("helper", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        };

        graph.add_file(&file);
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_pagerank() {
        let mut graph = SymbolGraph::new();

        // Create a simple graph: A -> B -> C, A -> C
        let file = RepoFile {
            path: "/test/main.py".into(),
            relative_path: "main.py".to_owned(),
            language: Some("python".to_owned()),
            size_bytes: 100,
            token_count: Default::default(),
            symbols: vec![
                Symbol::new("a", SymbolKind::Function),
                Symbol::new("b", SymbolKind::Function),
                Symbol::new("c", SymbolKind::Function),
            ],
            importance: 0.5,
            content: None,
        };

        graph.add_file(&file);
        graph.add_reference("main.py:a", "main.py:b", EdgeType::Calls);
        graph.add_reference("main.py:b", "main.py:c", EdgeType::Calls);
        graph.add_reference("main.py:a", "main.py:c", EdgeType::Calls);

        let ranks = graph.compute_pagerank(0.85, 100);

        // C should have highest rank (most incoming edges)
        let rank_a = ranks.get("main.py:a").unwrap();
        let rank_c = ranks.get("main.py:c").unwrap();
        assert!(rank_c > rank_a);
    }
}
