//! End-to-End Tests for Infiniloom CLI
//!
//! These tests verify user expectations from a black-box perspective.
//! Each test answers: "As a user, I expect X to happen when I do Y."
//!
//! Run with: cargo test --test e2e

// Declare submodules from the e2e/ directory
mod e2e {
    pub mod chunk_tests;
    pub mod config_tests;
    pub mod error_tests;
    pub mod format_tests;
    pub mod helpers;
    pub mod index_diff_tests;
    pub mod map_tests;
    pub mod pack_tests;
    pub mod security_tests;
    pub mod watch_tests;
}

// Re-export for cleaner test organization
pub use e2e::*;
