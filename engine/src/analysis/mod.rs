//! Code analysis module for advanced features
//!
//! This module provides:
//! - Type signature extraction with parameters, return types, generics
//! - Type hierarchy navigation (extends/implements chains)
//! - Documentation extraction (JSDoc/docstring parsing)
//! - Complexity metrics (cyclomatic, cognitive complexity)
//! - Dead code detection
//! - Breaking change detection
//! - Multi-repository indexing

pub mod breaking_changes;
pub mod complexity;
pub mod dead_code;
pub mod documentation;
pub mod multi_repo;
pub mod type_hierarchy;
pub mod type_signature;
pub mod types;

// Re-export main types
pub use breaking_changes::*;
pub use complexity::*;
pub use dead_code::*;
pub use documentation::*;
pub use multi_repo::*;
pub use type_hierarchy::*;
pub use type_signature::*;
pub use types::*;
