//! Plain text document parser with heuristic structure detection.
//!
//! Detects headings via:
//! - ALL CAPS lines followed by blank lines
//! - Lines followed by === or --- underlines
//! - Numbered section patterns (1. Title, 1.1 Subtitle)
//! - Lines significantly shorter than surrounding paragraphs

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse plain text content into a Document with detected structure.
pub fn parse(content: &str, _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let mut doc = Document::new("", DocumentFormat::PlainText);
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Ok(doc);
    }

    let mut sections: Vec<Section> = Vec::new();
    let mut current_section = Section::root();
    let mut para_buf = String::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Blank line: flush paragraph
        if line.trim().is_empty() {
            flush_para(&mut para_buf, &mut current_section);
            i += 1;
            continue;
        }

        // Underline heading: next line is === or ---
        if i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if !line.trim().is_empty() && is_underline(next, '=') {
                flush_para(&mut para_buf, &mut current_section);
                push_section(&mut sections, &mut current_section);
                current_section = Section::new(1, line.trim());
                i += 2;
                continue;
            }
            if !line.trim().is_empty() && is_underline(next, '-') && next.len() >= 3 {
                flush_para(&mut para_buf, &mut current_section);
                push_section(&mut sections, &mut current_section);
                current_section = Section::new(2, line.trim());
                i += 2;
                continue;
            }
        }

        // ALL CAPS heading (must be followed by blank line or EOF)
        let trimmed = line.trim();
        if is_all_caps_heading(trimmed) && (i + 1 >= lines.len() || lines[i + 1].trim().is_empty())
        {
            flush_para(&mut para_buf, &mut current_section);
            push_section(&mut sections, &mut current_section);
            current_section = Section::new(1, trimmed);
            i += 1;
            continue;
        }

        // Numbered section heading: "1." or "1.2" or "1.2.3" at start
        if let Some((level, title)) = parse_numbered_heading(trimmed) {
            if i + 1 >= lines.len() || lines[i + 1].trim().is_empty() {
                flush_para(&mut para_buf, &mut current_section);
                push_section(&mut sections, &mut current_section);
                let mut section = Section::new(level, title);
                // Extract the number part
                if let Some(num_end) = trimmed.find(|c: char| c.is_alphabetic()) {
                    section.number = Some(
                        trimmed[..num_end]
                            .trim()
                            .trim_end_matches('.')
                            .trim_end_matches(' ')
                            .to_owned(),
                    );
                }
                current_section = section;
                i += 1;
                continue;
            }
        }

        // Bullet list detection
        if is_bullet_line(trimmed) {
            flush_para(&mut para_buf, &mut current_section);
            let mut items = Vec::new();
            let ordered = trimmed.chars().next().map_or(false, |c| c.is_ascii_digit());
            while i < lines.len() {
                let l = lines[i];
                let l_trimmed = l.trim();
                if l_trimmed.is_empty() {
                    break;
                }
                if let Some(text) = strip_bullet(l_trimmed) {
                    items.push(ListItem { text: text.to_owned(), children: None });
                } else if l.starts_with("  ") || l.starts_with('\t') {
                    // Continuation of previous list item (indented line)
                    if let Some(last) = items.last_mut() {
                        last.text.push(' ');
                        last.text.push_str(l_trimmed);
                    }
                } else {
                    break;
                }
                i += 1;
            }
            if !items.is_empty() {
                current_section
                    .content
                    .push(ContentBlock::List(List { ordered, items }));
            }
            continue;
        }

        // Regular paragraph text
        if !para_buf.is_empty() {
            para_buf.push(' ');
        }
        para_buf.push_str(trimmed);
        i += 1;
    }

    flush_para(&mut para_buf, &mut current_section);
    push_section(&mut sections, &mut current_section);

    doc.sections = sections;
    Ok(doc)
}

fn flush_para(buf: &mut String, section: &mut Section) {
    if !buf.is_empty() {
        section
            .content
            .push(ContentBlock::Paragraph(std::mem::take(buf)));
    }
}

fn push_section(sections: &mut Vec<Section>, current: &mut Section) {
    let section = std::mem::replace(current, Section::root());
    if section.title.is_some() || !section.content.is_empty() {
        sections.push(section);
    }
}

fn is_underline(line: &str, ch: char) -> bool {
    !line.is_empty() && line.len() >= 3 && line.chars().all(|c| c == ch)
}

fn is_all_caps_heading(line: &str) -> bool {
    if line.len() < 3 || line.len() > 120 {
        return false;
    }
    let alpha_count = line.chars().filter(|c| c.is_alphabetic()).count();
    if alpha_count < 2 {
        return false;
    }
    line.chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase())
}

fn parse_numbered_heading(line: &str) -> Option<(u8, String)> {
    // Match patterns like "1. Title", "1.2 Title", "1.2.3 Title"
    let mut chars = line.chars().peekable();
    let first = chars.peek()?;
    if !first.is_ascii_digit() {
        return None;
    }

    let mut dots = 0;
    let mut num_end = 0;
    for (i, ch) in line.char_indices() {
        if ch.is_ascii_digit() {
            num_end = i + 1;
        } else if ch == '.' {
            dots += 1;
            num_end = i + 1;
        } else if ch == ' ' {
            num_end = i;
            break;
        } else {
            return None;
        }
    }

    let rest = line[num_end..].trim();
    if rest.is_empty() || rest.len() > 120 {
        return None;
    }

    // Must have at least one alpha char in the title
    if !rest.chars().any(|c| c.is_alphabetic()) {
        return None;
    }

    let level = (dots + 1).min(6) as u8;
    Some((level, rest.to_owned()))
}

fn is_bullet_line(line: &str) -> bool {
    strip_bullet(line).is_some()
}

fn strip_bullet(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("• "))
    {
        return Some(rest);
    }
    // Ordered: "1. text", "1) text", or multi-digit "10. text"
    let first = trimmed.chars().next()?;
    if first.is_ascii_digit() {
        let digit_end = trimmed
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(trimmed.len());
        if digit_end > 0 {
            let after = &trimmed[digit_end..];
            if let Some(text) = after
                .strip_prefix(". ")
                .or_else(|| after.strip_prefix(") "))
            {
                return Some(text);
            }
        }
    }
    // "a) text" pattern
    if first.is_ascii_lowercase() && trimmed.len() > 2 {
        if let Some(text) = trimmed[1..].strip_prefix(") ") {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;

    #[test]
    fn test_all_caps_heading() {
        let text = "INTRODUCTION\n\nThis is the body text.\n\nCONCLUSION\n\nFinal text.";
        let doc = parse(text, &ParseOptions::default()).unwrap();
        assert!(doc.section_count() >= 2);
    }

    #[test]
    fn test_underline_heading() {
        let text = "Title\n=====\n\nBody text.\n\nSubtitle\n--------\n\nMore text.";
        let doc = parse(text, &ParseOptions::default()).unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].level, 1);
        assert_eq!(doc.sections[1].level, 2);
    }

    #[test]
    fn test_numbered_sections() {
        let text = "1. Introduction\n\nSome text.\n\n2. Methods\n\nMore text.";
        let doc = parse(text, &ParseOptions::default()).unwrap();
        assert!(doc.section_count() >= 2);
    }

    #[test]
    fn test_bullet_list() {
        let text = "Items:\n\n- Apple\n- Banana\n- Cherry\n";
        let doc = parse(text, &ParseOptions::default()).unwrap();
        let has_list = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .any(|b| matches!(b, ContentBlock::List(_)));
        assert!(has_list);
    }

    #[test]
    fn test_plain_paragraphs() {
        let text = "First paragraph text.\n\nSecond paragraph text.";
        let doc = parse(text, &ParseOptions::default()).unwrap();
        let para_count = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .filter(|b| matches!(b, ContentBlock::Paragraph(_)))
            .count();
        assert_eq!(para_count, 2);
    }

    #[test]
    fn test_is_all_caps() {
        assert!(is_all_caps_heading("INTRODUCTION"));
        assert!(is_all_caps_heading("SECTION ONE"));
        assert!(!is_all_caps_heading("Introduction"));
        assert!(!is_all_caps_heading("A")); // too short
        assert!(!is_all_caps_heading("123")); // no alpha
    }
}
