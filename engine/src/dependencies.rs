//! Full AST-based dependency resolution and import graph
//!
//! This module provides comprehensive dependency analysis using actual AST parsing
//! to build accurate import graphs, call graphs, and symbol reference tracking.

use crate::types::{RepoFile, Repository, SymbolKind};
use once_cell::sync::Lazy;
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// Cached regex patterns for import extraction (compiled once, reused)
static JS_REQUIRE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).expect("Invalid require regex")
});
static JS_IMPORT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:from|import)\s*\(\s*['"]([^'"]+)['"]\s*\)|from\s+['"]([^'"]+)['"]"#)
        .expect("Invalid import regex")
});

/// A node in the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// File path (relative)
    pub path: String,
    /// Module name (derived from path)
    pub module_name: String,
    /// Symbols exported from this module
    pub exports: Vec<String>,
    /// Token count for this module
    pub tokens: u32,
    /// Importance score (0.0 - 1.0)
    pub importance: f64,
}

/// Types of dependencies between modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyType {
    /// Direct import/require
    Import,
    /// Re-export (export from)
    Reexport,
    /// Type-only import (TypeScript)
    TypeImport,
    /// Dynamic import
    DynamicImport,
    /// Inheritance/implementation
    Inheritance,
}

/// An edge in the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// Type of dependency
    pub dep_type: DependencyType,
    /// Imported symbols (empty means wildcard/all)
    pub symbols: Vec<String>,
    /// Source line number
    pub line: u32,
    /// Weight (for ranking)
    pub weight: f64,
}

/// A resolved import target
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Source file path
    pub from_path: String,
    /// Target file path (resolved)
    pub to_path: Option<String>,
    /// Original import specifier
    pub specifier: String,
    /// Imported symbols
    pub symbols: Vec<String>,
    /// Import type
    pub import_type: DependencyType,
    /// Line number
    pub line: u32,
    /// Whether this is an external package
    pub is_external: bool,
}

/// Symbol reference for call graph analysis
#[derive(Debug, Clone)]
pub struct SymbolReference {
    /// Symbol being referenced
    pub symbol_name: String,
    /// File containing the reference
    pub file_path: String,
    /// Line number
    pub line: u32,
    /// Context (e.g., "call", "type", "assignment")
    pub context: ReferenceContext,
}

/// Context of a symbol reference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceContext {
    /// Function call
    Call,
    /// Type annotation
    Type,
    /// Assignment target
    Assignment,
    /// Parameter
    Parameter,
    /// Return type
    Return,
    /// Generic reference
    Reference,
}

/// Full dependency graph for a repository
pub struct DependencyGraph {
    /// Graph of file-level dependencies
    graph: DiGraph<DependencyNode, DependencyEdge>,
    /// File path to node index mapping
    path_to_node: HashMap<String, NodeIndex>,
    /// Module name to file path mapping
    module_to_path: HashMap<String, String>,
    /// Symbol to file path mapping (for cross-file references)
    symbol_to_file: HashMap<String, String>,
    /// All resolved imports
    imports: Vec<ResolvedImport>,
    /// External dependencies (packages not in repo)
    external_deps: HashSet<String>,
    /// Circular dependency groups
    circular_deps: Vec<Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            path_to_node: HashMap::new(),
            module_to_path: HashMap::new(),
            symbol_to_file: HashMap::new(),
            imports: Vec::new(),
            external_deps: HashSet::new(),
            circular_deps: Vec::new(),
        }
    }

    /// Build dependency graph from a repository
    pub fn build(repo: &Repository) -> Self {
        let mut graph = Self::new();

        // Phase 1: Add all files as nodes
        for file in &repo.files {
            graph.add_file(file);
        }

        // Phase 2: Extract and resolve imports
        for file in &repo.files {
            graph.extract_imports(file, repo);
        }

        // Phase 3: Detect circular dependencies
        graph.detect_cycles();

        // Phase 4: Compute importance scores
        graph.compute_importance();

        graph
    }

    /// Add a file to the graph
    fn add_file(&mut self, file: &RepoFile) {
        let module_name = Self::path_to_module(&file.relative_path);

        // Collect exported symbols
        let exports: Vec<String> = file
            .symbols
            .iter()
            .filter(|s| s.kind != SymbolKind::Import)
            .map(|s| s.name.clone())
            .collect();

        // Index symbols for cross-file reference resolution
        for export in &exports {
            let key = format!("{}::{}", module_name, export);
            self.symbol_to_file.insert(key, file.relative_path.clone());
            // Also index by simple name for ambiguous resolution
            self.symbol_to_file
                .entry(export.clone())
                .or_insert_with(|| file.relative_path.clone());
        }

        let node = DependencyNode {
            path: file.relative_path.clone(),
            module_name: module_name.clone(),
            exports,
            tokens: file.token_count.claude,
            importance: file.importance as f64,
        };

        let idx = self.graph.add_node(node);
        self.path_to_node.insert(file.relative_path.clone(), idx);
        self.module_to_path
            .insert(module_name, file.relative_path.clone());
    }

    /// Extract and resolve imports from a file
    fn extract_imports(&mut self, file: &RepoFile, repo: &Repository) {
        // Process symbol-based imports (from parser)
        for symbol in &file.symbols {
            if symbol.kind != SymbolKind::Import {
                continue;
            }

            // Parse the import statement
            let parsed = self.parse_import_statement(&symbol.name);

            for import in parsed {
                // Try to resolve the import target
                let resolved = self.resolve_import(&import, file, repo);

                if let Some(target_path) = &resolved.to_path {
                    // Add edge to graph
                    if let (Some(&from_idx), Some(&to_idx)) = (
                        self.path_to_node.get(&file.relative_path),
                        self.path_to_node.get(target_path),
                    ) {
                        let edge = DependencyEdge {
                            dep_type: resolved.import_type,
                            symbols: resolved.symbols.clone(),
                            line: resolved.line,
                            weight: 1.0,
                        };
                        self.graph.add_edge(from_idx, to_idx, edge);
                    }
                } else if resolved.is_external {
                    self.external_deps.insert(resolved.specifier.clone());
                }

                self.imports.push(resolved);
            }
        }

        // Fallback: scan file content for CommonJS require() and ESM imports
        // This catches cases where tree-sitter didn't extract imports (e.g., dynamic requires)
        if let Some(content) = &file.content {
            self.extract_imports_from_content(content, file);
        }
    }

    /// Extract imports by scanning file content with regex
    /// Catches CommonJS require(), dynamic imports, and any missed ESM imports
    fn extract_imports_from_content(&mut self, content: &str, file: &RepoFile) {
        let is_js = file
            .language
            .as_deref()
            .map(|l| matches!(l, "javascript" | "typescript" | "jsx" | "tsx"))
            .unwrap_or(false);

        if !is_js {
            return;
        }

        let mut found_packages = HashSet::new();

        // Use cached regex for require('pkg') and require("pkg")
        for cap in JS_REQUIRE_RE.captures_iter(content) {
            if let Some(pkg) = cap.get(1) {
                let specifier = pkg.as_str();
                if Self::is_external_specifier(specifier) {
                    let pkg_name = Self::extract_package_name(specifier);
                    found_packages.insert(pkg_name);
                }
            }
        }

        // Use cached regex for import ... from 'pkg' and import('pkg')
        for cap in JS_IMPORT_RE.captures_iter(content) {
            // Check both capture groups (dynamic import vs from)
            let specifier = cap.get(1).or_else(|| cap.get(2));
            if let Some(pkg) = specifier {
                let spec = pkg.as_str();
                if Self::is_external_specifier(spec) {
                    let pkg_name = Self::extract_package_name(spec);
                    found_packages.insert(pkg_name);
                }
            }
        }

        // Add found packages to external deps (filtered for validity)
        for pkg in found_packages {
            if Self::is_valid_package_name(&pkg) {
                self.external_deps.insert(pkg);
            }
        }
    }

    /// Check if an import specifier is external (not a relative path)
    fn is_external_specifier(spec: &str) -> bool {
        !spec.starts_with('.') && !spec.starts_with('/')
    }

    /// Extract the package name from a specifier (handles scoped packages)
    fn extract_package_name(spec: &str) -> String {
        if spec.starts_with('@') {
            // Scoped package: @scope/package/path -> @scope/package
            let parts: Vec<&str> = spec.splitn(3, '/').collect();
            if parts.len() >= 2 {
                format!("{}/{}", parts[0], parts[1])
            } else {
                spec.to_owned()
            }
        } else {
            // Regular package: package/path -> package
            spec.split('/').next().unwrap_or(spec).to_owned()
        }
    }

    /// Validate a package name (filter out false positives from regex)
    fn is_valid_package_name(name: &str) -> bool {
        // Skip empty, contains whitespace, or special characters
        if name.is_empty() || name.contains(' ') || name.contains('+') {
            return false;
        }
        // Skip names that look like URLs or paths
        if name.contains("://") || name.starts_with('/') {
            return false;
        }
        // Skip very short names that are likely false positives
        if name.len() < 2 {
            return false;
        }
        // Must start with valid chars (letter, @, or _)
        // Safe: we already checked name.is_empty() above
        let Some(first) = name.chars().next() else {
            return false;
        };
        if !first.is_ascii_alphabetic() && first != '@' && first != '_' {
            return false;
        }
        true
    }

    /// Parse an import statement into structured data
    fn parse_import_statement(&self, import_text: &str) -> Vec<ParsedImport> {
        let mut imports = Vec::new();
        let text = import_text.trim();

        // JS/TS imports also start with "import"; detect quoted specifiers first.
        let js_specifier = if text.starts_with("import ") || text.contains("require(") {
            Self::extract_string_literal(text)
        } else {
            None
        };

        // JavaScript/TypeScript: import { x } from 'y' / require('y')
        if text.contains("require(") || (text.starts_with("import ") && js_specifier.is_some()) {
            if let Some(spec) = js_specifier {
                let symbols = Self::extract_import_symbols(text);
                let import_type = if text.contains("type ") {
                    DependencyType::TypeImport
                } else if text.contains("import(") {
                    DependencyType::DynamicImport
                } else {
                    DependencyType::Import
                };
                imports.push(ParsedImport { specifier: spec, symbols, import_type });
            }
        }
        // Python: import x / from x import y
        else if text.starts_with("import ") {
            let module = text.trim_start_matches("import ").trim();
            // Handle "import x as y"
            let module = module.split(" as ").next().unwrap_or(module);
            // Handle "import x, y, z"
            for m in module.split(',') {
                imports.push(ParsedImport {
                    specifier: m.trim().to_owned(),
                    symbols: vec![],
                    import_type: DependencyType::Import,
                });
            }
        } else if text.starts_with("from ") {
            // from x import y, z
            if let Some(rest) = text.strip_prefix("from ") {
                let parts: Vec<&str> = rest.splitn(2, " import ").collect();
                if parts.len() == 2 {
                    let module = parts[0].trim();
                    let symbols: Vec<String> = parts[1]
                        .split(',')
                        .map(|s| s.split(" as ").next().unwrap_or(s).trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                    imports.push(ParsedImport {
                        specifier: module.to_owned(),
                        symbols,
                        import_type: DependencyType::Import,
                    });
                }
            }
        }
        // Rust: use x::y
        else if text.starts_with("use ") {
            let path = text.trim_start_matches("use ").trim_end_matches(';').trim();
            // Handle "use x::{y, z}"
            if path.contains("::") {
                let parts: Vec<&str> = path.rsplitn(2, "::").collect();
                let base = if parts.len() == 2 { parts[1] } else { "" };
                let symbols_part = parts[0].trim_matches(|c| c == '{' || c == '}');
                let symbols: Vec<String> = symbols_part
                    .split(',')
                    .map(|s| s.split(" as ").next().unwrap_or(s).trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                imports.push(ParsedImport {
                    specifier: base.to_owned(),
                    symbols,
                    import_type: DependencyType::Import,
                });
            } else {
                imports.push(ParsedImport {
                    specifier: path.to_owned(),
                    symbols: vec![],
                    import_type: DependencyType::Import,
                });
            }
        }
        // Go: import "x" or import ( "x" "y" )
        else if text.contains("import") {
            // Extract all quoted strings
            let mut i = 0;
            let chars: Vec<char> = text.chars().collect();
            while i < chars.len() {
                if chars[i] == '"' {
                    let start = i + 1;
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        i += 1;
                    }
                    if i < chars.len() {
                        let spec: String = chars[start..i].iter().collect();
                        imports.push(ParsedImport {
                            specifier: spec,
                            symbols: vec![],
                            import_type: DependencyType::Import,
                        });
                    }
                }
                i += 1;
            }
        }

        imports
    }

    /// Extract string literal from import statement
    fn extract_string_literal(text: &str) -> Option<String> {
        // Find first quoted string
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                let start = i + 1;
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                if i < chars.len() {
                    return Some(chars[start..i].iter().collect());
                }
            }
            i += 1;
        }
        None
    }

    /// Extract imported symbols from import statement
    fn extract_import_symbols(text: &str) -> Vec<String> {
        let mut symbols = Vec::new();

        // Look for { ... }
        if let Some(start) = text.find('{') {
            if let Some(end) = text.find('}') {
                let inner = &text[start + 1..end];
                for sym in inner.split(',') {
                    let sym = sym.split(" as ").next().unwrap_or(sym).trim();
                    if !sym.is_empty() && sym != "type" {
                        symbols.push(sym.to_owned());
                    }
                }
            }
        }
        // Default import: import X from '...'
        else if text.starts_with("import ") {
            let after_import = text.trim_start_matches("import ");
            if let Some(default_name) = after_import.split_whitespace().next() {
                if default_name != "type" && default_name != "*" && !default_name.starts_with('{') {
                    symbols.push(default_name.to_owned());
                }
            }
        }

        symbols
    }

    /// Resolve an import to a file path
    fn resolve_import(
        &self,
        import: &ParsedImport,
        from_file: &RepoFile,
        repo: &Repository,
    ) -> ResolvedImport {
        let specifier = &import.specifier;

        // Check if external package
        if self.is_external_import(specifier) {
            return ResolvedImport {
                from_path: from_file.relative_path.clone(),
                to_path: None,
                specifier: specifier.clone(),
                symbols: import.symbols.clone(),
                import_type: import.import_type,
                line: 0,
                is_external: true,
            };
        }

        // Try to resolve relative imports
        let base_dir = Path::new(&from_file.relative_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Possible resolved paths to try
        let candidates =
            self.generate_resolution_candidates(specifier, &base_dir, &from_file.language);

        // Find first existing file
        for candidate in candidates {
            if repo.files.iter().any(|f| f.relative_path == candidate) {
                return ResolvedImport {
                    from_path: from_file.relative_path.clone(),
                    to_path: Some(candidate),
                    specifier: specifier.clone(),
                    symbols: import.symbols.clone(),
                    import_type: import.import_type,
                    line: 0,
                    is_external: false,
                };
            }
        }

        // Check module name mapping
        if let Some(path) = self.module_to_path.get(specifier) {
            return ResolvedImport {
                from_path: from_file.relative_path.clone(),
                to_path: Some(path.clone()),
                specifier: specifier.clone(),
                symbols: import.symbols.clone(),
                import_type: import.import_type,
                line: 0,
                is_external: false,
            };
        }

        // Unresolved internal import
        ResolvedImport {
            from_path: from_file.relative_path.clone(),
            to_path: None,
            specifier: specifier.clone(),
            symbols: import.symbols.clone(),
            import_type: import.import_type,
            line: 0,
            is_external: false,
        }
    }

    /// Check if import is for an external package
    fn is_external_import(&self, specifier: &str) -> bool {
        // Relative imports are internal
        if specifier.starts_with('.') || specifier.starts_with('/') {
            return false;
        }

        // Known external package prefixes
        let external_prefixes = [
            "react",
            "vue",
            "angular",
            "express",
            "lodash",
            "axios",
            "std",
            "core",
            "alloc",
            "collections", // Rust std
            "fmt",
            "os",
            "io",
            "net",
            "http",
            "sync",
            "context", // Go std
            "java.",
            "javax.",
            "org.apache",
            "com.google", // Java
            "numpy",
            "pandas",
            "torch",
            "tensorflow",
            "sklearn", // Python
        ];

        for prefix in external_prefixes {
            if specifier.starts_with(prefix) {
                return true;
            }
        }

        // Scoped packages (@org/pkg)
        if specifier.starts_with('@') {
            return true;
        }

        // No dots/slashes usually means external
        !specifier.contains('/') && !specifier.contains('\\')
    }

    /// Generate candidate paths for import resolution
    fn generate_resolution_candidates(
        &self,
        specifier: &str,
        base_dir: &str,
        language: &Option<String>,
    ) -> Vec<String> {
        let mut candidates = Vec::new();

        // Language-specific resolution
        let extensions = match language.as_deref() {
            Some("python") => vec!["py", "pyi"],
            Some("javascript") | Some("jsx") => vec!["js", "jsx", "mjs", "cjs", "ts", "tsx"],
            Some("typescript") | Some("tsx") => vec!["ts", "tsx", "js", "jsx"],
            Some("rust") => vec!["rs"],
            Some("go") => vec!["go"],
            Some("java") => vec!["java"],
            _ => vec![""],
        };

        // Relative path resolution
        if specifier.starts_with('.') {
            let resolved = normalize_path(&format!("{}/{}", base_dir, specifier));

            // Try with extensions
            for ext in &extensions {
                if ext.is_empty() {
                    candidates.push(resolved.clone());
                } else {
                    candidates.push(format!("{}.{}", resolved, ext));
                }
            }

            // Try as directory with index file
            for ext in &extensions {
                if !ext.is_empty() {
                    candidates.push(format!("{}/index.{}", resolved, ext));
                    candidates.push(format!("{}/mod.{}", resolved, ext)); // Rust
                    candidates.push(format!("{}/__init__.{}", resolved, ext)); // Python
                }
            }
        } else {
            // Module name resolution
            // Try src/module, lib/module, module directly
            let prefixes = ["src", "lib", "app", "pkg", "internal", ""];

            for prefix in prefixes {
                let base = if prefix.is_empty() {
                    specifier.to_owned()
                } else {
                    format!("{}/{}", prefix, specifier)
                };

                for ext in &extensions {
                    if ext.is_empty() {
                        candidates.push(base.clone());
                    } else {
                        candidates.push(format!("{}.{}", base, ext));
                        candidates.push(format!("{}/index.{}", base, ext));
                        candidates.push(format!("{}/mod.{}", base, ext));
                    }
                }
            }
        }

        candidates
    }

    /// Detect circular dependencies using Tarjan's algorithm
    fn detect_cycles(&mut self) {
        let sccs = tarjan_scc(&self.graph);

        for scc in sccs {
            if scc.len() > 1 {
                // This is a cycle
                let cycle: Vec<String> = scc
                    .iter()
                    .filter_map(|&idx| self.graph.node_weight(idx))
                    .map(|n| n.path.clone())
                    .collect();
                self.circular_deps.push(cycle);
            }
        }
    }

    /// Compute importance scores using PageRank variant
    fn compute_importance(&mut self) {
        let node_count = self.graph.node_count();
        if node_count == 0 {
            return;
        }

        // Initialize with uniform importance
        let initial = 1.0 / node_count as f64;
        let mut importance: Vec<f64> = vec![initial; node_count];
        let mut new_importance: Vec<f64> = vec![0.0; node_count];

        let damping = 0.85;
        let iterations = 30;

        // PageRank iteration
        for _ in 0..iterations {
            let teleport = (1.0 - damping) / node_count as f64;
            new_importance.fill(teleport);

            for node_idx in self.graph.node_indices() {
                let out_degree = self.graph.neighbors(node_idx).count();
                if out_degree > 0 {
                    let contribution = damping * importance[node_idx.index()] / out_degree as f64;
                    for neighbor in self.graph.neighbors(node_idx) {
                        new_importance[neighbor.index()] += contribution;
                    }
                }
            }

            std::mem::swap(&mut importance, &mut new_importance);
        }

        // Normalize and apply to nodes
        let max_importance = importance.iter().cloned().fold(0.0_f64, f64::max);
        if max_importance > 0.0 {
            for (idx, node) in self.graph.node_weights_mut().enumerate() {
                node.importance = importance[idx] / max_importance;
            }
        }
    }

    /// Convert file path to module name
    fn path_to_module(path: &str) -> String {
        let path = path
            .trim_start_matches("src/")
            .trim_start_matches("lib/")
            .trim_start_matches("app/");

        // Remove extension
        let path = if let Some(pos) = path.rfind('.') {
            &path[..pos]
        } else {
            path
        };

        // Convert path separators to module separators
        path.chars()
            .map(|c| if c == '/' || c == '\\' { '.' } else { c })
            .collect::<String>()
            .trim_matches('.')
            .to_owned()
    }

    // Public query methods

    /// Get all files that import from a given file
    pub fn get_importers(&self, file_path: &str) -> Vec<&str> {
        if let Some(&node_idx) = self.path_to_node.get(file_path) {
            self.graph
                .neighbors_directed(node_idx, petgraph::Direction::Incoming)
                .filter_map(|idx| self.graph.node_weight(idx))
                .map(|n| n.path.as_str())
                .collect()
        } else {
            vec![]
        }
    }

    /// Get all files that a given file imports
    pub fn get_imports(&self, file_path: &str) -> Vec<&str> {
        if let Some(&node_idx) = self.path_to_node.get(file_path) {
            self.graph
                .neighbors(node_idx)
                .filter_map(|idx| self.graph.node_weight(idx))
                .map(|n| n.path.as_str())
                .collect()
        } else {
            vec![]
        }
    }

    /// Get all circular dependency groups
    pub fn get_circular_deps(&self) -> &[Vec<String>] {
        &self.circular_deps
    }

    /// Get external dependencies
    pub fn get_external_deps(&self) -> &HashSet<String> {
        &self.external_deps
    }

    /// Get top N most important files by import graph
    pub fn get_most_important(&self, n: usize) -> Vec<(&str, f64)> {
        let mut nodes: Vec<_> = self
            .graph
            .node_weights()
            .map(|n| (n.path.as_str(), n.importance))
            .collect();

        nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        nodes.truncate(n);
        nodes
    }

    /// Get all resolved imports
    pub fn get_all_imports(&self) -> &[ResolvedImport] {
        &self.imports
    }

    /// Get unresolved imports (potential issues)
    pub fn get_unresolved_imports(&self) -> Vec<&ResolvedImport> {
        self.imports
            .iter()
            .filter(|i| i.to_path.is_none() && !i.is_external)
            .collect()
    }

    /// Get summary statistics
    pub fn stats(&self) -> DependencyStats {
        DependencyStats {
            total_files: self.graph.node_count(),
            total_edges: self.graph.edge_count(),
            external_deps: self.external_deps.len(),
            circular_dep_groups: self.circular_deps.len(),
            unresolved_imports: self
                .imports
                .iter()
                .filter(|i| i.to_path.is_none() && !i.is_external)
                .count(),
        }
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for dependency analysis
#[derive(Debug, Clone)]
pub struct DependencyStats {
    pub total_files: usize,
    pub total_edges: usize,
    pub external_deps: usize,
    pub circular_dep_groups: usize,
    pub unresolved_imports: usize,
}

/// Parsed import statement (intermediate representation)
struct ParsedImport {
    specifier: String,
    symbols: Vec<String>,
    import_type: DependencyType,
}

/// Normalize a file path (resolve . and ..)
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();

    for part in path.split('/') {
        match part {
            "." | "" => continue,
            ".." => {
                parts.pop();
            },
            _ => parts.push(part),
        }
    }

    parts.join("/")
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_module() {
        assert_eq!(DependencyGraph::path_to_module("src/foo/bar.py"), "foo.bar");
        assert_eq!(DependencyGraph::path_to_module("lib/utils.rs"), "utils");
        assert_eq!(DependencyGraph::path_to_module("app/main.ts"), "main");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("foo/./bar"), "foo/bar");
        assert_eq!(normalize_path("foo/bar/../baz"), "foo/baz");
        assert_eq!(normalize_path("./foo/bar"), "foo/bar");
    }

    #[test]
    fn test_is_external_import() {
        let graph = DependencyGraph::new();

        assert!(graph.is_external_import("react"));
        assert!(graph.is_external_import("numpy"));
        assert!(graph.is_external_import("@types/node"));

        assert!(!graph.is_external_import("./utils"));
        assert!(!graph.is_external_import("../lib/foo"));
    }

    #[test]
    fn test_build_graph() {
        let repo = Repository::new("test", "/tmp/test");
        // Would need to add files to test more
        let graph = DependencyGraph::build(&repo);

        assert_eq!(graph.stats().total_files, 0);
    }

    #[test]
    fn test_extract_string_literal() {
        assert_eq!(
            DependencyGraph::extract_string_literal("import 'react'"),
            Some("react".to_string())
        );
        assert_eq!(
            DependencyGraph::extract_string_literal("require(\"lodash\")"),
            Some("lodash".to_string())
        );
    }
}
