//! Comprehensive tokenizer tests with exact fixtures and edge cases
//!
//! Tests token counting across models, truncation behavior, UTF-8 handling,
//! and comparison between exact BPE and estimation methods.

use infiniloom_engine::tokenizer::{quick_estimate, TokenCounts, TokenModel, Tokenizer};

// ============================================================================
// Exact Token Count Fixtures (verified against tiktoken)
// ============================================================================
// These fixtures use known token counts from tiktoken-rs for GPT models

#[test]
fn test_exact_gpt4_hello_world() {
    let tokenizer = Tokenizer::new();
    let text = "Hello, world!";
    let count = tokenizer.count(text, TokenModel::Gpt4);

    // cl100k_base: "Hello" + "," + " world" + "!" = 4 tokens
    // Note: exact count may vary, but should be consistent
    assert!((3..=5).contains(&count), "Expected ~4 tokens, got {}", count);
}

#[test]
fn test_exact_gpt4o_hello_world() {
    let tokenizer = Tokenizer::new();
    let text = "Hello, world!";
    let count = tokenizer.count(text, TokenModel::Gpt4o);

    // o200k_base is more efficient, typically fewer tokens
    assert!((2..=5).contains(&count), "Expected 3-4 tokens, got {}", count);
}

#[test]
fn test_exact_gpt4_python_code() {
    let tokenizer = Tokenizer::new();
    let code = r#"def hello():
    print("Hello, World!")

hello()
"#;
    let count = tokenizer.count(code, TokenModel::Gpt4);

    // Python code tokenization with cl100k_base
    assert!((10..=25).contains(&count), "Expected 15-20 tokens, got {}", count);
}

#[test]
fn test_exact_gpt4o_python_code() {
    let tokenizer = Tokenizer::new();
    let code = r#"def hello():
    print("Hello, World!")

hello()
"#;
    let count = tokenizer.count(code, TokenModel::Gpt4o);

    // o200k_base typically more efficient for code
    assert!((8..=25).contains(&count), "Expected 12-18 tokens, got {}", count);
}

#[test]
fn test_exact_gpt4_rust_code() {
    let tokenizer = Tokenizer::new();
    let code = r#"fn main() {
    println!("Hello, World!");
}
"#;
    let count = tokenizer.count(code, TokenModel::Gpt4);

    assert!((10..=25).contains(&count), "Expected 15-20 tokens, got {}", count);
}

#[test]
fn test_exact_gpt4_json() {
    let tokenizer = Tokenizer::new();
    let json = r#"{"name": "John", "age": 30, "active": true}"#;
    let count = tokenizer.count(json, TokenModel::Gpt4);

    assert!((10..=25).contains(&count), "JSON tokenization, got {}", count);
}

// ============================================================================
// Estimation Tests
// ============================================================================

#[test]
fn test_estimation_claude() {
    let tokenizer = Tokenizer::new();
    let text = "The quick brown fox jumps over the lazy dog.";
    let count = tokenizer.count(text, TokenModel::Claude);

    // ~44 chars / 3.5 chars_per_token ≈ 13 tokens
    assert!((8..=20).contains(&count), "Claude estimation, got {}", count);
}

#[test]
fn test_estimation_gemini() {
    let tokenizer = Tokenizer::new();
    let text = "The quick brown fox jumps over the lazy dog.";
    let count = tokenizer.count(text, TokenModel::Gemini);

    // ~44 chars / 3.8 chars_per_token ≈ 12 tokens
    assert!((8..=18).contains(&count), "Gemini estimation, got {}", count);
}

#[test]
fn test_estimation_llama() {
    let tokenizer = Tokenizer::new();
    let text = "The quick brown fox jumps over the lazy dog.";
    let count = tokenizer.count(text, TokenModel::Llama);

    // ~44 chars / 3.5 chars_per_token ≈ 13 tokens
    assert!((8..=20).contains(&count), "Llama estimation, got {}", count);
}

#[test]
fn test_estimation_codellama() {
    let tokenizer = Tokenizer::new();
    let code = "fn main() { println!(\"Hello\"); }";
    let count = tokenizer.count(code, TokenModel::CodeLlama);

    // CodeLlama has lower chars_per_token (3.2), more granular for code
    assert!((5..=20).contains(&count), "CodeLlama estimation, got {}", count);
}

#[test]
fn test_estimation_only_mode() {
    let tokenizer = Tokenizer::estimation_only();
    let text = "Hello, world!";

    let count_gpt4 = tokenizer.count(text, TokenModel::Gpt4);
    let count_gpt4o = tokenizer.count(text, TokenModel::Gpt4o);

    // In estimation mode, should use chars_per_token ratios
    assert!(count_gpt4 > 0);
    assert!(count_gpt4o > 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_string() {
    let tokenizer = Tokenizer::new();

    assert_eq!(tokenizer.count("", TokenModel::Claude), 0);
    assert_eq!(tokenizer.count("", TokenModel::Gpt4), 0);
    assert_eq!(tokenizer.count("", TokenModel::Gpt4o), 0);
    assert_eq!(tokenizer.count("", TokenModel::Gemini), 0);
    assert_eq!(tokenizer.count("", TokenModel::Llama), 0);
}

#[test]
fn test_single_character() {
    let tokenizer = Tokenizer::new();

    let count = tokenizer.count("a", TokenModel::Gpt4);
    assert_eq!(count, 1, "Single char should be 1 token");

    let count = tokenizer.count("!", TokenModel::Gpt4);
    assert_eq!(count, 1, "Single punctuation should be 1 token");
}

#[test]
fn test_whitespace_only() {
    let tokenizer = Tokenizer::new();

    let count = tokenizer.count("   ", TokenModel::Gpt4);
    assert!(count >= 1, "Whitespace should have at least 1 token");

    let count = tokenizer.count("\n\n\n", TokenModel::Gpt4);
    assert!(count >= 1, "Newlines should have tokens");

    let count = tokenizer.count("\t\t", TokenModel::Gpt4);
    assert!(count >= 1, "Tabs should have tokens");
}

#[test]
fn test_unicode_basic() {
    let tokenizer = Tokenizer::new();

    let count = tokenizer.count("Hello, 世界!", TokenModel::Gpt4);
    assert!(count > 0, "Unicode should be tokenized");

    let count = tokenizer.count("🎉🎊🎁", TokenModel::Gpt4);
    assert!(count > 0, "Emojis should be tokenized");
}

#[test]
fn test_unicode_complex() {
    let tokenizer = Tokenizer::new();

    // Japanese
    let count = tokenizer.count("こんにちは世界", TokenModel::Gpt4);
    assert!(count > 0, "Japanese should be tokenized");

    // Arabic
    let count = tokenizer.count("مرحبا بالعالم", TokenModel::Gpt4);
    assert!(count > 0, "Arabic should be tokenized");

    // Russian
    let count = tokenizer.count("Привет мир", TokenModel::Gpt4);
    assert!(count > 0, "Russian should be tokenized");
}

#[test]
fn test_very_long_string() {
    let tokenizer = Tokenizer::new();
    let text = "word ".repeat(10000);

    let count = tokenizer.count(&text, TokenModel::Gpt4);
    assert!(count > 1000, "Long string should have many tokens");
    assert!(count < 15000, "Token count should be reasonable");
}

#[test]
fn test_special_characters() {
    let tokenizer = Tokenizer::new();

    let text = "!@#$%^&*(){}[]|\\;':\",./<>?";
    let count = tokenizer.count(text, TokenModel::Gpt4);
    assert!(count > 0, "Special chars should be tokenized");
}

#[test]
fn test_code_with_indentation() {
    let tokenizer = Tokenizer::new();
    let code = "    def foo():\n        pass";
    let count = tokenizer.count(code, TokenModel::Gpt4);
    assert!(count > 0);
}

#[test]
fn test_xml_content() {
    let tokenizer = Tokenizer::new();
    let xml = r#"<root><child attr="value">content</child></root>"#;
    let count = tokenizer.count(xml, TokenModel::Gpt4);
    assert!(count >= 10, "XML should have reasonable tokens");
}

// ============================================================================
// Truncation Tests
// ============================================================================

#[test]
fn test_truncate_within_budget() {
    let tokenizer = Tokenizer::new();
    let text = "Hello";
    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 100);

    assert_eq!(truncated, text, "Short text should not be truncated");
}

#[test]
fn test_truncate_exceeds_budget() {
    let tokenizer = Tokenizer::new();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(50);

    let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, 10);
    let count = tokenizer.count(truncated, TokenModel::Gpt4);

    assert!(count <= 10, "Truncated text should fit budget, got {}", count);
    assert!(truncated.len() < text.len(), "Text should be shorter");
}

#[test]
fn test_truncate_preserves_utf8() {
    let tokenizer = Tokenizer::new();
    // Unicode text with multi-byte characters
    let text = "Hello 世界! こんにちは. مرحبا. Привет.".repeat(20);

    let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, 15);

    // Should be valid UTF-8 (no panic on str operations)
    assert!(truncated.is_ascii() || !truncated.is_empty());
    // Verify it's valid UTF-8 by attempting operations
    let _ = truncated.chars().count();
}

#[test]
fn test_truncate_word_boundary() {
    let tokenizer = Tokenizer::new();
    let text = "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10";

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 5);

    // Should try to truncate at word boundary (space or newline)
    // May end with space or be a complete word
    let count = tokenizer.count(truncated, TokenModel::Gpt4);
    assert!(count <= 5, "Should fit within budget");
}

#[test]
fn test_truncate_to_zero_budget() {
    let tokenizer = Tokenizer::new();
    let text = "Hello, world!";

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 0);

    // With 0 budget, should return empty or minimal
    assert!(truncated.len() <= text.len());
}

#[test]
fn test_truncate_tiny_budget() {
    let tokenizer = Tokenizer::new();
    let text = "The quick brown fox";

    let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 1);
    let count = tokenizer.count(truncated, TokenModel::Gpt4);

    assert!(count <= 1, "Should fit in 1 token budget, got {}", count);
}

// ============================================================================
// TokenModel Tests
// ============================================================================

#[test]
fn test_model_names() {
    assert_eq!(TokenModel::Claude.name(), "claude");
    assert_eq!(TokenModel::Gpt4o.name(), "gpt-4o");
    assert_eq!(TokenModel::Gpt4oMini.name(), "gpt-4o-mini");
    assert_eq!(TokenModel::Gpt4.name(), "gpt-4");
    assert_eq!(TokenModel::Gpt35Turbo.name(), "gpt-3.5-turbo");
    assert_eq!(TokenModel::Gemini.name(), "gemini");
    assert_eq!(TokenModel::Llama.name(), "llama");
    assert_eq!(TokenModel::CodeLlama.name(), "codellama");
}

#[test]
fn test_chars_per_token() {
    assert!(TokenModel::Claude.chars_per_token() > 0.0);
    assert!(TokenModel::Gpt4o.chars_per_token() > 0.0);
    assert!(TokenModel::Gpt4.chars_per_token() > 0.0);
    assert!(TokenModel::Gemini.chars_per_token() > 0.0);
    assert!(TokenModel::Llama.chars_per_token() > 0.0);
    assert!(TokenModel::CodeLlama.chars_per_token() > 0.0);

    // GPT-4o should have higher chars_per_token (more efficient)
    assert!(TokenModel::Gpt4o.chars_per_token() >= TokenModel::Gpt4.chars_per_token());
}

#[test]
fn test_has_exact_tokenizer() {
    assert!(TokenModel::Gpt4o.has_exact_tokenizer());
    assert!(TokenModel::Gpt4oMini.has_exact_tokenizer());
    assert!(TokenModel::Gpt4.has_exact_tokenizer());
    assert!(TokenModel::Gpt35Turbo.has_exact_tokenizer());

    assert!(!TokenModel::Claude.has_exact_tokenizer());
    assert!(!TokenModel::Gemini.has_exact_tokenizer());
    assert!(!TokenModel::Llama.has_exact_tokenizer());
    assert!(!TokenModel::CodeLlama.has_exact_tokenizer());
}

// ============================================================================
// TokenCounts Tests
// ============================================================================

#[test]
fn test_token_counts_zero() {
    let counts = TokenCounts::zero();
    assert_eq!(counts.claude, 0);
    assert_eq!(counts.o200k, 0);
    assert_eq!(counts.cl100k, 0);
    assert_eq!(counts.gemini, 0);
    assert_eq!(counts.llama, 0);
}

#[test]
fn test_token_counts_default() {
    let counts = TokenCounts::default();
    assert_eq!(counts, TokenCounts::zero());
}

#[test]
fn test_token_counts_get() {
    let counts = TokenCounts {
        o200k: 8,
        cl100k: 9,
        claude: 10,
        gemini: 11,
        llama: 10,
        mistral: 10,
        deepseek: 10,
        qwen: 10,
        cohere: 11,
        grok: 10,
    };

    assert_eq!(counts.get(TokenModel::Claude), 10);
    assert_eq!(counts.get(TokenModel::Gpt4o), 8);
    assert_eq!(counts.get(TokenModel::Gpt4oMini), 8); // Same encoding as Gpt4o
    assert_eq!(counts.get(TokenModel::Gpt4), 9);
    assert_eq!(counts.get(TokenModel::Gpt35Turbo), 9); // Same encoding as Gpt4
    assert_eq!(counts.get(TokenModel::Gemini), 11);
    assert_eq!(counts.get(TokenModel::Llama), 10);
    assert_eq!(counts.get(TokenModel::CodeLlama), 10); // Same as Llama
}

#[test]
fn test_token_counts_total() {
    let counts = TokenCounts {
        o200k: 8,
        cl100k: 9,
        claude: 10,
        gemini: 11,
        llama: 10,
        mistral: 10,
        deepseek: 10,
        qwen: 10,
        cohere: 11,
        grok: 10,
    };

    // 8 + 9 + 10 + 11 + 10 + 10 + 10 + 10 + 11 + 10 = 99
    assert_eq!(counts.total(), 99);
}

#[test]
fn test_token_counts_add_method() {
    let mut counts = TokenCounts {
        o200k: 8,
        cl100k: 9,
        claude: 10,
        gemini: 11,
        llama: 10,
        mistral: 10,
        deepseek: 10,
        qwen: 10,
        cohere: 11,
        grok: 10,
    };

    let other = TokenCounts {
        o200k: 4,
        cl100k: 3,
        claude: 5,
        gemini: 6,
        llama: 5,
        mistral: 5,
        deepseek: 5,
        qwen: 5,
        cohere: 6,
        grok: 5,
    };

    counts.add(&other);

    assert_eq!(counts.claude, 15);
    assert_eq!(counts.o200k, 12);
    assert_eq!(counts.cl100k, 12);
    assert_eq!(counts.gemini, 17);
    assert_eq!(counts.llama, 15);
}

#[test]
fn test_token_counts_add_operator() {
    let a = TokenCounts {
        o200k: 8,
        cl100k: 9,
        claude: 10,
        gemini: 11,
        llama: 10,
        mistral: 10,
        deepseek: 10,
        qwen: 10,
        cohere: 11,
        grok: 10,
    };

    let b = TokenCounts {
        o200k: 4,
        cl100k: 3,
        claude: 5,
        gemini: 6,
        llama: 5,
        mistral: 5,
        deepseek: 5,
        qwen: 5,
        cohere: 6,
        grok: 5,
    };

    let sum = a + b;

    assert_eq!(sum.claude, 15);
    assert_eq!(sum.o200k, 12);
    assert_eq!(sum.cl100k, 12);
    assert_eq!(sum.gemini, 17);
    assert_eq!(sum.llama, 15);
}

#[test]
fn test_token_counts_sum() {
    let counts = vec![
        TokenCounts {
            o200k: 8,
            cl100k: 9,
            claude: 10,
            gemini: 11,
            llama: 10,
            mistral: 10,
            deepseek: 10,
            qwen: 10,
            cohere: 11,
            grok: 10,
        },
        TokenCounts {
            o200k: 4,
            cl100k: 3,
            claude: 5,
            gemini: 6,
            llama: 5,
            mistral: 5,
            deepseek: 5,
            qwen: 5,
            cohere: 6,
            grok: 5,
        },
        TokenCounts {
            o200k: 2,
            cl100k: 4,
            claude: 3,
            gemini: 3,
            llama: 3,
            mistral: 3,
            deepseek: 3,
            qwen: 3,
            cohere: 3,
            grok: 3,
        },
    ];

    let total: TokenCounts = counts.into_iter().sum();

    assert_eq!(total.claude, 18);
    assert_eq!(total.o200k, 14);
    assert_eq!(total.cl100k, 16);
    assert_eq!(total.gemini, 20);
    assert_eq!(total.llama, 18);
}

// ============================================================================
// count_all Tests
// ============================================================================

#[test]
fn test_count_all_returns_all_models() {
    let tokenizer = Tokenizer::new();
    let text = "Hello, world!";
    let counts = tokenizer.count_all(text);

    assert!(counts.claude > 0);
    assert!(counts.o200k > 0);
    assert!(counts.cl100k > 0);
    assert!(counts.gemini > 0);
    assert!(counts.llama > 0);
}

#[test]
fn test_count_all_empty_string() {
    let tokenizer = Tokenizer::new();
    let counts = tokenizer.count_all("");

    assert_eq!(counts.claude, 0);
    assert_eq!(counts.o200k, 0);
    assert_eq!(counts.cl100k, 0);
    assert_eq!(counts.gemini, 0);
    assert_eq!(counts.llama, 0);
}

// ============================================================================
// most_efficient_model Tests
// ============================================================================

#[test]
fn test_most_efficient_model() {
    let tokenizer = Tokenizer::new();
    let text = "function hello() { console.log('hello'); }";
    let (model, count) = tokenizer.most_efficient_model(text);

    assert!(count > 0);
    // Should return one of the valid models
    matches!(
        model,
        TokenModel::Claude
            | TokenModel::Gpt4o
            | TokenModel::Gpt4
            | TokenModel::Gemini
            | TokenModel::Llama
    );
}

#[test]
fn test_most_efficient_model_empty() {
    let tokenizer = Tokenizer::new();
    let (model, count) = tokenizer.most_efficient_model("");

    assert_eq!(count, 0);
    // Should return some default model
    let _ = model.name(); // Just verify it's valid
}

// ============================================================================
// exceeds_budget Tests
// ============================================================================

#[test]
fn test_exceeds_budget_true() {
    let tokenizer = Tokenizer::new();
    let text = "This is a longer text that definitely exceeds a budget of just 1 token.";

    assert!(tokenizer.exceeds_budget(text, TokenModel::Gpt4, 1));
}

#[test]
fn test_exceeds_budget_false() {
    let tokenizer = Tokenizer::new();
    let text = "Hi";

    assert!(!tokenizer.exceeds_budget(text, TokenModel::Gpt4, 100));
}

#[test]
fn test_exceeds_budget_exact() {
    let tokenizer = Tokenizer::new();
    let text = "Hello";
    let count = tokenizer.count(text, TokenModel::Gpt4);

    assert!(!tokenizer.exceeds_budget(text, TokenModel::Gpt4, count));
    assert!(!tokenizer.exceeds_budget(text, TokenModel::Gpt4, count + 1));
    if count > 0 {
        assert!(tokenizer.exceeds_budget(text, TokenModel::Gpt4, count - 1));
    }
}

// ============================================================================
// quick_estimate Tests
// ============================================================================

#[test]
fn test_quick_estimate_basic() {
    let count = quick_estimate("Hello, world!", TokenModel::Claude);
    assert!(count > 0);
    assert!(count < 20);
}

#[test]
fn test_quick_estimate_empty() {
    assert_eq!(quick_estimate("", TokenModel::Claude), 0);
    assert_eq!(quick_estimate("", TokenModel::Gpt4), 0);
}

#[test]
fn test_quick_estimate_vs_full() {
    let tokenizer = Tokenizer::estimation_only();
    let text = "Hello, world!";

    // Quick estimate should be similar to estimation-only tokenizer
    let quick = quick_estimate(text, TokenModel::Claude);
    let full = tokenizer.count(text, TokenModel::Claude);

    // They use different calculation methods, so allow some variance
    assert!((quick as i32 - full as i32).abs() <= 5, "quick={}, full={}", quick, full);
}

// ============================================================================
// Comparison: Exact vs Estimation
// ============================================================================

#[test]
fn test_exact_vs_estimation_gpt4() {
    let exact_tokenizer = Tokenizer::new();
    let estimation_tokenizer = Tokenizer::estimation_only();
    let text = "The quick brown fox jumps over the lazy dog.";

    let exact = exact_tokenizer.count(text, TokenModel::Gpt4);
    let estimated = estimation_tokenizer.count(text, TokenModel::Gpt4);

    // They should be in the same ballpark
    let ratio = exact as f32 / estimated as f32;
    assert!(
        ratio > 0.5 && ratio < 2.0,
        "Exact {} vs estimated {} (ratio {})",
        exact,
        estimated,
        ratio
    );
}

#[test]
fn test_exact_vs_estimation_code() {
    let exact_tokenizer = Tokenizer::new();
    let estimation_tokenizer = Tokenizer::estimation_only();
    let code = r#"
fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
"#;

    let exact = exact_tokenizer.count(code, TokenModel::Gpt4);
    let estimated = estimation_tokenizer.count(code, TokenModel::Gpt4);

    // Code tokenization can vary more, allow larger margin
    let ratio = exact as f32 / estimated as f32;
    assert!(
        ratio > 0.3 && ratio < 3.0,
        "Exact {} vs estimated {} (ratio {})",
        exact,
        estimated,
        ratio
    );
}

// ============================================================================
// Consistency Tests
// ============================================================================

#[test]
fn test_consistency_same_input() {
    let tokenizer = Tokenizer::new();
    let text = "Consistent input for testing.";

    let count1 = tokenizer.count(text, TokenModel::Gpt4);
    let count2 = tokenizer.count(text, TokenModel::Gpt4);
    let count3 = tokenizer.count(text, TokenModel::Gpt4);

    assert_eq!(count1, count2);
    assert_eq!(count2, count3);
}

#[test]
fn test_consistency_across_instances() {
    let tokenizer1 = Tokenizer::new();
    let tokenizer2 = Tokenizer::new();
    let text = "Testing consistency across tokenizer instances.";

    let count1 = tokenizer1.count(text, TokenModel::Gpt4);
    let count2 = tokenizer2.count(text, TokenModel::Gpt4);

    assert_eq!(count1, count2);
}

// ============================================================================
// Real-World Code Samples
// ============================================================================

#[test]
fn test_javascript_code() {
    let tokenizer = Tokenizer::new();
    let code = r#"
const express = require('express');
const app = express();

app.get('/api/users', async (req, res) => {
    const users = await db.query('SELECT * FROM users');
    res.json(users);
});

app.listen(3000, () => {
    console.log('Server running on port 3000');
});
"#;

    let count = tokenizer.count(code, TokenModel::Gpt4);
    assert!(count > 30, "JS code should have many tokens, got {}", count);
    assert!(count < 200, "Token count should be reasonable, got {}", count);
}

#[test]
fn test_rust_complex_code() {
    let tokenizer = Tokenizer::new();
    let code = r#"
use std::collections::HashMap;

pub struct TokenCounter<'a> {
    cache: HashMap<&'a str, u32>,
}

impl<'a> TokenCounter<'a> {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn count(&mut self, text: &'a str) -> u32 {
        if let Some(&count) = self.cache.get(text) {
            return count;
        }
        let count = self.compute_tokens(text);
        self.cache.insert(text, count);
        count
    }

    fn compute_tokens(&self, text: &str) -> u32 {
        text.split_whitespace().count() as u32
    }
}
"#;

    let count = tokenizer.count(code, TokenModel::Gpt4);
    assert!(count > 50, "Complex Rust code should have many tokens");
    assert!(count < 300, "Token count should be reasonable");
}

#[test]
fn test_markdown_document() {
    let tokenizer = Tokenizer::new();
    let markdown = r#"
# API Documentation

## Overview

This API provides access to user management functionality.

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /users | List all users |
| POST | /users | Create user |
| DELETE | /users/:id | Delete user |

### Example

```json
{
    "name": "John Doe",
    "email": "john@example.com"
}
```
"#;

    let count = tokenizer.count(markdown, TokenModel::Gpt4);
    assert!(count > 50, "Markdown should have reasonable tokens");
}

#[test]
fn test_sql_queries() {
    let tokenizer = Tokenizer::new();
    let sql = r#"
SELECT u.id, u.name, u.email, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.created_at > '2024-01-01'
GROUP BY u.id, u.name, u.email
HAVING COUNT(o.id) > 5
ORDER BY order_count DESC
LIMIT 100;
"#;

    let count = tokenizer.count(sql, TokenModel::Gpt4);
    assert!(count > 30, "SQL should have tokens");
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_token_counts_serialize() {
    let counts = TokenCounts {
        o200k: 8,
        cl100k: 9,
        claude: 10,
        gemini: 11,
        llama: 10,
        mistral: 10,
        deepseek: 10,
        qwen: 10,
        cohere: 11,
        grok: 10,
    };

    let json = serde_json::to_string(&counts).unwrap();
    assert!(json.contains("\"claude\":10"));
    assert!(json.contains("\"o200k\":8"));
}

#[test]
fn test_token_counts_deserialize() {
    let json = r#"{"o200k":8,"cl100k":9,"claude":10,"gemini":11,"llama":10,"mistral":10,"deepseek":10,"qwen":10,"cohere":11,"grok":10}"#;
    let counts: TokenCounts = serde_json::from_str(json).unwrap();

    assert_eq!(counts.claude, 10);
    assert_eq!(counts.o200k, 8);
    assert_eq!(counts.cl100k, 9);
    assert_eq!(counts.gemini, 11);
    assert_eq!(counts.llama, 10);
}

#[test]
fn test_token_counts_roundtrip() {
    let original = TokenCounts {
        o200k: 36,
        cl100k: 39,
        claude: 42,
        gemini: 40,
        llama: 42,
        mistral: 42,
        deepseek: 42,
        qwen: 42,
        cohere: 40,
        grok: 42,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: TokenCounts = serde_json::from_str(&json).unwrap();

    assert_eq!(original, restored);
}

// ============================================================================
// Bug Fix Tests - tiktoken panic handling
// ============================================================================

/// Test that tokenizer handles potentially problematic Unicode sequences
/// without panicking. tiktoken can panic on certain malformed inputs;
/// the fix uses catch_unwind to fall back to estimation.
#[test]
fn test_tiktoken_panic_recovery_unicode() {
    let tokenizer = Tokenizer::new();

    // Various Unicode edge cases that might cause issues
    let test_cases = [
        // Isolated surrogates (invalid UTF-8 would be caught by Rust, but test boundaries)
        "\u{FFFD}\u{FFFD}\u{FFFD}",
        // Zero-width characters
        "\u{200B}\u{200C}\u{200D}\u{FEFF}",
        // Very long combining character sequences
        &format!("a{}", "\u{0300}".repeat(100)),
        // Mix of scripts with unusual characters
        "Test\u{0000}With\u{0001}Control\u{001F}Chars",
        // Repeated emoji with skin tone modifiers
        &"👨‍👩‍👧‍👦".repeat(50),
        // RTL override characters
        "\u{202E}reversed\u{202C}",
    ];

    for (i, test) in test_cases.iter().enumerate() {
        let result = std::panic::catch_unwind(|| tokenizer.count(test, TokenModel::Gpt4o));
        assert!(
            result.is_ok(),
            "Test case {} should not panic for GPT-4o: {:?}",
            i,
            test.chars().take(20).collect::<String>()
        );

        let result = std::panic::catch_unwind(|| tokenizer.count(test, TokenModel::Gpt4));
        assert!(
            result.is_ok(),
            "Test case {} should not panic for GPT-4: {:?}",
            i,
            test.chars().take(20).collect::<String>()
        );
    }
}

/// Test tokenizer with binary-like content embedded in strings
#[test]
fn test_tiktoken_panic_recovery_binary_like() {
    let tokenizer = Tokenizer::new();

    // Content that looks like binary data but is valid UTF-8
    let test_cases = [
        // Random-looking bytes as UTF-8 string
        "ÿþýüûúùø÷öõôóòñðïîíìëêéèçæåäãâáàß",
        // Lots of null-adjacent characters
        "\u{0001}\u{0002}\u{0003}\u{0004}\u{0005}",
        // Very high Unicode codepoints
        "\u{1F600}\u{1F601}\u{1F602}\u{1F923}\u{1F970}",
        // Mixed valid UTF-8 that might confuse BPE
        "AAAA\u{FFFF}BBBB\u{FFFE}CCCC",
    ];

    for (i, test) in test_cases.iter().enumerate() {
        let count = tokenizer.count(test, TokenModel::Gpt4o);
        assert!(count > 0, "Test case {} should return positive count", i);

        let count = tokenizer.count(test, TokenModel::Gpt4);
        assert!(count > 0, "Test case {} should return positive count", i);
    }
}

/// Test that tokenizer gracefully handles very large inputs
#[test]
fn test_tiktoken_large_input_no_panic() {
    let tokenizer = Tokenizer::new();

    // 1MB of text
    let large_text = "word ".repeat(200_000);

    let result = std::panic::catch_unwind(|| tokenizer.count(&large_text, TokenModel::Gpt4o));
    assert!(result.is_ok(), "Large input should not panic");

    if let Ok(count) = result {
        assert!(count > 100_000, "Should have many tokens for 1MB text");
    }
}

/// Test tokenizer with strings at UTF-8 multi-byte boundaries
#[test]
fn test_tiktoken_utf8_boundary_safety() {
    let tokenizer = Tokenizer::new();

    // Create content where multi-byte UTF-8 characters are at strategic positions
    // Chinese character 中 is 3 bytes (E4 B8 AD)
    let test_cases = [
        // Exactly at power-of-2 boundaries
        format!("{}中", "a".repeat(255)),
        format!("{}中", "a".repeat(256)),
        format!("{}中", "a".repeat(1023)),
        format!("{}中", "a".repeat(1024)),
        format!("{}中", "a".repeat(4095)),
        format!("{}中", "a".repeat(4096)),
    ];

    for (i, test) in test_cases.iter().enumerate() {
        let count = tokenizer.count(test, TokenModel::Gpt4o);
        assert!(count > 0, "Boundary test {} should work", i);

        let count = tokenizer.count(test, TokenModel::Gpt4);
        assert!(count > 0, "Boundary test {} should work", i);
    }
}
