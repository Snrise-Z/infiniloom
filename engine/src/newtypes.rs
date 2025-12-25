//! Type-safe wrappers for primitive types
//!
//! This module provides newtype wrappers that prevent accidentally mixing
//! different kinds of values (e.g., token counts vs line counts).
//!
//! # Example
//!
//! ```rust
//! use infiniloom_engine::newtypes::{TokenCount, LineNumber, ByteOffset};
//!
//! let tokens = TokenCount::new(1000);
//! let line = LineNumber::new(42);
//!
//! // These are different types and can't be accidentally mixed
//! // tokens + line; // This would be a compile error
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A count of tokens (for LLM context budgeting)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct TokenCount(u32);

impl TokenCount {
    /// Create a new token count
    #[inline]
    pub const fn new(count: u32) -> Self {
        Self(count)
    }

    /// Create a zero token count
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Get the inner value
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Check if this is zero
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Saturating subtraction
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Saturating addition
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Calculate percentage of another token count
    #[inline]
    pub fn percentage_of(self, total: Self) -> f32 {
        if total.0 == 0 {
            0.0
        } else {
            (self.0 as f32 / total.0 as f32) * 100.0
        }
    }
}

impl Add for TokenCount {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for TokenCount {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for TokenCount {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for TokenCount {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl From<u32> for TokenCount {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<TokenCount> for u32 {
    #[inline]
    fn from(value: TokenCount) -> Self {
        value.0
    }
}

impl fmt::Display for TokenCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} tokens", self.0)
    }
}

impl std::iter::Sum for TokenCount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
    }
}

/// A 1-indexed line number in source code
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct LineNumber(u32);

impl LineNumber {
    /// Create a new line number (1-indexed)
    #[inline]
    pub const fn new(line: u32) -> Self {
        Self(line)
    }

    /// Get the first line (line 1)
    #[inline]
    pub const fn first() -> Self {
        Self(1)
    }

    /// Get the inner value
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Check if this is a valid line number (> 0)
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }

    /// Convert to 0-indexed offset
    #[inline]
    pub const fn to_zero_indexed(self) -> u32 {
        self.0.saturating_sub(1)
    }

    /// Create from 0-indexed offset
    #[inline]
    pub const fn from_zero_indexed(offset: u32) -> Self {
        Self(offset + 1)
    }

    /// Calculate line count between this and another line (inclusive)
    #[inline]
    pub const fn lines_to(self, end: Self) -> u32 {
        if end.0 >= self.0 {
            end.0 - self.0 + 1
        } else {
            1
        }
    }
}

impl From<u32> for LineNumber {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<LineNumber> for u32 {
    #[inline]
    fn from(value: LineNumber) -> Self {
        value.0
    }
}

impl fmt::Display for LineNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

/// A byte offset in a file or string
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct ByteOffset(usize);

impl ByteOffset {
    /// Create a new byte offset
    #[inline]
    pub const fn new(offset: usize) -> Self {
        Self(offset)
    }

    /// Create a zero offset
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Get the inner value
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ByteOffset {
    #[inline]
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<ByteOffset> for usize {
    #[inline]
    fn from(value: ByteOffset) -> Self {
        value.0
    }
}

impl fmt::Display for ByteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

/// A unique identifier for a symbol in the index
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct SymbolId(u32);

impl SymbolId {
    /// Create a new symbol ID
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Create an invalid/unknown symbol ID
    #[inline]
    pub const fn unknown() -> Self {
        Self(0)
    }

    /// Get the inner value
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Check if this is a valid symbol ID
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

impl From<u32> for SymbolId {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<SymbolId> for u32 {
    #[inline]
    fn from(value: SymbolId) -> Self {
        value.0
    }
}

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// File size in bytes
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct FileSize(u64);

impl FileSize {
    /// Create a new file size
    #[inline]
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Create a zero size
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Get the inner value in bytes
    #[inline]
    pub const fn bytes(self) -> u64 {
        self.0
    }

    /// Get size in kilobytes
    #[inline]
    pub const fn kilobytes(self) -> u64 {
        self.0 / 1024
    }

    /// Get size in megabytes
    #[inline]
    pub const fn megabytes(self) -> u64 {
        self.0 / (1024 * 1024)
    }

    /// Check if this exceeds a limit
    #[inline]
    pub const fn exceeds(self, limit: Self) -> bool {
        self.0 > limit.0
    }
}

impl From<u64> for FileSize {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<FileSize> for u64 {
    #[inline]
    fn from(value: FileSize) -> Self {
        value.0
    }
}

impl fmt::Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 1024 * 1024 {
            write!(f, "{:.1} MB", self.0 as f64 / (1024.0 * 1024.0))
        } else if self.0 >= 1024 {
            write!(f, "{:.1} KB", self.0 as f64 / 1024.0)
        } else {
            write!(f, "{} bytes", self.0)
        }
    }
}

/// Importance score (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ImportanceScore(f32);

impl ImportanceScore {
    /// Create a new importance score, clamping to valid range
    #[inline]
    pub fn new(score: f32) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    /// Create a zero importance score
    #[inline]
    pub const fn zero() -> Self {
        Self(0.0)
    }

    /// Create maximum importance score
    #[inline]
    pub const fn max() -> Self {
        Self(1.0)
    }

    /// Create default importance score
    #[inline]
    pub const fn default_score() -> Self {
        Self(0.5)
    }

    /// Get the inner value
    #[inline]
    pub const fn get(self) -> f32 {
        self.0
    }

    /// Check if this is considered high importance (> 0.7)
    #[inline]
    pub fn is_high(self) -> bool {
        self.0 > 0.7
    }

    /// Check if this is considered low importance (< 0.3)
    #[inline]
    pub fn is_low(self) -> bool {
        self.0 < 0.3
    }
}

impl From<f32> for ImportanceScore {
    #[inline]
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

impl From<ImportanceScore> for f32 {
    #[inline]
    fn from(value: ImportanceScore) -> Self {
        value.0
    }
}

impl fmt::Display for ImportanceScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

impl Eq for ImportanceScore {}

impl PartialOrd for ImportanceScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImportanceScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use f32's partial_cmp directly, treating NaN as equal (shouldn't happen with clamped values)
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl std::hash::Hash for ImportanceScore {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ==========================================================================
    // TokenCount tests
    // ==========================================================================

    #[test]
    fn test_token_count_operations() {
        let a = TokenCount::new(100);
        let b = TokenCount::new(50);

        assert_eq!((a + b).get(), 150);
        assert_eq!((a - b).get(), 50);
        assert_eq!(a.saturating_sub(TokenCount::new(200)).get(), 0);
    }

    #[test]
    fn test_token_count_percentage() {
        let part = TokenCount::new(25);
        let total = TokenCount::new(100);

        assert!((part.percentage_of(total) - 25.0).abs() < 0.01);
        assert_eq!(part.percentage_of(TokenCount::zero()), 0.0);
    }

    #[test]
    fn test_token_count_sum() {
        let counts = vec![TokenCount::new(10), TokenCount::new(20), TokenCount::new(30)];
        let sum: TokenCount = counts.into_iter().sum();
        assert_eq!(sum.get(), 60);
    }

    #[test]
    fn test_token_count_zero() {
        let zero = TokenCount::zero();
        assert_eq!(zero.get(), 0);
        assert!(zero.is_zero());
    }

    #[test]
    fn test_token_count_is_zero() {
        assert!(TokenCount::new(0).is_zero());
        assert!(!TokenCount::new(1).is_zero());
        assert!(!TokenCount::new(100).is_zero());
    }

    #[test]
    fn test_token_count_saturating_add() {
        let a = TokenCount::new(u32::MAX - 10);
        let b = TokenCount::new(20);
        let result = a.saturating_add(b);
        assert_eq!(result.get(), u32::MAX);
    }

    #[test]
    fn test_token_count_add_assign() {
        let mut count = TokenCount::new(100);
        count += TokenCount::new(50);
        assert_eq!(count.get(), 150);
    }

    #[test]
    fn test_token_count_sub_assign() {
        let mut count = TokenCount::new(100);
        count -= TokenCount::new(30);
        assert_eq!(count.get(), 70);
    }

    #[test]
    fn test_token_count_from_u32() {
        let count: TokenCount = 42u32.into();
        assert_eq!(count.get(), 42);
    }

    #[test]
    fn test_token_count_into_u32() {
        let count = TokenCount::new(42);
        let value: u32 = count.into();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_token_count_default() {
        let count = TokenCount::default();
        assert_eq!(count.get(), 0);
        assert!(count.is_zero());
    }

    #[test]
    fn test_token_count_clone_copy() {
        let a = TokenCount::new(100);
        let b = a; // Copy
        let c = a.clone(); // Clone
        assert_eq!(a.get(), b.get());
        assert_eq!(a.get(), c.get());
    }

    #[test]
    fn test_token_count_ord() {
        let a = TokenCount::new(100);
        let b = TokenCount::new(200);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_token_count_hash() {
        let mut set = HashSet::new();
        set.insert(TokenCount::new(100));
        set.insert(TokenCount::new(200));
        set.insert(TokenCount::new(100)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_token_count_debug() {
        let count = TokenCount::new(42);
        let debug_str = format!("{:?}", count);
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_token_count_sum_empty() {
        let counts: Vec<TokenCount> = vec![];
        let sum: TokenCount = counts.into_iter().sum();
        assert_eq!(sum.get(), 0);
    }

    // ==========================================================================
    // LineNumber tests
    // ==========================================================================

    #[test]
    fn test_line_number_indexing() {
        let line = LineNumber::new(10);

        assert_eq!(line.to_zero_indexed(), 9);
        assert_eq!(LineNumber::from_zero_indexed(9).get(), 10);
    }

    #[test]
    fn test_line_number_range() {
        let start = LineNumber::new(5);
        let end = LineNumber::new(10);

        assert_eq!(start.lines_to(end), 6);
        assert_eq!(end.lines_to(start), 1); // Invalid range returns 1
    }

    #[test]
    fn test_line_number_first() {
        let first = LineNumber::first();
        assert_eq!(first.get(), 1);
    }

    #[test]
    fn test_line_number_is_valid() {
        assert!(!LineNumber::new(0).is_valid());
        assert!(LineNumber::new(1).is_valid());
        assert!(LineNumber::new(100).is_valid());
    }

    #[test]
    fn test_line_number_to_zero_indexed_edge() {
        assert_eq!(LineNumber::new(0).to_zero_indexed(), 0); // Saturating
        assert_eq!(LineNumber::new(1).to_zero_indexed(), 0);
    }

    #[test]
    fn test_line_number_from_zero_indexed() {
        assert_eq!(LineNumber::from_zero_indexed(0).get(), 1);
        assert_eq!(LineNumber::from_zero_indexed(99).get(), 100);
    }

    #[test]
    fn test_line_number_from_u32() {
        let line: LineNumber = 42u32.into();
        assert_eq!(line.get(), 42);
    }

    #[test]
    fn test_line_number_into_u32() {
        let line = LineNumber::new(42);
        let value: u32 = line.into();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_line_number_default() {
        let line = LineNumber::default();
        assert_eq!(line.get(), 0);
    }

    #[test]
    fn test_line_number_clone_copy() {
        let a = LineNumber::new(10);
        let b = a;
        let c = a.clone();
        assert_eq!(a.get(), b.get());
        assert_eq!(a.get(), c.get());
    }

    #[test]
    fn test_line_number_ord() {
        let a = LineNumber::new(10);
        let b = LineNumber::new(20);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_line_number_hash() {
        let mut set = HashSet::new();
        set.insert(LineNumber::new(10));
        set.insert(LineNumber::new(20));
        set.insert(LineNumber::new(10)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_line_number_debug() {
        let line = LineNumber::new(42);
        let debug_str = format!("{:?}", line);
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_line_number_lines_to_same() {
        let line = LineNumber::new(5);
        assert_eq!(line.lines_to(line), 1);
    }

    // ==========================================================================
    // ByteOffset tests
    // ==========================================================================

    #[test]
    fn test_byte_offset() {
        let offset = ByteOffset::new(1024);
        assert_eq!(offset.get(), 1024);
        assert_eq!(ByteOffset::zero().get(), 0);
    }

    #[test]
    fn test_byte_offset_from_usize() {
        let offset: ByteOffset = 1024usize.into();
        assert_eq!(offset.get(), 1024);
    }

    #[test]
    fn test_byte_offset_into_usize() {
        let offset = ByteOffset::new(1024);
        let value: usize = offset.into();
        assert_eq!(value, 1024);
    }

    #[test]
    fn test_byte_offset_default() {
        let offset = ByteOffset::default();
        assert_eq!(offset.get(), 0);
    }

    #[test]
    fn test_byte_offset_clone_copy() {
        let a = ByteOffset::new(1024);
        let b = a;
        let c = a.clone();
        assert_eq!(a.get(), b.get());
        assert_eq!(a.get(), c.get());
    }

    #[test]
    fn test_byte_offset_ord() {
        let a = ByteOffset::new(100);
        let b = ByteOffset::new(200);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_byte_offset_hash() {
        let mut set = HashSet::new();
        set.insert(ByteOffset::new(100));
        set.insert(ByteOffset::new(200));
        set.insert(ByteOffset::new(100)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_byte_offset_debug() {
        let offset = ByteOffset::new(1024);
        let debug_str = format!("{:?}", offset);
        assert!(debug_str.contains("1024"));
    }

    #[test]
    fn test_byte_offset_display() {
        let offset = ByteOffset::new(1024);
        assert_eq!(offset.to_string(), "@1024");
    }

    // ==========================================================================
    // SymbolId tests
    // ==========================================================================

    #[test]
    fn test_symbol_id_validity() {
        assert!(!SymbolId::unknown().is_valid());
        assert!(SymbolId::new(1).is_valid());
        assert!(!SymbolId::new(0).is_valid());
    }

    #[test]
    fn test_symbol_id_unknown() {
        let unknown = SymbolId::unknown();
        assert_eq!(unknown.get(), 0);
        assert!(!unknown.is_valid());
    }

    #[test]
    fn test_symbol_id_from_u32() {
        let id: SymbolId = 42u32.into();
        assert_eq!(id.get(), 42);
    }

    #[test]
    fn test_symbol_id_into_u32() {
        let id = SymbolId::new(42);
        let value: u32 = id.into();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_symbol_id_default() {
        let id = SymbolId::default();
        assert_eq!(id.get(), 0);
        assert!(!id.is_valid());
    }

    #[test]
    fn test_symbol_id_clone_copy() {
        let a = SymbolId::new(42);
        let b = a;
        let c = a.clone();
        assert_eq!(a.get(), b.get());
        assert_eq!(a.get(), c.get());
    }

    #[test]
    fn test_symbol_id_ord() {
        let a = SymbolId::new(10);
        let b = SymbolId::new(20);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_symbol_id_hash() {
        let mut set = HashSet::new();
        set.insert(SymbolId::new(10));
        set.insert(SymbolId::new(20));
        set.insert(SymbolId::new(10)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_symbol_id_debug() {
        let id = SymbolId::new(42);
        let debug_str = format!("{:?}", id);
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_symbol_id_display() {
        let id = SymbolId::new(42);
        assert_eq!(id.to_string(), "#42");
    }

    // ==========================================================================
    // FileSize tests
    // ==========================================================================

    #[test]
    fn test_file_size_conversions() {
        let size = FileSize::new(1024 * 1024 + 512 * 1024); // 1.5 MB

        assert_eq!(size.kilobytes(), 1536);
        assert_eq!(size.megabytes(), 1);
    }

    #[test]
    fn test_file_size_display() {
        assert_eq!(FileSize::new(500).to_string(), "500 bytes");
        assert_eq!(FileSize::new(2048).to_string(), "2.0 KB");
        assert_eq!(FileSize::new(1024 * 1024).to_string(), "1.0 MB");
    }

    #[test]
    fn test_file_size_zero() {
        let zero = FileSize::zero();
        assert_eq!(zero.bytes(), 0);
    }

    #[test]
    fn test_file_size_bytes() {
        let size = FileSize::new(12345);
        assert_eq!(size.bytes(), 12345);
    }

    #[test]
    fn test_file_size_exceeds() {
        let size = FileSize::new(1000);
        let limit = FileSize::new(500);
        assert!(size.exceeds(limit));
        assert!(!limit.exceeds(size));
        assert!(!size.exceeds(size)); // Equal doesn't exceed
    }

    #[test]
    fn test_file_size_from_u64() {
        let size: FileSize = 1024u64.into();
        assert_eq!(size.bytes(), 1024);
    }

    #[test]
    fn test_file_size_into_u64() {
        let size = FileSize::new(1024);
        let value: u64 = size.into();
        assert_eq!(value, 1024);
    }

    #[test]
    fn test_file_size_default() {
        let size = FileSize::default();
        assert_eq!(size.bytes(), 0);
    }

    #[test]
    fn test_file_size_clone_copy() {
        let a = FileSize::new(1024);
        let b = a;
        let c = a.clone();
        assert_eq!(a.bytes(), b.bytes());
        assert_eq!(a.bytes(), c.bytes());
    }

    #[test]
    fn test_file_size_ord() {
        let a = FileSize::new(100);
        let b = FileSize::new(200);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_file_size_hash() {
        let mut set = HashSet::new();
        set.insert(FileSize::new(100));
        set.insert(FileSize::new(200));
        set.insert(FileSize::new(100)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_file_size_debug() {
        let size = FileSize::new(1024);
        let debug_str = format!("{:?}", size);
        assert!(debug_str.contains("1024"));
    }

    #[test]
    fn test_file_size_display_edge_cases() {
        // Just under 1KB
        assert_eq!(FileSize::new(1023).to_string(), "1023 bytes");
        // Exactly 1KB
        assert_eq!(FileSize::new(1024).to_string(), "1.0 KB");
        // Just under 1MB
        let just_under_mb = 1024 * 1024 - 1;
        assert!(FileSize::new(just_under_mb).to_string().contains("KB"));
    }

    // ==========================================================================
    // ImportanceScore tests
    // ==========================================================================

    #[test]
    fn test_importance_score_clamping() {
        assert_eq!(ImportanceScore::new(-0.5).get(), 0.0);
        assert_eq!(ImportanceScore::new(1.5).get(), 1.0);
        assert_eq!(ImportanceScore::new(0.5).get(), 0.5);
    }

    #[test]
    fn test_importance_score_classification() {
        assert!(ImportanceScore::new(0.8).is_high());
        assert!(!ImportanceScore::new(0.5).is_high());
        assert!(ImportanceScore::new(0.2).is_low());
        assert!(!ImportanceScore::new(0.5).is_low());
    }

    #[test]
    fn test_importance_score_zero() {
        let zero = ImportanceScore::zero();
        assert_eq!(zero.get(), 0.0);
        assert!(zero.is_low());
    }

    #[test]
    fn test_importance_score_max() {
        let max = ImportanceScore::max();
        assert_eq!(max.get(), 1.0);
        assert!(max.is_high());
    }

    #[test]
    fn test_importance_score_default_score() {
        let default = ImportanceScore::default_score();
        assert_eq!(default.get(), 0.5);
        assert!(!default.is_high());
        assert!(!default.is_low());
    }

    #[test]
    fn test_importance_score_from_f32() {
        let score: ImportanceScore = 0.75f32.into();
        assert_eq!(score.get(), 0.75);
    }

    #[test]
    fn test_importance_score_from_f32_clamped() {
        let score: ImportanceScore = 2.0f32.into();
        assert_eq!(score.get(), 1.0);
    }

    #[test]
    fn test_importance_score_into_f32() {
        let score = ImportanceScore::new(0.75);
        let value: f32 = score.into();
        assert_eq!(value, 0.75);
    }

    #[test]
    fn test_importance_score_default() {
        let score = ImportanceScore::default();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn test_importance_score_clone_copy() {
        let a = ImportanceScore::new(0.5);
        let b = a;
        let c = a.clone();
        assert_eq!(a.get(), b.get());
        assert_eq!(a.get(), c.get());
    }

    #[test]
    fn test_importance_score_ord() {
        let a = ImportanceScore::new(0.3);
        let b = ImportanceScore::new(0.7);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_importance_score_partial_ord() {
        let a = ImportanceScore::new(0.3);
        let b = ImportanceScore::new(0.7);
        assert!(a.partial_cmp(&b) == Some(std::cmp::Ordering::Less));
        assert!(b.partial_cmp(&a) == Some(std::cmp::Ordering::Greater));
    }

    #[test]
    fn test_importance_score_eq() {
        let a = ImportanceScore::new(0.5);
        let b = ImportanceScore::new(0.5);
        let c = ImportanceScore::new(0.6);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_importance_score_hash() {
        let mut set = HashSet::new();
        set.insert(ImportanceScore::new(0.3));
        set.insert(ImportanceScore::new(0.7));
        set.insert(ImportanceScore::new(0.3)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_importance_score_debug() {
        let score = ImportanceScore::new(0.5);
        let debug_str = format!("{:?}", score);
        assert!(debug_str.contains("0.5"));
    }

    #[test]
    fn test_importance_score_display() {
        let score = ImportanceScore::new(0.5);
        assert_eq!(score.to_string(), "0.50");
    }

    #[test]
    fn test_importance_score_boundary_high() {
        // Exactly at boundary
        assert!(!ImportanceScore::new(0.7).is_high()); // Not strictly > 0.7
        assert!(ImportanceScore::new(0.71).is_high());
    }

    #[test]
    fn test_importance_score_boundary_low() {
        // Exactly at boundary
        assert!(!ImportanceScore::new(0.3).is_low()); // Not strictly < 0.3
        assert!(ImportanceScore::new(0.29).is_low());
    }

    // ==========================================================================
    // Display tests
    // ==========================================================================

    #[test]
    fn test_display_formatting() {
        assert_eq!(TokenCount::new(100).to_string(), "100 tokens");
        assert_eq!(LineNumber::new(42).to_string(), "L42");
        assert_eq!(ByteOffset::new(1000).to_string(), "@1000");
        assert_eq!(SymbolId::new(5).to_string(), "#5");
    }

    #[test]
    fn test_importance_score_display_precision() {
        assert_eq!(ImportanceScore::new(0.123).to_string(), "0.12");
        assert_eq!(ImportanceScore::new(0.999).to_string(), "1.00"); // Clamped
        assert_eq!(ImportanceScore::zero().to_string(), "0.00");
    }

    // ==========================================================================
    // Serde tests
    // ==========================================================================

    #[test]
    fn test_token_count_serde() {
        let count = TokenCount::new(100);
        let json = serde_json::to_string(&count).unwrap();
        let parsed: TokenCount = serde_json::from_str(&json).unwrap();
        assert_eq!(count, parsed);
    }

    #[test]
    fn test_line_number_serde() {
        let line = LineNumber::new(42);
        let json = serde_json::to_string(&line).unwrap();
        let parsed: LineNumber = serde_json::from_str(&json).unwrap();
        assert_eq!(line, parsed);
    }

    #[test]
    fn test_byte_offset_serde() {
        let offset = ByteOffset::new(1024);
        let json = serde_json::to_string(&offset).unwrap();
        let parsed: ByteOffset = serde_json::from_str(&json).unwrap();
        assert_eq!(offset, parsed);
    }

    #[test]
    fn test_symbol_id_serde() {
        let id = SymbolId::new(42);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: SymbolId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_file_size_serde() {
        let size = FileSize::new(1024);
        let json = serde_json::to_string(&size).unwrap();
        let parsed: FileSize = serde_json::from_str(&json).unwrap();
        assert_eq!(size, parsed);
    }

    #[test]
    fn test_importance_score_serde() {
        let score = ImportanceScore::new(0.75);
        let json = serde_json::to_string(&score).unwrap();
        let parsed: ImportanceScore = serde_json::from_str(&json).unwrap();
        assert_eq!(score, parsed);
    }
}
