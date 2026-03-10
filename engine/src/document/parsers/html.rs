//! HTML document parser with semantic structure extraction.
//!
//! Extracts headings, paragraphs, lists, tables, code blocks, and blockquotes
//! from HTML by walking the DOM structure. Strips navigation, scripts, and styles.

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Maximum input size for HTML parsing (50 MB).
/// Prevents unbounded memory allocation from very large HTML files.
const MAX_HTML_SIZE: usize = 50 * 1024 * 1024;

/// Maximum iterations for tag stripping loops to prevent quadratic behavior.
const MAX_STRIP_ITERATIONS: usize = 10_000;

/// Parse HTML content into a Document.
///
/// Uses a simple tag-based approach without external HTML parser dependencies.
/// For production use with malformed HTML, consider adding `scraper`/`html5ever`.
pub fn parse(content: &str, _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    if content.len() > MAX_HTML_SIZE {
        return Err(InfiniloomError::invalid_input(format!(
            "HTML input exceeds size limit ({} bytes > {} bytes)",
            content.len(),
            MAX_HTML_SIZE
        )));
    }
    let mut doc = Document::new("", DocumentFormat::Html);

    // Extract <title> for metadata
    if let Some(title) = extract_tag_content(content, "title") {
        doc.metadata.title = Some(decode_entities(&title));
        doc.title = doc.metadata.title.clone();
    }

    // Extract <meta> tags for metadata
    extract_meta_tags(content, &mut doc.metadata);

    // Strip non-content elements
    let cleaned = strip_non_content(content);

    // Extract body content (or use full content if no <body>)
    let body = extract_tag_content(&cleaned, "body").unwrap_or_else(|| cleaned.clone());

    // Parse the body into sections
    doc.sections = parse_body(&body);

    Ok(doc)
}

fn parse_body(html: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current = Section::root();
    let mut pos = 0;
    let bytes = html.as_bytes();

    while pos < bytes.len() {
        // Find next tag
        if let Some(tag_start) = html[pos..].find('<') {
            let tag_start = pos + tag_start;

            // Collect text before this tag
            let text_before = html[pos..tag_start].trim();
            if !text_before.is_empty() {
                let decoded = decode_entities(text_before);
                if !decoded.trim().is_empty() {
                    current
                        .content
                        .push(ContentBlock::Paragraph(decoded.trim().to_owned()));
                }
            }

            // Find tag end
            if let Some(tag_end) = html[tag_start..].find('>') {
                let tag_end = tag_start + tag_end + 1;
                let tag = &html[tag_start..tag_end];
                let tag_lower = tag.to_ascii_lowercase();

                // Heading tags h1-h6
                let mut found_heading = false;
                for level in 1u8..=6 {
                    let open = format!("<h{}", level);
                    let close = format!("</h{}>", level);
                    if tag_lower.starts_with(&open) {
                        if let Some(close_pos) = html[tag_end..].to_ascii_lowercase().find(&close) {
                            let heading_text = strip_tags(&html[tag_end..tag_end + close_pos]);
                            let decoded = decode_entities(&heading_text);
                            if !decoded.trim().is_empty() {
                                push_section(&mut sections, &mut current);
                                current = Section::new(level, decoded.trim());
                            }
                            pos = tag_end + close_pos + close.len();
                            found_heading = true;
                        }
                        break;
                    }
                }
                if found_heading {
                    continue;
                }

                // Paragraph
                if tag_lower.starts_with("<p")
                    && (tag_lower.as_bytes().get(2) == Some(&b'>')
                        || tag_lower.as_bytes().get(2) == Some(&b' '))
                {
                    if let Some(close_pos) = html[tag_end..].to_ascii_lowercase().find("</p>") {
                        let para_text = strip_tags(&html[tag_end..tag_end + close_pos]);
                        let decoded = decode_entities(&para_text);
                        if !decoded.trim().is_empty() {
                            current
                                .content
                                .push(ContentBlock::Paragraph(decoded.trim().to_owned()));
                        }
                        pos = tag_end + close_pos + 4;
                        continue;
                    }
                }

                // Lists (ul/ol)
                if tag_lower.starts_with("<ul") || tag_lower.starts_with("<ol") {
                    let ordered = tag_lower.starts_with("<ol");
                    let open_tag = if ordered { "ol" } else { "ul" };
                    if let Some(close_pos) = find_matching_close_tag(&html[tag_end..], open_tag) {
                        let list_html = &html[tag_end..tag_end + close_pos];
                        let items = extract_list_items(list_html);
                        if !items.is_empty() {
                            current
                                .content
                                .push(ContentBlock::List(List { ordered, items }));
                        }
                        let close_tag = format!("</{}>", open_tag);
                        pos = tag_end + close_pos + close_tag.len();
                        continue;
                    }
                }

                // Table
                if tag_lower.starts_with("<table") {
                    if let Some(close_pos) = html[tag_end..].to_ascii_lowercase().find("</table>") {
                        let table_html = &html[tag_end..tag_end + close_pos];
                        if let Some(table) = extract_table(table_html) {
                            current.content.push(ContentBlock::Table(table));
                        }
                        pos = tag_end + close_pos + 8;
                        continue;
                    }
                }

                // Pre/code blocks
                if tag_lower.starts_with("<pre") {
                    if let Some(close_pos) = html[tag_end..].to_ascii_lowercase().find("</pre>") {
                        let code_text = strip_tags(&html[tag_end..tag_end + close_pos]);
                        let decoded = decode_entities(&code_text);
                        current.content.push(ContentBlock::CodeBlock(CodeBlock {
                            language: None,
                            content: decoded,
                        }));
                        pos = tag_end + close_pos + 6;
                        continue;
                    }
                }

                // Blockquote
                if tag_lower.starts_with("<blockquote") {
                    if let Some(close_pos) =
                        html[tag_end..].to_ascii_lowercase().find("</blockquote>")
                    {
                        let bq_text = strip_tags(&html[tag_end..tag_end + close_pos]);
                        let decoded = decode_entities(&bq_text);
                        if !decoded.trim().is_empty() {
                            current
                                .content
                                .push(ContentBlock::Blockquote(decoded.trim().to_owned()));
                        }
                        pos = tag_end + close_pos + 13;
                        continue;
                    }
                }

                // HR / thematic break
                if tag_lower.starts_with("<hr") {
                    current.content.push(ContentBlock::ThematicBreak);
                    pos = tag_end;
                    continue;
                }

                pos = tag_end;
            } else {
                pos = tag_start + 1;
            }
        } else {
            // No more tags — collect remaining text
            let remaining = html[pos..].trim();
            if !remaining.is_empty() {
                let decoded = decode_entities(remaining);
                if !decoded.trim().is_empty() {
                    current
                        .content
                        .push(ContentBlock::Paragraph(decoded.trim().to_owned()));
                }
            }
            break;
        }
    }

    push_section(&mut sections, &mut current);
    sections
}

fn push_section(sections: &mut Vec<Section>, current: &mut Section) {
    let section = std::mem::replace(current, Section::root());
    if section.title.is_some() || !section.content.is_empty() {
        sections.push(section);
    }
}

fn strip_non_content(html: &str) -> String {
    let mut result = html.to_owned();
    // Remove script, style, nav, footer, header tags and their content.
    // Uses to_ascii_lowercase to preserve byte positions for non-ASCII content.
    for tag in &["script", "style", "nav", "footer", "noscript", "svg", "iframe"] {
        let mut iterations = 0;
        loop {
            if iterations >= MAX_STRIP_ITERATIONS {
                break;
            }
            iterations += 1;
            let lower = result.to_ascii_lowercase();
            let open = format!("<{}", tag);
            let close = format!("</{}>", tag);
            if let Some(start) = lower.find(&open) {
                if let Some(end) = lower[start..].find(&close) {
                    result =
                        format!("{}{}", &result[..start], &result[start + end + close.len()..]);
                    continue;
                }
            }
            break;
        }
    }
    // Remove HTML comments
    let mut iterations = 0;
    loop {
        if iterations >= MAX_STRIP_ITERATIONS {
            break;
        }
        iterations += 1;
        if let Some(start) = result.find("<!--") {
            if let Some(end) = result[start..].find("-->") {
                result = format!("{}{}", &result[..start], &result[start + end + 3..]);
                continue;
            }
        }
        break;
    }
    result
}

fn extract_tag_content(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start = lower.find(&open)?;
    let content_start = html[start..].find('>')? + start + 1;
    let end = lower[content_start..].find(&close)? + content_start;
    Some(html[content_start..end].to_owned())
}

fn extract_meta_tags(html: &str, metadata: &mut DocumentMetadata) {
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<meta") {
        let start = pos + start;
        if let Some(end) = lower[start..].find('>') {
            let tag = &html[start..start + end + 1];
            let tag_lower = tag.to_ascii_lowercase();

            let name =
                extract_attr(&tag_lower, "name").or_else(|| extract_attr(&tag_lower, "property"));
            let content = extract_attr(tag, "content");

            if let (Some(name), Some(content)) = (name, content) {
                match name.to_lowercase().as_str() {
                    "author" => metadata.author = Some(content),
                    "description" | "subject" => metadata.subject = Some(content),
                    "keywords" => {
                        metadata.keywords =
                            content.split(',').map(|s| s.trim().to_owned()).collect();
                    },
                    _ => {},
                }
            }
            pos = start + end + 1;
        } else {
            break;
        }
    }
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    // Try double-quoted first, then single-quoted
    let dq_pattern = format!("{}=\"", attr);
    if let Some(dq_start) = tag.find(&dq_pattern) {
        let start = dq_start + dq_pattern.len();
        let end = tag[start..].find('"')? + start;
        return Some(tag[start..end].to_owned());
    }
    let sq_pattern = format!("{}='", attr);
    if let Some(sq_start) = tag.find(&sq_pattern) {
        let start = sq_start + sq_pattern.len();
        let end = tag[start..].find('\'')? + start;
        return Some(tag[start..end].to_owned());
    }
    None
}

fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Collapse whitespace
    let mut prev_space = false;
    let mut collapsed = String::with_capacity(result.len());
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }
    collapsed
}

fn decode_entities(text: &str) -> String {
    let mut result = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", "\u{00A0}")
        .replace("&mdash;", "\u{2014}")
        .replace("&ndash;", "\u{2013}")
        .replace("&hellip;", "\u{2026}")
        .replace("&copy;", "\u{00A9}")
        .replace("&reg;", "\u{00AE}")
        .replace("&trade;", "\u{2122}")
        .replace("&laquo;", "\u{00AB}")
        .replace("&raquo;", "\u{00BB}")
        .replace("&bull;", "\u{2022}")
        .replace("&middot;", "\u{00B7}");

    // Decode numeric character references: &#NNN; and &#xHHHH;
    let mut search_from = 0;
    while let Some(offset) = result[search_from..].find("&#") {
        let start = search_from + offset;
        let rest = &result[start + 2..];
        if let Some(semi) = rest.find(';') {
            let num_str = &rest[..semi];
            let decoded = if let Some(hex) = num_str
                .strip_prefix('x')
                .or_else(|| num_str.strip_prefix('X'))
            {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else {
                num_str.parse::<u32>().ok().and_then(char::from_u32)
            };
            if let Some(ch) = decoded {
                let before = &result[..start];
                let after = &result[start + 2 + semi + 1..];
                result = format!("{before}{ch}{after}");
                // Continue from the position after the decoded character
                search_from = start + ch.len_utf8();
                continue;
            }
        }
        // Can't decode this entity, skip past "&#" and continue looking
        search_from = start + 2;
    }

    result
}

/// Find the position of a matching closing tag, accounting for nesting depth.
/// Returns the byte position of the start of the closing tag, or None.
fn find_matching_close_tag(html: &str, tag: &str) -> Option<usize> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let mut depth = 1usize;
    let mut pos = 0;
    while pos < lower.len() {
        let next_open = lower[pos..].find(&open);
        let next_close = lower[pos..].find(&close);
        match (next_open, next_close) {
            (_, None) => return None,
            (None, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos + c);
                }
                pos += c + close.len();
            },
            (Some(o), Some(c)) if o < c => {
                // Check that the open tag is actually an opening tag (not a substring)
                let after_open = pos + o + open.len();
                if after_open < lower.len() {
                    let next_char = lower.as_bytes()[after_open];
                    if next_char == b'>' || next_char == b' ' || next_char == b'\n' {
                        depth += 1;
                    }
                }
                pos += o + 1;
            },
            (Some(_), Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos + c);
                }
                pos += c + close.len();
            },
        }
    }
    None
}

fn extract_list_items(html: &str) -> Vec<ListItem> {
    let mut items = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<li") {
        let start = pos + start;
        if let Some(content_start) = html[start..].find('>') {
            let content_start = start + content_start + 1;
            // Find closing </li> or next <li
            let end = lower[content_start..]
                .find("</li>")
                .map_or(html.len(), |e| content_start + e);
            let text = strip_tags(&html[content_start..end]);
            let decoded = decode_entities(&text);
            if !decoded.trim().is_empty() {
                items.push(ListItem { text: decoded.trim().to_owned(), children: None });
            }
            pos = (end + 5).min(html.len());
        } else {
            break;
        }
    }
    items
}

fn extract_table(html: &str) -> Option<Table> {
    let lower = html.to_ascii_lowercase();
    let mut headers = Vec::new();
    let mut rows = Vec::new();

    // Extract headers from <th> tags
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<th") {
        let start = pos + start;
        if let Some(content_start) = html[start..].find('>') {
            let content_start = start + content_start + 1;
            let end = lower[content_start..]
                .find("</th>")
                .map_or(html.len(), |e| content_start + e);
            let text = strip_tags(&html[content_start..end]);
            headers.push(decode_entities(&text).trim().to_owned());
            pos = (end + 5).min(html.len());
        } else {
            break;
        }
    }

    // Extract rows from <tr> tags containing <td>
    pos = 0;
    while let Some(tr_start) = lower[pos..].find("<tr") {
        let tr_start = pos + tr_start;
        let tr_end = lower[tr_start..]
            .find("</tr>")
            .map_or(html.len(), |e| tr_start + e);
        let tr_html = &html[tr_start..tr_end];
        let tr_lower = tr_html.to_ascii_lowercase();

        // Only process rows with <td> (skip header rows)
        if tr_lower.contains("<td") {
            let mut cells = Vec::new();
            let mut td_pos = 0;
            while let Some(td_start) = tr_lower[td_pos..].find("<td") {
                let td_start = td_pos + td_start;
                if let Some(content_start) = tr_html[td_start..].find('>') {
                    let content_start = td_start + content_start + 1;
                    let td_end = tr_lower[content_start..]
                        .find("</td>")
                        .map_or(tr_html.len(), |e| content_start + e);
                    let text = strip_tags(&tr_html[content_start..td_end]);
                    cells.push(decode_entities(&text).trim().to_owned());
                    td_pos = (td_end + 5).min(tr_html.len());
                } else {
                    break;
                }
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
        pos = (tr_end + 5).min(html.len());
    }

    if headers.is_empty() && rows.is_empty() {
        return None;
    }

    Some(Table { caption: None, headers, rows, alignments: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;

    #[test]
    fn test_basic_html() {
        let html =
            "<html><head><title>Test</title></head><body><h1>Hello</h1><p>World</p></body></html>";
        let doc = parse(html, &ParseOptions::default()).unwrap();
        assert_eq!(doc.title.as_deref(), Some("Test"));
        assert!(!doc.sections.is_empty());
    }

    #[test]
    fn test_html_list() {
        let html = "<ul><li>A</li><li>B</li><li>C</li></ul>";
        let doc = parse(html, &ParseOptions::default()).unwrap();
        let has_list = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .any(|b| matches!(b, ContentBlock::List(l) if l.items.len() == 3));
        assert!(has_list);
    }

    #[test]
    fn test_html_table() {
        let html = "<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>";
        let doc = parse(html, &ParseOptions::default()).unwrap();
        let has_table = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .any(|b| matches!(b, ContentBlock::Table(_)));
        assert!(has_table);
    }

    #[test]
    fn test_strip_scripts() {
        let html = "<p>Before</p><script>alert('xss')</script><p>After</p>";
        let doc = parse(html, &ParseOptions::default()).unwrap();
        let text = doc.full_text();
        assert!(!text.contains("alert"));
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_entities("&amp; &lt; &gt;"), "& < >");
    }

    #[test]
    fn test_decode_entities_numeric() {
        // Basic numeric entity
        assert_eq!(decode_entities("&#65;"), "A");
        // Hex entity
        assert_eq!(decode_entities("&#x41;"), "A");
        // Malformed entity should not prevent decoding subsequent ones
        assert_eq!(decode_entities("&#invalid; and &#169;"), "&#invalid; and \u{00A9}");
    }

    #[test]
    fn test_nested_list() {
        let html = "<ul><li>A<ul><li>B</li></ul></li><li>C</li></ul>";
        let doc = parse(html, &ParseOptions::default()).unwrap();
        let text = doc.full_text();
        assert!(text.contains("A"));
        assert!(text.contains("C"));
    }
}
