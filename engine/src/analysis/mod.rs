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

pub mod types;
pub mod type_signature;
pub mod type_hierarchy;
pub mod documentation;
pub mod complexity;
pub mod dead_code;
pub mod breaking_changes;
pub mod multi_repo;

// Re-export main types
pub use types::*;
pub use type_signature::*;
pub use type_hierarchy::*;
pub use documentation::*;
pub use complexity::*;
pub use dead_code::*;
pub use breaking_changes::*;
pub use multi_repo::*;
