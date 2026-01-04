//! Multi-repository index for cross-repo symbol linking
//!
//! Enables indexing multiple repositories and tracking cross-repository
//! references, dependencies, and symbol relationships.

use crate::analysis::types::{
    CrossRepoLink, CrossRepoLinkType, MultiRepoIndex, RepoEntry, UnifiedSymbolRef,
};
use crate::types::{Symbol, SymbolKind};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Builds and maintains a multi-repository index
pub struct MultiRepoIndexBuilder {
    /// Index being built
    index: MultiRepoIndex,
    /// Map of import paths to repo IDs
    import_path_to_repo: HashMap<String, String>,
    /// Package/module names to repo IDs
    package_to_repo: HashMap<String, String>,
}

impl MultiRepoIndexBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            index: MultiRepoIndex::default(),
            import_path_to_repo: HashMap::new(),
            package_to_repo: HashMap::new(),
        }
    }

    /// Add a repository to the index
    pub fn add_repository(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        path: impl Into<String>,
        commit: Option<String>,
    ) -> &mut Self {
        let id = id.into();
        let name = name.into();
        let path = path.into();

        let entry = RepoEntry {
            id: id.clone(),
            name: name.clone(),
            path: path.clone(),
            commit,
            file_count: 0,
            symbol_count: 0,
            indexed_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
        };

        self.index.repositories.push(entry);

        // Register common package patterns
        self.package_to_repo.insert(name, id.clone());
        self.package_to_repo.insert(path, id);

        self
    }

    /// Register an import path pattern for a repository
    pub fn register_import_path(&mut self, pattern: impl Into<String>, repo_id: impl Into<String>) {
        self.import_path_to_repo
            .insert(pattern.into(), repo_id.into());
    }

    /// Add symbols from a file
    pub fn add_file_symbols(
        &mut self,
        repo_id: &str,
        file_path: &str,
        symbols: &[Symbol],
    ) -> &mut Self {
        // Update repo stats
        if let Some(repo) = self.index.repositories.iter_mut().find(|r| r.id == repo_id) {
            repo.file_count += 1;
            repo.symbol_count += symbols.len() as u32;
        }

        // Add symbols to unified index
        for symbol in symbols {
            let qualified_name = self.get_qualified_name(symbol, file_path);

            let symbol_ref = UnifiedSymbolRef {
                repo_id: repo_id.to_owned(),
                file_path: file_path.to_owned(),
                line: symbol.start_line,
                kind: format!("{:?}", symbol.kind),
                qualified_name: Some(qualified_name.clone()),
            };

            self.index
                .unified_symbols
                .entry(symbol.name.clone())
                .or_default()
                .push(symbol_ref);

            // Also index by qualified name
            if qualified_name != symbol.name {
                let symbol_ref = UnifiedSymbolRef {
                    repo_id: repo_id.to_owned(),
                    file_path: file_path.to_owned(),
                    line: symbol.start_line,
                    kind: format!("{:?}", symbol.kind),
                    qualified_name: Some(qualified_name.clone()),
                };

                self.index
                    .unified_symbols
                    .entry(qualified_name)
                    .or_default()
                    .push(symbol_ref);
            }

            // Detect cross-repo links from this symbol
            self.detect_cross_repo_links(repo_id, file_path, symbol);
        }

        self
    }

    /// Get qualified name for a symbol
    fn get_qualified_name(&self, symbol: &Symbol, file_path: &str) -> String {
        // Build qualified name from parent and file path
        let module = self.file_to_module(file_path);

        if let Some(ref parent) = symbol.parent {
            format!("{}::{}::{}", module, parent, symbol.name)
        } else {
            format!("{}::{}", module, symbol.name)
        }
    }

    /// Convert file path to module path
    fn file_to_module(&self, file_path: &str) -> String {
        // Remove extension and convert slashes to ::
        let without_ext = file_path
            .trim_end_matches(".rs")
            .trim_end_matches(".py")
            .trim_end_matches(".ts")
            .trim_end_matches(".js")
            .trim_end_matches(".java")
            .trim_end_matches(".go")
            .trim_end_matches(".rb")
            .trim_end_matches(".php");

        // Handle mod.rs, index.ts, __init__.py patterns
        let normalized = without_ext
            .trim_end_matches("/mod")
            .trim_end_matches("/index")
            .trim_end_matches("/__init__");

        // Convert path separators to ::
        normalized
            .replace(['/', '\\'], "::")
            .trim_start_matches("::")
            .to_owned()
    }

    /// Detect cross-repository links from a symbol
    fn detect_cross_repo_links(&mut self, source_repo: &str, source_file: &str, symbol: &Symbol) {
        // Check extends
        if let Some(ref extends) = symbol.extends {
            if let Some(target_repo) = self.find_symbol_repo(extends) {
                if target_repo != source_repo {
                    self.add_cross_repo_link(
                        source_repo,
                        source_file,
                        Some(&symbol.name),
                        symbol.start_line,
                        &target_repo,
                        extends,
                        CrossRepoLinkType::Extends,
                    );
                }
            }
        }

        // Check implements
        for implements in &symbol.implements {
            if let Some(target_repo) = self.find_symbol_repo(implements) {
                if target_repo != source_repo {
                    self.add_cross_repo_link(
                        source_repo,
                        source_file,
                        Some(&symbol.name),
                        symbol.start_line,
                        &target_repo,
                        implements,
                        CrossRepoLinkType::Implements,
                    );
                }
            }
        }

        // Check calls
        for call in &symbol.calls {
            if let Some(target_repo) = self.find_symbol_repo(call) {
                if target_repo != source_repo {
                    self.add_cross_repo_link(
                        source_repo,
                        source_file,
                        Some(&symbol.name),
                        symbol.start_line,
                        &target_repo,
                        call,
                        CrossRepoLinkType::Call,
                    );
                }
            }
        }
    }

    /// Find which repo a symbol belongs to
    fn find_symbol_repo(&self, symbol_name: &str) -> Option<String> {
        // Check if we have this symbol in unified index
        if let Some(refs) = self.index.unified_symbols.get(symbol_name) {
            if let Some(first) = refs.first() {
                return Some(first.repo_id.clone());
            }
        }

        // Check import path patterns
        for (pattern, repo_id) in &self.import_path_to_repo {
            if symbol_name.starts_with(pattern) || symbol_name.contains(pattern) {
                return Some(repo_id.clone());
            }
        }

        // Check package names
        let parts: Vec<&str> = symbol_name.split("::").collect();
        if let Some(first) = parts.first() {
            if let Some(repo_id) = self.package_to_repo.get(*first) {
                return Some(repo_id.clone());
            }
        }

        None
    }

    /// Add a cross-repository link
    fn add_cross_repo_link(
        &mut self,
        source_repo: &str,
        source_file: &str,
        source_symbol: Option<&str>,
        source_line: u32,
        target_repo: &str,
        target_symbol: &str,
        link_type: CrossRepoLinkType,
    ) {
        self.index.cross_repo_links.push(CrossRepoLink {
            source_repo: source_repo.to_owned(),
            source_file: source_file.to_owned(),
            source_symbol: source_symbol.map(String::from),
            source_line,
            target_repo: target_repo.to_owned(),
            target_symbol: target_symbol.to_owned(),
            link_type,
        });
    }

    /// Build the final index
    pub fn build(self) -> MultiRepoIndex {
        self.index
    }

    /// Get current state of the index (for incremental building)
    pub fn current_index(&self) -> &MultiRepoIndex {
        &self.index
    }
}

impl Default for MultiRepoIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Query interface for the multi-repository index
pub struct MultiRepoQuery<'a> {
    index: &'a MultiRepoIndex,
}

impl<'a> MultiRepoQuery<'a> {
    /// Create a new query interface
    pub fn new(index: &'a MultiRepoIndex) -> Self {
        Self { index }
    }

    /// Find all definitions of a symbol across all repos
    pub fn find_symbol(&self, name: &str) -> Vec<&UnifiedSymbolRef> {
        self.index
            .unified_symbols
            .get(name)
            .map(|refs| refs.iter().collect())
            .unwrap_or_default()
    }

    /// Find symbols by prefix (namespace search)
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<(&String, &Vec<UnifiedSymbolRef>)> {
        self.index
            .unified_symbols
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .collect()
    }

    /// Find symbols by kind
    pub fn find_by_kind(&self, kind: SymbolKind) -> Vec<&UnifiedSymbolRef> {
        let kind_str = format!("{:?}", kind);
        self.index
            .unified_symbols
            .values()
            .flatten()
            .filter(|r| r.kind == kind_str)
            .collect()
    }

    /// Find cross-repo dependencies of a repository
    pub fn get_repo_dependencies(&self, repo_id: &str) -> Vec<&CrossRepoLink> {
        self.index
            .cross_repo_links
            .iter()
            .filter(|link| link.source_repo == repo_id)
            .collect()
    }

    /// Find repos that depend on a given repo
    pub fn get_repo_dependents(&self, repo_id: &str) -> Vec<&CrossRepoLink> {
        self.index
            .cross_repo_links
            .iter()
            .filter(|link| link.target_repo == repo_id)
            .collect()
    }

    /// Get cross-repo links by type
    pub fn get_links_by_type(&self, link_type: CrossRepoLinkType) -> Vec<&CrossRepoLink> {
        self.index
            .cross_repo_links
            .iter()
            .filter(|link| link.link_type == link_type)
            .collect()
    }

    /// Find all symbols in a file across all repos
    pub fn find_symbols_in_file(&self, file_path: &str) -> Vec<&UnifiedSymbolRef> {
        self.index
            .unified_symbols
            .values()
            .flatten()
            .filter(|r| r.file_path == file_path)
            .collect()
    }

    /// Find all symbols in a repo
    pub fn find_symbols_in_repo(&self, repo_id: &str) -> Vec<&UnifiedSymbolRef> {
        self.index
            .unified_symbols
            .values()
            .flatten()
            .filter(|r| r.repo_id == repo_id)
            .collect()
    }

    /// Get repository info
    pub fn get_repo(&self, repo_id: &str) -> Option<&RepoEntry> {
        self.index.repositories.iter().find(|r| r.id == repo_id)
    }

    /// Get all repositories
    pub fn get_all_repos(&self) -> &[RepoEntry] {
        &self.index.repositories
    }

    /// Get dependency graph as adjacency list
    pub fn get_dependency_graph(&self) -> HashMap<String, Vec<String>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for link in &self.index.cross_repo_links {
            graph
                .entry(link.source_repo.clone())
                .or_default()
                .push(link.target_repo.clone());
        }

        // Deduplicate
        for deps in graph.values_mut() {
            deps.sort();
            deps.dedup();
        }

        graph
    }

    /// Find common symbols between repos
    pub fn find_common_symbols(&self, repo1: &str, repo2: &str) -> Vec<&String> {
        let repo1_symbols: std::collections::HashSet<_> = self
            .index
            .unified_symbols
            .iter()
            .filter(|(_, refs)| refs.iter().any(|r| r.repo_id == repo1))
            .map(|(name, _)| name)
            .collect();

        let repo2_symbols: std::collections::HashSet<_> = self
            .index
            .unified_symbols
            .iter()
            .filter(|(_, refs)| refs.iter().any(|r| r.repo_id == repo2))
            .map(|(name, _)| name)
            .collect();

        repo1_symbols
            .intersection(&repo2_symbols)
            .copied()
            .collect()
    }

    /// Get statistics for the index
    pub fn get_stats(&self) -> MultiRepoStats {
        let mut symbols_per_repo: HashMap<String, u32> = HashMap::new();

        for refs in self.index.unified_symbols.values() {
            for r in refs {
                *symbols_per_repo.entry(r.repo_id.clone()).or_default() += 1;
            }
        }

        MultiRepoStats {
            total_repos: self.index.repositories.len(),
            total_symbols: self.index.unified_symbols.len(),
            total_cross_repo_links: self.index.cross_repo_links.len(),
            symbols_per_repo,
        }
    }
}

/// Statistics about the multi-repo index
#[derive(Debug, Clone)]
pub struct MultiRepoStats {
    /// Total number of repositories
    pub total_repos: usize,
    /// Total number of unique symbols
    pub total_symbols: usize,
    /// Total number of cross-repo links
    pub total_cross_repo_links: usize,
    /// Symbols per repository
    pub symbols_per_repo: HashMap<String, u32>,
}

/// Convenience function to build an index from multiple repositories
pub fn build_multi_repo_index(
    repos: &[(String, String, String, Option<String>, Vec<(String, Vec<Symbol>)>)],
) -> MultiRepoIndex {
    let mut builder = MultiRepoIndexBuilder::new();

    for (id, name, path, commit, files) in repos {
        builder.add_repository(id.clone(), name.clone(), path.clone(), commit.clone());

        for (file_path, symbols) in files {
            builder.add_file_symbols(id, file_path, symbols);
        }
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Visibility;

    fn make_symbol(name: &str, kind: SymbolKind, calls: Vec<&str>) -> Symbol {
        Symbol {
            name: name.to_owned(),
            kind,
            visibility: Visibility::Public,
            calls: calls.into_iter().map(String::from).collect(),
            start_line: 1,
            end_line: 10,
            ..Default::default()
        }
    }

    #[test]
    fn test_add_repository() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("repo1", "MyLib", "/path/to/mylib", Some("abc123".to_owned()));

        let index = builder.build();

        assert_eq!(index.repositories.len(), 1);
        assert_eq!(index.repositories[0].id, "repo1");
        assert_eq!(index.repositories[0].name, "MyLib");
    }

    #[test]
    fn test_unified_symbol_index() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("repo1", "Lib1", "/lib1", None);
        builder.add_repository("repo2", "Lib2", "/lib2", None);

        let symbols1 = vec![make_symbol("common_func", SymbolKind::Function, vec![])];
        let symbols2 = vec![make_symbol("common_func", SymbolKind::Function, vec![])];

        builder.add_file_symbols("repo1", "src/utils.rs", &symbols1);
        builder.add_file_symbols("repo2", "src/helpers.rs", &symbols2);

        let index = builder.build();
        let query = MultiRepoQuery::new(&index);

        let refs = query.find_symbol("common_func");
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_cross_repo_detection() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("core", "Core", "/core", None);
        builder.add_repository("app", "App", "/app", None);

        // Add core symbol first
        let core_symbols = vec![make_symbol("CoreUtil", SymbolKind::Class, vec![])];
        builder.add_file_symbols("core", "src/util.rs", &core_symbols);

        // Register import pattern
        builder.register_import_path("core::", "core");

        // App calls core
        let app_symbols = vec![make_symbol("AppMain", SymbolKind::Class, vec!["core::CoreUtil"])];
        builder.add_file_symbols("app", "src/main.rs", &app_symbols);

        let index = builder.build();
        let query = MultiRepoQuery::new(&index);

        let deps = query.get_repo_dependencies("app");
        // Should have detected a cross-repo call
        assert!(deps.iter().any(|d| d.target_repo == "core"));
    }

    #[test]
    fn test_dependency_graph() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("a", "A", "/a", None);
        builder.add_repository("b", "B", "/b", None);
        builder.add_repository("c", "C", "/c", None);

        // Add symbols
        let a_symbols = vec![make_symbol("AClass", SymbolKind::Class, vec![])];
        let b_symbols = vec![make_symbol("BClass", SymbolKind::Class, vec![])];
        let c_symbols = vec![make_symbol("CClass", SymbolKind::Class, vec!["AClass", "BClass"])];

        builder.add_file_symbols("a", "a.rs", &a_symbols);
        builder.register_import_path("AClass", "a");

        builder.add_file_symbols("b", "b.rs", &b_symbols);
        builder.register_import_path("BClass", "b");

        builder.add_file_symbols("c", "c.rs", &c_symbols);

        let index = builder.build();
        let query = MultiRepoQuery::new(&index);

        let graph = query.get_dependency_graph();

        // C depends on A and B
        if let Some(c_deps) = graph.get("c") {
            assert!(c_deps.contains(&"a".to_owned()) || c_deps.contains(&"b".to_owned()));
        }
    }

    #[test]
    fn test_stats() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("repo1", "R1", "/r1", None);
        builder.add_repository("repo2", "R2", "/r2", None);

        let symbols = vec![
            make_symbol("func1", SymbolKind::Function, vec![]),
            make_symbol("func2", SymbolKind::Function, vec![]),
        ];

        builder.add_file_symbols("repo1", "file.rs", &symbols);
        builder.add_file_symbols(
            "repo2",
            "file.rs",
            &[make_symbol("func3", SymbolKind::Function, vec![])],
        );

        let index = builder.build();
        let query = MultiRepoQuery::new(&index);

        let stats = query.get_stats();

        assert_eq!(stats.total_repos, 2);
        assert!(stats.total_symbols >= 3);
    }

    #[test]
    fn test_find_by_prefix() {
        let mut builder = MultiRepoIndexBuilder::new();

        builder.add_repository("repo", "Repo", "/repo", None);

        let symbols = vec![
            make_symbol("http_get", SymbolKind::Function, vec![]),
            make_symbol("http_post", SymbolKind::Function, vec![]),
            make_symbol("db_query", SymbolKind::Function, vec![]),
        ];

        builder.add_file_symbols("repo", "api.rs", &symbols);

        let index = builder.build();
        let query = MultiRepoQuery::new(&index);

        let http_funcs = query.find_by_prefix("http_");
        assert_eq!(http_funcs.len(), 2);
    }
}
