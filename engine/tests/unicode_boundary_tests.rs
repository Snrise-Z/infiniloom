//! Unicode boundary and edge case tests
//!
//! These tests are designed to catch bugs similar to the Unicode crash at extraction.rs:148
//! where string slicing at invalid UTF-8 boundaries caused panics.
//!
//! Key patterns tested:
//! - Multi-byte UTF-8 characters near signature extraction boundaries
//! - Very long identifiers in different scripts
//! - Mixed script code with Unicode identifiers
//! - Files with Unicode file names
//! - Combining characters and diacritics in code

use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::SymbolKind;

// ============================================================================
// Tests for extraction.rs Unicode safety
// ============================================================================

#[test]
fn test_signature_extraction_near_200_byte_boundary_tamil() {
    // Tamil characters are 3 bytes each in UTF-8
    // Create code where the 200-byte boundary falls inside a Tamil character
    let mut parser = Parser::new();

    // Tamil comment that's about 198 bytes, then a function
    // Each Tamil char is 3 bytes, so 66 chars ≈ 198 bytes
    let tamil_comment = "தமிழ்".repeat(40); // 40 * 15 bytes = 600 bytes
    let code = format!(
        "# {}\ndef test_func():\n    pass\n",
        &tamil_comment[..198.min(tamil_comment.len())]
    );

    // Should not panic
    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle Tamil near 200-byte boundary");
}

#[test]
fn test_signature_extraction_near_200_byte_boundary_japanese() {
    let mut parser = Parser::new();

    // Hiragana/Katakana are 3 bytes each
    let japanese = "あいうえおかきくけこ".repeat(10); // 30 chars * 3 bytes * 10 = 900 bytes
    let code = format!(
        "# {}\ndef テスト関数():\n    pass\n",
        &japanese[..198.min(japanese.len())]
    );

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle Japanese near boundary");
}

#[test]
fn test_signature_extraction_near_200_byte_boundary_thai() {
    let mut parser = Parser::new();

    // Thai characters are typically 3 bytes
    let thai = "กขคงจฉชซฌญฎฏ".repeat(20);
    let code = format!(
        "# {}\ndef ฟังก์ชัน():\n    pass\n",
        &thai[..198.min(thai.len())]
    );

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle Thai near boundary");
}

#[test]
fn test_signature_extraction_near_200_byte_boundary_khmer() {
    let mut parser = Parser::new();

    // Khmer characters are 3 bytes
    let khmer = "កខគឃងចឆជឈញ".repeat(20);
    let code = format!(
        "# {}\ndef អនុគមន៍():\n    pass\n",
        &khmer[..198.min(khmer.len())]
    );

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle Khmer near boundary");
}

#[test]
fn test_signature_extraction_near_200_byte_boundary_emoji() {
    let mut parser = Parser::new();

    // Emoji are 4 bytes each
    let emoji = "🎉🎊🎁🎂🎈🎀🎄🎃🎅🤶".repeat(5); // 50 chars * 4 bytes = 200 bytes
    let code = format!(
        "# {}\ndef emoji_func():\n    pass\n",
        emoji
    );

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle emoji near boundary");
}

#[test]
fn test_signature_extraction_exact_200_byte_multibyte_boundary() {
    let mut parser = Parser::new();

    // Create exactly 200 bytes ending in middle of a 4-byte emoji
    // 196 ASCII chars + first 2 bytes of a 4-byte emoji would be invalid
    // Instead we'll test that our code handles truncation safely
    let mut content = String::new();
    for i in 0..196 {
        content.push((b'a' + (i % 26) as u8) as char);
    }
    content.push_str("🎉"); // 4-byte emoji

    let code = format!("def f(): # {}\n    pass", content);

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle exact 200-byte boundary safely");
}

#[test]
fn test_very_long_unicode_function_name() {
    let mut parser = Parser::new();

    // Very long function name with Unicode
    let long_name = "функция_".repeat(50); // 400+ bytes

    // Use safe char boundary slicing (same pattern as our fix in extraction.rs)
    let target_len = 200.min(long_name.len());
    let mut safe_len = target_len;
    while safe_len > 0 && !long_name.is_char_boundary(safe_len) {
        safe_len -= 1;
    }

    let code = format!(
        "def {}():\n    pass\n",
        &long_name[..safe_len]
    );

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle very long Unicode function names");
}

#[test]
fn test_chinese_function_with_long_body() {
    let mut parser = Parser::new();

    let code = r#"
def 处理数据函数(数据参数一, 数据参数二, 数据参数三):
    """
    这是一个很长的文档字符串，包含很多中文字符。
    用于测试解析器是否能正确处理中文代码。
    参数说明：
    - 数据参数一：第一个参数
    - 数据参数二：第二个参数
    - 数据参数三：第三个参数
    """
    结果 = 数据参数一 + 数据参数二
    return 结果
"#;

    let symbols = parser.parse(code, Language::Python).unwrap();
    assert!(symbols.iter().any(|s| s.name.contains("处理数据函数") || s.name == "处理数据函数"));
}

#[test]
fn test_mixed_script_identifiers() {
    let mut parser = Parser::new();

    // Mix of Latin, Greek, and Cyrillic (all look similar but different scripts)
    let code = r#"
def аbc():  # Cyrillic 'а'
    pass

def αβγ():  # Greek
    pass

def abc():  # Latin
    pass
"#;

    let symbols = parser.parse(code, Language::Python).unwrap();
    let funcs: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    // Should find all three functions
    assert_eq!(funcs.len(), 3, "Should find all three mixed-script functions");
}

// ============================================================================
// Tests for combining characters and diacritics
// ============================================================================

#[test]
fn test_combining_characters_in_strings() {
    let mut parser = Parser::new();

    // NFC vs NFD forms
    let code = r#"
def test():
    # Precomposed: café (4 chars, 5 bytes)
    s1 = "café"
    # Decomposed: cafe + combining acute (5 chars, 6 bytes)
    s2 = "cafe\u0301"
    return s1, s2
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle combining characters");
}

#[test]
fn test_zero_width_characters_in_identifiers() {
    let mut parser = Parser::new();

    // Zero-width joiner/non-joiner in different positions
    let code = r#"
def normal_func():
    pass

# Note: ZWJ/ZWNJ in identifiers may cause issues
message = "Hello\u200DWorld"  # ZWJ
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle zero-width characters");
}

// ============================================================================
// Tests for RTL scripts
// ============================================================================

#[test]
fn test_arabic_in_comments() {
    let mut parser = Parser::new();

    let code = r#"
# هذا تعليق بالعربية
def greet():
    # مرحبا بالعالم
    return "Hello"
"#;

    let symbols = parser.parse(code, Language::Python).unwrap();
    assert!(symbols.iter().any(|s| s.name == "greet"));
}

#[test]
fn test_hebrew_in_strings() {
    let mut parser = Parser::new();

    let code = r#"
def test():
    # Comment with שלום
    message = "שלום עולם"
    return message
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle Hebrew");
}

#[test]
fn test_bidi_text_mixed() {
    let mut parser = Parser::new();

    // Mixed left-to-right and right-to-left
    let code = r#"
def test():
    english = "Hello"
    arabic = "مرحبا"
    mixed = "Hello مرحبا World"
    return mixed
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle mixed bidi text");
}

// ============================================================================
// Tests for edge cases in different languages
// ============================================================================

#[test]
fn test_rust_unicode_doc_comments() {
    let mut parser = Parser::new();

    let code = r#"
/// Документация на русском языке
///
/// # Примеры использования
///
/// ```rust
/// let результат = функция();
/// ```
fn функция() -> String {
    "Привет мир!".to_owned()
}
"#;

    let symbols = parser.parse(code, Language::Rust).unwrap();
    // Should find the function even with Cyrillic name
    assert!(!symbols.is_empty(), "Should parse Rust with Unicode docs");
}

#[test]
fn test_javascript_template_literals_unicode() {
    let mut parser = Parser::new();

    let code = r#"
function greet(name) {
    // 你好世界
    return `Hello, ${name}! 🌍 مرحبا`;
}

const 变量 = "中文变量名";
"#;

    let result = parser.parse(code, Language::JavaScript);
    assert!(result.is_ok(), "Should handle JS template literals with Unicode");
}

#[test]
fn test_typescript_interface_unicode_properties() {
    let mut parser = Parser::new();

    let code = r#"
interface 用户 {
    名前: string;
    メール?: string;
    возраст: number;
}

function 获取用户(id: number): 用户 {
    return { 名前: "Test", возраст: 25 };
}
"#;

    let result = parser.parse(code, Language::TypeScript);
    assert!(result.is_ok(), "Should handle TypeScript with Unicode");
}

#[test]
fn test_go_unicode_struct_fields() {
    let mut parser = Parser::new();

    let code = r#"
package main

// Структура с русскими комментариями
type 用户 struct {
    名前    string
    возраст int
}

func 新建用户(名前 string) *用户 {
    return &用户{名前: 名前}
}
"#;

    let result = parser.parse(code, Language::Go);
    assert!(result.is_ok(), "Should handle Go with Unicode");
}

// ============================================================================
// Tests for boundary conditions in signature extraction
// ============================================================================

#[test]
fn test_function_with_exactly_200_byte_signature() {
    let mut parser = Parser::new();

    // Create a function with a signature that's exactly 200 bytes
    let params = (0..20)
        .map(|i| format!("param_{:03}", i))
        .collect::<Vec<_>>()
        .join(", ");

    let code = format!("def func_with_many_params({}):\n    pass", params);

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle 200-byte signature");
}

#[test]
fn test_function_with_201_byte_unicode_signature() {
    let mut parser = Parser::new();

    // Create a function where the 201st byte is in the middle of a Unicode char
    let mut sig = String::new();
    for i in 0..66 {
        sig.push_str(&format!("п{}", i % 10)); // Cyrillic п is 2 bytes
    }

    let code = format!("def func({}):\n    pass", sig);

    let result = parser.parse(&code, Language::Python);
    assert!(result.is_ok(), "Should handle 201-byte Unicode signature");
}

#[test]
fn test_very_long_signature_with_defaults() {
    let mut parser = Parser::new();

    // Long default values with Unicode
    let code = r#"
def func(
    a="这是一个很长的默认值字符串，用于测试解析器的行为",
    b="これは日本語のデフォルト値です",
    c="Это русский текст по умолчанию"
):
    pass
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle long Unicode default values");
}

// ============================================================================
// Stress tests for parser stability
// ============================================================================

#[test]
fn test_many_small_unicode_functions() {
    let mut parser = Parser::new();

    // Many small functions with Unicode names
    let mut code = String::new();
    for i in 0..100 {
        code.push_str(&format!("def функция_{}():\n    pass\n\n", i));
    }

    let symbols = parser.parse(&code, Language::Python).unwrap();
    let func_count = symbols.iter().filter(|s| s.kind == SymbolKind::Function).count();
    assert_eq!(func_count, 100, "Should find all 100 Unicode functions");
}

#[test]
fn test_alternating_script_functions() {
    let mut parser = Parser::new();

    let code = r#"
def english_func():
    pass

def 中文函数():
    pass

def функция_рус():
    pass

def 日本語関数():
    pass

def 한국어함수():
    pass

def ελληνική():
    pass
"#;

    let symbols = parser.parse(code, Language::Python).unwrap();
    let func_count = symbols.iter().filter(|s| s.kind == SymbolKind::Function).count();
    assert_eq!(func_count, 6, "Should find all 6 multi-script functions");
}

#[test]
fn test_unicode_in_all_positions() {
    let mut parser = Parser::new();

    // Unicode in function name, parameters, docstring, body, and return type hint
    let code = r#"
def обработка_данных(параметр: str = "默认值") -> str:
    """
    Документация: 処理データ関数

    Args:
        параметр: 입력 데이터

    Returns:
        처리된 文字列
    """
    результат = параметр + "世界"
    return результат
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should handle Unicode in all positions");
}

// ============================================================================
// Regression tests for specific bug patterns
// ============================================================================

#[test]
fn test_issue_1_unicode_crash_tamil() {
    // Specific reproduction case for the Tamil crash
    let mut parser = Parser::new();

    let code = "def தமிழ்_செயல்பாடு(): pass";

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should not crash on Tamil function names");
}

#[test]
fn test_issue_1_unicode_crash_khmer() {
    // Specific reproduction case for Khmer
    let mut parser = Parser::new();

    let code = "def អនុគមន៍_ខ្មែរ(): pass";

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should not crash on Khmer function names");
}

#[test]
fn test_issue_1_unicode_crash_turkish() {
    let mut parser = Parser::new();

    // Turkish has special casing for i/İ and ı/I
    let code = r#"
def türkçe_fonksiyon():
    değişken = "Merhaba"
    return değişken
"#;

    let result = parser.parse(code, Language::Python);
    assert!(result.is_ok(), "Should not crash on Turkish identifiers");
}

// ============================================================================
// Line number accuracy with Unicode
// ============================================================================

#[test]
fn test_line_numbers_accuracy_unicode_heavy() {
    let mut parser = Parser::new();

    let code = r#"# -*- coding: utf-8 -*-
# 中文注释第一行
# 日本語コメント

def 第一个函数():
    pass

# 更多中文

def 第二个函数():
    pass
"#;

    let symbols = parser.parse(code, Language::Python).unwrap();

    // Find the functions
    let funcs: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();

    assert_eq!(funcs.len(), 2, "Should find both functions");

    // First function should be on line 5
    if let Some(f1) = funcs.iter().find(|s| s.name.contains("第一个")) {
        assert_eq!(f1.start_line, 5, "First function should be on line 5");
    }

    // Second function should be on line 10
    if let Some(f2) = funcs.iter().find(|s| s.name.contains("第二个")) {
        assert_eq!(f2.start_line, 10, "Second function should be on line 10");
    }
}
