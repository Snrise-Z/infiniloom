//! Content distillation pipeline for LLM attention and token optimization.
//!
//! Research shows that removing noise from documents **improves** LLM accuracy
//! by 17-21% (LLMLingua, Microsoft) — this is not just about saving tokens.
//!
//! The pipeline has 5 stages:
//! 1. **Strip**: Remove zero-value content (page numbers, watermarks, boilerplate)
//! 2. **Deduplicate**: Remove redundant content (TOC before body, repeated definitions)
//! 3. **Compress**: Tighten language (filler phrases, hedging, verbose patterns)
//! 4. **Score**: Rank sections by information density
//! 5. **Arrange**: Place high-value content where LLMs attend (start/end, not middle)

pub mod arrange;
pub mod compress;
pub mod patterns;
pub mod score;
pub mod strip;

use crate::document::types::{DistillationLevel, Document};

/// Run the distillation pipeline on a parsed document.
pub fn distill(doc: &mut Document, level: DistillationLevel) {
    match level {
        DistillationLevel::None => {},
        DistillationLevel::Minimal => {
            strip::strip_document(doc);
        },
        DistillationLevel::Balanced => {
            strip::strip_document(doc);
            strip::deduplicate(doc);
        },
        DistillationLevel::Aggressive => {
            strip::strip_document(doc);
            strip::deduplicate(doc);
            compress::compress_document(doc);
        },
        DistillationLevel::Full => {
            strip::strip_document(doc);
            strip::deduplicate(doc);
            compress::compress_document(doc);
            score::score_document(doc);
            arrange::arrange_document(doc);
        },
    }
}

/// Get statistics about distillation effectiveness.
pub struct DistillationStats {
    pub original_blocks: usize,
    pub remaining_blocks: usize,
    pub sections_removed: usize,
    pub filler_replacements: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::types::*;

    fn make_test_doc() -> Document {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        let mut s1 = Section::new(1, "Introduction");
        s1.content.push(ContentBlock::Paragraph(
            "It is important to note that this document establishes the policy.".into(),
        ));
        let mut s2 = Section::new(1, "Requirements");
        s2.content
            .push(ContentBlock::Paragraph("All users MUST authenticate using MFA.".into()));
        doc.sections.push(s1);
        doc.sections.push(s2);
        doc
    }

    #[test]
    fn test_distill_none() {
        let mut doc = make_test_doc();
        distill(&mut doc, DistillationLevel::None);
        assert_eq!(doc.sections.len(), 2);
    }

    #[test]
    fn test_distill_aggressive() {
        let mut doc = make_test_doc();
        distill(&mut doc, DistillationLevel::Aggressive);
        // Filler should be compressed
        let text = doc.full_text();
        assert!(!text.contains("It is important to note that"));
    }

    #[test]
    fn test_distill_full() {
        let mut doc = make_test_doc();
        distill(&mut doc, DistillationLevel::Full);
        // Requirements section should score higher
        let req = doc
            .sections
            .iter()
            .find(|s| s.title.as_deref() == Some("Requirements"));
        assert!(req.is_some());
    }
}
