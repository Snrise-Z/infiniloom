//! Document chunking for multi-turn LLM conversations.
//!
//! Splits large documents into token-budgeted chunks that respect
//! section boundaries and never split tables or lists.

use serde::{Deserialize, Serialize};

use crate::document::types::*;
use crate::tokenizer::{TokenModel, Tokenizer};

/// A chunk of a document with metadata for multi-turn use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// Chunk index (0-based)
    pub index: usize,
    /// Total number of chunks
    pub total: usize,
    /// Section path breadcrumb (e.g., "Chapter 3 > Section 3.2")
    pub breadcrumb: String,
    /// The chunk content as sections
    pub sections: Vec<Section>,
    /// Approximate token count (claude model)
    pub token_count: usize,
}

/// Configuration for document chunking.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum tokens per chunk (default: 4000)
    pub max_tokens: usize,
    /// Overlap tokens between chunks for context continuity (default: 200)
    pub overlap_tokens: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self { max_tokens: 4000, overlap_tokens: 200 }
    }
}

/// A flattened section candidate for chunking, with its breadcrumb path
/// and estimated token count.
struct SectionCandidate {
    section: Section,
    breadcrumb: String,
    tokens: usize,
}

/// Estimate the token count for a single section (not including children).
fn estimate_section_tokens(section: &Section, tokenizer: &Tokenizer) -> usize {
    let mut text = String::new();
    if let Some(title) = &section.title {
        text.push_str(title);
        text.push('\n');
    }
    for block in &section.content {
        text.push_str(&block.text());
        text.push('\n');
    }
    tokenizer.count(&text, TokenModel::Claude) as usize
}

/// Flatten a section tree into a list of candidates, each with its breadcrumb path.
/// Children are recursively flattened, but each section is emitted without its children
/// (the children become their own candidates).
fn flatten_sections(sections: &[Section], parent_breadcrumb: &str) -> Vec<SectionCandidate> {
    let tokenizer = Tokenizer::new();
    let mut candidates = Vec::new();

    for section in sections {
        let breadcrumb = if parent_breadcrumb.is_empty() {
            section.title.clone().unwrap_or_default()
        } else if let Some(title) = &section.title {
            format!("{} > {}", parent_breadcrumb, title)
        } else {
            parent_breadcrumb.to_string()
        };

        // Create a candidate for this section without its children
        let leaf_section = Section {
            id: section.id.clone(),
            level: section.level,
            title: section.title.clone(),
            number: section.number.clone(),
            content: section.content.clone(),
            children: Vec::new(),
            importance: section.importance,
        };

        let tokens = estimate_section_tokens(&leaf_section, &tokenizer);
        candidates.push(SectionCandidate {
            section: leaf_section,
            breadcrumb: breadcrumb.clone(),
            tokens,
        });

        // Recursively flatten children
        if !section.children.is_empty() {
            candidates.extend(flatten_sections(&section.children, &breadcrumb));
        }
    }

    candidates
}

/// Split a document into token-budgeted chunks.
///
/// The algorithm:
/// 1. Flatten the document's section tree into candidates
/// 2. Greedily pack candidates into chunks without exceeding `max_tokens`
/// 3. Tables and lists are never split across chunks
/// 4. Breaks prefer section boundaries (headings)
/// 5. For overlap, the last section title from the previous chunk is included as context
pub fn chunk_document(doc: &Document, config: &ChunkConfig) -> Vec<DocumentChunk> {
    if doc.sections.is_empty() {
        return Vec::new();
    }

    let candidates = flatten_sections(&doc.sections, "");
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<(Vec<Section>, String, usize)> = Vec::new(); // (sections, breadcrumb, tokens)
    let mut current_sections: Vec<Section> = Vec::new();
    let mut current_tokens: usize = 0;
    let mut current_breadcrumb = String::new();
    let mut last_chunk_last_title: Option<String> = None;

    for candidate in candidates {
        let candidate_tokens = candidate.tokens;

        // If adding this candidate would exceed the budget and we have content,
        // finalize the current chunk
        if !current_sections.is_empty() && current_tokens + candidate_tokens > config.max_tokens {
            // Save the last section title for overlap context
            let last_title = current_sections.last().and_then(|s| s.title.clone());

            chunks.push((
                std::mem::take(&mut current_sections),
                std::mem::take(&mut current_breadcrumb),
                current_tokens,
            ));
            current_tokens = 0;
            last_chunk_last_title = last_title;
        }

        // If this is the first section in a new chunk and we have overlap context,
        // add a breadcrumb hint from the previous chunk
        if current_sections.is_empty() {
            if let Some(ref prev_title) = last_chunk_last_title {
                // Include the previous section title as a context marker
                if !prev_title.is_empty() {
                    let overlap_section = Section {
                        id: None,
                        level: 0,
                        title: Some(format!("[continued from: {}]", prev_title)),
                        number: None,
                        content: Vec::new(),
                        children: Vec::new(),
                        importance: 0.0,
                    };
                    let tokenizer = Tokenizer::new();
                    let overlap_tokens = estimate_section_tokens(&overlap_section, &tokenizer);
                    if overlap_tokens <= config.overlap_tokens {
                        current_sections.push(overlap_section);
                        current_tokens += overlap_tokens;
                    }
                }
            }
        }

        // Update breadcrumb to the first real section in this chunk
        if current_breadcrumb.is_empty() && !candidate.breadcrumb.is_empty() {
            current_breadcrumb = candidate.breadcrumb.clone();
        }

        // Add the candidate (even if it alone exceeds the budget - it gets its own chunk)
        current_sections.push(candidate.section);
        current_tokens += candidate_tokens;
    }

    // Don't forget the last chunk
    if !current_sections.is_empty() {
        chunks.push((current_sections, current_breadcrumb, current_tokens));
    }

    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, (sections, breadcrumb, token_count))| DocumentChunk {
            index: i,
            total,
            breadcrumb,
            sections,
            token_count,
        })
        .collect()
}

/// Format chunks as a JSON array.
pub fn format_chunks_json(chunks: &[DocumentChunk]) -> String {
    serde_json::to_string_pretty(chunks).unwrap_or_default()
}

/// Format a single chunk as readable text with header.
pub fn format_chunk_text(chunk: &DocumentChunk) -> String {
    let mut out = String::new();
    out.push_str(&format!("--- Chunk {}/{} ---\n", chunk.index + 1, chunk.total));
    if !chunk.breadcrumb.is_empty() {
        out.push_str(&format!("Context: {}\n", chunk.breadcrumb));
    }
    out.push_str(&format!("Tokens: ~{}\n\n", chunk.token_count));
    for section in &chunk.sections {
        if let Some(title) = &section.title {
            let level = section.level.min(6).max(1) as usize;
            let prefix = "#".repeat(level);
            out.push_str(&format!("{prefix} {title}\n\n"));
        }
        for block in &section.content {
            out.push_str(&block.text());
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_section(level: u8, title: &str, text: &str) -> Section {
        let mut s = Section::new(level, title);
        if !text.is_empty() {
            s.content.push(ContentBlock::Paragraph(text.to_string()));
        }
        s
    }

    fn make_table_section(title: &str, rows: usize) -> Section {
        let mut s = Section::new(1, title);
        let table = Table {
            caption: Some("Test Table".to_string()),
            headers: vec!["Col A".to_string(), "Col B".to_string()],
            rows: (0..rows)
                .map(|i| vec![format!("Row {i} A data value"), format!("Row {i} B data value")])
                .collect(),
            alignments: vec![],
        };
        s.content.push(ContentBlock::Table(table));
        s
    }

    #[test]
    fn test_small_document_single_chunk() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        doc.sections.push(make_section(1, "Intro", "Hello world."));
        doc.sections.push(make_section(1, "Conclusion", "Goodbye."));

        let config = ChunkConfig { max_tokens: 4000, overlap_tokens: 200 };
        let chunks = chunk_document(&doc, &config);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].total, 1);
        assert!(!chunks[0].sections.is_empty());
    }

    #[test]
    fn test_large_document_multiple_chunks() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        // Create many sections with enough text to exceed a small token budget
        for i in 0..20 {
            let text = format!(
                "This is section {i} with enough text to contribute meaningful tokens. \
                 We need to ensure that the total document exceeds our small budget. \
                 Adding more words helps ensure proper splitting across chunks."
            );
            doc.sections
                .push(make_section(1, &format!("Section {i}"), &text));
        }

        let config = ChunkConfig {
            max_tokens: 100, // Very small budget to force splitting
            overlap_tokens: 0,
        };
        let chunks = chunk_document(&doc, &config);

        assert!(chunks.len() > 1, "Should split into multiple chunks");
        // All chunks should have correct total
        for chunk in &chunks {
            assert_eq!(chunk.total, chunks.len());
        }
        // Indices should be sequential
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn test_tables_not_split() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        // A section with a big table
        doc.sections.push(make_table_section("Big Table", 50));

        let config = ChunkConfig {
            max_tokens: 50, // Very small, but table should still be in one chunk
            overlap_tokens: 0,
        };
        let chunks = chunk_document(&doc, &config);

        // The table section should be entirely in one chunk
        let table_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| {
                c.sections.iter().any(|s| {
                    s.content
                        .iter()
                        .any(|b| matches!(b, ContentBlock::Table(_)))
                })
            })
            .collect();

        assert_eq!(table_chunks.len(), 1, "Table should be entirely within a single chunk");
    }

    #[test]
    fn test_breadcrumb_generation() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut parent = Section::new(1, "Chapter 1");
        parent
            .content
            .push(ContentBlock::Paragraph("Chapter intro text.".to_string()));
        let child = make_section(2, "Section 1.1", "Child content.");
        parent.children.push(child);
        doc.sections.push(parent);

        let config = ChunkConfig { max_tokens: 4000, overlap_tokens: 200 };
        let chunks = chunk_document(&doc, &config);

        // The breadcrumb should contain the top-level section title
        assert!(!chunks.is_empty());
        assert!(
            chunks[0].breadcrumb.contains("Chapter 1"),
            "Breadcrumb should contain parent title, got: {}",
            chunks[0].breadcrumb
        );
    }

    #[test]
    fn test_chunk_index_total_numbering() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        for i in 0..10 {
            let text = "A".repeat(500); // enough text to force many chunks with small budget
            doc.sections.push(make_section(1, &format!("S{i}"), &text));
        }

        let config = ChunkConfig { max_tokens: 50, overlap_tokens: 0 };
        let chunks = chunk_document(&doc, &config);

        let total = chunks.len();
        assert!(total > 1);
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
            assert_eq!(chunk.total, total);
        }
    }

    #[test]
    fn test_empty_document_returns_empty() {
        let doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let config = ChunkConfig::default();
        let chunks = chunk_document(&doc, &config);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_config_defaults() {
        let config = ChunkConfig::default();
        assert_eq!(config.max_tokens, 4000);
        assert_eq!(config.overlap_tokens, 200);
    }

    #[test]
    fn test_format_chunk_text() {
        let chunk = DocumentChunk {
            index: 0,
            total: 2,
            breadcrumb: "Chapter 1 > Section 1.1".to_string(),
            sections: vec![make_section(1, "Section 1.1", "Some content here.")],
            token_count: 42,
        };

        let text = format_chunk_text(&chunk);
        assert!(text.contains("--- Chunk 1/2 ---"));
        assert!(text.contains("Context: Chapter 1 > Section 1.1"));
        assert!(text.contains("Tokens: ~42"));
        assert!(text.contains("# Section 1.1"));
        assert!(text.contains("Some content here."));
    }

    #[test]
    fn test_format_chunks_json() {
        let chunks = vec![DocumentChunk {
            index: 0,
            total: 1,
            breadcrumb: "Intro".to_string(),
            sections: vec![make_section(1, "Intro", "Hello.")],
            token_count: 10,
        }];

        let json = format_chunks_json(&chunks);
        assert!(json.contains("\"index\": 0"));
        assert!(json.contains("\"total\": 1"));
        assert!(json.contains("\"breadcrumb\": \"Intro\""));
    }
}
