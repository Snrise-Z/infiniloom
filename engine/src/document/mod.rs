//! Document ingestion module for converting human-readable documents into
//! LLM-optimized structured formats.
//!
//! This module provides:
//! - **Type system**: `Document`, `Section`, `ContentBlock` for representing document structure
//! - **Parsers**: Format-specific parsers (Markdown, HTML, plain text, CSV, DOCX)
//! - **Distillation**: Content compression pipeline that removes filler and optimizes for LLM attention
//! - **Output**: Document-specific formatters for Claude (XML), GPT (Markdown), agents (JSON)

pub mod distillation;
pub mod output;
pub mod parsers;
pub mod types;

pub use types::*;

use std::path::Path;

use crate::error::InfiniloomError;

/// Parse a document from a file path, auto-detecting the format.
pub fn parse_document(path: &Path, options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let format = DocumentFormat::from_extension(ext).ok_or_else(|| {
        InfiniloomError::not_supported(format!("Unsupported document format: .{ext}"))
    })?;

    let content = std::fs::read_to_string(path).map_err(|e| {
        InfiniloomError::invalid_input(format!("Failed to read {}: {e}", path.display()))
    })?;

    let mut doc = parse_content(&content, format, options)?;
    doc.source = path.to_path_buf();

    // Extract title from metadata or first heading
    if doc.title.is_none() {
        doc.title = doc.metadata.title.clone();
    }
    if doc.title.is_none() {
        doc.title = doc.sections.first().and_then(|s| s.title.clone());
    }

    Ok(doc)
}

/// Parse document content from a string with a known format.
pub fn parse_content(
    content: &str,
    format: DocumentFormat,
    options: &ParseOptions,
) -> Result<Document, InfiniloomError> {
    match format {
        DocumentFormat::Markdown => parsers::markdown::parse(content, options),
        DocumentFormat::PlainText => parsers::plaintext::parse(content, options),
        DocumentFormat::Html => parsers::html::parse(content, options),
        DocumentFormat::Csv => parsers::csv::parse(content, options),
        _ => Err(InfiniloomError::not_supported(format!(
            "Parser not yet implemented for {}",
            format.name()
        ))),
    }
}

/// Options for document parsing.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Extract tables from content
    pub extract_tables: bool,
    /// Maximum heading depth to track
    pub max_depth: u8,
    /// Distillation level to apply after parsing
    pub distillation: DistillationLevel,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self { extract_tables: true, max_depth: 6, distillation: DistillationLevel::Balanced }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_markdown() {
        let content = "# Hello\n\nThis is a test.\n\n## Section 2\n\nMore text.";
        let doc =
            parse_content(content, DocumentFormat::Markdown, &ParseOptions::default()).unwrap();
        assert_eq!(doc.section_count(), 2);
    }

    #[test]
    fn test_parse_content_plaintext() {
        let content = "INTRODUCTION\n\nSome text here.\n\nCONCLUSION\n\nFinal text.";
        let doc =
            parse_content(content, DocumentFormat::PlainText, &ParseOptions::default()).unwrap();
        assert!(doc.section_count() >= 1);
    }

    #[test]
    fn test_unsupported_format() {
        let result = parse_content("test", DocumentFormat::Docx, &ParseOptions::default());
        assert!(result.is_err());
    }
}
