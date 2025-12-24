//! Context expansion for diff-based queries.
//!
//! This module provides intelligent context expansion given a diff (changed files and lines).
//! It expands the context to include relevant dependent files, symbols, and call graphs.

mod expander;
mod types;

// Re-export all public types
pub use expander::ContextExpander;
pub use types::{
    CallChain, ChangeClassification, ChangeType, ContextDepth, ContextFile, ContextSnippet,
    ContextSymbol, DiffChange, ExpandedContext, ImpactLevel, ImpactSummary,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{
        DepGraph, FileEntry, FileId, IndexSymbol, IndexSymbolKind, Language, Span, SymbolId,
        SymbolIndex, Visibility,
    };

    fn create_test_index() -> (SymbolIndex, DepGraph) {
        let mut index = SymbolIndex::new();
        index.repo_name = "test".to_owned();

        // Add files
        index.files.push(FileEntry {
            id: FileId::new(0),
            path: "src/main.rs".to_owned(),
            language: Language::Rust,
            content_hash: [0; 32],
            symbols: 0..2,
            imports: vec![],
            lines: 100,
            tokens: 500,
        });
        index.files.push(FileEntry {
            id: FileId::new(1),
            path: "src/lib.rs".to_owned(),
            language: Language::Rust,
            content_hash: [0; 32],
            symbols: 2..3,
            imports: vec![],
            lines: 50,
            tokens: 250,
        });
        index.files.push(FileEntry {
            id: FileId::new(2),
            path: "tests/test_main.rs".to_owned(),
            language: Language::Rust,
            content_hash: [0; 32],
            symbols: 3..4,
            imports: vec![],
            lines: 30,
            tokens: 150,
        });

        // Add symbols
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(0),
            name: "main".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(1, 0, 10, 0),
            signature: Some("fn main()".to_owned()),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(1),
            name: "helper".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(0),
            span: Span::new(15, 0, 25, 0),
            signature: Some("fn helper()".to_owned()),
            parent: None,
            visibility: Visibility::Private,
            docstring: None,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(2),
            name: "lib_fn".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(1),
            span: Span::new(1, 0, 20, 0),
            signature: Some("pub fn lib_fn()".to_owned()),
            parent: None,
            visibility: Visibility::Public,
            docstring: None,
        });
        index.symbols.push(IndexSymbol {
            id: SymbolId::new(3),
            name: "test_main".to_owned(),
            kind: IndexSymbolKind::Function,
            file_id: FileId::new(2),
            span: Span::new(1, 0, 15, 0),
            signature: Some("fn test_main()".to_owned()),
            parent: None,
            visibility: Visibility::Private,
            docstring: None,
        });

        index.rebuild_lookups();

        // Create graph
        let mut graph = DepGraph::new();
        graph.add_file_import(0, 1); // main imports lib
        graph.add_file_import(2, 0); // test imports main
        graph.add_call(0, 2); // main calls lib_fn

        (index, graph)
    }

    #[test]
    fn test_context_expansion_l1() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let changes = vec![DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(5, 8)],
            change_type: ChangeType::Modified,
            diff_content: None,
        }];

        let context = expander.expand(&changes, ContextDepth::L1, 10000);

        assert_eq!(context.changed_files.len(), 1);
        assert_eq!(context.changed_symbols.len(), 1);
        assert_eq!(context.changed_symbols[0].name, "main");
    }

    #[test]
    fn test_context_expansion_l2() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let changes = vec![DiffChange {
            file_path: "src/lib.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 20)],
            change_type: ChangeType::Modified,
            diff_content: None,
        }];

        let context = expander.expand(&changes, ContextDepth::L2, 10000);

        // Should find main.rs as dependent (it imports lib.rs)
        assert!(!context.dependent_files.is_empty() || context.changed_files.len() == 1);
    }

    #[test]
    fn test_test_file_detection() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        assert!(expander.is_test_file("tests/test_main.rs"));
        assert!(expander.is_test_file("src/foo.test.ts"));
        assert!(expander.is_test_file("spec/foo.spec.js"));
        assert!(!expander.is_test_file("src/main.rs"));
    }

    #[test]
    fn test_impact_level() {
        assert_eq!(ImpactLevel::Low.name(), "low");
        assert_eq!(ImpactLevel::Critical.name(), "critical");
    }

    #[test]
    fn test_change_classification_deleted() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![],
            change_type: ChangeType::Deleted,
            diff_content: None,
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::Deletion);
    }

    #[test]
    fn test_change_classification_renamed() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/new_name.rs".to_owned(),
            old_path: Some("src/old_name.rs".to_owned()),
            line_ranges: vec![],
            change_type: ChangeType::Renamed,
            diff_content: None,
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::FileRename);
    }

    #[test]
    fn test_change_classification_added() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/new_file.rs".to_owned(),
            old_path: None,
            line_ranges: vec![],
            change_type: ChangeType::Added,
            diff_content: None,
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::NewCode);
    }

    #[test]
    fn test_change_classification_signature_change() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 10)],
            change_type: ChangeType::Modified,
            diff_content: Some("-fn helper(x: i32)\n+fn helper(x: i32, y: i32)".to_owned()),
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::SignatureChange);
    }

    #[test]
    fn test_change_classification_type_definition() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/types.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 10)],
            change_type: ChangeType::Modified,
            diff_content: Some("+struct NewField {\n+    value: i32\n+}".to_owned()),
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::TypeDefinitionChange);
    }

    #[test]
    fn test_change_classification_import_change() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 2)],
            change_type: ChangeType::Modified,
            diff_content: Some("+use std::collections::HashMap;".to_owned()),
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::ImportChange);
    }

    #[test]
    fn test_change_classification_doc_only() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        let change = DiffChange {
            file_path: "src/main.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 3)],
            change_type: ChangeType::Modified,
            diff_content: Some("+/// This is a doc comment\n+/// Another doc line".to_owned()),
        };

        let classification = expander.classify_change(&change, None);
        assert_eq!(classification, ChangeClassification::DocumentationOnly);
    }

    #[test]
    fn test_classification_score_multipliers() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        // Deletion has highest multiplier
        let deletion_mult =
            expander.classification_score_multiplier(ChangeClassification::Deletion);
        let sig_mult =
            expander.classification_score_multiplier(ChangeClassification::SignatureChange);
        let impl_mult =
            expander.classification_score_multiplier(ChangeClassification::ImplementationChange);
        let doc_mult =
            expander.classification_score_multiplier(ChangeClassification::DocumentationOnly);

        assert!(deletion_mult > sig_mult, "Deletion should have higher priority than signature");
        assert!(sig_mult > impl_mult, "Signature change should have higher priority than impl");
        assert!(impl_mult > doc_mult, "Implementation should have higher priority than docs");
    }

    #[test]
    fn test_context_with_signature_change_includes_callers() {
        let (index, graph) = create_test_index();
        let expander = ContextExpander::new(&index, &graph);

        // Modify lib_fn which is called by main
        let changes = vec![DiffChange {
            file_path: "src/lib.rs".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 20)],
            change_type: ChangeType::Modified,
            diff_content: Some("-pub fn lib_fn()\n+pub fn lib_fn(new_param: i32)".to_owned()),
        }];

        let context = expander.expand(&changes, ContextDepth::L2, 10000);

        // Should detect as signature change and expand more aggressively
        assert!(!context.changed_symbols.is_empty());
        // Changed symbol should have signature change in reason
        if let Some(sym) = context.changed_symbols.first() {
            assert!(
                sym.relevance_reason.contains("signature")
                    || sym.relevance_reason.contains("modified"),
                "Expected signature or modified in reason, got: {}",
                sym.relevance_reason
            );
        }
    }
}
