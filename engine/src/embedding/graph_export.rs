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
//! - `CALLS`: Symbol calls another symbol (from `chunk.context.calls`)
//! - `DEFINED_IN`: Symbol is defined in a file
//! - `BELONGS_TO`: File belongs to a module
//! - `CONTAINS`: Parent symbol contains child symbol

use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    if candidates.iter().any(|chunk| chunk.kind.is_part()) {
        candidates
            .iter()
            .filter(|chunk| chunk.kind.is_part())
            .map(|chunk| chunk.id.as_str())
            .collect()
    } else {
        candidates
            .first()
            .map(|chunk| vec![chunk.id.as_str()])
            .unwrap_or_default()
    }
}

/// Generate a Neptune-compatible graph export from embedding chunks.
///
/// This performs best-effort symbol resolution: calls that cannot be matched
/// to a known chunk are silently skipped.
pub fn generate_graph_export(chunks: &[EmbedChunk]) -> GraphExport {
    // Build lookup: symbol_name -> list of chunk IDs (for call resolution)
    // We use (symbol_name, file) as key for precise matching, falling back to name-only
    let mut symbol_by_name_and_file: HashMap<(&str, &str), Vec<&EmbedChunk>> = HashMap::new();
    let mut symbol_by_name: HashMap<&str, Vec<&EmbedChunk>> = HashMap::new();

    for chunk in chunks {
        let name = chunk.source.symbol.as_str();
        let file = chunk.source.file.as_str();
        symbol_by_name_and_file
            .entry((name, file))
            .or_default()
            .push(chunk);
        // For name-only lookup, first match wins (deterministic due to sorted input)
        symbol_by_name.entry(name).or_default().push(chunk);
    }

    // Track unique files and modules (sorted for determinism)
    let mut files: BTreeSet<&str> = BTreeSet::new();
    let mut modules: BTreeSet<String> = BTreeSet::new();

    // Track parent->children relationships by matching source.parent
    let mut parent_map: HashMap<(&str, &str), Vec<&str>> = HashMap::new();

    for chunk in chunks {
        files.insert(&chunk.source.file);

        // Derive module path from file path (e.g., "src/auth/mod.rs" -> "src/auth")
        if let Some(module_path) = derive_module_path(&chunk.source.file) {
            modules.insert(module_path);
        }

        // Track parent->child relationships
        if let Some(ref parent_name) = chunk.source.parent {
            parent_map
                .entry((parent_name.as_str(), chunk.source.file.as_str()))
                .or_default()
                .push(&chunk.id);
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
    // Collect into a BTreeMap for deterministic iteration
    let module_files: BTreeMap<&str, Vec<&str>> = {
        let mut mf: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for file in &files {
            if let Some(module_path) = derive_module_path(file) {
                if modules.contains(&module_path) {
                    mf.entry(modules.get(&module_path).map_or("", |s| s.as_str()))
                        .or_default()
                        .push(file);
                }
            }
        }
        mf
    };

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
        for call_name in &chunk.context.calls {
            // Try precise match (same file first), then name-only
            let target_ids = symbol_by_name_and_file
                .get(&(call_name.as_str(), chunk.source.file.as_str()))
                .or_else(|| symbol_by_name.get(call_name.as_str()))
                .map(|candidates| logical_target_ids(candidates))
                .unwrap_or_default();

            for target in target_ids {
                // Skip self-calls
                if target != chunk.id.as_str() {
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
                .map(|candidates| logical_target_ids(candidates))
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
        let chunks = vec![
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
    fn test_unresolved_calls_skipped() {
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
