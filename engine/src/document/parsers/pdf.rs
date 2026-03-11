//! PDF document parser using `pdf-extract`.
//!
//! Extracts plain text content from PDF pages. Each page becomes a separate
//! section in the resulting [`Document`]. The parser does not attempt to
//! reconstruct headings or tables from the raw PDF text — those would require
//! heuristic layout analysis that is out of scope for this initial
//! implementation.
//!
//! This module is gated behind the `document-pdf` feature flag.

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse a PDF file from raw bytes into a [`Document`].
pub fn parse(content: &[u8], _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let text = pdf_extract::extract_text_from_mem(content).map_err(|e| {
        InfiniloomError::invalid_input(format!("Failed to extract text from PDF: {e}"))
    })?;

    let mut doc = Document::new("", DocumentFormat::Pdf);

    // Split extracted text into pages using form-feed characters (\x0C),
    // which pdf-extract inserts between pages.
    let pages: Vec<&str> = text.split('\u{000C}').collect();

    for (i, page_text) in pages.iter().enumerate() {
        let trimmed = page_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut section = Section::new(1, format!("Page {}", i + 1));

        // Split page text into paragraphs on blank lines.
        for paragraph in trimmed.split("\n\n") {
            let para = paragraph.trim();
            if !para.is_empty() {
                // Collapse internal newlines into spaces for cleaner output.
                let normalized = para.lines().map(str::trim).collect::<Vec<_>>().join(" ");
                section.content.push(ContentBlock::Paragraph(normalized));
            }
        }

        if !section.content.is_empty() {
            doc.sections.push(section);
        }
    }

    // If no form-feed delimiters were found and we have content, create a
    // single root section with all the text.
    if doc.sections.is_empty() && !text.trim().is_empty() {
        let mut section = Section::root();
        for paragraph in text.trim().split("\n\n") {
            let para = paragraph.trim();
            if !para.is_empty() {
                let normalized = para.lines().map(str::trim).collect::<Vec<_>>().join(" ");
                section.content.push(ContentBlock::Paragraph(normalized));
            }
        }
        if !section.content.is_empty() {
            doc.sections.push(section);
        }
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_pdf_returns_error() {
        let result = parse(b"not a pdf file", &ParseOptions::default());
        assert!(result.is_err());
    }
}
