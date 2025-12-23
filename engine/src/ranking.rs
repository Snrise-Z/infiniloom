//! Symbol importance ranking

#[cfg(test)]
use crate::types::RepoFile;
use crate::types::{Repository, Symbol, SymbolKind};
use std::collections::HashMap;

/// Count references to symbols across all files
/// This is a second pass that updates Symbol.references based on how many
/// times each symbol name appears in other files' calls lists
pub fn count_symbol_references(repo: &mut Repository) {
    // Build a map of symbol names to their (file_index, symbol_index) locations
    let mut symbol_locations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    for (file_idx, file) in repo.files.iter().enumerate() {
        for (sym_idx, symbol) in file.symbols.iter().enumerate() {
            symbol_locations
                .entry(symbol.name.clone())
                .or_default()
                .push((file_idx, sym_idx));
        }
    }

    // Count references: for each symbol's calls, increment the referenced symbol's count
    let mut reference_counts: HashMap<String, u32> = HashMap::new();

    for file in &repo.files {
        for symbol in &file.symbols {
            for called_name in &symbol.calls {
                *reference_counts.entry(called_name.clone()).or_insert(0) += 1;
            }
        }
    }

    // Also count imports as references
    for file in &repo.files {
        for symbol in &file.symbols {
            if symbol.kind == SymbolKind::Import {
                // The import name might reference another symbol
                *reference_counts.entry(symbol.name.clone()).or_insert(0) += 1;
            }
        }
    }

    // Update symbol references
    for file in &mut repo.files {
        for symbol in &mut file.symbols {
            if let Some(&count) = reference_counts.get(&symbol.name) {
                symbol.references = count;
            }
        }
    }
}

/// Symbol ranker using multiple heuristics
pub struct SymbolRanker {
    /// Weight for reference count
    reference_weight: f32,
    /// Weight for symbol type
    type_weight: f32,
    /// Weight for file importance
    file_weight: f32,
    /// Weight for line count (larger = more important)
    size_weight: f32,
}

impl Default for SymbolRanker {
    fn default() -> Self {
        Self { reference_weight: 0.4, type_weight: 0.25, file_weight: 0.2, size_weight: 0.15 }
    }
}

impl SymbolRanker {
    /// Create a new ranker with default weights
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom weights
    pub fn with_weights(mut self, reference: f32, type_: f32, file: f32, size: f32) -> Self {
        self.reference_weight = reference;
        self.type_weight = type_;
        self.file_weight = file;
        self.size_weight = size;
        self
    }

    /// Rank all symbols in a repository
    pub fn rank(&self, repo: &mut Repository) {
        // First pass: collect statistics
        let stats = self.collect_stats(repo);

        // Second pass: compute importance scores
        for file in &mut repo.files {
            let file_importance = file.importance;

            for symbol in &mut file.symbols {
                let score = self.compute_score(symbol, file_importance, &stats);
                symbol.importance = score;
            }

            // Update file importance based on symbol importance
            if !file.symbols.is_empty() {
                let avg_symbol_importance: f32 =
                    file.symbols.iter().map(|s| s.importance).sum::<f32>()
                        / file.symbols.len() as f32;
                file.importance = (file.importance + avg_symbol_importance) / 2.0;
            }
        }
    }

    fn collect_stats(&self, repo: &Repository) -> RankingStats {
        let mut stats = RankingStats::default();

        for file in &repo.files {
            for symbol in &file.symbols {
                stats.total_symbols += 1;
                stats.max_references = stats.max_references.max(symbol.references);
                stats.max_lines = stats.max_lines.max(symbol.line_count());

                *stats.type_counts.entry(symbol.kind).or_insert(0) += 1;
            }
        }

        stats
    }

    fn compute_score(&self, symbol: &Symbol, file_importance: f32, stats: &RankingStats) -> f32 {
        // Reference score (normalized)
        let ref_score = if stats.max_references > 0 {
            symbol.references as f32 / stats.max_references as f32
        } else {
            0.0
        };

        // Type score (based on symbol kind)
        let type_score = type_importance(symbol.kind);

        // Size score (normalized)
        let size_score = if stats.max_lines > 0 {
            (symbol.line_count() as f32 / stats.max_lines as f32).min(1.0)
        } else {
            0.0
        };

        // Combine scores
        let score = self.reference_weight * ref_score
            + self.type_weight * type_score
            + self.file_weight * file_importance
            + self.size_weight * size_score;

        // Clamp to [0, 1]
        score.clamp(0.0, 1.0)
    }
}

/// Statistics for ranking normalization
#[derive(Default)]
struct RankingStats {
    total_symbols: usize,
    max_references: u32,
    max_lines: u32,
    type_counts: HashMap<SymbolKind, usize>,
}

/// Get base importance for a symbol kind
fn type_importance(kind: SymbolKind) -> f32 {
    match kind {
        // Entry points and main interfaces are most important
        SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait => 1.0,
        // Public API functions
        SymbolKind::Function | SymbolKind::Method => 0.8,
        // Types and structures
        SymbolKind::Struct | SymbolKind::Enum | SymbolKind::TypeAlias => 0.7,
        // Constants and exports
        SymbolKind::Constant | SymbolKind::Export => 0.6,
        // Modules
        SymbolKind::Module => 0.5,
        // Less important
        SymbolKind::Variable | SymbolKind::Import | SymbolKind::Macro => 0.3,
    }
}

/// Rank files by importance using heuristics
/// Priority: Entry points > Core implementation > Libraries > Config > Tests
pub fn rank_files(repo: &mut Repository) {
    // Critical entry point patterns (highest priority)
    let critical_entry_patterns = [
        "__main__.py",
        "main.rs",
        "main.go",
        "main.c",
        "main.cpp",
        "main.ts",
        "main.js",
        "index.ts",
        "index.js",
        "index.tsx",
        "index.jsx",
        "app.ts",
        "app.js",
        "app.py",
        "app.go",
        "app.rb",
        "server.ts",
        "server.js",
        "server.py",
        "server.go",
        "cli.rs",
        "cli.ts",
        "cli.js",
        "cli.py",
        "lib.rs",
        "mod.rs",
    ];

    // Important implementation directories
    let core_dirs =
        ["/src/", "/lib/", "/core/", "/pkg/", "/internal/", "/app/", "/cmd/", "/bin/", "/crates/"];

    // Entry point file prefixes (less specific)
    let entry_prefixes = [
        "main.",
        "index.",
        "app.",
        "server.",
        "cli.",
        "mod.",
        "lib.",
        "init.",
        "__init__.",
        "entry.",
        "bootstrap.",
    ];

    // Documentation (medium-low importance but still useful)
    let doc_patterns = ["readme.", "changelog.", "contributing.", "license.", "authors."];

    // Config patterns (low importance - metadata not code)
    let config_patterns = [
        "config.",
        "settings.",
        ".config",
        "package.json",
        "cargo.toml",
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "tsconfig.",
        "webpack.",
        ".eslint",
        ".prettier",
        "jest.config",
        "vite.config",
        ".env",
        "makefile",
        "dockerfile",
        "docker-compose",
        ".github/",
        ".gitlab",
    ];

    // Test patterns (lowest priority for code understanding)
    // Note: Use both with and without leading slash to match root dirs
    let test_patterns = [
        "test_",
        "_test.",
        ".test.",
        ".fixture.",
        "_fixture.",
        "spec.",
        "_spec.",
        "/tests/",
        "tests/",
        "/test/",
        "test/",
        "/__tests__/",
        "__tests__/",
        "/testing/",
        "testing/",
        "/fixtures/",
        "fixtures/",
        "/__fixtures__/",
        "__fixtures__/",
        "/mocks/",
        "mocks/",
        "/__mocks__/",
        "__mocks__/",
        "mock_",
        "_mock.",
        "/e2e/",
        "e2e/",
        "/integration/",
        "integration/",
        "/unit/",
        "unit/",
        "/examples/",
        "examples/",
        "/example/",
        "example/",
        "/benchmark/",
        "benchmark/",
        "/cypress/",
        "cypress/",
        "/playwright/",
        "playwright/",
    ];

    // Vendor/generated patterns (exclude or very low priority)
    let vendor_patterns = [
        "/vendor/",
        "vendor/",
        "/node_modules/",
        "node_modules/",
        "/dist/",
        "dist/",
        "/build/",
        "build/",
        "/target/",
        "target/",
        "/__pycache__/",
        "__pycache__/",
        "/.next/",
        ".next/",
        "/coverage/",
        "coverage/",
        "/.cache/",
        ".cache/",
        "/generated/",
        "generated/",
        "/.generated/",
        ".generated/",
        "/gen/",
        "gen/",
        ".min.js",
        ".min.css",
        ".bundle.",
        "/benchmarks/",
        "benchmarks/",
    ];

    for file in &mut repo.files {
        let filename = file.filename().to_lowercase();
        let path = file.relative_path.to_lowercase();

        let mut importance: f32;

        // Check vendor/generated first (exclude from ranking)
        if vendor_patterns.iter().any(|p| path.contains(p)) {
            importance = 0.05;
        }
        // Check test patterns (low priority)
        else if test_patterns.iter().any(|p| path.contains(p)) {
            importance = 0.15;
        }
        // Check config patterns (low priority)
        else if config_patterns
            .iter()
            .any(|p| filename.contains(p) || path.contains(p))
        {
            importance = 0.25;
        }
        // Check doc patterns
        else if doc_patterns.iter().any(|p| filename.starts_with(p)) {
            importance = 0.35;
        }
        // Check critical entry points (highest priority)
        else if critical_entry_patterns.iter().any(|p| filename == *p) {
            importance = 1.0;
        }
        // Check entry point prefixes
        else if entry_prefixes.iter().any(|p| filename.starts_with(p)) {
            importance = 0.9;
        }
        // Check core directories
        else if core_dirs.iter().any(|p| path.contains(p)) {
            importance = 0.75;
        }
        // Default for other source files
        else {
            importance = 0.5;
        }

        // Only apply boosts if not in test/vendor directories
        let is_test_or_vendor = vendor_patterns.iter().any(|p| path.contains(p))
            || test_patterns.iter().any(|p| path.contains(p));

        if !is_test_or_vendor {
            // Boost based on symbol count (more symbols = more important code)
            let symbol_boost = (file.symbols.len() as f32 / 50.0).min(0.15);
            importance = (importance + symbol_boost).min(1.0);

            // Slight boost for files with common implementation names
            if filename.contains("handler")
                || filename.contains("service")
                || filename.contains("controller")
                || filename.contains("model")
                || filename.contains("util")
                || filename.contains("helper")
                || filename.contains("router")
                || filename.contains("middleware")
            {
                importance = (importance + 0.1).min(1.0);
            }
        }

        file.importance = importance;
    }
}

/// Sort repository files by importance (highest first)
pub fn sort_files_by_importance(repo: &mut Repository) {
    repo.files.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
#[allow(clippy::str_to_string)]
mod tests {
    use super::*;
    use crate::types::TokenCounts;

    #[test]
    fn test_type_importance() {
        assert!(type_importance(SymbolKind::Class) > type_importance(SymbolKind::Variable));
        assert!(type_importance(SymbolKind::Function) > type_importance(SymbolKind::Import));
    }

    #[test]
    fn test_ranker() {
        let mut repo = Repository::new("test", "/tmp/test");
        repo.files.push(RepoFile {
            path: "/tmp/test/main.py".into(),
            relative_path: "main.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![
                Symbol {
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    references: 10,
                    start_line: 1,
                    end_line: 20,
                    ..Symbol::new("main", SymbolKind::Function)
                },
                Symbol {
                    name: "helper".to_string(),
                    kind: SymbolKind::Function,
                    references: 2,
                    start_line: 25,
                    end_line: 30,
                    ..Symbol::new("helper", SymbolKind::Function)
                },
            ],
            importance: 0.5,
            content: None,
        });

        let ranker = SymbolRanker::new();
        ranker.rank(&mut repo);

        // Main should have higher importance due to more references
        let main_importance = repo.files[0].symbols[0].importance;
        let helper_importance = repo.files[0].symbols[1].importance;
        assert!(main_importance > helper_importance);
    }

    #[test]
    fn test_count_symbol_references() {
        let mut repo = Repository::new("test", "/tmp/test");

        // File 1: defines helper() and calls process()
        repo.files.push(RepoFile {
            path: "/tmp/test/utils.py".into(),
            relative_path: "utils.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![Symbol {
                name: "helper".to_string(),
                kind: SymbolKind::Function,
                references: 0, // Not yet counted
                start_line: 1,
                end_line: 10,
                calls: vec!["process".to_string()],
                ..Symbol::new("helper", SymbolKind::Function)
            }],
            importance: 0.5,
            content: None,
        });

        // File 2: defines main() and process(), calls helper() twice
        repo.files.push(RepoFile {
            path: "/tmp/test/main.py".into(),
            relative_path: "main.py".to_string(),
            language: Some("python".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![
                Symbol {
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    references: 0,
                    start_line: 1,
                    end_line: 10,
                    calls: vec!["helper".to_string(), "helper".to_string()],
                    ..Symbol::new("main", SymbolKind::Function)
                },
                Symbol {
                    name: "process".to_string(),
                    kind: SymbolKind::Function,
                    references: 0,
                    start_line: 15,
                    end_line: 25,
                    calls: vec![],
                    ..Symbol::new("process", SymbolKind::Function)
                },
            ],
            importance: 0.5,
            content: None,
        });

        // Count references
        count_symbol_references(&mut repo);

        // helper is called twice (by main)
        assert_eq!(repo.files[0].symbols[0].references, 2, "helper should have 2 references");

        // process is called once (by helper)
        assert_eq!(repo.files[1].symbols[1].references, 1, "process should have 1 reference");

        // main is not called by anyone
        assert_eq!(repo.files[1].symbols[0].references, 0, "main should have 0 references");
    }

    #[test]
    fn test_fixture_files_low_importance() {
        let mut repo = Repository::new("test", "/tmp/test");

        // Regular source file
        repo.files.push(RepoFile {
            path: "/tmp/test/src/api.go".into(),
            relative_path: "src/api.go".to_string(),
            language: Some("go".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        // Fixture file (should get low importance)
        repo.files.push(RepoFile {
            path: "/tmp/test/pkg/tools/ReadFile.fixture.go".into(),
            relative_path: "pkg/tools/ReadFile.fixture.go".to_string(),
            language: Some("go".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        // Test file (should get low importance)
        repo.files.push(RepoFile {
            path: "/tmp/test/src/api_test.go".into(),
            relative_path: "src/api_test.go".to_string(),
            language: Some("go".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        rank_files(&mut repo);

        let api_importance = repo.files[0].importance;
        let fixture_importance = repo.files[1].importance;
        let test_importance = repo.files[2].importance;

        // Source files should have higher importance than fixture/test files
        assert!(
            api_importance > fixture_importance,
            "api.go ({}) should have higher importance than ReadFile.fixture.go ({})",
            api_importance,
            fixture_importance
        );
        assert!(
            api_importance > test_importance,
            "api.go ({}) should have higher importance than api_test.go ({})",
            api_importance,
            test_importance
        );

        // Fixture and test files should have low importance (0.15 = test pattern match)
        assert!(
            fixture_importance <= 0.20,
            "fixture file importance ({}) should be <= 0.20",
            fixture_importance
        );
        assert!(
            test_importance <= 0.20,
            "test file importance ({}) should be <= 0.20",
            test_importance
        );
    }

    #[test]
    fn test_dist_files_low_importance() {
        let mut repo = Repository::new("test", "/tmp/test");

        // Regular source file
        repo.files.push(RepoFile {
            path: "/tmp/test/src/index.ts".into(),
            relative_path: "src/index.ts".to_string(),
            language: Some("typescript".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        // dist file (should get very low importance)
        repo.files.push(RepoFile {
            path: "/tmp/test/dist/index.js".into(),
            relative_path: "dist/index.js".to_string(),
            language: Some("javascript".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        // node_modules file (should get very low importance)
        repo.files.push(RepoFile {
            path: "/tmp/test/node_modules/pkg/index.js".into(),
            relative_path: "node_modules/pkg/index.js".to_string(),
            language: Some("javascript".to_string()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None,
        });

        rank_files(&mut repo);

        let src_importance = repo.files[0].importance;
        let dist_importance = repo.files[1].importance;
        let node_modules_importance = repo.files[2].importance;

        // Source files should have higher importance than vendor/dist files
        assert!(
            src_importance > dist_importance,
            "src/index.ts ({}) should have higher importance than dist/index.js ({})",
            src_importance,
            dist_importance
        );
        assert!(
            src_importance > node_modules_importance,
            "src/index.ts ({}) should have higher importance than node_modules file ({})",
            src_importance,
            node_modules_importance
        );

        // dist and node_modules should have very low importance (0.05)
        assert!(
            dist_importance <= 0.10,
            "dist file importance ({}) should be <= 0.10",
            dist_importance
        );
        assert!(
            node_modules_importance <= 0.10,
            "node_modules file importance ({}) should be <= 0.10",
            node_modules_importance
        );
    }
}
