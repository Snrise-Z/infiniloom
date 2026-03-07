//! Stage 5: Attention-optimized content arrangement.
//!
//! Based on the "Lost in the Middle" research (Liu et al., 2023), LLMs exhibit
//! a U-shaped attention curve: high attention at start and end, low in the middle.
//!
//! This stage reorders sections to place high-importance content at the start
//! and end positions, with lower-importance content in the middle.

use crate::document::types::*;

/// Rearrange document sections for optimal LLM attention.
///
/// Strategy:
/// - Sort sections by importance score
/// - Place top-scoring sections at the beginning
/// - Place second-tier sections at the end
/// - Place lowest-scoring sections in the middle
pub fn arrange_document(doc: &mut Document) {
    if doc.sections.len() < 3 {
        // Not enough sections to meaningfully rearrange
        return;
    }

    // Build (index, importance) pairs
    let mut scored: Vec<(usize, f32)> = doc
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.importance))
        .collect();

    // Sort by importance descending
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Partition into three groups
    let total = scored.len();
    let top_count = total.div_ceil(3);
    let bottom_count = total.div_ceil(3);
    let top_indices: Vec<usize> = scored[..top_count].iter().map(|(i, _)| *i).collect();
    let end_indices: Vec<usize> = scored[top_count..top_count + bottom_count]
        .iter()
        .map(|(i, _)| *i)
        .collect();
    let middle_indices: Vec<usize> = scored[top_count + bottom_count..]
        .iter()
        .map(|(i, _)| *i)
        .collect();

    // Reconstruct in attention-optimized order:
    // [high importance] [low importance] [medium importance]
    let original: Vec<Section> = std::mem::take(&mut doc.sections);
    let mut arranged = Vec::with_capacity(total);

    // Start: highest importance (preserving relative order)
    let mut top_sorted = top_indices;
    top_sorted.sort();
    for &i in &top_sorted {
        arranged.push(original[i].clone());
    }

    // Middle: lowest importance
    let mut mid_sorted = middle_indices;
    mid_sorted.sort();
    for &i in &mid_sorted {
        arranged.push(original[i].clone());
    }

    // End: second-highest importance (preserving relative order)
    let mut end_sorted = end_indices;
    end_sorted.sort();
    for &i in &end_sorted {
        arranged.push(original[i].clone());
    }

    doc.sections = arranged;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrange_preserves_small_docs() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        doc.sections.push(Section::new(1, "Only"));
        arrange_document(&mut doc);
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn test_arrange_reorders_by_importance() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);

        let mut s1 = Section::new(1, "Low");
        s1.importance = 0.1;
        let mut s2 = Section::new(1, "High");
        s2.importance = 0.9;
        let mut s3 = Section::new(1, "Medium");
        s3.importance = 0.5;
        let mut s4 = Section::new(1, "VeryHigh");
        s4.importance = 1.0;
        let mut s5 = Section::new(1, "VeryLow");
        s5.importance = 0.05;

        doc.sections = vec![s1, s2, s3, s4, s5];
        arrange_document(&mut doc);

        // First sections should be high importance
        assert!(doc.sections[0].importance >= 0.5);
        // Last sections should also be reasonably important
        let last = doc.sections.last().unwrap();
        assert!(last.importance >= 0.1);
    }

    #[test]
    fn test_arrange_three_sections() {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);

        let mut low = Section::new(1, "Low");
        low.importance = 0.1;
        let mut high = Section::new(1, "High");
        high.importance = 0.9;
        let mut mid = Section::new(1, "Mid");
        mid.importance = 0.5;

        doc.sections = vec![low, high, mid];
        arrange_document(&mut doc);

        // High should be at start
        assert_eq!(doc.sections[0].title.as_deref(), Some("High"));
    }
}
