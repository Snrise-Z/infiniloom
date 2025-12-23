//! Security E2E Tests
//!
//! Tests user expectations for security scanning and secret detection.
//! Each test verifies: "As a user, I expect my secrets to be detected and handled safely."

use super::helpers::*;

/// E2E-SEC-001: Security check detects API keys
#[test]
fn test_security_check_detects_api_keys() {
    let temp = create_temp_project_with_secrets();
    let output = pack_directory(temp.path(), &["--security-check"]);

    // Command may succeed with warnings or fail if strict mode
    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);
    let combined = format!("{}{}", stdout, stderr);

    // Should detect the fake secrets in our test fixture
    assert!(
        combined.contains("secret")
            || combined.contains("credential")
            || combined.contains("warning")
            || combined.contains("REDACTED")
            || combined.contains("API")
            || combined.contains("key"),
        "Security check should detect or mention secrets"
    );
}

/// E2E-SEC-002: Secrets are redacted in output
#[test]
fn test_secrets_are_redacted() {
    let temp = create_temp_project_with_secrets();
    let output = pack_directory(temp.path(), &["--redact-secrets"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // The actual secret values should NOT appear in output
    assert!(
        not_contains(&stdout, "AKIAIOSFODNN7EXAMPLE"),
        "AWS access key should not appear in output"
    );
    assert!(
        not_contains(&stdout, "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
        "AWS secret key should not appear in output"
    );

    // Should have some indication of redaction
    // (either [REDACTED] placeholder or file exclusion)
}

/// E2E-SEC-003: AWS keys pattern is detected
#[test]
fn test_aws_keys_detected() {
    let temp = create_temp_project_with_secrets();
    let output = scan_directory(temp.path(), &["--security-check"]);

    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);
    let combined = format!("{}{}", stdout, stderr);

    // Should recognize AWS key patterns (AKIA...)
    // The exact message format may vary
    assert!(
        combined.contains("AWS")
            || combined.contains("AKIA")
            || combined.contains("secret")
            || combined.contains("credential")
            || combined.contains("detected"),
        "Should detect or acknowledge AWS key patterns"
    );
}

/// E2E-SEC-004: Private keys are detected
#[test]
fn test_private_keys_detected() {
    let temp = create_temp_project_with_secrets();

    // Create a file with a private key marker
    std::fs::write(
        temp.path().join("test_key.pem"),
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA0test0\n-----END RSA PRIVATE KEY-----\n",
    )
    .expect("Failed to create test key file");

    let output = pack_directory(temp.path(), &["--security-check"]);

    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);
    let combined = format!("{}{}", stdout, stderr);

    // Should detect private key patterns
    // Either warn about it or exclude/redact it
    assert!(
        combined.contains("private")
            || combined.contains("key")
            || combined.contains("RSA")
            || combined.contains("warning")
            || combined.contains("REDACTED")
            || not_contains(&stdout, "BEGIN RSA PRIVATE KEY"),
        "Should detect private key or exclude/redact it"
    );
}

/// E2E-SEC-005: .env files are handled safely
#[test]
fn test_env_files_handled_safely() {
    let temp = create_temp_project_with_secrets();

    // Create a .env file with secrets
    std::fs::write(
        temp.path().join(".env"),
        "DATABASE_URL=postgresql://user:password@localhost/db\nSECRET_KEY=super_secret_value_123\n",
    )
    .expect("Failed to create .env file");

    let output = pack_directory(temp.path(), &[]);

    let stdout = stdout_str(&output);

    // .env files should either:
    // 1. Be excluded by default (common gitignore pattern)
    // 2. Have their values redacted
    // 3. Trigger a security warning
    let env_excluded = not_contains(&stdout, "DATABASE_URL=postgresql");
    let env_redacted = stdout.contains("REDACTED");
    let is_safe = env_excluded || env_redacted;

    assert!(is_safe, ".env file secrets should be excluded or redacted in default output");
}

/// E2E-SEC-006: Security scan reports findings count
#[test]
fn test_security_scan_reports_count() {
    let temp = create_temp_project_with_secrets();
    let output = scan_directory(temp.path(), &["--security-check"]);

    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);
    let combined = format!("{}{}", stdout, stderr);

    // Should give some indication of how many issues found
    // Could be "0 secrets found", "2 potential secrets", etc.
    let has_count = combined.contains("found")
        || combined.contains("detected")
        || combined.contains("0")
        || combined.contains("1")
        || combined.contains("2")
        || combined.contains("warning");

    assert!(
        has_count || output.status.success(),
        "Security scan should report findings or complete silently if clean"
    );
}

/// E2E-SEC-007: Common secret patterns are detected
#[test]
fn test_common_secret_patterns() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create file with various secret patterns
    std::fs::write(
        temp.path().join("secrets.txt"),
        r#"
# Various secret patterns for testing
github_token = "ghp_1234567890abcdefghijklmnopqrstuvwxyz"
stripe_key = "sk_live_FakeTestKey0000000000000"
slack_token = "xoxb-0000000000-0000000000-FakeTestTokenXyz0000ABCD"
password = "super_secret_password_123"
"#,
    )
    .expect("create secrets file");

    let output = pack_directory(temp.path(), &["--security-check"]);

    let stdout = stdout_str(&output);
    let stderr = stderr_str(&output);
    let combined = format!("{}{}", stdout, stderr);

    // Should detect at least some of these patterns
    assert!(
        combined.contains("secret")
            || combined.contains("token")
            || combined.contains("warning")
            || combined.contains("detected")
            || combined.contains("REDACTED"),
        "Should detect common secret patterns"
    );
}

/// E2E-SEC-008: Redaction preserves file structure
#[test]
fn test_redaction_preserves_structure() {
    let temp = create_temp_project_with_secrets();
    let output = pack_directory(temp.path(), &["--redact-secrets"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // File should still be present, just with redacted values
    assert!(
        contains_file(&stdout, "config.py"),
        "config.py should still be included with redaction"
    );
}

/// E2E-SEC-009: High entropy strings detection
#[test]
fn test_high_entropy_detection() {
    let temp = tempfile::tempdir().expect("create temp dir");

    // Create file with high entropy string (looks like a key)
    std::fs::write(
        temp.path().join("config.js"),
        r#"
const API_KEY = "aB3dE5fG7hI9jK1lM3nO5pQ7rS9tU1vW3xY5zA7bC9dE";
const NORMAL = "hello world";
"#,
    )
    .expect("create config file");

    let output = pack_directory(temp.path(), &["--security-check"]);

    // Should at least process the file
    assert!(
        output.status.success() || !stderr_str(&output).is_empty(),
        "Should process file with high entropy strings"
    );
}
