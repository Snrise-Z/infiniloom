//! Markdown document parser using pulldown-cmark.
//!
//! Converts CommonMark + GFM tables into the Document model with proper
//! heading hierarchy, lists, code blocks, and tables.

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse Markdown content into a Document.
pub fn parse(content: &str, _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    // Strip YAML frontmatter (--- delimited block at the start of file)
    let content = strip_frontmatter(content);

    let mut doc = Document::new("", DocumentFormat::Markdown);
    let mut sections: Vec<Section> = Vec::new();
    let mut current_section = Section::root();

    let mut lines = content.lines().peekable();
    let mut in_code_block = false;
    let mut code_fence_char: char = '`';
    let mut code_lang: Option<String> = None;
    let mut code_buf = String::new();
    let mut para_buf = String::new();
    let mut in_table = false;
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_alignments: Vec<Alignment> = Vec::new();
    let mut in_list = false;
    let mut list_ordered = false;
    let mut list_items: Vec<ListItem> = Vec::new();
    let mut in_blockquote = false;
    let mut blockquote_buf = String::new();

    while let Some(line) = lines.next() {
        // Code fence handling — closing fence must match opening fence character
        if line.starts_with("```") || line.starts_with("~~~") {
            let fence_char = line.chars().next().unwrap();
            if in_code_block && fence_char == code_fence_char {
                current_section
                    .content
                    .push(ContentBlock::CodeBlock(CodeBlock {
                        language: code_lang.take(),
                        content: code_buf.trim_end().to_owned(),
                    }));
                code_buf.clear();
                in_code_block = false;
                continue;
            }
            if !in_code_block {
                in_code_block = true;
                code_fence_char = fence_char;
                let lang = line.trim_start_matches(fence_char).trim();
                code_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_owned())
                };
                continue;
            }
        }
        if in_code_block {
            if !code_buf.is_empty() {
                code_buf.push('\n');
            }
            code_buf.push_str(line);
            continue;
        }

        // Blockquote
        if line.starts_with("> ") || line == ">" {
            if !in_blockquote {
                flush_paragraph(&mut para_buf, &mut current_section);
                flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
                in_blockquote = true;
            }
            let text = line.strip_prefix("> ").unwrap_or("");
            if !blockquote_buf.is_empty() {
                blockquote_buf.push('\n');
            }
            blockquote_buf.push_str(text);
            continue;
        }
        if in_blockquote {
            current_section
                .content
                .push(ContentBlock::Blockquote(blockquote_buf.trim().to_owned()));
            blockquote_buf.clear();
            in_blockquote = false;
        }

        // Table handling — require leading or trailing pipe to avoid false positives
        // from prose containing | (shell commands, logical expressions, etc.)
        if !line.trim().is_empty() && (line.trim().starts_with('|') || line.trim().ends_with('|')) {
            let cells: Vec<String> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect();

            // Check if this is a separator row (---|---|---)
            if cells
                .iter()
                .all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
            {
                table_alignments = cells
                    .iter()
                    .map(|c| {
                        let c = c.trim();
                        if c.starts_with(':') && c.ends_with(':') {
                            Alignment::Center
                        } else if c.ends_with(':') {
                            Alignment::Right
                        } else if c.starts_with(':') {
                            Alignment::Left
                        } else {
                            Alignment::None
                        }
                    })
                    .collect();
                continue;
            }

            if !in_table {
                flush_paragraph(&mut para_buf, &mut current_section);
                flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
                in_table = true;
                table_headers = cells;
            } else {
                table_rows.push(cells);
            }
            continue;
        }
        if in_table {
            current_section.content.push(ContentBlock::Table(Table {
                caption: None,
                headers: std::mem::take(&mut table_headers),
                rows: std::mem::take(&mut table_rows),
                alignments: std::mem::take(&mut table_alignments),
            }));
            in_table = false;
        }

        // Blank line
        if line.trim().is_empty() {
            flush_paragraph(&mut para_buf, &mut current_section);
            flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
            continue;
        }

        // Thematic break
        if is_thematic_break(line) {
            flush_paragraph(&mut para_buf, &mut current_section);
            flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
            current_section.content.push(ContentBlock::ThematicBreak);
            continue;
        }

        // ATX Heading (# Title)
        if let Some((level, title)) = parse_atx_heading(line) {
            flush_paragraph(&mut para_buf, &mut current_section);
            flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);

            // Save current section
            if current_section.title.is_some() || !current_section.content.is_empty() {
                sections.push(current_section);
            }
            current_section = Section::new(level, title);
            continue;
        }

        // Setext heading (underline with === or ---)
        if let Some(next_line) = lines.peek() {
            if next_line.starts_with("===") && !line.trim().is_empty() {
                flush_paragraph(&mut para_buf, &mut current_section);
                flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
                if current_section.title.is_some() || !current_section.content.is_empty() {
                    sections.push(current_section);
                }
                current_section = Section::new(1, line.trim());
                lines.next(); // consume underline
                continue;
            }
            if next_line.starts_with("---") && !next_line.contains('|') && !line.trim().is_empty() {
                flush_paragraph(&mut para_buf, &mut current_section);
                flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
                if current_section.title.is_some() || !current_section.content.is_empty() {
                    sections.push(current_section);
                }
                current_section = Section::new(2, line.trim());
                lines.next(); // consume underline
                continue;
            }
        }

        // List items
        if let Some(text) = parse_list_item(line) {
            flush_paragraph(&mut para_buf, &mut current_section);
            let is_ordered = line
                .trim_start()
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_digit());
            if !in_list {
                in_list = true;
                list_ordered = is_ordered;
            }
            list_items.push(ListItem { text: text.to_owned(), children: None });
            continue;
        }

        // Continuation of list item (indented)
        if in_list && (line.starts_with("  ") || line.starts_with('\t')) {
            if let Some(last) = list_items.last_mut() {
                last.text.push(' ');
                last.text.push_str(line.trim());
            }
            continue;
        }

        // Regular paragraph text
        flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);
        if !para_buf.is_empty() {
            para_buf.push(' ');
        }
        para_buf.push_str(line.trim());
    }

    // Flush remaining state
    if in_code_block {
        current_section
            .content
            .push(ContentBlock::CodeBlock(CodeBlock {
                language: code_lang,
                content: code_buf.trim_end().to_owned(),
            }));
    }
    if in_blockquote {
        current_section
            .content
            .push(ContentBlock::Blockquote(blockquote_buf.trim().to_owned()));
    }
    if in_table {
        current_section.content.push(ContentBlock::Table(Table {
            caption: None,
            headers: table_headers,
            rows: table_rows,
            alignments: table_alignments,
        }));
    }
    flush_paragraph(&mut para_buf, &mut current_section);
    flush_list(&mut in_list, &mut list_items, list_ordered, &mut current_section);

    if current_section.title.is_some() || !current_section.content.is_empty() {
        sections.push(current_section);
    }

    // Build heading hierarchy
    doc.sections = build_hierarchy(sections);

    // Extract title from metadata or first H1
    doc.title = doc.metadata.title.clone().or_else(|| {
        doc.sections
            .first()
            .and_then(|s| if s.level == 1 { s.title.clone() } else { None })
    });

    Ok(doc)
}

fn flush_paragraph(buf: &mut String, section: &mut Section) {
    if !buf.is_empty() {
        section
            .content
            .push(ContentBlock::Paragraph(buf.trim().to_owned()));
        buf.clear();
    }
}

fn flush_list(in_list: &mut bool, items: &mut Vec<ListItem>, ordered: bool, section: &mut Section) {
    if *in_list && !items.is_empty() {
        section
            .content
            .push(ContentBlock::List(List { ordered, items: std::mem::take(items) }));
    }
    *in_list = false;
}

fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    // Remove trailing # characters
    let title = rest.trim().trim_end_matches('#').trim();
    Some((level as u8, title.to_owned()))
}

fn parse_list_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    // Unordered: - item, * item, + item
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return Some(rest);
    }
    // Ordered: 1. item, 2. item, etc.
    // Find the end of leading digits
    let digit_end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if digit_end > 0 {
        let after_digits = &trimmed[digit_end..];
        if let Some(text) = after_digits
            .strip_prefix(". ")
            .or_else(|| after_digits.strip_prefix(") "))
        {
            return Some(text);
        }
    }
    None
}

fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let ch = trimmed.chars().next().unwrap_or(' ');
    (ch == '-' || ch == '*' || ch == '_')
        && trimmed.chars().all(|c| c == ch || c == ' ')
        && trimmed.chars().filter(|&c| c == ch).count() >= 3
}

/// Build a proper heading hierarchy from a flat list of sections.
fn build_hierarchy(flat: Vec<Section>) -> Vec<Section> {
    if flat.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<Section> = Vec::new();
    let mut stack: Vec<Section> = Vec::new();

    for section in flat {
        // Pop sections from stack that are at same or deeper level
        while let Some(top) = stack.last() {
            if section.level > 0 && top.level > 0 && top.level >= section.level {
                let popped = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(popped);
                } else {
                    result.push(popped);
                }
            } else {
                break;
            }
        }
        stack.push(section);
    }

    // Flush remaining stack
    while let Some(popped) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(popped);
        } else {
            result.push(popped);
        }
    }

    result
}

/// Strip YAML frontmatter from the beginning of a Markdown document.
/// Frontmatter is delimited by `---` at the very start of the file.
fn strip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    // Find the closing `---` (must be on its own line after the opening)
    let after_open = &content[3..];
    // Skip the rest of the opening line
    let after_newline = match after_open.find('\n') {
        Some(pos) => &after_open[pos + 1..],
        None => return content, // No newline after opening `---`
    };
    // Find closing `---` on its own line
    // Use byte-accurate offset tracking to handle both LF and CRLF line endings
    let mut byte_offset = 0;
    for line in after_newline.lines() {
        let line_byte_len = line.len();
        // Advance past the line content
        byte_offset += line_byte_len;
        // Advance past the line ending (LF or CRLF)
        if byte_offset < after_newline.len() {
            if after_newline.as_bytes()[byte_offset] == b'\r' {
                byte_offset += 1; // CR
            }
            if byte_offset < after_newline.len() && after_newline.as_bytes()[byte_offset] == b'\n' {
                byte_offset += 1; // LF
            }
        }
        if line.trim() == "---" {
            let remaining = &after_newline[byte_offset.min(after_newline.len())..];
            return remaining;
        }
    }
    // No closing `---` found, return content as-is
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;

    #[test]
    fn test_parse_basic_markdown() {
        let md =
            "# Title\n\nParagraph text.\n\n## Section 2\n\nMore text.\n\n### Sub-section\n\nDeep.";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        assert_eq!(doc.sections.len(), 1); // One top-level H1
        assert_eq!(doc.sections[0].title.as_deref(), Some("Title"));
        assert_eq!(doc.sections[0].children.len(), 1); // One H2 child
        assert_eq!(doc.sections[0].children[0].children.len(), 1); // One H3 grandchild
    }

    #[test]
    fn test_parse_code_block() {
        let md = "# Code\n\n```rust\nfn main() {}\n```\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let blocks = &doc.sections[0].content;
        assert!(blocks.iter().any(
            |b| matches!(b, ContentBlock::CodeBlock(c) if c.language.as_deref() == Some("rust"))
        ));
    }

    #[test]
    fn test_parse_table() {
        let md = "# Data\n\n| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let blocks = &doc.sections[0].content;
        let table = blocks.iter().find_map(|b| {
            if let ContentBlock::Table(t) = b {
                Some(t)
            } else {
                None
            }
        });
        assert!(table.is_some());
        let t = table.unwrap();
        assert_eq!(t.headers, vec!["A", "B"]);
        assert_eq!(t.rows.len(), 2);
    }

    #[test]
    fn test_parse_list() {
        let md = "# List\n\n- Item 1\n- Item 2\n- Item 3\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let blocks = &doc.sections[0].content;
        let list = blocks.iter().find_map(|b| {
            if let ContentBlock::List(l) = b {
                Some(l)
            } else {
                None
            }
        });
        assert!(list.is_some());
        assert_eq!(list.unwrap().items.len(), 3);
        assert!(!list.unwrap().ordered);
    }

    #[test]
    fn test_parse_ordered_list() {
        let md = "1. First\n2. Second\n3. Third\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let blocks: Vec<_> = doc.sections.iter().flat_map(|s| &s.content).collect();
        let list = blocks.iter().find_map(|b| {
            if let ContentBlock::List(l) = b {
                Some(l)
            } else {
                None
            }
        });
        assert!(list.is_some());
        assert!(list.unwrap().ordered);
    }

    #[test]
    fn test_parse_blockquote() {
        let md = "# Quote\n\n> This is quoted text\n> continues here\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let blocks = &doc.sections[0].content;
        assert!(blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Blockquote(_))));
    }

    #[test]
    fn test_atx_heading_parsing() {
        assert_eq!(parse_atx_heading("# Title"), Some((1, "Title".into())));
        assert_eq!(parse_atx_heading("### Deep"), Some((3, "Deep".into())));
        assert_eq!(parse_atx_heading("## Title ##"), Some((2, "Title".into())));
        assert_eq!(parse_atx_heading("Not a heading"), None);
        assert_eq!(parse_atx_heading("#NoSpace"), None);
    }

    #[test]
    fn test_hierarchy_building() {
        let flat = vec![
            Section::new(1, "H1"),
            Section::new(2, "H2a"),
            Section::new(2, "H2b"),
            Section::new(1, "H1b"),
        ];
        let tree = build_hierarchy(flat);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[1].children.len(), 0);
    }

    #[test]
    fn test_no_heading_document() {
        let md = "Just some plain text\nwith multiple lines.\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        assert!(doc.section_count() >= 1);
    }

    #[test]
    fn test_frontmatter_stripped() {
        let md = "---\ntitle: My Doc\ndate: 2025-01-01\n---\n# Heading\nContent here.\n";
        let doc = parse(md, &ParseOptions::default()).unwrap();
        let text = doc.full_text();
        assert!(!text.contains("title: My Doc"));
        assert!(text.contains("Heading"));
        assert!(text.contains("Content here"));
    }
}
