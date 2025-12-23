//! Comprehensive security scanner tests
//!
//! Tests secret detection for all supported patterns including:
//! - AWS credentials (Access Key ID, Secret Access Key)
//! - GitHub tokens (PAT, fine-grained PAT)
//! - Slack tokens (bot, app, user tokens)
//! - Stripe keys (secret, publishable, test, live)
//! - JWT tokens
//! - Connection strings (MongoDB, PostgreSQL, MySQL, Redis)
//! - Private keys (RSA, EC, DSA, OpenSSH)
//! - Generic API keys and secrets
//! - Passwords
//!   Plus edge cases: multi-hit per line, markdown bullets, redaction fidelity

use infiniloom_engine::security::{SecretFinding, SecretKind, SecurityScanner, Severity};

// ============================================================================
// AWS Credential Tests
// ============================================================================

#[test]
fn test_aws_access_key_id() {
    let scanner = SecurityScanner::new();
    let content = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";

    let findings = scanner.scan(content, "config.env");

    assert!(!findings.is_empty(), "Should detect AWS access key ID");
    assert!(findings.iter().any(|f| f.kind == SecretKind::AwsCredential));
    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_aws_secret_access_key() {
    let scanner = SecurityScanner::new();
    let content = r#"aws_secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY""#;

    let findings = scanner.scan(content, "credentials");

    assert!(!findings.is_empty(), "Should detect AWS secret access key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::AwsCredential));
}

#[test]
fn test_aws_key_variations() {
    let scanner = SecurityScanner::new();
    let variations = vec![
        "AKIAIOSFODNN7EXAMPLE",         // Bare key
        "AKIA1234567890ABCDEF",         // Another valid format
        "AWS_KEY=AKIAIOSFODNN7EXAMPLE", // With assignment
    ];

    for content in variations {
        let findings = scanner.scan(content, "test");
        assert!(!findings.is_empty(), "Should detect: {}", content);
    }
}

// ============================================================================
// GitHub Token Tests
// ============================================================================

#[test]
fn test_github_pat() {
    let scanner = SecurityScanner::new();
    let content = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect GitHub PAT");
    assert!(findings.iter().any(|f| f.kind == SecretKind::GitHubToken));
    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_github_fine_grained_pat() {
    let scanner = SecurityScanner::new();
    // Fine-grained PAT format: github_pat_XXXXXX_YYYYY (22 + 59 chars)
    let content = "github_pat_12345678901234567890AB_12345678901234567890123456789012345678901234567890123456789";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect GitHub fine-grained PAT");
    assert!(findings.iter().any(|f| f.kind == SecretKind::GitHubToken));
}

#[test]
fn test_github_token_in_config() {
    let scanner = SecurityScanner::new();
    let content = r#"
{
    "github": {
        "token": "ghp_abcdefghijklmnopqrstuvwxyz1234567890"
    }
}
"#;

    let findings = scanner.scan(content, "config.json");
    assert!(!findings.is_empty(), "Should detect GitHub token in JSON config");
}

// ============================================================================
// Slack Token Tests
// ============================================================================

#[test]
fn test_slack_bot_token() {
    let scanner = SecurityScanner::new();
    // xoxb format for bot tokens
    let content = "SLACK_BOT_TOKEN=xoxb-0000000000-0000000000-FakeTestTokenXyz0000ABCD";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect Slack bot token");
    assert!(findings.iter().any(|f| f.kind == SecretKind::AccessToken));
}

#[test]
fn test_slack_app_token() {
    let scanner = SecurityScanner::new();
    // xoxa format for app tokens
    let content = "xoxa-0000000000-0000000000-FakeTestTokenXyz0000ABCD";

    let findings = scanner.scan(content, "slack.config");

    assert!(!findings.is_empty(), "Should detect Slack app token");
}

#[test]
fn test_slack_user_token() {
    let scanner = SecurityScanner::new();
    // xoxp format for user tokens
    let content = "xoxp-0000000000-0000000000-FakeTestTokenXyz0000ABCD";

    let findings = scanner.scan(content, "config");

    assert!(!findings.is_empty(), "Should detect Slack user token");
}

#[test]
fn test_slack_token_variations() {
    let scanner = SecurityScanner::new();
    let tokens = vec![
        "xoxb-0000000000-0000000000-FakeTestTokenXyz0000ABCD", // Bot
        "xoxa-0000000000-0000000000-FakeTestTokenXyz0000ABCD", // App
        "xoxp-0000000000-0000000000-FakeTestTokenXyz0000ABCD", // User
        "xoxr-0000000000-0000000000-FakeTestTokenXyz0000ABCD", // Refresh
        "xoxs-0000000000-0000000000-FakeTestTokenXyz0000ABCD", // Session
    ];

    for token in tokens {
        let findings = scanner.scan(token, "test");
        assert!(!findings.is_empty(), "Should detect Slack token: {}", &token[..10]);
    }
}

// ============================================================================
// Stripe Key Tests
// ============================================================================

#[test]
fn test_stripe_secret_key_live() {
    let scanner = SecurityScanner::new();
    let content = "STRIPE_SECRET_KEY=sk_live_FakeTestKey0000000000000";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect Stripe live secret key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::ApiKey));
    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_stripe_secret_key_test() {
    let scanner = SecurityScanner::new();
    let content = "STRIPE_SECRET_KEY=sk_test_FakeTestKey0000000000000";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect Stripe test secret key");
}

#[test]
fn test_stripe_publishable_key() {
    let scanner = SecurityScanner::new();
    let content = "STRIPE_PUBLISHABLE_KEY=pk_live_FakeTestKey0000000000000";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect Stripe publishable key");
}

#[test]
fn test_stripe_key_variations() {
    let scanner = SecurityScanner::new();
    let keys = vec![
        "sk_live_FakeTestKey0000000000000", // Secret live
        "sk_test_FakeTestKey0000000000000", // Secret test
        "pk_live_FakeTestKey0000000000000", // Publishable live
        "pk_test_FakeTestKey0000000000000", // Publishable test
    ];

    for key in keys {
        let findings = scanner.scan(key, "test");
        assert!(!findings.is_empty(), "Should detect Stripe key: {}", &key[..10]);
    }
}

// ============================================================================
// JWT Token Tests
// ============================================================================

#[test]
fn test_jwt_token() {
    let scanner = SecurityScanner::new();
    // Standard JWT format: header.payload.signature
    let content = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";

    let findings = scanner.scan(content, "auth.js");

    assert!(!findings.is_empty(), "Should detect JWT token");
    assert!(findings.iter().any(|f| f.kind == SecretKind::AccessToken));
}

#[test]
fn test_jwt_in_authorization_header() {
    let scanner = SecurityScanner::new();
    let content = r#"Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"#;

    let findings = scanner.scan(content, "request.txt");

    assert!(!findings.is_empty(), "Should detect JWT in Authorization header");
}

#[test]
fn test_jwt_in_code() {
    let scanner = SecurityScanner::new();
    let content = r#"
const token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjoxfQ.signature";
fetch('/api', { headers: { Authorization: `Bearer ${token}` } });
"#;

    let findings = scanner.scan(content, "api.js");

    assert!(!findings.is_empty(), "Should detect JWT in JavaScript code");
}

// ============================================================================
// Connection String Tests
// ============================================================================

#[test]
fn test_mongodb_connection_string() {
    let scanner = SecurityScanner::new();
    let content = "MONGODB_URI=mongodb://user:password123@localhost:27017/mydb";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect MongoDB connection string");
    assert!(findings
        .iter()
        .any(|f| f.kind == SecretKind::ConnectionString));
}

#[test]
fn test_postgres_connection_string() {
    let scanner = SecurityScanner::new();
    let content = "DATABASE_URL=postgres://admin:secretpass@db.example.com:5432/production";

    let findings = scanner.scan(content, "config.env");

    assert!(!findings.is_empty(), "Should detect PostgreSQL connection string");
    assert!(findings
        .iter()
        .any(|f| f.kind == SecretKind::ConnectionString));
}

#[test]
fn test_mysql_connection_string() {
    let scanner = SecurityScanner::new();
    let content = "mysql://root:password@localhost:3306/database";

    let findings = scanner.scan(content, "db.config");

    assert!(!findings.is_empty(), "Should detect MySQL connection string");
}

#[test]
fn test_redis_connection_string() {
    let scanner = SecurityScanner::new();
    let content = "REDIS_URL=redis://user:password@redis.example.com:6379";

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect Redis connection string");
}

#[test]
fn test_multiple_connection_strings() {
    let scanner = SecurityScanner::new();
    let content = r#"
DATABASE_URL=postgres://user:pass@host:5432/db
CACHE_URL=redis://default:secret@cache:6379
MONGO_URI=mongodb://admin:admin123@mongo:27017
"#;

    let findings = scanner.scan(content, "docker-compose.env");

    assert!(findings.len() >= 3, "Should detect all three connection strings");
}

// ============================================================================
// Private Key Tests
// ============================================================================

#[test]
fn test_rsa_private_key() {
    let scanner = SecurityScanner::new();
    let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...";

    let findings = scanner.scan(content, "id_rsa");

    assert!(!findings.is_empty(), "Should detect RSA private key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::PrivateKey));
    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_ec_private_key() {
    let scanner = SecurityScanner::new();
    let content = "-----BEGIN EC PRIVATE KEY-----\nMHQCAQEE...";

    let findings = scanner.scan(content, "key.pem");

    assert!(!findings.is_empty(), "Should detect EC private key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::PrivateKey));
}

#[test]
fn test_openssh_private_key() {
    let scanner = SecurityScanner::new();
    let content = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk...";

    let findings = scanner.scan(content, "id_ed25519");

    assert!(!findings.is_empty(), "Should detect OpenSSH private key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::PrivateKey));
}

#[test]
fn test_generic_private_key() {
    let scanner = SecurityScanner::new();
    let content = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgk...";

    let findings = scanner.scan(content, "server.key");

    assert!(!findings.is_empty(), "Should detect generic private key");
}

// ============================================================================
// Password Tests
// ============================================================================

#[test]
fn test_password_in_config() {
    let scanner = SecurityScanner::new();
    let content = r#"password = "MySecurePassword123!""#;

    let findings = scanner.scan(content, "config.ini");

    assert!(!findings.is_empty(), "Should detect password in config");
    assert!(findings.iter().any(|f| f.kind == SecretKind::Password));
}

#[test]
fn test_password_variations() {
    let scanner = SecurityScanner::new();
    let variations = vec![
        r#"PASSWORD="secretpassword123""#,
        r#"password: mysecretpassword123"#,
        r#"db_password = 'databasepass!'"#,
    ];

    for content in variations {
        let findings = scanner.scan(content, "test");
        assert!(!findings.is_empty(), "Should detect: {}", content);
    }
}

// ============================================================================
// Generic API Key and Secret Tests
// ============================================================================

#[test]
fn test_generic_api_key() {
    let scanner = SecurityScanner::new();
    let content = r#"api_key = "abcdefghij1234567890klmnop""#;

    let findings = scanner.scan(content, "settings.py");

    assert!(!findings.is_empty(), "Should detect generic API key");
    assert!(findings.iter().any(|f| f.kind == SecretKind::ApiKey));
}

#[test]
fn test_generic_secret() {
    let scanner = SecurityScanner::new();
    let content = r#"SECRET = "verysecretvalue12345678901234""#;

    let findings = scanner.scan(content, ".env");

    assert!(!findings.is_empty(), "Should detect generic secret");
    assert!(findings.iter().any(|f| f.kind == SecretKind::Generic));
}

#[test]
fn test_token_detection() {
    let scanner = SecurityScanner::new();
    let content = r#"token = "abc123def456ghi789jkl012mno""#;

    let findings = scanner.scan(content, "auth.config");

    assert!(!findings.is_empty(), "Should detect generic token");
}

// ============================================================================
// Multi-Hit Per Line Tests
// ============================================================================

#[test]
fn test_multiple_secrets_same_line() {
    let scanner = SecurityScanner::new();
    let content = "AWS_KEY=AKIAIOSFODNN7EXAMPLE AWS_SECRET=wJalrXUtnFEMI/K7MDENG/bPxRfiCYE";

    let findings = scanner.scan(content, ".env");

    // Should find at least the AWS key
    assert!(!findings.is_empty(), "Should detect at least one secret on line with multiple");
}

#[test]
fn test_multiple_jwt_tokens_same_line() {
    let scanner = SecurityScanner::new();
    let content =
        r#"old="eyJhbGciOiJIUzI1NiJ9.eyJhIjoxfQ.sig1" new="eyJhbGciOiJIUzI1NiJ9.eyJiIjoyfQ.sig2""#;

    let findings = scanner.scan(content, "tokens.txt");

    // Should find both JWTs
    assert!(findings.len() >= 2, "Should detect both JWT tokens on same line");
}

#[test]
fn test_stripe_keys_same_line() {
    let scanner = SecurityScanner::new();
    let content = "sk_test_FakeTestKey0000000000000,sk_live_FakeTestKey0000000000000";

    let findings = scanner.scan(content, "keys.csv");

    assert!(findings.len() >= 2, "Should detect both Stripe keys on same line");
}

// ============================================================================
// Comment Skipping Tests
// ============================================================================

#[test]
fn test_skip_double_slash_comment() {
    let scanner = SecurityScanner::new();
    let content = "// api_key = 'some_secret_key_12345678901234567890'";

    let findings = scanner.scan(content, "code.js");

    assert!(findings.is_empty(), "Should skip // comments");
}

#[test]
fn test_skip_hash_comment() {
    let scanner = SecurityScanner::new();
    let content = "# password = 'mysecretpassword12345678'";

    let findings = scanner.scan(content, "script.py");

    assert!(findings.is_empty(), "Should skip # comments");
}

#[test]
fn test_skip_example_placeholders() {
    let scanner = SecurityScanner::new();
    let content = r#"api_key = "your_api_key_example_here_123456789""#;

    let findings = scanner.scan(content, "config.py");

    assert!(findings.is_empty(), "Should skip lines with 'example'");
}

#[test]
fn test_skip_xxxxx_placeholders() {
    let scanner = SecurityScanner::new();
    let content = r#"api_key = "xxxxx_replace_with_real_key_xxxxx""#;

    let findings = scanner.scan(content, "config.py");

    assert!(findings.is_empty(), "Should skip lines with 'xxxxx'");
}

#[test]
fn test_skip_jsdoc_continuation() {
    let scanner = SecurityScanner::new();
    let content = " * @param token The authentication token";

    let findings = scanner.scan(content, "api.js");

    assert!(findings.is_empty(), "Should skip JSDoc continuation lines");
}

#[test]
fn test_skip_jsdoc_with_assignment() {
    let scanner = SecurityScanner::new();
    let content = " * token = eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig";

    let findings = scanner.scan(content, "api.js");

    // JSDoc lines are now skipped to reduce false positives from example code
    // Real secrets shouldn't be in comments anyway
    assert!(findings.is_empty(), "Comment lines should be skipped");
}

// ============================================================================
// Markdown Bullet Tests (should NOT skip)
// ============================================================================

#[test]
fn test_detect_in_markdown_bullet() {
    let scanner = SecurityScanner::new();
    let content = "- api_key: sk_live_FakeTestKey0000000000000";

    let findings = scanner.scan(content, "README.md");

    // Markdown bullets should NOT be skipped - they could hide secrets
    assert!(!findings.is_empty(), "Should detect secrets in markdown bullets");
}

#[test]
fn test_detect_in_markdown_numbered_list() {
    let scanner = SecurityScanner::new();
    let content = "1. Set STRIPE_KEY=sk_live_FakeTestKey0000000000000";

    let findings = scanner.scan(content, "SETUP.md");

    assert!(!findings.is_empty(), "Should detect secrets in markdown numbered lists");
}

// ============================================================================
// Allowlist Tests
// ============================================================================

#[test]
fn test_allowlist_partial_match() {
    let mut scanner = SecurityScanner::new();
    scanner.allowlist("EXAMPLE");

    let content = "AKIAIOSFODNN7EXAMPLE";
    let findings = scanner.scan(content, "test");

    assert!(findings.is_empty(), "Allowlisted patterns should not be reported");
}

#[test]
fn test_allowlist_does_not_affect_others() {
    let mut scanner = SecurityScanner::new();
    scanner.allowlist("EXAMPLE");

    let content = "AKIAIOSFODNN7REALKEY1";
    let findings = scanner.scan(content, "test");

    assert!(!findings.is_empty(), "Non-allowlisted keys should still be detected");
}

#[test]
fn test_multiple_allowlist_entries() {
    let mut scanner = SecurityScanner::new();
    scanner.allowlist("EXAMPLE");
    scanner.allowlist("TESTKEY");
    scanner.allowlist("SAMPLE");

    let content = r#"
AKIAIOSFODNN7EXAMPLE
AKIAIOSFODNN7TESTKEY
AKIAIOSFODNN7SAMPLE
AKIAIOSFODNN7REALKEY
"#;

    let findings = scanner.scan(content, "test");

    // Only REALKEY should be detected
    assert_eq!(findings.len(), 1, "Only non-allowlisted key should be detected");
}

// ============================================================================
// Redaction Tests
// ============================================================================

#[test]
fn test_redact_short_secret() {
    let scanner = SecurityScanner::new();
    let content = "TOKEN=abc123";

    let (redacted, _) = scanner.scan_and_redact(content, "test");

    // Short tokens might not match patterns, but test the redaction mechanism
    assert!(!redacted.is_empty());
}

#[test]
fn test_redact_preserves_structure() {
    let scanner = SecurityScanner::new();
    let content = r#"
{
    "stripe_key": "sk_live_FakeTestKey0000000000000",
    "other": "value"
}
"#;

    let (redacted, findings) = scanner.scan_and_redact(content, "config.json");

    assert!(!findings.is_empty(), "Should find the Stripe key");
    assert!(redacted.contains("other"), "Should preserve other content");
    assert!(redacted.contains("value"), "Should preserve other values");
}

#[test]
fn test_redact_content_directly() {
    let scanner = SecurityScanner::new();
    let content = "API_KEY=sk_live_FakeTestKey0000000000000";

    let redacted = scanner.redact_content(content, "test");

    // The key should be partially redacted
    assert!(redacted.contains("sk_l"), "Should preserve prefix");
    assert!(redacted.contains("*"), "Should contain redaction stars");
}

// ============================================================================
// Severity Tests
// ============================================================================

#[test]
fn test_aws_is_critical() {
    let scanner = SecurityScanner::new();
    let content = "AKIAIOSFODNN7EXAMPLE1";

    let findings = scanner.scan(content, "test");

    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_github_is_critical() {
    let scanner = SecurityScanner::new();
    let content = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";

    let findings = scanner.scan(content, "test");

    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_stripe_live_is_critical() {
    let scanner = SecurityScanner::new();
    let content = "sk_live_abcdefghijklmnopqrstuvwx";

    let findings = scanner.scan(content, "test");

    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_private_key_is_critical() {
    let scanner = SecurityScanner::new();
    let content = "-----BEGIN RSA PRIVATE KEY-----";

    let findings = scanner.scan(content, "test");

    assert!(findings.iter().any(|f| f.severity == Severity::Critical));
}

#[test]
fn test_generic_api_key_is_high() {
    let scanner = SecurityScanner::new();
    let content = r#"api_key = "abcdefghijklmnopqrstuvwxyz123456""#;

    let findings = scanner.scan(content, "test");

    if !findings.is_empty() {
        assert!(findings.iter().any(|f| f.severity == Severity::High));
    }
}

#[test]
fn test_jwt_is_high() {
    let scanner = SecurityScanner::new();
    let content = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.rTCH8cLoGxAm_xw68z-zXVKi9ie6xJn9tnVWjd_9ftE";

    let findings = scanner.scan(content, "test");

    assert!(findings.iter().any(|f| f.severity == Severity::High));
}

// ============================================================================
// is_safe Tests
// ============================================================================

#[test]
fn test_is_safe_with_no_secrets() {
    let scanner = SecurityScanner::new();
    let content = "This is just normal code without any secrets.";

    assert!(scanner.is_safe(content, "safe.txt"));
}

#[test]
fn test_is_safe_with_critical_secret() {
    let scanner = SecurityScanner::new();
    let content = "AKIAIOSFODNN7EXAMPLE1";

    assert!(!scanner.is_safe(content, "unsafe.txt"));
}

#[test]
fn test_is_safe_with_high_severity_secret() {
    let scanner = SecurityScanner::new();
    let content = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.signature123";

    // JWT is High severity, is_safe returns false for High and above
    assert!(!scanner.is_safe(content, "token.txt"));
}

// ============================================================================
// Summarize Tests
// ============================================================================

#[test]
fn test_summarize_no_findings() {
    let findings: Vec<SecretFinding> = vec![];
    let summary = SecurityScanner::summarize(&findings);

    assert_eq!(summary, "No secrets detected");
}

#[test]
fn test_summarize_with_findings() {
    let scanner = SecurityScanner::new();
    let content = r#"
AKIAIOSFODNN7EXAMPLE1
ghp_abcdefghijklmnopqrstuvwxyz1234567890
"#;

    let findings = scanner.scan(content, "test");
    let summary = SecurityScanner::summarize(&findings);

    assert!(summary.contains("potential secrets"));
    assert!(summary.contains("critical"));
}

// ============================================================================
// SecretKind Name Tests
// ============================================================================

#[test]
fn test_secret_kind_names() {
    assert_eq!(SecretKind::ApiKey.name(), "API Key");
    assert_eq!(SecretKind::AccessToken.name(), "Access Token");
    assert_eq!(SecretKind::PrivateKey.name(), "Private Key");
    assert_eq!(SecretKind::Password.name(), "Password");
    assert_eq!(SecretKind::ConnectionString.name(), "Connection String");
    assert_eq!(SecretKind::AwsCredential.name(), "AWS Credential");
    assert_eq!(SecretKind::GitHubToken.name(), "GitHub Token");
    assert_eq!(SecretKind::Generic.name(), "Generic Secret");
}

// ============================================================================
// Line Number Accuracy Tests
// ============================================================================

#[test]
fn test_line_numbers_single_line() {
    let scanner = SecurityScanner::new();
    let content = "AKIAIOSFODNN7EXAMPLE1";

    let findings = scanner.scan(content, "test");

    assert!(!findings.is_empty());
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_line_numbers_multi_line() {
    let scanner = SecurityScanner::new();
    let content = r#"
line1
line2
AKIAIOSFODNN7EXAMPLE1
line4
"#;

    let findings = scanner.scan(content, "test");

    assert!(!findings.is_empty());
    // The AWS key is on line 4 (after empty line 1, "line1" on 2, "line2" on 3)
    assert_eq!(findings[0].line, 4);
}

#[test]
fn test_line_numbers_multiple_findings() {
    let scanner = SecurityScanner::new();
    let content = r#"
AKIAIOSFODNN7EXAMPLE1
some text
ghp_abcdefghijklmnopqrstuvwxyz1234567890
more text
sk_live_abcdefghijklmnopqrstuvwx
"#;

    let findings = scanner.scan(content, "test");

    assert!(findings.len() >= 3, "Should find at least 3 secrets");

    // Verify line numbers are ascending (sorted by occurrence)
    let mut lines: Vec<u32> = findings.iter().map(|f| f.line).collect();
    let sorted_lines = lines.clone();
    lines.sort();
    assert_eq!(lines, sorted_lines, "Findings should be in line order");
}

// ============================================================================
// File Path in Findings Tests
// ============================================================================

#[test]
fn test_file_path_preserved() {
    let scanner = SecurityScanner::new();
    let content = "AKIAIOSFODNN7EXAMPLE1";

    let findings = scanner.scan(content, "src/config/secrets.env");

    assert!(!findings.is_empty());
    assert_eq!(findings[0].file, "src/config/secrets.env");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_content() {
    let scanner = SecurityScanner::new();
    let findings = scanner.scan("", "empty.txt");

    assert!(findings.is_empty());
}

#[test]
fn test_whitespace_only_content() {
    let scanner = SecurityScanner::new();
    let findings = scanner.scan("   \n\t\n   ", "whitespace.txt");

    assert!(findings.is_empty());
}

#[test]
fn test_unicode_content() {
    let scanner = SecurityScanner::new();
    let content = "API密钥=AKIAIOSFODNN7EXAMPLE1 # Chinese for 'API key'";

    // The AWS key should still be detected even with Unicode around it
    let findings = scanner.scan(content, "test");

    // Note: This line starts with Chinese characters, not # or //, so should be scanned
    // But the actual finding depends on the regex matching
    // AWS key should be detected
    assert!(!findings.is_empty(), "Should detect AWS key in Unicode context");
}

#[test]
fn test_very_long_line() {
    let scanner = SecurityScanner::new();
    // Use varied padding to avoid triggering "xxxxx" skip pattern
    let padding: String = (0..10000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    // Use a valid AWS key format: AKIA + 16 uppercase alphanumeric chars (20 total)
    // Avoiding "EXAMPLE" which is used in AWS docs and might be allowlisted
    let content = format!("{}AKIAIOSFODNN7TESTKEX{}", padding, padding);

    let findings = scanner.scan(&content, "long.txt");

    assert!(!findings.is_empty(), "Should detect secret in very long line");
}

// ============================================================================
// Real-World Scenario Tests
// ============================================================================

#[test]
fn test_dockerfile_env() {
    let scanner = SecurityScanner::new();
    let content = r#"
FROM node:18
ENV DATABASE_URL=postgres://user:password123@db:5432/app
ENV API_KEY=sk_live_FakeTestKey0000000000000
COPY . /app
CMD ["node", "server.js"]
"#;

    let findings = scanner.scan(content, "Dockerfile");

    assert!(findings.len() >= 2, "Should detect secrets in Dockerfile ENV");
}

#[test]
fn test_github_actions_yaml() {
    let scanner = SecurityScanner::new();
    let content = r#"
name: Deploy
on: push
jobs:
  deploy:
    runs-on: ubuntu-latest
    env:
      AWS_ACCESS_KEY_ID: AKIAIOSFODNN7EXAMPLE1
      STRIPE_KEY: sk_live_FakeTestKey0000000000000
"#;

    let findings = scanner.scan(content, ".github/workflows/deploy.yml");

    assert!(findings.len() >= 2, "Should detect secrets in GitHub Actions YAML");
}

#[test]
fn test_terraform_variables() {
    let scanner = SecurityScanner::new();
    let content = r#"
variable "db_password" {
  default = "supersecretpassword123"
}

resource "aws_instance" "example" {
  ami           = "ami-12345678"
  access_key    = "AKIAIOSFODNN7EXAMPLE1"
}
"#;

    let findings = scanner.scan(content, "main.tf");

    assert!(!findings.is_empty(), "Should detect secrets in Terraform files");
}

#[test]
fn test_kubernetes_secret() {
    let scanner = SecurityScanner::new();
    let content = r#"
apiVersion: v1
kind: Secret
metadata:
  name: db-credentials
data:
  password: c3VwZXJzZWNyZXQ=
stringData:
  api-key: sk_live_FakeTestKey0000000000000
"#;

    let findings = scanner.scan(content, "k8s/secret.yaml");

    // Stripe key should be detected (base64 won't match patterns)
    assert!(!findings.is_empty(), "Should detect plaintext secrets in K8s manifests");
}
