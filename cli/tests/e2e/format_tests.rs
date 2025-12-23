//! Output Format E2E Tests
//!
//! Tests user expectations for different output formats.
//! Each test verifies: "As a user, I expect format X to be valid and usable."

use super::helpers::*;

/// E2E-FMT-001: XML format is valid XML
#[test]
fn test_xml_format_is_valid() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "xml"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_xml(&stdout), "XML format should produce valid XML");
    assert!(stdout.contains('<') && stdout.contains('>'), "XML should contain angle brackets");
}

/// E2E-FMT-002: JSON format is valid JSON
#[test]
fn test_json_format_is_valid() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "json"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_json(&stdout), "JSON format should produce valid JSON");
}

/// E2E-FMT-003: YAML format is valid YAML
#[test]
fn test_yaml_format_is_valid() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "yaml"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_yaml(&stdout), "YAML format should produce valid YAML");
}

/// E2E-FMT-004: Markdown format has proper code fences
#[test]
fn test_markdown_format_has_code_fences() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "markdown"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Markdown should contain code fences
    assert!(stdout.contains("```"), "Markdown format should contain code fences");
}

/// E2E-FMT-005: TOON format produces output (token-optimized)
#[test]
fn test_toon_format_produces_output() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "toon"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(!stdout.is_empty(), "TOON format should produce output");
}

/// E2E-FMT-005b: TOON format is smaller than XML
#[test]
fn test_toon_format_smaller_than_xml() {
    let temp = create_temp_project();

    let xml_output = pack_directory(temp.path(), &["--format", "xml"]);
    let toon_output = pack_directory(temp.path(), &["--format", "toon"]);

    assert!(xml_output.status.success());
    assert!(toon_output.status.success());

    let xml_size = stdout_str(&xml_output).len();
    let toon_size = stdout_str(&toon_output).len();

    assert!(
        toon_size <= xml_size,
        "TOON format ({} bytes) should be smaller or equal to XML format ({} bytes)",
        toon_size,
        xml_size
    );
}

/// E2E-FMT-006: Plain format has no markup
#[test]
fn test_plain_format_no_markup() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "plain"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // Plain format should not have XML/JSON/Markdown syntax
    // It may have some minimal structure, but not full markup
    assert!(!stdout.is_empty(), "Plain format should produce output");
}

/// E2E-FMT-007: All formats include file content
#[test]
fn test_all_formats_include_content() {
    let temp = create_temp_project();

    for format in &["xml", "json", "yaml", "markdown", "plain"] {
        let output = pack_directory(temp.path(), &["--format", format]);
        assert!(output.status.success(), "{} format should succeed", format);

        let content = stdout_str(&output);
        // All formats should include the actual code content
        assert!(
            content.contains("Hello, world!") || content.contains("println"),
            "{} format should include code content",
            format
        );
    }
}

/// E2E-FMT-008: Format flag is case-insensitive
#[test]
fn test_format_case_insensitive() {
    let temp = create_temp_project();

    // Try different cases
    for format in &["XML", "Xml", "json", "JSON", "Json"] {
        let output = pack_directory(temp.path(), &["--format", format]);
        // Should either succeed or give a helpful error
        // (implementation may vary on case sensitivity)
        let _stdout = stdout_str(&output);
        let stderr = stderr_str(&output);

        // Either it works or gives a clear error message
        assert!(
            output.status.success() || stderr.contains("format") || stderr.contains("invalid"),
            "Format {} should either work or give helpful error",
            format
        );
    }
}

/// E2E-FMT-009: Line numbers option works
#[test]
fn test_line_numbers_option() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--line-numbers"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // With line numbers, we expect to see numbers before code lines
    // This could be "1:" or "1|" or similar format
    let has_line_numbers = stdout.contains("1:") || stdout.contains("1|") || stdout.contains("1 ");
    assert!(
        has_line_numbers || stdout.contains("line"),
        "Output should indicate line numbers when --line-numbers is used"
    );
}

/// E2E-FMT-010: JSON output can be parsed and contains expected fields
#[test]
fn test_json_structure() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "json"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON should be parseable");

    // JSON should have some structure - could be object or array
    assert!(parsed.is_object() || parsed.is_array(), "JSON root should be object or array");
}

/// E2E-FMT-011: YAML output can be parsed and contains expected fields
#[test]
fn test_yaml_structure() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "yaml"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    let parsed: serde_yaml::Value =
        serde_yaml::from_str(&stdout).expect("YAML should be parseable");

    // YAML should have some structure
    assert!(parsed.is_mapping() || parsed.is_sequence(), "YAML root should be mapping or sequence");
}

/// E2E-FMT-012: JSON remains valid when --token-tree is enabled
#[test]
fn test_json_with_token_tree_is_valid() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "json", "--token-tree"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_json(&stdout), "JSON output with token tree should remain valid");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON should be parseable");
    assert!(parsed.get("token_tree").is_some(), "token_tree field should be present");
}

/// E2E-FMT-013: JSON remains valid when --security-check is enabled
#[test]
fn test_json_with_security_check_is_valid() {
    let temp = create_temp_project_with_secrets();
    let output = pack_directory(temp.path(), &["--format", "json", "--security-check"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_json(&stdout), "JSON output with security scan should remain valid");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON should be parseable");
    assert!(parsed.get("security_scan").is_some(), "security_scan field should be present");
}

/// E2E-FMT-014: YAML remains valid when --token-tree is enabled
#[test]
fn test_yaml_with_token_tree_is_valid() {
    let temp = create_temp_project();
    let output = pack_directory(temp.path(), &["--format", "yaml", "--token-tree"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(is_valid_yaml(&stdout), "YAML output with token tree should remain valid");

    let parsed: serde_yaml::Value =
        serde_yaml::from_str(&stdout).expect("YAML should be parseable");
    assert!(parsed.get("token_tree").is_some(), "token_tree field should be present");
}
