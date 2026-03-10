//! Stage 4: Information density scoring.
//!
//! Scores each section by information value using heuristics:
//! - Requirement keywords (SHALL, MUST) → high score
//! - Definitions → high score
//! - Data content (numbers, percentages, dates) → high score
//! - Tables → high score
//! - Generic narrative → medium score
//! - Short transitional paragraphs → low score

use super::patterns::{INFORMATIVE_KEYWORDS, REQUIREMENT_KEYWORDS};
use crate::document::types::*;

/// Score all sections in a document by information density.
pub fn score_document(doc: &mut Document) {
    for section in &mut doc.sections {
        score_section(section);
    }
}

fn score_section(section: &mut Section) {
    let mut total_score: f32 = 0.0;
    let mut block_count: f32 = 0.0;

    for block in &section.content {
        total_score += score_block(block);
        block_count += 1.0;
    }

    // Normalize: average score per block first
    let avg_score = if block_count > 0.0 {
        total_score / block_count
    } else {
        0.3 // Empty sections get low default
    };

    // Title-based scoring — applied as bounded adjustments to the average
    let mut title_adjustment: f32 = 0.0;
    if let Some(title) = &section.title {
        let lower = title.to_lowercase();
        // Definitions/glossary sections are high value
        if lower.contains("definition") || lower.contains("glossary") || lower.contains("terms") {
            title_adjustment += 0.2;
        }
        // Requirements sections are high value
        if lower.contains("requirement")
            || lower.contains("obligation")
            || lower.contains("control")
        {
            title_adjustment += 0.2;
        }
        // Introduction/background sections are lower value
        if lower.contains("introduction")
            || lower.contains("background")
            || lower.contains("overview")
            || lower.contains("purpose")
        {
            title_adjustment -= 0.15;
        }
    }

    section.importance = (avg_score + title_adjustment).clamp(0.0, 1.0);

    // Recursively score children
    for child in &mut section.children {
        score_section(child);
    }
}

fn score_block(block: &ContentBlock) -> f32 {
    match block {
        ContentBlock::Table(_) => 0.9, // Tables are almost always high-value structured data
        ContentBlock::Definition(_) => 0.85, // Definitions are high-value
        ContentBlock::CodeBlock(_) => 0.7, // Code/config snippets are usually important
        ContentBlock::List(list) => {
            // Lists with requirement keywords are high value
            let has_requirements = list.items.iter().any(|item| {
                let lower = item.text.to_lowercase();
                REQUIREMENT_KEYWORDS.iter().any(|kw| lower.contains(kw))
            });
            if has_requirements {
                0.85
            } else {
                0.6
            }
        },
        ContentBlock::Paragraph(text) => score_paragraph(text),
        ContentBlock::Blockquote(text) => {
            // Blockquotes are often important callouts
            score_paragraph(text) + 0.1
        },
        ContentBlock::CrossReference(_) => 0.5,
        ContentBlock::ThematicBreak | ContentBlock::Raw(_) => 0.2,
    }
}

fn score_paragraph(text: &str) -> f32 {
    let lower = text.to_lowercase();
    let mut score: f32 = 0.5; // baseline

    // Requirement keywords boost score significantly
    let req_count = REQUIREMENT_KEYWORDS
        .iter()
        .filter(|kw| lower.contains(*kw))
        .count();
    score += req_count as f32 * 0.15;

    // Informative keywords slightly reduce score
    let info_count = INFORMATIVE_KEYWORDS
        .iter()
        .filter(|kw| lower.contains(*kw))
        .count();
    score -= info_count as f32 * 0.05;

    // Data density: numbers, percentages, dates boost score
    let data_tokens = text
        .split_whitespace()
        .filter(|w| {
            w.contains('%')
                || w.parse::<f64>().is_ok()
                || w.contains('/')
                    && w.len() <= 10
                    && w.chars().filter(|c| c.is_ascii_digit()).count() >= 2
        })
        .count();
    let total_tokens = text.split_whitespace().count().max(1);
    let data_density = data_tokens as f32 / total_tokens as f32;
    score += data_density * 0.3;

    // Very short paragraphs are often transitional
    if text.len() < 50 {
        score -= 0.15;
    }

    // Very long paragraphs may contain important detail
    if text.len() > 300 {
        score += 0.05;
    }

    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_requirement() {
        let score = score_paragraph("All users SHALL authenticate before accessing the system.");
        assert!(score > 0.6);
    }

    #[test]
    fn test_score_data() {
        let score = score_paragraph(
            "The system threshold is 99.9% uptime with a 15 minute RTO and 4 hour RPO target.",
        );
        assert!(score > 0.5);
    }

    #[test]
    fn test_score_filler() {
        let score = score_paragraph("See below for more details.");
        assert!(score < 0.5);
    }

    #[test]
    fn test_table_high_score() {
        let score = score_block(&ContentBlock::Table(Table {
            caption: None,
            headers: vec!["A".into()],
            rows: vec![vec!["1".into()]],
            alignments: vec![],
        }));
        assert!(score > 0.8);
    }

    #[test]
    fn test_score_document_sets_importance() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut req = Section::new(1, "Requirements");
        req.content
            .push(ContentBlock::Paragraph("All systems MUST implement encryption at rest.".into()));
        let mut intro = Section::new(1, "Introduction");
        intro
            .content
            .push(ContentBlock::Paragraph("This document provides an overview.".into()));
        doc.sections.push(req);
        doc.sections.push(intro);

        score_document(&mut doc);

        assert!(doc.sections[0].importance > doc.sections[1].importance);
    }
}
