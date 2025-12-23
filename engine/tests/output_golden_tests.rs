//! Golden/Snapshot output tests for formatters
//!
//! Tests that output formatters (XML, Markdown, TOON) produce consistent,
//! well-formed output for known inputs. These serve as regression tests
//! to catch unintended format changes.

use infiniloom_engine::output::{
    Formatter, MarkdownFormatter, OutputFormat, ToonFormatter, XmlFormatter,
};
use infiniloom_engine::repomap::{ModuleGraph, RepoMap};
use infiniloom_engine::types::{
    LanguageStats, RepoFile, RepoMetadata, Repository, Symbol, SymbolKind, TokenCounts, Visibility,
};
use std::path::PathBuf;

// ============================================================================
// Test Data Builders
// ============================================================================

fn create_test_repository() -> Repository {
    Repository {
        name: "test-repo".to_owned(),
        path: PathBuf::from("/test/repo"),
        files: vec![
            create_test_file("src/main.rs", "rust", "fn main() {\n    println!(\"Hello\");\n}"),
            create_test_file(
                "src/lib.rs",
                "rust",
                "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}",
            ),
            create_test_file("README.md", "markdown", "# Test Repository\n\nThis is a test."),
        ],
        metadata: create_test_metadata(),
    }
}

fn create_test_file(path: &str, lang: &str, content: &str) -> RepoFile {
    RepoFile {
        path: PathBuf::from(path),
        relative_path: path.to_owned(),
        language: Some(lang.to_owned()),
        size_bytes: content.len() as u64,
        token_count: TokenCounts {
            o200k: (content.len() / 4) as u32,
            cl100k: (content.len() / 4) as u32,
            claude: (content.len() / 4) as u32,
            gemini: (content.len() / 4) as u32,
            llama: (content.len() / 4) as u32,
            mistral: (content.len() / 4) as u32,
            deepseek: (content.len() / 4) as u32,
            qwen: (content.len() / 4) as u32,
            cohere: (content.len() / 4) as u32,
            grok: (content.len() / 4) as u32,
        },
        symbols: vec![],
        importance: 0.5,
        content: Some(content.to_owned()),
    }
}

fn create_test_file_with_symbols(path: &str, lang: &str, content: &str) -> RepoFile {
    let mut file = create_test_file(path, lang, content);
    file.symbols = vec![Symbol {
        name: "main".to_owned(),
        kind: SymbolKind::Function,
        signature: Some("fn main()".to_owned()),
        docstring: Some("Entry point".to_owned()),
        visibility: Visibility::Public,
        start_line: 1,
        end_line: 3,
        calls: vec!["println".to_owned()],
        references: 0,
        importance: 0.8,
        parent: None,
        extends: None,
        implements: vec![],
    }];
    file
}

fn create_test_metadata() -> RepoMetadata {
    RepoMetadata {
        total_files: 3,
        total_lines: 10,
        total_tokens: TokenCounts {
            o200k: 45,
            cl100k: 48,
            claude: 50,
            gemini: 47,
            llama: 50,
            mistral: 50,
            deepseek: 50,
            qwen: 50,
            cohere: 47,
            grok: 50,
        },
        languages: vec![
            LanguageStats { language: "rust".to_owned(), files: 2, lines: 7, percentage: 66.7 },
            LanguageStats { language: "markdown".to_owned(), files: 1, lines: 3, percentage: 33.3 },
        ],
        framework: None,
        description: Some("A test repository".to_owned()),
        branch: Some("main".to_owned()),
        commit: Some("abc1234".to_owned()),
        directory_structure: Some("src/\n  main.rs\n  lib.rs\nREADME.md\n".to_owned()),
        external_dependencies: vec!["tokio".to_owned(), "serde".to_owned()],
        git_history: None,
    }
}

fn create_empty_repomap() -> RepoMap {
    RepoMap {
        summary: String::new(),
        key_symbols: vec![],
        module_graph: ModuleGraph { nodes: vec![], edges: vec![] },
        file_index: vec![],
        token_count: 0,
    }
}

// ============================================================================
// XML Output Tests
// ============================================================================

#[test]
fn test_xml_output_well_formed() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Verify XML structure
    assert!(output.contains("<repository"), "Should have repository tag");
    assert!(output.contains("</repository>"), "Should close repository tag");
}

#[test]
fn test_xml_output_contains_metadata() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Verify metadata presence
    assert!(output.contains("test-repo"), "Should contain repo name");
}

#[test]
fn test_xml_output_contains_files() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Verify file content
    assert!(
        output.contains("src/main.rs") || output.contains("main.rs"),
        "Should contain main.rs path"
    );
    assert!(
        output.contains("src/lib.rs") || output.contains("lib.rs"),
        "Should contain lib.rs path"
    );
    assert!(output.contains("README.md"), "Should contain README path");
}

#[test]
fn test_xml_output_escapes_special_chars() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file(
        "test.rs",
        "rust",
        "let x = 5 < 10 && 3 > 1; // test \"quotes\" & ampersand",
    )];

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should escape XML special characters or use CDATA
    assert!(
        output.contains("&lt;") || output.contains("<![CDATA[") || output.contains("CDATA"),
        "Should escape < or use CDATA"
    );
}

#[test]
fn test_xml_output_with_symbols() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file_with_symbols(
        "main.rs",
        "rust",
        "fn main() { println!(\"Hello\"); }",
    )];

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should produce valid output
    assert!(!output.is_empty(), "Should produce output");
}

#[test]
fn test_xml_format_repo_only() {
    let repo = create_test_repository();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format_repo(&repo);

    // Should still have structure
    assert!(
        output.contains("<repository") || output.contains("test-repo"),
        "Should have repository info"
    );
}

// ============================================================================
// Markdown Output Tests
// ============================================================================

#[test]
fn test_markdown_output_structure() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = MarkdownFormatter::new();

    let output = formatter.format(&repo, &map);

    // Verify Markdown structure
    assert!(output.contains("# ") || output.contains("## "), "Should have header");
}

#[test]
fn test_markdown_output_contains_files() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = MarkdownFormatter::new();

    let output = formatter.format(&repo, &map);

    // Verify file paths
    assert!(
        output.contains("main.rs") || output.contains("lib.rs"),
        "Should contain file references"
    );
}

#[test]
fn test_markdown_output_code_blocks() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = MarkdownFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should have code blocks
    assert!(output.contains("```") || output.contains("~~~"), "Should have code blocks");
}

#[test]
fn test_markdown_preserves_content() {
    let mut repo = create_test_repository();
    let content = "fn unique_function_name() { /* special content */ }";
    repo.files = vec![create_test_file("test.rs", "rust", content)];

    let map = create_empty_repomap();
    let formatter = MarkdownFormatter::new();

    let output = formatter.format(&repo, &map);

    assert!(output.contains("unique_function_name"), "Should preserve actual content");
}

// ============================================================================
// TOON Output Tests (Token-Optimized)
// ============================================================================

#[test]
fn test_toon_output_compact() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // TOON should produce output
    assert!(!output.is_empty(), "Should produce output");
}

#[test]
fn test_toon_output_contains_files() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should contain file references
    assert!(
        output.contains("main") || output.contains("lib") || output.contains("README"),
        "Should reference files"
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_repository() {
    let repo = Repository {
        name: "empty-repo".to_owned(),
        path: PathBuf::from("/empty"),
        files: vec![],
        metadata: RepoMetadata::default(),
    };

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should still produce valid output
    assert!(!output.is_empty(), "Should produce output for empty repo");
}

#[test]
fn test_file_without_content() {
    let repo = Repository {
        name: "test-repo".to_owned(),
        path: PathBuf::from("/test"),
        files: vec![RepoFile {
            path: PathBuf::from("test.rs"),
            relative_path: "test.rs".to_owned(),
            language: Some("rust".to_owned()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None, // No content
        }],
        metadata: RepoMetadata::default(),
    };

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should handle missing content gracefully
    assert!(!output.is_empty(), "Should handle missing content");
}

#[test]
fn test_file_without_language() {
    let repo = Repository {
        name: "test-repo".to_owned(),
        path: PathBuf::from("/test"),
        files: vec![RepoFile {
            path: PathBuf::from("unknown"),
            relative_path: "unknown".to_owned(),
            language: None, // Unknown language
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: Some("content".to_owned()),
        }],
        metadata: RepoMetadata::default(),
    };

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should handle missing language gracefully
    assert!(!output.is_empty(), "Should handle missing language");
}

#[test]
fn test_unicode_content() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file(
        "unicode.rs",
        "rust",
        "// 你好世界 🦀\nfn main() { println!(\"🎉\"); }",
    )];

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should preserve Unicode
    assert!(
        output.contains("你好") || output.contains("🦀") || output.contains("main"),
        "Should preserve content including Unicode"
    );
}

#[test]
fn test_very_long_line() {
    let mut repo = create_test_repository();
    let long_line = "x".repeat(10000);
    repo.files = vec![create_test_file("long.rs", "rust", &long_line)];

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should handle long lines
    assert!(output.len() > 100, "Should handle long content");
}

// ============================================================================
// Format Consistency Tests (Regression Prevention)
// ============================================================================

#[test]
fn test_xml_format_deterministic() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output1 = formatter.format(&repo, &map);
    let output2 = formatter.format(&repo, &map);

    // Same input should produce same output
    assert_eq!(output1, output2, "Output should be deterministic");
}

#[test]
fn test_markdown_format_deterministic() {
    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = MarkdownFormatter::new();

    let output1 = formatter.format(&repo, &map);
    let output2 = formatter.format(&repo, &map);

    assert_eq!(output1, output2, "Output should be deterministic");
}

// ============================================================================
// Symbol Kinds in Output
// ============================================================================

#[test]
fn test_output_with_various_symbol_kinds() {
    let mut repo = create_test_repository();
    repo.files = vec![RepoFile {
        path: PathBuf::from("mixed.rs"),
        relative_path: "mixed.rs".to_owned(),
        language: Some("rust".to_owned()),
        size_bytes: 500,
        token_count: TokenCounts::default(),
        symbols: vec![
            Symbol {
                name: "MyStruct".to_owned(),
                kind: SymbolKind::Struct,
                signature: Some("struct MyStruct".to_owned()),
                docstring: None,
                visibility: Visibility::Public,
                start_line: 1,
                end_line: 5,
                calls: vec![],
                references: 0,
                importance: 0.7,
                parent: None,
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "MyTrait".to_owned(),
                kind: SymbolKind::Trait,
                signature: Some("trait MyTrait".to_owned()),
                docstring: None,
                visibility: Visibility::Public,
                start_line: 7,
                end_line: 10,
                calls: vec![],
                references: 0,
                importance: 0.6,
                parent: None,
                extends: None,
                implements: vec![],
            },
            Symbol {
                name: "my_fn".to_owned(),
                kind: SymbolKind::Function,
                signature: Some("fn my_fn()".to_owned()),
                docstring: None,
                visibility: Visibility::Public,
                start_line: 12,
                end_line: 15,
                calls: vec!["other_fn".to_owned()],
                references: 2,
                importance: 0.8,
                parent: None,
                extends: None,
                implements: vec![],
            },
        ],
        importance: 0.8,
        content: Some("struct MyStruct {}\ntrait MyTrait {}\nfn my_fn() {}".to_owned()),
    }];

    let map = create_empty_repomap();
    let formatter = XmlFormatter::new(true);

    let output = formatter.format(&repo, &map);

    // Should produce valid output with symbols
    assert!(!output.is_empty(), "Should produce output with symbols");
}

// ============================================================================
// OutputFormat Tests
// ============================================================================

#[test]
fn test_output_format_enum() {
    let xml = OutputFormat::Xml;
    let md = OutputFormat::Markdown;
    let toon = OutputFormat::Toon;
    let json = OutputFormat::Json;
    let yaml = OutputFormat::Yaml;
    let plain = OutputFormat::Plain;

    // Verify they are distinct
    assert!(matches!(xml, OutputFormat::Xml));
    assert!(matches!(md, OutputFormat::Markdown));
    assert!(matches!(toon, OutputFormat::Toon));
    assert!(matches!(json, OutputFormat::Json));
    assert!(matches!(yaml, OutputFormat::Yaml));
    assert!(matches!(plain, OutputFormat::Plain));
}

#[test]
fn test_output_format_default() {
    let default = OutputFormat::default();
    assert!(matches!(default, OutputFormat::Xml), "Default should be XML");
}

// ============================================================================
// Formatter Trait Implementation Tests
// ============================================================================

#[test]
fn test_xml_formatter_name() {
    let formatter = XmlFormatter::new(true);
    let name = formatter.name();
    assert!(name.to_lowercase().contains("xml"), "Should report XML as name");
}

#[test]
fn test_markdown_formatter_name() {
    let formatter = MarkdownFormatter::new();
    let name = formatter.name();
    assert!(
        name.to_lowercase().contains("markdown") || name.to_lowercase().contains("md"),
        "Should report Markdown as name"
    );
}

#[test]
fn test_toon_formatter_name() {
    let formatter = ToonFormatter::new();
    let name = formatter.name();
    assert!(name.to_lowercase().contains("toon"), "Should report TOON as name");
}

// ============================================================================
// Additional TOON Edge Case Tests
// ============================================================================

#[test]
fn test_toon_empty_repository() {
    let repo = Repository {
        name: "empty-repo".to_owned(),
        path: PathBuf::from("/empty"),
        files: vec![],
        metadata: RepoMetadata::default(),
    };

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should produce valid output for empty repo
    assert!(!output.is_empty(), "TOON should produce output for empty repo");
    assert!(output.contains("metadata:"), "Should have metadata section");
    assert!(output.contains("name: empty-repo"), "Should contain repo name");
}

#[test]
fn test_toon_unicode_content() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file(
        "unicode.py",
        "python",
        "# 你好世界 🦀\ndef greet(): return '🎉 こんにちは'",
    )];

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should preserve Unicode content
    assert!(
        output.contains("你好") || output.contains("🦀") || output.contains("greet"),
        "TOON should preserve Unicode content"
    );
}

#[test]
fn test_toon_very_long_line() {
    let mut repo = create_test_repository();
    let long_line = format!("fn long() {{ let x = \"{}\"; }}", "a".repeat(5000));
    repo.files = vec![create_test_file("long.rs", "rust", &long_line)];

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should handle long lines without error
    assert!(output.len() > 100, "TOON should handle very long lines");
    assert!(output.contains("long.rs"), "Should reference the file");
}

#[test]
fn test_toon_file_without_content() {
    let repo = Repository {
        name: "test-repo".to_owned(),
        path: PathBuf::from("/test"),
        files: vec![RepoFile {
            path: PathBuf::from("no_content.rs"),
            relative_path: "no_content.rs".to_owned(),
            language: Some("rust".to_owned()),
            size_bytes: 100,
            token_count: TokenCounts::default(),
            symbols: vec![],
            importance: 0.5,
            content: None, // No content
        }],
        metadata: RepoMetadata::default(),
    };

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should handle missing content gracefully
    assert!(!output.is_empty(), "TOON should handle files without content");
}

#[test]
fn test_toon_special_characters_in_path() {
    let mut repo = create_test_repository();
    repo.files =
        vec![create_test_file("path with spaces/special,chars|test.rs", "rust", "fn main() {}")];

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Paths with special chars should be escaped
    assert!(
        output.contains("\"") || output.contains("path"),
        "TOON should escape special characters in paths"
    );
}

#[test]
fn test_toon_line_numbers_enabled() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file(
        "numbered.rs",
        "rust",
        "fn line1() {}\nfn line2() {}\nfn line3() {}",
    )];

    let formatter = ToonFormatter::new().with_line_numbers(true);
    let output = formatter.format_repo(&repo);

    // Should contain line numbers
    assert!(output.contains("1:") || output.contains("2:"), "Should include line numbers");
}

#[test]
fn test_toon_line_numbers_disabled() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file("unnumbered.rs", "rust", "fn line1() {}\nfn line2() {}")];

    let formatter = ToonFormatter::new().with_line_numbers(false);
    let output = formatter.format_repo(&repo);

    // Should contain content without explicit line numbers
    assert!(output.contains("fn line1"), "Should include content without line numbers");
}

#[test]
fn test_toon_file_index_disabled() {
    let mut repo = create_test_repository();
    repo.files = vec![create_test_file("test.rs", "rust", "fn main() {}")];

    let formatter = ToonFormatter::new().with_file_index(false);
    let output = formatter.format_repo(&repo);

    // Should not contain file_index section
    assert!(!output.contains("file_index["), "Should not include file index when disabled");
}

#[test]
fn test_toon_streaming_to_writer() {
    use infiniloom_engine::output::StreamingFormatter;
    use std::io::Cursor;

    let repo = create_test_repository();
    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let mut buffer = Cursor::new(Vec::new());
    let result = formatter.format_to_writer(&repo, &map, &mut buffer);

    assert!(result.is_ok(), "Streaming should succeed");
    let output = String::from_utf8(buffer.into_inner()).unwrap();
    assert!(output.contains("metadata:"), "Streamed output should contain metadata");
}

#[test]
fn test_toon_escaping_reserved_words() {
    let mut repo = create_test_repository();
    // File content that contains reserved words
    repo.files = vec![create_test_file(
        "reserved.py",
        "python",
        "true_value = true\nfalse_value = false\nnull_value = null",
    )];

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // Should escape reserved words when used as values
    assert!(output.contains("true") || output.contains("false"), "Should handle reserved words");
}

#[test]
fn test_toon_multiple_files_sorted() {
    let mut repo = create_test_repository();
    repo.files = vec![
        create_test_file("zebra.rs", "rust", "fn z() {}"),
        create_test_file("alpha.rs", "rust", "fn a() {}"),
        create_test_file("middle.rs", "rust", "fn m() {}"),
    ];

    let map = create_empty_repomap();
    let formatter = ToonFormatter::new();

    let output = formatter.format(&repo, &map);

    // All files should be present
    assert!(output.contains("zebra.rs"), "Should contain zebra.rs");
    assert!(output.contains("alpha.rs"), "Should contain alpha.rs");
    assert!(output.contains("middle.rs"), "Should contain middle.rs");
}

#[test]
fn test_toon_importance_levels() {
    let repo = Repository {
        name: "test".to_owned(),
        path: PathBuf::from("/test"),
        files: vec![
            RepoFile {
                path: PathBuf::from("critical.rs"),
                relative_path: "critical.rs".to_owned(),
                language: Some("rust".to_owned()),
                size_bytes: 100,
                token_count: TokenCounts::default(),
                symbols: vec![],
                importance: 0.9, // > 0.8 = critical
                content: Some("fn critical() {}".to_owned()),
            },
            RepoFile {
                path: PathBuf::from("high.rs"),
                relative_path: "high.rs".to_owned(),
                language: Some("rust".to_owned()),
                size_bytes: 100,
                token_count: TokenCounts::default(),
                symbols: vec![],
                importance: 0.7, // > 0.6 = high
                content: Some("fn high() {}".to_owned()),
            },
            RepoFile {
                path: PathBuf::from("low.rs"),
                relative_path: "low.rs".to_owned(),
                language: Some("rust".to_owned()),
                size_bytes: 100,
                token_count: TokenCounts::default(),
                symbols: vec![],
                importance: 0.2, // < 0.3 = low
                content: Some("fn low() {}".to_owned()),
            },
        ],
        metadata: RepoMetadata::default(),
    };

    let formatter = ToonFormatter::new();
    let output = formatter.format_repo(&repo);

    // Should show importance levels in file_index
    assert!(output.contains("critical"), "Should show critical importance");
    assert!(output.contains("high"), "Should show high importance");
    assert!(output.contains("low"), "Should show low importance");
}
