//! Hierarchical chunking for improved RAG recall
//!
//! This module provides hierarchical chunking that creates summary chunks for
//! container types (classes, structs, modules) with references to their children.
//! This enables RAG systems to retrieve both high-level overviews and specific
//! implementation details.
//!
//! # Hierarchy Levels
//!
//! 1. **Container Summary**: Class/struct with docstring, signature, and child list
//! 2. **Child Chunks**: Individual methods, fields, nested types
//!
//! # Example Output
//!
//! For a class `UserService`:
//! - Summary chunk: Contains class docstring, signature, and list of method names
//! - Method chunks: Individual `get_user()`, `create_user()`, etc.
//!
//! # Usage
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::hierarchy::HierarchyBuilder;
//!
//! let builder = HierarchyBuilder::new();
//! let hierarchical_chunks = builder.build_hierarchy(&chunks);
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::hasher::hash_content;
use super::types::{ChunkContext, ChunkKind, ChunkSource, EmbedChunk};

/// Configuration for hierarchical chunking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyConfig {
    /// Generate summary chunks for classes
    pub summarize_classes: bool,

    /// Generate summary chunks for structs
    pub summarize_structs: bool,

    /// Generate summary chunks for modules
    pub summarize_modules: bool,

    /// Minimum number of children to generate a summary
    pub min_children_for_summary: usize,

    /// Include child signatures in summary
    pub include_child_signatures: bool,

    /// Maximum number of children to list in summary
    pub max_children_in_summary: usize,
}

impl Default for HierarchyConfig {
    fn default() -> Self {
        Self {
            summarize_classes: true,
            summarize_structs: true,
            summarize_modules: false, // Often too broad
            min_children_for_summary: 2,
            include_child_signatures: true,
            max_children_in_summary: 20,
        }
    }
}

/// Reference to a child chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildReference {
    /// Child chunk ID
    pub id: String,

    /// Child symbol name
    pub name: String,

    /// Child kind (method, field, etc.)
    pub kind: ChunkKind,

    /// Child signature (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Brief description from docstring (first line)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
}

/// Summary chunk for a container type (class, struct, module)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchySummary {
    /// The container chunk this summarizes
    pub container_id: String,

    /// Container symbol name
    pub container_name: String,

    /// Container kind
    pub container_kind: ChunkKind,

    /// List of child references
    pub children: Vec<ChildReference>,

    /// Total number of children (may exceed max_children_in_summary)
    pub total_children: usize,
}

/// Builder for hierarchical chunks
pub struct HierarchyBuilder {
    config: HierarchyConfig,
}

impl Default for HierarchyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchyBuilder {
    /// Create a new hierarchy builder with default config
    pub fn new() -> Self {
        Self { config: HierarchyConfig::default() }
    }

    /// Create with custom configuration
    pub fn with_config(config: HierarchyConfig) -> Self {
        Self { config }
    }

    /// Build hierarchy from chunks, returning summary chunks
    ///
    /// This identifies parent-child relationships and generates summary chunks
    /// for containers that meet the threshold.
    pub fn build_hierarchy(&self, chunks: &[EmbedChunk]) -> Vec<EmbedChunk> {
        // Group children by parent
        let mut parent_children: HashMap<String, Vec<&EmbedChunk>> = HashMap::new();

        // Find all chunks that have a parent
        for chunk in chunks {
            if let Some(ref parent) = chunk.source.parent {
                let key = format!("{}:{}", chunk.source.file, parent);
                parent_children.entry(key).or_default().push(chunk);
            }
        }

        // Find container chunks and create summaries
        let mut summaries = Vec::new();

        for chunk in chunks {
            if !self.should_summarize(&chunk.kind) {
                continue;
            }

            let key = format!("{}:{}", chunk.source.file, chunk.source.symbol);
            let children = parent_children.get(&key);

            if let Some(children) = children {
                if children.len() >= self.config.min_children_for_summary {
                    if let Some(summary) = self.create_summary_chunk(chunk, children) {
                        summaries.push(summary);
                    }
                }
            }
        }

        summaries
    }

    /// Check if a chunk kind should have a summary
    fn should_summarize(&self, kind: &ChunkKind) -> bool {
        match kind {
            ChunkKind::Class => self.config.summarize_classes,
            ChunkKind::Struct => self.config.summarize_structs,
            ChunkKind::Module => self.config.summarize_modules,
            ChunkKind::Interface | ChunkKind::Trait => self.config.summarize_classes,
            _ => false,
        }
    }

    /// Create a summary chunk for a container
    fn create_summary_chunk(
        &self,
        container: &EmbedChunk,
        children: &[&EmbedChunk],
    ) -> Option<EmbedChunk> {
        // Build child references
        let mut child_refs: Vec<ChildReference> = children
            .iter()
            .take(self.config.max_children_in_summary)
            .map(|child| ChildReference {
                id: child.id.clone(),
                name: child.source.symbol.clone(),
                kind: child.kind,
                signature: if self.config.include_child_signatures {
                    child.context.signature.clone()
                } else {
                    None
                },
                brief: child.context.docstring.as_ref().and_then(|d| {
                    d.lines().next().map(|s| {
                        let s = s.trim();
                        if s.len() > 100 {
                            format!("{}...", &s[..97])
                        } else {
                            s.to_owned()
                        }
                    })
                }),
            })
            .collect();

        // Sort by name for determinism
        child_refs.sort_by(|a, b| a.name.cmp(&b.name));

        // Build summary content
        let summary_content = self.build_summary_content(container, &child_refs, children.len());

        // Hash the summary content
        let hash = hash_content(&summary_content);

        // Create tags for the summary
        let mut tags = vec!["summary".to_owned(), "hierarchy".to_owned()];
        tags.extend(container.context.tags.iter().cloned());

        Some(EmbedChunk {
            id: hash.short_id,
            full_hash: hash.full_hash,
            content: summary_content,
            tokens: 0, // Will be computed by caller if needed
            kind: container.kind,
            source: ChunkSource {
                repo: container.source.repo.clone(),
                file: container.source.file.clone(),
                lines: container.source.lines,
                symbol: format!("{}_summary", container.source.symbol),
                fqn: container
                    .source
                    .fqn
                    .as_ref()
                    .map(|f| format!("{}_summary", f)),
                language: container.source.language.clone(),
                parent: container.source.parent.clone(),
                visibility: container.source.visibility,
                is_test: container.source.is_test,
            },
            context: ChunkContext {
                docstring: container.context.docstring.clone(),
                comments: Vec::new(),
                signature: container.context.signature.clone(),
                calls: Vec::new(), // Summary doesn't have direct calls
                called_by: Vec::new(),
                imports: container.context.imports.clone(),
                tags,
                lines_of_code: 0,
                max_nesting_depth: 0,
            },
            part: None,
        })
    }

    /// Build the summary content string
    fn build_summary_content(
        &self,
        container: &EmbedChunk,
        child_refs: &[ChildReference],
        total_children: usize,
    ) -> String {
        let mut content = String::new();

        // Add container signature if available
        if let Some(ref sig) = container.context.signature {
            content.push_str(sig);
            content.push('\n');
        }

        // Add docstring if available
        if let Some(ref doc) = container.context.docstring {
            content.push('\n');
            content.push_str(doc);
            content.push('\n');
        }

        // Add child list
        content.push_str("\n/* Members:\n");

        for child in child_refs {
            content.push_str(" * - ");
            content.push_str(&child.name);

            if let Some(ref sig) = child.signature {
                // Compact signature (remove body, keep just declaration)
                let sig_line = sig.lines().next().unwrap_or(sig).trim();
                if sig_line != child.name {
                    content.push_str(": ");
                    content.push_str(sig_line);
                }
            }

            if let Some(ref brief) = child.brief {
                content.push_str(" - ");
                content.push_str(brief);
            }

            content.push('\n');
        }

        if total_children > child_refs.len() {
            content.push_str(&format!(" * ... and {} more\n", total_children - child_refs.len()));
        }

        content.push_str(" */\n");

        content
    }

    /// Enrich existing chunks with hierarchy metadata
    ///
    /// This adds `hierarchy` information to chunk context tags
    pub fn enrich_chunks(&self, chunks: &mut [EmbedChunk]) {
        // Build parent -> children map
        let mut parent_children: HashMap<String, Vec<String>> = HashMap::new();

        for chunk in chunks.iter() {
            if let Some(ref parent) = chunk.source.parent {
                let key = format!("{}:{}", chunk.source.file, parent);
                parent_children
                    .entry(key)
                    .or_default()
                    .push(chunk.source.symbol.clone());
            }
        }

        // Enrich container chunks with child count
        for chunk in chunks.iter_mut() {
            let key = format!("{}:{}", chunk.source.file, chunk.source.symbol);
            if let Some(children) = parent_children.get(&key) {
                chunk
                    .context
                    .tags
                    .push(format!("has-children:{}", children.len()));
            }

            // Mark chunks that have a parent
            if chunk.source.parent.is_some() {
                chunk.context.tags.push("has-parent".to_owned());
            }
        }
    }
}

/// Get the hierarchy summary for a specific container
pub fn get_hierarchy_summary(
    chunks: &[EmbedChunk],
    container_symbol: &str,
    file: &str,
) -> Option<HierarchySummary> {
    // Find the container
    let container = chunks
        .iter()
        .find(|c| c.source.symbol == container_symbol && c.source.file == file)?;

    // Find children
    let children: Vec<ChildReference> = chunks
        .iter()
        .filter(|c| c.source.parent.as_deref() == Some(container_symbol) && c.source.file == file)
        .map(|c| ChildReference {
            id: c.id.clone(),
            name: c.source.symbol.clone(),
            kind: c.kind,
            signature: c.context.signature.clone(),
            brief: c
                .context
                .docstring
                .as_ref()
                .and_then(|d| d.lines().next().map(|s| s.trim().to_owned())),
        })
        .collect();

    Some(HierarchySummary {
        container_id: container.id.clone(),
        container_name: container.source.symbol.clone(),
        container_kind: container.kind,
        total_children: children.len(),
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::types::{RepoIdentifier, Visibility};

    fn create_test_chunk(
        id: &str,
        symbol: &str,
        kind: ChunkKind,
        parent: Option<&str>,
        signature: Option<&str>,
        docstring: Option<&str>,
    ) -> EmbedChunk {
        EmbedChunk {
            id: id.to_owned(),
            full_hash: format!("{}_full", id),
            content: format!("content of {}", symbol),
            tokens: 100,
            kind,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "test.rs".to_owned(),
                lines: (1, 10),
                symbol: symbol.to_owned(),
                fqn: Some(format!("test::{}", symbol)),
                language: "Rust".to_owned(),
                parent: parent.map(String::from),
                visibility: Visibility::Public,
                is_test: false,
            },
            context: ChunkContext {
                docstring: docstring.map(String::from),
                comments: Vec::new(),
                signature: signature.map(String::from),
                calls: Vec::new(),
                called_by: Vec::new(),
                imports: Vec::new(),
                tags: Vec::new(),
                lines_of_code: 10,
                max_nesting_depth: 2,
            },
            part: None,
        }
    }

    #[test]
    fn test_build_hierarchy_basic() {
        let chunks = vec![
            create_test_chunk(
                "c1",
                "UserService",
                ChunkKind::Class,
                None,
                Some("class UserService"),
                Some("Service for user management"),
            ),
            create_test_chunk(
                "c2",
                "get_user",
                ChunkKind::Method,
                Some("UserService"),
                Some("fn get_user(&self, id: u64) -> User"),
                Some("Get a user by ID"),
            ),
            create_test_chunk(
                "c3",
                "create_user",
                ChunkKind::Method,
                Some("UserService"),
                Some("fn create_user(&self, data: UserData) -> User"),
                Some("Create a new user"),
            ),
            create_test_chunk(
                "c4",
                "delete_user",
                ChunkKind::Method,
                Some("UserService"),
                Some("fn delete_user(&self, id: u64)"),
                Some("Delete a user"),
            ),
        ];

        let builder = HierarchyBuilder::new();
        let summaries = builder.build_hierarchy(&chunks);

        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].source.symbol.contains("summary"));
        assert!(summaries[0].content.contains("Members:"));
        assert!(summaries[0].content.contains("get_user"));
        assert!(summaries[0].content.contains("create_user"));
        assert!(summaries[0].content.contains("delete_user"));
    }

    #[test]
    fn test_hierarchy_min_children() {
        let chunks = vec![
            create_test_chunk(
                "c1",
                "SmallClass",
                ChunkKind::Class,
                None,
                Some("class SmallClass"),
                None,
            ),
            create_test_chunk(
                "c2",
                "only_method",
                ChunkKind::Method,
                Some("SmallClass"),
                None,
                None,
            ),
        ];

        let builder = HierarchyBuilder::with_config(HierarchyConfig {
            min_children_for_summary: 2, // Requires at least 2 children
            ..Default::default()
        });

        let summaries = builder.build_hierarchy(&chunks);
        assert!(summaries.is_empty()); // Only 1 child, no summary
    }

    #[test]
    fn test_hierarchy_enrich_chunks() {
        let mut chunks = vec![
            create_test_chunk("c1", "MyClass", ChunkKind::Class, None, None, None),
            create_test_chunk("c2", "method1", ChunkKind::Method, Some("MyClass"), None, None),
            create_test_chunk("c3", "method2", ChunkKind::Method, Some("MyClass"), None, None),
        ];

        let builder = HierarchyBuilder::new();
        builder.enrich_chunks(&mut chunks);

        // Container should have child count tag
        assert!(chunks[0]
            .context
            .tags
            .iter()
            .any(|t| t.starts_with("has-children:")));

        // Children should have parent tag
        assert!(chunks[1].context.tags.contains(&"has-parent".to_owned()));
        assert!(chunks[2].context.tags.contains(&"has-parent".to_owned()));
    }

    #[test]
    fn test_get_hierarchy_summary() {
        let chunks = vec![
            create_test_chunk(
                "c1",
                "MyStruct",
                ChunkKind::Struct,
                None,
                Some("struct MyStruct"),
                None,
            ),
            create_test_chunk("c2", "field1", ChunkKind::Variable, Some("MyStruct"), None, None),
            create_test_chunk(
                "c3",
                "new",
                ChunkKind::Function,
                Some("MyStruct"),
                Some("fn new() -> Self"),
                Some("Create a new instance"),
            ),
        ];

        let summary = get_hierarchy_summary(&chunks, "MyStruct", "test.rs");
        assert!(summary.is_some());

        let summary = summary.unwrap();
        assert_eq!(summary.container_name, "MyStruct");
        assert_eq!(summary.total_children, 2);
        assert!(summary.children.iter().any(|c| c.name == "field1"));
        assert!(summary.children.iter().any(|c| c.name == "new"));
    }

    #[test]
    fn test_summary_content_format() {
        let chunks = vec![
            create_test_chunk(
                "c1",
                "Calculator",
                ChunkKind::Class,
                None,
                Some("pub struct Calculator"),
                Some("A simple calculator"),
            ),
            create_test_chunk(
                "c2",
                "add",
                ChunkKind::Method,
                Some("Calculator"),
                Some("fn add(&self, a: i32, b: i32) -> i32"),
                Some("Add two numbers"),
            ),
            create_test_chunk(
                "c3",
                "subtract",
                ChunkKind::Method,
                Some("Calculator"),
                Some("fn subtract(&self, a: i32, b: i32) -> i32"),
                Some("Subtract two numbers"),
            ),
        ];

        let builder = HierarchyBuilder::new();
        let summaries = builder.build_hierarchy(&chunks);

        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];

        // Check content structure
        assert!(summary.content.contains("pub struct Calculator"));
        assert!(summary.content.contains("A simple calculator"));
        assert!(summary.content.contains("/* Members:"));
        assert!(summary.content.contains(" * - add"));
        assert!(summary.content.contains(" * - subtract"));
        assert!(summary.content.contains(" */"));
    }

    #[test]
    fn test_config_options() {
        let config = HierarchyConfig {
            summarize_classes: true,
            summarize_structs: false,
            summarize_modules: false,
            min_children_for_summary: 1,
            include_child_signatures: false,
            max_children_in_summary: 5,
        };

        let builder = HierarchyBuilder::with_config(config);

        let class_chunks = vec![
            create_test_chunk("c1", "MyClass", ChunkKind::Class, None, None, None),
            create_test_chunk("c2", "m1", ChunkKind::Method, Some("MyClass"), None, None),
        ];

        let struct_chunks = vec![
            create_test_chunk("s1", "MyStruct", ChunkKind::Struct, None, None, None),
            create_test_chunk("s2", "f1", ChunkKind::Variable, Some("MyStruct"), None, None),
        ];

        // Class should get summary
        assert_eq!(builder.build_hierarchy(&class_chunks).len(), 1);

        // Struct should NOT get summary (disabled)
        assert_eq!(builder.build_hierarchy(&struct_chunks).len(), 0);
    }
}
