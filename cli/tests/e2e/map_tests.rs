//! Map Command E2E Tests
//!
//! Tests user expectations for the `infiniloom map` command.

use super::helpers::*;

/// E2E-MAP-001: Map output includes summary and key symbols
#[test]
fn test_map_includes_key_symbols() {
    let temp = create_temp_project();
    let output = run_infiniloom(&["map", temp.path().to_str().unwrap()]);

    assert!(output.status.success(), "map command should succeed");

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Summary"), "map output should include summary section");
    assert!(stdout.contains("Key Symbols"), "map output should include key symbols section");
    assert!(
        stdout.contains("main") || stdout.contains("add") || stdout.contains("greet"),
        "map output should reference known symbols"
    );
}
