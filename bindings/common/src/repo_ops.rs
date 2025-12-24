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
    use infiniloom_engine::types::{RepoFile, RepoMetadata};
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
    fn test_apply_compression_balanced() {
        let mut repo = create_test_repo();
        repo.files[0].content = Some("// comment\ncode\n# python comment\nmore code".to_string());
        apply_compression(&mut repo, CompressionLevel::Balanced);
        assert_eq!(repo.files[0].content.as_deref(), Some("code\nmore code"));
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
    fn test_redact_secrets() {
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
}
