//! BLAKE3-based content hashing for deterministic chunk IDs
//!
//! This module provides fast, cryptographically secure hashing for:
//! - Chunk ID generation (128-bit truncated, collision-resistant)
//! - Content verification (full 256-bit hash)
//! - Manifest integrity checksums
//!
//! # Hash Format
//!
//! - **Short ID**: `ec_` + 32 hex chars (128 bits of BLAKE3)
//!   - Collision-resistant for ~2^64 chunks (enterprise scale)
//!   - Human-readable prefix identifies embedding chunks
//!
//! - **Full hash**: 64 hex chars (256 bits of BLAKE3)
//!   - Used for collision verification
//!   - Stored in manifest for integrity checking
//!
//! # Performance
//!
//! BLAKE3 is extremely fast:
//! - ~3x faster than SHA-256
//! - ~6x faster than SHA-512
//! - Parallelizable for large inputs
//! - SIMD-accelerated on modern CPUs

use super::error::EmbedError;
use super::normalizer::normalize_for_hash;

/// Result of hashing content, containing both short ID and full hash
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashResult {
    /// Short ID for display and indexing: "ec_" + 32 hex chars (128 bits)
    /// Collision-resistant for ~2^64 unique chunks
    pub short_id: String,

    /// Full hash for collision detection: 64 hex chars (256 bits)
    /// Used to verify that same short_id means same content
    pub full_hash: String,
}

impl HashResult {
    /// Create a new HashResult from raw hash bytes
    #[inline]
    fn from_hash(hash: blake3::Hash) -> Self {
        let hex = hash.to_hex();

        Self {
            // 128 bits = 32 hex chars (collision resistant for 2^64 chunks)
            short_id: format!("ec_{}", &hex[..32]),
            // Full 256-bit hash for verification
            full_hash: hex.to_string(),
        }
    }
}

/// Generate deterministic hashes from content
///
/// This is the primary hashing function. It:
/// 1. Normalizes content for cross-platform consistency
/// 2. Computes BLAKE3 hash of normalized content
/// 3. Returns both short ID and full hash
///
/// # Example
///
/// ```
/// use infiniloom_engine::embedding::hash_content;
///
/// let result = hash_content("fn foo() { bar(); }");
/// assert!(result.short_id.starts_with("ec_"));
/// assert_eq!(result.short_id.len(), 3 + 32); // "ec_" + 32 hex chars
/// assert_eq!(result.full_hash.len(), 64);    // 256 bits = 64 hex chars
/// ```
#[inline]
pub fn hash_content(content: &str) -> HashResult {
    let normalized = normalize_for_hash(content);
    let hash = blake3::hash(normalized.as_bytes());
    HashResult::from_hash(hash)
}

/// Hash content that is already normalized
///
/// Use this when you've already called `normalize_for_hash` on the content.
/// Skips redundant normalization for better performance.
///
/// # Safety
///
/// The caller must ensure the content is already normalized. If not,
/// the hash will be different from `hash_content()` for the same original content.
#[inline]
pub(super) fn hash_normalized(normalized_content: &str) -> HashResult {
    let hash = blake3::hash(normalized_content.as_bytes());
    HashResult::from_hash(hash)
}

/// Hash raw bytes without normalization
///
/// Use for non-text content or when you need raw byte hashing.
#[inline]
pub(super) fn hash_bytes(bytes: &[u8]) -> HashResult {
    let hash = blake3::hash(bytes);
    HashResult::from_hash(hash)
}

/// Verify that two chunks with the same short ID have the same content
///
/// This detects hash collisions (extremely rare but possible).
/// Call this when you encounter a chunk with an existing short ID.
///
/// # Returns
///
/// - `Ok(())` if hashes match (no collision)
/// - `Err(HashCollision)` if hashes differ (collision detected)
pub(super) fn verify_no_collision(id: &str, hash1: &str, hash2: &str) -> Result<(), EmbedError> {
    if hash1 != hash2 {
        return Err(EmbedError::HashCollision {
            id: id.to_owned(),
            hash1: hash1.to_owned(),
            hash2: hash2.to_owned(),
        });
    }
    Ok(())
}

/// Compute a hash for manifest integrity verification
///
/// Used to detect tampering with the manifest file.
pub(super) fn compute_integrity_hash(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Incrementally hash multiple pieces of data
///
/// More efficient than concatenating strings when hashing multiple items.
///
/// # Example
///
/// ```ignore
/// let mut hasher = IncrementalHasher::new();
/// hasher.update(b"settings json");
/// hasher.update(b"chunk1");
/// hasher.update(b"chunk2");
/// let result = hasher.finalize();
/// ```
pub(super) struct IncrementalHasher {
    hasher: blake3::Hasher,
}

impl IncrementalHasher {
    /// Create a new incremental hasher
    #[inline]
    pub(super) fn new() -> Self {
        Self { hasher: blake3::Hasher::new() }
    }

    /// Update the hash with additional data
    #[inline]
    pub(super) fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Update the hash with a string
    #[inline]
    pub(super) fn update_str(&mut self, s: &str) {
        self.hasher.update(s.as_bytes());
    }

    /// Update with a u32 value (little-endian)
    #[inline]
    pub(super) fn update_u32(&mut self, n: u32) {
        self.hasher.update(&n.to_le_bytes());
    }

    /// Update with a u64 value (little-endian)
    #[inline]
    pub(super) fn update_u64(&mut self, n: u64) {
        self.hasher.update(&n.to_le_bytes());
    }

    /// Finalize and return the hash result
    #[inline]
    pub(super) fn finalize(self) -> HashResult {
        HashResult::from_hash(self.hasher.finalize())
    }

    /// Finalize and return just the hex string (256 bits)
    #[inline]
    pub(super) fn finalize_hex(self) -> String {
        self.hasher.finalize().to_hex().to_string()
    }
}

impl Default for IncrementalHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let content = "fn foo() { bar(); }";
        let h1 = hash_content(content);
        let h2 = hash_content(content);

        assert_eq!(h1.short_id, h2.short_id);
        assert_eq!(h1.full_hash, h2.full_hash);
    }

    #[test]
    fn test_format() {
        let h = hash_content("test");

        assert!(h.short_id.starts_with("ec_"));
        assert_eq!(h.short_id.len(), 3 + 32); // "ec_" + 32 hex
        assert_eq!(h.full_hash.len(), 64); // 256 bits = 64 hex
    }

    #[test]
    fn test_different_content() {
        let h1 = hash_content("fn foo() {}");
        let h2 = hash_content("fn bar() {}");

        assert_ne!(h1.short_id, h2.short_id);
        assert_ne!(h1.full_hash, h2.full_hash);
    }

    #[test]
    fn test_cross_platform_consistency() {
        // These should all produce the same hash after normalization
        let variants = [
            "fn foo() {\n    bar();\n}",
            "fn foo() {\r\n    bar();\r\n}",
            "fn foo() {\r    bar();\r}",
            "fn foo() {   \n    bar();   \n}",
        ];

        let hashes: Vec<_> = variants.iter().map(|c| hash_content(c)).collect();

        for i in 1..hashes.len() {
            assert_eq!(hashes[0].short_id, hashes[i].short_id, "Hash mismatch for variant {i}");
        }
    }

    #[test]
    fn test_unicode_consistency() {
        // NFD: e + combining acute accent
        let nfd = "cafe\u{0301}";
        // NFC: single character é
        let nfc = "caf\u{00E9}";

        let h1 = hash_content(nfd);
        let h2 = hash_content(nfc);

        assert_eq!(h1.short_id, h2.short_id);
    }

    #[test]
    fn test_verify_no_collision_ok() {
        let result = verify_no_collision("ec_test", "abc123", "abc123");
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_no_collision_detected() {
        let result = verify_no_collision("ec_test", "abc123", "def456");
        assert!(result.is_err());
        assert!(matches!(result, Err(EmbedError::HashCollision { .. })));
    }

    #[test]
    fn test_hash_normalized() {
        let content = "fn foo() { bar(); }";
        let normalized = normalize_for_hash(content);

        let h1 = hash_content(content);
        let h2 = hash_normalized(&normalized);

        assert_eq!(h1.short_id, h2.short_id);
        assert_eq!(h1.full_hash, h2.full_hash);
    }

    #[test]
    fn test_hash_bytes() {
        let bytes = b"hello world";
        let result = hash_bytes(bytes);

        assert!(result.short_id.starts_with("ec_"));
        assert_eq!(result.full_hash.len(), 64);
    }

    #[test]
    fn test_incremental_hasher() {
        // Concatenated hash
        let concat = "part1part2part3";
        let h1 = hash_bytes(concat.as_bytes());

        // Incremental hash
        let mut hasher = IncrementalHasher::new();
        hasher.update(b"part1");
        hasher.update(b"part2");
        hasher.update(b"part3");
        let h2 = hasher.finalize();

        assert_eq!(h1.short_id, h2.short_id);
    }

    #[test]
    fn test_incremental_with_numbers() {
        let mut hasher = IncrementalHasher::new();
        hasher.update_u32(42);
        hasher.update_u64(123456789);
        hasher.update_str("test");
        let result = hasher.finalize_hex();

        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_compute_integrity_hash() {
        let data = b"manifest data here";
        let hash = compute_integrity_hash(data);

        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_empty_content() {
        let h1 = hash_content("");
        let h2 = hash_content("\n\n\n"); // Normalizes to empty

        assert_eq!(h1.short_id, h2.short_id);
    }

    #[test]
    fn test_whitespace_only() {
        let h1 = hash_content("   ");
        let h2 = hash_content("  \n  \n  ");

        // Both normalize to empty
        assert_eq!(h1.short_id, h2.short_id);
    }

    #[test]
    fn test_hash_result_clone() {
        let result = hash_content("test");
        let cloned = result.clone();

        assert_eq!(result, cloned);
    }
}
