//! Watch Mode E2E Tests
//!
//! Tests user expectations for `infiniloom pack --watch`.

use super::helpers::*;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct ChildGuard {
    child: std::process::Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _unused = self.child.kill();
        let _unused = self.child.wait();
    }
}

fn wait_for_output_contains(path: &Path, needle: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains(needle) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// E2E-WATCH-001: Watch mode regenerates output on file changes
#[test]
fn test_watch_mode_regenerates_output() {
    let temp = create_temp_project();
    let output_dir = tempfile::tempdir().expect("create output dir");
    let output_path = output_dir.path().join("context.txt");

    let mut cmd = Command::new(infiniloom_binary());
    cmd.arg("pack")
        .arg(temp.path())
        .arg("--format")
        .arg("plain")
        .arg("--output")
        .arg(&output_path)
        .arg("--watch")
        .env("NO_COLOR", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().expect("failed to start watch process");
    let _guard = ChildGuard { child };

    // Wait for initial output
    assert!(
        wait_for_output_contains(&output_path, "Hello, world!", Duration::from_secs(10)),
        "watch output should be generated initially"
    );

    // Give the watcher time to start before triggering changes
    std::thread::sleep(Duration::from_secs(1));

    // Create a new file to trigger watch regeneration
    std::fs::write(temp.path().join("watch.txt"), "watch output regeneration test v1")
        .expect("Failed to create watch.txt");

    // Modify the file again to ensure a second event
    std::thread::sleep(Duration::from_secs(1));
    std::fs::write(temp.path().join("watch.txt"), "watch output regeneration test v2")
        .expect("Failed to update watch.txt");

    // Verify output updates with new file content
    assert!(
        wait_for_output_contains(
            &output_path,
            "watch output regeneration test v2",
            Duration::from_secs(15)
        ),
        "watch output should update after file changes"
    );
}
