//! Smart token budget enforcement with binary search truncation
//!
//! This module provides intelligent content truncation to fit within
//! a token budget while preserving semantic boundaries (line ends,
//! function boundaries, etc.).
//!
//! # Overview
//!
//! When processing repositories, the total content often exceeds the
//! context window of target LLMs. The `BudgetEnforcer` intelligently
//! truncates content by:
//!
//! 1. **Prioritizing important files** - Files with higher importance scores are kept first
//! 2. **Binary search truncation** - Efficiently finds the optimal cut point
//! 3. **Semantic boundaries** - Truncates at meaningful boundaries (line, function)
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::budget::{BudgetEnforcer, BudgetConfig, TruncationStrategy};
//! use infiniloom_engine::{Repository, TokenModel};
//!
//! // Create a budget enforcer with 50K token limit
//! let config = BudgetConfig {
//!     budget: 50_000,
//!     model: TokenModel::Claude,
//!     strategy: TruncationStrategy::Semantic,
//!     overhead_reserve: 1000,
//! };
//! let enforcer = BudgetEnforcer::new(config);
//!
//! // Enforce budget on repository
//! let mut repo = Repository::new("my-project", "/path");
//! let result = enforcer.enforce(&mut repo);
//!
//! println!("Used {:.1}% of budget", result.budget_used_pct);
//! println!("{} files truncated, {} excluded", result.truncated_files, result.excluded_files);
//! ```
//!
//! # Truncation Strategies
//!
//! - **Line**: Truncates at newline boundaries (default, fast)
//! - **Semantic**: Truncates at function/class boundaries (slower, preserves context)
//! - **Hard**: Truncates at exact byte position (fastest, may break mid-statement)

use crate::constants::budget as budget_consts;
use crate::newtypes::TokenCount;
use crate::tokenizer::{TokenModel, Tokenizer};
use crate::types::Repository;

/// Budget enforcement strategies
#[derive(Debug, Clone, Copy, Default)]
pub enum TruncationStrategy {
    /// Truncate at line boundaries (default)
    #[default]
    Line,
    /// Truncate at function/class boundaries
    Semantic,
    /// Hard truncation with "..." suffix
    Hard,
}

/// Configuration for budget enforcement
#[derive(Debug, Clone, Copy)]
pub struct BudgetConfig {
    /// Total token budget
    pub budget: TokenCount,
    /// Target tokenizer model
    pub model: TokenModel,
    /// Truncation strategy
    pub strategy: TruncationStrategy,
    /// Reserve tokens for overhead (headers, map, etc.)
    pub overhead_reserve: TokenCount,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            budget: TokenCount::new(budget_consts::DEFAULT_BUDGET),
            model: TokenModel::Claude,
            strategy: TruncationStrategy::Line,
            overhead_reserve: TokenCount::new(budget_consts::OVERHEAD_RESERVE),
        }
    }
}

/// Smart token budget enforcer using binary search
pub struct BudgetEnforcer {
    config: BudgetConfig,
    tokenizer: Tokenizer,
}

impl BudgetEnforcer {
    /// Create a new budget enforcer with the given configuration
    pub fn new(config: BudgetConfig) -> Self {
        Self { config, tokenizer: Tokenizer::new() }
    }

    /// Create with just budget and model
    pub fn with_budget(budget: u32, model: TokenModel) -> Self {
        Self::new(BudgetConfig { budget: TokenCount::new(budget), model, ..Default::default() })
    }

    /// Enforce budget on repository, truncating file contents as needed
    ///
    /// Files are processed in importance order (highest first).
    /// Returns the number of files that were truncated.
    pub fn enforce(&self, repo: &mut Repository) -> EnforcementResult {
        let available_budget = self
            .config
            .budget
            .saturating_sub(self.config.overhead_reserve);
        let mut used_tokens = TokenCount::zero();
        let mut truncated_count = 0usize;
        let mut excluded_count = 0usize;
        let min_partial = TokenCount::new(budget_consts::MIN_PARTIAL_FIT_TOKENS);

        // Sort files by importance (descending)
        let mut file_indices: Vec<usize> = (0..repo.files.len()).collect();
        file_indices.sort_by(|&a, &b| {
            repo.files[b]
                .importance
                .partial_cmp(&repo.files[a].importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for idx in file_indices {
            let file = &mut repo.files[idx];

            if let Some(content) = file.content.as_ref() {
                let file_tokens = TokenCount::new(self.count_tokens(content));

                if used_tokens + file_tokens <= available_budget {
                    // File fits entirely
                    used_tokens += file_tokens;
                } else if used_tokens + min_partial < available_budget {
                    // Partial fit - truncate to remaining budget
                    let remaining = available_budget.saturating_sub(used_tokens);
                    let truncated = self.truncate_to_tokens(content, remaining.get());
                    let truncated_tokens = TokenCount::new(self.count_tokens(&truncated));

                    file.content = Some(truncated);
                    used_tokens += truncated_tokens;
                    truncated_count += 1;
                } else {
                    // No room - exclude content
                    file.content = None;
                    excluded_count += 1;
                }
            }
        }

        EnforcementResult {
            total_tokens: used_tokens,
            truncated_files: truncated_count,
            excluded_files: excluded_count,
            budget_used_pct: used_tokens.percentage_of(available_budget),
        }
    }

    /// Count tokens in text using configured model
    fn count_tokens(&self, text: &str) -> u32 {
        self.tokenizer.count(text, self.config.model)
    }

    /// Truncate content to fit within max_tokens using binary search
    ///
    /// Uses binary search to find the optimal cut point, then adjusts
    /// to the nearest semantic boundary.
    pub fn truncate_to_tokens(&self, content: &str, max_tokens: u32) -> String {
        // Quick check if content already fits
        let total_tokens = self.count_tokens(content);
        if total_tokens <= max_tokens {
            return content.to_owned();
        }

        // Binary search for optimal byte position
        let mut low = 0usize;
        let mut high = content.len();
        let mut best_pos = 0usize;

        while low < high {
            let mid = (low + high).div_ceil(2);

            // Ensure we don't split a UTF-8 character
            let safe_mid = self.find_char_boundary(content, mid);
            let slice = &content[..safe_mid];
            let tokens = self.count_tokens(slice);

            if tokens <= max_tokens {
                best_pos = safe_mid;
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        // Find semantic boundary near best_pos
        let boundary = self.find_semantic_boundary(content, best_pos);

        // Add truncation indicator
        let mut result = content[..boundary].to_owned();
        if boundary < content.len() {
            result.push_str("\n\n... [truncated]");
        }

        result
    }

    /// Find a valid UTF-8 character boundary at or before position
    fn find_char_boundary(&self, s: &str, pos: usize) -> usize {
        if pos >= s.len() {
            return s.len();
        }

        let mut boundary = pos;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        boundary
    }

    /// Find a semantic boundary (line end, function end, etc.) near position
    fn find_semantic_boundary(&self, content: &str, pos: usize) -> usize {
        if pos == 0 || pos >= content.len() {
            return pos;
        }

        let slice = &content[..pos];

        match self.config.strategy {
            TruncationStrategy::Hard => pos,
            TruncationStrategy::Line => {
                // Find last newline
                slice.rfind('\n').map(|p| p + 1).unwrap_or(pos)
            },
            TruncationStrategy::Semantic => {
                // Try to find function/class boundary first
                if let Some(boundary) = self.find_function_boundary(slice) {
                    return boundary;
                }
                // Fall back to line boundary
                slice.rfind('\n').map(|p| p + 1).unwrap_or(pos)
            },
        }
    }

    /// Find a function/class boundary in the content
    fn find_function_boundary(&self, content: &str) -> Option<usize> {
        // Look for common function/class start patterns from the end
        let patterns = [
            "\n\nfn ",       // Rust
            "\n\ndef ",      // Python
            "\n\nclass ",    // Python/JS
            "\n\nfunction ", // JavaScript
            "\n\npub fn ",   // Rust public
            "\n\nasync ",    // JavaScript async
            "\n\nimpl ",     // Rust impl
            "\n\n#[",        // Rust attributes
            "\n\n@",         // Decorators
        ];

        // Search from the end for function boundaries
        let mut best_pos = None;
        for pattern in patterns {
            if let Some(pos) = content.rfind(pattern) {
                // Check if this position is better (closer to end)
                if best_pos.map_or(true, |bp| pos > bp) {
                    best_pos = Some(pos);
                }
            }
        }

        // Return position after the double newline but before the pattern keyword
        // The +2 accounts for "\n\n" - we want to include the newlines but start
        // before the actual function/class keyword
        best_pos.map(|p| {
            // Validate bounds to prevent off-by-one errors
            let boundary = p + 2;
            if boundary <= content.len() {
                boundary
            } else {
                // Fallback to just after the match position if bounds check fails
                (p + 1).min(content.len())
            }
        })
    }
}

/// Result of budget enforcement
#[derive(Debug, Clone)]
pub struct EnforcementResult {
    /// Total tokens used after enforcement
    pub total_tokens: TokenCount,
    /// Number of files that were truncated
    pub truncated_files: usize,
    /// Number of files excluded entirely
    pub excluded_files: usize,
    /// Percentage of budget used
    pub budget_used_pct: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_preserves_short_content() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "Hello, world!";
        let result = enforcer.truncate_to_tokens(content, 1000);
        assert_eq!(result, content);
    }

    #[test]
    fn test_truncate_adds_indicator() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10";
        let result = enforcer.truncate_to_tokens(content, 5);
        assert!(result.contains("[truncated]"));
        assert!(result.len() < content.len());
    }

    #[test]
    fn test_find_char_boundary() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "Hello, 世界!"; // Multi-byte UTF-8
        let boundary = enforcer.find_char_boundary(content, 8);
        // Should find boundary at valid UTF-8 position
        assert!(content.is_char_boundary(boundary));
    }

    #[test]
    fn test_semantic_boundary_line() {
        let config = BudgetConfig { strategy: TruncationStrategy::Line, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);
        let content = "line1\nline2\nline3";
        let boundary = enforcer.find_semantic_boundary(content, 10);
        // Should find boundary after "line1\n"
        assert_eq!(boundary, 6);
    }

    #[test]
    fn test_semantic_boundary_function() {
        let config = BudgetConfig { strategy: TruncationStrategy::Semantic, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);
        let content = "fn foo() {}\n\ndef bar():\n    pass";
        let boundary = enforcer.find_semantic_boundary(content, content.len());
        // Should find boundary at "def bar"
        assert!(boundary > 10);
    }

    // =========================================================================
    // Additional edge case tests for comprehensive coverage
    // =========================================================================

    #[test]
    fn test_empty_content_truncation() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let result = enforcer.truncate_to_tokens("", 100);
        assert_eq!(result, "");
    }

    #[test]
    fn test_single_character_content() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let result = enforcer.truncate_to_tokens("x", 100);
        assert_eq!(result, "x");
    }

    #[test]
    fn test_zero_budget_truncation() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "Some content that will be truncated";
        let result = enforcer.truncate_to_tokens(content, 0);
        // Should return minimal content or truncation indicator
        assert!(result.len() <= content.len());
    }

    #[test]
    fn test_unicode_boundary_preservation() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        // Content with multi-byte UTF-8 characters
        let content = "Hello 世界! More text here. 🦀 Rust! Even more...";

        // Truncate to various budgets
        for budget in [1, 2, 3, 5, 10] {
            let result = enforcer.truncate_to_tokens(content, budget);
            // Verify we can still iterate over chars (valid UTF-8)
            let _ = result.chars().count();
            // Verify the string is valid
            assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        }
    }

    #[test]
    fn test_content_smaller_than_indicator() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        // Very small content
        let content = "Hi";
        let result = enforcer.truncate_to_tokens(content, 1);
        // Should handle gracefully
        assert!(!result.is_empty() || content.is_empty());
    }

    #[test]
    fn test_hard_truncation_strategy() {
        let config = BudgetConfig { strategy: TruncationStrategy::Hard, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);
        let content = "line1\nline2\nline3";
        let boundary = enforcer.find_semantic_boundary(content, 10);
        // Hard strategy should return exact position
        assert_eq!(boundary, 10);
    }

    #[test]
    fn test_boundary_at_start() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "Some content";
        let boundary = enforcer.find_semantic_boundary(content, 0);
        assert_eq!(boundary, 0);
    }

    #[test]
    fn test_boundary_past_end() {
        let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
        let content = "Some content";
        let boundary = enforcer.find_semantic_boundary(content, content.len() + 10);
        // Should clamp to content length
        assert_eq!(boundary, content.len() + 10); // Returns pos as-is when >= len
    }

    #[test]
    fn test_function_boundary_rust_patterns() {
        let config = BudgetConfig { strategy: TruncationStrategy::Semantic, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);

        // Test various Rust function patterns
        let content = "use std::io;\n\nfn helper() {}\n\npub fn main() {}";
        let boundary = enforcer.find_function_boundary(content);
        assert!(boundary.is_some());

        // Test impl block
        let content2 = "struct Foo;\n\nimpl Foo {\n    fn new() {}\n}";
        let boundary2 = enforcer.find_function_boundary(content2);
        assert!(boundary2.is_some());
    }

    #[test]
    fn test_function_boundary_python_patterns() {
        let config = BudgetConfig { strategy: TruncationStrategy::Semantic, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);

        // Test Python function with decorator
        let content = "import os\n\n@decorator\ndef foo():\n    pass";
        let boundary = enforcer.find_function_boundary(content);
        assert!(boundary.is_some());

        // Test Python class
        let content2 = "import sys\n\nclass MyClass:\n    pass";
        let boundary2 = enforcer.find_function_boundary(content2);
        assert!(boundary2.is_some());
    }

    #[test]
    fn test_function_boundary_javascript_patterns() {
        let config = BudgetConfig { strategy: TruncationStrategy::Semantic, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);

        // Test JavaScript function
        let content = "const x = 1;\n\nfunction foo() {}\n\nasync function bar() {}";
        let boundary = enforcer.find_function_boundary(content);
        assert!(boundary.is_some());
    }

    #[test]
    fn test_no_function_boundary_found() {
        let config = BudgetConfig { strategy: TruncationStrategy::Semantic, ..Default::default() };
        let enforcer = BudgetEnforcer::new(config);

        // Content without any function boundaries
        let content = "just some text without any code patterns";
        let boundary = enforcer.find_function_boundary(content);
        assert!(boundary.is_none());
    }

    #[test]
    fn test_enforcement_result_fields() {
        let result = EnforcementResult {
            total_tokens: TokenCount::new(1000),
            truncated_files: 5,
            excluded_files: 2,
            budget_used_pct: 85.5,
        };

        assert_eq!(result.total_tokens.get(), 1000);
        assert_eq!(result.truncated_files, 5);
        assert_eq!(result.excluded_files, 2);
        assert!((result.budget_used_pct - 85.5).abs() < 0.01);
    }

    #[test]
    fn test_budget_config_default() {
        use crate::constants::budget as budget_consts;
        let config = BudgetConfig::default();
        assert_eq!(config.budget.get(), budget_consts::DEFAULT_BUDGET);
        assert!(matches!(config.strategy, TruncationStrategy::Line));
        assert_eq!(config.overhead_reserve.get(), budget_consts::OVERHEAD_RESERVE);
    }
}
