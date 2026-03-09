//! Safe bincode deserialization with size limits
//!
//! This module provides wrapper functions for bincode serialization and
//! deserialization using bincode 2.0's serde compatibility layer.
//!
//! A compile-time size limit is enforced to prevent memory exhaustion attacks
//! from maliciously crafted bincode files.
//!
//! # Security
//!
//! All deserialization functions in this module enforce a 1GB size limit.
//! This prevents attackers from causing DoS via memory exhaustion.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

/// Maximum absolute size limit (1GB) as a hard cap.
///
/// In bincode 2.0, limits are const generics set at compile time.
/// We use a generous 1GB limit that handles all realistic use cases
/// while still preventing memory exhaustion from malicious input.
const MAX_SIZE_LIMIT: usize = 1024 * 1024 * 1024;

/// Bincode configuration: standard encoding with a 1GB size limit.
fn limited_config() -> bincode::config::Configuration<
    bincode::config::LittleEndian,
    bincode::config::Varint,
    bincode::config::Limit<MAX_SIZE_LIMIT>,
> {
    bincode::config::standard().with_limit::<MAX_SIZE_LIMIT>()
}

/// Serialize a value using bincode 2.0 serde compat layer
///
/// Uses `bincode::config::standard()` for consistent serialization.
///
/// # Errors
///
/// Returns `bincode::error::EncodeError` if serialization fails.
#[inline]
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
    bincode::serde::encode_to_vec(value, bincode::config::standard())
}

/// Serialize a value into a writer using bincode 2.0 serde compat layer
///
/// # Errors
///
/// Returns `bincode::error::EncodeError` if serialization fails.
#[inline]
pub fn serialize_into<T: Serialize, W: std::io::Write>(
    mut writer: W,
    value: &T,
) -> Result<(), bincode::error::EncodeError> {
    bincode::serde::encode_into_std_write(value, &mut writer, bincode::config::standard())?;
    Ok(())
}

/// Deserialize from bytes with size limit
///
/// Enforces a 1GB size limit to prevent memory exhaustion.
///
/// # Errors
///
/// Returns `bincode::error::DecodeError` if:
/// - The data is invalid bincode
/// - Deserialization would exceed the size limit
/// - The type doesn't match the data
#[inline]
pub fn deserialize_with_limit<T: DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, bincode::error::DecodeError> {
    let (value, _bytes_read) = bincode::serde::decode_from_slice(bytes, limited_config())?;
    Ok(value)
}

/// Deserialize from a reader with size limit
///
/// # Arguments
///
/// * `reader` - The reader to deserialize from
/// * `_expected_size` - Expected size (retained for API compatibility; limit is compile-time)
///
/// # Errors
///
/// Returns `bincode::error::DecodeError` if deserialization fails or exceeds the limit.
#[inline]
pub fn deserialize_from_with_limit<T: DeserializeOwned, R: Read>(
    mut reader: R,
    _expected_size: u64,
) -> Result<T, bincode::error::DecodeError> {
    bincode::serde::decode_from_std_read(&mut reader, limited_config())
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

    #[test]
    fn test_deserialize_valid() {
        let original = TestStruct { name: "test".to_owned(), values: vec![1, 2, 3, 4, 5] };

        let bytes = serialize(&original).unwrap();
        let restored: TestStruct = deserialize_with_limit(&bytes).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_deserialize_empty() {
        let original: Vec<u8> = Vec::new();
        let bytes = serialize(&original).unwrap();
        let restored: Vec<u8> = deserialize_with_limit(&bytes).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_deserialize_from_reader() {
        let original = TestStruct { name: "reader_test".to_owned(), values: vec![10, 20, 30] };

        let bytes = serialize(&original).unwrap();
        let cursor = std::io::Cursor::new(&bytes);
        let restored: TestStruct = deserialize_from_with_limit(cursor, bytes.len() as u64).unwrap();

        assert_eq!(original, restored);
    }
}
