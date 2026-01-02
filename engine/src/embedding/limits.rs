//! Resource limits to prevent DoS attacks
//!
//! This module provides configurable limits for:
//! - AST recursion depth (prevent stack overflow)
//! - File sizes (prevent memory exhaustion)
//! - Total chunks (prevent output explosion)
//! - Concurrent operations (prevent resource exhaustion)

use serde::{Deserialize, Serialize};

/// Resource limits to prevent DoS attacks and resource exhaustion
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum recursion depth for AST traversal
    /// Default: 500 (handles deeply nested code)
    pub max_recursion_depth: u32,

    /// Maximum file size to process (bytes)
    /// Default: 10 MB (larger files are skipped with warning)
    pub max_file_size: u64,

    /// Maximum total chunks to generate
    /// Default: 1,000,000 (enterprise scale)
    pub max_total_chunks: usize,

    /// Maximum files to process
    /// Default: 500,000 (large monorepo scale)
    pub max_files: usize,

    /// Maximum concurrent file operations
    /// Default: 32 (reasonable for most systems)
    pub max_concurrent_loads: usize,

    /// Maximum line length to process (bytes)
    /// Default: 10,000 (prevents single-line minified files)
    pub max_line_length: usize,

    /// Maximum content size per chunk (bytes)
    /// Default: 1 MB (prevents extremely large chunks)
    pub max_chunk_size: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_recursion_depth: 500,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_total_chunks: 1_000_000,     // 1 million chunks
            max_files: 500_000,              // 500k files
            max_concurrent_loads: 32,        // 32 concurrent operations
            max_line_length: 10_000,         // 10k chars per line
            max_chunk_size: 1024 * 1024,     // 1 MB per chunk
        }
    }
}

impl ResourceLimits {
    /// Create limits suitable for trusted input (development)
    pub fn relaxed() -> Self {
        Self {
            max_recursion_depth: 1000,
            max_file_size: 50 * 1024 * 1024, // 50 MB
            max_total_chunks: 10_000_000,    // 10 million
            max_files: 1_000_000,            // 1 million files
            max_concurrent_loads: 64,        // More concurrency
            max_line_length: 100_000,        // 100k chars
            max_chunk_size: 5 * 1024 * 1024, // 5 MB
        }
    }

    /// Strict limits for untrusted input (CI/CD, public APIs)
    pub fn strict() -> Self {
        Self {
            max_recursion_depth: 100,
            max_file_size: 1024 * 1024, // 1 MB
            max_total_chunks: 100_000,  // 100k chunks
            max_files: 50_000,          // 50k files
            max_concurrent_loads: 8,    // Limited concurrency
            max_line_length: 1000,      // 1k chars
            max_chunk_size: 100 * 1024, // 100 KB
        }
    }

    /// Create limits suitable for a quick scan or test
    pub fn minimal() -> Self {
        Self {
            max_recursion_depth: 50,
            max_file_size: 100 * 1024, // 100 KB
            max_total_chunks: 1000,    // 1k chunks
            max_files: 100,            // 100 files
            max_concurrent_loads: 4,   // Minimal concurrency
            max_line_length: 500,      // 500 chars
            max_chunk_size: 10 * 1024, // 10 KB
        }
    }

    /// Check if a file size is within limits
    #[inline]
    pub fn check_file_size(&self, size: u64) -> bool {
        size <= self.max_file_size
    }

    /// Check if recursion depth is within limits
    #[inline]
    pub fn check_recursion_depth(&self, depth: u32) -> bool {
        depth <= self.max_recursion_depth
    }

    /// Check if chunk count is within limits
    #[inline]
    pub fn check_chunk_count(&self, count: usize) -> bool {
        count <= self.max_total_chunks
    }

    /// Check if file count is within limits
    #[inline]
    pub fn check_file_count(&self, count: usize) -> bool {
        count <= self.max_files
    }

    /// Check if line length is within limits
    #[inline]
    pub fn check_line_length(&self, length: usize) -> bool {
        length <= self.max_line_length
    }

    /// Check if chunk size is within limits
    #[inline]
    pub fn check_chunk_size(&self, size: usize) -> bool {
        size <= self.max_chunk_size
    }

    /// Builder-style: set max recursion depth
    pub fn with_max_recursion_depth(mut self, depth: u32) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Builder-style: set max file size
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Builder-style: set max total chunks
    pub fn with_max_total_chunks(mut self, count: usize) -> Self {
        self.max_total_chunks = count;
        self
    }

    /// Builder-style: set max files
    pub fn with_max_files(mut self, count: usize) -> Self {
        self.max_files = count;
        self
    }

    /// Builder-style: set max concurrent loads
    pub fn with_max_concurrent_loads(mut self, count: usize) -> Self {
        self.max_concurrent_loads = count;
        self
    }

    /// Builder-style: set max line length
    pub fn with_max_line_length(mut self, length: usize) -> Self {
        self.max_line_length = length;
        self
    }

    /// Builder-style: set max chunk size
    pub fn with_max_chunk_size(mut self, size: usize) -> Self {
        self.max_chunk_size = size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_recursion_depth, 500);
        assert_eq!(limits.max_file_size, 10 * 1024 * 1024);
        assert_eq!(limits.max_total_chunks, 1_000_000);
        assert_eq!(limits.max_files, 500_000);
    }

    #[test]
    fn test_strict_limits() {
        let limits = ResourceLimits::strict();
        assert_eq!(limits.max_recursion_depth, 100);
        assert_eq!(limits.max_file_size, 1024 * 1024);
        assert!(limits.max_total_chunks < ResourceLimits::default().max_total_chunks);
    }

    #[test]
    fn test_relaxed_limits() {
        let limits = ResourceLimits::relaxed();
        assert!(limits.max_recursion_depth > ResourceLimits::default().max_recursion_depth);
        assert!(limits.max_file_size > ResourceLimits::default().max_file_size);
    }

    #[test]
    fn test_check_file_size() {
        let limits = ResourceLimits::default();
        assert!(limits.check_file_size(1024)); // 1 KB
        assert!(limits.check_file_size(10 * 1024 * 1024)); // Exactly 10 MB
        assert!(!limits.check_file_size(11 * 1024 * 1024)); // 11 MB
    }

    #[test]
    fn test_check_recursion_depth() {
        let limits = ResourceLimits::default();
        assert!(limits.check_recursion_depth(100));
        assert!(limits.check_recursion_depth(500)); // Exactly at limit
        assert!(!limits.check_recursion_depth(501)); // Over limit
    }

    #[test]
    fn test_builder_pattern() {
        let limits = ResourceLimits::default()
            .with_max_file_size(5 * 1024 * 1024)
            .with_max_recursion_depth(200)
            .with_max_total_chunks(50_000);

        assert_eq!(limits.max_file_size, 5 * 1024 * 1024);
        assert_eq!(limits.max_recursion_depth, 200);
        assert_eq!(limits.max_total_chunks, 50_000);
    }

    #[test]
    fn test_serialization() {
        let limits = ResourceLimits::default();
        let json = serde_json::to_string(&limits).unwrap();
        let deserialized: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(limits, deserialized);
    }

    #[test]
    fn test_minimal_limits() {
        let limits = ResourceLimits::minimal();
        assert!(limits.max_files <= 100);
        assert!(limits.max_total_chunks <= 1000);
    }
}
