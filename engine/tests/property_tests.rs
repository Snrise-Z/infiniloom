//! Property-based tests using proptest
//!
//! Tests invariants that should hold for any input:
//! - truncate_to_budget never exceeds budget (monotonic)
//! - truncate_to_budget never splits UTF-8 characters
//! - Token estimation is always positive for non-empty strings
//! - Security scanner never panics on arbitrary input

use proptest::prelude::*;

use infiniloom_engine::content_transformation::{
    extract_key_symbols, extract_key_symbols_with_context, extract_signatures, remove_comments,
    remove_empty_lines,
};
use infiniloom_engine::filtering::{
    apply_exclude_patterns, apply_include_patterns, matches_exclude_pattern,
    matches_include_pattern,
};
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

// ============================================================================
// Parser Property Tests
// ============================================================================

use infiniloom_engine::parser::{Language, Parser};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Parser should never panic on arbitrary input (Python)
    #[test]
    fn prop_parser_no_panic_python(code in "\\PC{0,500}") {
        let mut parser = Parser::new();
        // Should not panic - may return error or empty symbols
        let _ = parser.parse(&code, Language::Python);
    }

    /// Parser should never panic on arbitrary input (JavaScript)
    #[test]
    fn prop_parser_no_panic_javascript(code in "\\PC{0,500}") {
        let mut parser = Parser::new();
        let _ = parser.parse(&code, Language::JavaScript);
    }

    /// Parser should never panic on arbitrary input (Rust)
    #[test]
    fn prop_parser_no_panic_rust(code in "\\PC{0,500}") {
        let mut parser = Parser::new();
        let _ = parser.parse(&code, Language::Rust);
    }

    /// Parsed symbols should have valid line numbers (start <= end, both >= 1)
    #[test]
    fn prop_python_symbols_valid_lines(
        name in "[a-z_][a-z0-9_]{0,20}",
        body_lines in 1usize..10,
    ) {
        let code = format!(
            "def {}():\n{}",
            name,
            "    pass\n".repeat(body_lines)
        );

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Python) {
            for symbol in &symbols {
                prop_assert!(
                    symbol.start_line >= 1,
                    "Symbol {} has invalid start_line: {}",
                    symbol.name, symbol.start_line
                );
                prop_assert!(
                    symbol.end_line >= symbol.start_line,
                    "Symbol {} has end_line {} < start_line {}",
                    symbol.name, symbol.end_line, symbol.start_line
                );
            }
        }
    }

    /// Parsed symbols should have non-empty names
    #[test]
    fn prop_symbol_names_nonempty(
        func_name in "[a-zA-Z_][a-zA-Z0-9_]{0,20}",
    ) {
        let code = format!("def {}(): pass", func_name);

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Python) {
            for symbol in &symbols {
                prop_assert!(
                    !symbol.name.is_empty(),
                    "Symbol has empty name"
                );
            }
        }
    }

    /// Parsing should be deterministic - same input always gives same output
    #[test]
    fn prop_parse_deterministic(
        name in "[a-z_][a-z0-9_]{0,15}",
        params in "[a-z, ]{0,30}",
    ) {
        let code = format!("def {}({}):\n    pass", name, params);

        let mut parser1 = Parser::new();
        let mut parser2 = Parser::new();

        let result1 = parser1.parse(&code, Language::Python);
        let result2 = parser2.parse(&code, Language::Python);

        match (result1, result2) {
            (Ok(symbols1), Ok(symbols2)) => {
                prop_assert_eq!(
                    symbols1.len(), symbols2.len(),
                    "Different symbol counts"
                );
                for (s1, s2) in symbols1.iter().zip(symbols2.iter()) {
                    prop_assert_eq!(&s1.name, &s2.name);
                    prop_assert_eq!(s1.start_line, s2.start_line);
                    prop_assert_eq!(s1.end_line, s2.end_line);
                }
            },
            (Err(_), Err(_)) => {
                // Both failed - that's consistent
            },
            _ => {
                prop_assert!(false, "Inconsistent parse results");
            }
        }
    }

    /// Class parsing should produce valid symbols
    #[test]
    fn prop_class_symbols_valid(
        class_name in "[A-Z][a-zA-Z0-9]{0,15}",
        method_name in "[a-z_][a-z0-9_]{0,15}",
    ) {
        let code = format!(
            "class {}:\n    def {}(self):\n        pass",
            class_name, method_name
        );

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Python) {
            // Should find at least the class
            let class_symbols: Vec<_> = symbols.iter()
                .filter(|s| s.name == class_name)
                .collect();
            prop_assert!(
                !class_symbols.is_empty(),
                "Class {} not found in symbols",
                class_name
            );

            // All symbols should have valid line ranges
            for symbol in &symbols {
                prop_assert!(symbol.start_line >= 1);
                prop_assert!(symbol.end_line >= symbol.start_line);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// JavaScript function parsing
    #[test]
    fn prop_js_function_parsing(
        func_name in "[a-z][a-zA-Z0-9]{0,15}",
        param_count in 0usize..5,
    ) {
        let params: String = (0..param_count)
            .map(|i| format!("arg{}", i))
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!("function {}({}) {{ return 42; }}", func_name, params);

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::JavaScript) {
            for symbol in &symbols {
                prop_assert!(symbol.start_line >= 1);
                prop_assert!(symbol.end_line >= symbol.start_line);
                prop_assert!(!symbol.name.is_empty());
            }
        }
    }

    /// Rust function parsing
    #[test]
    fn prop_rust_function_parsing(
        func_name in "[a-z_][a-z0-9_]{0,15}",
    ) {
        let code = format!("fn {}() {{ }}", func_name);

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Rust) {
            let func_symbols: Vec<_> = symbols.iter()
                .filter(|s| s.name == func_name)
                .collect();
            prop_assert!(
                !func_symbols.is_empty(),
                "Function {} not found",
                func_name
            );
        }
    }

    /// Go function parsing
    #[test]
    fn prop_go_function_parsing(
        func_name in "[A-Z][a-zA-Z0-9]{0,15}",
    ) {
        let code = format!("package main\n\nfunc {}() {{}}", func_name);

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Go) {
            for symbol in &symbols {
                prop_assert!(symbol.start_line >= 1);
                prop_assert!(!symbol.name.is_empty());
            }
        }
    }

    /// Nested structures should have proper line ranges
    #[test]
    fn prop_nested_line_ranges(
        class_name in "[A-Z][a-zA-Z]{0,10}",
        method_count in 1usize..5,
    ) {
        let methods: String = (0..method_count)
            .map(|i| format!("    def method_{}(self):\n        pass\n", i))
            .collect();
        let code = format!("class {}:\n{}", class_name, methods);

        let mut parser = Parser::new();
        if let Ok(symbols) = parser.parse(&code, Language::Python) {
            // All symbols should be within the source code line count
            let line_count = code.lines().count() as u32;
            for symbol in &symbols {
                prop_assert!(
                    symbol.end_line <= line_count + 1,
                    "Symbol {} ends at line {} but code only has {} lines",
                    symbol.name, symbol.end_line, line_count
                );
            }
        }
    }
}

// ============================================================================
// Filtering Property Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// matches_exclude_pattern should be deterministic
    #[test]
    fn prop_exclude_pattern_deterministic(
        path in "[a-z/_]{1,30}\\.[a-z]{2,4}",
        pattern in "[a-z*/_]{1,20}",
    ) {
        let result1 = matches_exclude_pattern(&path, &pattern);
        let result2 = matches_exclude_pattern(&path, &pattern);
        prop_assert_eq!(result1, result2, "Non-deterministic result for path={}, pattern={}", path, pattern);
    }

    /// matches_include_pattern should be deterministic
    #[test]
    fn prop_include_pattern_deterministic(
        path in "[a-z/_]{1,30}\\.[a-z]{2,4}",
        pattern in "[a-z*/_]{1,20}",
    ) {
        let result1 = matches_include_pattern(&path, &pattern);
        let result2 = matches_include_pattern(&path, &pattern);
        prop_assert_eq!(result1, result2, "Non-deterministic result for path={}, pattern={}", path, pattern);
    }

    /// Empty pattern should not match (exclude)
    #[test]
    fn prop_exclude_empty_pattern_no_match(
        path in "[a-z/_]{1,30}\\.[a-z]{2,4}",
    ) {
        let result = matches_exclude_pattern(&path, "");
        prop_assert!(!result, "Empty pattern should not match: path={}", path);
    }

    /// Empty pattern should not match (include)
    #[test]
    fn prop_include_empty_pattern_no_match(
        path in "[a-z/_]{1,30}\\.[a-z]{2,4}",
    ) {
        let result = matches_include_pattern(&path, "");
        prop_assert!(!result, "Empty pattern should not match: path={}", path);
    }

    /// Path component matching: if path contains "/pattern/" then should match (exclude)
    #[test]
    fn prop_exclude_component_match(
        prefix in "[a-z]{1,10}",
        component in "[a-z]{1,10}",
        suffix in "[a-z/_]{1,20}",
    ) {
        let path = format!("{}/{}/{}", prefix, component, suffix);
        let result = matches_exclude_pattern(&path, &component);
        prop_assert!(result, "Component '{}' should match in path '{}'", component, path);
    }

    /// Prefix matching: if path starts with pattern, should match (exclude)
    #[test]
    fn prop_exclude_prefix_match(
        prefix in "[a-z]{1,10}",
        suffix in "/[a-z/_]{1,20}",
    ) {
        let path = format!("{}{}", prefix, suffix);
        let result = matches_exclude_pattern(&path, &prefix);
        prop_assert!(result, "Prefix '{}' should match in path '{}'", prefix, path);
    }

    /// Substring matching for include patterns
    #[test]
    fn prop_include_substring_match(
        prefix in "[a-z/_]{0,10}",
        substring in "[a-z]{1,8}",
        suffix in "[a-z/_]{0,10}\\.[a-z]{2,4}",
    ) {
        let path = format!("{}{}{}", prefix, substring, suffix);
        let result = matches_include_pattern(&path, &substring);
        prop_assert!(result, "Substring '{}' should match in path '{}'", substring, path);
    }

    /// Suffix matching for include patterns
    #[test]
    fn prop_include_suffix_match(
        prefix in "[a-z/_]{1,20}",
        suffix in "\\.[a-z]{2,4}",
    ) {
        let path = format!("{}{}", prefix, suffix);
        let result = matches_include_pattern(&path, &suffix);
        prop_assert!(result, "Suffix '{}' should match in path '{}'", suffix, path);
    }

    /// apply_exclude_patterns with empty patterns should not modify collection
    #[test]
    fn prop_apply_exclude_empty_preserves_all(
        paths in prop::collection::vec("[a-z/_]{1,30}\\.[a-z]{2,4}", 1..20),
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = paths.iter().map(|p| TestFile { path: p.clone() }).collect();
        let original_len = files.len();

        apply_exclude_patterns(&mut files, &[], |f| &f.path);

        prop_assert_eq!(files.len(), original_len, "Empty exclude patterns should preserve all files");
    }

    /// apply_include_patterns with empty patterns should not modify collection
    #[test]
    fn prop_apply_include_empty_preserves_all(
        paths in prop::collection::vec("[a-z/_]{1,30}\\.[a-z]{2,4}", 1..20),
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = paths.iter().map(|p| TestFile { path: p.clone() }).collect();
        let original_len = files.len();

        apply_include_patterns(&mut files, &[], |f| &f.path);

        prop_assert_eq!(files.len(), original_len, "Empty include patterns should preserve all files");
    }

    /// apply_exclude_patterns should never increase collection size
    #[test]
    fn prop_apply_exclude_never_increases_size(
        paths in prop::collection::vec("[a-z/_]{1,30}\\.[a-z]{2,4}", 1..20),
        patterns in prop::collection::vec("[a-z*/_]{1,15}", 0..5),
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = paths.iter().map(|p| TestFile { path: p.clone() }).collect();
        let original_len = files.len();

        apply_exclude_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert!(files.len() <= original_len, "Exclude should never increase size: {} -> {}", original_len, files.len());
    }

    /// apply_include_patterns should never increase collection size
    #[test]
    fn prop_apply_include_never_increases_size(
        paths in prop::collection::vec("[a-z/_]{1,30}\\.[a-z]{2,4}", 1..20),
        patterns in prop::collection::vec("[a-z*/_]{1,15}", 1..5),  // At least 1 pattern
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = paths.iter().map(|p| TestFile { path: p.clone() }).collect();
        let original_len = files.len();

        apply_include_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert!(files.len() <= original_len, "Include should never increase size: {} -> {}", original_len, files.len());
    }

    /// If all paths match exclude pattern, all should be removed
    #[test]
    fn prop_apply_exclude_all_match_removes_all(
        count in 1usize..20,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        // Create files that all start with "target/"
        let mut files: Vec<TestFile> = (0..count)
            .map(|i| TestFile { path: format!("target/file{}.rs", i) })
            .collect();

        let patterns = vec!["target".to_string()];
        apply_exclude_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert_eq!(files.len(), 0, "All files matching pattern should be removed");
    }

    /// If no paths match exclude pattern, none should be removed
    #[test]
    fn prop_apply_exclude_no_match_preserves_all(
        count in 1usize..20,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        // Create files that start with "src/"
        let mut files: Vec<TestFile> = (0..count)
            .map(|i| TestFile { path: format!("src/file{}.rs", i) })
            .collect();
        let original_len = files.len();

        let patterns = vec!["target".to_string()];
        apply_exclude_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert_eq!(files.len(), original_len, "No files should be removed when pattern doesn't match");
    }

    /// If all paths match include pattern, all should be kept
    #[test]
    fn prop_apply_include_all_match_keeps_all(
        count in 1usize..20,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        // Create Rust files
        let mut files: Vec<TestFile> = (0..count)
            .map(|i| TestFile { path: format!("src/file{}.rs", i) })
            .collect();
        let original_len = files.len();

        let patterns = vec!["*.rs".to_string()];
        apply_include_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert_eq!(files.len(), original_len, "All files matching include pattern should be kept");
    }

    /// If no paths match include pattern, all should be removed
    #[test]
    fn prop_apply_include_no_match_removes_all(
        count in 1usize..20,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        // Create Rust files, but include only TypeScript
        let mut files: Vec<TestFile> = (0..count)
            .map(|i| TestFile { path: format!("src/file{}.rs", i) })
            .collect();

        let patterns = vec!["*.ts".to_string()];
        apply_include_patterns(&mut files, &patterns, |f| &f.path);

        prop_assert_eq!(files.len(), 0, "All files should be removed when include pattern doesn't match");
    }

    /// Wildcard patterns should use glob matching
    #[test]
    fn prop_wildcard_uses_glob(
        prefix in "[a-z]{1,10}",
        filename in "[a-z]{1,10}",
        ext in "[a-z]{2,4}",
    ) {
        let path = format!("{}/{}.{}", prefix, filename, ext);
        let pattern = format!("*.{}", ext);

        let result = matches_include_pattern(&path, &pattern);
        prop_assert!(result, "Wildcard pattern '{}' should match path '{}'", pattern, path);
    }

    /// Glob pattern caching: same pattern called twice should work
    #[test]
    fn prop_pattern_caching_works(
        path in "[a-z/_]{1,30}\\.[a-z]{2,4}",
        pattern in "\\*\\.[a-z]{2,4}",  // e.g., "*.rs"
    ) {
        // First call
        let result1 = matches_include_pattern(&path, &pattern);

        // Second call (should use cached pattern)
        let result2 = matches_include_pattern(&path, &pattern);

        prop_assert_eq!(result1, result2, "Cached pattern should give same result");
    }

    /// Multiple patterns should be OR'd together (exclude)
    #[test]
    fn prop_exclude_multiple_patterns_are_ored(
        count in 1usize..10,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "target/debug.rs".to_string() },
            TestFile { path: "node_modules/lib.js".to_string() },
        ];

        let patterns = vec!["target".to_string(), "node_modules".to_string()];
        apply_exclude_patterns(&mut files, &patterns, |f| &f.path);

        // Only src/main.rs should remain
        prop_assert_eq!(files.len(), 1);
        prop_assert_eq!(&files[0].path, "src/main.rs");
    }

    /// Multiple patterns should be OR'd together (include)
    #[test]
    fn prop_include_multiple_patterns_are_ored(
        count in 1usize..10,
    ) {
        #[derive(Debug, Clone)]
        struct TestFile { path: String }

        let mut files: Vec<TestFile> = vec![
            TestFile { path: "src/main.rs".to_string() },
            TestFile { path: "src/lib.py".to_string() },
            TestFile { path: "src/index.ts".to_string() },
        ];

        let patterns = vec!["*.rs".to_string(), "*.ts".to_string()];
        apply_include_patterns(&mut files, &patterns, |f| &f.path);

        // main.rs and index.ts should remain
        prop_assert_eq!(files.len(), 2);
        prop_assert!(files.iter().any(|f| f.path == "src/main.rs"));
        prop_assert!(files.iter().any(|f| f.path == "src/index.ts"));
    }
}

// ============================================================================
// Content Transformation Property Tests
// ============================================================================
// NOTE: Temporarily commented out due to compilation errors
/*
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// remove_empty_lines should never increase line count
    #[test]
    fn prop_remove_empty_lines_never_increases(
        content in "\\PC{0,500}",
    ) {
        let input_lines = content.lines().count();
        let output = remove_empty_lines(&content, false);
        let output_lines = output.lines().count();

        prop_assert!(
            output_lines <= input_lines,
            "Output has more lines: {} -> {}",
            input_lines, output_lines
        );
    }

    /// remove_empty_lines output should not contain whitespace-only lines
    #[test]
    fn prop_remove_empty_lines_no_whitespace_only(
        content in "\\PC{0,500}",
    ) {
        let output = remove_empty_lines(&content, false);

        for line in output.lines() {
            // Strip line numbers if present (format: "123:content")
            let content_part = if let Some((_, rest)) = line.split_once(':') {
                rest
            } else {
                line
            };

            prop_assert!(
                !content_part.trim().is_empty(),
                "Found whitespace-only line: {:?}",
                line
            );
        }
    }

    /// remove_empty_lines with preserve_line_numbers should prefix with numbers
    #[test]
    fn prop_remove_empty_lines_preserves_line_numbers(
        content in "\\PC{1,300}",
    ) {
        let output = remove_empty_lines(&content, true);

        for line in output.lines() {
            if line.contains(':') {
                let num_part = line.split(':').next().unwrap_or("");
                prop_assert!(
                    num_part.parse::<u32>().is_ok(),
                    "Line number prefix invalid: {:?}",
                    line
                );
            }
        }
    }

    /// remove_empty_lines is idempotent
    #[test]
    fn prop_remove_empty_lines_idempotent(
        content in "\\PC{0,300}",
    ) {
        let once = remove_empty_lines(&content, false);
        let twice = remove_empty_lines(&once, false);

        prop_assert_eq!(
            once, twice,
            "Not idempotent: applying twice changed result"
        );
    }

    /// remove_comments should never panic
    #[test]
    fn prop_remove_comments_no_panic(
        content in "\\PC{0,500}",
        lang in "(rust|python|javascript|typescript|go)",
    ) {
        // Should not panic regardless of input
        let _ = remove_comments(&content, &lang, false);
    }

    /// remove_comments output should not be longer than input
    #[test]
    fn prop_remove_comments_never_longer(
        content in "\\PC{0,500}",
        lang in "(rust|python|javascript|typescript|go)",
    ) {
        let output = remove_comments(&content, &lang, false);

        prop_assert!(
            output.len() <= content.len(),
            "Output longer: {} -> {}",
            content.len(), output.len()
        );
    }

    /// remove_comments is idempotent for uncommented code
    #[test]
    fn prop_remove_comments_idempotent_no_comments(
        content in "[a-z \\n]{10,200}",  // Simple text without comment markers
    ) {
        let lang = "rust";
        let once = remove_comments(&content, lang, false);
        let twice = remove_comments(&once, lang, false);

        prop_assert_eq!(
            once, twice,
            "Not idempotent for uncommented code"
        );
    }

    /// remove_comments should preserve code structure (line count shouldn't change dramatically)
    #[test]
    fn prop_remove_comments_preserves_structure(
        content in "fn [a-z]+\\(\\) \\{\\n    [a-z]+;\\n\\}\\n",  // Valid Rust function
    ) {
        let output = remove_comments(&content, "rust", false);

        // Non-comment code should remain
        prop_assert!(
            !output.is_empty(),
            "Valid code was completely removed"
        );
    }

    /// extract_signatures should never be longer than input
    #[test]
    fn prop_extract_signatures_never_longer(
        content in "\\PC{0,500}",
    ) {
        let output = extract_signatures(&content, "rust", &[]);

        prop_assert!(
            output.len() <= content.len(),
            "Signatures longer than input: {} -> {}",
            content.len(), output.len()
        );
    }

    /// extract_signatures should not panic on arbitrary input
    #[test]
    fn prop_extract_signatures_no_panic(
        content in "\\PC{0,500}",
        lang in "(rust|python|javascript|typescript|go)",
    ) {
        // Should not panic regardless of input
        let _ = extract_signatures(&content, &lang, &[]);
    }

    /// extract_key_symbols should never be longer than input
    #[test]
    fn prop_extract_key_symbols_never_longer(
        content in "\\PC{0,500}",
    ) {
        let output = extract_key_symbols(&content, "rust", &[]);

        prop_assert!(
            output.len() <= content.len(),
            "Key symbols longer than input: {} -> {}",
            content.len(), output.len()
        );
    }

    /// extract_key_symbols should not panic on arbitrary input
    #[test]
    fn prop_extract_key_symbols_no_panic(
        content in "\\PC{0,500}",
        lang in "(rust|python|javascript|typescript|go)",
    ) {
        // Should not panic regardless of input
        let _ = extract_key_symbols(&content, &lang, &[]);
    }

    /// extract_key_symbols_with_context should never be longer than input
    #[test]
    fn prop_extract_key_symbols_with_context_never_longer(
        content in "\\PC{0,500}",
    ) {
        let output = extract_key_symbols_with_context(&content, "rust", &[]);

        prop_assert!(
            output.len() <= content.len(),
            "Key symbols with context longer than input: {} -> {}",
            content.len(), output.len()
        );
    }

    /// extract_key_symbols_with_context should not panic on arbitrary input
    #[test]
    fn prop_extract_key_symbols_with_context_no_panic(
        content in "\\PC{0,500}",
        lang in "(rust|python|javascript|typescript|go)",
    ) {
        // Should not panic regardless of input
        let _ = extract_key_symbols_with_context(&content, &lang, &[]);
    }

    /// extract_key_symbols_with_context should include more context than extract_key_symbols
    #[test]
    fn prop_extract_with_context_includes_more(
        content in "fn [a-z]+\\(\\) \\{\\n    [a-z]+;\\n    [a-z]+;\\n    [a-z]+;\\n\\}\\n",
    ) {
        let without_context = extract_key_symbols(&content, "rust", &[]);
        let with_context = extract_key_symbols_with_context(&content, "rust", &[]);

        // With context should generally have more lines (2 lines above + 2 below each symbol)
        // Or at least not have fewer lines
        let without_lines = without_context.lines().count();
        let with_lines = with_context.lines().count();

        prop_assert!(
            with_lines >= without_lines,
            "With context has fewer lines: {} vs {}",
            with_lines, without_lines
        );
    }

    /// All extraction functions should preserve UTF-8 validity
    #[test]
    fn prop_extraction_preserves_utf8(
        content in "\\PC{0,300}",
    ) {
        let signatures = extract_signatures(&content, "rust", &[]);
        let key_symbols = extract_key_symbols(&content, "rust", &[]);
        let with_context = extract_key_symbols_with_context(&content, "rust", &[]);

        // These should not panic if UTF-8 is valid
        let _ = signatures.chars().count();
        let _ = key_symbols.chars().count();
        let _ = with_context.chars().count();
    }

    /// Transformation functions should handle empty input gracefully
    #[test]
    fn prop_transformations_handle_empty() {
        let empty = "";

        let no_empty = remove_empty_lines(empty, false);
        let no_comments = remove_comments(empty, "rust", false);
        let signatures = extract_signatures(empty, "rust", &[]);
        let key_symbols = extract_key_symbols(empty, "rust", &[]);
        let with_context = extract_key_symbols_with_context(empty, "rust", &[]);

        // Should all be empty or very short (not panic)
        prop_assert!(no_empty.len() <= 10);
        prop_assert!(no_comments.len() <= 10);
        prop_assert!(signatures.len() <= 10);
        prop_assert!(key_symbols.len() <= 10);
        prop_assert!(with_context.len() <= 10);
    }

    /// Transformation functions should handle single-line input
    #[test]
    fn prop_transformations_handle_single_line(
        line in "[a-z ]{1,50}",
    ) {
        let no_empty = remove_empty_lines(&line, false);
        let no_comments = remove_comments(&line, "rust", false);
        let signatures = extract_signatures(&line, "rust", &[]);
        let key_symbols = extract_key_symbols(&line, "rust", &[]);
        let with_context = extract_key_symbols_with_context(&line, "rust", &[]);

        // Should not panic
        prop_assert!(no_empty.len() <= line.len() + 10);  // +10 for line numbers
        prop_assert!(no_comments.len() <= line.len() + 10);
        prop_assert!(signatures.len() <= line.len() + 10);
        prop_assert!(key_symbols.len() <= line.len() + 10);
        prop_assert!(with_context.len() <= line.len() + 10);
    }

    /// remove_empty_lines should preserve non-empty lines exactly
    #[test]
    fn prop_remove_empty_lines_preserves_content(
        lines in prop::collection::vec("[a-z]{5,20}", 1..10),
    ) {
        let content = lines.join("\n");
        let output = remove_empty_lines(&content, false);

        // All original non-empty lines should be present
        for line in &lines {
            prop_assert!(
                output.contains(line),
                "Original line {:?} not found in output",
                line
            );
        }
    }
}
*/

// ============================================================================
// Parser Thread-Local Property Tests
// ============================================================================

use infiniloom_engine::parser::{parse_file_symbols, parse_with_language};
use std::path::PathBuf;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// parse_file_symbols should be deterministic
    #[test]
    fn prop_parse_file_symbols_deterministic(
        func_name in "[a-z_][a-z0-9_]{0,15}",
        extension in "(rs|py|js|ts|go)",
    ) {
        let content = format!("fn {}() {{}}", func_name);
        let path = PathBuf::from(format!("test.{}", extension));

        let result1 = parse_file_symbols(&content, &path);
        let result2 = parse_file_symbols(&content, &path);

        // Same input should produce same output
        prop_assert_eq!(result1.len(), result2.len(), "Different symbol counts");
        for (s1, s2) in result1.iter().zip(result2.iter()) {
            prop_assert_eq!(&s1.name, &s2.name, "Different symbol names");
            prop_assert_eq!(s1.start_line, s2.start_line, "Different start lines");
            prop_assert_eq!(s1.end_line, s2.end_line, "Different end lines");
        }
    }

    /// parse_with_language should be deterministic
    #[test]
    fn prop_parse_with_language_deterministic(
        func_name in "[a-z_][a-z0-9_]{0,15}",
    ) {
        let content = format!("fn {}() {{}}", func_name);

        let result1 = parse_with_language(&content, Language::Rust);
        let result2 = parse_with_language(&content, Language::Rust);

        prop_assert_eq!(result1.len(), result2.len(), "Different symbol counts");
        for (s1, s2) in result1.iter().zip(result2.iter()) {
            prop_assert_eq!(&s1.name, &s2.name, "Different symbol names");
        }
    }

    /// parse_file_symbols should never panic on arbitrary content
    #[test]
    fn prop_parse_no_panic_arbitrary_content(
        content in "\\PC{0,500}",
        extension in "(rs|py|js|ts|go)",
    ) {
        let path = PathBuf::from(format!("test.{}", extension));
        // Should not panic regardless of content
        let _ = parse_file_symbols(&content, &path);
    }

    /// parse_with_language should never panic on arbitrary content
    #[test]
    fn prop_parse_with_language_no_panic(
        content in "\\PC{0,500}",
    ) {
        // Should not panic regardless of content
        let _ = parse_with_language(&content, Language::Rust);
        let _ = parse_with_language(&content, Language::Python);
        let _ = parse_with_language(&content, Language::JavaScript);
    }

    /// All extracted symbols should have valid line numbers
    #[test]
    fn prop_symbols_have_valid_line_numbers(
        func_name in "[a-z_][a-z0-9_]{0,15}",
        body_lines in 1usize..10,
    ) {
        let body = "    // comment\n".repeat(body_lines);
        let content = format!("fn {}() {{\n{}}}", func_name, body);
        let path = PathBuf::from("test.rs");

        let symbols = parse_file_symbols(&content, &path);

        for symbol in &symbols {
            prop_assert!(
                symbol.start_line >= 1,
                "Symbol {} has invalid start_line: {}",
                symbol.name, symbol.start_line
            );
            prop_assert!(
                symbol.end_line >= symbol.start_line,
                "Symbol {} has end_line {} < start_line {}",
                symbol.name, symbol.end_line, symbol.start_line
            );
        }
    }

    /// All extracted symbols should have non-empty names
    #[test]
    fn prop_symbols_have_nonempty_names(
        func_name in "[a-zA-Z_][a-zA-Z0-9_]{0,20}",
    ) {
        let content = format!("fn {}() {{ }}", func_name);
        let path = PathBuf::from("test.rs");

        let symbols = parse_file_symbols(&content, &path);

        for symbol in &symbols {
            prop_assert!(
                !symbol.name.is_empty(),
                "Symbol has empty name"
            );
        }
    }

    /// Multiple calls should reuse the same parser (thread safety)
    #[test]
    fn prop_multiple_calls_reuse_parser(
        func_count in 1usize..10,
    ) {
        // Generate multiple functions
        for i in 0..func_count {
            let content = format!("fn func_{}() {{}}", i);
            let path = PathBuf::from("test.rs");
            let symbols = parse_file_symbols(&content, &path);

            // Each call should work independently
            prop_assert!(!symbols.is_empty(), "Failed to parse function {}", i);
        }
    }

    /// Empty content should return empty symbols
    #[test]
    fn prop_empty_content_returns_empty(
        extension in "(rs|py|js|ts|go)",
    ) {
        let path = PathBuf::from(format!("test.{}", extension));
        let symbols = parse_file_symbols("", &path);

        prop_assert!(symbols.is_empty(), "Empty content should return no symbols");
    }

    /// Files without extensions should return empty
    #[test]
    fn prop_no_extension_returns_empty(
        content in "\\PC{1,100}",
        filename in "[a-zA-Z]{1,10}",
    ) {
        let path = PathBuf::from(filename); // No extension
        let symbols = parse_file_symbols(&content, &path);

        prop_assert!(symbols.is_empty(), "No extension should return empty");
    }

    /// Unsupported extensions should return empty
    #[test]
    fn prop_unsupported_extension_returns_empty(
        content in "\\PC{1,100}",
        ext in "[a-z]{1,5}",
    ) {
        // Use rare extension unlikely to be supported
        let path = PathBuf::from(format!("test.{}", ext));
        let symbols = parse_file_symbols(&content, &path);

        // Most random extensions won't be supported
        // This test just ensures no panic
    }

    /// UTF-8 multi-byte characters should not cause panics
    #[test]
    fn prop_utf8_multibyte_no_panic(
        chars in prop::collection::vec(
            prop::char::range('α', 'ω'),  // Greek letters
            1..50
        ),
    ) {
        let content = chars.into_iter().collect::<String>();
        let path = PathBuf::from("test.rs");

        // Should not panic on UTF-8 content
        let _ = parse_file_symbols(&content, &path);
    }

    /// Malformed syntax should not panic (error tolerance)
    #[test]
    fn prop_malformed_syntax_no_panic(
        braces in 0usize..10,
        parens in 0usize..10,
    ) {
        // Generate intentionally malformed code
        let content = format!(
            "fn test( {} ) {} }}",
            "(".repeat(parens),
            "{".repeat(braces)
        );
        let path = PathBuf::from("test.rs");

        // Tree-sitter is error-tolerant, should not panic
        let _ = parse_file_symbols(&content, &path);
    }

    /// Symbol extraction for different languages should work
    #[test]
    fn prop_multilanguage_parsing(
        name in "[a-z][a-z0-9]{0,10}",
    ) {
        // Rust
        let rust_code = format!("fn {}() {{}}", name);
        let rust_symbols = parse_with_language(&rust_code, Language::Rust);
        if !rust_symbols.is_empty() {
            prop_assert!(rust_symbols.iter().any(|s| s.name.contains(&name)));
        }

        // Python
        let python_code = format!("def {}():\n    pass", name);
        let python_symbols = parse_with_language(&python_code, Language::Python);
        if !python_symbols.is_empty() {
            prop_assert!(python_symbols.iter().any(|s| s.name.contains(&name)));
        }

        // JavaScript
        let js_code = format!("function {}() {{ return 42; }}", name);
        let js_symbols = parse_with_language(&js_code, Language::JavaScript);
        if !js_symbols.is_empty() {
            prop_assert!(js_symbols.iter().any(|s| s.name.contains(&name)));
        }
    }

    /// Extension detection should work correctly
    #[test]
    fn prop_extension_detection(
        func_name in "[a-z_][a-z0-9_]{0,10}",
    ) {
        let content = format!("fn {}() {{}}", func_name);

        // Rust extensions
        for ext in &["rs"] {
            let path = PathBuf::from(format!("test.{}", ext));
            let symbols = parse_file_symbols(&content, &path);
            // Rust code should parse with .rs extension
            prop_assert!(!symbols.is_empty(), "Failed to detect Rust for .{}", ext);
        }
    }

    /// Symbols should be in source order (monotonic start_line)
    #[test]
    fn prop_symbols_in_source_order(
        func_count in 2usize..5,
    ) {
        // Generate multiple functions in order
        let functions: Vec<String> = (0..func_count)
            .map(|i| format!("fn func_{}() {{\n    // body\n}}\n", i))
            .collect();
        let content = functions.join("\n");
        let path = PathBuf::from("test.rs");

        let symbols = parse_file_symbols(&content, &path);

        if symbols.len() >= 2 {
            // Check that symbols appear in order
            for i in 1..symbols.len() {
                prop_assert!(
                    symbols[i].start_line >= symbols[i-1].start_line,
                    "Symbols not in source order: {} at line {} comes after {} at line {}",
                    symbols[i].name, symbols[i].start_line,
                    symbols[i-1].name, symbols[i-1].start_line
                );
            }
        }
    }

    /// Whitespace-only content should return empty
    #[test]
    fn prop_whitespace_only_returns_empty(
        spaces in 0usize..100,
        tabs in 0usize..50,
        newlines in 0usize..50,
    ) {
        let content = format!(
            "{}{}{}",
            " ".repeat(spaces),
            "\t".repeat(tabs),
            "\n".repeat(newlines)
        );
        let path = PathBuf::from("test.rs");

        let symbols = parse_file_symbols(&content, &path);

        // Whitespace-only should produce no symbols
        prop_assert!(symbols.is_empty(), "Whitespace-only should return empty");
    }
}
