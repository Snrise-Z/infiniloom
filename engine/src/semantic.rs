//! Semantic analysis and compression module
//!
//! This module provides semantic code understanding through embeddings,
//! enabling similarity search and intelligent code compression.
//!
//! # Feature: `embeddings`
//!
//! When the `embeddings` feature is enabled, this module provides:
//! - Embedding generation for code content (currently uses character-frequency heuristics)
//! - Cosine similarity computation between code snippets
//! - Clustering-based compression that groups similar code chunks
//!
//! ## Current Implementation Status
//!
//! **Important**: The current embeddings implementation uses a simple character-frequency
//! based algorithm, NOT neural network embeddings. This is a lightweight placeholder that
//! provides reasonable results for basic similarity detection without requiring external
//! model dependencies.
//!
//! Future versions may integrate actual transformer-based embeddings via:
//! - Candle (Rust-native ML framework)
//! - ONNX Runtime for pre-trained models
//! - External embedding services (OpenAI, Cohere, etc.)
//!
//! ## Without `embeddings` Feature
//!
//! Falls back to heuristic-based compression that:
//! - Splits content at paragraph boundaries
//! - Keeps every Nth chunk based on budget ratio
//! - No similarity computation (all operations return 0.0)

#[cfg(feature = "embeddings")]
use std::collections::HashMap;

/// Result type for semantic operations
pub type Result<T> = std::result::Result<T, SemanticError>;

/// Errors that can occur during semantic operations
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Embedding generation failed: {0}")]
    EmbeddingError(String),

    #[error("Clustering failed: {0}")]
    ClusteringError(String),

    #[error("Feature not available: embeddings feature not enabled")]
    FeatureNotEnabled,
}

// ============================================================================
// Semantic Analyzer (for similarity and embeddings)
// ============================================================================

/// Semantic analyzer using code embeddings
///
/// When the `embeddings` feature is enabled, uses the configured model path
/// for neural network-based embeddings. Without the feature, provides
/// heuristic-based similarity estimates.
#[derive(Debug)]
pub struct SemanticAnalyzer {
    /// Path to the embedding model (used when embeddings feature is enabled)
    #[cfg(feature = "embeddings")]
    model_path: Option<String>,
    /// Placeholder for non-embeddings build (maintains API compatibility)
    #[cfg(not(feature = "embeddings"))]
    _model_path: Option<String>,
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "embeddings")]
            model_path: None,
            #[cfg(not(feature = "embeddings"))]
            _model_path: None,
        }
    }

    /// Create a semantic analyzer with a custom model path
    ///
    /// The model path is used when the `embeddings` feature is enabled.
    /// Without the feature, the path is stored but not used.
    pub fn with_model(model_path: &str) -> Self {
        Self {
            #[cfg(feature = "embeddings")]
            model_path: Some(model_path.to_owned()),
            #[cfg(not(feature = "embeddings"))]
            _model_path: Some(model_path.to_owned()),
        }
    }

    /// Get the configured model path (if any)
    #[cfg(feature = "embeddings")]
    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }

    /// Generate embeddings for code content
    ///
    /// # Current Implementation
    ///
    /// Uses a character-frequency based embedding algorithm that:
    /// 1. Creates a 384-dimensional vector (matching common transformer output size)
    /// 2. Accumulates weighted character frequencies based on position
    /// 3. Normalizes to unit length for cosine similarity
    ///
    /// This is a **lightweight placeholder** that provides reasonable similarity
    /// estimates for code without requiring ML model dependencies. It captures:
    /// - Character distribution patterns
    /// - Position-weighted frequency (earlier chars weighted more)
    /// - Basic structural patterns through punctuation distribution
    ///
    /// For production use cases requiring high accuracy, consider integrating
    /// actual transformer embeddings.
    #[cfg(feature = "embeddings")]
    pub fn embed(&self, content: &str) -> Result<Vec<f32>> {
        // Character-frequency based embedding (see doc comment for rationale)
        let mut embedding = vec![0.0f32; 384];
        for (i, c) in content.chars().enumerate() {
            let idx = (c as usize) % 384;
            // Position-weighted contribution: earlier characters contribute more
            embedding[idx] += 1.0 / ((i + 1) as f32);
        }
        // L2 normalize for cosine similarity
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        Ok(embedding)
    }

    /// Generate embeddings (stub when feature disabled)
    #[cfg(not(feature = "embeddings"))]
    pub fn embed(&self, _content: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0; 384])
    }

    /// Calculate similarity between two code snippets
    #[cfg(feature = "embeddings")]
    pub fn similarity(&self, a: &str, b: &str) -> Result<f32> {
        let emb_a = self.embed(a)?;
        let emb_b = self.embed(b)?;
        Ok(cosine_similarity(&emb_a, &emb_b))
    }

    /// Calculate similarity (stub when feature disabled)
    #[cfg(not(feature = "embeddings"))]
    pub fn similarity(&self, _a: &str, _b: &str) -> Result<f32> {
        Ok(0.0)
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Semantic Compressor (for reducing content while preserving meaning)
// ============================================================================

/// Configuration for semantic compression
#[derive(Debug, Clone)]
pub struct SemanticConfig {
    /// Similarity threshold for clustering (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Minimum chunk size in characters
    pub min_chunk_size: usize,
    /// Maximum chunk size in characters
    pub max_chunk_size: usize,
    /// Budget ratio (0.0 - 1.0) - target size relative to original
    pub budget_ratio: f32,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            min_chunk_size: 100,
            max_chunk_size: 2000,
            budget_ratio: 0.5,
        }
    }
}

/// A chunk of code
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// The original content
    pub content: String,
    /// Start offset in original content
    pub start: usize,
    /// End offset in original content
    pub end: usize,
    /// Embedding vector (when computed)
    pub embedding: Option<Vec<f32>>,
    /// Cluster assignment
    pub cluster_id: Option<usize>,
}

/// Semantic compressor for code content
///
/// Uses embeddings-based clustering when the `embeddings` feature is enabled,
/// otherwise falls back to heuristic-based compression.
pub struct SemanticCompressor {
    config: SemanticConfig,
    /// Semantic analyzer for generating embeddings and computing similarity
    analyzer: SemanticAnalyzer,
}

impl SemanticCompressor {
    /// Create a new semantic compressor with default config
    pub fn new() -> Self {
        Self::with_config(SemanticConfig::default())
    }

    /// Create a new semantic compressor with custom config
    pub fn with_config(config: SemanticConfig) -> Self {
        Self { config, analyzer: SemanticAnalyzer::new() }
    }

    /// Get a reference to the internal semantic analyzer
    ///
    /// This allows access to the analyzer for similarity computations
    /// or custom embedding operations.
    pub fn analyzer(&self) -> &SemanticAnalyzer {
        &self.analyzer
    }

    /// Compress content semantically
    ///
    /// When the `embeddings` feature is enabled, uses neural embeddings
    /// to cluster similar code chunks and select representatives.
    ///
    /// Without the feature, falls back to heuristic-based compression.
    pub fn compress(&self, content: &str) -> Result<String> {
        // First, check for repetitive content (Bug #6 fix)
        if let Some(compressed) = self.compress_repetitive(content) {
            return Ok(compressed);
        }

        #[cfg(feature = "embeddings")]
        {
            return self.compress_with_embeddings(content);
        }

        #[cfg(not(feature = "embeddings"))]
        {
            self.compress_heuristic(content)
        }
    }

    /// Detect and compress repetitive content (Bug #6 fix)
    ///
    /// Handles cases like "sentence ".repeat(500) by detecting the repeated pattern
    /// and returning a compressed representation.
    fn compress_repetitive(&self, content: &str) -> Option<String> {
        // Only process content above a minimum threshold
        if content.len() < 200 {
            return None;
        }

        // Try to find a repeating pattern
        // Start with small patterns and work up
        for pattern_len in 1..=100.min(content.len() / 3) {
            let pattern = &content[..pattern_len];

            // Skip patterns that are just whitespace
            if pattern.chars().all(|c| c.is_whitespace()) {
                continue;
            }

            // Count how many times this pattern repeats consecutively
            let mut count = 0;
            let mut pos = 0;
            while pos + pattern_len <= content.len() {
                if &content[pos..pos + pattern_len] == pattern {
                    count += 1;
                    pos += pattern_len;
                } else {
                    break;
                }
            }

            // If pattern repeats enough times and covers most of the content
            let coverage = (count * pattern_len) as f32 / content.len() as f32;
            if count >= 3 && coverage >= 0.8 {
                // Calculate how many instances to keep based on budget_ratio
                let instances_to_show = (count as f32 * self.config.budget_ratio)
                    .ceil()
                    .max(1.0)
                    .min(5.0) as usize;

                let shown_content = pattern.repeat(instances_to_show);
                let remainder = &content[count * pattern_len..];

                let result = if remainder.is_empty() {
                    format!(
                        "{}\n/* ... pattern repeated {} times (showing {}) ... */",
                        shown_content.trim_end(),
                        count,
                        instances_to_show
                    )
                } else {
                    format!(
                        "{}\n/* ... pattern repeated {} times (showing {}) ... */\n{}",
                        shown_content.trim_end(),
                        count,
                        instances_to_show,
                        remainder.trim()
                    )
                };

                return Some(result);
            }
        }

        // Also detect line-based repetition (same line repeated many times)
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() >= 3 {
            let mut line_counts: std::collections::HashMap<&str, usize> =
                std::collections::HashMap::new();
            for line in &lines {
                *line_counts.entry(*line).or_insert(0) += 1;
            }

            // Find the most repeated line
            if let Some((repeated_line, count)) = line_counts
                .iter()
                .filter(|(line, _)| !line.trim().is_empty())
                .max_by_key(|(_, count)| *count)
            {
                let repetition_ratio = *count as f32 / lines.len() as f32;
                if *count >= 3 && repetition_ratio >= 0.5 {
                    // Build compressed output preserving unique lines
                    let mut result = String::new();
                    let mut consecutive_count = 0;
                    let mut last_was_repeated = false;

                    for line in &lines {
                        if *line == *repeated_line {
                            consecutive_count += 1;
                            if !last_was_repeated {
                                if !result.is_empty() {
                                    result.push('\n');
                                }
                                result.push_str(line);
                            }
                            last_was_repeated = true;
                        } else {
                            if last_was_repeated && consecutive_count > 1 {
                                result.push_str(&format!(
                                    "\n/* ... above line repeated {} times ... */",
                                    consecutive_count
                                ));
                            }
                            consecutive_count = 0;
                            last_was_repeated = false;
                            if !result.is_empty() {
                                result.push('\n');
                            }
                            result.push_str(line);
                        }
                    }

                    if last_was_repeated && consecutive_count > 1 {
                        result.push_str(&format!(
                            "\n/* ... above line repeated {} times ... */",
                            consecutive_count
                        ));
                    }

                    // Only return if we actually compressed significantly
                    if result.len() < content.len() / 2 {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Split content into semantic chunks (Bug #6 fix - handles content without \n\n)
    fn split_into_chunks(&self, content: &str) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let mut current_start = 0;

        // First try: Split on double newlines (paragraph-like boundaries)
        for (i, _) in content.match_indices("\n\n") {
            if i > current_start && i - current_start >= self.config.min_chunk_size {
                let chunk_content = &content[current_start..i];
                if chunk_content.len() <= self.config.max_chunk_size {
                    chunks.push(CodeChunk {
                        content: chunk_content.to_owned(),
                        start: current_start,
                        end: i,
                        embedding: None,
                        cluster_id: None,
                    });
                }
                current_start = i + 2;
            }
        }

        // Handle remaining content
        if current_start < content.len() {
            let remaining = &content[current_start..];
            if remaining.len() >= self.config.min_chunk_size {
                chunks.push(CodeChunk {
                    content: remaining.to_owned(),
                    start: current_start,
                    end: content.len(),
                    embedding: None,
                    cluster_id: None,
                });
            }
        }

        // Fallback: If no chunks found (no \n\n separators), try single newlines
        if chunks.is_empty() && content.len() >= self.config.min_chunk_size {
            current_start = 0;
            for (i, _) in content.match_indices('\n') {
                if i > current_start && i - current_start >= self.config.min_chunk_size {
                    let chunk_content = &content[current_start..i];
                    if chunk_content.len() <= self.config.max_chunk_size {
                        chunks.push(CodeChunk {
                            content: chunk_content.to_owned(),
                            start: current_start,
                            end: i,
                            embedding: None,
                            cluster_id: None,
                        });
                    }
                    current_start = i + 1;
                }
            }
            // Handle remaining after single newline split
            if current_start < content.len() {
                let remaining = &content[current_start..];
                if remaining.len() >= self.config.min_chunk_size {
                    chunks.push(CodeChunk {
                        content: remaining.to_owned(),
                        start: current_start,
                        end: content.len(),
                        embedding: None,
                        cluster_id: None,
                    });
                }
            }
        }

        // Second fallback: If still no chunks, split by sentence boundaries (. followed by space)
        if chunks.is_empty() && content.len() >= self.config.min_chunk_size {
            current_start = 0;
            for (i, _) in content.match_indices(". ") {
                if i > current_start && i - current_start >= self.config.min_chunk_size {
                    let chunk_content = &content[current_start..=i]; // include the period
                    if chunk_content.len() <= self.config.max_chunk_size {
                        chunks.push(CodeChunk {
                            content: chunk_content.to_owned(),
                            start: current_start,
                            end: i + 1,
                            embedding: None,
                            cluster_id: None,
                        });
                    }
                    current_start = i + 2;
                }
            }
            // Handle remaining
            if current_start < content.len() {
                let remaining = &content[current_start..];
                if remaining.len() >= self.config.min_chunk_size {
                    chunks.push(CodeChunk {
                        content: remaining.to_owned(),
                        start: current_start,
                        end: content.len(),
                        embedding: None,
                        cluster_id: None,
                    });
                }
            }
        }

        // Final fallback: If content is large but can't be split, force split by max_chunk_size
        if chunks.is_empty() && content.len() > self.config.max_chunk_size {
            let mut pos = 0;
            while pos < content.len() {
                let end = (pos + self.config.max_chunk_size).min(content.len());
                chunks.push(CodeChunk {
                    content: content[pos..end].to_owned(),
                    start: pos,
                    end,
                    embedding: None,
                    cluster_id: None,
                });
                pos = end;
            }
        }

        chunks
    }

    /// Compress using heuristic methods (fallback when embeddings unavailable)
    fn compress_heuristic(&self, content: &str) -> Result<String> {
        let chunks = self.split_into_chunks(content);

        if chunks.is_empty() {
            return Ok(content.to_owned());
        }

        // Keep every Nth chunk based on budget ratio
        let target_chunks = ((chunks.len() as f32) * self.config.budget_ratio).ceil() as usize;
        let step = chunks.len() / target_chunks.max(1);

        let mut result = String::new();
        let mut kept = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            if i % step.max(1) == 0 && kept < target_chunks {
                if !result.is_empty() {
                    result.push_str("\n\n");
                }
                result.push_str(&chunk.content);
                kept += 1;
            }
        }

        // Add truncation marker if we removed content
        if kept < chunks.len() {
            result.push_str(&format!(
                "\n\n/* ... {} chunks compressed ({:.0}% of original) ... */",
                chunks.len() - kept,
                (kept as f32 / chunks.len() as f32) * 100.0
            ));
        }

        Ok(result)
    }

    /// Compress using neural embeddings
    #[cfg(feature = "embeddings")]
    fn compress_with_embeddings(&self, content: &str) -> Result<String> {
        let mut chunks = self.split_into_chunks(content);

        if chunks.is_empty() {
            return Ok(content.to_owned());
        }

        // Generate embeddings for each chunk
        for chunk in &mut chunks {
            chunk.embedding = Some(self.analyzer.embed(&chunk.content)?);
        }

        // Cluster similar chunks
        let clusters = self.cluster_chunks(&chunks)?;

        // Select representative from each cluster
        let mut result = String::new();
        for cluster in clusters.values() {
            if let Some(representative) = self.select_representative(cluster) {
                if !result.is_empty() {
                    result.push_str("\n\n");
                }
                result.push_str(&representative.content);
            }
        }

        Ok(result)
    }

    /// Cluster chunks by embedding similarity
    #[cfg(feature = "embeddings")]
    fn cluster_chunks<'a>(
        &self,
        chunks: &'a [CodeChunk],
    ) -> Result<HashMap<usize, Vec<&'a CodeChunk>>> {
        let mut clusters: HashMap<usize, Vec<&CodeChunk>> = HashMap::new();
        let mut next_cluster = 0;

        for chunk in chunks {
            let embedding = chunk
                .embedding
                .as_ref()
                .ok_or_else(|| SemanticError::ClusteringError("Missing embedding".into()))?;

            // Find existing cluster with similar embedding
            let mut assigned = false;
            for (&cluster_id, cluster_chunks) in &clusters {
                if let Some(first) = cluster_chunks.first() {
                    if let Some(ref first_emb) = first.embedding {
                        let similarity = cosine_similarity(embedding, first_emb);
                        if similarity >= self.config.similarity_threshold {
                            clusters.get_mut(&cluster_id).unwrap().push(chunk);
                            assigned = true;
                            break;
                        }
                    }
                }
            }

            if !assigned {
                clusters.insert(next_cluster, vec![chunk]);
                next_cluster += 1;
            }
        }

        Ok(clusters)
    }

    /// Select the best representative from a cluster
    #[cfg(feature = "embeddings")]
    fn select_representative<'a>(&self, chunks: &[&'a CodeChunk]) -> Option<&'a CodeChunk> {
        // Select the longest chunk as representative (most informative)
        chunks.iter().max_by_key(|c| c.content.len()).copied()
    }
}

impl Default for SemanticCompressor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Honest Type Aliases
// ============================================================================
// The names below more accurately describe the implementation:
// - "Semantic" implies neural/ML understanding, but we use heuristics
// - These aliases are provided for clarity and recommended for new code

/// Alias for `SemanticAnalyzer` - more honest name reflecting the actual implementation.
///
/// This analyzer uses character-frequency heuristics for similarity detection,
/// NOT neural network embeddings. Use this alias when you want to be explicit
/// about the implementation approach.
pub type CharacterFrequencyAnalyzer = SemanticAnalyzer;

/// Alias for `SemanticCompressor` - more honest name reflecting the actual implementation.
///
/// This compressor uses chunk-based heuristics with optional character-frequency
/// clustering, NOT neural semantic understanding. Use this alias when you want
/// to be explicit about the implementation approach.
pub type HeuristicCompressor = SemanticCompressor;

/// Alias for `SemanticConfig` - more honest name.
pub type HeuristicCompressionConfig = SemanticConfig;

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute cosine similarity between two vectors
///
/// Returns a value between -1.0 and 1.0, where 1.0 indicates identical
/// direction, 0.0 indicates orthogonal vectors, and -1.0 indicates
/// opposite direction.
///
/// # Note
/// This function is used by the embeddings feature for clustering and
/// is also tested directly. The `#[cfg_attr]` suppresses warnings in
/// builds without the embeddings feature.
#[cfg_attr(not(feature = "embeddings"), allow(dead_code))]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = SemanticAnalyzer::new();
        // Verify analyzer is created successfully
        // Model path is None by default (accessed via model_path() when embeddings enabled)
        #[cfg(feature = "embeddings")]
        assert!(analyzer.model_path().is_none());
        #[cfg(not(feature = "embeddings"))]
        drop(analyzer); // Explicitly drop to satisfy lint
    }

    #[test]
    fn test_analyzer_with_model() {
        let analyzer = SemanticAnalyzer::with_model("/path/to/model");
        #[cfg(feature = "embeddings")]
        assert_eq!(analyzer.model_path(), Some("/path/to/model"));
        #[cfg(not(feature = "embeddings"))]
        drop(analyzer); // Explicitly drop to satisfy lint
    }

    #[test]
    fn test_compressor_analyzer_access() {
        let compressor = SemanticCompressor::new();
        // Verify we can access the analyzer through the compressor
        let _analyzer = compressor.analyzer();
    }

    #[test]
    fn test_semantic_config_default() {
        let config = SemanticConfig::default();
        assert_eq!(config.similarity_threshold, 0.7);
        assert_eq!(config.budget_ratio, 0.5);
    }

    #[test]
    fn test_split_into_chunks() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 10,
            max_chunk_size: 1000,
            ..Default::default()
        });

        let content = "First chunk here\n\nSecond chunk here\n\nThird chunk";
        let chunks = compressor.split_into_chunks(content);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_heuristic_compression() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 5,
            max_chunk_size: 100,
            budget_ratio: 0.5,
            ..Default::default()
        });

        let content = "Chunk 1\n\nChunk 2\n\nChunk 3\n\nChunk 4";
        let result = compressor.compress_heuristic(content).unwrap();
        // Should complete without error
        assert!(!result.is_empty() || content.is_empty());
    }

    #[test]
    fn test_empty_content() {
        let compressor = SemanticCompressor::new();
        let result = compressor.compress("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &c);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // Bug #6 tests - repetitive content compression
    #[test]
    fn test_repetitive_pattern_compression() {
        let compressor = SemanticCompressor::new();
        // Test "sentence ".repeat(500) - exactly the reported bug case
        let content = "sentence ".repeat(500);
        let result = compressor.compress(&content).unwrap();

        // Result should be significantly smaller than original
        assert!(
            result.len() < content.len() / 2,
            "Compressed size {} should be less than half of original {}",
            result.len(),
            content.len()
        );

        // Should contain the pattern and a compression marker
        assert!(result.contains("sentence"));
        assert!(
            result.contains("repeated") || result.contains("pattern"),
            "Should indicate compression occurred"
        );
    }

    #[test]
    fn test_repetitive_line_compression() {
        let compressor = SemanticCompressor::new();
        // Test repeated lines
        let content = "same line\n".repeat(100);
        let result = compressor.compress(&content).unwrap();

        // Result should be significantly smaller
        assert!(
            result.len() < content.len() / 2,
            "Compressed size {} should be less than half of original {}",
            result.len(),
            content.len()
        );
    }

    #[test]
    fn test_non_repetitive_content_unchanged() {
        let compressor = SemanticCompressor::new();
        // Non-repetitive content should not trigger repetition compression
        let content = "This is some unique content that does not repeat.";
        let result = compressor.compress(content).unwrap();

        // Short non-repetitive content should be returned as-is
        assert_eq!(result, content);
    }

    #[test]
    fn test_repetitive_with_variation() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.3,
            ..Default::default()
        });

        // Content with some repetition mixed with unique parts
        let mut content = String::new();
        for i in 0..50 {
            content.push_str(&format!("item {} ", i % 5)); // Repeated pattern with variation
        }

        let result = compressor.compress(&content).unwrap();
        // This may or may not compress depending on pattern detection
        // Just verify it doesn't panic
        assert!(!result.is_empty());
    }
}
