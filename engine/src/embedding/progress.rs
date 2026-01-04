//! Progress reporting abstraction for the embedding system
//!
//! This module provides a trait-based progress reporting system that can be:
//! - Used with terminal progress bars (indicatif)
//! - Silenced for tests/scripts
//! - Extended for custom reporting (webhooks, logging, etc.)

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Progress reporting abstraction
///
/// Implementations must be thread-safe (Send + Sync) for parallel processing.
pub trait ProgressReporter: Send + Sync {
    /// Set the current phase/stage name
    fn set_phase(&self, phase: &str);

    /// Set the total number of items to process
    fn set_total(&self, total: usize);

    /// Set the current progress (items completed)
    fn set_progress(&self, current: usize);

    /// Increment progress by one
    fn increment(&self) {
        // Default implementation - subclasses may override for efficiency
    }

    /// Log a warning message
    fn warn(&self, message: &str);

    /// Log an info message
    fn info(&self, message: &str);

    /// Log a debug message (may be ignored in production)
    fn debug(&self, _message: &str) {
        // Default: ignore debug messages
    }

    /// Mark progress as complete
    fn finish(&self) {
        // Default: no-op
    }
}

/// Terminal progress reporter with counters
///
/// This is a simple implementation that tracks progress internally.
/// For actual terminal UI, use the CLI's TerminalProgress which wraps indicatif.
pub struct TerminalProgress {
    phase: std::sync::RwLock<String>,
    total: AtomicUsize,
    current: AtomicUsize,
    show_output: bool,
}

impl TerminalProgress {
    /// Create a new terminal progress reporter
    pub fn new() -> Self {
        Self {
            phase: std::sync::RwLock::new(String::new()),
            total: AtomicUsize::new(0),
            current: AtomicUsize::new(0),
            show_output: true,
        }
    }

    /// Create a terminal progress reporter with optional output
    pub fn with_output(show_output: bool) -> Self {
        Self {
            phase: std::sync::RwLock::new(String::new()),
            total: AtomicUsize::new(0),
            current: AtomicUsize::new(0),
            show_output,
        }
    }

    /// Get current progress as (current, total)
    pub fn progress(&self) -> (usize, usize) {
        (self.current.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed))
    }

    /// Get current phase name
    pub fn phase(&self) -> String {
        self.phase.read().unwrap().clone()
    }
}

impl Default for TerminalProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter for TerminalProgress {
    fn set_phase(&self, phase: &str) {
        *self.phase.write().unwrap() = phase.to_owned();
        if self.show_output {
            eprintln!("[infiniloom] {phase}");
        }
    }

    fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
    }

    fn set_progress(&self, current: usize) {
        self.current.store(current, Ordering::Relaxed);
    }

    fn increment(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    fn warn(&self, message: &str) {
        if self.show_output {
            eprintln!("[infiniloom] WARN: {message}");
        }
    }

    fn info(&self, message: &str) {
        if self.show_output {
            eprintln!("[infiniloom] INFO: {message}");
        }
    }

    fn debug(&self, message: &str) {
        if self.show_output {
            eprintln!("[infiniloom] DEBUG: {message}");
        }
    }
}

/// Quiet progress reporter (no output)
///
/// Use this for tests, scripts, or when output should be suppressed.
pub struct QuietProgress;

impl ProgressReporter for QuietProgress {
    fn set_phase(&self, _: &str) {}
    fn set_total(&self, _: usize) {}
    fn set_progress(&self, _: usize) {}
    fn increment(&self) {}
    fn warn(&self, _: &str) {}
    fn info(&self, _: &str) {}
}

/// Callback-based progress reporter
///
/// Allows custom handling of progress events.
pub(super) struct CallbackProgress<F>
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    callback: F,
    total: AtomicUsize,
    current: AtomicUsize,
}

impl<F> CallbackProgress<F>
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    /// Create a new callback progress reporter
    pub(super) fn new(callback: F) -> Self {
        Self { callback, total: AtomicUsize::new(0), current: AtomicUsize::new(0) }
    }
}

impl<F> ProgressReporter for CallbackProgress<F>
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    fn set_phase(&self, phase: &str) {
        (self.callback)(ProgressEvent::Phase(phase.to_owned()));
    }

    fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        (self.callback)(ProgressEvent::Total(total));
    }

    fn set_progress(&self, current: usize) {
        self.current.store(current, Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);
        (self.callback)(ProgressEvent::Progress { current, total });
    }

    fn increment(&self) {
        let current = self.current.fetch_add(1, Ordering::Relaxed) + 1;
        let total = self.total.load(Ordering::Relaxed);
        (self.callback)(ProgressEvent::Progress { current, total });
    }

    fn warn(&self, message: &str) {
        (self.callback)(ProgressEvent::Warning(message.to_owned()));
    }

    fn info(&self, message: &str) {
        (self.callback)(ProgressEvent::Info(message.to_owned()));
    }

    fn debug(&self, message: &str) {
        (self.callback)(ProgressEvent::Debug(message.to_owned()));
    }

    fn finish(&self) {
        (self.callback)(ProgressEvent::Finished);
    }
}

/// Progress event for callback-based reporting
#[derive(Debug, Clone)]
pub(super) enum ProgressEvent {
    /// Phase changed
    Phase(String),
    /// Total items set
    Total(usize),
    /// Progress updated
    Progress { current: usize, total: usize },
    /// Warning message
    Warning(String),
    /// Info message
    Info(String),
    /// Debug message
    Debug(String),
    /// Processing finished
    Finished,
}

/// Shared progress reporter that can be cloned
///
/// Wraps a ProgressReporter in an Arc for sharing across threads.
#[derive(Clone)]
pub(super) struct SharedProgress {
    inner: Arc<dyn ProgressReporter>,
}

impl SharedProgress {
    /// Create a new shared progress reporter
    pub(super) fn new<P: ProgressReporter + 'static>(reporter: P) -> Self {
        Self { inner: Arc::new(reporter) }
    }

    /// Create a quiet shared progress reporter
    pub(super) fn quiet() -> Self {
        Self::new(QuietProgress)
    }
}

impl ProgressReporter for SharedProgress {
    fn set_phase(&self, phase: &str) {
        self.inner.set_phase(phase);
    }

    fn set_total(&self, total: usize) {
        self.inner.set_total(total);
    }

    fn set_progress(&self, current: usize) {
        self.inner.set_progress(current);
    }

    fn increment(&self) {
        self.inner.increment();
    }

    fn warn(&self, message: &str) {
        self.inner.warn(message);
    }

    fn info(&self, message: &str) {
        self.inner.info(message);
    }

    fn debug(&self, message: &str) {
        self.inner.debug(message);
    }

    fn finish(&self) {
        self.inner.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_quiet_progress() {
        let progress = QuietProgress;
        // Should not panic
        progress.set_phase("test");
        progress.set_total(100);
        progress.set_progress(50);
        progress.warn("warning");
        progress.info("info");
    }

    #[test]
    fn test_terminal_progress() {
        let progress = TerminalProgress::with_output(false);

        progress.set_phase("Scanning");
        progress.set_total(100);
        progress.set_progress(50);
        progress.increment();

        let (current, total) = progress.progress();
        assert_eq!(current, 51);
        assert_eq!(total, 100);
        assert_eq!(progress.phase(), "Scanning");
    }

    #[test]
    fn test_callback_progress() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let progress = CallbackProgress::new(move |event| {
            events_clone.lock().unwrap().push(event);
        });

        progress.set_phase("Testing");
        progress.set_total(10);
        progress.set_progress(5);
        progress.increment();
        progress.warn("test warning");
        progress.finish();

        let captured = events.lock().unwrap();
        assert!(captured.len() >= 5);
    }

    #[test]
    fn test_shared_progress() {
        let progress = SharedProgress::new(TerminalProgress::with_output(false));

        // Clone and use from multiple "threads"
        let p1 = progress.clone();
        let p2 = progress;

        p1.set_total(100);
        p2.set_progress(50);

        // Both should work
        p1.increment();
        p2.increment();
    }

    #[test]
    fn test_shared_progress_quiet() {
        let progress = SharedProgress::quiet();
        progress.set_phase("test");
        progress.set_total(100);
        // Should not panic
    }
}
