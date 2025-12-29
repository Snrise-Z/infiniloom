//! Unit Tests for Phase 2 Refactored Modules
//!
//! These tests verify that splitting large files into focused modules
//! (Items 7, 8, 9 from Phase 2) did not break functionality.
//!
//! Run with: cargo test --test unit_tests

// Declare submodules from the unit/ directory
mod unit {
    pub mod diff_module_tests;
    pub mod pack_module_tests;
}

// Re-export for cleaner test organization
pub use unit::*;
