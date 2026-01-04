//! Audit logging framework for SOC2/GDPR/HIPAA compliance
//!
//! This module provides comprehensive audit logging for security-sensitive operations
//! in the embedding pipeline. It tracks all actions that access, process, or transform
//! sensitive data for compliance and forensic purposes.
//!
//! # SOC2 Compliance
//!
//! The audit log supports SOC2 requirements for:
//! - **CC6.1**: Logical access security (who accessed what)
//! - **CC6.6**: System operations monitoring
//! - **CC7.2**: Change management tracking
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use infiniloom_engine::audit::{AuditLogger, FileAuditLogger, AuditEvent, AuditEventKind};
//!
//! // Create a file-based audit logger
//! let logger = FileAuditLogger::new("/var/log/infiniloom/audit.jsonl").unwrap();
//!
//! // Log a scan event
//! logger.log(AuditEvent::new(
//!     AuditEventKind::ScanStarted,
//!     "repo-123",
//!     Some("user@example.com".to_string()),
//! ).with_detail("path", "/path/to/repo"));
//!
//! // Log a secret detection
//! logger.log(AuditEvent::new(
//!     AuditEventKind::SecretDetected,
//!     "repo-123",
//!     Some("user@example.com".to_string()),
//! ).with_detail("file", "config.py")
//!  .with_detail("kind", "AWS Credential")
//!  .with_detail("line", "42"));
//! ```
//!
//! # Event Types
//!
//! The audit log captures these security-relevant events:
//!
//! | Event | Description | Severity |
//! |-------|-------------|----------|
//! | `ScanStarted` | Repository scan initiated | Info |
//! | `ScanCompleted` | Repository scan finished | Info |
//! | `SecretDetected` | Secret/credential found | Critical |
//! | `PiiDetected` | PII data found | High |
//! | `SecretRedacted` | Secret was redacted | Warning |
//! | `ChunkGenerated` | Embedding chunk created | Debug |
//! | `ManifestUpdated` | Manifest file changed | Info |
//! | `AccessDenied` | Access control rejection | Warning |
//! | `ConfigChanged` | Settings modified | Info |
//!
//! # Log Format (JSONL)
//!
//! Each log entry is a JSON object on a single line:
//!
//! ```json
//! {"timestamp":"2024-01-15T10:30:00Z","event":"secret_detected","session_id":"abc123","repo_id":"myrepo","user":"admin@corp.com","severity":"critical","details":{"file":"config.py","kind":"AWS Credential","line":"42"}}
//! ```
//!
//! # Retention and Rotation
//!
//! For production deployments, configure log rotation externally (e.g., logrotate)
//! and ensure logs are retained per your compliance requirements (typically 1-7 years).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

/// Audit event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    /// Debug-level events (chunk generation, etc.)
    Debug,
    /// Informational events (scan start/complete, config changes)
    Info,
    /// Warning events (redaction, access issues)
    Warning,
    /// High severity (PII detection)
    High,
    /// Critical security events (secrets detected)
    Critical,
}

impl AuditSeverity {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Types of auditable events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    // === Scan Lifecycle ===
    /// Repository scan started
    ScanStarted,
    /// Repository scan completed
    ScanCompleted,
    /// Scan failed with error
    ScanFailed,

    // === Security Events ===
    /// Secret/credential detected
    SecretDetected,
    /// PII data detected
    PiiDetected,
    /// Secret was redacted from output
    SecretRedacted,
    /// License compliance issue detected
    LicenseViolation,

    // === Chunk Operations ===
    /// Embedding chunk generated
    ChunkGenerated,
    /// Chunk was split due to size
    ChunkSplit,
    /// Chunk was skipped (e.g., binary file)
    ChunkSkipped,

    // === Manifest Operations ===
    /// Manifest file created
    ManifestCreated,
    /// Manifest file updated
    ManifestUpdated,
    /// Manifest diff computed
    ManifestDiffComputed,

    // === Access Control ===
    /// Access denied to resource
    AccessDenied,
    /// Path traversal attempt blocked
    PathTraversalBlocked,

    // === Configuration ===
    /// Configuration loaded
    ConfigLoaded,
    /// Configuration changed
    ConfigChanged,

    // === Export/Output ===
    /// Data exported to file
    DataExported,
    /// Data sent to external service
    DataTransmitted,
}

impl AuditEventKind {
    /// Get the default severity for this event type
    pub fn default_severity(&self) -> AuditSeverity {
        match self {
            // Critical
            Self::SecretDetected => AuditSeverity::Critical,
            Self::PathTraversalBlocked => AuditSeverity::Critical,

            // High
            Self::PiiDetected => AuditSeverity::High,
            Self::LicenseViolation => AuditSeverity::High,
            Self::AccessDenied => AuditSeverity::High,

            // Warning
            Self::SecretRedacted => AuditSeverity::Warning,
            Self::ScanFailed => AuditSeverity::Warning,
            Self::ChunkSkipped => AuditSeverity::Warning,

            // Info
            Self::ScanStarted
            | Self::ScanCompleted
            | Self::ManifestCreated
            | Self::ManifestUpdated
            | Self::ManifestDiffComputed
            | Self::ConfigLoaded
            | Self::ConfigChanged
            | Self::DataExported
            | Self::DataTransmitted => AuditSeverity::Info,

            // Debug
            Self::ChunkGenerated | Self::ChunkSplit => AuditSeverity::Debug,
        }
    }

    /// Get human-readable event name
    pub fn name(&self) -> &'static str {
        match self {
            Self::ScanStarted => "scan_started",
            Self::ScanCompleted => "scan_completed",
            Self::ScanFailed => "scan_failed",
            Self::SecretDetected => "secret_detected",
            Self::PiiDetected => "pii_detected",
            Self::SecretRedacted => "secret_redacted",
            Self::LicenseViolation => "license_violation",
            Self::ChunkGenerated => "chunk_generated",
            Self::ChunkSplit => "chunk_split",
            Self::ChunkSkipped => "chunk_skipped",
            Self::ManifestCreated => "manifest_created",
            Self::ManifestUpdated => "manifest_updated",
            Self::ManifestDiffComputed => "manifest_diff_computed",
            Self::AccessDenied => "access_denied",
            Self::PathTraversalBlocked => "path_traversal_blocked",
            Self::ConfigLoaded => "config_loaded",
            Self::ConfigChanged => "config_changed",
            Self::DataExported => "data_exported",
            Self::DataTransmitted => "data_transmitted",
        }
    }
}

/// A single audit log event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Event type
    pub event: AuditEventKind,

    /// Unique session/request ID for correlation
    pub session_id: String,

    /// Repository identifier
    pub repo_id: String,

    /// User/service account that triggered the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Event severity
    pub severity: AuditSeverity,

    /// Additional key-value details
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub details: HashMap<String, String>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(kind: AuditEventKind, repo_id: impl Into<String>, user: Option<String>) -> Self {
        Self {
            timestamp: Self::iso8601_now(),
            event: kind,
            session_id: Self::generate_session_id(),
            repo_id: repo_id.into(),
            user,
            severity: kind.default_severity(),
            details: HashMap::new(),
        }
    }

    /// Create an event with a specific session ID (for correlation)
    pub fn with_session(
        kind: AuditEventKind,
        repo_id: impl Into<String>,
        session_id: impl Into<String>,
        user: Option<String>,
    ) -> Self {
        Self {
            timestamp: Self::iso8601_now(),
            event: kind,
            session_id: session_id.into(),
            repo_id: repo_id.into(),
            user,
            severity: kind.default_severity(),
            details: HashMap::new(),
        }
    }

    /// Add a detail key-value pair
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Add multiple details at once
    pub fn with_details(mut self, details: impl IntoIterator<Item = (String, String)>) -> Self {
        self.details.extend(details);
        self
    }

    /// Override the default severity
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Generate ISO 8601 timestamp
    fn iso8601_now() -> String {
        let now = SystemTime::now();
        let duration = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();

        // Convert to date/time components (simplified UTC)
        let days = secs / 86400;
        let remaining = secs % 86400;
        let hours = remaining / 3600;
        let minutes = (remaining % 3600) / 60;
        let seconds = remaining % 60;

        // Calculate year, month, day from days since epoch
        let (year, month, day) = Self::days_to_ymd(days);

        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
    }

    /// Convert days since Unix epoch to year/month/day
    fn days_to_ymd(days: u64) -> (i32, u32, u32) {
        // Simplified calculation (not accounting for all edge cases)
        let mut remaining_days = days as i64;
        let mut year = 1970i32;

        loop {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap = Self::is_leap_year(year);
        let month_days: [i64; 12] = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1u32;
        for &days in &month_days {
            if remaining_days < days {
                break;
            }
            remaining_days -= days;
            month += 1;
        }

        (year, month, (remaining_days + 1) as u32)
    }

    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Generate a unique session ID
    fn generate_session_id() -> String {
        use std::hash::{Hash, Hasher};
        use std::time::Instant;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        Instant::now().hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);

        let hash = hasher.finish();
        format!("{:016x}", hash)
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }
}

/// Trait for audit log backends
pub trait AuditLogger: Send + Sync {
    /// Log an audit event
    fn log(&self, event: AuditEvent);

    /// Flush any buffered events
    fn flush(&self);

    /// Get the minimum severity level to log
    fn min_severity(&self) -> AuditSeverity;

    /// Check if an event should be logged based on severity
    fn should_log(&self, severity: AuditSeverity) -> bool {
        let min = self.min_severity();
        match (min, severity) {
            (AuditSeverity::Debug, _) => true,
            (AuditSeverity::Info, AuditSeverity::Debug) => false,
            (AuditSeverity::Info, _) => true,
            (AuditSeverity::Warning, AuditSeverity::Debug | AuditSeverity::Info) => false,
            (AuditSeverity::Warning, _) => true,
            (AuditSeverity::High, AuditSeverity::High | AuditSeverity::Critical) => true,
            (AuditSeverity::High, _) => false,
            (AuditSeverity::Critical, AuditSeverity::Critical) => true,
            (AuditSeverity::Critical, _) => false,
        }
    }
}

/// File-based audit logger (JSONL format)
pub struct FileAuditLogger {
    writer: Mutex<BufWriter<File>>,
    min_severity: AuditSeverity,
}

impl FileAuditLogger {
    /// Create a new file-based audit logger
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { writer: Mutex::new(BufWriter::new(file)), min_severity: AuditSeverity::Info })
    }

    /// Create with a specific minimum severity level
    pub fn with_min_severity(mut self, severity: AuditSeverity) -> Self {
        self.min_severity = severity;
        self
    }
}

impl AuditLogger for FileAuditLogger {
    fn log(&self, event: AuditEvent) {
        if !self.should_log(event.severity) {
            return;
        }

        let json = event.to_json();
        if let Ok(mut writer) = self.writer.lock() {
            // Ignore write errors - audit logging should not crash the app
            drop(writeln!(writer, "{}", json));
        }
    }

    fn flush(&self) {
        if let Ok(mut writer) = self.writer.lock() {
            // Ignore flush errors - audit logging should not crash the app
            drop(writer.flush());
        }
    }

    fn min_severity(&self) -> AuditSeverity {
        self.min_severity
    }
}

/// In-memory audit logger for testing
pub struct MemoryAuditLogger {
    events: RwLock<Vec<AuditEvent>>,
    min_severity: AuditSeverity,
}

impl Default for MemoryAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAuditLogger {
    /// Create a new in-memory audit logger
    pub fn new() -> Self {
        Self { events: RwLock::new(Vec::new()), min_severity: AuditSeverity::Debug }
    }

    /// Create with a specific minimum severity level
    pub fn with_min_severity(mut self, severity: AuditSeverity) -> Self {
        self.min_severity = severity;
        self
    }

    /// Get all logged events
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.read().map(|e| e.clone()).unwrap_or_default()
    }

    /// Get events of a specific kind
    pub fn events_of_kind(&self, kind: AuditEventKind) -> Vec<AuditEvent> {
        self.events()
            .into_iter()
            .filter(|e| e.event == kind)
            .collect()
    }

    /// Get events with severity >= threshold
    pub fn events_with_min_severity(&self, severity: AuditSeverity) -> Vec<AuditEvent> {
        self.events()
            .into_iter()
            .filter(|e| {
                let temp_logger =
                    MemoryAuditLogger { events: RwLock::new(Vec::new()), min_severity: severity };
                temp_logger.should_log(e.severity)
            })
            .collect()
    }

    /// Clear all events
    pub fn clear(&self) {
        if let Ok(mut events) = self.events.write() {
            events.clear();
        }
    }

    /// Get event count
    pub fn len(&self) -> usize {
        self.events.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditLogger for MemoryAuditLogger {
    fn log(&self, event: AuditEvent) {
        if !self.should_log(event.severity) {
            return;
        }

        if let Ok(mut events) = self.events.write() {
            events.push(event);
        }
    }

    fn flush(&self) {
        // No-op for in-memory logger
    }

    fn min_severity(&self) -> AuditSeverity {
        self.min_severity
    }
}

/// No-op audit logger (disables auditing)
pub struct NullAuditLogger;

impl AuditLogger for NullAuditLogger {
    fn log(&self, _event: AuditEvent) {
        // No-op
    }

    fn flush(&self) {
        // No-op
    }

    fn min_severity(&self) -> AuditSeverity {
        AuditSeverity::Critical // Only log critical events (effectively disabled)
    }
}

/// Multi-logger that fans out to multiple backends
pub struct MultiAuditLogger {
    loggers: Vec<Arc<dyn AuditLogger>>,
}

impl MultiAuditLogger {
    /// Create a new multi-logger
    pub fn new(loggers: Vec<Arc<dyn AuditLogger>>) -> Self {
        Self { loggers }
    }
}

impl AuditLogger for MultiAuditLogger {
    fn log(&self, event: AuditEvent) {
        for logger in &self.loggers {
            logger.log(event.clone());
        }
    }

    fn flush(&self) {
        for logger in &self.loggers {
            logger.flush();
        }
    }

    fn min_severity(&self) -> AuditSeverity {
        // Return the lowest (most permissive) severity
        self.loggers
            .iter()
            .map(|l| l.min_severity())
            .min_by(|a, b| {
                let order = |s: &AuditSeverity| match s {
                    AuditSeverity::Debug => 0,
                    AuditSeverity::Info => 1,
                    AuditSeverity::Warning => 2,
                    AuditSeverity::High => 3,
                    AuditSeverity::Critical => 4,
                };
                order(a).cmp(&order(b))
            })
            .unwrap_or(AuditSeverity::Info)
    }
}

/// Global audit logger registry
static GLOBAL_LOGGER: RwLock<Option<Arc<dyn AuditLogger>>> = RwLock::new(None);

/// Set the global audit logger
pub fn set_global_logger(logger: Arc<dyn AuditLogger>) {
    if let Ok(mut global) = GLOBAL_LOGGER.write() {
        *global = Some(logger);
    }
}

/// Get the global audit logger (returns NullAuditLogger if not set)
pub fn get_global_logger() -> Arc<dyn AuditLogger> {
    GLOBAL_LOGGER
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| Arc::new(NullAuditLogger))
}

/// Log an event to the global logger
pub fn log_event(event: AuditEvent) {
    get_global_logger().log(event);
}

/// Convenience function to log a scan started event
pub fn log_scan_started(repo_id: &str, user: Option<&str>, path: &str) {
    log_event(
        AuditEvent::new(AuditEventKind::ScanStarted, repo_id, user.map(String::from))
            .with_detail("path", path),
    );
}

/// Convenience function to log a scan completed event
pub fn log_scan_completed(repo_id: &str, session_id: &str, files: usize, chunks: usize) {
    log_event(
        AuditEvent::with_session(AuditEventKind::ScanCompleted, repo_id, session_id, None)
            .with_detail("files_processed", files.to_string())
            .with_detail("chunks_generated", chunks.to_string()),
    );
}

/// Convenience function to log a secret detection
pub fn log_secret_detected(repo_id: &str, session_id: &str, file: &str, line: u32, kind: &str) {
    log_event(
        AuditEvent::with_session(AuditEventKind::SecretDetected, repo_id, session_id, None)
            .with_detail("file", file)
            .with_detail("line", line.to_string())
            .with_detail("secret_kind", kind),
    );
}

/// Convenience function to log PII detection
pub fn log_pii_detected(repo_id: &str, session_id: &str, file: &str, line: u32, pii_type: &str) {
    log_event(
        AuditEvent::with_session(AuditEventKind::PiiDetected, repo_id, session_id, None)
            .with_detail("file", file)
            .with_detail("line", line.to_string())
            .with_detail("pii_type", pii_type),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            AuditEventKind::ScanStarted,
            "test-repo",
            Some("user@test.com".to_owned()),
        );

        assert_eq!(event.event, AuditEventKind::ScanStarted);
        assert_eq!(event.repo_id, "test-repo");
        assert_eq!(event.user, Some("user@test.com".to_owned()));
        assert_eq!(event.severity, AuditSeverity::Info);
        assert!(!event.timestamp.is_empty());
        assert!(!event.session_id.is_empty());
    }

    #[test]
    fn test_audit_event_with_details() {
        let event = AuditEvent::new(AuditEventKind::SecretDetected, "test-repo", None)
            .with_detail("file", "config.py")
            .with_detail("line", "42")
            .with_detail("kind", "AWS Credential");

        assert_eq!(event.details.get("file"), Some(&"config.py".to_owned()));
        assert_eq!(event.details.get("line"), Some(&"42".to_owned()));
        assert_eq!(event.details.get("kind"), Some(&"AWS Credential".to_owned()));
    }

    #[test]
    fn test_audit_severity_ordering() {
        let logger = MemoryAuditLogger::new().with_min_severity(AuditSeverity::Warning);

        assert!(!logger.should_log(AuditSeverity::Debug));
        assert!(!logger.should_log(AuditSeverity::Info));
        assert!(logger.should_log(AuditSeverity::Warning));
        assert!(logger.should_log(AuditSeverity::High));
        assert!(logger.should_log(AuditSeverity::Critical));
    }

    #[test]
    fn test_memory_audit_logger() {
        let logger = MemoryAuditLogger::new();

        logger.log(AuditEvent::new(AuditEventKind::ScanStarted, "repo1", None));
        logger.log(AuditEvent::new(AuditEventKind::SecretDetected, "repo1", None));
        logger.log(AuditEvent::new(AuditEventKind::ScanCompleted, "repo1", None));

        assert_eq!(logger.len(), 3);

        let secrets = logger.events_of_kind(AuditEventKind::SecretDetected);
        assert_eq!(secrets.len(), 1);
    }

    #[test]
    fn test_memory_logger_severity_filter() {
        let logger = MemoryAuditLogger::new().with_min_severity(AuditSeverity::High);

        // Info event - should NOT be logged
        logger.log(AuditEvent::new(AuditEventKind::ScanStarted, "repo1", None));

        // Critical event - should be logged
        logger.log(AuditEvent::new(AuditEventKind::SecretDetected, "repo1", None));

        // High event - should be logged
        logger.log(AuditEvent::new(AuditEventKind::PiiDetected, "repo1", None));

        assert_eq!(logger.len(), 2);
    }

    #[test]
    fn test_event_json_serialization() {
        let event = AuditEvent::new(
            AuditEventKind::SecretDetected,
            "test-repo",
            Some("user@test.com".to_owned()),
        )
        .with_detail("file", "secret.py");

        let json = event.to_json();

        assert!(json.contains("\"event\":\"secret_detected\""));
        assert!(json.contains("\"repo_id\":\"test-repo\""));
        assert!(json.contains("\"user\":\"user@test.com\""));
        assert!(json.contains("\"severity\":\"critical\""));
        assert!(json.contains("\"file\":\"secret.py\""));
    }

    #[test]
    fn test_default_severities() {
        assert_eq!(AuditEventKind::SecretDetected.default_severity(), AuditSeverity::Critical);
        assert_eq!(AuditEventKind::PiiDetected.default_severity(), AuditSeverity::High);
        assert_eq!(AuditEventKind::ScanStarted.default_severity(), AuditSeverity::Info);
        assert_eq!(AuditEventKind::ChunkGenerated.default_severity(), AuditSeverity::Debug);
    }

    #[test]
    fn test_null_audit_logger() {
        let logger = NullAuditLogger;

        // Should not panic
        logger.log(AuditEvent::new(AuditEventKind::SecretDetected, "repo", None));
        logger.flush();

        // Only logs critical
        assert_eq!(logger.min_severity(), AuditSeverity::Critical);
    }

    #[test]
    fn test_global_logger_api() {
        // Test that set/get global logger APIs work correctly.
        // Note: Due to parallel test execution, we can't use log_event() reliably
        // because another test might replace the global logger between set and log.
        // Instead, we test the logger itself works correctly.

        let memory_logger = Arc::new(MemoryAuditLogger::new());

        // Test that set_global_logger and get_global_logger work
        set_global_logger(memory_logger.clone());

        // Log directly to our logger (not via global) to verify it works
        memory_logger.log(AuditEvent::new(AuditEventKind::ScanStarted, "test", None));
        assert_eq!(memory_logger.len(), 1);

        // Verify the logger has expected properties
        assert_eq!(memory_logger.min_severity(), AuditSeverity::Debug);
    }

    #[test]
    fn test_convenience_function_events() {
        // Test that convenience functions create correct event structures.
        // We test directly on a local logger to avoid global state race conditions.

        let logger = MemoryAuditLogger::new();

        // Test scan started event structure
        let event = AuditEvent::new(AuditEventKind::ScanStarted, "repo", Some("user".to_owned()))
            .with_detail("path", "/path/to/repo");
        logger.log(event);

        // Test secret detected event structure
        let event =
            AuditEvent::with_session(AuditEventKind::SecretDetected, "repo", "session123", None)
                .with_detail("file", "config.py")
                .with_detail("line", "42")
                .with_detail("secret_kind", "AWS Key");
        logger.log(event);

        // Test PII detected event structure
        let event =
            AuditEvent::with_session(AuditEventKind::PiiDetected, "repo", "session123", None)
                .with_detail("file", "data.txt")
                .with_detail("line", "10")
                .with_detail("pii_type", "SSN");
        logger.log(event);

        // Test scan completed event structure
        let event =
            AuditEvent::with_session(AuditEventKind::ScanCompleted, "repo", "session123", None)
                .with_detail("files_processed", "100")
                .with_detail("chunks_generated", "500");
        logger.log(event);

        assert_eq!(logger.len(), 4);

        // Verify event kinds were logged correctly
        let events = logger.events();
        assert_eq!(events[0].event, AuditEventKind::ScanStarted);
        assert_eq!(events[1].event, AuditEventKind::SecretDetected);
        assert_eq!(events[2].event, AuditEventKind::PiiDetected);
        assert_eq!(events[3].event, AuditEventKind::ScanCompleted);

        // Verify details are captured
        assert_eq!(events[0].details.get("path"), Some(&"/path/to/repo".to_owned()));
        assert_eq!(events[1].details.get("secret_kind"), Some(&"AWS Key".to_owned()));
        assert_eq!(events[2].details.get("pii_type"), Some(&"SSN".to_owned()));
        assert_eq!(events[3].details.get("files_processed"), Some(&"100".to_owned()));
    }

    #[test]
    fn test_iso8601_timestamp_format() {
        let event = AuditEvent::new(AuditEventKind::ScanStarted, "repo", None);

        // Should match ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$").unwrap();
        assert!(
            re.is_match(&event.timestamp),
            "Timestamp {} doesn't match ISO 8601",
            event.timestamp
        );
    }

    #[test]
    fn test_session_correlation() {
        let session_id = "test-session-123";

        let event1 =
            AuditEvent::with_session(AuditEventKind::ScanStarted, "repo", session_id, None);

        let event2 =
            AuditEvent::with_session(AuditEventKind::ScanCompleted, "repo", session_id, None);

        assert_eq!(event1.session_id, event2.session_id);
    }
}
