//! Neptune-compatible graph export (vertices + edges JSONL)
//!
//! Generates graph data from embedding chunks suitable for bulk-loading into
//! Amazon Neptune or other property graph databases. The output format uses
//! Neptune's `~id`, `~from`, `~to`, `~label` conventions.
//!
//! # Vertex Types
//!
//! - **Symbol vertices**: One per chunk (function, method, class, etc.)
//! - **File vertices**: One per unique source file
//! - **Module vertices**: One per unique module path (if available)
//!
//! # Edge Types
//!
//! - `CALLS`: Symbol calls another symbol (from resolved `chunk.context.qualified_calls`)
//! - `DEFINED_IN`: Symbol is defined in a file
//! - `BELONGS_TO`: File belongs to a module
//! - `CONTAINS`: Parent symbol contains child symbol

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use super::hasher::hash_content;
use super::types::EmbedChunk;

/// A Neptune-compatible graph vertex
#[derive(Debug, Clone, Serialize)]
pub struct GraphVertex {
    /// Vertex ID (Neptune convention)
    #[serde(rename = "~id")]
    pub id: String,

    /// Vertex label (Neptune convention)
    #[serde(rename = "~label")]
    pub label: String,

    /// Additional properties flattened into the JSON object
    #[serde(flatten)]
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// A Neptune-compatible graph edge
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    /// Edge ID (Neptune convention) - deterministic hash of from+to+label
    #[serde(rename = "~id")]
    pub id: String,

    /// Source vertex ID
    #[serde(rename = "~from")]
    pub from: String,

    /// Target vertex ID
    #[serde(rename = "~to")]
    pub to: String,

    /// Edge label (relationship type)
    #[serde(rename = "~label")]
    pub label: String,
}

/// Complete graph export containing vertices and edges
#[derive(Debug, Clone)]
pub struct GraphExport {
    /// All vertices (symbol, file, module)
    pub vertices: Vec<GraphVertex>,

    /// All edges (CALLS, DEFINED_IN, BELONGS_TO, CONTAINS)
    pub edges: Vec<GraphEdge>,
}

/// Generate a deterministic edge ID from its components
fn edge_id(from: &str, to: &str, label: &str) -> String {
    let input = format!("{from}\0{to}\0{label}");
    let result = hash_content(&input);
    // Use "e_" prefix instead of "ec_" to distinguish edge IDs from chunk IDs
    format!("e_{}", &result.short_id[3..])
}

fn logical_target_ids<'a>(candidates: &'a [&'a EmbedChunk]) -> Vec<&'a str> {
    let mut candidates = candidates.to_vec();
    candidates.sort_by(|left, right| {
        left.source
            .file
            .cmp(&right.source.file)
            .then_with(|| left.source.lines.0.cmp(&right.source.lines.0))
            .then_with(|| left.source.lines.1.cmp(&right.source.lines.1))
            .then_with(|| left.source.symbol.cmp(&right.source.symbol))
            .then_with(|| left.id.cmp(&right.id))
    });

    if candidates.iter().any(|chunk| chunk.kind.is_part()) {
        candidates
            .iter()
            .filter(|chunk| chunk.kind.is_part())
            .filter(|chunk| chunk.part.as_ref().is_none_or(|part| part.part == 1))
            .map(|chunk| chunk.id.as_str())
            .collect()
    } else {
        candidates
            .first()
            .map(|chunk| vec![chunk.id.as_str()])
            .unwrap_or_default()
    }
}

fn contains_parent_ids<'a>(
    child: &'a EmbedChunk,
    candidates: &'a [&'a EmbedChunk],
) -> Vec<&'a str> {
    if let Some(parent_chunk_id) = child.source.parent_chunk_id.as_deref() {
        if candidates.iter().any(|chunk| chunk.id == parent_chunk_id) {
            return vec![parent_chunk_id];
        }
    }

    if candidates.iter().any(|chunk| chunk.kind.is_part()) {
        return candidates
            .iter()
            .filter(|chunk| chunk.kind.is_part())
            .filter(|chunk| line_ranges_overlap(chunk.source.lines, child.source.lines))
            .map(|chunk| chunk.id.as_str())
            .collect();
    }

    candidates
        .first()
        .map(|chunk| vec![chunk.id.as_str()])
        .unwrap_or_default()
}

fn line_ranges_overlap(a: (u32, u32), b: (u32, u32)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

fn same_logical_fqn(left: &EmbedChunk, right: &EmbedChunk) -> bool {
    left.source
        .fqn
        .as_deref()
        .zip(right.source.fqn.as_deref())
        .is_some_and(|(left_fqn, right_fqn)| left_fqn == right_fqn)
}

/// Generate a Neptune-compatible graph export from embedding chunks.
///
/// This performs best-effort symbol resolution: calls that cannot be matched
/// to a known chunk are silently skipped.
pub fn generate_graph_export(chunks: &[EmbedChunk]) -> GraphExport {
    // Build lookups for hierarchy and resolved call targets.
    let mut chunk_by_id: HashMap<&str, &EmbedChunk> = HashMap::new();
    let mut symbol_by_name_and_file: HashMap<(&str, &str), Vec<&EmbedChunk>> = HashMap::new();
    let mut symbol_by_fqn: HashMap<&str, Vec<&EmbedChunk>> = HashMap::new();

    for chunk in chunks {
        chunk_by_id.insert(chunk.id.as_str(), chunk);
        let name = chunk.source.symbol.as_str();
        let file = chunk.source.file.as_str();
        symbol_by_name_and_file
            .entry((name, file))
            .or_default()
            .push(chunk);
        if let Some(fqn) = chunk.source.fqn.as_deref() {
            symbol_by_fqn.entry(fqn).or_default().push(chunk);
        }
    }

    // Track unique files and modules (sorted for determinism)
    let mut files: BTreeSet<&str> = BTreeSet::new();
    let mut modules: BTreeSet<String> = BTreeSet::new();

    for chunk in chunks {
        files.insert(&chunk.source.file);

        // Derive module path from file path (e.g., "src/auth/mod.rs" -> "src/auth")
        if let Some(module_path) = derive_module_path(&chunk.source.file) {
            modules.insert(module_path);
        }
    }

    let mut vertices = Vec::new();
    let mut edges = Vec::new();

    // --- Symbol vertices (one per chunk) ---
    for chunk in chunks {
        let mut props = serde_json::Map::new();
        props.insert("name".to_owned(), serde_json::Value::String(chunk.source.symbol.clone()));
        props.insert("file".to_owned(), serde_json::Value::String(chunk.source.file.clone()));
        props.insert(
            "language".to_owned(),
            serde_json::Value::String(chunk.source.language.clone()),
        );
        props.insert(
            "visibility".to_owned(),
            serde_json::Value::String(chunk.source.visibility.name().to_owned()),
        );
        if let Some(ref sig) = chunk.context.signature {
            props.insert("signature".to_owned(), serde_json::Value::String(sig.clone()));
        }
        props.insert(
            "start_line".to_owned(),
            serde_json::Value::Number(chunk.source.lines.0.into()),
        );
        props.insert("end_line".to_owned(), serde_json::Value::Number(chunk.source.lines.1.into()));
        props.insert("tokens".to_owned(), serde_json::Value::Number(chunk.tokens.into()));

        vertices.push(GraphVertex {
            id: chunk.id.clone(),
            label: chunk.kind.name().to_owned(),
            properties: props,
        });
    }

    // --- File vertices ---
    for file in &files {
        let mut props = serde_json::Map::new();
        props.insert("path".to_owned(), serde_json::Value::String((*file).to_owned()));

        // Detect language from first chunk with this file
        if let Some(chunk) = chunks.iter().find(|c| c.source.file == *file) {
            props.insert(
                "language".to_owned(),
                serde_json::Value::String(chunk.source.language.clone()),
            );
        }

        vertices.push(GraphVertex {
            id: format!("file:{file}"),
            label: "file".to_owned(),
            properties: props,
        });
    }

    // --- Module vertices ---
    for module_path in &modules {
        let mut props = serde_json::Map::new();
        props.insert("module_path".to_owned(), serde_json::Value::String(module_path.clone()));

        vertices.push(GraphVertex {
            id: format!("mod:{module_path}"),
            label: "module".to_owned(),
            properties: props,
        });
    }

    // --- DEFINED_IN edges (chunk -> file) ---
    for chunk in chunks {
        let file_id = format!("file:{}", chunk.source.file);
        edges.push(GraphEdge {
            id: edge_id(&chunk.id, &file_id, "DEFINED_IN"),
            from: chunk.id.clone(),
            to: file_id,
            label: "DEFINED_IN".to_owned(),
        });
    }

    // --- CALLS edges (chunk -> called chunk) ---
    for chunk in chunks {
        for qcall in &chunk.context.qualified_calls {
            let target_ids = symbol_by_fqn
                .get(qcall.as_str())
                .map(|candidates| logical_target_ids(candidates))
                .unwrap_or_default();

            for target in target_ids {
                // Skip self-calls
                let is_self_id = target == chunk.id.as_str();
                let is_same_logical_fqn = chunk_by_id
                    .get(target)
                    .is_some_and(|target_chunk| same_logical_fqn(chunk, target_chunk));
                if !is_self_id && !is_same_logical_fqn {
                    edges.push(GraphEdge {
                        id: edge_id(&chunk.id, target, "CALLS"),
                        from: chunk.id.clone(),
                        to: target.to_owned(),
                        label: "CALLS".to_owned(),
                    });
                }
            }
        }
    }

    // --- BELONGS_TO edges (file -> module) ---
    for file in &files {
        if let Some(module_path) = derive_module_path(file) {
            if modules.contains(&module_path) {
                let file_id = format!("file:{file}");
                let mod_id = format!("mod:{module_path}");
                edges.push(GraphEdge {
                    id: edge_id(&file_id, &mod_id, "BELONGS_TO"),
                    from: file_id,
                    to: mod_id,
                    label: "BELONGS_TO".to_owned(),
                });
            }
        }
    }

    // --- CONTAINS edges (parent chunk -> child chunk) ---
    for chunk in chunks {
        if let Some(ref parent_name) = chunk.source.parent {
            // Find the parent chunk by name in the same file
            let parent_ids = symbol_by_name_and_file
                .get(&(parent_name.as_str(), chunk.source.file.as_str()))
                .map(|candidates| contains_parent_ids(chunk, candidates))
                .unwrap_or_default();

            for pid in parent_ids {
                if pid != chunk.id.as_str() {
                    edges.push(GraphEdge {
                        id: edge_id(pid, &chunk.id, "CONTAINS"),
                        from: pid.to_owned(),
                        to: chunk.id.clone(),
                        label: "CONTAINS".to_owned(),
                    });
                }
            }
        }
    }

    // Sort for determinism
    vertices.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    GraphExport { vertices, edges }
}

/// Derive a module path from a file path.
///
/// Examples:
/// - "src/auth/mod.rs" -> Some("src/auth")
/// - "src/auth/token.rs" -> Some("src/auth")
/// - "src/lib.rs" -> Some("src")
/// - "main.rs" -> None
fn derive_module_path(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let parent = path.parent()?;
    let parent_str = parent.to_str()?;
    if parent_str.is_empty() {
        None
    } else {
        // Normalize path separators to forward slash for cross-platform consistency
        Some(parent_str.replace('\\', "/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::types::{
        ChunkContext, ChunkKind, ChunkSource, EmbedChunk, RepoIdentifier, Visibility,
    };

    fn make_chunk(
        id: &str,
        symbol: &str,
        file: &str,
        kind: ChunkKind,
        calls: Vec<String>,
        parent: Option<String>,
    ) -> EmbedChunk {
        EmbedChunk {
            id: id.to_owned(),
            full_hash: format!("{id}_full"),
            content: format!("fn {symbol}() {{}}"),
            tokens: 10,
            kind,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: file.to_owned(),
                lines: (1, 5),
                symbol: symbol.to_owned(),
                fqn: None,
                language: "Rust".to_owned(),
                parent,
                visibility: Visibility::Public,
                is_test: false,
                module_path: None,
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext { calls, ..Default::default() },
            children_ids: Vec::new(),
            repr: "code".to_string(),
            code_chunk_id: None,
            part: None,
        }
    }

    #[test]
    fn test_generate_graph_basic() {
        let mut chunks = vec![
            make_chunk(
                "ec_aaa",
                "foo",
                "src/lib.rs",
                ChunkKind::Function,
                vec!["bar".into()],
                None,
            ),
            make_chunk("ec_bbb", "bar", "src/lib.rs", ChunkKind::Function, vec![], None),
        ];
        chunks[0].context.qualified_calls = vec!["src::lib::bar".to_owned()];
        chunks[1].source.fqn = Some("src::lib::bar".to_owned());

        let graph = generate_graph_export(&chunks);

        // 2 symbol vertices + 1 file vertex + 1 module vertex
        assert!(graph.vertices.len() >= 3);

        // Should have DEFINED_IN edges for both chunks + CALLS edge foo->bar + BELONGS_TO
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, "ec_aaa");
        assert_eq!(calls[0].to, "ec_bbb");

        let defined_in: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.label == "DEFINED_IN")
            .collect();
        assert_eq!(defined_in.len(), 2);
    }

    #[test]
    fn test_contains_edge() {
        let chunks = vec![
            make_chunk("ec_cls", "MyClass", "src/model.rs", ChunkKind::Class, vec![], None),
            make_chunk(
                "ec_mth",
                "my_method",
                "src/model.rs",
                ChunkKind::Method,
                vec![],
                Some("MyClass".into()),
            ),
        ];

        let graph = generate_graph_export(&chunks);

        let contains: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.label == "CONTAINS")
            .collect();
        assert_eq!(contains.len(), 1);
        assert_eq!(contains[0].from, "ec_cls");
        assert_eq!(contains[0].to, "ec_mth");
    }

    #[test]
    fn test_calls_to_split_symbol_target_entry_part_only() {
        fn split_part(id: &str, part_num: u32) -> EmbedChunk {
            let mut chunk =
                make_chunk(id, "big", "src/lib.rs", ChunkKind::FunctionPart, vec![], None);
            chunk.part = Some(crate::embedding::types::ChunkPart {
                part: part_num,
                of: 2,
                parent_id: "parent_big".to_owned(),
                parent_signature: "fn big()".to_owned(),
                overlap_lines: 0,
            });
            chunk
        }

        let mut chunks = vec![
            make_chunk(
                "ec_caller",
                "caller",
                "src/lib.rs",
                ChunkKind::Function,
                vec!["big".into()],
                None,
            ),
            split_part("ec_big_part_1", 1),
            split_part("ec_big_part_2", 2),
        ];
        chunks[0].context.qualified_calls = vec!["src::lib::big".to_owned()];
        chunks[1].source.fqn = Some("src::lib::big".to_owned());
        chunks[2].source.fqn = Some("src::lib::big".to_owned());

        let graph = generate_graph_export(&chunks);
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, "ec_caller");
        assert_eq!(calls[0].to, "ec_big_part_1");
    }

    #[test]
    fn test_calls_skip_same_fqn_split_fragments() {
        fn split_part(id: &str, part_num: u32) -> EmbedChunk {
            let mut chunk =
                make_chunk(id, "big", "src/lib.rs", ChunkKind::FunctionPart, vec![], None);
            chunk.part = Some(crate::embedding::types::ChunkPart {
                part: part_num,
                of: 2,
                parent_id: "parent_big".to_owned(),
                parent_signature: "fn big()".to_owned(),
                overlap_lines: 0,
            });
            chunk.source.fqn = Some("src::lib::big".to_owned());
            chunk
        }

        let mut chunks = vec![split_part("ec_big_part_1", 1), split_part("ec_big_part_2", 2)];
        chunks[0].context.qualified_calls = vec!["src::lib::big".to_owned()];

        let graph = generate_graph_export(&chunks);
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();

        assert!(
            calls.is_empty(),
            "split fragments of the same logical symbol should not call each other: {calls:#?}"
        );
    }

    #[test]
    fn test_contains_to_split_class_targets_fragment_parent_only() {
        fn class_part(id: &str, part_num: u32, lines: (u32, u32)) -> EmbedChunk {
            let mut chunk =
                make_chunk(id, "BigService", "src/model.rs", ChunkKind::ClassPart, vec![], None);
            chunk.source.lines = lines;
            chunk.part = Some(crate::embedding::types::ChunkPart {
                part: part_num,
                of: 2,
                parent_id: "parent_big_service".to_owned(),
                parent_signature: "class BigService".to_owned(),
                overlap_lines: 0,
            });
            chunk
        }

        let mut child = make_chunk(
            "ec_child",
            "worker",
            "src/model.rs",
            ChunkKind::Method,
            vec![],
            Some("BigService".into()),
        );
        child.source.lines = (75, 80);
        child.source.parent_chunk_id = Some("ec_class_part_2".to_owned());

        let chunks = vec![
            class_part("ec_class_part_1", 1, (1, 50)),
            class_part("ec_class_part_2", 2, (51, 100)),
            child,
        ];

        let graph = generate_graph_export(&chunks);
        let contains: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.label == "CONTAINS")
            .collect();

        assert_eq!(contains.len(), 1);
        assert_eq!(contains[0].from, "ec_class_part_2");
        assert_eq!(contains[0].to, "ec_child");
    }

    #[test]
    fn test_raw_unmatched_calls_do_not_create_edges() {
        let chunks = vec![make_chunk(
            "ec_aaa",
            "foo",
            "src/lib.rs",
            ChunkKind::Function,
            vec!["nonexistent".into()],
            None,
        )];

        let graph = generate_graph_export(&chunks);

        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();
        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn test_raw_calls_do_not_fallback_to_same_name() {
        let chunks = vec![
            make_chunk(
                "ec_caller",
                "caller",
                "src/main.rs",
                ChunkKind::Function,
                vec!["target".into()],
                None,
            ),
            make_chunk("ec_a", "target", "src/a.rs", ChunkKind::Function, vec![], None),
            make_chunk("ec_b", "target", "src/b.rs", ChunkKind::Function, vec![], None),
        ];

        let graph = generate_graph_export(&chunks);
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();

        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn test_qualified_call_targets_exact_fqn_only() {
        let mut chunks = vec![
            make_chunk(
                "ec_caller",
                "caller",
                "src/main.rs",
                ChunkKind::Function,
                vec!["target".into()],
                None,
            ),
            make_chunk("ec_a", "target", "src/a.rs", ChunkKind::Function, vec![], None),
            make_chunk("ec_b", "target", "src/b.rs", ChunkKind::Function, vec![], None),
        ];
        chunks[0].context.qualified_calls = vec!["src::a::target".to_owned()];
        chunks[1].source.fqn = Some("src::a::target".to_owned());
        chunks[2].source.fqn = Some("src::b::target".to_owned());

        let graph = generate_graph_export(&chunks);
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, "ec_caller");
        assert_eq!(calls[0].to, "ec_a");
    }

    #[test]
    fn test_duplicate_fqn_call_targets_first_source_order_chunk() {
        let mut chunks = vec![
            make_chunk(
                "ec_caller",
                "caller",
                "src/main.py",
                ChunkKind::Function,
                vec!["create".into()],
                None,
            ),
            make_chunk("ec_late", "create", "src/factory.py", ChunkKind::Function, vec![], None),
            make_chunk("ec_early", "create", "src/factory.py", ChunkKind::Function, vec![], None),
        ];
        chunks[0].context.qualified_calls = vec!["src::factory::create".to_owned()];
        chunks[1].source.fqn = Some("src::factory::create".to_owned());
        chunks[1].source.lines = (30, 30);
        chunks[2].source.fqn = Some("src::factory::create".to_owned());
        chunks[2].source.lines = (10, 10);

        let graph = generate_graph_export(&chunks);
        let calls: Vec<_> = graph.edges.iter().filter(|e| e.label == "CALLS").collect();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, "ec_caller");
        assert_eq!(calls[0].to, "ec_early");
    }

    #[test]
    fn test_edge_id_deterministic() {
        let id1 = edge_id("a", "b", "CALLS");
        let id2 = edge_id("a", "b", "CALLS");
        assert_eq!(id1, id2);

        let id3 = edge_id("a", "b", "DEFINED_IN");
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_derive_module_path() {
        assert_eq!(derive_module_path("src/auth/mod.rs"), Some("src/auth".into()));
        assert_eq!(derive_module_path("src/auth/token.rs"), Some("src/auth".into()));
        assert_eq!(derive_module_path("src/lib.rs"), Some("src".into()));
        assert_eq!(derive_module_path("main.rs"), None);
    }

    #[test]
    fn test_output_sorted_deterministically() {
        let chunks = vec![
            make_chunk("ec_zzz", "zeta", "src/z.rs", ChunkKind::Function, vec![], None),
            make_chunk("ec_aaa", "alpha", "src/a.rs", ChunkKind::Function, vec![], None),
        ];

        let graph = generate_graph_export(&chunks);

        // Vertices should be sorted by ID
        let ids: Vec<_> = graph.vertices.iter().map(|v| v.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }
}
