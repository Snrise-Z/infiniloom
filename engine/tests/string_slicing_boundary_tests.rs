//! String slicing boundary tests
//!
//! These tests verify that string truncation operations handle UTF-8
//! boundaries correctly, preventing panics like the one fixed in
//! extraction.rs:148.
//!
//! Key patterns tested:
//! - BudgetEnforcer.truncate_to_tokens() with Unicode content
//! - Tokenizer.truncate_to_budget() with Unicode content
//! - Edge cases at exact byte boundaries
//! - Multi-byte characters at truncation points
//! - Various scripts and emoji

use infiniloom_engine::budget::{BudgetConfig, BudgetEnforcer, TruncationStrategy};
use infiniloom_engine::tokenizer::{TokenModel, Tokenizer};

// ============================================================================
// BudgetEnforcer UTF-8 Boundary Tests
// ============================================================================

#[test]
fn test_budget_enforcer_truncate_chinese_near_boundary() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Chinese characters are 3 bytes each
    // Create content where truncation boundary likely falls in middle of char
    let chinese = "这是一个测试字符串用于验证UTF8边界处理是否正确工作";

    for budget in [1, 2, 3, 5, 10, 15, 20] {
        let result = enforcer.truncate_to_tokens(chinese, budget);
        // Should not panic and should be valid UTF-8
        assert!(result.chars().count() > 0 || budget == 0);
        // Verify it's valid by iterating
        for c in result.chars() {
            let _ = c;
        }
    }
}

#[test]
fn test_budget_enforcer_truncate_japanese_hiragana() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Hiragana: 3 bytes per char
    let japanese = "あいうえおかきくけこさしすせそたちつてと";

    for budget in [1, 2, 3, 5, 8, 10] {
        let result = enforcer.truncate_to_tokens(japanese, budget);
        // Verify valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_korean() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Korean Hangul: 3 bytes per char
    let korean = "안녕하세요세계프로그래밍테스트입니다";

    for budget in [1, 2, 4, 6, 8, 12] {
        let result = enforcer.truncate_to_tokens(korean, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_emoji_4byte() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Emoji are 4 bytes each
    let emoji = "🎉🎊🎁🎂🎈🎀🎄🎃🎅🤶🦀🐍🦊🐸🦋";

    for budget in [1, 2, 3, 5, 8] {
        let result = enforcer.truncate_to_tokens(emoji, budget);
        // Must be valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_emoji_sequences() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Emoji with ZWJ sequences (can be 7-25 bytes per visual emoji)
    let emoji_sequences = "👨‍👩‍👧‍👦👨‍💻👩‍🔬🏳️‍🌈";

    for budget in [1, 2, 3, 5, 10] {
        let result = enforcer.truncate_to_tokens(emoji_sequences, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_arabic_rtl() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Arabic: 2-4 bytes per char with RTL
    let arabic = "مرحبا بالعالم هذا اختبار للغة العربية";

    for budget in [1, 2, 4, 6, 10] {
        let result = enforcer.truncate_to_tokens(arabic, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_hebrew() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Hebrew: 2 bytes per char
    let hebrew = "שלום עולם זהו מבחן לעברית";

    for budget in [1, 2, 3, 5, 8] {
        let result = enforcer.truncate_to_tokens(hebrew, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_cyrillic() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Cyrillic: 2 bytes per char
    let cyrillic = "Привет мир это тест для кириллицы и юникода";

    for budget in [1, 2, 3, 5, 10, 15] {
        let result = enforcer.truncate_to_tokens(cyrillic, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_thai() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Thai: 3 bytes per char with complex clusters
    let thai = "สวัสดีโลกนี่คือการทดสอบภาษาไทย";

    for budget in [1, 2, 4, 6, 10] {
        let result = enforcer.truncate_to_tokens(thai, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_mixed_scripts() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Mix of ASCII, Chinese, Cyrillic, and emoji
    let mixed = "Hello世界Привет🌍مرحباשלום";

    for budget in [1, 2, 3, 5, 8, 12] {
        let result = enforcer.truncate_to_tokens(mixed, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_combining_chars() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Combining diacritical marks
    // e + combining acute = é (2 code points, but visually 1 char)
    let combining = "cafe\u{0301} re\u{0301}sume\u{0301} nai\u{0308}ve";

    for budget in [1, 2, 3, 5, 8] {
        let result = enforcer.truncate_to_tokens(combining, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_truncate_at_exact_boundary() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Create content where we try to truncate exactly at various positions
    // 196 ASCII + 4-byte emoji = try to truncate at 197, 198, 199
    let mut content = String::new();
    for i in 0..196 {
        content.push((b'a' + (i % 26) as u8) as char);
    }
    content.push_str("🎉"); // 4-byte emoji

    // Try different budgets that might land in the middle of the emoji
    for budget in [45, 46, 47, 48, 49, 50, 51, 52] {
        let result = enforcer.truncate_to_tokens(&content, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_budget_enforcer_all_strategies_with_unicode() {
    // Test all truncation strategies with Unicode content
    let strategies = [
        TruncationStrategy::Line,
        TruncationStrategy::Semantic,
        TruncationStrategy::Hard,
    ];

    let content = "fn 函数名():\n    print('你好世界')\n\ndef 另一个函数():\n    pass";

    for strategy in strategies {
        let config = BudgetConfig {
            budget: 10000.into(),
            model: TokenModel::Claude,
            strategy,
            overhead_reserve: 100.into(),
        };
        let enforcer = BudgetEnforcer::new(config);

        for budget in [1, 2, 5, 10, 15] {
            let result = enforcer.truncate_to_tokens(content, budget);
            assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        }
    }
}

// ============================================================================
// Tokenizer truncate_to_budget UTF-8 Boundary Tests
// ============================================================================

#[test]
fn test_tokenizer_truncate_chinese() {
    let tokenizer = Tokenizer::new();

    let chinese = "这是一个很长的中文字符串用于测试分词器的截断功能是否正确处理UTF8边界";

    for budget in [1, 2, 5, 10, 20, 30] {
        let result = tokenizer.truncate_to_budget(chinese, TokenModel::Claude, budget);
        // Result is a slice, should be valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_tokenizer_truncate_japanese() {
    let tokenizer = Tokenizer::new();

    let japanese = "これは日本語のテスト文字列ですトークナイザーの動作を確認します";

    for budget in [1, 2, 5, 10, 15, 25] {
        let result = tokenizer.truncate_to_budget(japanese, TokenModel::Claude, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_tokenizer_truncate_emoji() {
    let tokenizer = Tokenizer::new();

    let emoji = "🦀🐍🦊🐸🦋🌸🌺🌹🌻🌼🪻🌷";

    for budget in [1, 2, 3, 5, 8] {
        let result = tokenizer.truncate_to_budget(emoji, TokenModel::Claude, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_tokenizer_truncate_openai_models() {
    let tokenizer = Tokenizer::new();

    let content = "Hello世界🌍مرحباПривет שלום";

    // Test with different OpenAI models (exact tokenization)
    let models = [
        TokenModel::Gpt52,
        TokenModel::Gpt51,
        TokenModel::Gpt5,
        TokenModel::O4Mini,
        TokenModel::O3,
        TokenModel::O1,
        TokenModel::Gpt4o,
        TokenModel::Gpt4oMini,
        TokenModel::Gpt4,
    ];

    for model in models {
        for budget in [1, 2, 5, 10] {
            let result = tokenizer.truncate_to_budget(content, model, budget);
            assert!(
                std::str::from_utf8(result.as_bytes()).is_ok(),
                "Invalid UTF-8 for model {:?} with budget {}",
                model,
                budget
            );
        }
    }
}

#[test]
fn test_tokenizer_truncate_non_openai_models() {
    let tokenizer = Tokenizer::new();

    let content = "Привет世界🌍Hello مرحبا";

    // Test with estimation-based models
    let models = [
        TokenModel::Claude,
        TokenModel::Gemini,
        TokenModel::Llama,
        TokenModel::Mistral,
        TokenModel::DeepSeek,
        TokenModel::Qwen,
        TokenModel::Cohere,
        TokenModel::Grok,
    ];

    for model in models {
        for budget in [1, 2, 5, 10] {
            let result = tokenizer.truncate_to_budget(content, model, budget);
            assert!(
                std::str::from_utf8(result.as_bytes()).is_ok(),
                "Invalid UTF-8 for model {:?} with budget {}",
                model,
                budget
            );
        }
    }
}

#[test]
fn test_tokenizer_truncate_zero_budget() {
    let tokenizer = Tokenizer::new();

    let content = "Some content 一些内容 🦀";

    let result = tokenizer.truncate_to_budget(content, TokenModel::Claude, 0);
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn test_tokenizer_truncate_large_budget() {
    let tokenizer = Tokenizer::new();

    let content = "Short 短い 짧은";

    // Budget larger than content
    let result = tokenizer.truncate_to_budget(content, TokenModel::Claude, 10000);
    assert_eq!(result, content);
}

#[test]
fn test_tokenizer_truncate_at_word_boundary() {
    let tokenizer = Tokenizer::new();

    // Content with spaces and Unicode
    let content = "Hello 世界 Привет мир";

    for budget in [1, 2, 3, 5, 8] {
        let result = tokenizer.truncate_to_budget(content, TokenModel::Claude, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        // Should try to break at word boundary (space)
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_string_truncation() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
    let tokenizer = Tokenizer::new();

    let empty = "";

    let result1 = enforcer.truncate_to_tokens(empty, 10);
    let result2 = tokenizer.truncate_to_budget(empty, TokenModel::Claude, 10);

    assert_eq!(result1, "");
    assert_eq!(result2, "");
}

#[test]
fn test_single_multibyte_char() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
    let tokenizer = Tokenizer::new();

    let single_chars = ["世", "🦀", "Ж", "א", "م"];

    for s in single_chars {
        let result1 = enforcer.truncate_to_tokens(s, 1);
        let result2 = tokenizer.truncate_to_budget(s, TokenModel::Claude, 1);

        assert!(std::str::from_utf8(result1.as_bytes()).is_ok());
        assert!(std::str::from_utf8(result2.as_bytes()).is_ok());
    }
}

#[test]
fn test_string_with_null_bytes() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // String with embedded null bytes (valid UTF-8)
    let with_nulls = "Hello\0World\0Test";

    let result = enforcer.truncate_to_tokens(with_nulls, 5);
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn test_very_long_unicode_string() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);
    let tokenizer = Tokenizer::new();

    // Very long Unicode string
    let long_string = "世界".repeat(10000);

    let result1 = enforcer.truncate_to_tokens(&long_string, 100);
    let result2 = tokenizer.truncate_to_budget(&long_string, TokenModel::Claude, 100);

    assert!(std::str::from_utf8(result1.as_bytes()).is_ok());
    assert!(std::str::from_utf8(result2.as_bytes()).is_ok());
    assert!(result1.len() < long_string.len());
    assert!(result2.len() < long_string.len());
}

#[test]
fn test_supplementary_plane_characters() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Characters from supplementary planes (4 bytes each)
    // Mathematical symbols, musical symbols, ancient scripts
    let supplementary = "𝄞𝄢𝅗𝅥𝅘𝅥𝅮𝅘𝅥𝅯𝆕"; // Musical symbols

    for budget in [1, 2, 3] {
        let result = enforcer.truncate_to_tokens(supplementary, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_devanagari_with_combining_marks() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Devanagari with combining vowel marks
    let devanagari = "नमस्ते दुनिया यह एक परीक्षण है";

    for budget in [1, 2, 3, 5, 10] {
        let result = enforcer.truncate_to_tokens(devanagari, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_tamil_script() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Tamil script - was involved in original crash at extraction.rs:148
    let tamil = "தமிழ் மொழி பரிசோதனை";

    for budget in [1, 2, 3, 5, 10] {
        let result = enforcer.truncate_to_tokens(tamil, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_khmer_script() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Khmer script - complex clusters
    let khmer = "ភាសាខ្មែរ ការធ្វើតេស្ត";

    for budget in [1, 2, 3, 5, 10] {
        let result = enforcer.truncate_to_tokens(khmer, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_myanmar_script() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Myanmar/Burmese script
    let myanmar = "မြန်မာဘာသာ စမ်းသပ်မှု";

    for budget in [1, 2, 3, 5] {
        let result = enforcer.truncate_to_tokens(myanmar, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_gujarati_script() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Gujarati script
    let gujarati = "ગુજરાતી ભાષા પરીક્ષણ";

    for budget in [1, 2, 3, 5, 10] {
        let result = enforcer.truncate_to_tokens(gujarati, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

// ============================================================================
// Code-like Content Tests
// ============================================================================

#[test]
fn test_code_with_unicode_identifiers() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Python code with Unicode identifiers
    let code = r#"
def 处理数据(输入参数):
    """处理数据的函数"""
    结果 = 输入参数 * 2
    return 结果

class 用户类:
    def __init__(self, 姓名):
        self.姓名 = 姓名
"#;

    for budget in [5, 10, 15, 20, 30] {
        let result = enforcer.truncate_to_tokens(code, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_code_with_unicode_strings() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Rust code with Unicode string literals
    let code = r#"
fn main() {
    let greeting = "Hello, 世界! 🦀";
    println!("{}", greeting);

    let russian = "Привет, мир!";
    let arabic = "مرحبا بالعالم";
}
"#;

    for budget in [5, 10, 20, 30] {
        let result = enforcer.truncate_to_tokens(code, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_code_with_unicode_comments() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // JavaScript code with Unicode comments
    let code = r#"
// 这是一个中文注释
function greet(name) {
    // Приветствие на русском
    console.log(`Hello, ${name}! 🌍`);
    // مرحبا
    return true;
}
"#;

    for budget in [5, 10, 15, 25] {
        let result = enforcer.truncate_to_tokens(code, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_many_different_scripts() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Content with many different scripts
    let multi_script = concat!(
        "English ",
        "中文 ",
        "日本語 ",
        "한국어 ",
        "العربية ",
        "עברית ",
        "Русский ",
        "Ελληνικά ",
        "हिन्दी ",
        "ไทย ",
        "தமிழ் ",
        "🌍🦀🎉"
    );

    for budget in [1, 2, 3, 5, 8, 12, 20] {
        let result = enforcer.truncate_to_tokens(multi_script, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_alternating_ascii_unicode() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Rapidly alternating between ASCII and multi-byte
    let alternating = "a世b界c日d本e語f한g국h어i";

    for budget in [1, 2, 3, 5, 8, 12] {
        let result = enforcer.truncate_to_tokens(alternating, budget);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }
}

#[test]
fn test_random_byte_counts() {
    let enforcer = BudgetEnforcer::with_budget(10000, TokenModel::Claude);

    // Mix of 1, 2, 3, and 4-byte characters
    // ASCII (1) + Cyrillic (2) + CJK (3) + Emoji (4)
    let mixed = "aБ中🎉bЖ日🦀cИ語🌍dЯ界🎊";

    for budget in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        let result = enforcer.truncate_to_tokens(mixed, budget);
        assert!(
            std::str::from_utf8(result.as_bytes()).is_ok(),
            "Failed at budget {}",
            budget
        );
    }
}
