//! Stage 3: Language compression — remove filler phrases and tighten language.

use super::patterns::FILLER_PATTERNS;
use crate::document::types::*;

/// Compress filler phrases in all text content of a document.
pub fn compress_document(doc: &mut Document) {
    for section in &mut doc.sections {
        compress_section(section);
    }
}

fn compress_section(section: &mut Section) {
    for block in &mut section.content {
        compress_block(block);
    }
    for child in &mut section.children {
        compress_section(child);
    }
}

fn compress_block(block: &mut ContentBlock) {
    match block {
        ContentBlock::Paragraph(text) => {
            *text = compress_text(text);
        },
        ContentBlock::Blockquote(text) => {
            *text = compress_text(text);
        },
        ContentBlock::List(list) => {
            for item in &mut list.items {
                item.text = compress_text(&item.text);
            }
        },
        ContentBlock::Definition(def) => {
            def.definition = compress_text(&def.definition);
        },
        _ => {},
    }
}

/// Apply filler pattern replacement to a text string.
pub fn compress_text(text: &str) -> String {
    let mut result = text.to_owned();

    for &(pattern, replacement) in FILLER_PATTERNS {
        // Case-insensitive replacement of ALL occurrences
        loop {
            let lower = result.to_lowercase();
            let pat_lower = pattern;
            if let Some(pos) = lower.find(pat_lower) {
                let end_pos = pos + pattern.len();

                // If replacement is empty and pattern was at sentence start,
                // capitalize the next word
                let new_result = if replacement.is_empty() {
                    let before = &result[..pos];
                    let after = &result[end_pos..];
                    let after = capitalize_first_alpha(after);
                    format!("{before}{after}")
                } else {
                    let before = &result[..pos];
                    let after = &result[end_pos..];
                    // Preserve original casing for the first character if at sentence start
                    let rep = if pos == 0
                        || result[..pos].ends_with(". ")
                        || result[..pos].ends_with(".\n")
                    {
                        capitalize_first(replacement)
                    } else {
                        replacement.to_owned()
                    };
                    format!("{before}{rep}{after}")
                };

                result = new_result;
            } else {
                break;
            }
        }
    }

    // Clean up double spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }

    result.trim().to_owned()
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn capitalize_first_alpha(s: &str) -> String {
    let mut chars = s.char_indices();
    for (i, ch) in chars.by_ref() {
        if ch.is_alphabetic() {
            let upper: String = ch.to_uppercase().collect();
            return format!("{}{}{}", &s[..i], upper, &s[i + ch.len_utf8()..]);
        }
    }
    s.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_filler() {
        let text = "It is important to note that the system requires MFA.";
        let result = compress_text(text);
        assert_eq!(result, "The system requires MFA.");
    }

    #[test]
    fn test_compress_verbose() {
        let text = "In order to access the system, users must authenticate.";
        let result = compress_text(text);
        assert_eq!(result, "To access the system, users must authenticate.");
    }

    #[test]
    fn test_compress_hedging() {
        let text = "It may be possible that the system is vulnerable.";
        let result = compress_text(text);
        assert!(result.contains("Possibly,") || result.contains("possibly,"));
    }

    #[test]
    fn test_compress_preserves_content() {
        let text = "All users MUST authenticate using MFA before accessing production.";
        let result = compress_text(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_compress_document() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut section = Section::new(1, "Test");
        section.content.push(ContentBlock::Paragraph(
            "Due to the fact that the system is complex, we need testing.".into(),
        ));
        doc.sections.push(section);

        compress_document(&mut doc);

        let text = doc.full_text();
        assert!(text.contains("Because"));
        assert!(!text.contains("Due to the fact that"));
    }
}
