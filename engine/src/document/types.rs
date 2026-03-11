//! Core type definitions for document ingestion.
//!
//! Documents have a fundamentally different structure than code:
//! - Code: File → Symbols (functions, classes)
//! - Documents: Document → Sections → ContentBlocks (paragraphs, tables, lists)

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::tokenizer::TokenCounts;

/// A parsed document ready for LLM-optimized output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Document title (extracted or inferred)
    pub title: Option<String>,
    /// Source file path
    pub source: PathBuf,
    /// Original format
    pub format: DocumentFormat,
    /// Document metadata
    pub metadata: DocumentMetadata,
    /// Hierarchical content structure
    pub sections: Vec<Section>,
    /// Token counts across models
    pub token_count: TokenCounts,
}

impl Document {
    /// Create a new empty document from a source path.
    pub fn new(source: impl Into<PathBuf>, format: DocumentFormat) -> Self {
        Self {
            title: None,
            source: source.into(),
            format,
            metadata: DocumentMetadata::default(),
            sections: Vec::new(),
            token_count: TokenCounts::default(),
        }
    }

    /// Total number of sections (including nested).
    pub fn section_count(&self) -> usize {
        fn count(sections: &[Section]) -> usize {
            sections.iter().map(|s| 1 + count(&s.children)).sum()
        }
        count(&self.sections)
    }

    /// Total number of content blocks across all sections.
    pub fn block_count(&self) -> usize {
        fn count(sections: &[Section]) -> usize {
            sections
                .iter()
                .map(|s| s.content.len() + count(&s.children))
                .sum()
        }
        count(&self.sections)
    }

    /// Flatten all text content into a single string.
    pub fn full_text(&self) -> String {
        let mut buf = String::new();
        fn collect(sections: &[Section], buf: &mut String) {
            for s in sections {
                if let Some(title) = &s.title {
                    buf.push_str(title);
                    buf.push('\n');
                }
                for block in &s.content {
                    buf.push_str(&block.text());
                    buf.push('\n');
                }
                collect(&s.children, buf);
            }
        }
        collect(&self.sections, &mut buf);
        buf
    }
}

/// Supported document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentFormat {
    Docx,
    Html,
    Markdown,
    PlainText,
    Csv,
    Xlsx,
    Pdf,
}

impl DocumentFormat {
    /// Detect format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "docx" => Some(Self::Docx),
            "html" | "htm" | "xhtml" => Some(Self::Html),
            "md" | "markdown" | "mdx" => Some(Self::Markdown),
            "txt" | "text" | "log" | "rst" => Some(Self::PlainText),
            "csv" | "tsv" => Some(Self::Csv),
            "xlsx" | "xls" => Some(Self::Xlsx),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Human-readable format name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Docx => "DOCX",
            Self::Html => "HTML",
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain Text",
            Self::Csv => "CSV",
            Self::Xlsx => "XLSX",
            Self::Pdf => "PDF",
        }
    }
}

/// Document metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    /// Document version/revision (compliance)
    pub version: Option<String>,
    /// Effective date (compliance)
    pub effective_date: Option<String>,
    /// Document classification (e.g., "Internal", "Confidential")
    pub classification: Option<String>,
    /// Total pages (if applicable)
    pub pages: Option<u32>,
    /// Custom key-value metadata
    pub custom: BTreeMap<String, String>,
}

/// A document section with optional heading and nested children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section ID for cross-referencing
    pub id: Option<String>,
    /// Heading level (1-6, 0 for no heading)
    pub level: u8,
    /// Section title/heading text
    pub title: Option<String>,
    /// Section number (e.g., "3.2.1")
    pub number: Option<String>,
    /// Content blocks within this section
    pub content: Vec<ContentBlock>,
    /// Nested subsections
    pub children: Vec<Section>,
    /// Information density score (set by distillation pipeline)
    pub importance: f32,
}

impl Section {
    /// Create a new section with a heading.
    pub fn new(level: u8, title: impl Into<String>) -> Self {
        Self {
            id: None,
            level,
            title: Some(title.into()),
            number: None,
            content: Vec::new(),
            children: Vec::new(),
            importance: 0.5,
        }
    }

    /// Create a root section (no heading) to hold top-level content.
    pub fn root() -> Self {
        Self {
            id: None,
            level: 0,
            title: None,
            number: None,
            content: Vec::new(),
            children: Vec::new(),
            importance: 0.5,
        }
    }
}

/// A block of content within a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    /// A paragraph of text
    Paragraph(String),
    /// A table with optional caption
    Table(Table),
    /// An ordered or unordered list
    List(List),
    /// A code block or preformatted text
    CodeBlock(CodeBlock),
    /// A definition (term + definition) — common in compliance docs
    Definition(Definition),
    /// A blockquote or callout
    Blockquote(String),
    /// A cross-reference to another section
    CrossReference(CrossRef),
    /// A horizontal rule / thematic break
    ThematicBreak,
    /// Raw content that couldn't be classified
    Raw(String),
}

impl ContentBlock {
    /// Extract plain text content from this block.
    pub fn text(&self) -> String {
        match self {
            Self::Paragraph(t) | Self::Blockquote(t) | Self::Raw(t) => t.clone(),
            Self::Table(t) => t.to_text(),
            Self::List(l) => l.to_text(),
            Self::CodeBlock(c) => c.content.clone(),
            Self::Definition(d) => format!("{}: {}", d.term, d.definition),
            Self::CrossReference(r) => r.display_text.clone(),
            Self::ThematicBreak => String::new(),
        }
    }
}

/// A table with headers and rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub alignments: Vec<Alignment>,
}

impl Table {
    pub fn to_text(&self) -> String {
        let mut buf = String::new();
        if let Some(cap) = &self.caption {
            buf.push_str(cap);
            buf.push('\n');
        }
        if !self.headers.is_empty() {
            buf.push_str(&self.headers.join(" | "));
            buf.push('\n');
        }
        for row in &self.rows {
            buf.push_str(&row.join(" | "));
            buf.push('\n');
        }
        buf
    }
}

/// Column alignment in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
    None,
}

/// A list (ordered or unordered, possibly nested).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct List {
    pub ordered: bool,
    pub items: Vec<ListItem>,
}

impl List {
    pub fn to_text(&self) -> String {
        let mut buf = String::new();
        for (i, item) in self.items.iter().enumerate() {
            if self.ordered {
                buf.push_str(&format!("{}. {}\n", i + 1, item.text));
            } else {
                buf.push_str(&format!("- {}\n", item.text));
            }
            if let Some(sub) = &item.children {
                for line in sub.to_text().lines() {
                    buf.push_str(&format!("  {}\n", line));
                }
            }
        }
        buf
    }
}

/// A single list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub text: String,
    pub children: Option<List>,
}

/// A code block with optional language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub content: String,
}

/// A definition (term + explanation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub term: String,
    pub definition: String,
}

/// A cross-reference to another section or document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRef {
    pub target_id: String,
    pub display_text: String,
    pub internal: bool,
}

/// Content classification for distillation scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentClass {
    /// Normative requirement (SHALL/MUST)
    Requirement,
    /// Informative guidance (NOTE/EXAMPLE)
    Informative,
    /// Definition of a term
    DefinitionText,
    /// Reference to external standard
    ExternalReference,
    /// Data-bearing content (numbers, tables, thresholds)
    Data,
    /// Boilerplate (standard disclaimers, copyright)
    Boilerplate,
    /// General text
    General,
}

/// Distillation level controlling how aggressively content is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DistillationLevel {
    /// No distillation — raw conversion only
    None,
    /// Strip only — remove zero-value content (safe for legal)
    Minimal,
    /// Strip + deduplicate (default)
    #[default]
    Balanced,
    /// Strip + deduplicate + language compression
    Aggressive,
    /// All stages including scoring and attention arrangement
    Full,
}

impl DistillationLevel {
    pub fn parse_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "balanced" => Some(Self::Balanced),
            "aggressive" => Some(Self::Aggressive),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
            Self::Full => "full",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_format_from_extension() {
        assert_eq!(DocumentFormat::from_extension("md"), Some(DocumentFormat::Markdown));
        assert_eq!(DocumentFormat::from_extension("docx"), Some(DocumentFormat::Docx));
        assert_eq!(DocumentFormat::from_extension("HTML"), Some(DocumentFormat::Html));
        assert_eq!(DocumentFormat::from_extension("csv"), Some(DocumentFormat::Csv));
        assert_eq!(DocumentFormat::from_extension("rs"), None);
    }

    #[test]
    fn test_document_section_count() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut s1 = Section::new(1, "Intro");
        s1.children.push(Section::new(2, "Sub"));
        doc.sections.push(s1);
        doc.sections.push(Section::new(1, "Conclusion"));
        assert_eq!(doc.section_count(), 3);
    }

    #[test]
    fn test_content_block_text() {
        let p = ContentBlock::Paragraph("Hello world".into());
        assert_eq!(p.text(), "Hello world");

        let d = ContentBlock::Definition(Definition {
            term: "LLM".into(),
            definition: "Large Language Model".into(),
        });
        assert_eq!(d.text(), "LLM: Large Language Model");
    }

    #[test]
    fn test_distillation_level() {
        assert_eq!(DistillationLevel::parse_name("balanced"), Some(DistillationLevel::Balanced));
        assert_eq!(DistillationLevel::parse_name("FULL"), Some(DistillationLevel::Full));
        assert_eq!(DistillationLevel::parse_name("unknown"), None);
        assert_eq!(DistillationLevel::default(), DistillationLevel::Balanced);
    }

    #[test]
    fn test_table_to_text() {
        let t = Table {
            caption: Some("Access Matrix".into()),
            headers: vec!["Role".into(), "Access".into()],
            rows: vec![vec!["Admin".into(), "Full".into()]],
            alignments: vec![],
        };
        let text = t.to_text();
        assert!(text.contains("Access Matrix"));
        assert!(text.contains("Role | Access"));
        assert!(text.contains("Admin | Full"));
    }

    #[test]
    fn test_list_to_text() {
        let l = List {
            ordered: true,
            items: vec![
                ListItem { text: "First".into(), children: None },
                ListItem { text: "Second".into(), children: None },
            ],
        };
        let text = l.to_text();
        assert!(text.contains("1. First"));
        assert!(text.contains("2. Second"));
    }
}
