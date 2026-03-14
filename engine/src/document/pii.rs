//! PII (Personally Identifiable Information) detection for documents.
//!
//! Scans document text for common PII patterns including SSNs, credit card numbers
//! (Luhn-validated), email addresses, phone numbers, IP addresses (IPv4 and IPv6),
//! IBAN bank account numbers, UK National Insurance Numbers, international phone
//! numbers (E.164), and EU VAT numbers. Provides both detection (scan) and redaction
//! (replace with `[REDACTED-KIND]` placeholders).
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::document::pii;
//!
//! let findings = pii::scan_document(&doc);
//! // Note: avoid logging f.text in production — it contains raw PII values
//! for f in &findings {
//!     println!("{:?} found at {} line ~{}", f.kind, f.location, f.line_approx);
//! }
//! println!("{}", pii::summarize(&findings));
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

/// IBAN: 2 uppercase letters (country) + 2 check digits + up to 30 alphanumeric characters.
static RE_IBAN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{4,30}\b").expect("RE_IBAN: invalid regex"));

/// UK National Insurance Number: 2 letters + 6 digits + 1 letter (with optional spaces).
/// First letter excludes D, F, I, Q, U, V; second letter also excludes O.
static RE_UK_NINO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-CEGHJ-PR-TW-Z][A-CEGHJ-NPR-TW-Z]\s?\d{2}\s?\d{2}\s?\d{2}\s?[A-D]\b")
        .expect("RE_UK_NINO: invalid regex")
});

/// International phone number in E.164 format: + followed by country code and subscriber
/// number, totaling 7-15 digits. Must start with + to distinguish from US phone patterns.
static RE_INTL_PHONE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\+[1-9]\d{0,2}[-.\s]?\d[\d\-.\s]{4,13}\d\b").expect("RE_INTL_PHONE: invalid regex")
});

/// IPv6 address: 8 groups of 4 hex digits separated by colons, with support for :: shorthand.
static RE_IPV6: Lazy<Regex> = Lazy::new(|| {
    // Match full 8-group IPv6, or various :: shorthand forms.
    Regex::new(concat!(
        r"(?i)\b(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,7}:\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,6}:[0-9a-f]{1,4}\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,5}(?::[0-9a-f]{1,4}){1,2}\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,4}(?::[0-9a-f]{1,4}){1,3}\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,3}(?::[0-9a-f]{1,4}){1,4}\b",
        r"|(?i)\b(?:[0-9a-f]{1,4}:){1,2}(?::[0-9a-f]{1,4}){1,5}\b",
        r"|(?i)\b[0-9a-f]{1,4}:(?::[0-9a-f]{1,4}){1,6}\b",
        r"|(?i)(?:^|\s)::(?:[0-9a-f]{1,4}:){0,5}[0-9a-f]{1,4}\b",
    ))
    .expect("RE_IPV6: invalid regex")
});

/// EU VAT number: 2 uppercase letters (EU country code) + 2-12 alphanumeric characters.
static RE_EU_VAT: Lazy<Regex> = Lazy::new(|| {
    // Only match known EU member state country codes to reduce false positives.
    Regex::new(concat!(
        r"\b(?:AT|BE|BG|CY|CZ|DE|DK|EE|EL|ES|FI|FR|HR|HU|IE|IT|LT|LU|LV|MT|NL|PL|PT|RO|",
        r"SE|SI|SK)[A-Z0-9]{2,12}\b",
    ))
    .expect("RE_EU_VAT: invalid regex")
});

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
    /// IBAN bank account number (international)
    Iban,
    /// UK National Insurance Number
    UkNino,
    /// International phone number (E.164 format)
    IntlPhone,
    /// IPv6 address
    Ipv6Address,
    /// EU VAT identification number
    EuVat,
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
            Self::Iban => "IBAN",
            Self::UkNino => "UK-NINO",
            Self::IntlPhone => "INTL-PHONE",
            Self::Ipv6Address => "IPV6-ADDRESS",
            Self::EuVat => "EU-VAT",
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
    sum.is_multiple_of(10)
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

/// Validate IBAN using the MOD-97 check digit algorithm (ISO 13616).
/// Returns true if the IBAN has valid check digits.
fn is_valid_iban(iban: &str) -> bool {
    let clean: String = iban.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() < 5 || clean.len() > 34 {
        return false;
    }
    // Move first 4 chars to end
    let rearranged = format!("{}{}", &clean[4..], &clean[..4]);
    // Convert letters to numbers (A=10, B=11, ..., Z=35)
    let mut numeric = String::with_capacity(rearranged.len() * 2);
    for ch in rearranged.chars() {
        if ch.is_ascii_digit() {
            numeric.push(ch);
        } else if ch.is_ascii_uppercase() {
            let val = (ch as u32) - ('A' as u32) + 10;
            numeric.push_str(&val.to_string());
        } else {
            return false;
        }
    }
    // MOD 97 on the large number (process in chunks to avoid overflow)
    let mut remainder: u64 = 0;
    for chunk in numeric.as_bytes().chunks(9) {
        // Safety: `numeric` is built from ASCII digits only, so from_utf8 and parse
        // should never fail. If they do, the IBAN is structurally invalid.
        let s = match std::str::from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let combined = format!("{remainder}{s}");
        remainder = match combined.parse::<u64>() {
            Ok(n) => n % 97,
            Err(_) => return false,
        };
    }
    remainder == 1
}

/// Validate that an E.164 phone number has the right digit count (7-15 digits total).
fn is_valid_intl_phone(phone: &str) -> bool {
    let digit_count = phone.chars().filter(|c| c.is_ascii_digit()).count();
    (7..=15).contains(&digit_count)
}

/// Validate an EU VAT number using country-specific format rules.
/// The input must start with a 2-letter EU country code followed by the body.
fn is_valid_vat(s: &str) -> bool {
    if s.len() < 4 {
        return false;
    }
    let country = &s[..2];
    let body = &s[2..];

    // Body must contain at least one digit (redundant with regex, but defensive).
    if !body.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    match country {
        // DE: exactly 9 digits
        "DE" => body.len() == 9 && body.chars().all(|c| c.is_ascii_digit()),
        // FR: 2 chars (digit or letter) + 9 digits = 11 chars total
        "FR" => {
            body.len() == 11
                && body[..2].chars().all(|c| c.is_ascii_alphanumeric())
                && body[2..].chars().all(|c| c.is_ascii_digit())
        },
        // IT: exactly 11 digits
        "IT" => body.len() == 11 && body.chars().all(|c| c.is_ascii_digit()),
        // ES: 1 letter + 7 digits + 1 alphanumeric = 9 chars total
        "ES" => {
            body.len() == 9
                && body
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_ascii_alphabetic())
                && body[1..8].chars().all(|c| c.is_ascii_digit())
                && body
                    .chars()
                    .last()
                    .map_or(false, |c| c.is_ascii_alphanumeric())
        },
        // All other EU countries: require at least 2 digits in the body
        _ => body.chars().filter(|c| c.is_ascii_digit()).count() >= 2,
    }
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
            // Skip code blocks to match the redaction behavior in redact_block(),
            // which intentionally leaves CodeBlock content untouched. Code blocks
            // frequently contain example patterns (IPs, tokens, etc.) that are not
            // real PII and would produce noisy false positives.
            if matches!(block, ContentBlock::CodeBlock(_)) {
                continue;
            }
            let text = block.text();
            scan_text(&text, &location, findings);
        }

        scan_sections(&section.children, &current_path, findings);
    }
}

/// Specificity rank for PII kinds. Higher = more specific.
/// When two findings overlap in byte range, we keep the more specific one.
fn specificity_rank(kind: PiiKind) -> u8 {
    match kind {
        PiiKind::Ssn => 10,
        PiiKind::CreditCard => 10,
        PiiKind::Phone => 9, // US phone is more specific than intl phone
        PiiKind::Email => 10,
        PiiKind::IpAddress => 10,
        PiiKind::Iban => 10,
        PiiKind::UkNino => 10,
        PiiKind::IntlPhone => 5, // Broad pattern, lower specificity
        PiiKind::Ipv6Address => 10,
        PiiKind::EuVat => 10,
    }
}

fn scan_text(text: &str, location: &str, findings: &mut Vec<PiiFinding>) {
    // Collect findings with byte ranges so we can deduplicate overlapping matches
    // (e.g. "+1-555-123-4567" matches both US phone and intl phone regexes).
    // Each entry: (start_byte, end_byte, kind, matched_text, line_number)
    let mut raw: Vec<(usize, usize, PiiKind, String, usize)> = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        let line_offset = line.as_ptr() as usize - text.as_ptr() as usize;

        for m in RE_SSN.find_iter(line) {
            let matched = m.as_str();
            if is_valid_ssn(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::Ssn,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }

        for m in RE_CREDIT_CARD.find_iter(line) {
            let matched = m.as_str();
            if luhn_check(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::CreditCard,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }

        for m in RE_EMAIL.find_iter(line) {
            raw.push((
                line_offset + m.start(),
                line_offset + m.end(),
                PiiKind::Email,
                m.as_str().to_owned(),
                line_idx + 1,
            ));
        }

        for m in RE_PHONE.find_iter(line) {
            raw.push((
                line_offset + m.start(),
                line_offset + m.end(),
                PiiKind::Phone,
                m.as_str().to_owned(),
                line_idx + 1,
            ));
        }

        for m in RE_IP_ADDRESS.find_iter(line) {
            let matched = m.as_str();
            if is_valid_ipv4(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::IpAddress,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }

        for m in RE_IBAN.find_iter(line) {
            let matched = m.as_str();
            if is_valid_iban(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::Iban,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }

        for m in RE_UK_NINO.find_iter(line) {
            raw.push((
                line_offset + m.start(),
                line_offset + m.end(),
                PiiKind::UkNino,
                m.as_str().to_owned(),
                line_idx + 1,
            ));
        }

        for m in RE_INTL_PHONE.find_iter(line) {
            let matched = m.as_str();
            if is_valid_intl_phone(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::IntlPhone,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }

        for m in RE_IPV6.find_iter(line) {
            raw.push((
                line_offset + m.start(),
                line_offset + m.end(),
                PiiKind::Ipv6Address,
                m.as_str().to_owned(),
                line_idx + 1,
            ));
        }

        for m in RE_EU_VAT.find_iter(line) {
            let matched = m.as_str();
            if is_valid_vat(matched) {
                raw.push((
                    line_offset + m.start(),
                    line_offset + m.end(),
                    PiiKind::EuVat,
                    matched.to_owned(),
                    line_idx + 1,
                ));
            }
        }
    }

    // Deduplicate overlapping findings. When two findings overlap in byte range,
    // keep the more specific one (higher specificity_rank). This prevents e.g.
    // "+1-555-123-4567" from producing both a US Phone and an IntlPhone finding.
    raw.sort_by_key(|r| (r.0, r.1));
    let mut deduped: Vec<(usize, usize, PiiKind, String, usize)> = Vec::with_capacity(raw.len());
    for entry in raw {
        let dominated = deduped.iter().any(|kept| {
            let overlaps = kept.0 < entry.1 && entry.0 < kept.1;
            overlaps && specificity_rank(kept.2) >= specificity_rank(entry.2)
        });
        if dominated {
            continue;
        }
        // Remove any previously-kept entries that this new (more specific) entry dominates
        deduped.retain(|kept| {
            let overlaps = kept.0 < entry.1 && entry.0 < kept.1;
            !(overlaps && specificity_rank(entry.2) > specificity_rank(kept.2))
        });
        deduped.push(entry);
    }

    findings.extend(
        deduped
            .into_iter()
            .map(|(_, _, kind, text, line_approx)| PiiFinding {
                kind,
                text,
                location: location.to_owned(),
                line_approx,
            }),
    );
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

    // Phone (US)
    result = RE_PHONE
        .replace_all(&result, "[REDACTED-PHONE]")
        .into_owned();

    // IP address — needs octet validation
    result = redact_ip_addresses(&result);

    // IBAN — needs MOD-97 validation
    result = redact_ibans(&result);

    // UK NINO
    result = RE_UK_NINO
        .replace_all(&result, "[REDACTED-UK-NINO]")
        .into_owned();

    // International phone — needs digit count validation
    result = redact_intl_phones(&result);

    // IPv6
    result = RE_IPV6
        .replace_all(&result, "[REDACTED-IPV6-ADDRESS]")
        .into_owned();

    // EU VAT (validated per-country format)
    result = redact_vat_numbers(&result);

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

fn redact_ibans(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_IBAN.find_iter(text) {
        if is_valid_iban(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-IBAN]");
        } else {
            result.push_str(&text[last_end..m.end()]);
        }
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

fn redact_intl_phones(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_INTL_PHONE.find_iter(text) {
        if is_valid_intl_phone(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-INTL-PHONE]");
        } else {
            result.push_str(&text[last_end..m.end()]);
        }
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

fn redact_vat_numbers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in RE_EU_VAT.find_iter(text) {
        if is_valid_vat(m.as_str()) {
            result.push_str(&text[last_end..m.start()]);
            result.push_str("[REDACTED-EU-VAT]");
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
    let iban_count = findings.iter().filter(|f| f.kind == PiiKind::Iban).count();
    let nino_count = findings
        .iter()
        .filter(|f| f.kind == PiiKind::UkNino)
        .count();
    let intl_phone_count = findings
        .iter()
        .filter(|f| f.kind == PiiKind::IntlPhone)
        .count();
    let ipv6_count = findings
        .iter()
        .filter(|f| f.kind == PiiKind::Ipv6Address)
        .count();
    let vat_count = findings.iter().filter(|f| f.kind == PiiKind::EuVat).count();

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
    if iban_count > 0 {
        parts.push(format!("{iban_count} IBAN{}", if iban_count > 1 { "s" } else { "" }));
    }
    if nino_count > 0 {
        parts.push(format!("{nino_count} UK NINO{}", if nino_count > 1 { "s" } else { "" }));
    }
    if intl_phone_count > 0 {
        parts.push(format!(
            "{intl_phone_count} intl phone{}",
            if intl_phone_count > 1 { "s" } else { "" }
        ));
    }
    if ipv6_count > 0 {
        parts.push(format!("{ipv6_count} IPv6 address{}", if ipv6_count > 1 { "es" } else { "" }));
    }
    if vat_count > 0 {
        parts.push(format!("{vat_count} EU VAT{}", if vat_count > 1 { "s" } else { "" }));
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

    // -----------------------------------------------------------------------
    // International PII pattern tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_iban_detection() {
        let mut findings = Vec::new();
        // Valid German IBAN (passes MOD-97)
        scan_text("IBAN: DE89370400440532013000", "test", &mut findings);
        let ibans: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Iban)
            .collect();
        assert_eq!(ibans.len(), 1);
        assert_eq!(ibans[0].text, "DE89370400440532013000");
    }

    #[test]
    fn test_iban_gb() {
        let mut findings = Vec::new();
        // Valid UK IBAN
        scan_text("Account: GB29NWBK60161331926819", "test", &mut findings);
        let ibans: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Iban)
            .collect();
        assert_eq!(ibans.len(), 1);
        assert_eq!(ibans[0].text, "GB29NWBK60161331926819");
    }

    #[test]
    fn test_iban_invalid_check_digits() {
        let mut findings = Vec::new();
        // Invalid check digits (DE00 instead of DE89)
        scan_text("IBAN: DE00370400440532013000", "test", &mut findings);
        let ibans: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Iban)
            .collect();
        assert!(ibans.is_empty(), "Invalid IBAN check digits should not match");
    }

    #[test]
    fn test_iban_redact() {
        let result = redact_text("IBAN: DE89370400440532013000");
        assert!(result.contains("[REDACTED-IBAN]"));
        assert!(!result.contains("DE89370400440532013000"));
    }

    #[test]
    fn test_iban_redact_preserves_invalid() {
        let result = redact_text("IBAN: DE00370400440532013000");
        assert!(result.contains("DE00370400440532013000"), "Invalid IBAN should be preserved");
    }

    #[test]
    fn test_uk_nino_detection() {
        let mut findings = Vec::new();
        scan_text("NI number: AB123456C", "test", &mut findings);
        let ninos: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::UkNino)
            .collect();
        assert_eq!(ninos.len(), 1);
        assert_eq!(ninos[0].text, "AB123456C");
    }

    #[test]
    fn test_uk_nino_with_spaces() {
        let mut findings = Vec::new();
        scan_text("NI: AB 12 34 56 C", "test", &mut findings);
        let ninos: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::UkNino)
            .collect();
        assert_eq!(ninos.len(), 1);
    }

    #[test]
    fn test_uk_nino_invalid_prefix() {
        let mut findings = Vec::new();
        // D is not allowed as first letter
        scan_text("NI: DA123456C", "test", &mut findings);
        let ninos: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::UkNino)
            .collect();
        assert!(ninos.is_empty(), "Invalid NINO prefix should not match");
    }

    #[test]
    fn test_uk_nino_redact() {
        let result = redact_text("NI number: AB123456C");
        assert!(result.contains("[REDACTED-UK-NINO]"));
        assert!(!result.contains("AB123456C"));
    }

    #[test]
    fn test_intl_phone_e164_detection() {
        let mut findings = Vec::new();
        // UK phone number
        scan_text("Phone: +442071234567", "test", &mut findings);
        let phones: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IntlPhone)
            .collect();
        assert_eq!(phones.len(), 1);
        assert_eq!(phones[0].text, "+442071234567");
    }

    #[test]
    fn test_intl_phone_with_separators() {
        let mut findings = Vec::new();
        // German phone with separators
        scan_text("Tel: +49 30 1234567", "test", &mut findings);
        let phones: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IntlPhone)
            .collect();
        assert_eq!(phones.len(), 1);
    }

    #[test]
    fn test_intl_phone_too_short() {
        let mut findings = Vec::new();
        // Only 5 digits total — too short for E.164
        scan_text("Code: +12345", "test", &mut findings);
        let phones: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IntlPhone)
            .collect();
        assert!(phones.is_empty(), "Too-short intl phone should not match");
    }

    #[test]
    fn test_intl_phone_redact() {
        let result = redact_text("Call: +442071234567");
        assert!(result.contains("[REDACTED-INTL-PHONE]"));
        assert!(!result.contains("+442071234567"));
    }

    #[test]
    fn test_ipv6_full_detection() {
        let mut findings = Vec::new();
        scan_text("Server: 2001:0db8:85a3:0000:0000:8a2e:0370:7334", "test", &mut findings);
        let ipv6s: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Ipv6Address)
            .collect();
        assert_eq!(ipv6s.len(), 1);
        assert_eq!(ipv6s[0].text, "2001:0db8:85a3:0000:0000:8a2e:0370:7334");
    }

    #[test]
    fn test_ipv6_abbreviated() {
        let mut findings = Vec::new();
        scan_text("Host: 2001:db8::1", "test", &mut findings);
        let ipv6s: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Ipv6Address)
            .collect();
        assert_eq!(ipv6s.len(), 1);
    }

    #[test]
    fn test_ipv6_loopback() {
        let mut findings = Vec::new();
        scan_text("Loopback: ::1", "test", &mut findings);
        let ipv6s: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Ipv6Address)
            .collect();
        assert_eq!(ipv6s.len(), 1);
    }

    #[test]
    fn test_ipv6_redact() {
        let result = redact_text("Server: 2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert!(result.contains("[REDACTED-IPV6-ADDRESS]"));
        assert!(!result.contains("2001:0db8:85a3:0000:0000:8a2e:0370:7334"));
    }

    #[test]
    fn test_eu_vat_detection() {
        let mut findings = Vec::new();
        // German VAT number
        scan_text("VAT: DE123456789", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "DE123456789");
    }

    #[test]
    fn test_eu_vat_fr() {
        let mut findings = Vec::new();
        // French VAT number
        scan_text("TVA: FR12345678901", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "FR12345678901");
    }

    #[test]
    fn test_eu_vat_non_eu_country() {
        let mut findings = Vec::new();
        // US is not an EU country code
        scan_text("VAT: US123456789", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert!(vats.is_empty(), "Non-EU country code should not match");
    }

    #[test]
    fn test_eu_vat_redact() {
        let result = redact_text("VAT: DE123456789");
        assert!(result.contains("[REDACTED-EU-VAT]"));
        assert!(!result.contains("DE123456789"));
    }

    #[test]
    fn test_is_valid_iban() {
        // Valid IBANs
        assert!(is_valid_iban("DE89370400440532013000"));
        assert!(is_valid_iban("GB29NWBK60161331926819"));
        assert!(is_valid_iban("FR7630006000011234567890189"));
        // Invalid check digits
        assert!(!is_valid_iban("DE00370400440532013000"));
        // Too short
        assert!(!is_valid_iban("DE89"));
    }

    #[test]
    fn test_is_valid_intl_phone() {
        assert!(is_valid_intl_phone("+442071234567")); // 12 digits
        assert!(is_valid_intl_phone("+49 30 1234567")); // 11 digits
        assert!(!is_valid_intl_phone("+123")); // only 3 digits
    }

    #[test]
    fn test_summarize_international() {
        let findings = vec![
            PiiFinding {
                kind: PiiKind::Iban,
                text: "DE89370400440532013000".into(),
                location: "test".into(),
                line_approx: 1,
            },
            PiiFinding {
                kind: PiiKind::UkNino,
                text: "AB123456C".into(),
                location: "test".into(),
                line_approx: 2,
            },
            PiiFinding {
                kind: PiiKind::Ipv6Address,
                text: "::1".into(),
                location: "test".into(),
                line_approx: 3,
            },
        ];
        let summary = summarize(&findings);
        assert!(summary.contains("found 3 items"));
        assert!(summary.contains("1 IBAN"));
        assert!(summary.contains("1 UK NINO"));
        assert!(summary.contains("1 IPv6 address"));
    }

    // -----------------------------------------------------------------------
    // Task 1: EU VAT validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_eu_vat_false_positive_english_words() {
        for word in &["DESIGN", "FRONT", "BELOW", "DEFEAT", "BESIDE", "BEFORE"] {
            let mut findings = Vec::new();
            scan_text(word, "test", &mut findings);
            let vats: Vec<_> = findings
                .iter()
                .filter(|f| f.kind == PiiKind::EuVat)
                .collect();
            assert!(vats.is_empty(), "{word} should NOT be detected as EU VAT");
        }
    }

    #[test]
    fn test_eu_vat_fr2024_not_matched() {
        let mut findings = Vec::new();
        scan_text("Published in FR2024", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert!(vats.is_empty(), "FR2024 should NOT match as EU VAT");
    }

    #[test]
    fn test_eu_vat_valid_de() {
        let mut findings = Vec::new();
        scan_text("VAT: DE123456789", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "DE123456789");
    }

    #[test]
    fn test_eu_vat_valid_fr_scan() {
        let mut findings = Vec::new();
        scan_text("VAT: FR12345678901", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "FR12345678901");
    }

    #[test]
    fn test_eu_vat_valid_it_scan() {
        let mut findings = Vec::new();
        scan_text("VAT: IT12345678901", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "IT12345678901");
    }

    #[test]
    fn test_eu_vat_valid_es_scan() {
        let mut findings = Vec::new();
        scan_text("VAT: ESA12345678", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert_eq!(vats.len(), 1);
        assert_eq!(vats[0].text, "ESA12345678");
    }

    #[test]
    fn test_eu_vat_de_wrong_length() {
        let mut findings = Vec::new();
        scan_text("VAT: DE12345678", "test", &mut findings);
        let vats: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::EuVat)
            .collect();
        assert!(vats.is_empty(), "DE with 8 digits should not match");
    }

    #[test]
    fn test_eu_vat_redact_validates() {
        let result = redact_text("The DESIGN is ready");
        assert!(result.contains("DESIGN"), "DESIGN should not be redacted as VAT");

        let result = redact_text("VAT: DE123456789");
        assert!(result.contains("[REDACTED-EU-VAT]"));
        assert!(!result.contains("DE123456789"));
    }

    #[test]
    fn test_is_valid_vat() {
        assert!(is_valid_vat("DE123456789"));
        assert!(is_valid_vat("FR12345678901"));
        assert!(is_valid_vat("IT12345678901"));
        assert!(is_valid_vat("ESA1234567B"));
        assert!(is_valid_vat("NL123456789B01"));

        assert!(!is_valid_vat("DE12345678"));
        assert!(!is_valid_vat("DE1234567890"));
        assert!(!is_valid_vat("FR2024"));
        assert!(!is_valid_vat("AB"));
    }

    // -----------------------------------------------------------------------
    // Task 2: Code block scan/redact asymmetry test
    // -----------------------------------------------------------------------

    #[test]
    fn test_scan_skips_code_blocks() {
        use crate::document::types::CodeBlock as CB;
        let mut doc = Document::new("/tmp/test.md", super::super::types::DocumentFormat::Markdown);
        let mut section = Section::root();

        section.content.push(ContentBlock::CodeBlock(CB {
            language: Some("text".into()),
            content: "SSN: 123-45-6789\nIP: 192.168.1.1".into(),
        }));
        section
            .content
            .push(ContentBlock::Paragraph("Email: alice@corp.com".into()));

        doc.sections.push(section);

        let findings = scan_document(&doc);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, PiiKind::Email);
    }

    // -----------------------------------------------------------------------
    // Task 3: US/intl phone deduplication test
    // -----------------------------------------------------------------------

    #[test]
    fn test_us_intl_phone_no_duplicate() {
        let mut findings = Vec::new();
        scan_text("Phone: +1-555-123-4567", "test", &mut findings);
        let phone_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::Phone || f.kind == PiiKind::IntlPhone)
            .collect();
        assert_eq!(
            phone_findings.len(),
            1,
            "Should have exactly one phone finding, got: {phone_findings:?}"
        );
        assert_eq!(
            phone_findings[0].kind,
            PiiKind::Phone,
            "Should keep the more specific US phone"
        );
    }

    #[test]
    fn test_intl_phone_kept_when_no_us_overlap() {
        let mut findings = Vec::new();
        scan_text("Phone: +442071234567", "test", &mut findings);
        let intl: Vec<_> = findings
            .iter()
            .filter(|f| f.kind == PiiKind::IntlPhone)
            .collect();
        assert_eq!(intl.len(), 1);
        assert_eq!(intl[0].text, "+442071234567");
    }

    #[test]
    fn test_dedup_preserves_non_overlapping() {
        let mut findings = Vec::new();
        scan_text("SSN: 123-45-6789 email: user@test.com", "test", &mut findings);
        assert!(findings.iter().any(|f| f.kind == PiiKind::Ssn));
        assert!(findings.iter().any(|f| f.kind == PiiKind::Email));
    }

    // Regression test for #124: IBAN validation must not panic on invalid UTF-8 or
    // large numeric strings. The old code used unwrap_or which silently masked errors.
    #[test]
    fn test_iban_validation_no_panic_on_edge_cases() {
        // Very long IBAN-like string that could cause parse overflow
        assert!(!is_valid_iban("DE89999999999999999999999999999999999"));
        // Minimum-length valid structure but wrong check digits
        assert!(!is_valid_iban("AB12C"));
        // All-digit IBAN (no letters in body) — should compute MOD-97 without panic
        assert!(!is_valid_iban("DE00000000000000000000"));
        // Valid IBAN still works after the fix
        assert!(is_valid_iban("DE89370400440532013000"));
        assert!(is_valid_iban("GB29NWBK60161331926819"));
    }
}
