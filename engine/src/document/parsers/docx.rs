//! DOCX document parser for Office Open XML (.docx) files.
//!
//! DOCX files are ZIP archives containing XML files. The main content
//! lives in `word/document.xml` and metadata in `docProps/core.xml`.
//!
//! This parser extracts:
//! - Heading hierarchy (Heading1–Heading9 styles)
//! - Paragraphs with text runs
//! - Tables (headers and data rows)
//! - Lists (numbered and bulleted via `w:numPr`)
//! - Metadata (title, author, dates, subject, keywords)

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse a DOCX file from raw bytes into a [`Document`].
pub fn parse(content: &[u8], options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let cursor = Cursor::new(content);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| {
        InfiniloomError::invalid_input(format!("Failed to open DOCX as ZIP archive: {e}"))
    })?;

    let mut doc = Document::new("", DocumentFormat::Docx);

    // Extract metadata from docProps/core.xml (if present).
    if let Ok(mut entry) = archive.by_name("docProps/core.xml") {
        let mut xml = String::new();
        if entry.read_to_string(&mut xml).is_ok() {
            extract_core_metadata(&xml, &mut doc.metadata);
        }
    }

    // Parse main document body from word/document.xml.
    let body_xml = {
        let mut entry = archive.by_name("word/document.xml").map_err(|e| {
            InfiniloomError::invalid_input(format!("DOCX archive missing word/document.xml: {e}"))
        })?;
        let mut xml = String::new();
        entry.read_to_string(&mut xml).map_err(|e| {
            InfiniloomError::invalid_input(format!("Failed to read word/document.xml: {e}"))
        })?;
        xml
    };

    doc.sections = parse_body(&body_xml, options);

    // Promote title from metadata.
    if doc.metadata.title.is_some() {
        doc.title = doc.metadata.title.clone();
    }

    Ok(doc)
}

// ---------------------------------------------------------------------------
// Body parsing
// ---------------------------------------------------------------------------

/// State machine for walking the OOXML body.
#[derive(Debug, Default)]
struct BodyParser {
    // Section building
    sections: Vec<Section>,
    current: Option<Section>,

    // Paragraph accumulation
    para_text: String,
    heading_level: Option<u8>,
    is_list_item: bool,
    is_bold: bool,
    is_italic: bool,

    // Table accumulation
    in_table: bool,
    table_rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    cell_text: String,

    // Element tracking
    in_run: bool,
    in_text: bool,
    in_para_props: bool,
    in_run_props: bool,
    in_table_cell: bool,

    max_depth: u8,
}

fn parse_body(xml: &str, options: &ParseOptions) -> Vec<Section> {
    let mut reader = Reader::from_str(xml);

    let mut parser = BodyParser {
        current: Some(Section::root()),
        max_depth: options.max_depth,
        ..Default::default()
    };

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name_owned(e.name().as_ref());
                parser.handle_start(&local, e);
            },
            Ok(Event::End(ref e)) => {
                let local = local_name_owned(e.name().as_ref());
                parser.handle_end(&local);
            },
            Ok(Event::Text(ref e)) => {
                if let Ok(text) = e.unescape() {
                    parser.handle_text(&text);
                }
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }

    // Flush anything remaining.
    parser.flush_paragraph();
    parser.flush_current_section();
    parser.sections
}

impl BodyParser {
    fn handle_start(&mut self, local: &str, event: &quick_xml::events::BytesStart<'_>) {
        match local {
            // Paragraph
            "p" => {
                self.para_text.clear();
                self.heading_level = None;
                self.is_list_item = false;
                self.is_bold = false;
                self.is_italic = false;
                self.in_run = false;
            },
            // Paragraph properties
            "pPr" => {
                self.in_para_props = true;
            },
            // Paragraph style — detect headings
            "pStyle" if self.in_para_props => {
                if let Some(level) = extract_heading_level(event) {
                    if level <= self.max_depth {
                        self.heading_level = Some(level);
                    }
                }
            },
            // Numbering properties — detect lists
            "numPr" if self.in_para_props => {
                self.is_list_item = true;
            },
            // Run
            "r" => {
                self.in_run = true;
            },
            // Run properties
            "rPr" if self.in_run => {
                self.in_run_props = true;
            },
            // Bold
            "b" if self.in_run_props => {
                self.is_bold = true;
            },
            // Italic
            "i" if self.in_run_props => {
                self.is_italic = true;
            },
            // Text content
            "t" if self.in_run => {
                self.in_text = true;
            },
            // Table
            "tbl" => {
                self.flush_paragraph();
                self.in_table = true;
                self.table_rows.clear();
            },
            // Table row
            "tr" if self.in_table => {
                self.current_row.clear();
            },
            // Table cell
            "tc" if self.in_table => {
                self.in_table_cell = true;
                self.cell_text.clear();
            },
            // Hyperlink — runs inside still produce text
            "hyperlink" => {},
            _ => {},
        }
    }

    fn handle_end(&mut self, local: &str) {
        match local {
            "pPr" => {
                self.in_para_props = false;
            },
            "rPr" => {
                self.in_run_props = false;
            },
            "r" => {
                self.in_run = false;
            },
            "t" => {
                self.in_text = false;
            },
            "p" => {
                // If we are inside a table cell, accumulate into cell_text
                // instead of creating a content block.
                if self.in_table_cell {
                    if !self.cell_text.is_empty() && !self.para_text.is_empty() {
                        self.cell_text.push(' ');
                    }
                    self.cell_text.push_str(self.para_text.trim());
                } else {
                    self.flush_paragraph();
                }
            },
            "tc" if self.in_table => {
                self.in_table_cell = false;
                self.current_row.push(self.cell_text.trim().to_owned());
                self.cell_text.clear();
            },
            "tr" if self.in_table => {
                if !self.current_row.is_empty() {
                    self.table_rows.push(std::mem::take(&mut self.current_row));
                }
            },
            "tbl" => {
                self.in_table = false;
                self.flush_table();
            },
            _ => {},
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.in_text && self.in_run {
            self.para_text.push_str(text);
        }
    }

    /// Flush the accumulated paragraph text into the current section.
    fn flush_paragraph(&mut self) {
        let text = self.para_text.trim().to_owned();
        self.para_text.clear();

        if text.is_empty() {
            return;
        }

        // If this paragraph is a heading, start a new section.
        if let Some(level) = self.heading_level.take() {
            self.flush_current_section();
            self.current = Some(Section::new(level, &text));
            return;
        }

        let current = self.current.get_or_insert_with(Section::root);

        if self.is_list_item {
            // Try to append to an existing trailing list, otherwise create one.
            let item = ListItem { text, children: None };
            if let Some(ContentBlock::List(list)) = current.content.last_mut() {
                list.items.push(item);
            } else {
                current.content.push(ContentBlock::List(List {
                    ordered: false, // DOCX numbering type detection is complex; default to unordered.
                    items: vec![item],
                }));
            }
        } else {
            current.content.push(ContentBlock::Paragraph(text));
        }
    }

    /// Flush accumulated table rows into a `ContentBlock::Table`.
    fn flush_table(&mut self) {
        if self.table_rows.is_empty() {
            return;
        }

        let rows = std::mem::take(&mut self.table_rows);

        // First row treated as header.
        let (headers, data_rows) = if rows.len() > 1 {
            (rows[0].clone(), rows[1..].to_vec())
        } else {
            (Vec::new(), rows)
        };

        let table = Table { caption: None, headers, rows: data_rows, alignments: Vec::new() };

        let current = self.current.get_or_insert_with(Section::root);
        current.content.push(ContentBlock::Table(table));
    }

    /// Push the current section onto the section list and reset.
    fn flush_current_section(&mut self) {
        if let Some(section) = self.current.take() {
            // Only push non-empty sections.
            if section.title.is_some() || !section.content.is_empty() {
                self.sections.push(section);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata extraction from docProps/core.xml
// ---------------------------------------------------------------------------

fn extract_core_metadata(xml: &str, meta: &mut DocumentMetadata) {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut current_tag = String::new();
    let mut in_keywords = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = local_name_owned(e.name().as_ref());
                in_keywords = current_tag == "keywords";
            },
            Ok(Event::Text(ref e)) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim().to_owned();
                    if text.is_empty() {
                        continue;
                    }
                    match current_tag.as_str() {
                        "title" => meta.title = Some(text),
                        "creator" => meta.author = Some(text),
                        "created" => meta.created = Some(text),
                        "modified" => meta.modified = Some(text),
                        "subject" => meta.subject = Some(text),
                        "keywords" if in_keywords => {
                            meta.keywords = text.split(',').map(|k| k.trim().to_owned()).collect();
                        },
                        _ => {},
                    }
                }
            },
            Ok(Event::End(_)) => {
                current_tag.clear();
                in_keywords = false;
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Strip the namespace prefix from an XML element name, e.g. `w:p` -> `p`.
fn local_name_owned(full: &[u8]) -> String {
    let s = std::str::from_utf8(full).unwrap_or("");
    s.rsplit_once(':').map_or(s, |(_, local)| local).to_owned()
}

/// Extract the heading level from a `w:pStyle` element's `w:val` attribute.
///
/// Matches: `Heading1`–`Heading9`, `heading 1`–`heading 9` (case-insensitive).
fn extract_heading_level(event: &quick_xml::events::BytesStart<'_>) -> Option<u8> {
    for attr in event.attributes().flatten() {
        let key = local_name_owned(attr.key.as_ref());
        if key == "val" {
            let val = String::from_utf8_lossy(&attr.value).to_lowercase();
            // "heading1", "heading 1", "heading2", etc.
            let stripped = val.strip_prefix("heading")?;
            let stripped = stripped.trim();
            return stripped.parse::<u8>().ok().filter(|l| (1..=9).contains(l));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;
    use std::io::Write;

    /// Helper: build a minimal .docx ZIP in memory from raw `word/document.xml` content
    /// and an optional `docProps/core.xml`.
    fn create_test_docx(document_xml: &str, core_xml: Option<&str>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            // [Content_Types].xml (minimal)
            zip.start_file("[Content_Types].xml", opts).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
</Types>"#,
            )
            .unwrap();

            // word/document.xml
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(document_xml.as_bytes()).unwrap();

            // docProps/core.xml (optional)
            if let Some(core) = core_xml {
                zip.start_file("docProps/core.xml", opts).unwrap();
                zip.write_all(core.as_bytes()).unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    /// Wrap paragraphs in a minimal OOXML document body.
    fn wrap_body(inner: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    {inner}
  </w:body>
</w:document>"#
        )
    }

    // -----------------------------------------------------------------------
    // Paragraph extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_simple_paragraphs() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:r><w:t>Hello world</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:t>Second paragraph</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        assert_eq!(doc.format, DocumentFormat::Docx);
        assert!(doc.section_count() >= 1);

        let text = doc.full_text();
        assert!(text.contains("Hello world"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_multiple_runs_in_paragraph() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:r><w:t>Hello </w:t></w:r>
              <w:r><w:t>world</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();
        let text = doc.full_text();
        assert!(text.contains("Hello world"));
    }

    // -----------------------------------------------------------------------
    // Heading extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_heading_extraction() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
              <w:r><w:t>Introduction</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:t>Some intro text.</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
              <w:r><w:t>Background</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:t>Background info.</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        // Should have 2 heading sections.
        let headings: Vec<_> = doc.sections.iter().filter(|s| s.title.is_some()).collect();
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].title.as_deref(), Some("Introduction"));
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[1].title.as_deref(), Some("Background"));
        assert_eq!(headings[1].level, 2);
    }

    #[test]
    fn test_heading_case_insensitive() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:pPr><w:pStyle w:val="heading 2"/></w:pPr>
              <w:r><w:t>Lower case heading</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        let heading = doc.sections.iter().find(|s| s.title.is_some()).unwrap();
        assert_eq!(heading.level, 2);
        assert_eq!(heading.title.as_deref(), Some("Lower case heading"));
    }

    // -----------------------------------------------------------------------
    // Table extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_table_extraction() {
        let body = wrap_body(
            r#"
            <w:tbl>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc>
              </w:tr>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Alpha</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc>
              </w:tr>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Beta</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>200</w:t></w:r></w:p></w:tc>
              </w:tr>
            </w:tbl>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        let table = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .find_map(|b| match b {
                ContentBlock::Table(t) => Some(t),
                _ => None,
            })
            .expect("expected a table");

        assert_eq!(table.headers, vec!["Name", "Value"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["Alpha", "100"]);
        assert_eq!(table.rows[1], vec!["Beta", "200"]);
    }

    // -----------------------------------------------------------------------
    // List extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_extraction() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
              <w:r><w:t>First item</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
              <w:r><w:t>Second item</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        let list = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .find_map(|b| match b {
                ContentBlock::List(l) => Some(l),
                _ => None,
            })
            .expect("expected a list");

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].text, "First item");
        assert_eq!(list.items[1].text, "Second item");
    }

    // -----------------------------------------------------------------------
    // Metadata extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_metadata_extraction() {
        let body = wrap_body(r#"<w:p><w:r><w:t>Content</w:t></w:r></w:p>"#);
        let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>Test Document</dc:title>
  <dc:creator>Alice</dc:creator>
  <dc:subject>Testing</dc:subject>
  <cp:keywords>rust, docx, parser</cp:keywords>
  <dcterms:created>2025-01-15T10:00:00Z</dcterms:created>
  <dcterms:modified>2025-06-01T12:30:00Z</dcterms:modified>
</cp:coreProperties>"#;

        let docx = create_test_docx(&body, Some(core));
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        assert_eq!(doc.metadata.title.as_deref(), Some("Test Document"));
        assert_eq!(doc.metadata.author.as_deref(), Some("Alice"));
        assert_eq!(doc.metadata.subject.as_deref(), Some("Testing"));
        assert_eq!(doc.metadata.created.as_deref(), Some("2025-01-15T10:00:00Z"));
        assert_eq!(doc.metadata.modified.as_deref(), Some("2025-06-01T12:30:00Z"));
        assert_eq!(doc.metadata.keywords, vec!["rust", "docx", "parser"]);
        assert_eq!(doc.title.as_deref(), Some("Test Document"));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_invalid_zip() {
        let result = parse(b"not a zip file", &ParseOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_document_xml() {
        // Valid ZIP but no word/document.xml
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("dummy.txt", opts).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }
        let result = parse(&buf, &ParseOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_document() {
        let body = wrap_body("");
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();
        assert_eq!(doc.format, DocumentFormat::Docx);
        // No content, so no sections with content.
        let total_blocks: usize = doc.sections.iter().map(|s| s.content.len()).sum();
        assert_eq!(total_blocks, 0);
    }

    #[test]
    fn test_no_core_metadata() {
        let body = wrap_body(r#"<w:p><w:r><w:t>Just text</w:t></w:r></w:p>"#);
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();
        assert!(doc.metadata.title.is_none());
        assert!(doc.metadata.author.is_none());
    }

    #[test]
    fn test_heading_level_capped_by_max_depth() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
              <w:r><w:t>H1</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:pStyle w:val="Heading4"/></w:pPr>
              <w:r><w:t>H4 too deep</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);

        let mut opts = ParseOptions::default();
        opts.max_depth = 3;
        let doc = parse(&docx, &opts).unwrap();

        // H1 should be a section heading, H4 should be a regular paragraph.
        let heading_sections: Vec<_> = doc.sections.iter().filter(|s| s.title.is_some()).collect();
        assert_eq!(heading_sections.len(), 1);
        assert_eq!(heading_sections[0].title.as_deref(), Some("H1"));

        // "H4 too deep" should appear as paragraph text.
        let text = doc.full_text();
        assert!(text.contains("H4 too deep"));
    }

    #[test]
    fn test_mixed_content() {
        let body = wrap_body(
            r#"
            <w:p>
              <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
              <w:r><w:t>Title</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:t>Intro paragraph.</w:t></w:r>
            </w:p>
            <w:tbl>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Col A</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Col B</w:t></w:r></w:p></w:tc>
              </w:tr>
              <w:tr>
                <w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc>
              </w:tr>
            </w:tbl>
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
              <w:r><w:t>List item A</w:t></w:r>
            </w:p>
            "#,
        );
        let docx = create_test_docx(&body, None);
        let doc = parse(&docx, &ParseOptions::default()).unwrap();

        // Should have a heading section with mixed content.
        let section = doc
            .sections
            .iter()
            .find(|s| s.title.as_deref() == Some("Title"))
            .expect("expected Title section");

        let has_paragraph = section
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::Paragraph(_)));
        let has_table = section
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::Table(_)));
        let has_list = section
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::List(_)));

        assert!(has_paragraph);
        assert!(has_table);
        assert!(has_list);
    }
}
