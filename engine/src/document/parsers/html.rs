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
    let mut lower = result.to_ascii_lowercase();
    // Remove script, style, nav, footer, header tags and their content.
    for tag in &["script", "style", "nav", "footer", "noscript", "svg", "iframe"] {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        let mut iterations = 0;
        loop {
            if iterations >= MAX_STRIP_ITERATIONS {
                break;
            }
            iterations += 1;
            if let Some(start) = lower.find(&open) {
                if let Some(end) = lower[start..].find(&close) {
                    let remove_end = start + end + close.len();
                    let new_len = result.len() - (end + close.len());
                    let mut new_result = String::with_capacity(start + new_len);
                    new_result.push_str(&result[..start]);
                    new_result.push_str(&result[remove_end..]);
                    let mut new_lower = String::with_capacity(start + new_len);
                    new_lower.push_str(&lower[..start]);
                    new_lower.push_str(&lower[remove_end..]);
                    result = new_result;
                    lower = new_lower;
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
                let mut new_result = String::with_capacity(result.len() - (end + 3));
                new_result.push_str(&result[..start]);
                new_result.push_str(&result[start + end + 3..]);
                result = new_result;
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
    let result = text
        // XML predefined entities
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        // Whitespace / formatting
        .replace("&nbsp;", "\u{00A0}")
        .replace("&ensp;", "\u{2002}")
        .replace("&emsp;", "\u{2003}")
        .replace("&thinsp;", "\u{2009}")
        .replace("&shy;", "\u{00AD}")
        .replace("&zwj;", "\u{200D}")
        .replace("&zwnj;", "\u{200C}")
        .replace("&lrm;", "\u{200E}")
        .replace("&rlm;", "\u{200F}")
        // Dashes and hyphens
        .replace("&mdash;", "\u{2014}")
        .replace("&ndash;", "\u{2013}")
        .replace("&minus;", "\u{2212}")
        // Quotation marks
        .replace("&lsquo;", "\u{2018}")
        .replace("&rsquo;", "\u{2019}")
        .replace("&sbquo;", "\u{201A}")
        .replace("&ldquo;", "\u{201C}")
        .replace("&rdquo;", "\u{201D}")
        .replace("&bdquo;", "\u{201E}")
        .replace("&laquo;", "\u{00AB}")
        .replace("&raquo;", "\u{00BB}")
        .replace("&lsaquo;", "\u{2039}")
        .replace("&rsaquo;", "\u{203A}")
        .replace("&prime;", "\u{2032}")
        .replace("&Prime;", "\u{2033}")
        // Punctuation and symbols
        .replace("&hellip;", "\u{2026}")
        .replace("&bull;", "\u{2022}")
        .replace("&middot;", "\u{00B7}")
        .replace("&iexcl;", "\u{00A1}")
        .replace("&iquest;", "\u{00BF}")
        .replace("&sect;", "\u{00A7}")
        .replace("&para;", "\u{00B6}")
        .replace("&dagger;", "\u{2020}")
        .replace("&Dagger;", "\u{2021}")
        // Intellectual property
        .replace("&copy;", "\u{00A9}")
        .replace("&reg;", "\u{00AE}")
        .replace("&trade;", "\u{2122}")
        // Currency
        .replace("&cent;", "\u{00A2}")
        .replace("&pound;", "\u{00A3}")
        .replace("&yen;", "\u{00A5}")
        .replace("&euro;", "\u{20AC}")
        .replace("&curren;", "\u{00A4}")
        // Math and technical
        .replace("&times;", "\u{00D7}")
        .replace("&divide;", "\u{00F7}")
        .replace("&plusmn;", "\u{00B1}")
        .replace("&deg;", "\u{00B0}")
        .replace("&micro;", "\u{00B5}")
        .replace("&frac12;", "\u{00BD}")
        .replace("&frac14;", "\u{00BC}")
        .replace("&frac34;", "\u{00BE}")
        .replace("&sup1;", "\u{00B9}")
        .replace("&sup2;", "\u{00B2}")
        .replace("&sup3;", "\u{00B3}")
        .replace("&not;", "\u{00AC}")
        .replace("&macr;", "\u{00AF}")
        // Arrows
        .replace("&larr;", "\u{2190}")
        .replace("&uarr;", "\u{2191}")
        .replace("&rarr;", "\u{2192}")
        .replace("&darr;", "\u{2193}")
        .replace("&harr;", "\u{2194}")
        // Common Latin characters
        .replace("&Agrave;", "\u{00C0}")
        .replace("&Aacute;", "\u{00C1}")
        .replace("&Acirc;", "\u{00C2}")
        .replace("&Atilde;", "\u{00C3}")
        .replace("&Auml;", "\u{00C4}")
        .replace("&Aring;", "\u{00C5}")
        .replace("&AElig;", "\u{00C6}")
        .replace("&Ccedil;", "\u{00C7}")
        .replace("&Egrave;", "\u{00C8}")
        .replace("&Eacute;", "\u{00C9}")
        .replace("&Euml;", "\u{00CB}")
        .replace("&Igrave;", "\u{00CC}")
        .replace("&Iacute;", "\u{00CD}")
        .replace("&Iuml;", "\u{00CF}")
        .replace("&Ntilde;", "\u{00D1}")
        .replace("&Ograve;", "\u{00D2}")
        .replace("&Oacute;", "\u{00D3}")
        .replace("&Ouml;", "\u{00D6}")
        .replace("&Oslash;", "\u{00D8}")
        .replace("&Ugrave;", "\u{00D9}")
        .replace("&Uacute;", "\u{00DA}")
        .replace("&Uuml;", "\u{00DC}")
        .replace("&szlig;", "\u{00DF}")
        .replace("&agrave;", "\u{00E0}")
        .replace("&aacute;", "\u{00E1}")
        .replace("&acirc;", "\u{00E2}")
        .replace("&atilde;", "\u{00E3}")
        .replace("&auml;", "\u{00E4}")
        .replace("&aring;", "\u{00E5}")
        .replace("&aelig;", "\u{00E6}")
        .replace("&ccedil;", "\u{00E7}")
        .replace("&egrave;", "\u{00E8}")
        .replace("&eacute;", "\u{00E9}")
        .replace("&euml;", "\u{00EB}")
        .replace("&igrave;", "\u{00EC}")
        .replace("&iacute;", "\u{00ED}")
        .replace("&iuml;", "\u{00EF}")
        .replace("&ntilde;", "\u{00F1}")
        .replace("&ograve;", "\u{00F2}")
        .replace("&oacute;", "\u{00F3}")
        .replace("&ouml;", "\u{00F6}")
        .replace("&oslash;", "\u{00F8}")
        .replace("&ugrave;", "\u{00F9}")
        .replace("&uacute;", "\u{00FA}")
        .replace("&uuml;", "\u{00FC}")
        .replace("&yuml;", "\u{00FF}");

    // Decode numeric character references: &#NNN; and &#xHHHH;
    // Build result in a single pass to avoid quadratic string concatenation.
    let mut out = String::with_capacity(result.len());
    let mut remaining = result.as_str();
    while let Some(offset) = remaining.find("&#") {
        out.push_str(&remaining[..offset]);
        let rest = &remaining[offset + 2..];
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
                out.push(ch);
                remaining = &rest[semi + 1..];
                continue;
            }
        }
        // Can't decode this entity, emit "&#" literally and continue
        out.push_str("&#");
        remaining = rest;
    }
    out.push_str(remaining);

    out
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

    #[test]
    fn test_decode_entities_currency_in_price_text() {
        let input = "Price: &pound;99.99 or &euro;115 or &yen;15000";
        let result = decode_entities(input);
        assert_eq!(result, "Price: \u{00A3}99.99 or \u{20AC}115 or \u{00A5}15000");
    }

    #[test]
    fn test_decode_entities_smart_quotes_in_prose() {
        let input = "&ldquo;Hello,&rdquo; she said, &lsquo;goodbye.&rsquo;";
        let result = decode_entities(input);
        assert_eq!(result, "\u{201C}Hello,\u{201D} she said, \u{2018}goodbye.\u{2019}");
    }

    #[test]
    fn test_decode_entities_math_notation() {
        let input = "2 &times; 3 &plusmn; 1 = 6 &plusmn; 1";
        let result = decode_entities(input);
        assert_eq!(result, "2 \u{00D7} 3 \u{00B1} 1 = 6 \u{00B1} 1");
    }

    #[test]
    fn test_decode_entities_navigation_arrows() {
        let input = "&larr; Back | Next &rarr;";
        let result = decode_entities(input);
        assert_eq!(result, "\u{2190} Back | Next \u{2192}");
    }

    #[test]
    fn test_decode_entities_accented_multilingual_text() {
        let input = "Caf&eacute; na&iuml;vet&eacute; in Espa&ntilde;a";
        let result = decode_entities(input);
        assert_eq!(result, "Caf\u{00E9} na\u{00EF}vet\u{00E9} in Espa\u{00F1}a");
    }

    #[test]
    fn test_decode_entities_mixed_entities_and_plain_text() {
        // A realistic HTML snippet mixing named entities, numeric entities, and plain text
        let input = "Copyright &copy; 2026 &mdash; Built with &hearts; by Caf&eacute; Co&period; \
                      Price: &pound;5 &amp; &euro;6. Rating: 4&frac12; stars &#x2605;";
        let result = decode_entities(input);
        // &hearts; and &period; are not in the entity table, so they pass through
        assert!(result.contains("Copyright \u{00A9} 2026 \u{2014}"));
        assert!(result.contains("&hearts;"));
        assert!(result.contains("Caf\u{00E9}"));
        assert!(result.contains("&period;"));
        assert!(result.contains("\u{00A3}5 & \u{20AC}6"));
        assert!(result.contains("4\u{00BD} stars \u{2605}"));
    }

    #[test]
    fn test_decode_entities_unknown_entities_pass_through() {
        let input = "&foobar; stays and &unknown; stays too";
        let result = decode_entities(input);
        assert_eq!(result, "&foobar; stays and &unknown; stays too");
    }

    #[test]
    fn test_decode_entities_numeric_alongside_named() {
        // &#8364; is euro (decimal), &#x20AC; is euro (hex) — same as &euro;
        let input = "&euro; and &#8364; and &#x20AC; are all euro";
        let result = decode_entities(input);
        assert_eq!(result, "\u{20AC} and \u{20AC} and \u{20AC} are all euro");
    }
}
