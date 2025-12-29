#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // pattern_matches_file Tests
    // ============================================

    #[test]
    fn test_pattern_matches_file_exact_extension() {
        let pattern = glob::Pattern::new("*.rs").unwrap();
        assert!(pattern_matches_file(&pattern, "main.rs"));
        assert!(pattern_matches_file(&pattern, "src/lib.rs"));
        assert!(!pattern_matches_file(&pattern, "main.py"));
    }

    #[test]
    fn test_pattern_matches_file_directory_glob() {
        let pattern = glob::Pattern::new("src/**/*.rs").unwrap();
        assert!(pattern_matches_file(&pattern, "src/main.rs"));
        assert!(pattern_matches_file(&pattern, "src/utils/helper.rs"));
        assert!(!pattern_matches_file(&pattern, "tests/test.rs"));
    }

    #[test]
    fn test_pattern_matches_file_filename_only() {
        let pattern = glob::Pattern::new("Cargo.toml").unwrap();
        assert!(pattern_matches_file(&pattern, "Cargo.toml"));
        assert!(pattern_matches_file(&pattern, "subdir/Cargo.toml"));
    }

    #[test]
    fn test_pattern_matches_file_no_match() {
        let pattern = glob::Pattern::new("*.txt").unwrap();
        assert!(!pattern_matches_file(&pattern, "main.rs"));
        assert!(!pattern_matches_file(&pattern, "src/lib.py"));
    }

    // ============================================
    // is_inside_string Tests
    // ============================================

    #[test]
    fn test_is_inside_string_double_quotes_open() {
        assert!(is_inside_string("\"hello"));
    }

    #[test]
    fn test_is_inside_string_double_quotes_closed() {
        assert!(!is_inside_string("\"hello\""));
    }

    #[test]
    fn test_is_inside_string_single_quotes_open() {
        assert!(is_inside_string("'hello"));
    }

    #[test]
    fn test_is_inside_string_single_quotes_closed() {
        assert!(!is_inside_string("'hello'"));
    }

    #[test]
    fn test_is_inside_string_escaped_quote() {
        assert!(is_inside_string("\"hello\\\"")); // Ends with escaped quote, still open
    }

    #[test]
    fn test_is_inside_string_escaped_then_close() {
        assert!(!is_inside_string("\"hello\\\"world\"")); // Escaped quote then closing
    }

    #[test]
    fn test_is_inside_string_nested_quotes() {
        assert!(!is_inside_string("\"it's a test\"")); // Single inside double
        assert!(!is_inside_string("'he said \"hi\"'")); // Double inside single
    }

    #[test]
    fn test_is_inside_string_empty() {
        assert!(!is_inside_string(""));
    }

    #[test]
    fn test_is_inside_string_no_quotes() {
        assert!(!is_inside_string("hello world"));
    }

    // ============================================
    // remove_empty_lines_from_content Tests
    // ============================================

    #[test]
    fn test_remove_empty_lines_basic() {
        let input = "line1\n\nline2\n\n\nline3";
        let result = remove_empty_lines_from_content(input, false);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_remove_empty_lines_preserve_numbers() {
        // Line numbers are 1-indexed from original positions
        // line1 at index 0 -> "1:line1", line2 at index 2 -> "3:line2", line3 at index 4 -> "5:line3"
        let input = "line1\n\nline2\n\nline3";
        let result = remove_empty_lines_from_content(input, true);
        assert!(result.contains("1:line1"));
        assert!(result.contains("3:line2")); // Original line 3 (index 2)
        assert!(result.contains("5:line3")); // Original line 5 (index 4)
    }

    #[test]
    fn test_remove_empty_lines_with_embedded_numbers() {
        let input = "1:code here\n2:\n3:more code";
        let result = remove_empty_lines_from_content(input, true);
        assert!(result.contains("1:code here"));
        assert!(result.contains("3:more code"));
        assert!(!result.contains("2:"));
    }

    #[test]
    fn test_remove_empty_lines_whitespace_only() {
        let input = "line1\n   \nline2\n\t\nline3";
        let result = remove_empty_lines_from_content(input, false);
        assert_eq!(result, "line1\nline2\nline3");
    }

    // ============================================
    // remove_comments_from_content Tests
    // ============================================

    #[test]
    fn test_remove_comments_python() {
        let input = "# comment\ncode = 1\n# another comment\nmore_code = 2";
        let result = remove_comments_from_content(input, "python", false);
        assert!(result.contains("code = 1"));
        assert!(result.contains("more_code = 2"));
        assert!(!result.contains("# comment"));
    }

    #[test]
    fn test_remove_comments_rust_line() {
        let input = "// comment\nlet x = 1;\n// another\nlet y = 2;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(result.contains("let y = 2;"));
        assert!(!result.contains("// comment"));
    }

    #[test]
    fn test_remove_comments_rust_block() {
        let input = "/* block comment */\nlet x = 1;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(!result.contains("block comment"));
    }

    #[test]
    fn test_remove_comments_javascript() {
        let input = "// single\nconst x = 1;\n/* multi\nline */\nconst y = 2;";
        let result = remove_comments_from_content(input, "javascript", false);
        assert!(result.contains("const x = 1;"));
        assert!(result.contains("const y = 2;"));
        assert!(!result.contains("single"));
        assert!(!result.contains("multi"));
    }

    #[test]
    fn test_remove_comments_html() {
        let input = "<!-- comment -->\n<div>content</div>";
        let result = remove_comments_from_content(input, "html", false);
        assert!(result.contains("<div>content</div>"));
        assert!(!result.contains("comment"));
    }

    #[test]
    fn test_remove_comments_sql() {
        let input = "-- comment\nSELECT * FROM table;\n/* block */\nUPDATE table;";
        let result = remove_comments_from_content(input, "sql", false);
        assert!(result.contains("SELECT * FROM table;"));
        assert!(result.contains("UPDATE table;"));
        assert!(!result.contains("-- comment"));
    }

    #[test]
    fn test_remove_comments_preserves_string_comments() {
        let input = "let x = \"// not a comment\";\nlet y = 1;";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("\"// not a comment\""));
    }

    #[test]
    fn test_remove_comments_inline() {
        let input = "let x = 1; // inline comment";
        let result = remove_comments_from_content(input, "rust", false);
        assert!(result.contains("let x = 1;"));
        assert!(!result.contains("inline comment"));
    }

    // ============================================
    // budget_token_model_for Tests
    // ============================================

    #[test]
    fn test_budget_token_model_claude() {
        let result = budget_token_model_for(TokenizerModel::Claude);
        assert_eq!(result, TokenModel::Claude);
    }

    #[test]
    fn test_budget_token_model_gpt5_variants() {
        // All GPT-5 variants map to Gpt4o for budget
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt52), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt51), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt5), TokenModel::Gpt4o);
    }

    #[test]
    fn test_budget_token_model_o_series() {
        // O-series models map to Gpt4o for budget
        assert_eq!(budget_token_model_for(TokenizerModel::O4Mini), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::O3), TokenModel::Gpt4o);
        assert_eq!(budget_token_model_for(TokenizerModel::O1), TokenModel::Gpt4o);
    }

    #[test]
    fn test_budget_token_model_legacy_gpt4() {
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt4), TokenModel::Gpt4);
        assert_eq!(budget_token_model_for(TokenizerModel::Gpt35Turbo), TokenModel::Gpt4);
    }

    #[test]
    fn test_budget_token_model_other_vendors() {
        assert_eq!(budget_token_model_for(TokenizerModel::Gemini), TokenModel::Gemini);
        assert_eq!(budget_token_model_for(TokenizerModel::Llama), TokenModel::Llama);
        assert_eq!(budget_token_model_for(TokenizerModel::Mistral), TokenModel::Mistral);
        assert_eq!(budget_token_model_for(TokenizerModel::DeepSeek), TokenModel::DeepSeek);
        assert_eq!(budget_token_model_for(TokenizerModel::Qwen), TokenModel::Qwen);
        assert_eq!(budget_token_model_for(TokenizerModel::Cohere), TokenModel::Cohere);
        assert_eq!(budget_token_model_for(TokenizerModel::Grok), TokenModel::Grok);
    }

    // ============================================
    // estimate_tokens Tests
    // ============================================

    #[test]
    fn test_estimate_tokens_basic() {
        let text = "Hello, world!";
        let tokens = estimate_tokens(text, TokenizerModel::Claude);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = estimate_tokens("", TokenizerModel::Claude);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        let text = "This is a longer piece of text that should have more tokens.";
        let short_text = "Hi";
        let long_tokens = estimate_tokens(text, TokenizerModel::Claude);
        let short_tokens = estimate_tokens(short_text, TokenizerModel::Claude);
        assert!(long_tokens > short_tokens);
    }

    // ============================================
    // truncate_to_tokens Tests
    // ============================================

    #[test]
    fn test_truncate_to_tokens_no_truncation() {
        let text = "Hello, world!";
        let result = truncate_to_tokens(text, 1000, TokenizerModel::Claude);
        assert_eq!(result, text);
    }

    #[test]
    fn test_truncate_to_tokens_truncates() {
        let text = "This is some text. ".repeat(100);
        let result = truncate_to_tokens(&text, 50, TokenizerModel::Claude);
        assert!(result.len() < text.len());
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_truncate_to_tokens_empty() {
        let result = truncate_to_tokens("", 100, TokenizerModel::Claude);
        assert_eq!(result, "");
    }

    // ============================================
    // extract_signatures_heuristic Tests
    // ============================================

    #[test]
    fn test_extract_signatures_rust() {
        let content =
            "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() {\n    // do something\n}";
        let result = extract_signatures_heuristic(content, "rust");
        assert!(result.contains("fn main()"));
        assert!(result.contains("fn helper()"));
    }

    #[test]
    fn test_extract_signatures_python() {
        let content = "def main():\n    print('hello')\n\ndef helper():\n    pass\n\nclass MyClass:\n    pass";
        let result = extract_signatures_heuristic(content, "python");
        assert!(result.contains("def main()"));
        assert!(result.contains("def helper()"));
        assert!(result.contains("class MyClass"));
    }

    #[test]
    fn test_extract_signatures_javascript() {
        let content = "function main() {\n    console.log('hi');\n}\n\nconst helper = () => {};\n\nclass MyClass {}";
        let result = extract_signatures_heuristic(content, "javascript");
        assert!(result.contains("function main()"));
        assert!(result.contains("class MyClass"));
    }

    #[test]
    fn test_extract_signatures_typescript() {
        // TypeScript patterns: function, class, const, let, export, async
        let content = "function main(): void {\n}\n\nclass Config {\n    name: string;\n}\n\nconst result = { ok: true };";
        let result = extract_signatures_heuristic(content, "typescript");
        assert!(result.contains("function main()"));
        assert!(result.contains("class Config"));
        assert!(result.contains("const result"));
    }

    #[test]
    fn test_extract_signatures_go() {
        let content = "func main() {\n}\n\ntype Config struct {\n    Name string\n}";
        let result = extract_signatures_heuristic(content, "go");
        assert!(result.contains("func main()"));
        assert!(result.contains("type Config struct"));
    }

    #[test]
    fn test_extract_signatures_empty() {
        let result = extract_signatures_heuristic("", "rust");
        assert!(result.is_empty());
    }

    // ============================================
    // TokenTreeEntry Tests
    // ============================================

    #[test]
    fn test_token_tree_entry_serialization() {
        let entry = TokenTreeEntry { path: "src/main.rs".to_string(), tokens: 1000 };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("1000"));
    }

    // ============================================
    // SecurityIssueEntry Tests
    // ============================================

    #[test]
    fn test_security_issue_entry_serialization() {
        let entry = SecurityIssueEntry {
            file: "config.py".to_string(),
            line: 42,
            kind: "API_KEY".to_string(),
            severity: "High".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("config.py"));
        assert!(json.contains("42"));
        assert!(json.contains("API_KEY"));
        assert!(json.contains("High"));
    }

    // ============================================
    // append_yaml_block Tests
    // ============================================

    #[test]
    fn test_append_yaml_block_single_line() {
        let mut output = String::new();
        append_yaml_block(&mut output, "header", "Hello World");
        assert!(output.contains("\nheader: |\n"));
        assert!(output.contains("  Hello World\n"));
    }

    #[test]
    fn test_append_yaml_block_multi_line() {
        let mut output = String::new();
        append_yaml_block(&mut output, "description", "Line 1\nLine 2\nLine 3");
        assert!(output.contains("\ndescription: |\n"));
        assert!(output.contains("  Line 1\n"));
        assert!(output.contains("  Line 2\n"));
        assert!(output.contains("  Line 3\n"));
    }

    // ============================================
    // Integration Tests
    // ============================================

    #[test]
    fn test_remove_comments_then_empty_lines() {
        let input = "// comment\nlet x = 1;\n\n// another\nlet y = 2;\n\n";
        let without_comments = remove_comments_from_content(input, "rust", false);
        let result = remove_empty_lines_from_content(&without_comments, false);
        assert!(result.contains("let x = 1;"));
        assert!(result.contains("let y = 2;"));
        // Should not have empty lines or comments
        assert!(!result.contains("//"));
    }

    #[test]
    fn test_escape_chain() {
        // Test escaping special chars for XML then YAML
        let input = "foo & <bar> \"test\"";
        let xml_escaped = escape_xml_text(input);
        assert!(xml_escaped.contains("&amp;"));
        assert!(xml_escaped.contains("&lt;"));
        assert!(xml_escaped.contains("&quot;"));

        let yaml_escaped = escape_yaml_string(input);
        assert!(yaml_escaped.starts_with('"'));
        assert!(yaml_escaped.ends_with('"'));
        assert!(yaml_escaped.contains("\\\""));
    }
}
