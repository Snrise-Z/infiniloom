//! Security scanning operations
//!
//! This module provides security scanning functionality for detecting secrets
//! and sensitive information in code.

use crate::types::SecurityFinding;
use crate::utils::scan_repository_with_options;
use crate::validation::validate_path_option;
use infiniloom_engine::SecurityScanner;
use napi::Result;
use napi_derive::napi;

/// Scan a repository for security issues
///
/// # Arguments
/// * `path` - Path to repository root
///
/// # Returns
/// Array of security findings
///
/// # Example
/// ```javascript
/// const { scanSecurity } = require('infiniloom-node');
///
/// const findings = scanSecurity('./my-repo');
/// for (const finding of findings) {
///   console.log(`${finding.severity}: ${finding.kind} in ${finding.file}:${finding.line}`);
/// }
/// ```
#[napi]
pub fn scan_security(path: Option<String>) -> Result<Vec<SecurityFinding>> {
    let path = validate_path_option(path.as_deref())?;
    let repo = scan_repository_with_options(&path, true, true)?;

    let scanner = SecurityScanner::new();
    let mut findings = Vec::new();

    for file in &repo.files {
        if let Some(content) = &file.content {
            let file_findings = scanner.scan(content, &file.relative_path);
            for finding in file_findings {
                findings.push(SecurityFinding {
                    file: finding.file.clone(),
                    line: finding.line,
                    severity: format!("{:?}", finding.severity),
                    kind: finding.kind.name().to_string(),
                    pattern: finding.pattern.clone(),
                });
            }
        }
    }

    Ok(findings)
}
