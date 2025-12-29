//! Integration tests for centralized filtering module (Phase 3 Item 11)
//!
//! Run with: cargo test --test filtering_tests

mod integration {
    pub mod filtering_integration_tests;
}

// Re-export for easy access
pub use integration::*;
