//! Safe bincode deserialization with size limits
//!
//! This module provides wrapper functions for bincode deserialization
//! that enforce size limits to prevent memory exhaustion attacks.
//!
//! Without size limits, a maliciously crafted bincode file could declare
//! a huge array size, causing bincode to attempt to allocate terabytes
//! of memory before reading the actual data.
//!
//! # Security
//!
//! All deserialization functions in this module enforce a size limit
//! based on the actual input data size plus a reasonable multiplier.
//! This prevents attackers from causing DoS via memory exhaustion.

use bincode::Options;
use serde::de::DeserializeOwned;
use std::io::Read;

/// Maximum expansion factor for deserialized data
///
/// Bincode is a compact format, so deserialized data is typically
/// larger than the serialized form. A 4x multiplier is generous
/// and handles edge cases like highly compressible data.
const MAX_EXPANSION_FACTOR: u64 = 4;

/// Minimum size limit (1MB) to handle small files
const MIN_SIZE_LIMIT: u64 = 1024 * 1024;

/// Maximum absolute size limit (1GB) as a hard cap
const MAX_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

/// Deserialize from bytes with size limit
///
/// The size limit is calculated as:
/// `min(MAX_SIZE_LIMIT, max(MIN_SIZE_LIMIT, input_len * MAX_EXPANSION_FACTOR))`
///
/// # Errors
///
/// Returns `bincode::Error` if:
/// - The data is invalid bincode
/// - Deserialization would exceed the size limit
/// - The type doesn't match the data
#[inline]
pub fn deserialize_with_limit<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, bincode::Error> {
    let limit = calculate_limit(bytes.len() as u64);
    bincode::options().with_limit(limit).deserialize(bytes)
}

/// Deserialize from a reader with size limit
///
/// # Arguments
///
/// * `reader` - The reader to deserialize from
/// * `expected_size` - Expected size of the serialized data (e.g., from file metadata)
///
/// # Errors
///
/// Returns `bincode::Error` if deserialization fails or exceeds the limit.
#[inline]
pub fn deserialize_from_with_limit<T: DeserializeOwned, R: Read>(
    reader: R,
    expected_size: u64,
) -> Result<T, bincode::Error> {
    let limit = calculate_limit(expected_size);
    bincode::options()
        .with_limit(limit)
        .deserialize_from(reader)
}

/// Calculate the size limit for a given input size
#[inline]
fn calculate_limit(input_size: u64) -> u64 {
    let expanded = input_size.saturating_mul(MAX_EXPANSION_FACTOR);
    expanded.clamp(MIN_SIZE_LIMIT, MAX_SIZE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        values: Vec<u32>,
    }

    /// Serialize using the same options as deserialize for consistent roundtrip
    fn serialize_with_options<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::Error> {
        bincode::options().serialize(value)
    }

    #[test]
    fn test_deserialize_valid() {
        let original = TestStruct { name: "test".to_owned(), values: vec![1, 2, 3, 4, 5] };

        // Use options() for serialization to match deserialization config
        let bytes = serialize_with_options(&original).unwrap();
        let restored: TestStruct = deserialize_with_limit(&bytes).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_deserialize_empty() {
        let original: Vec<u8> = Vec::new();
        let bytes = serialize_with_options(&original).unwrap();
        let restored: Vec<u8> = deserialize_with_limit(&bytes).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_calculate_limit() {
        // Small input gets MIN_SIZE_LIMIT
        assert_eq!(calculate_limit(100), MIN_SIZE_LIMIT);

        // Medium input gets expanded
        let medium = 10 * 1024 * 1024; // 10MB
        assert_eq!(calculate_limit(medium), medium * MAX_EXPANSION_FACTOR);

        // Large input gets MAX_SIZE_LIMIT
        let huge = 10 * 1024 * 1024 * 1024; // 10GB
        assert_eq!(calculate_limit(huge), MAX_SIZE_LIMIT);
    }

    #[test]
    fn test_deserialize_from_reader() {
        let original = TestStruct { name: "reader_test".to_owned(), values: vec![10, 20, 30] };

        let bytes = serialize_with_options(&original).unwrap();
        let cursor = std::io::Cursor::new(&bytes);
        let restored: TestStruct = deserialize_from_with_limit(cursor, bytes.len() as u64).unwrap();

        assert_eq!(original, restored);
    }
}
