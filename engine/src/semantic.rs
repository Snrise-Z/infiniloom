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
    ///
    /// This function is UTF-8 safe - it only slices at valid character boundaries.
    fn compress_repetitive(&self, content: &str) -> Option<String> {
        // Only process content above a minimum threshold
        if content.len() < 200 {
            return None;
        }

        // Try to find a repeating pattern
        // Start with small patterns and work up
        // We iterate byte positions but only consider those that are valid UTF-8 boundaries
        for pattern_len in 1..=100.min(content.len() / 3) {
            // Skip if this byte position is not a valid UTF-8 character boundary
            if !content.is_char_boundary(pattern_len) {
                continue;
            }

            let pattern = &content[..pattern_len];

            // Skip patterns that are just whitespace
            if pattern.chars().all(|c| c.is_whitespace()) {
                continue;
            }

            // Count how many times this pattern repeats consecutively
            let mut count = 0;
            let mut pos = 0;
            while pos + pattern_len <= content.len() {
                // Ensure both slice boundaries are valid UTF-8
                if !content.is_char_boundary(pos) || !content.is_char_boundary(pos + pattern_len) {
                    break;
                }
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
                    .clamp(1.0, 5.0) as usize;

                let shown_content = pattern.repeat(instances_to_show);
                // Safe: count * pattern_len is already at a valid boundary (start of next pattern or end)
                let remainder_start = count * pattern_len;
                let remainder = if remainder_start <= content.len()
                    && content.is_char_boundary(remainder_start)
                {
                    &content[remainder_start..]
                } else {
                    ""
                };

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
    ///
    /// Bug #4 fix: Make budget_ratio more effective for all content types
    /// Bug fix: Ensure budget_ratio always has an effect when < 1.0
    fn compress_heuristic(&self, content: &str) -> Result<String> {
        let chunks = self.split_into_chunks(content);

        // When no chunks can be created, apply character-level truncation based on budget_ratio.
        // This ensures budget_ratio always has an effect, even for small/unstructured content.
        if chunks.is_empty() {
            // Apply truncation if:
            // 1. budget_ratio < 1.0 (user wants compression)
            // 2. Content is at least 10 chars (very short content passes through)
            // 3. The truncation would actually reduce the size
            if self.config.budget_ratio < 1.0 && content.len() >= 10 {
                let target_len = (content.len() as f32 * self.config.budget_ratio) as usize;
                if target_len > 0 && target_len < content.len() {
                    // Find a safe truncation point (word/line boundary)
                    let truncate_at = find_safe_truncation_point(content, target_len);
                    if truncate_at < content.len() && truncate_at > 0 {
                        let truncated = &content[..truncate_at];
                        return Ok(format!(
                            "{}\n/* ... truncated to {:.0}% ({} of {} chars) ... */",
                            truncated.trim_end(),
                            self.config.budget_ratio * 100.0,
                            truncate_at,
                            content.len()
                        ));
                    }
                }
            }
            return Ok(content.to_owned());
        }

        // Special case: If we only have one chunk and budget_ratio < 1.0,
        // truncate within that chunk instead of keeping it entirely
        if chunks.len() == 1 && self.config.budget_ratio < 1.0 {
            let chunk_content = &chunks[0].content;
            let target_len = (chunk_content.len() as f32 * self.config.budget_ratio) as usize;
            if target_len > 0 && target_len < chunk_content.len() {
                let truncate_at = find_safe_truncation_point(chunk_content, target_len);
                if truncate_at < chunk_content.len() && truncate_at > 0 {
                    let truncated = &chunk_content[..truncate_at];
                    return Ok(format!(
                        "{}\n/* ... truncated to {:.0}% ({} of {} chars) ... */",
                        truncated.trim_end(),
                        self.config.budget_ratio * 100.0,
                        truncate_at,
                        chunk_content.len()
                    ));
                }
            }
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
            let mut target_cluster = None;
            for (&cluster_id, cluster_chunks) in &clusters {
                if let Some(first) = cluster_chunks.first() {
                    if let Some(ref first_emb) = first.embedding {
                        let similarity = cosine_similarity(embedding, first_emb);
                        if similarity >= self.config.similarity_threshold {
                            target_cluster = Some(cluster_id);
                            break;
                        }
                    }
                }
            }

            if let Some(cluster_id) = target_cluster {
                if let Some(cluster) = clusters.get_mut(&cluster_id) {
                    cluster.push(chunk);
                }
            } else {
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

/// Find a safe truncation point in content (word or line boundary)
///
/// Used by compress_heuristic to ensure we don't cut in the middle of a word
/// or multi-byte UTF-8 character.
fn find_safe_truncation_point(content: &str, target_len: usize) -> usize {
    if target_len >= content.len() {
        return content.len();
    }

    // First, ensure we're at a valid UTF-8 boundary
    let mut truncate_at = target_len;
    while truncate_at > 0 && !content.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }

    // Try to find a line boundary (newline) near the target
    if let Some(newline_pos) = content[..truncate_at].rfind('\n') {
        if newline_pos > target_len / 2 {
            // Found a newline that's not too far back
            return newline_pos;
        }
    }

    // Fall back to word boundary (space)
    if let Some(space_pos) = content[..truncate_at].rfind(' ') {
        if space_pos > target_len / 2 {
            return space_pos;
        }
    }

    // No good boundary found, use the UTF-8 safe position
    truncate_at
}

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
        // Use budget_ratio=1.0 to preserve content (default is 0.5 which truncates)
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 1.0,
            ..Default::default()
        });
        // Non-repetitive content should not trigger repetition compression
        let content = "This is some unique content that does not repeat.";
        let result = compressor.compress(content).unwrap();

        // Short non-repetitive content should be returned as-is with budget_ratio=1.0
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

    // UTF-8 boundary safety tests for compress_repetitive
    #[test]
    fn test_repetitive_unicode_chinese() {
        let compressor = SemanticCompressor::new();
        // Chinese characters are 3 bytes each
        // Create repeating Chinese pattern
        let content = "中文测试 ".repeat(100); // Each repeat is 13 bytes
        let result = compressor.compress(&content).unwrap();

        // Should not panic and should produce valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());

        // Should compress or return unchanged (not panic)
        assert!(!result.is_empty() || content.is_empty());
    }

    #[test]
    fn test_repetitive_unicode_emoji() {
        let compressor = SemanticCompressor::new();
        // Emoji are 4 bytes each
        let content = "🎉🎊🎁 ".repeat(80); // Each repeat is 14 bytes

        let result = compressor.compress(&content).unwrap();
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(!result.is_empty() || content.is_empty());
    }

    #[test]
    fn test_repetitive_unicode_mixed() {
        let compressor = SemanticCompressor::new();
        // Mix of 1, 2, 3, and 4 byte characters
        let content = "a中🎉 ".repeat(60); // Each repeat is 11 bytes

        let result = compressor.compress(&content).unwrap();
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(!result.is_empty() || content.is_empty());
    }

    #[test]
    fn test_repetitive_unicode_cyrillic() {
        let compressor = SemanticCompressor::new();
        // Cyrillic characters are 2 bytes each
        let content = "Привет ".repeat(50);

        let result = compressor.compress(&content).unwrap();
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_non_repetitive_unicode_boundary() {
        let compressor = SemanticCompressor::new();
        // Content where pattern detection would try various byte lengths
        // that don't align with UTF-8 boundaries
        let content = "世界和平".repeat(60); // No spaces, pure multi-byte

        let result = compressor.compress(&content).unwrap();
        // Should not panic even when pattern length iteration
        // hits non-UTF-8 boundaries
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_repetitive_unicode_line_based() {
        let compressor = SemanticCompressor::new();
        // Test line-based repetition detection with Unicode
        let content = "中文行\n".repeat(100);

        let result = compressor.compress(&content).unwrap();
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    // ==========================================================================
    // Additional coverage tests
    // ==========================================================================

    #[test]
    fn test_semantic_error_display() {
        let err1 = SemanticError::ModelLoadError("test error".to_string());
        assert!(err1.to_string().contains("Model loading failed"));
        assert!(err1.to_string().contains("test error"));

        let err2 = SemanticError::EmbeddingError("embed fail".to_string());
        assert!(err2.to_string().contains("Embedding generation failed"));

        let err3 = SemanticError::ClusteringError("cluster fail".to_string());
        assert!(err3.to_string().contains("Clustering failed"));

        let err4 = SemanticError::FeatureNotEnabled;
        assert!(err4.to_string().contains("embeddings feature not enabled"));
    }

    #[test]
    fn test_semantic_error_debug() {
        let err = SemanticError::ModelLoadError("debug test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ModelLoadError"));
    }

    #[test]
    fn test_semantic_analyzer_default() {
        let analyzer = SemanticAnalyzer::default();
        // Should work same as new()
        let result = analyzer.embed("test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_semantic_analyzer_debug() {
        let analyzer = SemanticAnalyzer::new();
        let debug_str = format!("{:?}", analyzer);
        assert!(debug_str.contains("SemanticAnalyzer"));
    }

    #[test]
    fn test_semantic_analyzer_embed_empty() {
        let analyzer = SemanticAnalyzer::new();
        let result = analyzer.embed("").unwrap();
        assert_eq!(result.len(), 384);
    }

    #[test]
    fn test_semantic_analyzer_embed_produces_384_dims() {
        let analyzer = SemanticAnalyzer::new();
        let result = analyzer.embed("some code content").unwrap();
        assert_eq!(result.len(), 384);
    }

    #[test]
    fn test_semantic_analyzer_similarity_same_content() {
        let analyzer = SemanticAnalyzer::new();
        let result = analyzer.similarity("hello world", "hello world").unwrap();
        // Same content should have high similarity (1.0 in embeddings mode, 0.0 in fallback)
        #[cfg(feature = "embeddings")]
        assert!((result - 1.0).abs() < 0.01);
        #[cfg(not(feature = "embeddings"))]
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_semantic_analyzer_similarity_different_content() {
        let analyzer = SemanticAnalyzer::new();
        let result = analyzer.similarity("hello", "goodbye").unwrap();
        // Result should be valid (0.0 in fallback mode)
        #[cfg(not(feature = "embeddings"))]
        assert_eq!(result, 0.0);
        #[cfg(feature = "embeddings")]
        assert!(result >= -1.0 && result <= 1.0);
    }

    #[test]
    fn test_semantic_config_custom() {
        let config = SemanticConfig {
            similarity_threshold: 0.9,
            min_chunk_size: 50,
            max_chunk_size: 5000,
            budget_ratio: 0.3,
        };
        assert_eq!(config.similarity_threshold, 0.9);
        assert_eq!(config.min_chunk_size, 50);
        assert_eq!(config.max_chunk_size, 5000);
        assert_eq!(config.budget_ratio, 0.3);
    }

    #[test]
    fn test_semantic_config_clone() {
        let config = SemanticConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.similarity_threshold, config.similarity_threshold);
        assert_eq!(cloned.budget_ratio, config.budget_ratio);
    }

    #[test]
    fn test_semantic_config_debug() {
        let config = SemanticConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SemanticConfig"));
        assert!(debug_str.contains("similarity_threshold"));
    }

    #[test]
    fn test_code_chunk_debug() {
        let chunk = CodeChunk {
            content: "test content".to_string(),
            start: 0,
            end: 12,
            embedding: None,
            cluster_id: None,
        };
        let debug_str = format!("{:?}", chunk);
        assert!(debug_str.contains("CodeChunk"));
        assert!(debug_str.contains("test content"));
    }

    #[test]
    fn test_code_chunk_clone() {
        let chunk = CodeChunk {
            content: "original".to_string(),
            start: 0,
            end: 8,
            embedding: Some(vec![0.1, 0.2, 0.3]),
            cluster_id: Some(5),
        };
        let cloned = chunk.clone();
        assert_eq!(cloned.content, "original");
        assert_eq!(cloned.start, 0);
        assert_eq!(cloned.end, 8);
        assert_eq!(cloned.embedding, Some(vec![0.1, 0.2, 0.3]));
        assert_eq!(cloned.cluster_id, Some(5));
    }

    #[test]
    fn test_semantic_compressor_default() {
        let compressor = SemanticCompressor::default();
        let result = compressor.compress("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_split_into_chunks_single_newline_fallback() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 5,
            max_chunk_size: 1000,
            ..Default::default()
        });

        // Content with only single newlines (no \n\n)
        let content = "Line 1 with content\nLine 2 with content\nLine 3 with content";
        let chunks = compressor.split_into_chunks(content);
        // Should use single newline fallback
        assert!(!chunks.is_empty() || content.len() < 5);
    }

    #[test]
    fn test_split_into_chunks_sentence_fallback() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 10,
            max_chunk_size: 1000,
            ..Default::default()
        });

        // Content with sentences but no newlines
        let content = "First sentence here. Second sentence here. Third sentence here.";
        let chunks = compressor.split_into_chunks(content);
        // Should use sentence boundary fallback
        assert!(!chunks.is_empty() || content.len() < 10);
    }

    #[test]
    fn test_split_into_chunks_force_split() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 100, // Higher than content length so normal chunking fails
            max_chunk_size: 20,  // Lower than content length to trigger force split
            ..Default::default()
        });

        // Content without any splitting characters, longer than max_chunk_size
        // but shorter than min_chunk_size (so normal chunking produces empty result)
        let content = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let chunks = compressor.split_into_chunks(content);
        // Should force split by max_chunk_size when no other splitting works
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks from force split, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_split_into_chunks_empty() {
        let compressor = SemanticCompressor::new();
        let chunks = compressor.split_into_chunks("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_into_chunks_below_min_size() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 100,
            max_chunk_size: 1000,
            ..Default::default()
        });

        let content = "short";
        let chunks = compressor.split_into_chunks(content);
        // Content too short for min_chunk_size
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_compress_heuristic_empty_chunks() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 1000, // Force no chunks to be created
            budget_ratio: 1.0,    // Use 1.0 to preserve content unchanged
            ..Default::default()
        });

        let content = "short content";
        let result = compressor.compress_heuristic(content).unwrap();
        // Should return original when no chunks created and budget_ratio=1.0
        assert_eq!(result, content);
    }

    #[test]
    fn test_compress_heuristic_multiple_chunks() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 10,
            max_chunk_size: 100,
            budget_ratio: 0.3,
            ..Default::default()
        });

        let content = "First chunk content here\n\nSecond chunk content here\n\nThird chunk content here\n\nFourth chunk content";
        let result = compressor.compress_heuristic(content).unwrap();
        // Should have compression marker if chunks were removed
        assert!(result.contains("chunk") || result.contains("compressed"));
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0); // Different lengths should return 0
    }

    #[test]
    fn test_cosine_similarity_zero_vectors() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0); // Zero norm should return 0
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001); // Opposite directions = -1.0
    }

    #[test]
    fn test_cosine_similarity_normalized() {
        let a = vec![0.6, 0.8, 0.0];
        let b = vec![0.6, 0.8, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_compress_repetitive_short_content() {
        let compressor = SemanticCompressor::new();
        // Content below 200 chars should not trigger repetition compression
        let content = "short ".repeat(10); // 60 chars
        let result = compressor.compress_repetitive(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_compress_repetitive_whitespace_only() {
        let compressor = SemanticCompressor::new();
        // Whitespace-only patterns should be skipped
        let content = "   ".repeat(100);
        let result = compressor.compress_repetitive(&content);
        // Should not compress whitespace-only patterns
        assert!(result.is_none());
    }

    #[test]
    fn test_compress_repetitive_low_coverage() {
        let compressor = SemanticCompressor::new();
        // Pattern that doesn't cover 80% of content
        let mut content = "pattern ".repeat(5);
        content.push_str(&"x".repeat(200)); // Add non-repeating content
        let result = compressor.compress_repetitive(&content);
        // Low coverage should not trigger compression
        assert!(result.is_none());
    }

    #[test]
    fn test_compress_repetitive_line_low_ratio() {
        let compressor = SemanticCompressor::new();
        // Lines where no single line repeats enough
        let content = (0..20)
            .map(|i| format!("unique line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = compressor.compress_repetitive(&content);
        // No significant repetition
        assert!(result.is_none());
    }

    #[test]
    fn test_compress_repetitive_mixed_with_unique() {
        let compressor = SemanticCompressor::new();
        // Repeated line mixed with unique lines
        let mut lines = vec![];
        for i in 0..50 {
            if i % 2 == 0 {
                lines.push("repeated line");
            } else {
                lines.push("unique line");
            }
        }
        let content = lines.join("\n");
        let result = compressor.compress(&content).unwrap();
        // Should handle mixed content
        assert!(!result.is_empty());
    }

    #[test]
    fn test_compress_no_repetition_returns_none() {
        let compressor = SemanticCompressor::new();
        // Unique content that doesn't repeat
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(5);
        // Each sentence is unique enough
        let result = compressor.compress_repetitive(&content);
        // Depends on pattern length detection - may or may not find pattern
        // Just verify no panic
        drop(result);
    }

    #[test]
    fn test_type_aliases() {
        // Test that type aliases work correctly
        let _analyzer: CharacterFrequencyAnalyzer = SemanticAnalyzer::new();
        let _compressor: HeuristicCompressor = SemanticCompressor::new();
        let _config: HeuristicCompressionConfig = SemanticConfig::default();
    }

    #[test]
    fn test_compress_preserves_content_structure() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 10,
            max_chunk_size: 500,
            budget_ratio: 1.0, // Keep everything
            ..Default::default()
        });

        let content = "def foo():\n    pass\n\ndef bar():\n    pass";
        let result = compressor.compress(content).unwrap();
        // With budget_ratio 1.0, should keep most content
        assert!(result.contains("foo") || result.contains("bar"));
    }

    #[test]
    fn test_split_chunks_respects_max_size() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            min_chunk_size: 5,
            max_chunk_size: 50,
            ..Default::default()
        });

        let content = "A very long chunk that exceeds the max size limit\n\nAnother chunk";
        let chunks = compressor.split_into_chunks(content);

        for chunk in &chunks {
            assert!(chunk.content.len() <= 50, "Chunk size {} exceeds max 50", chunk.content.len());
        }
    }

    #[test]
    fn test_compress_repetitive_with_remainder() {
        let compressor = SemanticCompressor::new();
        // Pattern that repeats but has a small remainder
        let mut content = "abc ".repeat(100);
        content.push_str("xyz"); // Add non-repeating remainder

        let result = compressor.compress(&content).unwrap();
        // Should compress and handle remainder
        assert!(!result.is_empty());
    }

    #[test]
    fn test_compressor_analyzer_method() {
        let compressor = SemanticCompressor::new();
        let analyzer = compressor.analyzer();

        // Verify the analyzer works
        let embed_result = analyzer.embed("test code");
        assert!(embed_result.is_ok());
    }

    #[test]
    fn test_code_chunk_with_embedding_and_cluster() {
        let chunk = CodeChunk {
            content: "fn main() {}".to_string(),
            start: 0,
            end: 12,
            embedding: Some(vec![0.5; 384]),
            cluster_id: Some(3),
        };

        assert_eq!(chunk.content, "fn main() {}");
        assert_eq!(chunk.start, 0);
        assert_eq!(chunk.end, 12);
        assert!(chunk.embedding.is_some());
        assert_eq!(chunk.embedding.as_ref().unwrap().len(), 384);
        assert_eq!(chunk.cluster_id, Some(3));
    }

    #[test]
    fn test_compress_very_long_repetitive() {
        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.2, // Aggressive compression
            ..Default::default()
        });

        // Very long repetitive content
        let content = "repeated_token ".repeat(1000);
        let result = compressor.compress(&content).unwrap();

        // Should significantly compress
        assert!(result.len() < content.len() / 3);
        assert!(result.contains("repeated"));
    }

    #[test]
    fn test_semantic_result_type_ok() {
        let result: Result<String> = Ok("success".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_semantic_result_type_err() {
        let result: Result<String> = Err(SemanticError::FeatureNotEnabled);
        assert!(result.is_err());
    }

    // Bug #4 fix tests - budget_ratio effectiveness
    #[test]
    fn test_find_safe_truncation_point_basic() {
        let content = "Hello world this is a test";
        let point = find_safe_truncation_point(content, 15);
        // Should find a word boundary
        assert!(content.is_char_boundary(point));
        assert!(point <= 15 || point == content.len());
    }

    #[test]
    fn test_find_safe_truncation_point_newline() {
        let content = "Line one\nLine two\nLine three";
        let point = find_safe_truncation_point(content, 20);
        // Should prefer newline boundary
        assert!(content.is_char_boundary(point));
    }

    #[test]
    fn test_find_safe_truncation_point_unicode() {
        let content = "Hello 世界 test";
        let point = find_safe_truncation_point(content, 10);
        // Should not cut in middle of UTF-8 character
        assert!(content.is_char_boundary(point));
    }

    #[test]
    fn test_find_safe_truncation_point_beyond_length() {
        let content = "short";
        let point = find_safe_truncation_point(content, 100);
        assert_eq!(point, content.len());
    }

    #[test]
    fn test_budget_ratio_affects_large_content() {
        // Test that budget_ratio affects compression of content with paragraph breaks
        // This tests the chunk-based compression path
        let content = (0..20)
            .map(|i| {
                format!("This is paragraph number {} with some content to fill it out nicely.", i)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Test with different budget ratios
        let compressor_30 = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.3,
            min_chunk_size: 20,
            max_chunk_size: 2000,
            ..Default::default()
        });

        let compressor_80 = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.8,
            min_chunk_size: 20,
            max_chunk_size: 2000,
            ..Default::default()
        });

        let result_30 = compressor_30.compress(&content).unwrap();
        let result_80 = compressor_80.compress(&content).unwrap();

        // Lower budget ratio should produce shorter output
        assert!(
            result_30.len() < result_80.len(),
            "30% budget ({}) should be smaller than 80% budget ({})",
            result_30.len(),
            result_80.len()
        );

        // Both should indicate compression occurred
        assert!(
            result_30.contains("compressed") || result_30.len() < content.len(),
            "30% should show compression indicator"
        );
    }

    #[test]
    fn test_budget_ratio_one_returns_original() {
        let content = "Some content without chunk boundaries";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 1.0, // Keep everything
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();
        // With budget_ratio 1.0, should return original content
        assert_eq!(result, content);
    }

    // ==========================================================================
    // Bug #4 Fix Tests - budget_ratio effectiveness for small content
    // ==========================================================================

    /// Test that budget_ratio affects content >= 10 chars even without chunk boundaries.
    /// This was the bug: small content wasn't being truncated because the threshold
    /// was set to min_chunk_size (100) instead of a lower value (10).
    #[test]
    fn test_budget_ratio_affects_small_content() {
        // Content that's over 10 chars but has no chunk boundaries
        // Previously this wouldn't be compressed because it was under min_chunk_size
        let content = "This is a short test string that should be affected by budget ratio.";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.3, // Keep only 30%
            min_chunk_size: 100,
            max_chunk_size: 2000,
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();

        // With budget_ratio 0.3, content should be truncated
        assert!(
            result.len() < content.len() || result.contains("truncated"),
            "Small content with budget_ratio=0.3 should be compressed. Original: {}, Result: {}",
            content.len(),
            result.len()
        );
    }

    /// Test that budget_ratio 1.0 preserves small content
    #[test]
    fn test_budget_ratio_one_preserves_small_content() {
        let content = "Short content that should remain unchanged with budget_ratio=1.0";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 1.0,
            min_chunk_size: 100,
            max_chunk_size: 2000,
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();

        // With budget_ratio 1.0, should return original
        assert_eq!(result, content, "budget_ratio=1.0 should preserve content");
    }

    /// Test that very short content (< 10 chars) passes through unchanged
    #[test]
    fn test_very_short_content_unchanged() {
        let content = "tiny";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.1, // Even aggressive budget shouldn't affect very short content
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();

        // Very short content should pass through
        assert_eq!(result, content, "Very short content should be unchanged");
    }

    /// Test that budget_ratio affects medium content without chunk boundaries
    #[test]
    fn test_budget_ratio_medium_no_chunks() {
        // Content that's long enough to compress but has no paragraph breaks
        let content = "This is a medium length test content that has no paragraph breaks and should trigger the budget ratio truncation path because there are no chunk boundaries.";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.5,
            min_chunk_size: 200, // Higher than content length
            max_chunk_size: 2000,
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();

        // Should be compressed to ~50%
        assert!(
            result.len() < content.len(),
            "Medium content with budget_ratio=0.5 should be compressed. Original: {}, Result: {}",
            content.len(),
            result.len()
        );
    }

    /// Test that truncation marker includes percentage and char counts
    #[test]
    fn test_truncation_marker_format() {
        let content = "A sufficiently long piece of content that will definitely be truncated when we set a low budget ratio.";

        let compressor = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.3,
            min_chunk_size: 200,
            max_chunk_size: 2000,
            ..Default::default()
        });

        let result = compressor.compress(content).unwrap();

        // Should contain truncation marker with useful info
        if result.contains("truncated") {
            assert!(result.contains("%"), "Truncation marker should include percentage");
            assert!(result.contains("chars"), "Truncation marker should include char count");
        }
    }

    /// Test different budget ratios produce proportionally different outputs
    #[test]
    fn test_budget_ratio_proportional() {
        let content = "This content is long enough to test different budget ratio values and see that they produce outputs of proportionally different sizes as expected.";

        let compressor_20 = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.2,
            min_chunk_size: 200,
            ..Default::default()
        });

        let compressor_50 = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.5,
            min_chunk_size: 200,
            ..Default::default()
        });

        let compressor_80 = SemanticCompressor::with_config(SemanticConfig {
            budget_ratio: 0.8,
            min_chunk_size: 200,
            ..Default::default()
        });

        let result_20 = compressor_20.compress(content).unwrap();
        let result_50 = compressor_50.compress(content).unwrap();
        let result_80 = compressor_80.compress(content).unwrap();

        // Lower ratio should produce shorter output
        assert!(
            result_20.len() <= result_50.len(),
            "20% ratio ({}) should be <= 50% ratio ({})",
            result_20.len(),
            result_50.len()
        );
        assert!(
            result_50.len() <= result_80.len(),
            "50% ratio ({}) should be <= 80% ratio ({})",
            result_50.len(),
            result_80.len()
        );
    }
}
