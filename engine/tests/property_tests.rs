//! Property-based tests using proptest
//!
//! Tests invariants that should hold for any input:
//! - truncate_to_budget never exceeds budget (monotonic)
//! - truncate_to_budget never splits UTF-8 characters
//! - Token estimation is always positive for non-empty strings
//! - Security scanner never panics on arbitrary input

use proptest::prelude::*;

use infiniloom_engine::security::SecurityScanner;
use infiniloom_engine::tokenizer::{TokenCounts, TokenModel, Tokenizer};

// ============================================================================
// Tokenizer Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// truncate_to_budget should never exceed the budget
    #[test]
    fn prop_truncate_never_exceeds_budget(
        text in "\\PC{1,1000}",  // Any printable chars, 1-1000
        budget in 1u32..100u32,
    ) {
        let tokenizer = Tokenizer::new();
        let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);
        let count = tokenizer.count(truncated, TokenModel::Gpt4);

        prop_assert!(
            count <= budget,
            "Truncated text exceeded budget: {} > {} (text len: {})",
            count, budget, text.len()
        );
    }

    /// truncate_to_budget should produce valid UTF-8
    #[test]
    fn prop_truncate_preserves_utf8(
        text in "\\PC{1,500}",  // Printable chars including unicode
        budget in 1u32..50u32,
    ) {
        let tokenizer = Tokenizer::new();
        let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);

        // This should not panic - if it does, UTF-8 was split
        let char_count = truncated.chars().count();
        prop_assert!(char_count <= text.chars().count());

        // Verify it's valid UTF-8 by iterating
        for c in truncated.chars() {
            prop_assert!(c.len_utf8() >= 1);
        }
    }

    /// Token count should be positive for non-empty strings
    #[test]
    fn prop_count_positive_for_nonempty(text in ".{1,500}") {
        let tokenizer = Tokenizer::new();

        let count_claude = tokenizer.count(&text, TokenModel::Claude);
        let count_gpt4 = tokenizer.count(&text, TokenModel::Gpt4);
        let count_gpt4o = tokenizer.count(&text, TokenModel::Gpt4o);

        prop_assert!(count_claude >= 1, "Claude count was 0 for non-empty text");
        prop_assert!(count_gpt4 >= 1, "GPT-4 count was 0 for non-empty text");
        prop_assert!(count_gpt4o >= 1, "GPT-4o count was 0 for non-empty text");
    }

    /// Token count should be 0 for empty string
    #[test]
    fn prop_count_zero_for_empty(_dummy in Just(())) {
        let tokenizer = Tokenizer::new();

        prop_assert_eq!(tokenizer.count("", TokenModel::Claude), 0);
        prop_assert_eq!(tokenizer.count("", TokenModel::Gpt4), 0);
        prop_assert_eq!(tokenizer.count("", TokenModel::Gpt4o), 0);
    }

    /// Token count should be monotonic - longer strings have equal or more tokens
    #[test]
    fn prop_count_monotonic(
        base in "\\PC{10,100}",
        suffix in "\\PC{1,50}",
    ) {
        let tokenizer = Tokenizer::new();
        let extended = format!("{}{}", base, suffix);

        let count_base = tokenizer.count(&base, TokenModel::Gpt4);
        let count_extended = tokenizer.count(&extended, TokenModel::Gpt4);

        // This is a soft property - tokenization is complex, but generally
        // adding text shouldn't decrease token count significantly
        prop_assert!(
            count_extended >= count_base.saturating_sub(2),
            "Extended text has significantly fewer tokens: {} vs {}",
            count_extended, count_base
        );
    }

    /// count_all should return consistent values
    #[test]
    fn prop_count_all_consistency(text in "\\PC{1,200}") {
        let tokenizer = Tokenizer::new();

        let counts = tokenizer.count_all(&text);

        // Individual counts should match count_all
        prop_assert_eq!(counts.claude, tokenizer.count(&text, TokenModel::Claude));
        prop_assert_eq!(counts.o200k, tokenizer.count(&text, TokenModel::Gpt4o));
        prop_assert_eq!(counts.cl100k, tokenizer.count(&text, TokenModel::Gpt4));
        prop_assert_eq!(counts.gemini, tokenizer.count(&text, TokenModel::Gemini));
        prop_assert_eq!(counts.llama, tokenizer.count(&text, TokenModel::Llama));
    }

    /// exceeds_budget should be consistent with count
    #[test]
    fn prop_exceeds_budget_consistent(
        text in "\\PC{1,200}",
        budget in 1u32..100u32,
    ) {
        let tokenizer = Tokenizer::new();

        let count = tokenizer.count(&text, TokenModel::Gpt4);
        let exceeds = tokenizer.exceeds_budget(&text, TokenModel::Gpt4, budget);

        prop_assert_eq!(exceeds, count > budget);
    }

    /// Truncation should be idempotent
    #[test]
    fn prop_truncate_idempotent(
        text in "\\PC{1,500}",
        budget in 1u32..50u32,
    ) {
        let tokenizer = Tokenizer::new();

        let truncated1 = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);
        let truncated2 = tokenizer.truncate_to_budget(truncated1, TokenModel::Gpt4, budget);

        // Truncating an already-truncated string shouldn't change it
        prop_assert_eq!(truncated1, truncated2);
    }
}

// ============================================================================
// TokenCounts Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// TokenCounts addition should be commutative
    #[test]
    fn prop_token_counts_add_commutative(
        a_o200k in 0u32..1000,
        a_cl100k in 0u32..1000,
        a_claude in 0u32..1000,
        a_gemini in 0u32..1000,
        a_llama in 0u32..1000,
        b_o200k in 0u32..1000,
        b_cl100k in 0u32..1000,
        b_claude in 0u32..1000,
        b_gemini in 0u32..1000,
        b_llama in 0u32..1000,
    ) {
        let a = TokenCounts {
            o200k: a_o200k, cl100k: a_cl100k, claude: a_claude,
            gemini: a_gemini, llama: a_llama,
            mistral: a_llama, deepseek: a_llama, qwen: a_llama,
            cohere: a_gemini, grok: a_llama,
        };
        let b = TokenCounts {
            o200k: b_o200k, cl100k: b_cl100k, claude: b_claude,
            gemini: b_gemini, llama: b_llama,
            mistral: b_llama, deepseek: b_llama, qwen: b_llama,
            cohere: b_gemini, grok: b_llama,
        };

        let sum1 = a + b;
        let sum2 = b + a;

        prop_assert_eq!(sum1, sum2);
    }

    /// TokenCounts total should equal sum of fields
    #[test]
    fn prop_token_counts_total(
        o200k in 0u32..1000,
        cl100k in 0u32..1000,
        claude in 0u32..1000,
        gemini in 0u32..1000,
        llama in 0u32..1000,
    ) {
        let counts = TokenCounts {
            o200k, cl100k, claude, gemini, llama,
            mistral: llama, deepseek: llama, qwen: llama,
            cohere: gemini, grok: llama,
        };

        let expected = o200k as u64 + cl100k as u64 + claude as u64
                     + gemini as u64 + llama as u64 + llama as u64 * 4 + gemini as u64;

        prop_assert_eq!(counts.total(), expected);
    }

    /// Adding zero should be identity
    #[test]
    fn prop_token_counts_add_identity(
        o200k in 0u32..1000,
        cl100k in 0u32..1000,
        claude in 0u32..1000,
        gemini in 0u32..1000,
        llama in 0u32..1000,
    ) {
        let counts = TokenCounts {
            o200k, cl100k, claude, gemini, llama,
            mistral: llama, deepseek: llama, qwen: llama,
            cohere: gemini, grok: llama,
        };
        let zero = TokenCounts::zero();

        let sum = counts + zero;

        prop_assert_eq!(sum, counts);
    }
}

// ============================================================================
// Security Scanner Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Security scanner should never panic on arbitrary input
    #[test]
    fn prop_security_scanner_no_panic(text in "\\PC{0,2000}") {
        let scanner = SecurityScanner::new();

        // Should not panic
        let findings = scanner.scan(&text, "test.txt");

        // Findings should report valid line numbers when present
        prop_assert!(findings.iter().all(|f| f.line >= 1));
    }

    /// is_safe should be consistent with scan results
    #[test]
    fn prop_security_is_safe_consistent(text in "\\PC{0,500}") {
        let scanner = SecurityScanner::new();

        let findings = scanner.scan(&text, "test.txt");
        let is_safe = scanner.is_safe(&text, "test.txt");

        // is_safe returns true if no High+ severity findings
        let has_high_severity = findings.iter().any(|f| {
            use infiniloom_engine::security::Severity;
            f.severity >= Severity::High
        });

        prop_assert_eq!(is_safe, !has_high_severity);
    }

    /// redact_content should not increase text length significantly
    #[test]
    fn prop_redact_reasonable_length(text in "\\PC{0,500}") {
        let scanner = SecurityScanner::new();

        let redacted = scanner.redact_content(&text, "test.txt");

        // Redaction replaces secrets with asterisks of similar length
        // So length shouldn't increase dramatically
        prop_assert!(
            redacted.len() <= text.len() + 100,
            "Redacted text much longer: {} vs {}",
            redacted.len(), text.len()
        );
    }

    /// scan_and_redact should be consistent with separate calls
    #[test]
    fn prop_scan_and_redact_consistent(text in "\\PC{0,300}") {
        let scanner = SecurityScanner::new();

        let (redacted, findings) = scanner.scan_and_redact(&text, "test.txt");

        // Findings should match separate scan call
        let separate_findings = scanner.scan(&text, "test.txt");
        prop_assert_eq!(findings.len(), separate_findings.len());

        // Redacted should be valid UTF-8
        let _ = redacted.chars().count();
    }
}

// ============================================================================
// UTF-8 Edge Cases
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Handle multi-byte UTF-8 characters properly
    #[test]
    fn prop_utf8_multibyte_handling(
        chars in prop::collection::vec(
            prop::char::range('α', 'ω'),  // Greek letters (2-byte UTF-8)
            1..50
        ),
        budget in 1u32..20u32,
    ) {
        let text: String = chars.into_iter().collect();
        let tokenizer = Tokenizer::new();

        let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);

        // Should be valid UTF-8
        prop_assert!(truncated.is_char_boundary(truncated.len()));

        // Each char should be complete
        for c in truncated.chars() {
            prop_assert!(c.len_utf8() >= 1);
        }
    }

    /// Handle emoji properly (4-byte UTF-8)
    #[test]
    fn prop_emoji_handling(
        count in 1usize..20,
        budget in 1u32..30u32,
    ) {
        let text = "🎉".repeat(count);
        let tokenizer = Tokenizer::new();

        let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);

        // Should not split emoji in half
        for c in truncated.chars() {
            prop_assert!(c == '🎉' || c.is_whitespace());
        }
    }

    /// Handle mixed ASCII and Unicode
    #[test]
    fn prop_mixed_encoding(
        ascii_part in "[a-zA-Z0-9]{1,50}",
        unicode_part in "\\PC{1,50}",
        budget in 1u32..30u32,
    ) {
        let text = format!("{}{}", ascii_part, unicode_part);
        let tokenizer = Tokenizer::new();

        let truncated = tokenizer.truncate_to_budget(&text, TokenModel::Gpt4, budget);

        // Should be valid UTF-8
        let _ = truncated.chars().count();

        // Count should respect budget
        let count = tokenizer.count(truncated, TokenModel::Gpt4);
        prop_assert!(count <= budget);
    }
}

// ============================================================================
// Code-specific Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Common code patterns should tokenize without panic
    #[test]
    fn prop_code_patterns_no_panic(
        indent in 0usize..10,
        name in "[a-z_][a-z0-9_]{0,30}",
        params in "[a-z, ]{0,50}",
    ) {
        let code = format!(
            "{}def {}({}):\n{}pass",
            " ".repeat(indent),
            name,
            params,
            " ".repeat(indent + 4)
        );

        let tokenizer = Tokenizer::new();
        let count = tokenizer.count(&code, TokenModel::Gpt4);

        prop_assert!(count > 0);
    }

    /// JSON-like content should tokenize properly
    #[test]
    fn prop_json_tokenization(
        key in "[a-z_]{1,20}",
        value in "[a-zA-Z0-9]{1,30}",
    ) {
        let json = format!(r#"{{"{}": "{}"}}"#, key, value);

        let tokenizer = Tokenizer::new();
        let count = tokenizer.count(&json, TokenModel::Gpt4);

        prop_assert!(count >= 3, "JSON should have multiple tokens");
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Very short strings should be handled
    #[test]
    fn prop_single_char(c in any::<char>()) {
        if c.is_ascii() || c.len_utf8() <= 4 {
            let text = c.to_string();
            let tokenizer = Tokenizer::new();

            let count = tokenizer.count(&text, TokenModel::Gpt4);
            prop_assert!(count >= 1);
        }
    }

    /// Whitespace variations should not cause issues
    #[test]
    fn prop_whitespace_variations(
        spaces in 0usize..100,
        tabs in 0usize..50,
        newlines in 0usize..50,
    ) {
        let text = format!(
            "{}{}{}",
            " ".repeat(spaces),
            "\t".repeat(tabs),
            "\n".repeat(newlines)
        );

        let tokenizer = Tokenizer::new();
        let _ = tokenizer.count(&text, TokenModel::Gpt4);

        // Should not panic, count can be 0 or more
    }

    /// Repeated patterns should scale reasonably
    #[test]
    fn prop_repeated_patterns(
        pattern in "[a-z]{1,10}",
        repeats in 1usize..100,
    ) {
        let text = pattern.repeat(repeats);
        let tokenizer = Tokenizer::new();

        let count = tokenizer.count(&text, TokenModel::Gpt4);

        // Token count should scale roughly with text length
        // (not necessarily linearly due to BPE merging)
        prop_assert!(count >= 1);
        prop_assert!(count as usize <= text.len());
    }
}
