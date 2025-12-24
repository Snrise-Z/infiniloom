//! Centralized constants for Infiniloom
//!
//! All magic numbers and configuration defaults are defined here
//! for easy maintenance and consistency across the codebase.

/// Budget-related constants
pub mod budget {
    /// Default token budget for repository processing
    pub const DEFAULT_BUDGET: u32 = 100_000;

    /// Token reserve for headers, metadata, and overhead
    pub const OVERHEAD_RESERVE: u32 = 1_000;

    /// Minimum tokens needed to include a file partially
    pub const MIN_PARTIAL_FIT_TOKENS: u32 = 100;

    /// Default token budget for repository maps
    pub const DEFAULT_MAP_BUDGET: u32 = 2_000;

    /// Default chunk size in tokens
    pub const DEFAULT_CHUNK_SIZE: u32 = 8_000;
}

/// Compression-related constants
pub mod compression {
    /// Compression ratio for minimal level (keeps 90%)
    pub const MINIMAL_RATIO: f64 = 0.90;

    /// Compression ratio for balanced level (keeps 70%)
    pub const BALANCED_RATIO: f64 = 0.70;

    /// Compression ratio for aggressive level (keeps 50%)
    pub const AGGRESSIVE_RATIO: f64 = 0.50;

    /// Compression ratio for extreme level (keeps 30%)
    pub const EXTREME_RATIO: f64 = 0.30;

    /// Compression ratio for focused level (keeps 25%)
    pub const FOCUSED_RATIO: f64 = 0.25;

    /// Default semantic compression budget ratio
    pub const SEMANTIC_BUDGET_RATIO: f64 = 0.50;
}

/// Timeout constants (in seconds)
pub mod timeouts {
    /// Timeout for git operations
    pub const GIT_OPERATION_SECS: u64 = 30;

    /// Timeout for remote clone operations
    pub const REMOTE_CLONE_SECS: u64 = 300;

    /// Timeout for parsing operations
    pub const PARSE_TIMEOUT_SECS: u64 = 60;
}

/// File processing constants
pub mod files {
    /// Maximum file size to process (10 MB)
    pub const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

    /// Number of bytes to check for binary detection
    pub const BINARY_CHECK_BYTES: usize = 8192;

    /// Default importance score for new files
    pub const DEFAULT_IMPORTANCE: f32 = 0.5;

    /// Maximum line length before truncation
    pub const MAX_LINE_LENGTH: usize = 2000;

    /// Number of signature lines to extract
    pub const SIGNATURE_LINES: usize = 3;
}

/// PageRank constants
pub mod pagerank {
    /// Damping factor for PageRank algorithm
    pub const DAMPING_FACTOR: f64 = 0.85;

    /// Maximum iterations for PageRank convergence
    pub const MAX_ITERATIONS: usize = 100;

    /// Convergence threshold for PageRank
    pub const CONVERGENCE_THRESHOLD: f64 = 1e-6;
}

/// Security scanning constants
pub mod security {
    /// Minimum entropy threshold for secret detection
    pub const MIN_ENTROPY_THRESHOLD: f64 = 3.5;

    /// Maximum length for secret patterns
    pub const MAX_SECRET_LENGTH: usize = 500;
}

/// Index-related constants
pub mod index {
    /// Default context depth for diff operations
    pub const DEFAULT_CONTEXT_DEPTH: u8 = 2;

    /// Maximum context depth allowed
    pub const MAX_CONTEXT_DEPTH: u8 = 3;

    /// Default symbol ID when unknown
    pub const UNKNOWN_SYMBOL_ID: u32 = 0;
}

/// Repository map generation constants
pub mod repomap {
    /// Estimated tokens per symbol entry in the repository map
    /// Includes name, kind, file, line, signature (~25 tokens avg)
    pub const TOKENS_PER_SYMBOL: u32 = 25;

    /// Estimated tokens per file entry in the file index
    /// Includes path, tokens, importance (~10 tokens avg)
    pub const TOKENS_PER_FILE: u32 = 10;

    /// Fixed token overhead for headers, summary, and formatting
    pub const TOKEN_OVERHEAD: u32 = 100;

    /// Divisor for computing max symbols from budget (includes safety margin)
    /// Formula: max_symbols = budget / BUDGET_SYMBOL_DIVISOR
    pub const BUDGET_SYMBOL_DIVISOR: usize = 20;

    /// Maximum length for symbol summaries before truncation
    pub const SUMMARY_MAX_LEN: usize = 120;

    /// Minimum symbols to include regardless of budget
    pub const MIN_SYMBOLS: usize = 5;

    /// Maximum symbols to include regardless of budget
    pub const MAX_SYMBOLS: usize = 500;
}

/// Parser constants
pub mod parser {
    /// Maximum number of symbols to extract per file
    pub const MAX_SYMBOLS_PER_FILE: usize = 10_000;

    /// Maximum recursion depth for AST traversal
    pub const MAX_RECURSION_DEPTH: usize = 100;

    /// Maximum query result size
    pub const MAX_QUERY_MATCHES: usize = 50_000;
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests verify compile-time constants have sensible values.
    // The assertions are intentionally on constants to document invariants.
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_compression_ratios_are_valid() {
        assert!(compression::MINIMAL_RATIO > 0.0 && compression::MINIMAL_RATIO <= 1.0);
        assert!(compression::BALANCED_RATIO > 0.0 && compression::BALANCED_RATIO <= 1.0);
        assert!(compression::AGGRESSIVE_RATIO > 0.0 && compression::AGGRESSIVE_RATIO <= 1.0);
        assert!(compression::EXTREME_RATIO > 0.0 && compression::EXTREME_RATIO <= 1.0);
        assert!(compression::FOCUSED_RATIO > 0.0 && compression::FOCUSED_RATIO <= 1.0);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_compression_ratios_ordering() {
        // Minimal should keep more than balanced, etc.
        assert!(compression::MINIMAL_RATIO > compression::BALANCED_RATIO);
        assert!(compression::BALANCED_RATIO > compression::AGGRESSIVE_RATIO);
        assert!(compression::AGGRESSIVE_RATIO > compression::EXTREME_RATIO);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_budget_defaults_are_reasonable() {
        assert!(budget::DEFAULT_BUDGET > budget::OVERHEAD_RESERVE);
        assert!(budget::DEFAULT_MAP_BUDGET > 0);
        assert!(budget::DEFAULT_CHUNK_SIZE > 0);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_pagerank_constants_are_valid() {
        assert!(pagerank::DAMPING_FACTOR > 0.0 && pagerank::DAMPING_FACTOR < 1.0);
        assert!(pagerank::MAX_ITERATIONS > 0);
        assert!(pagerank::CONVERGENCE_THRESHOLD > 0.0);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn test_timeouts_are_reasonable() {
        assert!(timeouts::GIT_OPERATION_SECS > 0);
        assert!(timeouts::REMOTE_CLONE_SECS > timeouts::GIT_OPERATION_SECS);
    }
}
