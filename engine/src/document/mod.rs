//! Document ingestion module for converting human-readable documents into
//! LLM-optimized structured formats.
//!
//! This module provides:
//! - **Type system**: `Document`, `Section`, `ContentBlock` for representing document structure
//! - **Parsers**: Format-specific parsers (Markdown, HTML, plain text, CSV, DOCX)
//! - **Distillation**: Content compression pipeline that removes filler and optimizes for LLM attention
//! - **Output**: Document-specific formatters for Claude (XML), GPT (Markdown), agents (JSON)

// InfiniloomError is the project-wide error type; its size is not our concern here.
#![allow(clippy::result_large_err)]

pub mod distillation;
pub mod output;
pub mod parsers;
pub mod pii;
pub mod types;

pub use types::*;

use std::path::Path;

use crate::error::InfiniloomError;
use crate::tokenizer::{TokenCounts, Tokenizer};

/// Count tokens for a document's full text content across all model families.
pub fn count_document_tokens(doc: &mut Document) {
    let tokenizer = Tokenizer::new();
    let full_text = doc.full_text();
    doc.token_count = tokenizer.count_all(&full_text);
}

/// Count tokens for formatted output text across all model families.
pub fn count_output_tokens(output_text: &str) -> TokenCounts {
    let tokenizer = Tokenizer::new();
    tokenizer.count_all(output_text)
}

/// Parse a document from a file path, auto-detecting the format.
pub fn parse_document(path: &Path, options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let format = DocumentFormat::from_extension(ext).ok_or_else(|| {
        InfiniloomError::not_supported(format!("Unsupported document format: .{ext}"))
    })?;

    // DOCX and XLSX are binary formats — read as bytes, not as a UTF-8 string.
    let mut doc = if format == DocumentFormat::Docx {
        let bytes = std::fs::read(path).map_err(|e| {
            InfiniloomError::invalid_input(format!("Failed to read {}: {e}", path.display()))
        })?;
        parsers::docx::parse(&bytes, options)?
    } else if format == DocumentFormat::Xlsx {
        #[cfg(feature = "document-xlsx")]
        {
            let bytes = std::fs::read(path).map_err(|e| {
                InfiniloomError::invalid_input(format!("Failed to read {}: {e}", path.display()))
            })?;
            parsers::xlsx::parse(&bytes, options)?
        }
        #[cfg(not(feature = "document-xlsx"))]
        {
            return Err(InfiniloomError::not_supported(
                "XLSX parsing requires the 'document-xlsx' feature. \
                 Rebuild with: cargo build --features document-xlsx"
                    .to_string(),
            ));
        }
    } else {
        let content = std::fs::read_to_string(path).map_err(|e| {
            InfiniloomError::invalid_input(format!("Failed to read {}: {e}", path.display()))
        })?;
        parse_content(&content, format, options)?
    };

    doc.source = path.to_path_buf();

    // Extract title from metadata or first heading
    if doc.title.is_none() {
        doc.title = doc.metadata.title.clone();
    }
    if doc.title.is_none() {
        doc.title = doc.sections.first().and_then(|s| s.title.clone());
    }

    // Populate token counts for the parsed document
    count_document_tokens(&mut doc);

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
        let result = parse_content("test", DocumentFormat::Xlsx, &ParseOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_count_document_tokens_populates_nonzero() {
        let content = "# Introduction\n\nThis is a document with enough text to generate tokens.\n\n## Details\n\nMore detailed content goes here with several words.";
        let mut doc =
            parse_content(content, DocumentFormat::Markdown, &ParseOptions::default()).unwrap();
        // Token counts start at zero from parse_content (only parse_document calls counting)
        assert_eq!(doc.token_count.claude, 0);

        count_document_tokens(&mut doc);

        assert!(doc.token_count.claude > 0, "Claude tokens should be non-zero");
        assert!(doc.token_count.o200k > 0, "o200k tokens should be non-zero");
        assert!(doc.token_count.gemini > 0, "Gemini tokens should be non-zero");
    }

    #[test]
    fn test_count_output_tokens_returns_reasonable_counts() {
        let text = "This is a sample formatted output with several words and sentences for token counting.";
        let counts = count_output_tokens(text);

        assert!(counts.claude > 0, "Claude tokens should be non-zero");
        assert!(counts.o200k > 0, "o200k tokens should be non-zero");
        assert!(counts.gemini > 0, "Gemini tokens should be non-zero");
        // Sanity check: a ~90 character string should not produce thousands of tokens
        assert!(counts.claude < 100, "Token count should be reasonable");
    }
}
