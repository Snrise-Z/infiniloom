//! Stage 1 (Strip) and Stage 2 (Deduplicate) of the distillation pipeline.
//!
//! Strip removes zero-value content: page numbers, running headers, copyright,
//! watermarks, decorative separators, and empty sections.
//!
//! Deduplicate removes redundant content: TOC when body follows, near-duplicate
//! paragraphs, and repeated boilerplate text.

use super::patterns::BOILERPLATE_PATTERNS;
use crate::document::types::*;

/// Stage 1: Strip zero-value content from a document.
pub fn strip_document(doc: &mut Document) {
    for section in &mut doc.sections {
        strip_section(section);
    }
    // Remove empty sections
    doc.sections.retain(|s| !is_section_empty(s));
}

fn strip_section(section: &mut Section) {
    section.content.retain(|block| !is_zero_value(block));

    for child in &mut section.children {
        strip_section(child);
    }
    section.children.retain(|s| !is_section_empty(s));
}

fn is_section_empty(section: &Section) -> bool {
    section.content.is_empty() && section.children.is_empty()
}

fn is_zero_value(block: &ContentBlock) -> bool {
    match block {
        ContentBlock::Paragraph(text) | ContentBlock::Raw(text) => {
            let lower = text.to_lowercase();
            let trimmed = lower.trim();

            // Empty or whitespace only
            if trimmed.is_empty() {
                return true;
            }

            // Page numbers: "Page 1 of 5", "- 3 -", "1", etc.
            if is_page_number(trimmed) {
                return true;
            }

            // Very short lines that are likely structural noise
            if trimmed.len() < 5 && !trimmed.chars().any(|c| c.is_alphabetic()) {
                return true;
            }

            // Boilerplate patterns
            if BOILERPLATE_PATTERNS.iter().any(|p| lower.contains(p)) {
                return true;
            }

            false
        },
        ContentBlock::ThematicBreak => false, // Keep thematic breaks (they have structural meaning)
        _ => false,
    }
}

fn is_page_number(text: &str) -> bool {
    let trimmed = text.trim_matches(|c: char| c == '-' || c == ' ' || c == '–' || c == '—');
    // Pure number surrounded by dashes/spaces (e.g., "- 42 -", "-- 3 --")
    // Require surrounding dashes to distinguish from plain numbers like "2024"
    let has_dash_wrapper = text.trim() != trimmed;
    if has_dash_wrapper
        && trimmed.chars().all(|c| c.is_ascii_digit())
        && !trimmed.is_empty()
        && trimmed.len() <= 4
    {
        return true;
    }
    // "Page X" or "Page X of Y"
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("page ") {
        return true;
    }
    // "X / Y" pattern (e.g., "3 / 10")
    let parts: Vec<&str> = text.split('/').map(|s| s.trim()).collect();
    if parts.len() == 2
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

/// Stage 2: Remove redundant/duplicate content.
pub fn deduplicate(doc: &mut Document) {
    // Collect paragraph hashes to detect duplicates
    let mut seen_hashes = std::collections::HashSet::new();

    for section in &mut doc.sections {
        dedup_section(section, &mut seen_hashes);
    }
}

fn dedup_section(section: &mut Section, seen: &mut std::collections::HashSet<u64>) {
    section.content.retain(|block| {
        if let ContentBlock::Paragraph(text) = block {
            let normalized = normalize_for_dedup(text);
            if normalized.len() < 20 {
                // Don't dedup very short paragraphs (likely unique)
                return true;
            }
            let hash = simple_hash(&normalized);
            seen.insert(hash)
        } else {
            true
        }
    });

    for child in &mut section.children {
        dedup_section(child, seen);
    }
}

fn normalize_for_dedup(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn simple_hash(text: &str) -> u64 {
    // FNV-1a hash for fast, good-enough deduplication
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_page_numbers() {
        assert!(is_page_number("page 1 of 5"));
        assert!(is_page_number("Page 3"));
        assert!(is_page_number("- 3 -"));
        assert!(is_page_number("-- 42 --"));
        assert!(is_page_number("3 / 10"));
        // Bare numbers should NOT be detected as page numbers (avoids false positives on years, IDs)
        assert!(!is_page_number("42"));
        assert!(!is_page_number("2024"));
        assert!(!is_page_number("100"));
        assert!(!is_page_number("This is a sentence."));
    }

    #[test]
    fn test_strip_boilerplate() {
        assert!(is_zero_value(&ContentBlock::Paragraph(
            "Copyright (c) 2024 Acme Corp. All rights reserved.".into()
        )));
        assert!(is_zero_value(&ContentBlock::Paragraph(
            "This document is confidential and proprietary.".into()
        )));
    }

    #[test]
    fn test_keep_real_content() {
        assert!(!is_zero_value(&ContentBlock::Paragraph(
            "All users must authenticate using multi-factor authentication.".into()
        )));
    }

    #[test]
    fn test_deduplicate() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut s1 = Section::new(1, "Section 1");
        s1.content.push(ContentBlock::Paragraph(
            "This is a paragraph that appears multiple times in the document.".into(),
        ));
        let mut s2 = Section::new(1, "Section 2");
        s2.content.push(ContentBlock::Paragraph(
            "This is a paragraph that appears multiple times in the document.".into(),
        ));
        s2.content
            .push(ContentBlock::Paragraph("This is unique content that should be kept.".into()));
        doc.sections.push(s1);
        doc.sections.push(s2);

        deduplicate(&mut doc);

        // First occurrence kept, second removed
        let total_paras: usize = doc
            .sections
            .iter()
            .map(|s| {
                s.content
                    .iter()
                    .filter(|b| matches!(b, ContentBlock::Paragraph(_)))
                    .count()
            })
            .sum();
        assert_eq!(total_paras, 2); // original + unique, duplicate removed
    }
}
