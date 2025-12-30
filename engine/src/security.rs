//! Security scanning for secrets and sensitive data
//!
//! This module provides automatic detection and redaction of secrets, API keys,
//! tokens, and other sensitive data before sharing code with LLMs or external services.
//!
//! # Quick Start
//!
//! ```rust
//! use infiniloom_engine::security::SecurityScanner;
//!
//! let scanner = SecurityScanner::new();
//! let code = r#"
//!     const AWS_KEY = "AKIAIOSFODNN7EXAMPLE";
//!     const API_TOKEN = "sk-proj-abc123xyz789";
//! "#;
//!
//! // Scan for secrets
//! let findings = scanner.scan(code, "config.rs");
//!
//! if !findings.is_empty() {
//!     println!("⚠️  Found {} secrets!", findings.len());
//!     for finding in &findings {
//!         println!("  {} on line {}: {}",
//!             finding.kind.name(),
//!             finding.line,
//!             finding.pattern);  // Already redacted: "AKIA************MPLE"
//!     }
//! }
//! ```
//!
//! # Scanning with Detailed Results
//!
//! The scanner returns structured findings with metadata:
//!
//! ```rust
//! use infiniloom_engine::security::{SecurityScanner, Severity};
//!
//! let scanner = SecurityScanner::new();
//! let findings = scanner.scan(r#"
//!     DB_URL = "postgresql://user:pass@localhost/db"
//!     STRIPE_KEY = "sk_live_abc123xyz789"
//! "#, ".env");
//!
//! for finding in findings {
//!     match finding.severity {
//!         Severity::Critical => println!("🔴 CRITICAL: {}", finding.pattern),
//!         Severity::High => println!("🟠 HIGH: {}", finding.pattern),
//!         Severity::Medium => println!("🟡 MEDIUM: {}", finding.pattern),
//!         Severity::Low => println!("🟢 LOW: {}", finding.pattern),
//!     }
//! }
//! ```
//!
//! # Automatic Redaction
//!
//! Replace detected secrets with `[REDACTED]` markers:
//!
//! ```rust
//! use infiniloom_engine::security::SecurityScanner;
//!
//! let scanner = SecurityScanner::new();
//! let code = r#"
//!     const apiKey = "sk-proj-secret123";
//!     const githubToken = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
//! "#;
//!
//! // Scan and redact in one operation
//! let (redacted, findings) = scanner.scan_and_redact(code, "api.ts");
//!
//! println!("Original had {} secrets", findings.len());
//! println!("Redacted version:\n{}", redacted);
//! // Output: const apiKey = "sk-p****ect123";
//! //         const githubToken = "ghp_****7890";
//! ```
//!
//! # Custom Patterns
//!
//! Add organization-specific secret patterns:
//!
//! ```rust,no_run
//! use infiniloom_engine::security::SecurityScanner;
//!
//! let mut scanner = SecurityScanner::new();
//!
//! // Add custom patterns for internal systems
//! scanner.add_custom_pattern(r"MYCOMPANY_API_[A-Z0-9]{32}");
//! scanner.add_custom_pattern(r"INTERNAL_TOKEN_[a-f0-9]{64}");
//!
//! // Or add multiple at once
//! scanner.add_custom_patterns(&[
//!     "ORG_SECRET_[A-Z0-9]{16}".to_string(),
//!     "DEPLOY_KEY_[a-z0-9]{40}".to_string(),
//! ]);
//!
//! // Now scan with both built-in and custom patterns
//! let findings = scanner.scan(r#"
//!     MYCOMPANY_API_ABCD1234EFGH5678IJKL9012MNOP
//! "#, "internal.rs");
//!
//! assert!(!findings.is_empty());
//! ```
//!
//! # Allowlist for Test Data
//!
//! Mark known test/example secrets as safe:
//!
//! ```rust
//! use infiniloom_engine::security::SecurityScanner;
//!
//! let mut scanner = SecurityScanner::new();
//!
//! // Allowlist test keys that are intentionally public
//! scanner.allowlist("EXAMPLE");
//! scanner.allowlist("test_key");
//! scanner.allowlist("mock_secret");
//!
//! // This won't trigger detection (contains "EXAMPLE")
//! let test_code = r#"
//!     AWS_KEY = "AKIAIOSFODNN7EXAMPLE"  // Official AWS test key
//! "#;
//!
//! let findings = scanner.scan(test_code, "test.rs");
//! assert!(findings.is_empty(), "Test keys should be allowed");
//!
//! // But this WILL trigger (real key format)
//! let prod_code = r#"
//!     AWS_KEY = "AKIAIOSFODNN7PRODKEY"
//! "#;
//!
//! let findings = scanner.scan(prod_code, "prod.rs");
//! assert!(!findings.is_empty(), "Real keys should be detected");
//! ```
//!
//! # Repository Integration
//!
//! Scan all files in a repository:
//!
//! ```rust,ignore
//! use infiniloom_engine::security::SecurityScanner;
//!
//! let scanner = SecurityScanner::new();
//! let mut all_findings = Vec::new();
//!
//! for file in repository.files {
//!     let findings = scanner.scan(&file.content, &file.relative_path);
//!     all_findings.extend(findings);
//! }
//!
//! if !all_findings.is_empty() {
//!     eprintln!("⚠️  Security scan found {} secrets across {} files",
//!         all_findings.len(),
//!         all_findings.iter()
//!             .map(|f| &f.file)
//!             .collect::<std::collections::HashSet<_>>()
//!             .len()
//!     );
//!
//!     // Exit with error in CI/CD
//!     std::process::exit(1);
//! }
//! ```
//!
//! # Severity-Based Filtering
//!
//! Work with different severity levels:
//!
//! ```rust
//! use infiniloom_engine::security::{SecurityScanner, Severity};
//!
//! let scanner = SecurityScanner::new();
//! let findings = scanner.scan(r#"
//!     AWS_KEY = "AKIAIOSFODNN7PRODKEY"      # Critical
//!     password = "weak123"                  # High
//! "#, ".env");
//!
//! // Count by severity
//! let critical_count = findings.iter()
//!     .filter(|f| f.severity == Severity::Critical)
//!     .count();
//!
//! let high_count = findings.iter()
//!     .filter(|f| f.severity == Severity::High)
//!     .count();
//!
//! println!("Critical: {}, High: {}", critical_count, high_count);
//!
//! // Check if safe to proceed (only low/medium severity)
//! let is_safe = findings.iter()
//!     .all(|f| f.severity < Severity::High);
//!
//! if !is_safe {
//!     eprintln!("⛔ Cannot proceed - high/critical secrets detected");
//! }
//! ```
//!
//! # Supported Secret Types
//!
//! ## Cloud Credentials (Critical Severity)
//! - **AWS**: Access keys (AKIA...), Secret access keys
//! - **GitHub**: Personal access tokens (ghp_..., github_pat_...), OAuth tokens
//! - **Private Keys**: RSA, EC, DSA, OpenSSH private keys
//!
//! ## API Keys (Critical Severity)
//! - **OpenAI**: sk-... API keys
//! - **Anthropic**: sk-ant-... API keys
//! - **Stripe**: sk_live_..., pk_test_... keys
//!
//! ## Service Tokens (High Severity)
//! - **Slack**: xoxb-..., xoxa-... tokens
//! - **JWT**: Encoded JSON Web Tokens
//! - **Database**: Connection strings (PostgreSQL, MongoDB, MySQL, Redis, etc.)
//!
//! ## Generic Secrets (High Severity)
//! - Generic API keys (api_key=...)
//! - Access tokens (token=..., secret=...)
//! - Passwords (password=...)
//!
//! # Why Pre-compiled Patterns?
//!
//! The module uses `once_cell::sync::Lazy` for regex patterns:
//!
//! ```rust,ignore
//! static RE_AWS_KEY: Lazy<Regex> =
//!     Lazy::new(|| Regex::new(r"AKIA[0-9A-Z]{16}").unwrap());
//! ```
//!
//! **Benefits**:
//! - Compiled once at first use
//! - Reused across all scanner instances
//! - Thread-safe sharing
//! - Zero runtime compilation overhead
//!
//! **Pattern Order**: More specific patterns (Stripe, Slack, JWT) come BEFORE
//! generic patterns (api_key, secret) to ensure accurate detection and avoid
//! masking by broader patterns.
//!
//! # False Positive Reduction
//!
//! The scanner automatically skips:
//! - **Comments**: Lines starting with //, #, /*, *
//! - **Documentation**: Lines containing "example" as a word
//! - **Placeholders**: Lines with "xxxxx" or "placeholder"
//! - **Allowlisted patterns**: User-configured safe patterns
//!
//! This reduces false positives in documentation, test files, and examples
//! while catching real secrets in code.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

// Helper regex for word-boundary "example" detection (to skip documentation lines)
static RE_EXAMPLE_WORD: Lazy<Regex> = Lazy::new(|| {
    // Match "example" as a standalone word to skip documentation/tutorial content.
    // This helps reduce false positives in example code and documentation.
    //
    // Note: This does NOT prevent detection of AWS keys containing "EXAMPLE" like
    // AKIAIOSFODNN7EXAMPLE - those are detected by the AWS key pattern (RE_AWS_KEY)
    // which runs separately. This regex is only used to skip entire lines that
    // appear to be documentation examples (e.g., "# Example:" or "// example usage").
    //
    // The regex allows dots in word boundaries to handle domain examples like
    // db.example.com without matching.
    Regex::new(r"(?i)(?:^|[^a-zA-Z0-9.])example(?:[^a-zA-Z0-9.]|$)")
        .expect("RE_EXAMPLE_WORD: invalid regex pattern")
});

// Pre-compiled regex patterns (compiled once, reused across all scanner instances)
static RE_AWS_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"AKIA[0-9A-Z]{16}").expect("RE_AWS_KEY: invalid regex pattern"));
static RE_AWS_SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key['"]?\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})"#)
        .expect("RE_AWS_SECRET: invalid regex pattern")
});
// GitHub Personal Access Token (classic) - 36 alphanumeric chars after prefix
static RE_GITHUB_PAT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"ghp_[A-Za-z0-9]{36}").expect("RE_GITHUB_PAT: invalid regex pattern"));
// GitHub fine-grained PAT
static RE_GITHUB_FINE_PAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}")
        .expect("RE_GITHUB_FINE_PAT: invalid regex pattern")
});
// GitHub OAuth, user-to-server, server-to-server, and refresh tokens
static RE_GITHUB_OTHER_TOKENS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"gh[ours]_[A-Za-z0-9]{36,}").expect("RE_GITHUB_OTHER_TOKENS: invalid regex pattern")
});
static RE_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")
        .expect("RE_PRIVATE_KEY: invalid regex pattern")
});
static RE_API_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:api[_-]?key|apikey)['"]?\s*[:=]\s*['"]?([A-Za-z0-9_-]{20,})"#)
        .expect("RE_API_KEY: invalid regex pattern")
});
static RE_SECRET_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:secret|token)['"]?\s*[:=]\s*['"]?([A-Za-z0-9_-]{20,})"#)
        .expect("RE_SECRET_TOKEN: invalid regex pattern")
});
static RE_PASSWORD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)password['"]?\s*[:=]\s*['"]?([^'"\s]{8,})"#)
        .expect("RE_PASSWORD: invalid regex pattern")
});
static RE_CONN_STRING: Lazy<Regex> = Lazy::new(|| {
    // Note: postgres and postgresql are both valid (postgresql:// is more common in practice)
    Regex::new(
        r#"(?i)(?:mongodb|postgres(?:ql)?|mysql|redis|mariadb|cockroachdb|mssql)://[^\s'"]+"#,
    )
    .expect("RE_CONN_STRING: invalid regex pattern")
});
static RE_JWT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*")
        .expect("RE_JWT: invalid regex pattern")
});
static RE_SLACK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}-[a-zA-Z0-9]{24}")
        .expect("RE_SLACK: invalid regex pattern")
});
static RE_STRIPE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:sk|pk)_(?:test|live)_[A-Za-z0-9]{24,}")
        .expect("RE_STRIPE: invalid regex pattern")
});
// OpenAI API keys (sk-... followed by alphanumeric characters)
static RE_OPENAI: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"sk-[A-Za-z0-9]{32,}").expect("RE_OPENAI: invalid regex pattern"));
// Anthropic API keys (sk-ant-...)
static RE_ANTHROPIC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"sk-ant-[A-Za-z0-9-]{40,}").expect("RE_ANTHROPIC: invalid regex pattern")
});

/// A detected secret or sensitive data
#[derive(Debug, Clone)]
pub struct SecretFinding {
    /// Type of secret
    pub kind: SecretKind,
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Matched pattern (redacted)
    pub pattern: String,
    /// Severity level
    pub severity: Severity,
    /// Whether the secret was found in a comment (may be example/documentation)
    pub in_comment: bool,
}

/// Kind of secret detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// API key
    ApiKey,
    /// Access token
    AccessToken,
    /// Private key
    PrivateKey,
    /// Password
    Password,
    /// Database connection string
    ConnectionString,
    /// AWS credentials
    AwsCredential,
    /// GitHub token
    GitHubToken,
    /// Generic secret
    Generic,
}

impl SecretKind {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::ApiKey => "API Key",
            Self::AccessToken => "Access Token",
            Self::PrivateKey => "Private Key",
            Self::Password => "Password",
            Self::ConnectionString => "Connection String",
            Self::AwsCredential => "AWS Credential",
            Self::GitHubToken => "GitHub Token",
            Self::Generic => "Generic Secret",
        }
    }
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Security scanner
pub struct SecurityScanner {
    patterns: Vec<SecretPattern>,
    custom_patterns: Vec<CustomSecretPattern>,
    allowlist: HashSet<String>,
}

struct SecretPattern {
    kind: SecretKind,
    regex: &'static Lazy<Regex>,
    severity: Severity,
}

/// Custom user-defined secret pattern
struct CustomSecretPattern {
    regex: Regex,
    severity: Severity,
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityScanner {
    /// Create a new security scanner with default patterns
    /// Uses pre-compiled static regex patterns for optimal performance
    ///
    /// Pattern order matters: more specific patterns (Stripe, Slack, JWT) must come
    /// BEFORE generic patterns (API_KEY, SECRET_TOKEN) to ensure proper detection
    /// and redaction.
    pub fn new() -> Self {
        let patterns = vec![
            // === Critical: Specific cloud credentials (most specific patterns first) ===
            // AWS
            SecretPattern {
                kind: SecretKind::AwsCredential,
                regex: &RE_AWS_KEY,
                severity: Severity::Critical,
            },
            SecretPattern {
                kind: SecretKind::AwsCredential,
                regex: &RE_AWS_SECRET,
                severity: Severity::Critical,
            },
            // GitHub tokens (all types: ghp_, gho_, ghu_, ghs_, ghr_, github_pat_)
            SecretPattern {
                kind: SecretKind::GitHubToken,
                regex: &RE_GITHUB_PAT,
                severity: Severity::Critical,
            },
            SecretPattern {
                kind: SecretKind::GitHubToken,
                regex: &RE_GITHUB_FINE_PAT,
                severity: Severity::Critical,
            },
            SecretPattern {
                kind: SecretKind::GitHubToken,
                regex: &RE_GITHUB_OTHER_TOKENS,
                severity: Severity::Critical,
            },
            // Private keys
            SecretPattern {
                kind: SecretKind::PrivateKey,
                regex: &RE_PRIVATE_KEY,
                severity: Severity::Critical,
            },
            // Anthropic API keys (must come before OpenAI since sk-ant- is more specific)
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: &RE_ANTHROPIC,
                severity: Severity::Critical,
            },
            // OpenAI API keys (must come before Stripe since sk- is more general)
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: &RE_OPENAI,
                severity: Severity::Critical,
            },
            // Stripe keys (specific pattern: sk_live_, pk_test_, etc.)
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: &RE_STRIPE,
                severity: Severity::Critical,
            },
            // === High: Specific service tokens (must come before generic patterns) ===
            // Slack tokens (specific pattern: xoxb-, xoxa-, etc.)
            SecretPattern {
                kind: SecretKind::AccessToken,
                regex: &RE_SLACK,
                severity: Severity::High,
            },
            // JWT tokens (specific pattern: eyJ...eyJ...signature)
            SecretPattern {
                kind: SecretKind::AccessToken,
                regex: &RE_JWT,
                severity: Severity::High,
            },
            // Connection strings (specific pattern: mongodb://, postgres://, etc.)
            SecretPattern {
                kind: SecretKind::ConnectionString,
                regex: &RE_CONN_STRING,
                severity: Severity::High,
            },
            // === High: Generic patterns (must come LAST to avoid masking specific patterns) ===
            // Generic API keys (matches api_key=xxx, apikey:xxx, etc.)
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: &RE_API_KEY,
                severity: Severity::High,
            },
            // Generic secrets (matches secret=xxx, token=xxx, etc.)
            SecretPattern {
                kind: SecretKind::Generic,
                regex: &RE_SECRET_TOKEN,
                severity: Severity::High,
            },
            // Passwords
            SecretPattern {
                kind: SecretKind::Password,
                regex: &RE_PASSWORD,
                severity: Severity::High,
            },
        ];

        Self { patterns, custom_patterns: Vec::new(), allowlist: HashSet::new() }
    }

    /// Add a pattern to allowlist
    pub fn allowlist(&mut self, pattern: &str) {
        self.allowlist.insert(pattern.to_owned());
    }

    /// Add a custom regex pattern for secret detection
    ///
    /// Custom patterns are matched as generic secrets with High severity.
    /// Invalid regex patterns are silently ignored.
    ///
    /// # Example
    /// ```
    /// use infiniloom_engine::security::SecurityScanner;
    ///
    /// let mut scanner = SecurityScanner::new();
    /// scanner.add_custom_pattern(r"MY_SECRET_[A-Z0-9]{32}");
    /// ```
    pub fn add_custom_pattern(&mut self, pattern: &str) {
        if let Ok(regex) = Regex::new(pattern) {
            self.custom_patterns
                .push(CustomSecretPattern { regex, severity: Severity::High });
        }
    }

    /// Add multiple custom patterns at once
    pub fn add_custom_patterns(&mut self, patterns: &[String]) {
        for pattern in patterns {
            self.add_custom_pattern(pattern);
        }
    }

    /// Scan content for secrets
    pub fn scan(&self, content: &str, file_path: &str) -> Vec<SecretFinding> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Detect if line is likely a comment - skip entirely to reduce false positives
            // Real secrets shouldn't be in comments anyway
            let is_jsdoc_continuation =
                trimmed.starts_with("* ") && !trimmed.contains('=') && !trimmed.contains(':');
            let is_comment = trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*")
                || is_jsdoc_continuation;

            // Skip obvious false positives (example docs, placeholders, comments)
            let is_obvious_false_positive = is_comment
                || RE_EXAMPLE_WORD.is_match(trimmed)
                || trimmed.to_lowercase().contains("placeholder")
                || trimmed.contains("xxxxx");

            if is_obvious_false_positive {
                continue;
            }

            for pattern in &self.patterns {
                // Use find_iter to catch ALL matches on a line, not just the first
                for m in pattern.regex.find_iter(line) {
                    let matched = m.as_str();

                    // Check allowlist
                    if self.allowlist.iter().any(|a| matched.contains(a)) {
                        continue;
                    }

                    findings.push(SecretFinding {
                        kind: pattern.kind,
                        file: file_path.to_owned(),
                        line: (line_num + 1) as u32,
                        pattern: redact(matched),
                        severity: pattern.severity,
                        in_comment: false, // Non-comment lines only now
                    });
                }
            }

            // Check custom patterns
            for custom in &self.custom_patterns {
                for m in custom.regex.find_iter(line) {
                    let matched = m.as_str();

                    // Check allowlist
                    if self.allowlist.iter().any(|a| matched.contains(a)) {
                        continue;
                    }

                    findings.push(SecretFinding {
                        kind: SecretKind::Generic,
                        file: file_path.to_owned(),
                        line: (line_num + 1) as u32,
                        pattern: redact(matched),
                        severity: custom.severity,
                        in_comment: false,
                    });
                }
            }
        }

        findings
    }

    /// Scan a file and return whether it's safe to include
    pub fn is_safe(&self, content: &str, file_path: &str) -> bool {
        let findings = self.scan(content, file_path);
        findings.iter().all(|f| f.severity < Severity::High)
    }

    /// Get summary of findings
    pub fn summarize(findings: &[SecretFinding]) -> String {
        if findings.is_empty() {
            return "No secrets detected".to_owned();
        }

        let critical = findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let high = findings
            .iter()
            .filter(|f| f.severity == Severity::High)
            .count();

        format!(
            "Found {} potential secrets ({} critical, {} high severity)",
            findings.len(),
            critical,
            high
        )
    }

    /// Redact secrets from content, returning the redacted content
    /// This replaces detected secrets with redacted versions in the actual content
    pub fn redact_content(&self, content: &str, _file_path: &str) -> String {
        let mut result = content.to_owned();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Skip obvious false positives (example docs, placeholders)
            let is_obvious_false_positive = RE_EXAMPLE_WORD.is_match(trimmed)
                || trimmed.to_lowercase().contains("placeholder")
                || trimmed.contains("xxxxx");

            if is_obvious_false_positive {
                continue;
            }

            for pattern in &self.patterns {
                // Use find_iter to catch ALL matches on a line, not just the first
                for m in pattern.regex.find_iter(line) {
                    let matched = m.as_str();

                    // Check allowlist
                    if self.allowlist.iter().any(|a| matched.contains(a)) {
                        continue;
                    }

                    // Only redact high severity and above
                    if pattern.severity >= Severity::High {
                        let redacted = redact(matched);
                        // Replace in result - use line number to find the right occurrence
                        let line_start = result
                            .lines()
                            .take(line_num)
                            .map(|l| l.len() + 1)
                            .sum::<usize>();
                        if let Some(pos) = result[line_start..].find(matched) {
                            let abs_pos = line_start + pos;
                            result.replace_range(abs_pos..abs_pos + matched.len(), &redacted);
                        }
                    }
                }
            }

            // Check custom patterns for redaction
            for custom in &self.custom_patterns {
                for m in custom.regex.find_iter(line) {
                    let matched = m.as_str();

                    // Check allowlist
                    if self.allowlist.iter().any(|a| matched.contains(a)) {
                        continue;
                    }

                    // Only redact high severity and above
                    if custom.severity >= Severity::High {
                        let redacted = redact(matched);
                        let line_start = result
                            .lines()
                            .take(line_num)
                            .map(|l| l.len() + 1)
                            .sum::<usize>();
                        if let Some(pos) = result[line_start..].find(matched) {
                            let abs_pos = line_start + pos;
                            result.replace_range(abs_pos..abs_pos + matched.len(), &redacted);
                        }
                    }
                }
            }
        }

        result
    }

    /// Scan and redact all secrets from content.
    ///
    /// Returns a tuple of (redacted_content, findings) where:
    /// - `redacted_content` has all detected secrets replaced with `[REDACTED]`
    /// - `findings` is a list of all detected secrets with metadata
    ///
    /// # Important
    ///
    /// Always check the findings list to understand what was redacted and whether
    /// the file should be excluded from context entirely.
    #[must_use = "security findings should be reviewed"]
    pub fn scan_and_redact(&self, content: &str, file_path: &str) -> (String, Vec<SecretFinding>) {
        let findings = self.scan(content, file_path);
        let redacted = self.redact_content(content, file_path);
        (redacted, findings)
    }
}

/// Redact a matched secret for display
///
/// This function is UTF-8 safe - it uses character counts rather than byte
/// positions to avoid panics when secrets contain multi-byte characters.
fn redact(s: &str) -> String {
    let char_count = s.chars().count();

    if char_count <= 8 {
        return "*".repeat(char_count);
    }

    // Use character-based positions for UTF-8 safety
    let prefix_chars = 4.min(char_count / 4);
    let suffix_chars = 4.min(char_count / 4);
    let redact_chars = char_count.saturating_sub(prefix_chars + suffix_chars);

    // Collect prefix characters
    let prefix: String = s.chars().take(prefix_chars).collect();

    // Collect suffix characters
    let suffix: String = s.chars().skip(char_count - suffix_chars).collect();

    format!("{}{}{}", prefix, "*".repeat(redact_chars), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_key_detection() {
        let scanner = SecurityScanner::new();
        let content = r#"AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE""#;

        let findings = scanner.scan(content, "config.py");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::AwsCredential));
    }

    #[test]
    fn test_github_token_detection() {
        let scanner = SecurityScanner::new();
        let content = r#"GITHUB_TOKEN = "ghp_abcdefghijklmnopqrstuvwxyz1234567890""#;

        let findings = scanner.scan(content, ".env");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::GitHubToken));
    }

    #[test]
    fn test_private_key_detection() {
        let scanner = SecurityScanner::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpA...";

        let findings = scanner.scan(content, "key.pem");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::PrivateKey));
    }

    #[test]
    fn test_allowlist() {
        let mut scanner = SecurityScanner::new();
        scanner.allowlist("EXAMPLE");

        let content = r#"api_key = "AKIAIOSFODNN7EXAMPLE""#;
        let findings = scanner.scan(content, "test.py");

        assert!(findings.is_empty());
    }

    #[test]
    fn test_redact() {
        assert_eq!(redact("AKIAIOSFODNN7EXAMPLE"), "AKIA************MPLE");
        assert_eq!(redact("short"), "*****");
    }

    #[test]
    fn test_redact_unicode_safety() {
        // Test with Chinese characters (3 bytes each)
        // Should not panic when slicing
        let chinese_secret = "密钥ABCDEFGHIJKLMNOP密钥";
        let result = redact(chinese_secret);
        // Should produce valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        // Should contain asterisks
        assert!(result.contains('*'));

        // Test with emoji (4 bytes each)
        let emoji_secret = "🔑ABCDEFGHIJKLMNOP🔒";
        let result = redact(emoji_secret);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(result.contains('*'));

        // Test with mixed multi-byte characters
        let mixed_secret = "абвгдежзийклмноп"; // Cyrillic (2 bytes each)
        let result = redact(mixed_secret);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(result.contains('*'));

        // Test short Unicode strings (should all be asterisks)
        let short_chinese = "密钥";
        let result = redact(short_chinese);
        assert_eq!(result, "**"); // 2 characters
    }

    #[test]
    fn test_redact_edge_cases() {
        // Empty string
        assert_eq!(redact(""), "");

        // Single character
        assert_eq!(redact("x"), "*");

        // Exactly 8 characters (boundary)
        assert_eq!(redact("12345678"), "********");

        // 9 characters (first to show prefix/suffix)
        let result = redact("123456789");
        assert!(result.contains('*'));
        assert!(result.starts_with('1') || result.starts_with('*'));
    }

    #[test]
    fn test_comments_are_skipped() {
        let scanner = SecurityScanner::new();
        let content = "# api_key = 'some_secret_key_12345678901234567890'";

        let findings = scanner.scan(content, "test.py");

        // Comments are skipped entirely to reduce false positives
        assert!(findings.is_empty(), "Secrets in comments should be skipped");
    }

    #[test]
    fn test_non_comment_detected() {
        let scanner = SecurityScanner::new();
        let content = "api_key = 'some_secret_key_12345678901234567890'";

        let findings = scanner.scan(content, "test.py");

        assert!(!findings.is_empty(), "Secrets in non-comments should be detected");
        assert!(
            findings.iter().all(|f| !f.in_comment),
            "in_comment should be false for non-comment lines"
        );
    }

    #[test]
    fn test_custom_pattern() {
        let mut scanner = SecurityScanner::new();
        scanner.add_custom_pattern(r"CUSTOM_SECRET_[A-Z0-9]{16}");

        let content = "my_secret = CUSTOM_SECRET_ABCD1234EFGH5678";
        let findings = scanner.scan(content, "test.py");

        assert!(!findings.is_empty(), "Custom pattern should be detected");
        assert!(findings.iter().any(|f| f.kind == SecretKind::Generic));
    }

    #[test]
    fn test_custom_patterns_multiple() {
        let mut scanner = SecurityScanner::new();
        scanner.add_custom_patterns(&[
            r"MYAPP_KEY_[a-f0-9]{32}".to_owned(),
            r"MYAPP_TOKEN_[A-Z]{20}".to_owned(),
        ]);

        let content = "key = MYAPP_KEY_0123456789abcdef0123456789abcdef";
        let findings = scanner.scan(content, "test.py");

        assert!(!findings.is_empty(), "Custom patterns should be detected");
    }

    #[test]
    fn test_invalid_custom_pattern_ignored() {
        let mut scanner = SecurityScanner::new();
        // Invalid regex - unclosed bracket
        scanner.add_custom_pattern(r"INVALID_[PATTERN");

        // Should not panic, invalid patterns are ignored
        let content = "INVALID_[PATTERN here";
        let _findings = scanner.scan(content, "test.py");
    }
}
