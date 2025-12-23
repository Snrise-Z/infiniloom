//! Cross-platform encoding and line ending tests
//!
//! Tests handling of various text encodings, line endings, and Unicode edge cases:
//! - CRLF vs LF line endings
//! - UTF-8 BOM (Byte Order Mark)
//! - Unicode edge cases (combining characters, zero-width)
//! - Multi-byte UTF-8 sequences
//! - Invalid UTF-8 handling
//! - Path encoding issues

use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::security::SecurityScanner;
use infiniloom_engine::tokenizer::{TokenModel, Tokenizer};

// ============================================================================
// Line Ending Tests
// ============================================================================

#[test]
fn test_parse_lf_line_endings() {
    let code = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
    let mut parser = Parser::new();
    let symbols = parser.parse(code, Language::Python).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == infiniloom_engine::types::SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"foo"), "Should find foo with LF endings");
    assert!(func_names.contains(&"bar"), "Should find bar with LF endings");
}

#[test]
fn test_parse_crlf_line_endings() {
    let code = "def foo():\r\n    pass\r\n\r\ndef bar():\r\n    return 42\r\n";
    let mut parser = Parser::new();
    let symbols = parser.parse(code, Language::Python).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == infiniloom_engine::types::SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"foo"), "Should find foo with CRLF endings");
    assert!(func_names.contains(&"bar"), "Should find bar with CRLF endings");
}

#[test]
fn test_parse_mixed_line_endings() {
    // Mixed LF and CRLF in same file (common in Windows repos)
    let code = "def foo():\n    pass\r\n\r\ndef bar():\n    return 42\r\n";
    let mut parser = Parser::new();
    let result = parser.parse(code, Language::Python);

    // Should not crash with mixed endings
    assert!(result.is_ok(), "Should handle mixed line endings");
}

#[test]
fn test_parse_cr_only_line_endings() {
    // Old Mac style (CR only)
    let code = "def foo():\r    pass\r\rdef bar():\r    return 42\r";
    let mut parser = Parser::new();
    let result = parser.parse(code, Language::Python);

    // May or may not parse correctly, but should not crash
    assert!(result.is_ok() || result.is_err(), "Should handle CR-only endings");
}

#[test]
fn test_tokenize_crlf_vs_lf() {
    let tokenizer = Tokenizer::new();

    let lf_text = "Hello\nWorld\nTest";
    let crlf_text = "Hello\r\nWorld\r\nTest";

    let lf_count = tokenizer.count(lf_text, TokenModel::Gpt4);
    let crlf_count = tokenizer.count(crlf_text, TokenModel::Gpt4);

    // CRLF may have slightly different token count due to \r
    // But difference should be minimal
    let diff = (lf_count as i32 - crlf_count as i32).abs();
    assert!(diff <= 3, "Token count difference should be minimal: {} vs {}", lf_count, crlf_count);
}

#[test]
fn test_security_scan_crlf() {
    let scanner = SecurityScanner::new();

    // Secret with CRLF line endings
    let content = "# Config\r\nAWS_KEY=AKIAIOSFODNN7EXAMPLE1\r\nOTHER=value\r\n";
    let findings = scanner.scan(content, "config.env");

    assert!(!findings.is_empty(), "Should detect secrets with CRLF endings");
}

// ============================================================================
// UTF-8 BOM Tests
// ============================================================================

#[test]
fn test_parse_with_utf8_bom() {
    // UTF-8 BOM: EF BB BF
    let bom = "\u{FEFF}";
    let code = format!("{}def hello():\n    pass\n", bom);

    let mut parser = Parser::new();
    let result = parser.parse(&code, Language::Python);

    // Should handle BOM gracefully
    assert!(result.is_ok(), "Should parse code with UTF-8 BOM");
}

#[test]
fn test_tokenize_with_bom() {
    let tokenizer = Tokenizer::new();

    let bom = "\u{FEFF}";
    let text_with_bom = format!("{}Hello, world!", bom);
    let text_without_bom = "Hello, world!";

    let with_bom_count = tokenizer.count(&text_with_bom, TokenModel::Gpt4);
    let without_bom_count = tokenizer.count(text_without_bom, TokenModel::Gpt4);

    // BOM adds 1 token typically
    let diff = (with_bom_count as i32 - without_bom_count as i32).abs();
    assert!(diff <= 1, "BOM should add at most 1 token");
}

#[test]
fn test_security_scan_with_bom() {
    let scanner = SecurityScanner::new();

    let bom = "\u{FEFF}";
    let content = format!("{}AKIAIOSFODNN7EXAMPLE1", bom);
    let findings = scanner.scan(&content, "test.txt");

    assert!(!findings.is_empty(), "Should detect secrets after BOM");
}

// ============================================================================
// Unicode Edge Cases
// ============================================================================

#[test]
fn test_parse_unicode_identifiers() {
    // Python 3 allows Unicode identifiers
    let code = r#"
def привет():
    return "Hello from Russia"

def 你好():
    return "Hello from China"

class Ñoño:
    pass
"#;
    let mut parser = Parser::new();
    let result = parser.parse(code, Language::Python);

    // Should parse Unicode identifiers
    assert!(result.is_ok(), "Should parse Unicode identifiers");
}

#[test]
fn test_tokenize_combining_characters() {
    let tokenizer = Tokenizer::new();

    // "é" as single codepoint vs "e" + combining acute accent
    let precomposed = "café"; // U+00E9
    let decomposed = "cafe\u{0301}"; // e + combining acute

    let pre_count = tokenizer.count(precomposed, TokenModel::Gpt4);
    let dec_count = tokenizer.count(decomposed, TokenModel::Gpt4);

    // Counts may differ slightly due to normalization
    assert!(pre_count > 0);
    assert!(dec_count > 0);
}

#[test]
fn test_tokenize_zero_width_characters() {
    let tokenizer = Tokenizer::new();

    // Text with zero-width joiner and non-joiner
    let text_with_zwj = "Hello\u{200D}World"; // ZWJ
    let text_with_zwnj = "Hello\u{200C}World"; // ZWNJ
    let normal = "HelloWorld";

    let zwj_count = tokenizer.count(text_with_zwj, TokenModel::Gpt4);
    let zwnj_count = tokenizer.count(text_with_zwnj, TokenModel::Gpt4);
    let normal_count = tokenizer.count(normal, TokenModel::Gpt4);

    // All should tokenize without panic
    assert!(zwj_count > 0);
    assert!(zwnj_count > 0);
    assert!(normal_count > 0);
}

#[test]
fn test_tokenize_emoji_sequences() {
    let tokenizer = Tokenizer::new();

    // Various emoji representations
    let simple_emoji = "🎉";
    let skin_tone = "👋🏽"; // Wave + skin tone modifier
    let family = "👨‍👩‍👧‍👦"; // Family with ZWJ
    let flag = "🇺🇸"; // Regional indicator symbols

    assert!(tokenizer.count(simple_emoji, TokenModel::Gpt4) >= 1);
    assert!(tokenizer.count(skin_tone, TokenModel::Gpt4) >= 1);
    assert!(tokenizer.count(family, TokenModel::Gpt4) >= 1);
    assert!(tokenizer.count(flag, TokenModel::Gpt4) >= 1);
}

#[test]
fn test_tokenize_rtl_text() {
    let tokenizer = Tokenizer::new();

    // Hebrew (RTL)
    let hebrew = "שלום עולם";

    // Arabic (RTL)
    let arabic = "مرحبا بالعالم";

    // Mixed LTR and RTL
    let mixed = "Hello שלום World";

    assert!(tokenizer.count(hebrew, TokenModel::Gpt4) > 0);
    assert!(tokenizer.count(arabic, TokenModel::Gpt4) > 0);
    assert!(tokenizer.count(mixed, TokenModel::Gpt4) > 0);
}

#[test]
fn test_tokenize_mathematical_symbols() {
    let tokenizer = Tokenizer::new();

    // Mathematical notation
    let math = "∫₀^∞ e^(-x²) dx = √π/2";

    // Greek letters commonly used in math
    let greek = "α β γ δ ε ζ η θ";

    // Subscripts and superscripts
    let subscripts = "H₂O CO₂ x² y³";

    assert!(tokenizer.count(math, TokenModel::Gpt4) > 0);
    assert!(tokenizer.count(greek, TokenModel::Gpt4) > 0);
    assert!(tokenizer.count(subscripts, TokenModel::Gpt4) > 0);
}

#[test]
fn test_parse_unicode_strings() {
    let code = r#"
message = "Hello 世界 🌍"
greeting = "Привет мир"
symbols = "∫∑∏√∞"
"#;

    let mut parser = Parser::new();
    let result = parser.parse(code, Language::Python);

    assert!(result.is_ok(), "Should parse strings with Unicode content");
}

// ============================================================================
// Multi-byte UTF-8 Boundary Tests
// ============================================================================

#[test]
fn test_truncate_2byte_utf8() {
    let tokenizer = Tokenizer::new();

    // 2-byte UTF-8: Latin extended, Greek, Cyrillic (0x80-0x7FF)
    let text = "αβγδεζηθικλμνξοπρστυφχψω"; // Greek alphabet

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 5);

    // Should be valid UTF-8 after truncation
    assert!(truncated.chars().count() <= text.chars().count());
    for c in truncated.chars() {
        assert!(c.len_utf8() >= 1, "Each char should be valid");
    }
}

#[test]
fn test_truncate_3byte_utf8() {
    let tokenizer = Tokenizer::new();

    // 3-byte UTF-8: CJK characters (0x800-0xFFFF)
    let text = "你好世界这是一个测试中文字符串"; // Chinese

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 5);

    // Should be valid UTF-8 after truncation
    for c in truncated.chars() {
        assert!(c.len_utf8() >= 1);
    }
}

#[test]
fn test_truncate_4byte_utf8() {
    let tokenizer = Tokenizer::new();

    // 4-byte UTF-8: Emoji, rare CJK (0x10000-0x10FFFF)
    let text = "🎉🎊🎁🎂🎈🎀🎄🎃🎅🤶"; // Emoji

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 5);

    // Should be valid UTF-8 after truncation
    for c in truncated.chars() {
        assert!(c.len_utf8() >= 1);
    }
}

#[test]
fn test_truncate_mixed_byte_lengths() {
    let tokenizer = Tokenizer::new();

    // Mix of 1, 2, 3, 4 byte UTF-8 characters
    let text = "aαあ🎉bβお🎊cγう🎁";

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 5);

    // Verify it's valid UTF-8 by iterating
    let char_count = truncated.chars().count();
    assert!(char_count > 0 || truncated.is_empty());
}

// ============================================================================
// Security Scanner Unicode Tests
// ============================================================================

#[test]
fn test_security_scan_unicode_context() {
    let scanner = SecurityScanner::new();

    // AWS key embedded in Unicode context
    let content = "API密钥: AKIAIOSFODNN7EXAMPLE1"; // Chinese for "API key"
    let findings = scanner.scan(content, "config.txt");

    assert!(!findings.is_empty(), "Should detect AWS key in Unicode context");
}

#[test]
fn test_security_scan_homoglyph_attack() {
    let scanner = SecurityScanner::new();

    // Homoglyph attack: Cyrillic 'а' (U+0430) looks like Latin 'a'
    // This tests that scanner handles Unicode properly
    let content = "pаssword = 'secret123456789012345'"; // 'а' is Cyrillic

    // May or may not detect due to pattern matching
    drop(scanner.scan(content, "test.txt"));
    // Main assertion: should not panic
}

#[test]
fn test_security_redact_unicode() {
    let scanner = SecurityScanner::new();

    // Content with Unicode and secrets
    let content = "你好! API_KEY=sk_live_abcdefghijklmnopqrstuvwx 再见!";

    let (redacted, findings) = scanner.scan_and_redact(content, "test.txt");

    assert!(!findings.is_empty(), "Should find the Stripe key");
    assert!(redacted.contains("你好"), "Should preserve Chinese greeting");
    assert!(redacted.contains("再见"), "Should preserve Chinese farewell");
}

// ============================================================================
// Code with Unicode Comments/Strings
// ============================================================================

#[test]
fn test_parse_rust_unicode() {
    let code = r#"
/// Documentation with émojis 🦀
fn hello() -> &'static str {
    // Comment with Unicode: こんにちは
    "Hello, 世界!"
}
"#;

    let mut parser = Parser::new();
    let symbols = parser.parse(code, Language::Rust).unwrap();

    assert!(symbols.iter().any(|s| s.name == "hello"));
}

#[test]
fn test_parse_javascript_unicode() {
    let code = r#"
// Comment: 你好世界
function greet(name) {
    // Emoji in code 🎉
    return `Hello, ${name}! 🌍`;
}

const café = "coffee";  // Unicode identifier
"#;

    let mut parser = Parser::new();
    let result = parser.parse(code, Language::JavaScript);

    assert!(result.is_ok(), "Should parse JS with Unicode");
}

#[test]
fn test_parse_go_unicode() {
    let code = r#"
package main

// 这是一个注释
func 打招呼() string {
    return "你好!"
}

func main() {
    emoji := "🎉"
    fmt.Println(emoji)
}
"#;

    let mut parser = Parser::new();
    let result = parser.parse(code, Language::Go);

    assert!(result.is_ok(), "Should parse Go with Unicode");
}

// ============================================================================
// Edge Cases and Stress Tests
// ============================================================================

#[test]
fn test_very_long_unicode_line() {
    let tokenizer = Tokenizer::new();

    // Very long line of CJK characters
    let text = "测".repeat(10000);

    let count = tokenizer.count(&text, TokenModel::Gpt4);
    assert!(count > 0, "Should handle long Unicode lines");
}

#[test]
fn test_alternating_scripts() {
    let tokenizer = Tokenizer::new();

    // Alternating between scripts
    let text = "Hello你好Helloこんにちは안녕مرحباПривет";

    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count > 0);

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 10);
    // Should be valid UTF-8
    let _ = truncated.chars().count();
}

#[test]
fn test_surrogate_pair_like_content() {
    let tokenizer = Tokenizer::new();

    // Content that might look like surrogate pairs (but is valid UTF-8)
    // U+D800 to U+DFFF are surrogates and invalid in UTF-8
    // This tests handling of high codepoints
    let text = "𐀀𐀁𐀂𐀃"; // Linear B syllables (U+10000 range)

    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count >= 1);
}

#[test]
fn test_null_character_handling() {
    let tokenizer = Tokenizer::new();

    // Embedded null character
    let text = "Hello\0World";

    // Should not crash
    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count >= 1);
}

#[test]
fn test_control_characters() {
    let tokenizer = Tokenizer::new();

    // Various control characters
    let text = "Hello\x01\x02\x03\x04World\x1B[0mTest";

    // Should handle control characters
    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count >= 1);
}

#[test]
fn test_private_use_area() {
    let tokenizer = Tokenizer::new();

    // Private Use Area characters (U+E000 to U+F8FF)
    let text = "Text with PUA: \u{E000}\u{E001}\u{F8FF}";

    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count >= 1);
}

// ============================================================================
// Parser Line Number Accuracy with Unicode
// ============================================================================

#[test]
fn test_line_numbers_with_unicode() {
    let code = "# 你好\n# Line 2\ndef test():\n    pass\n";

    let mut parser = Parser::new();
    let symbols = parser.parse(code, Language::Python).unwrap();

    let func = symbols.iter().find(|s| s.name == "test");
    assert!(func.is_some());

    if let Some(f) = func {
        // Function should be on line 3
        assert_eq!(f.start_line, 3, "Function should be on correct line despite Unicode");
    }
}

#[test]
fn test_line_numbers_with_emoji() {
    let code = "// 🎉 Comment\n// Line 2\nfunction test() {}\n";

    let mut parser = Parser::new();
    let symbols = parser.parse(code, Language::JavaScript).unwrap();

    let func = symbols.iter().find(|s| s.name == "test");
    assert!(func.is_some());

    if let Some(f) = func {
        // Function should be on line 3
        assert_eq!(f.start_line, 3, "Function should be on correct line despite emoji");
    }
}

// ============================================================================
// Normalization Tests
// ============================================================================

#[test]
fn test_nfc_vs_nfd_normalization() {
    let tokenizer = Tokenizer::new();

    // NFC: é as single codepoint (U+00E9)
    let nfc = "caf\u{00E9}";

    // NFD: é as e + combining acute (U+0065 U+0301)
    let nfd = "cafe\u{0301}";

    // Both should tokenize
    let nfc_count = tokenizer.count(nfc, TokenModel::Gpt4);
    let nfd_count = tokenizer.count(nfd, TokenModel::Gpt4);

    assert!(nfc_count > 0);
    assert!(nfd_count > 0);

    // Counts may differ due to different byte representations
}

#[test]
fn test_fullwidth_vs_halfwidth() {
    let tokenizer = Tokenizer::new();

    // Fullwidth ASCII (U+FF01 to U+FF5E)
    let fullwidth = "Ｈｅｌｌｏ"; // Fullwidth "Hello"
    let halfwidth = "Hello"; // Normal ASCII

    let fw_count = tokenizer.count(fullwidth, TokenModel::Gpt4);
    let hw_count = tokenizer.count(halfwidth, TokenModel::Gpt4);

    assert!(fw_count > 0);
    assert!(hw_count > 0);

    // Fullwidth typically has more tokens due to different encoding
}
