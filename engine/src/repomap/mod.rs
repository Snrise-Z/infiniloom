//! Repository map generation with PageRank-based symbol ranking

mod graph;

use crate::constants::repomap as repomap_consts;

fn summarize_symbol(symbol: &Symbol) -> Option<String> {
    let line = symbol
        .docstring
        .as_deref()
        .and_then(first_nonempty_line)
        .or_else(|| symbol.signature.as_deref().and_then(first_nonempty_line));

    line.map(|text| truncate_summary(text, repomap_consts::SUMMARY_MAX_LEN))
}

fn first_nonempty_line(text: &str) -> Option<&str> {
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
}

fn truncate_summary(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_owned();
    }

    let mut truncated: String = text.chars().take(max_len).collect();
    truncated = truncated.trim_end().to_owned();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
use crate::types::RepoFile;
use crate::types::{Repository, Symbol, SymbolKind, TokenizerModel};
use graph::SymbolGraph;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use typed_builder::TypedBuilder;

/// A repository map - a concise summary of the codebase
#[derive(Debug, Clone, Serialize)]
pub struct RepoMap {
    /// Text summary of the repository
    pub summary: String,
    /// Most important symbols ranked by PageRank
    pub key_symbols: Vec<RankedSymbol>,
    /// Module/directory dependency graph
    pub module_graph: ModuleGraph,
    /// Index of all files with metadata
    pub file_index: Vec<FileIndexEntry>,
    /// Total token count for this map
    pub token_count: u32,
}

impl fmt::Display for RepoMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RepoMap({} symbols, {} files, {} tokens)",
            self.key_symbols.len(),
            self.file_index.len(),
            self.token_count
        )
    }
}

/// A symbol with its computed rank
#[derive(Debug, Clone, Serialize)]
pub struct RankedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// File containing the symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// Function/method signature
    pub signature: Option<String>,
    /// Short summary (docstring or signature)
    pub summary: Option<String>,
    /// Number of references
    pub references: u32,
    /// Rank (1 = most important)
    pub rank: u32,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
}

/// Graph of module dependencies
#[derive(Debug, Clone, Serialize)]
pub struct ModuleGraph {
    /// Module nodes
    pub nodes: Vec<ModuleNode>,
    /// Dependency edges
    pub edges: Vec<ModuleEdge>,
}

/// A module/directory node
#[derive(Debug, Clone, Serialize)]
pub struct ModuleNode {
    /// Module name (usually directory name)
    pub name: String,
    /// Number of files in module
    pub files: u32,
    /// Total tokens in module
    pub tokens: u32,
}

/// A dependency edge between modules
#[derive(Debug, Clone, Serialize)]
pub struct ModuleEdge {
    /// Source module
    pub from: String,
    /// Target module
    pub to: String,
    /// Number of imports/references
    pub weight: u32,
}

/// File index entry
#[derive(Debug, Clone, Serialize)]
pub struct FileIndexEntry {
    /// Relative file path
    pub path: String,
    /// Token count
    pub tokens: u32,
    /// Importance level (critical/high/normal/low)
    pub importance: String,
    /// Brief summary (optional)
    pub summary: Option<String>,
}

/// Generator for repository maps
///
/// # Examples
///
/// ```rust,ignore
/// use infiniloom_engine::{RepoMapGenerator, TokenizerModel};
///
/// // Use builder pattern
/// let generator = RepoMapGenerator::builder()
///     .token_budget(50_000)
///     .model(TokenizerModel::Gpt4o)
///     .build();
///
/// // Or use the convenience constructor
/// let generator = RepoMapGenerator::new(2000);
/// ```
#[derive(Debug, Clone, TypedBuilder)]
pub struct RepoMapGenerator {
    /// Maximum number of symbols to include (default: computed from budget)
    #[builder(default, setter(strip_option))]
    max_symbols: Option<usize>,

    /// Target model for token counting (default: Claude)
    #[builder(default = TokenizerModel::Claude)]
    model: TokenizerModel,

    /// Token budget for the map (default: 2000)
    #[builder(default = 2000)]
    token_budget: u32,
}

impl RepoMapGenerator {
    /// Create a new generator with token budget
    /// The token budget influences how many symbols are included in the map.
    /// Approximately 25 tokens per symbol entry, so max_symbols = budget / 30 (with overhead)
    pub fn new(token_budget: u32) -> Self {
        Self { max_symbols: None, model: TokenizerModel::Claude, token_budget }
    }

    /// Get effective max symbols (computed from budget if not set)
    ///
    /// Bug #7 fix: Increased max cap from 200 to 500 and made formula more responsive
    fn effective_max_symbols(&self) -> usize {
        self.max_symbols.unwrap_or_else(|| {
            // Use constants for token estimation (see module-level documentation)
            // Bug #7 fix: Increased max from 200 to 500 for larger budgets
            ((self.token_budget as usize) / repomap_consts::BUDGET_SYMBOL_DIVISOR)
                .clamp(repomap_consts::MIN_SYMBOLS, repomap_consts::MAX_SYMBOLS)
        })
    }

    /// Generate a repository map with PageRank-based symbol ranking.
    ///
    /// The generated map includes:
    /// - Key symbols ranked by importance (PageRank algorithm)
    /// - Module dependency graph
    /// - File index with importance levels
    /// - Summary statistics
    ///
    /// The map is optimized for the configured token budget.
    #[must_use = "repository map should be used or formatted"]
    pub fn generate(&self, repo: &Repository) -> RepoMap {
        // Build symbol graph
        let mut graph = SymbolGraph::new();
        for file in &repo.files {
            graph.add_file(file);
        }

        // Build lookup index for fast import resolution
        let symbol_index = self.build_symbol_index(repo);

        // Extract references from symbols using index
        self.extract_references_fast(&mut graph, repo, &symbol_index);

        // Compute PageRank once
        let ranks = graph.compute_pagerank(0.85, 20); // Reduced iterations for speed

        // Get top symbols using pre-computed ranks
        let key_symbols = self.build_ranked_symbols_fast(&graph, &ranks);

        // Build module graph
        let module_graph = self.build_module_graph(repo);

        // Build file index (with token budget enforcement based on symbol count)
        let file_index = self.build_file_index(repo, key_symbols.len());

        // Generate summary
        let summary = self.generate_summary(repo, &key_symbols);

        // Estimate token count
        let token_count = self.estimate_tokens(&key_symbols, &file_index);

        RepoMap { summary, key_symbols, module_graph, file_index, token_count }
    }

    /// Build an index of symbols for fast lookup
    ///
    /// Uses multi-value index to handle symbol name collisions across files.
    /// For example, multiple files may have a `main` function - all are indexed.
    fn build_symbol_index(&self, repo: &Repository) -> HashMap<String, Vec<String>> {
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for file in &repo.files {
            // Index by file path (without extension) - supports all 22 languages
            let path_key = Self::strip_language_extension(&file.relative_path);

            for symbol in &file.symbols {
                let symbol_key = format!("{}:{}", file.relative_path, symbol.name);

                // Index by symbol name (multi-value to handle collisions)
                index
                    .entry(symbol.name.clone())
                    .or_default()
                    .push(symbol_key.clone());

                // Index by path component (only for the first symbol to maintain backwards compat)
                // This allows import resolution like "from utils import foo" -> "utils.py:foo"
                index
                    .entry(path_key.to_owned())
                    .or_default()
                    .push(symbol_key);
            }
        }
        index
    }

    /// Fast reference extraction using pre-built index
    ///
    /// Uses multi-value index to properly resolve all matching symbols when
    /// there are name collisions across files.
    fn extract_references_fast(
        &self,
        graph: &mut SymbolGraph,
        repo: &Repository,
        index: &HashMap<String, Vec<String>>,
    ) {
        for file in &repo.files {
            for symbol in &file.symbols {
                let from_key = format!("{}:{}", file.relative_path, symbol.name);

                // Extract import references
                if symbol.kind == SymbolKind::Import {
                    // Fast lookup using index - connect to all matching symbols
                    if let Some(targets) = index.get(&symbol.name) {
                        for target in targets {
                            // Don't create self-references
                            if target != &from_key {
                                graph.add_reference(&from_key, target, graph::EdgeType::Imports);
                            }
                        }
                    }
                }

                // Extract inheritance relationships (extends)
                if let Some(ref base_class) = symbol.extends {
                    // Try fast lookup first, then fallback to name-based search
                    if let Some(targets) = index.get(base_class) {
                        for target in targets {
                            if target != &from_key {
                                graph.add_reference(&from_key, target, graph::EdgeType::Inherits);
                            }
                        }
                    } else {
                        graph.add_reference_by_name(
                            &from_key,
                            base_class,
                            graph::EdgeType::Inherits,
                        );
                    }
                }

                // Extract interface/trait implementations
                for iface in &symbol.implements {
                    if let Some(targets) = index.get(iface) {
                        for target in targets {
                            if target != &from_key {
                                graph.add_reference(&from_key, target, graph::EdgeType::Implements);
                            }
                        }
                    } else {
                        graph.add_reference_by_name(&from_key, iface, graph::EdgeType::Implements);
                    }
                }

                // Extract function call relationships
                for called_name in &symbol.calls {
                    if let Some(targets) = index.get(called_name) {
                        for target in targets {
                            if target != &from_key {
                                graph.add_reference(&from_key, target, graph::EdgeType::Calls);
                            }
                        }
                    } else {
                        graph.add_reference_by_name(&from_key, called_name, graph::EdgeType::Calls);
                    }
                }
            }
        }
    }

    /// Build ranked symbols using pre-computed ranks
    fn build_ranked_symbols_fast(
        &self,
        graph: &SymbolGraph,
        ranks: &HashMap<String, f64>,
    ) -> Vec<RankedSymbol> {
        let top_nodes = graph.get_top_symbols_with_ranks(ranks, self.effective_max_symbols());

        top_nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let key = format!("{}:{}", node.file_path, node.symbol.name);
                let rank_score = ranks.get(&key).copied().unwrap_or(0.0);

                RankedSymbol {
                    name: node.symbol.name.clone(),
                    kind: node.symbol.kind.name().to_owned(),
                    file: node.file_path.clone(),
                    line: node.symbol.start_line,
                    signature: node.symbol.signature.clone(),
                    summary: summarize_symbol(&node.symbol),
                    references: node.symbol.references,
                    rank: (i + 1) as u32,
                    importance: rank_score as f32,
                }
            })
            .collect()
    }

    fn build_module_graph(&self, repo: &Repository) -> ModuleGraph {
        let mut modules: HashMap<String, ModuleNode> = HashMap::new();
        let mut edge_counts: HashMap<(String, String), u32> = HashMap::new();

        // Build file index by module (first pass)
        for file in &repo.files {
            let module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root")
                .to_owned();

            let entry = modules.entry(module.clone()).or_insert(ModuleNode {
                name: module.clone(),
                files: 0,
                tokens: 0,
            });

            entry.files += 1;
            entry.tokens += file.token_count.get(self.model);
        }

        // Build a map of symbol names to their modules for cross-reference
        let mut symbol_to_module: HashMap<String, String> = HashMap::new();
        for file in &repo.files {
            let module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root")
                .to_owned();

            for symbol in &file.symbols {
                symbol_to_module.insert(symbol.name.clone(), module.clone());
            }
        }

        // Compute edges based on imports and calls between modules
        for file in &repo.files {
            let from_module = file
                .relative_path
                .split('/')
                .next()
                .unwrap_or("root")
                .to_owned();

            for symbol in &file.symbols {
                // Count import references to other modules
                if symbol.kind == SymbolKind::Import {
                    if let Some(target_module) = symbol_to_module.get(&symbol.name) {
                        if target_module != &from_module {
                            *edge_counts
                                .entry((from_module.clone(), target_module.clone()))
                                .or_insert(0) += 1;
                        }
                    }
                }

                // Count function calls to other modules
                for called_name in &symbol.calls {
                    if let Some(target_module) = symbol_to_module.get(called_name) {
                        if target_module != &from_module {
                            *edge_counts
                                .entry((from_module.clone(), target_module.clone()))
                                .or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // Convert edge counts to edges
        let edges: Vec<ModuleEdge> = edge_counts
            .into_iter()
            .map(|((from, to), weight)| ModuleEdge { from, to, weight })
            .collect();

        ModuleGraph { nodes: modules.into_values().collect(), edges }
    }

    fn build_file_index(&self, repo: &Repository, symbol_count: usize) -> Vec<FileIndexEntry> {
        let mut files: Vec<_> = repo
            .files
            .iter()
            .map(|f| {
                let importance = if f.importance > 0.8 {
                    "critical"
                } else if f.importance > 0.6 {
                    "high"
                } else if f.importance > 0.3 {
                    "normal"
                } else {
                    "low"
                };

                FileIndexEntry {
                    path: f.relative_path.clone(),
                    tokens: f.token_count.get(self.model),
                    importance: importance.to_owned(),
                    summary: None,
                }
            })
            .collect();

        // Sort by importance
        files.sort_by(|a, b| {
            let a_imp = match a.importance.as_str() {
                "critical" => 4,
                "high" => 3,
                "normal" => 2,
                _ => 1,
            };
            let b_imp = match b.importance.as_str() {
                "critical" => 4,
                "high" => 3,
                "normal" => 2,
                _ => 1,
            };
            b_imp.cmp(&a_imp)
        });

        // Enforce token budget: limit file entries based on remaining budget
        // Uses centralized constants for token estimation
        let symbol_tokens = symbol_count as u32 * repomap_consts::TOKENS_PER_SYMBOL;
        let remaining_budget = self
            .token_budget
            .saturating_sub(symbol_tokens)
            .saturating_sub(repomap_consts::TOKEN_OVERHEAD);
        let max_files = (remaining_budget / repomap_consts::TOKENS_PER_FILE) as usize;

        if files.len() > max_files && max_files > 0 {
            files.truncate(max_files);
        }

        files
    }

    fn generate_summary(&self, repo: &Repository, symbols: &[RankedSymbol]) -> String {
        // Deduplicate modules - take top symbols until we have 3 unique modules
        let mut seen = std::collections::HashSet::new();
        let top_modules: Vec<_> = symbols
            .iter()
            .filter_map(|s| s.file.split('/').next())
            .filter(|m| seen.insert(*m))
            .take(3)
            .collect();

        let primary_lang = repo
            .metadata
            .languages
            .first()
            .map(|l| l.language.as_str())
            .unwrap_or("unknown");

        format!(
            "Repository: {} ({} files, {} lines)\n\
             Primary language: {}\n\
             Key modules: {}",
            repo.name,
            repo.metadata.total_files,
            repo.metadata.total_lines,
            primary_lang,
            top_modules.join(", ")
        )
    }

    fn estimate_tokens(&self, symbols: &[RankedSymbol], files: &[FileIndexEntry]) -> u32 {
        // Uses centralized constants for token estimation
        let symbol_tokens = symbols.len() as u32 * repomap_consts::TOKENS_PER_SYMBOL;
        let file_tokens = files.len() as u32 * repomap_consts::TOKENS_PER_FILE;

        symbol_tokens + file_tokens + repomap_consts::TOKEN_OVERHEAD
    }

    /// Strip language extension from a file path for indexing
    /// Supports all 22 languages from the parser module
    fn strip_language_extension(path: &str) -> &str {
        // Extensions ordered by specificity (longer first to avoid partial matches)
        const EXTENSIONS: &[&str] = &[
            // Multi-char extensions first
            ".gemspec",
            ".fsscript",
            // TypeScript/JavaScript
            ".tsx",
            ".jsx",
            ".mjs",
            ".cjs",
            ".ts",
            ".js",
            // C/C++
            ".cpp",
            ".cxx",
            ".hpp",
            ".hxx",
            ".cc",
            ".hh",
            ".c",
            ".h",
            // C#
            ".cs",
            // Ruby
            ".rb",
            ".rake",
            // Shell
            ".bash",
            ".zsh",
            ".fish",
            ".sh",
            // PHP
            ".phtml",
            ".php3",
            ".php4",
            ".php5",
            ".phps",
            ".php",
            // Kotlin
            ".kts",
            ".kt",
            // Swift
            ".swift",
            // Scala
            ".scala",
            ".sc",
            // Haskell
            ".lhs",
            ".hs",
            // Elixir
            ".heex",
            ".leex",
            ".exs",
            ".eex",
            ".ex",
            // Clojure
            ".cljs",
            ".cljc",
            ".edn",
            ".clj",
            // OCaml
            ".mli",
            ".ml",
            // F#
            ".fsx",
            ".fsi",
            ".fs",
            // Lua
            ".lua",
            // R
            ".rmd",
            ".r",
            // Python
            ".pyw",
            ".py",
            // Rust
            ".rs",
            // Go
            ".go",
            // Java
            ".java",
        ];

        for ext in EXTENSIONS {
            if let Some(stripped) = path.strip_suffix(ext) {
                return stripped;
            }
        }
        path
    }
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::types::{RepoMetadata, TokenCounts};

    fn create_test_repo() -> Repository {
        Repository {
            name: "test-repo".to_owned(),
            path: "/tmp/test".into(),
            files: vec![RepoFile {
                path: "/tmp/test/src/main.py".into(),
                relative_path: "src/main.py".to_string(),
                language: Some("python".to_string()),
                size_bytes: 1000,
                token_count: TokenCounts {
                    o200k: 240,
                    cl100k: 245,
                    claude: 250,
                    gemini: 230,
                    llama: 235,
                    mistral: 235,
                    deepseek: 235,
                    qwen: 235,
                    cohere: 238,
                    grok: 235,
                },
                symbols: vec![Symbol {
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    signature: Some("def main() -> None".to_string()),
                    docstring: Some("Entry point".to_string()),
                    start_line: 10,
                    end_line: 25,
                    references: 5,
                    importance: 0.9,
                    parent: None,
                    visibility: crate::types::Visibility::Public,
                    calls: vec!["helper".to_string(), "process".to_string()],
                    extends: None,
                    implements: vec![],
                }],
                importance: 0.9,
                content: None,
            }],
            metadata: RepoMetadata {
                total_files: 1,
                total_lines: 100,
                total_tokens: TokenCounts {
                    o200k: 240,
                    cl100k: 245,
                    claude: 250,
                    gemini: 230,
                    llama: 235,
                    mistral: 235,
                    deepseek: 235,
                    qwen: 235,
                    cohere: 238,
                    grok: 235,
                },
                languages: vec![crate::types::LanguageStats {
                    language: "Python".to_string(),
                    files: 1,
                    lines: 100,
                    percentage: 100.0,
                }],
                framework: None,
                description: None,
                branch: None,
                commit: None,
                directory_structure: None,
                external_dependencies: vec![],
                git_history: None,
            },
        }
    }

    #[test]
    fn test_generate_repomap() {
        let repo = create_test_repo();
        let generator = RepoMapGenerator::new(2000);
        let map = generator.generate(&repo);

        assert!(!map.summary.is_empty());
        assert!(!map.file_index.is_empty());
    }

    // Bug #7 test - verify mapBudget has significant effect
    #[test]
    fn test_map_budget_affects_output() {
        // Test that different budgets produce different effective_max_symbols
        let small = RepoMapGenerator::new(500);
        let medium = RepoMapGenerator::new(2000);
        let large = RepoMapGenerator::new(10000);

        let small_max = small.effective_max_symbols();
        let medium_max = medium.effective_max_symbols();
        let large_max = large.effective_max_symbols();

        // Small budget should have fewer max symbols
        assert!(
            small_max < medium_max,
            "Small budget ({}) should have fewer symbols ({}) than medium ({}) with ({})",
            500,
            small_max,
            2000,
            medium_max
        );

        // Large budget should have more max symbols
        assert!(
            medium_max < large_max,
            "Medium budget ({}) should have fewer symbols ({}) than large ({}) with ({})",
            2000,
            medium_max,
            10000,
            large_max
        );

        // Verify the actual values are reasonable
        // With BUDGET_SYMBOL_DIVISOR = 20:
        // 500 / 20 = 25
        // 2000 / 20 = 100
        // 10000 / 20 = 500
        assert!(
            (20..=30).contains(&small_max),
            "Small max_symbols should be ~25, got {}",
            small_max
        );
        assert!(
            (90..=110).contains(&medium_max),
            "Medium max_symbols should be ~100, got {}",
            medium_max
        );
        assert!(
            (400..=500).contains(&large_max),
            "Large max_symbols should be ~500, got {}",
            large_max
        );
    }

    #[test]
    fn test_map_budget_min_max_clamped() {
        use crate::constants::repomap as consts;

        // Very small budget should clamp to minimum
        let tiny = RepoMapGenerator::new(10);
        assert_eq!(tiny.effective_max_symbols(), consts::MIN_SYMBOLS);

        // Very large budget should clamp to maximum
        let huge = RepoMapGenerator::new(100_000);
        assert_eq!(huge.effective_max_symbols(), consts::MAX_SYMBOLS);
    }
}
