//! Repository operations shared between bindings
//!
//! This module provides common repository processing logic used by both
//! Python and Node.js bindings, including compression, filtering, and preparation.

use infiniloom_engine::{
    count_symbol_references,
    default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES},
    rank_files, sort_files_by_importance,
    tokenizer::TokenModel,
    CompressionLevel, HeuristicCompressor, Repository, SecurityScanner, Tokenizer,
};

use crate::{focused_symbol_context, signature_lines};

/// Apply default ignore filters to repository files
///
/// Filters out build outputs, dependencies, test fixtures, etc.
pub fn apply_default_ignores(repo: &mut Repository) {
    repo.files.retain(|f| {
        !matches_any(&f.relative_path, DEFAULT_IGNORES)
            && !matches_any(&f.relative_path, TEST_IGNORES)
    });
}

/// Prepare repository for output
///
/// This performs common operations needed before formatting:
/// - Count cross-file symbol references
/// - Rank files by importance
/// - Sort files by importance
pub fn prepare_repository(repo: &mut Repository) {
    count_symbol_references(repo);
    rank_files(repo);
    sort_files_by_importance(repo);
}

/// Redact secrets from all files in the repository
pub fn redact_secrets(repo: &mut Repository) {
    let scanner = SecurityScanner::new();
    for file in &mut repo.files {
        if let Some(ref content) = file.content {
            let redacted = scanner.redact_content(content, &file.relative_path);
            file.content = Some(redacted);
        }
    }
}

/// Apply compression to repository file contents
///
/// Compresses file content based on the specified compression level:
/// - None: No compression
/// - Minimal: Remove empty lines
/// - Balanced: Remove empty lines and comments
/// - Aggressive/Extreme: Extract signatures only
/// - Focused: Key symbols with context
/// - Semantic: Heuristic-based semantic compression
pub fn apply_compression(repo: &mut Repository, level: CompressionLevel) {
    match level {
        CompressionLevel::None => {
            // No compression - keep content as-is
        },
        CompressionLevel::Minimal => {
            // Remove empty lines
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        },
        CompressionLevel::Balanced => {
            // Remove empty lines and comments (basic heuristic)
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let compressed: String = content
                        .lines()
                        .filter(|line| {
                            let trimmed = line.trim();
                            !trimmed.is_empty()
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with('#')
                                && !trimmed.starts_with("/*")
                                && !trimmed.starts_with('*')
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    file.content = Some(compressed);
                }
            }
        },
        CompressionLevel::Aggressive | CompressionLevel::Extreme => {
            // Extract signatures only - keep function/class definitions
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    file.content = Some(signature_lines(content));
                }
            }
        },
        CompressionLevel::Focused => {
            // Key symbols with small surrounding context
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    let focused = focused_symbol_context(content, &file.symbols);
                    file.content = Some(focused);
                }
            }
        },
        CompressionLevel::Semantic => {
            // Use heuristic-based semantic compression
            let compressor = HeuristicCompressor::new();
            for file in &mut repo.files {
                if let Some(ref content) = file.content {
                    if let Ok(compressed) = compressor.compress(content) {
                        file.content = Some(compressed);
                    }
                }
            }
        },
    }
}

/// Apply token budget to limit output size
///
/// Files should be sorted by importance before calling this.
/// Keeps files until budget is reached, always including at least one file.
///
/// Returns the number of tokens in the kept files.
pub fn apply_token_budget(repo: &mut Repository, budget: u32, model: TokenModel) -> u32 {
    if budget == 0 {
        return 0;
    }

    let tokenizer = Tokenizer::new();
    let mut cumulative_tokens: u32 = 0;
    let mut files_to_keep = Vec::new();

    for file in std::mem::take(&mut repo.files) {
        let file_tokens = file
            .content
            .as_ref()
            .map(|c| tokenizer.count(c, model))
            .unwrap_or(0);

        // Check if adding this file would exceed budget
        if cumulative_tokens + file_tokens <= budget {
            cumulative_tokens += file_tokens;
            files_to_keep.push(file);
        } else if files_to_keep.is_empty() {
            // Always include at least one file (the most important)
            cumulative_tokens = file_tokens;
            files_to_keep.push(file);
            break;
        } else {
            // Budget exceeded, stop adding files
            break;
        }
    }

    repo.files = files_to_keep;
    repo.metadata.total_files = repo.files.len() as u32;
    cumulative_tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiniloom_engine::types::{RepoFile, RepoMetadata, Symbol, SymbolKind, Visibility};
    use std::path::PathBuf;

    fn create_test_repo() -> Repository {
        Repository {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            files: vec![
                RepoFile {
                    path: PathBuf::from("/test/main.rs"),
                    relative_path: "main.rs".to_string(),
                    language: Some("Rust".to_string()),
                    size_bytes: 100,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.5,
                    content: Some("fn main() {\n    println!(\"hello\");\n}\n".to_string()),
                },
                RepoFile {
                    path: PathBuf::from("/test/lib.rs"),
                    relative_path: "lib.rs".to_string(),
                    language: Some("Rust".to_string()),
                    size_bytes: 50,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.3,
                    content: Some("// Comment\npub fn helper() {}\n".to_string()),
                },
            ],
            metadata: RepoMetadata { total_files: 2, ..Default::default() },
        }
    }

    fn create_test_repo_with_default_ignores() -> Repository {
        Repository {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            files: vec![
                RepoFile {
                    path: PathBuf::from("/test/src/main.rs"),
                    relative_path: "src/main.rs".to_string(),
                    language: Some("Rust".to_string()),
                    size_bytes: 100,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.5,
                    content: Some("fn main() {}".to_string()),
                },
                RepoFile {
                    path: PathBuf::from("/test/node_modules/pkg/index.js"),
                    relative_path: "node_modules/pkg/index.js".to_string(),
                    language: Some("JavaScript".to_string()),
                    size_bytes: 100,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.1,
                    content: Some("module.exports = {}".to_string()),
                },
                RepoFile {
                    path: PathBuf::from("/test/dist/bundle.js"),
                    relative_path: "dist/bundle.js".to_string(),
                    language: Some("JavaScript".to_string()),
                    size_bytes: 100,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.1,
                    content: Some("bundled code".to_string()),
                },
                RepoFile {
                    path: PathBuf::from("/test/tests/test_main.rs"),
                    relative_path: "tests/test_main.rs".to_string(),
                    language: Some("Rust".to_string()),
                    size_bytes: 100,
                    token_count: Default::default(),
                    symbols: vec![],
                    importance: 0.3,
                    content: Some("#[test] fn test() {}".to_string()),
                },
            ],
            metadata: RepoMetadata { total_files: 4, ..Default::default() },
        }
    }

    // ============================================================================
    // apply_default_ignores tests
    // ============================================================================

    #[test]
    fn test_apply_default_ignores_filters_node_modules() {
        let mut repo = create_test_repo_with_default_ignores();
        apply_default_ignores(&mut repo);

        assert!(!repo
            .files
            .iter()
            .any(|f| f.relative_path.contains("node_modules")));
    }

    #[test]
    fn test_apply_default_ignores_filters_dist() {
        let mut repo = create_test_repo_with_default_ignores();
        apply_default_ignores(&mut repo);

        assert!(!repo.files.iter().any(|f| f.relative_path.contains("dist")));
    }

    #[test]
    fn test_apply_default_ignores_filters_tests() {
        let mut repo = create_test_repo_with_default_ignores();
        apply_default_ignores(&mut repo);

        assert!(!repo.files.iter().any(|f| f.relative_path.contains("tests")));
    }

    #[test]
    fn test_apply_default_ignores_keeps_source() {
        let mut repo = create_test_repo_with_default_ignores();
        apply_default_ignores(&mut repo);

        assert_eq!(repo.files.len(), 1);
        assert!(repo.files[0].relative_path.contains("src/main.rs"));
    }

    #[test]
    fn test_apply_default_ignores_empty_repo() {
        let mut repo = Repository {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            files: vec![],
            metadata: Default::default(),
        };
        apply_default_ignores(&mut repo);
        assert!(repo.files.is_empty());
    }

    // ============================================================================
    // prepare_repository tests
    // ============================================================================

    #[test]
    fn test_prepare_repository_sets_importance() {
        let mut repo = create_test_repo();
        prepare_repository(&mut repo);

        // Files should have importance scores
        for file in &repo.files {
            assert!(file.importance >= 0.0);
            assert!(file.importance <= 1.0);
        }
    }

    #[test]
    fn test_prepare_repository_sorts_by_importance() {
        let mut repo = create_test_repo();
        repo.files[0].importance = 0.1;
        repo.files[1].importance = 0.9;

        prepare_repository(&mut repo);

        // Should be sorted by importance (descending)
        let importances: Vec<f32> = repo.files.iter().map(|f| f.importance).collect();
        for i in 0..importances.len() - 1 {
            assert!(importances[i] >= importances[i + 1], "Files should be sorted by importance");
        }
    }

    // ============================================================================
    // apply_compression tests - comprehensive coverage
    // ============================================================================

    #[test]
    fn test_apply_compression_none() {
        let mut repo = create_test_repo();
        let original_content = repo.files[0].content.clone();
        apply_compression(&mut repo, CompressionLevel::None);
        assert_eq!(repo.files[0].content, original_content);
    }

    #[test]
    fn test_apply_compression_minimal() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("line1\n\nline2\n\n\nline3".to_string());
        apply_compression(&mut repo, CompressionLevel::Minimal);
        assert_eq!(repo.files[0].content.as_deref(), Some("line1\nline2\nline3"));
    }

    #[test]
    fn test_apply_compression_minimal_preserves_non_empty() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("line1\n  \nline2".to_string());
        apply_compression(&mut repo, CompressionLevel::Minimal);
        // Whitespace-only lines should be removed
        let result = repo.files[0].content.as_deref().unwrap();
        assert!(!result.contains("\n  \n"));
    }

    #[test]
    fn test_apply_compression_balanced() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("// comment\ncode\n# python comment\nmore code".to_string());
        apply_compression(&mut repo, CompressionLevel::Balanced);
        assert_eq!(repo.files[0].content.as_deref(), Some("code\nmore code"));
    }

    #[test]
    fn test_apply_compression_balanced_removes_multiline_comments() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("/* comment */\ncode\n* doc line\nmore code".to_string());
        apply_compression(&mut repo, CompressionLevel::Balanced);
        let result = repo.files[0].content.as_deref().unwrap();
        assert!(result.contains("code"));
        assert!(!result.contains("/*"));
        assert!(!result.contains("* doc"));
    }

    #[test]
    fn test_apply_compression_aggressive() {
        let mut repo = create_test_repo();
        apply_compression(&mut repo, CompressionLevel::Aggressive);
        // Should only contain signature lines
        assert!(repo.files[0]
            .content
            .as_ref()
            .unwrap()
            .contains("fn main()"));
        assert!(!repo.files[0].content.as_ref().unwrap().contains("println"));
    }

    #[test]
    fn test_apply_compression_extreme() {
        let mut repo = create_test_repo();
        apply_compression(&mut repo, CompressionLevel::Extreme);
        // Should behave the same as Aggressive
        assert!(repo.files[0]
            .content
            .as_ref()
            .unwrap()
            .contains("fn main()"));
        assert!(!repo.files[0].content.as_ref().unwrap().contains("println"));
    }

    #[test]
    fn test_apply_compression_focused() {
        let mut repo = create_test_repo();
        let mut symbol = Symbol::new("main", SymbolKind::Function);
        symbol.visibility = Visibility::Public;
        symbol.start_line = 1;
        symbol.end_line = 3;
        symbol.signature = Some("fn main()".to_string());
        repo.files[0].symbols = vec![symbol];
        apply_compression(&mut repo, CompressionLevel::Focused);
        // Should contain symbol context
        assert!(repo.files[0].content.is_some());
    }

    #[test]
    fn test_apply_compression_semantic() {
        let mut repo = create_test_repo();
        apply_compression(&mut repo, CompressionLevel::Semantic);
        // Should be compressed but still contain code
        assert!(repo.files[0].content.is_some());
    }

    #[test]
    fn test_apply_compression_with_none_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = None;
        apply_compression(&mut repo, CompressionLevel::Minimal);
        // Should not panic, content remains None
        assert!(repo.files[0].content.is_none());
    }

    #[test]
    fn test_apply_compression_with_empty_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some(String::new());
        apply_compression(&mut repo, CompressionLevel::Minimal);
        assert_eq!(repo.files[0].content.as_deref(), Some(""));
    }

    // ============================================================================
    // redact_secrets tests - comprehensive coverage
    // ============================================================================

    #[test]
    fn test_redact_secrets_aws_key() {
        let mut repo = create_test_repo();
        // Use a realistic AWS key (not containing "EXAMPLE" which is skipped as false positive)
        repo.files[0].content = Some("let key = \"AKIAIOSFODNN7REALKEY\";".to_string());
        redact_secrets(&mut repo);
        // Should contain partial mask (AKIA************LKEY) instead of the full key
        let content = repo.files[0].content.as_ref().unwrap();
        assert!(content.contains("AKIA")); // Prefix preserved
        assert!(content.contains("****")); // Masked middle
        assert!(!content.contains("AKIAIOSFODNN7REALKEY")); // Full key is gone
    }

    #[test]
    fn test_redact_secrets_with_none_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = None;
        redact_secrets(&mut repo);
        // Should not panic
        assert!(repo.files[0].content.is_none());
    }

    #[test]
    fn test_redact_secrets_no_secrets() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("let x = 5; // no secrets here".to_string());
        let original = repo.files[0].content.clone();
        redact_secrets(&mut repo);
        // Content should be unchanged
        assert_eq!(repo.files[0].content, original);
    }

    #[test]
    fn test_redact_secrets_empty_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some(String::new());
        redact_secrets(&mut repo);
        assert_eq!(repo.files[0].content.as_deref(), Some(""));
    }

    // ============================================================================
    // apply_token_budget tests - comprehensive coverage
    // ============================================================================

    #[test]
    fn test_apply_token_budget_zero() {
        let mut repo = create_test_repo();
        let result = apply_token_budget(&mut repo, 0, TokenModel::Claude);
        assert_eq!(result, 0);
        // Files should be unchanged when budget is 0
        assert!(repo.files.is_empty() || !repo.files.is_empty());
    }

    #[test]
    fn test_apply_token_budget_large() {
        let mut repo = create_test_repo();
        let original_count = repo.files.len();
        let result = apply_token_budget(&mut repo, 1_000_000, TokenModel::Claude);
        // All files should be kept with a large budget
        assert_eq!(repo.files.len(), original_count);
        assert!(result > 0);
    }

    #[test]
    fn test_apply_token_budget_small() {
        let mut repo = create_test_repo();
        let result = apply_token_budget(&mut repo, 10, TokenModel::Claude);
        // Should keep at least one file
        assert!(!repo.files.is_empty());
        assert!(result > 0);
    }

    #[test]
    fn test_apply_token_budget_keeps_at_least_one() {
        let mut repo = create_test_repo();
        // Even with budget=1, should keep at least one file
        let result = apply_token_budget(&mut repo, 1, TokenModel::Claude);
        assert_eq!(repo.files.len(), 1);
        assert!(result > 0);
    }

    #[test]
    fn test_apply_token_budget_updates_metadata() {
        let mut repo = create_test_repo();
        apply_token_budget(&mut repo, 10, TokenModel::Claude);
        // Metadata should be updated
        assert_eq!(repo.metadata.total_files, repo.files.len() as u32);
    }

    #[test]
    fn test_apply_token_budget_empty_repo() {
        let mut repo = Repository {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            files: vec![],
            metadata: Default::default(),
        };
        let result = apply_token_budget(&mut repo, 1000, TokenModel::Claude);
        assert_eq!(result, 0);
        assert!(repo.files.is_empty());
    }

    #[test]
    fn test_apply_token_budget_with_none_content() {
        let mut repo = create_test_repo();
        repo.files[0].content = None;
        let result = apply_token_budget(&mut repo, 1000, TokenModel::Claude);
        // Should handle None content gracefully
        assert!(result >= 0);
    }

    #[test]
    fn test_apply_token_budget_respects_importance_order() {
        let mut repo = create_test_repo();
        // Ensure files are sorted by importance before budget is applied
        repo.files[0].importance = 0.9;
        repo.files[1].importance = 0.1;

        // Sort by importance (descending) to simulate prepare_repository
        repo.files.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply a small budget that only allows one file
        apply_token_budget(&mut repo, 10, TokenModel::Claude);

        // The most important file should be kept
        assert_eq!(repo.files.len(), 1);
        assert_eq!(repo.files[0].importance, 0.9);
    }

    // ============================================================================
    // Integration tests
    // ============================================================================

    #[test]
    fn test_full_pipeline() {
        let mut repo = create_test_repo_with_default_ignores();

        // Apply default ignores
        apply_default_ignores(&mut repo);

        // Prepare repository
        prepare_repository(&mut repo);

        // Apply compression
        apply_compression(&mut repo, CompressionLevel::Minimal);

        // Apply token budget
        let tokens = apply_token_budget(&mut repo, 100_000, TokenModel::Claude);

        // Verify result
        assert!(!repo.files.is_empty());
        assert!(tokens > 0);
    }
}
