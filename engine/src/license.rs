//! License detection for compliance scanning
//!
//! This module detects open-source licenses in codebases, particularly
//! focusing on copyleft licenses (GPL, AGPL, LGPL) that may require
//! special handling in enterprise environments.
//!
//! # Compliance Use Cases
//!
//! - **Enterprise Code Audit**: Identify copyleft code before embedding
//! - **Legal Review**: Flag files requiring license compliance
//! - **CI/CD Gates**: Fail builds containing prohibited licenses
//!
//! # Supported Licenses
//!
//! | License | Risk Level | Notes |
//! |---------|------------|-------|
//! | GPL-3.0 | High | Strong copyleft, viral |
//! | GPL-2.0 | High | Strong copyleft |
//! | AGPL-3.0 | Critical | Network copyleft |
//! | LGPL-3.0 | Medium | Weak copyleft |
//! | LGPL-2.1 | Medium | Weak copyleft |
//! | MIT | Low | Permissive |
//! | Apache-2.0 | Low | Permissive |
//! | BSD-3-Clause | Low | Permissive |
//! | Unlicensed | Unknown | No license detected |
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::license::{LicenseScanner, LicenseRisk};
//!
//! let scanner = LicenseScanner::new();
//!
//! // Scan a file
//! if let Some(finding) = scanner.scan_file(Path::new("lib/crypto.rs")) {
//!     if finding.license.risk() >= LicenseRisk::High {
//!         println!("Warning: {} contains {}", finding.file, finding.license.name());
//!     }
//! }
//!
//! // Scan entire repository
//! let findings = scanner.scan_repository(repo_path)?;
//! let copyleft_files: Vec<_> = findings
//!     .iter()
//!     .filter(|f| f.license.is_copyleft())
//!     .collect();
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Detected license types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum License {
    // === Strong Copyleft ===
    /// GNU General Public License v3.0
    Gpl3,
    /// GNU General Public License v2.0
    Gpl2,
    /// GNU Affero General Public License v3.0 (network copyleft)
    Agpl3,

    // === Weak Copyleft ===
    /// GNU Lesser General Public License v3.0
    Lgpl3,
    /// GNU Lesser General Public License v2.1
    Lgpl21,
    /// Mozilla Public License 2.0
    Mpl2,
    /// Eclipse Public License 2.0
    Epl2,

    // === Permissive ===
    /// MIT License
    Mit,
    /// Apache License 2.0
    Apache2,
    /// BSD 3-Clause "New" License
    Bsd3Clause,
    /// BSD 2-Clause "Simplified" License
    Bsd2Clause,
    /// ISC License
    Isc,
    /// The Unlicense (public domain)
    Unlicense,
    /// Creative Commons Zero v1.0 Universal
    Cc0,
    /// Do What The Fuck You Want To Public License
    Wtfpl,

    // === Proprietary/Restricted ===
    /// Proprietary/Commercial license
    Proprietary,

    // === Unknown ===
    /// Unknown license
    Unknown,
}

impl License {
    /// Get the SPDX identifier for this license
    pub fn spdx_id(&self) -> &'static str {
        match self {
            Self::Gpl3 => "GPL-3.0-only",
            Self::Gpl2 => "GPL-2.0-only",
            Self::Agpl3 => "AGPL-3.0-only",
            Self::Lgpl3 => "LGPL-3.0-only",
            Self::Lgpl21 => "LGPL-2.1-only",
            Self::Mpl2 => "MPL-2.0",
            Self::Epl2 => "EPL-2.0",
            Self::Mit => "MIT",
            Self::Apache2 => "Apache-2.0",
            Self::Bsd3Clause => "BSD-3-Clause",
            Self::Bsd2Clause => "BSD-2-Clause",
            Self::Isc => "ISC",
            Self::Unlicense => "Unlicense",
            Self::Cc0 => "CC0-1.0",
            Self::Wtfpl => "WTFPL",
            Self::Proprietary => "PROPRIETARY",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gpl3 => "GNU General Public License v3.0",
            Self::Gpl2 => "GNU General Public License v2.0",
            Self::Agpl3 => "GNU Affero General Public License v3.0",
            Self::Lgpl3 => "GNU Lesser General Public License v3.0",
            Self::Lgpl21 => "GNU Lesser General Public License v2.1",
            Self::Mpl2 => "Mozilla Public License 2.0",
            Self::Epl2 => "Eclipse Public License 2.0",
            Self::Mit => "MIT License",
            Self::Apache2 => "Apache License 2.0",
            Self::Bsd3Clause => "BSD 3-Clause License",
            Self::Bsd2Clause => "BSD 2-Clause License",
            Self::Isc => "ISC License",
            Self::Unlicense => "The Unlicense",
            Self::Cc0 => "Creative Commons Zero v1.0",
            Self::Wtfpl => "WTFPL",
            Self::Proprietary => "Proprietary License",
            Self::Unknown => "Unknown License",
        }
    }

    /// Get the risk level for this license
    pub fn risk(&self) -> LicenseRisk {
        match self {
            Self::Agpl3 => LicenseRisk::Critical,
            Self::Gpl3 | Self::Gpl2 => LicenseRisk::High,
            Self::Lgpl3 | Self::Lgpl21 | Self::Mpl2 | Self::Epl2 => LicenseRisk::Medium,
            Self::Mit
            | Self::Apache2
            | Self::Bsd3Clause
            | Self::Bsd2Clause
            | Self::Isc
            | Self::Unlicense
            | Self::Cc0
            | Self::Wtfpl => LicenseRisk::Low,
            Self::Proprietary => LicenseRisk::High,
            Self::Unknown => LicenseRisk::Unknown,
        }
    }

    /// Check if this is a copyleft license
    pub fn is_copyleft(&self) -> bool {
        matches!(
            self,
            Self::Gpl3
                | Self::Gpl2
                | Self::Agpl3
                | Self::Lgpl3
                | Self::Lgpl21
                | Self::Mpl2
                | Self::Epl2
        )
    }

    /// Check if this is a strong (viral) copyleft license
    pub fn is_strong_copyleft(&self) -> bool {
        matches!(self, Self::Gpl3 | Self::Gpl2 | Self::Agpl3)
    }

    /// Check if this is a permissive license
    pub fn is_permissive(&self) -> bool {
        matches!(
            self,
            Self::Mit
                | Self::Apache2
                | Self::Bsd3Clause
                | Self::Bsd2Clause
                | Self::Isc
                | Self::Unlicense
                | Self::Cc0
                | Self::Wtfpl
        )
    }
}

/// License risk levels for compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseRisk {
    /// Unknown risk (no license detected)
    Unknown,
    /// Low risk (permissive licenses)
    Low,
    /// Medium risk (weak copyleft)
    Medium,
    /// High risk (strong copyleft, proprietary)
    High,
    /// Critical risk (AGPL - network copyleft)
    Critical,
}

impl LicenseRisk {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// A license detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFinding {
    /// File where license was found
    pub file: String,

    /// Detected license
    pub license: License,

    /// Line number where license indicator was found
    pub line: u32,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Text snippet that matched
    pub matched_text: String,
}

/// Configuration for license scanning
#[derive(Debug, Clone)]
pub struct LicenseScanConfig {
    /// Minimum confidence threshold (0.0 - 1.0)
    pub min_confidence: f32,

    /// Risk level threshold (only report licenses >= this level)
    pub min_risk: LicenseRisk,

    /// Scan LICENSE/COPYING files
    pub scan_license_files: bool,

    /// Scan source code headers
    pub scan_headers: bool,

    /// Maximum lines to scan per file (for headers)
    pub max_header_lines: usize,
}

impl Default for LicenseScanConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            min_risk: LicenseRisk::Unknown,
            scan_license_files: true,
            scan_headers: true,
            max_header_lines: 50,
        }
    }
}

/// License scanner for detecting licenses in codebases
pub struct LicenseScanner {
    config: LicenseScanConfig,
}

impl Default for LicenseScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseScanner {
    /// Create a new license scanner with default config
    pub fn new() -> Self {
        Self { config: LicenseScanConfig::default() }
    }

    /// Create with custom configuration
    pub fn with_config(config: LicenseScanConfig) -> Self {
        Self { config }
    }

    /// Scan file content for license indicators
    pub fn scan(&self, content: &str, file_path: &str) -> Vec<LicenseFinding> {
        let mut findings = Vec::new();

        // Check if this is a license file
        let is_license_file = self.is_license_file(file_path);

        if is_license_file && self.config.scan_license_files {
            if let Some(finding) = self.scan_license_file(content, file_path) {
                findings.push(finding);
            }
        }

        if self.config.scan_headers {
            findings.extend(self.scan_headers(content, file_path));
        }

        // Filter by confidence and risk
        findings
            .into_iter()
            .filter(|f| {
                f.confidence >= self.config.min_confidence
                    && f.license.risk() >= self.config.min_risk
            })
            .collect()
    }

    /// Check if a file is a license file
    fn is_license_file(&self, file_path: &str) -> bool {
        let path = Path::new(file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_default();

        matches!(
            file_name.as_str(),
            "LICENSE"
                | "LICENSE.MD"
                | "LICENSE.TXT"
                | "LICENCE"
                | "LICENCE.MD"
                | "LICENCE.TXT"
                | "COPYING"
                | "COPYING.MD"
                | "COPYING.TXT"
                | "LICENSE-MIT"
                | "LICENSE-APACHE"
                | "LICENSE.MIT"
                | "LICENSE.APACHE"
        )
    }

    /// Scan a LICENSE/COPYING file
    fn scan_license_file(&self, content: &str, file_path: &str) -> Option<LicenseFinding> {
        let content_lower = content.to_lowercase();

        // Check for specific license texts (in order of specificity)
        let detections: Vec<(License, f32, &str)> = vec![
            // AGPL (must check before GPL due to substring match)
            (License::Agpl3, 0.95, "gnu affero general public license"),
            (License::Agpl3, 0.9, "agpl-3.0"),
            (License::Agpl3, 0.85, "agpl version 3"),
            // LGPL (must check before GPL)
            (License::Lgpl3, 0.95, "gnu lesser general public license version 3"),
            (License::Lgpl3, 0.9, "lgpl-3.0"),
            (License::Lgpl21, 0.95, "gnu lesser general public license version 2.1"),
            (License::Lgpl21, 0.9, "lgpl-2.1"),
            (License::Lgpl21, 0.9, "lgpl version 2.1"),
            // GPL
            (License::Gpl3, 0.95, "gnu general public license version 3"),
            // Canonical GPL3 header: "GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007"
            (License::Gpl3, 0.95, "version 3, 29 june 2007"),
            (License::Gpl3, 0.9, "gpl-3.0"),
            (License::Gpl3, 0.85, "gplv3"),
            (License::Gpl2, 0.95, "gnu general public license version 2"),
            // Canonical GPL2 header: "GNU GENERAL PUBLIC LICENSE\nVersion 2, June 1991"
            (License::Gpl2, 0.95, "version 2, june 1991"),
            (License::Gpl2, 0.9, "gpl-2.0"),
            (License::Gpl2, 0.85, "gplv2"),
            // MPL
            (License::Mpl2, 0.95, "mozilla public license version 2.0"),
            (License::Mpl2, 0.9, "mpl-2.0"),
            // EPL
            (License::Epl2, 0.95, "eclipse public license - v 2.0"),
            (License::Epl2, 0.9, "epl-2.0"),
            // Apache
            (License::Apache2, 0.95, "apache license, version 2.0"),
            (License::Apache2, 0.95, "apache license version 2.0"),
            (License::Apache2, 0.9, "apache-2.0"),
            (License::Apache2, 0.85, "licensed under the apache license"),
            // MIT
            (License::Mit, 0.95, "mit license"),
            (License::Mit, 0.9, "permission is hereby granted, free of charge"),
            (License::Mit, 0.85, "the software is provided \"as is\", without warranty"),
            // BSD
            (License::Bsd3Clause, 0.95, "3-clause bsd license"),
            (License::Bsd3Clause, 0.9, "bsd-3-clause"),
            (License::Bsd3Clause, 0.85, "redistributions of source code must retain"),
            (License::Bsd2Clause, 0.95, "2-clause bsd license"),
            (License::Bsd2Clause, 0.9, "bsd-2-clause"),
            // ISC
            (License::Isc, 0.95, "isc license"),
            (License::Isc, 0.9, "permission to use, copy, modify, and/or distribute"),
            // Unlicense
            (License::Unlicense, 0.95, "this is free and unencumbered software"),
            (License::Unlicense, 0.9, "unlicense"),
            // CC0
            (License::Cc0, 0.95, "cc0 1.0 universal"),
            (License::Cc0, 0.9, "creative commons zero"),
            // WTFPL
            (License::Wtfpl, 0.95, "do what the fuck you want to public license"),
            (License::Wtfpl, 0.9, "wtfpl"),
        ];

        for (license, confidence, pattern) in detections {
            if content_lower.contains(pattern) {
                // Find the line number
                let line = content_lower
                    .lines()
                    .enumerate()
                    .find(|(_, l)| l.contains(pattern))
                    .map_or(1, |(i, _)| (i + 1) as u32);

                return Some(LicenseFinding {
                    file: file_path.to_owned(),
                    license,
                    line,
                    confidence,
                    matched_text: pattern.to_owned(),
                });
            }
        }

        None
    }

    /// Scan source code headers for SPDX identifiers and license comments
    fn scan_headers(&self, content: &str, file_path: &str) -> Vec<LicenseFinding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = content.lines().take(self.config.max_header_lines).collect();

        for (line_num, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();

            // Check for SPDX license identifiers
            if let Some(finding) = self.check_spdx_identifier(&line_lower, file_path, line_num + 1)
            {
                findings.push(finding);
                continue;
            }

            // Check for license comments
            if let Some(finding) = self.check_license_comment(&line_lower, file_path, line_num + 1)
            {
                findings.push(finding);
            }
        }

        findings
    }

    /// Check for SPDX license identifiers
    fn check_spdx_identifier(
        &self,
        line: &str,
        file_path: &str,
        line_num: usize,
    ) -> Option<LicenseFinding> {
        // Pattern: SPDX-License-Identifier: <license>
        if !line.contains("spdx-license-identifier") {
            return None;
        }

        let spdx_mappings: Vec<(&str, License)> = vec![
            ("agpl-3.0", License::Agpl3),
            ("gpl-3.0", License::Gpl3),
            ("gpl-2.0", License::Gpl2),
            ("lgpl-3.0", License::Lgpl3),
            ("lgpl-2.1", License::Lgpl21),
            ("mpl-2.0", License::Mpl2),
            ("epl-2.0", License::Epl2),
            ("apache-2.0", License::Apache2),
            ("mit", License::Mit),
            ("bsd-3-clause", License::Bsd3Clause),
            ("bsd-2-clause", License::Bsd2Clause),
            ("isc", License::Isc),
            ("unlicense", License::Unlicense),
            ("cc0-1.0", License::Cc0),
        ];

        for (spdx_id, license) in spdx_mappings {
            if line.contains(spdx_id) {
                return Some(LicenseFinding {
                    file: file_path.to_owned(),
                    license,
                    line: line_num as u32,
                    confidence: 0.99, // SPDX identifiers are very reliable
                    matched_text: format!("SPDX-License-Identifier: {}", spdx_id),
                });
            }
        }

        None
    }

    /// Check for license mentions in comments
    fn check_license_comment(
        &self,
        line: &str,
        file_path: &str,
        line_num: usize,
    ) -> Option<LicenseFinding> {
        // Must be in a comment
        if !line.contains("//")
            && !line.contains("/*")
            && !line.contains('*')
            && !line.contains('#')
        {
            return None;
        }

        let comment_patterns: Vec<(&str, License, f32)> = vec![
            // High confidence patterns
            ("licensed under agpl", License::Agpl3, 0.85),
            ("licensed under gpl", License::Gpl3, 0.8),
            ("licensed under lgpl", License::Lgpl3, 0.8),
            ("licensed under the mit license", License::Mit, 0.85),
            ("licensed under apache", License::Apache2, 0.85),
            // Medium confidence patterns
            ("this file is part of", License::Unknown, 0.5), // Often followed by license
            ("copyright", License::Unknown, 0.3),
        ];

        for (pattern, license, confidence) in comment_patterns {
            if line.contains(pattern) && license != License::Unknown {
                return Some(LicenseFinding {
                    file: file_path.to_owned(),
                    license,
                    line: line_num as u32,
                    confidence,
                    matched_text: pattern.to_owned(),
                });
            }
        }

        None
    }

    /// Scan a file path for license information
    pub fn scan_file(&self, path: &Path) -> Result<Vec<LicenseFinding>, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let file_path = path.to_string_lossy();
        Ok(self.scan(&content, &file_path))
    }

    /// Scan a repository for license information
    pub fn scan_repository(&self, repo_path: &Path) -> Result<Vec<LicenseFinding>, std::io::Error> {
        use ignore::WalkBuilder;

        let mut all_findings = Vec::new();

        let walker = WalkBuilder::new(repo_path)
            .hidden(false)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Scan license files
            if self.is_license_file(&path.to_string_lossy()) {
                if let Ok(findings) = self.scan_file(path) {
                    all_findings.extend(findings);
                }
                continue;
            }

            // Scan source file headers if configured
            if self.config.scan_headers {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let is_source = matches!(
                    ext,
                    "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "java" | "rb" | "php"
                );

                if is_source {
                    if let Ok(findings) = self.scan_file(path) {
                        all_findings.extend(findings);
                    }
                }
            }
        }

        // Deduplicate by file and license
        all_findings.sort_by(|a, b| {
            a.file
                .cmp(&b.file)
                .then_with(|| a.license.spdx_id().cmp(b.license.spdx_id()))
        });
        all_findings.dedup_by(|a, b| a.file == b.file && a.license == b.license);

        Ok(all_findings)
    }

    /// Get a summary of license findings
    pub fn summarize(findings: &[LicenseFinding]) -> LicenseSummary {
        let mut summary = LicenseSummary::default();

        for finding in findings {
            match finding.license.risk() {
                LicenseRisk::Critical => summary.critical_count += 1,
                LicenseRisk::High => summary.high_count += 1,
                LicenseRisk::Medium => summary.medium_count += 1,
                LicenseRisk::Low => summary.low_count += 1,
                LicenseRisk::Unknown => summary.unknown_count += 1,
            }

            if finding.license.is_copyleft() {
                summary.copyleft_files.push(finding.file.clone());
            }

            // Track unique licenses
            if !summary.licenses.contains(&finding.license) {
                summary.licenses.push(finding.license);
            }
        }

        summary.copyleft_files.sort();
        summary.copyleft_files.dedup();

        summary
    }
}

/// Summary of license findings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LicenseSummary {
    /// Count of critical risk licenses (AGPL)
    pub critical_count: usize,

    /// Count of high risk licenses (GPL, proprietary)
    pub high_count: usize,

    /// Count of medium risk licenses (LGPL, MPL)
    pub medium_count: usize,

    /// Count of low risk licenses (MIT, Apache, BSD)
    pub low_count: usize,

    /// Count of unknown licenses
    pub unknown_count: usize,

    /// Files containing copyleft licenses
    pub copyleft_files: Vec<String>,

    /// Unique licenses found
    pub licenses: Vec<License>,
}

impl LicenseSummary {
    /// Check if any copyleft licenses were found
    pub fn has_copyleft(&self) -> bool {
        !self.copyleft_files.is_empty()
    }

    /// Check if any high-risk licenses were found
    pub fn has_high_risk(&self) -> bool {
        self.critical_count > 0 || self.high_count > 0
    }

    /// Get total number of findings
    pub fn total(&self) -> usize {
        self.critical_count
            + self.high_count
            + self.medium_count
            + self.low_count
            + self.unknown_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_risk_levels() {
        assert_eq!(License::Agpl3.risk(), LicenseRisk::Critical);
        assert_eq!(License::Gpl3.risk(), LicenseRisk::High);
        assert_eq!(License::Lgpl3.risk(), LicenseRisk::Medium);
        assert_eq!(License::Mit.risk(), LicenseRisk::Low);
        assert_eq!(License::Unknown.risk(), LicenseRisk::Unknown);
    }

    #[test]
    fn test_copyleft_detection() {
        assert!(License::Gpl3.is_copyleft());
        assert!(License::Agpl3.is_copyleft());
        assert!(License::Lgpl3.is_copyleft());
        assert!(!License::Mit.is_copyleft());
        assert!(!License::Apache2.is_copyleft());
    }

    #[test]
    fn test_strong_copyleft() {
        assert!(License::Gpl3.is_strong_copyleft());
        assert!(License::Agpl3.is_strong_copyleft());
        assert!(!License::Lgpl3.is_strong_copyleft());
        assert!(!License::Mit.is_strong_copyleft());
    }

    #[test]
    fn test_scan_mit_license() {
        let scanner = LicenseScanner::new();
        let content = r#"
MIT License

Copyright (c) 2024 Example Corp

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software.
"#;

        let findings = scanner.scan(content, "LICENSE");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].license, License::Mit);
        assert!(findings[0].confidence >= 0.9);
    }

    #[test]
    fn test_scan_gpl3_license() {
        let scanner = LicenseScanner::new();
        let content = r#"
GNU GENERAL PUBLIC LICENSE
Version 3, 29 June 2007

Copyright (C) 2007 Free Software Foundation, Inc.
"#;

        let findings = scanner.scan(content, "COPYING");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].license, License::Gpl3);
    }

    #[test]
    fn test_scan_spdx_identifier() {
        let scanner = LicenseScanner::new();
        let content = r#"
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("Hello, world!");
}
"#;

        let findings = scanner.scan(content, "src/main.rs");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].license, License::Apache2);
        assert!(findings[0].confidence >= 0.95);
    }

    #[test]
    fn test_scan_agpl_in_header() {
        let scanner = LicenseScanner::new();
        let content = r#"
# Licensed under AGPL-3.0
# Copyright 2024 Example Corp

def main():
    pass
"#;

        let findings = scanner.scan(content, "main.py");
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.license == License::Agpl3));
    }

    #[test]
    fn test_license_summary() {
        let findings = vec![
            LicenseFinding {
                file: "lib/a.rs".to_owned(),
                license: License::Gpl3,
                line: 1,
                confidence: 0.95,
                matched_text: "gpl-3.0".to_owned(),
            },
            LicenseFinding {
                file: "lib/b.rs".to_owned(),
                license: License::Mit,
                line: 1,
                confidence: 0.9,
                matched_text: "mit".to_owned(),
            },
            LicenseFinding {
                file: "lib/c.rs".to_owned(),
                license: License::Agpl3,
                line: 1,
                confidence: 0.95,
                matched_text: "agpl-3.0".to_owned(),
            },
        ];

        let summary = LicenseScanner::summarize(&findings);

        assert_eq!(summary.critical_count, 1);
        assert_eq!(summary.high_count, 1);
        assert_eq!(summary.low_count, 1);
        assert!(summary.has_copyleft());
        assert!(summary.has_high_risk());
        assert_eq!(summary.copyleft_files.len(), 2);
    }

    #[test]
    fn test_is_license_file() {
        let scanner = LicenseScanner::new();

        assert!(scanner.is_license_file("LICENSE"));
        assert!(scanner.is_license_file("LICENSE.md"));
        assert!(scanner.is_license_file("COPYING"));
        assert!(scanner.is_license_file("LICENSE-MIT"));
        assert!(!scanner.is_license_file("src/main.rs"));
        assert!(!scanner.is_license_file("README.md"));
    }

    #[test]
    fn test_risk_ordering() {
        assert!(LicenseRisk::Critical > LicenseRisk::High);
        assert!(LicenseRisk::High > LicenseRisk::Medium);
        assert!(LicenseRisk::Medium > LicenseRisk::Low);
        assert!(LicenseRisk::Low > LicenseRisk::Unknown);
    }

    #[test]
    fn test_spdx_ids() {
        assert_eq!(License::Gpl3.spdx_id(), "GPL-3.0-only");
        assert_eq!(License::Mit.spdx_id(), "MIT");
        assert_eq!(License::Apache2.spdx_id(), "Apache-2.0");
    }
}
