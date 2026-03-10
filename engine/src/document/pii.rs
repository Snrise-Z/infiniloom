//! PII (Personally Identifiable Information) detection for documents.
//!
//! Scans document text for common PII patterns including SSNs, credit card numbers
//! (Luhn-validated), email addresses, phone numbers, and IP addresses. Provides both
//! detection (scan) and redaction (replace with `[REDACTED-KIND]` placeholders).
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::document::pii;
//!
//! let findings = pii::scan_document(&doc);
//! for f in &findings {
//!     println!("{:?} found at {}: {}", f.kind, f.location, f.text);
//! }
//!
//! // Redact all PII in-place
//! pii::redact_document(&mut doc);
//! ```

use once_cell::sync::Lazy;
use regex::Regex;

use super::types::{ContentBlock, Document, Section};

// ---------------------------------------------------------------------------
// PII pattern regexes (compiled once, reused across all calls)
// ---------------------------------------------------------------------------

static RE_SSN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("RE_SSN: invalid regex"));

static RE_CREDIT_CARD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d{4}[- ]?){3}\d{4}\b").expect("RE_CREDIT_CARD: invalid regex"));

static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
        .expect("RE_EMAIL: invalid regex")
});

static RE_PHONE: Lazy<Regex> = Lazy::new(|| {
    // Require either parentheses around area code OR at least one separator between digit groups
    // to avoid matching bare 10-digit numbers like account IDs or timestamps.
    Regex::new(r"(?:\+?1[-.\s]?)?\(\d{3}\)[-.\s]?\d{3}[-.\s]?\d{4}\b|\b(?:\+?1[-.\s]?)?\d{3}[-.\s]\d{3}[-.\s]\d{4}\b")
        .expect("RE_PHONE: invalid regex")
});

static RE_IP_ADDRESS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("RE_IP_ADDRESS: invalid regex"));

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Kind of PII detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiKind {
    /// Social Security Number (XXX-XX-XXXX)
    Ssn,
    /// Credit card number (Luhn-validated)
    CreditCard,
    /// Email address
    Email,
    /// Phone number (US formats)
    Phone,
    /// IPv4 address (each octet 0-255)
    IpAddress,
}

impl PiiKind {
    /// Human-readable label used in redaction placeholders.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ssn => "SSN",
            Self::CreditCard => "CREDIT-CARD",
            Self::Email => "EMAIL",
            Self::Phone => "PHONE",
            Self::IpAddress => "IP-ADDRESS",
        }
    }
}

/// A single PII finding within a document.
#[derive(Debug, Clone)]
pub struct PiiFinding {
    /// What kind of PII was detected.
    pub kind: PiiKind,
    /// The matched text.
    pub text: String,
    /// Section path or description of where it was found.
    pub location: String,
    /// Approximate line number in the source block (1-indexed within the block).
    pub line_approx: usize,
}

// ---------------------------------------------------------------------------
// Luhn check for credit card validation
// ---------------------------------------------------------------------------

fn luhn_check(digits: &str) -> bool {
    let digits: Vec<u32> = digits
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap())
        .collect();
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let dd = d * 2;
                if dd > 9 {
                    dd - 9
                } else {
                    dd
                }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

/// Validate SSN area/group/serial rules to reduce false positives.
/// Real SSNs never start with 000, 666, or 9xx; group is never 00; serial is never 0000.
fn is_valid_ssn(ssn: &str) -> bool {
    let parts: Vec<&str> = ssn.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let area: u16 = parts[0].parse().unwrap_or(0);
    let group: u16 = parts[1].parse().unwrap_or(0);
    let serial: u16 = parts[2].parse().unwrap_or(0);
    area != 0 && area != 666 && area < 900 && group != 0 && serial != 0
}

/// Validate that each octet of an IPv4 address is in 0-255.
fn is_valid_ipv4(ip: &str) -> bool {
    let octets: Vec<&str> = ip.split('.').collect();
    if octets.len() != 4 {
        return false;
    }
    octets
        .iter()
        .all(|o| o.parse::<u16>().map_or(false, |n| n <= 255))
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scan document text for PII patterns.
///
/// Walks the full section hierarchy, producing a `PiiFinding` for every match.
pub fn scan_document(doc: &Document) -> Vec<PiiFinding> {
    let mut findings = Vec::new();
    scan_sections(&doc.sections, &[], &mut findings);
    findings
}

fn scan_sections(sections: &[Section], path: &[String], findings: &mut Vec<PiiFinding>) {
    for section in sections {
        let mut current_path = path.to_vec();
        if let Some(title) = &section.title {
            current_path.push(title.clone());
        }
        let location = if current_path.is_empty() {
            "(root)".to_owned()
        } else {
            current_path.join(" > ")
        };

        for block in &section.content {
            let text = block.text();
            scan_text(&text, &location, findings);
        }

        scan_sections(&section.children, &current_path, findings);
    }
}

fn scan_text(text: &str, location: &str, findings: &mut Vec<PiiFinding>) {
    for (line_idx, line) in text.lines().enumerate() {
        // SSN (validated)
        for m in RE_SSN.find_iter(line) {
            let matched = m.as_str();
            if is_valid_ssn(matched) {
                findings.push(PiiFinding {
                    kind: PiiKind::Ssn,
                    text: matched.to_owned(),
                    location: location.to_owned(),
                    line_approx: line_idx + 1,
                });
            }
        }

        // Credit card (Luhn validated)
        for m in RE_CREDIT_CARD.find_iter(line) {
            let matched = m.as_str();
            if luhn_check(matched) {
                findings.push(PiiFinding {
                    kind: PiiKind::CreditCard,
                    text: matched.to_owned(),
                    location: location.to_owned(),
                    line_approx: line_idx + 1,
                });
            }
        }

        // Email
        for m in RE_EMAIL.find_iter(line) {
            findings.push(PiiFinding {
                kind: PiiKind::Email,
                text: m.as_str().to_owned(),
                location: location.to_owned(),
                line_approx: line_idx + 1,
            });
        }

        // Phone
        for m in RE_PHONE.find_iter(line) {
            findings.push(PiiFinding {
                kind: PiiKind::Phone,
                text: m.as_str().to_owned(),
                location: location.to_owned(),
                line_approx: line_idx + 1,
            });
        }

        // IP address (validated)
        for m in RE_IP_ADDRESS.find_iter(line) {
            let matched = m.as_str();
            if is_valid_ipv4(matched) {
                findings.push(PiiFinding {
                    kind: PiiKind::IpAddress,
                    text: matched.to_owned(),
                    location: location.to_owned(),
                    line_approx: line_idx + 1,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

/// Redact all PII patterns in a text string, replacing matches with
/// `[REDACTED-KIND]` placeholders (e.g. `[REDACTED-SSN]`).
pub fn redact_text(text: &str) -> String {
    let mut result = text.to_owned();

    // SSN (validated)
    result = redact_ssns(&result);

    // Credit card — needs Luhn validation, so we do manual replacement
    result = redact_credit_cards(&result);

    // Email
    result = RE_EMAIL
        .replace_all(&result, "[REDACTED-EMAIL]")
        .into_owned();

    // Phone
    result = RE_PHONE
        .replace_all(&result, "[REDACTED-PHONE]")
        .into_owned();

    // IP address — needs octet validation
    result = redact_ip_addresses(&result);

    result
}

fn redact_ssns(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_SSN.find_iter(text) {
        if is_valid_ssn(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-SSN]");
        } else {
            result.push_str(&text[last_end..m.end()]);
        }
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

fn redact_credit_cards(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_CREDIT_CARD.find_iter(text) {
        if luhn_check(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-CREDIT-CARD]");
        } else {
            result.push_str(&text[last_end..m.end()]);
        }
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

fn redact_ip_addresses(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_IP_ADDRESS.find_iter(text) {
        if is_valid_ipv4(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-IP-ADDRESS]");
        } else {
            result.push_str(&text[last_end..m.end()]);
        }
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

/// Redact PII in-place across an entire document.
pub fn redact_document(doc: &mut Document) {
    redact_sections(&mut doc.sections);
}

fn redact_sections(sections: &mut [Section]) {
    for section in sections.iter_mut() {
        for block in &mut section.content {
            redact_block(block);
        }
        redact_sections(&mut section.children);
    }
}

fn redact_block(block: &mut ContentBlock) {
    match block {
        ContentBlock::Paragraph(text) => *text = redact_text(text),
        ContentBlock::Blockquote(text) => *text = redact_text(text),
        ContentBlock::Raw(text) => *text = redact_text(text),
        ContentBlock::Table(table) => {
            if let Some(caption) = &mut table.caption {
                *caption = redact_text(caption);
            }
            for row in &mut table.rows {
                for cell in row.iter_mut() {
                    *cell = redact_text(cell);
                }
            }
            for header in &mut table.headers {
                *header = redact_text(header);
            }
        },
        ContentBlock::List(list) => {
            redact_list(list);
        },
        ContentBlock::Definition(def) => {
            def.term = redact_text(&def.term);
            def.definition = redact_text(&def.definition);
        },
        ContentBlock::CodeBlock(_)
        | ContentBlock::CrossReference(_)
        | ContentBlock::ThematicBreak => {},
    }
}

fn redact_list(list: &mut super::types::List) {
    for item in &mut list.items {
        item.text = redact_text(&item.text);
        if let Some(ref mut children) = item.children {
            redact_list(children);
        }
    }
}

/// Produce a human-readable summary of PII findings.
pub fn summarize(findings: &[PiiFinding]) -> String {
    if findings.is_empty() {
        return "PII scan: no items found".to_owned();
    }

    let ssn_count = findings.iter().filter(|f| f.kind == PiiKind::Ssn).count();
    let cc_count = findings
        .iter()
        .filter(|f| f.kind == PiiKind::CreditCard)
        .count();
    let email_count = findings.iter().filter(|f| f.kind == PiiKind::Email).count();
    let phone_count = findings.iter().filter(|f| f.kind == PiiKind::Phone).count();
    let ip_count = findings
        .iter()
        .filter(|f| f.kind == PiiKind::IpAddress)
        .count();

    let mut parts = Vec::new();
    if ssn_count > 0 {
        parts.push(format!("{ssn_count} SSN{}", if ssn_count > 1 { "s" } else { "" }));
    }
    if cc_count > 0 {
        parts.push(format!("{cc_count} credit card{}", if cc_count > 1 { "s" } else { "" }));
    }
    if email_count > 0 {
        parts.push(format!("{email_count} email{}", if email_count > 1 { "s" } else { "" }));
    }
    if phone_count > 0 {
        parts.push(format!("{phone_count} phone{}", if phone_count > 1 { "s" } else { "" }));
    }
    if ip_count > 0 {
        parts.push(format!("{ip_count} IP address{}", if ip_count > 1 { "es" } else { "" }));
    }

    format!("PII scan: found {} items ({})", findings.len(), parts.join(", "))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::types::{Alignment, Definition, List, ListItem, Section, Table};

    #[test]
    fn test_ssn_detection() {
        let mut findings = Vec::new();
        scan_text("My SSN is 123-45-6789", "test", &mut findings);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, PiiKind::Ssn);
        assert_eq!(findings[0].text, "123-45-6789");
    }

    #[test]
    fn test_ssn_no_false_positive() {
        let mut findings = Vec::new();
        // Not an SSN pattern (too many digits in first group)
        scan_text("The code is 1234-56-7890", "test", &mut findings);
        let ssn_findings: Vec<_> = findings.iter().filter(|f| f.kind == PiiKind::Ssn).collect();
        assert!(ssn_findings.is_empty());
    }

    #[test]
    fn test_ssn_invalid_area_group_serial() {
        let mut findings = Vec::new();
        // Area 000 is invalid
        scan_text("SSN: 000-12-3456", "test", &mut findings);
        assert!(findings.iter().all(|f| f.kind != PiiKind::Ssn));

        // Area 666 is invalid
        findings.clear();
        scan_text("SSN: 666-12-3456", "test", &mut findings);
        assert!(findings.iter().all(|f| f.kind != PiiKind::Ssn));

        // Area 900+ is invalid
        findings.clear();
        scan_text("SSN: 900-12-3456", "test", &mut findings);
        assert!(findings.iter().all(|f| f.kind != PiiKind::Ssn));

        // Group 00 is invalid
        findings.clear();
        scan_text("SSN: 123-00-3456", "test", &mut findings);
        assert!(findings.iter().all(|f| f.kind != PiiKind::Ssn));

        // Serial 0000 is invalid
        findings.clear();
        scan_text("SSN: 123-45-0000", "test", &mut findings);
        assert!(findings.iter().all(|f| f.kind != PiiKind::Ssn));
    }

    #[test]
    fn test_ssn_redact_validates() {
        // Invalid SSN should NOT be redacted
        let result = redact_text("SSN: 000-12-3456");
        assert!(result.contains("000-12-3456"), "Invalid SSN should be preserved");

        // Valid SSN should be redacted
        let result = redact_text("SSN: 123-45-6789");
        assert!(result.contains("[REDACTED-SSN]"));
        assert!(!result.contains("123-45-6789"));
    }

    #[test]
    fn test_credit_card_valid_luhn() {
        // 4532015112830366 is a valid Luhn number
        let mut findings = Vec::new();
        scan_text("Card: 4532015112830366", "test", &mut findings);
        let cc: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::CreditCard)
            .collect();
        assert_eq!(cc.len(), 1);
        assert_eq!(cc[0].text, "4532015112830366");
    }

    #[test]
    fn test_credit_card_invalid_luhn() {
        // 1234567890123456 does not pass Luhn
        let mut findings = Vec::new();
        scan_text("Card: 1234567890123456", "test", &mut findings);
        let cc: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::CreditCard)
            .collect();
        assert!(cc.is_empty(), "Invalid Luhn should not be detected");
    }

    #[test]
    fn test_credit_card_with_spaces() {
        // 4532 0151 1283 0366 (valid Luhn, spaced)
        let mut findings = Vec::new();
        scan_text("Card: 4532 0151 1283 0366", "test", &mut findings);
        let cc: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::CreditCard)
            .collect();
        assert_eq!(cc.len(), 1);
    }

    #[test]
    fn test_email_detection() {
        let mut findings = Vec::new();
        scan_text("Contact john@example.com for details", "test", &mut findings);
        let emails: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Email)
            .collect();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].text, "john@example.com");
    }

    #[test]
    fn test_phone_detection_parentheses() {
        let mut findings = Vec::new();
        scan_text("Call (555) 123-4567 for info", "test", &mut findings);
        let phones: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Phone)
            .collect();
        assert_eq!(phones.len(), 1);
    }

    #[test]
    fn test_phone_detection_international() {
        let mut findings = Vec::new();
        scan_text("Phone: +1-555-123-4567", "test", &mut findings);
        let phones: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Phone)
            .collect();
        assert_eq!(phones.len(), 1);
    }

    #[test]
    fn test_ip_address_valid() {
        let mut findings = Vec::new();
        scan_text("Server at 192.168.1.100", "test", &mut findings);
        let ips: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IpAddress)
            .collect();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].text, "192.168.1.100");
    }

    #[test]
    fn test_ip_address_invalid_octet() {
        let mut findings = Vec::new();
        scan_text("Not an IP: 999.999.999.999", "test", &mut findings);
        let ips: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IpAddress)
            .collect();
        assert!(ips.is_empty(), "Invalid octets should not match");
    }

    #[test]
    fn test_no_false_positives_normal_text() {
        let mut findings = Vec::new();
        scan_text(
            "The quick brown fox jumps over the lazy dog. Version 3.2.1 released.",
            "test",
            &mut findings,
        );
        assert!(findings.is_empty(), "Normal text should have no findings");
    }

    #[test]
    fn test_redact_ssn() {
        let result = redact_text("My SSN is 123-45-6789.");
        assert_eq!(result, "My SSN is [REDACTED-SSN].");
        assert!(!result.contains("123-45-6789"));
    }

    #[test]
    fn test_redact_email() {
        let result = redact_text("Email: user@domain.com here");
        assert!(result.contains("[REDACTED-EMAIL]"));
        assert!(!result.contains("user@domain.com"));
    }

    #[test]
    fn test_redact_credit_card() {
        let result = redact_text("CC: 4532015112830366");
        assert!(result.contains("[REDACTED-CREDIT-CARD]"));
        assert!(!result.contains("4532015112830366"));
    }

    #[test]
    fn test_redact_phone() {
        let result = redact_text("Call (555) 123-4567");
        assert!(result.contains("[REDACTED-PHONE]"));
    }

    #[test]
    fn test_redact_ip() {
        let result = redact_text("Server: 10.0.0.1");
        assert!(result.contains("[REDACTED-IP-ADDRESS]"));
        assert!(!result.contains("10.0.0.1"));
    }

    #[test]
    fn test_redact_preserves_invalid_cc() {
        // 1234567890123456 fails Luhn — should NOT be redacted
        let input = "Card: 1234567890123456";
        let result = redact_text(input);
        assert!(result.contains("1234567890123456"), "Invalid CC should be preserved");
    }

    #[test]
    fn test_redact_preserves_invalid_ip() {
        let input = "Value: 999.888.777.666";
        let result = redact_text(input);
        assert!(result.contains("999.888.777.666"), "Invalid IP should be preserved");
    }

    #[test]
    fn test_scan_document_traverses_sections() {
        let mut doc = Document::new("/tmp/test.md", super::super::types::DocumentFormat::Markdown);
        let mut s1 = Section::new(1, "Contact Info");
        s1.content
            .push(ContentBlock::Paragraph("Email: alice@corp.com".into()));

        let mut s2 = Section::new(2, "Payment");
        s2.content
            .push(ContentBlock::Paragraph("Card on file: 4532015112830366".into()));
        s1.children.push(s2);
        doc.sections.push(s1);

        let findings = scan_document(&doc);
        assert!(findings.iter().any(|f| f.kind == PiiKind::Email));
        assert!(findings.iter().any(|f| f.kind == PiiKind::CreditCard));
        assert_eq!(
            findings
                .iter()
                .find(|f| f.kind == PiiKind::Email)
                .unwrap()
                .location,
            "Contact Info"
        );
        assert_eq!(
            findings
                .iter()
                .find(|f| f.kind == PiiKind::CreditCard)
                .unwrap()
                .location,
            "Contact Info > Payment"
        );
    }

    #[test]
    fn test_redact_document_all_block_types() {
        let mut doc = Document::new("/tmp/test.md", super::super::types::DocumentFormat::Markdown);
        let mut section = Section::root();

        // Paragraph
        section
            .content
            .push(ContentBlock::Paragraph("SSN: 123-45-6789".into()));

        // Blockquote
        section
            .content
            .push(ContentBlock::Blockquote("Email: bob@test.com".into()));

        // Raw
        section
            .content
            .push(ContentBlock::Raw("IP: 10.0.0.1".into()));

        // Table
        section.content.push(ContentBlock::Table(Table {
            caption: None,
            headers: vec!["Name".into(), "Phone".into()],
            rows: vec![vec!["Alice".into(), "(555) 123-4567".into()]],
            alignments: vec![Alignment::Left, Alignment::Left],
        }));

        // List
        section.content.push(ContentBlock::List(List {
            ordered: false,
            items: vec![ListItem { text: "Card: 4532015112830366".into(), children: None }],
        }));

        // Definition
        section.content.push(ContentBlock::Definition(Definition {
            term: "SSN".into(),
            definition: "e.g. 987-65-4321".into(),
        }));

        doc.sections.push(section);

        redact_document(&mut doc);

        let full = doc.full_text();
        assert!(full.contains("[REDACTED-SSN]"), "SSN should be redacted");
        assert!(full.contains("[REDACTED-EMAIL]"), "Email should be redacted");
        assert!(full.contains("[REDACTED-IP-ADDRESS]"), "IP should be redacted");
        assert!(full.contains("[REDACTED-PHONE]"), "Phone should be redacted");
        assert!(full.contains("[REDACTED-CREDIT-CARD]"), "CC should be redacted");
        // Original PII should be gone
        assert!(!full.contains("123-45-6789"));
        assert!(!full.contains("bob@test.com"));
        assert!(!full.contains("10.0.0.1"));
        assert!(!full.contains("(555) 123-4567"));
        assert!(!full.contains("4532015112830366"));
    }

    #[test]
    fn test_summarize_empty() {
        assert_eq!(summarize(&[]), "PII scan: no items found");
    }

    #[test]
    fn test_summarize_mixed() {
        let findings = vec![
            PiiFinding {
                kind: PiiKind::Ssn,
                text: "123-45-6789".into(),
                location: "test".into(),
                line_approx: 1,
            },
            PiiFinding {
                kind: PiiKind::Email,
                text: "a@b.com".into(),
                location: "test".into(),
                line_approx: 1,
            },
            PiiFinding {
                kind: PiiKind::Email,
                text: "c@d.com".into(),
                location: "test".into(),
                line_approx: 2,
            },
        ];
        let summary = summarize(&findings);
        assert!(summary.contains("found 3 items"));
        assert!(summary.contains("1 SSN"));
        assert!(summary.contains("2 emails"));
    }

    #[test]
    fn test_luhn_check() {
        // Known valid: Visa test number
        assert!(luhn_check("4532015112830366"));
        // Known valid: another test number
        assert!(luhn_check("4111111111111111"));
        // Invalid
        assert!(!luhn_check("1234567890123456"));
        // Too short
        assert!(!luhn_check("123456"));
        // Too long
        assert!(!luhn_check("12345678901234567890"));
    }

    #[test]
    fn test_is_valid_ipv4() {
        assert!(is_valid_ipv4("192.168.1.1"));
        assert!(is_valid_ipv4("0.0.0.0"));
        assert!(is_valid_ipv4("255.255.255.255"));
        assert!(!is_valid_ipv4("256.0.0.1"));
        assert!(!is_valid_ipv4("192.168.1"));
        assert!(!is_valid_ipv4("999.999.999.999"));
    }

    #[test]
    fn test_multiple_pii_same_line() {
        let mut findings = Vec::new();
        scan_text("Contact: john@example.com, SSN: 123-45-6789", "test", &mut findings);
        assert!(findings.iter().any(|f| f.kind == PiiKind::Email));
        assert!(findings.iter().any(|f| f.kind == PiiKind::Ssn));
    }

    #[test]
    fn test_redact_multiple_same_line() {
        let result = redact_text("SSN: 123-45-6789, email: user@test.com");
        assert!(result.contains("[REDACTED-SSN]"));
        assert!(result.contains("[REDACTED-EMAIL]"));
        assert!(!result.contains("123-45-6789"));
        assert!(!result.contains("user@test.com"));
    }
}
