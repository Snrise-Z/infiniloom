//! Document-specific output formatters for LLM consumption.
//!
//! Formats a Document into XML (Claude), Markdown (GPT), or JSON (agents).

use crate::document::types::*;
use crate::output::escaping;

/// Format a document as Claude-optimized XML.
pub fn format_xml(doc: &Document) -> String {
    let mut out = String::with_capacity(doc.full_text().len() * 2);
    out.push_str("<document>\n");

    // Metadata
    format_xml_metadata(&mut out, doc);

    // Table of contents (compact)
    if doc.section_count() > 2 {
        out.push_str("  <table_of_contents>\n");
        write_toc_xml(&mut out, &doc.sections, 2);
        out.push_str("  </table_of_contents>\n\n");
    }

    // Sections
    for section in &doc.sections {
        write_section_xml(&mut out, section, 1);
    }

    out.push_str("</document>\n");
    out
}

fn format_xml_metadata(out: &mut String, doc: &Document) {
    out.push_str("  <metadata>\n");
    if let Some(title) = &doc.title {
        out.push_str(&format!("    <title>{}</title>\n", escaping::escape_xml_text(title)));
    }
    let m = &doc.metadata;
    if let Some(v) = &m.author {
        out.push_str(&format!("    <author>{}</author>\n", escaping::escape_xml_text(v)));
    }
    if let Some(v) = &m.version {
        out.push_str(&format!("    <version>{}</version>\n", escaping::escape_xml_text(v)));
    }
    if let Some(v) = &m.effective_date {
        out.push_str(&format!(
            "    <effective_date>{}</effective_date>\n",
            escaping::escape_xml_text(v)
        ));
    }
    if let Some(v) = &m.classification {
        out.push_str(&format!(
            "    <classification>{}</classification>\n",
            escaping::escape_xml_text(v)
        ));
    }
    out.push_str(&format!("    <format>{}</format>\n", doc.format.name()));
    out.push_str("  </metadata>\n\n");
}

fn write_toc_xml(out: &mut String, sections: &[Section], depth: usize) {
    let indent = " ".repeat(depth * 2);
    for section in sections {
        if let Some(title) = &section.title {
            let id_attr = section
                .id
                .as_ref()
                .map(|id| format!(" id=\"{}\"", id))
                .unwrap_or_default();
            let num_attr = section
                .number
                .as_ref()
                .map(|n| format!(" number=\"{}\"", n))
                .unwrap_or_default();
            out.push_str(&format!(
                "{indent}<entry level=\"{}\"{id_attr}{num_attr}>{}</entry>\n",
                section.level,
                escaping::escape_xml_text(title)
            ));
        }
        write_toc_xml(out, &section.children, depth + 1);
    }
}

fn write_section_xml(out: &mut String, section: &Section, depth: usize) {
    let indent = " ".repeat(depth * 2);

    let mut attrs = format!("level=\"{}\"", section.level);
    if let Some(id) = &section.id {
        attrs.push_str(&format!(" id=\"{id}\""));
    }
    if let Some(num) = &section.number {
        attrs.push_str(&format!(" number=\"{num}\""));
    }
    if let Some(title) = &section.title {
        attrs.push_str(&format!(" title=\"{}\"", escaping::escape_xml_text(title)));
    }

    out.push_str(&format!("{indent}<section {attrs}>\n"));

    for block in &section.content {
        write_block_xml(out, block, depth + 1);
    }

    for child in &section.children {
        write_section_xml(out, child, depth + 1);
    }

    out.push_str(&format!("{indent}</section>\n"));
}

fn write_block_xml(out: &mut String, block: &ContentBlock, depth: usize) {
    let indent = " ".repeat(depth * 2);
    match block {
        ContentBlock::Paragraph(text) => {
            out.push_str(&format!(
                "{indent}<paragraph>{}</paragraph>\n",
                escaping::escape_xml_text(text)
            ));
        },
        ContentBlock::Table(table) => {
            out.push_str(&format!("{indent}<table"));
            if let Some(cap) = &table.caption {
                out.push_str(&format!(" caption=\"{}\"", escaping::escape_xml_text(cap)));
            }
            out.push_str(">\n");
            if !table.headers.is_empty() {
                out.push_str(&format!("{indent}  <headers>\n"));
                for h in &table.headers {
                    out.push_str(&format!(
                        "{indent}    <col>{}</col>\n",
                        escaping::escape_xml_text(h)
                    ));
                }
                out.push_str(&format!("{indent}  </headers>\n"));
            }
            for row in &table.rows {
                out.push_str(&format!("{indent}  <row>"));
                for cell in row {
                    out.push_str(&format!("<cell>{}</cell>", escaping::escape_xml_text(cell)));
                }
                out.push_str("</row>\n");
            }
            out.push_str(&format!("{indent}</table>\n"));
        },
        ContentBlock::List(list) => {
            let tag = if list.ordered { "ordered_list" } else { "list" };
            out.push_str(&format!("{indent}<{tag}>\n"));
            for item in &list.items {
                out.push_str(&format!(
                    "{indent}  <item>{}</item>\n",
                    escaping::escape_xml_text(&item.text)
                ));
            }
            out.push_str(&format!("{indent}</{tag}>\n"));
        },
        ContentBlock::CodeBlock(code) => {
            let lang_attr = code
                .language
                .as_ref()
                .map(|l| format!(" language=\"{l}\""))
                .unwrap_or_default();
            out.push_str(&format!(
                "{indent}<code_block{lang_attr}><![CDATA[{}]]></code_block>\n",
                code.content
            ));
        },
        ContentBlock::Definition(def) => {
            out.push_str(&format!(
                "{indent}<definition term=\"{}\">{}</definition>\n",
                escaping::escape_xml_text(&def.term),
                escaping::escape_xml_text(&def.definition)
            ));
        },
        ContentBlock::Blockquote(text) => {
            out.push_str(&format!(
                "{indent}<blockquote>{}</blockquote>\n",
                escaping::escape_xml_text(text)
            ));
        },
        ContentBlock::CrossReference(cr) => {
            out.push_str(&format!(
                "{indent}<cross_ref target=\"{}\">{}</cross_ref>\n",
                escaping::escape_xml_text(&cr.target_id),
                escaping::escape_xml_text(&cr.display_text)
            ));
        },
        ContentBlock::ThematicBreak => {
            out.push_str(&format!("{indent}<hr/>\n"));
        },
        ContentBlock::Raw(text) => {
            out.push_str(&format!("{indent}<raw>{}</raw>\n", escaping::escape_xml_text(text)));
        },
    }
}

/// Format a document as GPT-optimized Markdown.
pub fn format_markdown(doc: &Document) -> String {
    let mut out = String::with_capacity(doc.full_text().len() * 2);

    // Title and metadata
    if let Some(title) = &doc.title {
        out.push_str(&format!("# {title}\n\n"));
    }

    // Metadata block
    let m = &doc.metadata;
    let mut meta_parts = Vec::new();
    if let Some(v) = &m.version {
        meta_parts.push(format!("**Version**: {v}"));
    }
    if let Some(v) = &m.effective_date {
        meta_parts.push(format!("**Effective**: {v}"));
    }
    if let Some(v) = &m.classification {
        meta_parts.push(format!("**Classification**: {v}"));
    }
    if let Some(v) = &m.author {
        meta_parts.push(format!("**Author**: {v}"));
    }
    if !meta_parts.is_empty() {
        out.push_str(&format!("> {}\n\n", meta_parts.join(" | ")));
    }

    // Sections
    for section in &doc.sections {
        write_section_md(&mut out, section);
    }

    out
}

fn write_section_md(out: &mut String, section: &Section) {
    if let Some(title) = &section.title {
        let prefix = "#".repeat(section.level.min(6) as usize);
        let number = section
            .number
            .as_ref()
            .map(|n| format!("{n} "))
            .unwrap_or_default();
        out.push_str(&format!("{prefix} {number}{title}\n\n"));
    }

    for block in &section.content {
        write_block_md(out, block);
        out.push('\n');
    }

    for child in &section.children {
        write_section_md(out, child);
    }
}

fn write_block_md(out: &mut String, block: &ContentBlock) {
    match block {
        ContentBlock::Paragraph(text) => {
            out.push_str(text);
            out.push_str("\n");
        },
        ContentBlock::Table(table) => {
            if !table.headers.is_empty() {
                out.push_str("| ");
                out.push_str(&table.headers.join(" | "));
                out.push_str(" |\n");
                out.push_str("|");
                for _ in &table.headers {
                    out.push_str("---|");
                }
                out.push('\n');
            }
            for row in &table.rows {
                out.push_str("| ");
                out.push_str(&row.join(" | "));
                out.push_str(" |\n");
            }
        },
        ContentBlock::List(list) => {
            for (i, item) in list.items.iter().enumerate() {
                if list.ordered {
                    out.push_str(&format!("{}. {}\n", i + 1, item.text));
                } else {
                    out.push_str(&format!("- {}\n", item.text));
                }
            }
        },
        ContentBlock::CodeBlock(code) => {
            let lang = code.language.as_deref().unwrap_or("");
            out.push_str(&format!("```{lang}\n{}\n```\n", code.content));
        },
        ContentBlock::Definition(def) => {
            out.push_str(&format!("**{}**: {}\n", def.term, def.definition));
        },
        ContentBlock::Blockquote(text) => {
            for line in text.lines() {
                out.push_str(&format!("> {line}\n"));
            }
        },
        ContentBlock::CrossReference(cr) => {
            out.push_str(&format!("[{}]({})\n", cr.display_text, cr.target_id));
        },
        ContentBlock::ThematicBreak => {
            out.push_str("---\n");
        },
        ContentBlock::Raw(text) => {
            out.push_str(text);
            out.push('\n');
        },
    }
}

/// Format a document as agent-friendly JSON.
pub fn format_json(doc: &Document) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_doc() -> Document {
        let mut doc = Document::new("/tmp/test.md", DocumentFormat::Markdown);
        doc.title = Some("Test Policy".into());
        doc.metadata.version = Some("1.0".into());
        doc.metadata.classification = Some("Internal".into());

        let mut s1 = Section::new(1, "Access Control");
        s1.content
            .push(ContentBlock::Paragraph("All users must authenticate.".into()));
        s1.content.push(ContentBlock::Table(Table {
            caption: Some("Access Matrix".into()),
            headers: vec!["Role".into(), "Access".into()],
            rows: vec![vec!["Admin".into(), "Full".into()]],
            alignments: vec![],
        }));

        let mut s2 = Section::new(2, "MFA Requirements");
        s2.content.push(ContentBlock::List(List {
            ordered: true,
            items: vec![
                ListItem { text: "Hardware key".into(), children: None },
                ListItem { text: "Authenticator app".into(), children: None },
            ],
        }));
        s1.children.push(s2);

        doc.sections.push(s1);
        doc
    }

    #[test]
    fn test_xml_output() {
        let doc = make_test_doc();
        let xml = format_xml(&doc);
        assert!(xml.contains("<document>"));
        assert!(xml.contains("<title>Test Policy</title>"));
        assert!(xml.contains("<section level=\"1\""));
        assert!(xml.contains("<table"));
        assert!(xml.contains("<ordered_list>"));
        assert!(xml.contains("</document>"));
    }

    #[test]
    fn test_markdown_output() {
        let doc = make_test_doc();
        let md = format_markdown(&doc);
        assert!(md.contains("# Test Policy"));
        assert!(md.contains("# Access Control"));
        assert!(md.contains("| Role | Access |"));
        assert!(md.contains("1. Hardware key"));
    }

    #[test]
    fn test_json_output() {
        let doc = make_test_doc();
        let json = format_json(&doc);
        assert!(json.contains("\"Test Policy\""));
        assert!(json.contains("\"Access Control\""));
    }
}
