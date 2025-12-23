#![no_main]

use libfuzzer_sys::fuzz_target;
use infiniloom_engine::security::SecurityScanner;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(content) = std::str::from_utf8(data) {
        // Skip very large inputs to avoid timeouts
        if content.len() > 500_000 {
            return;
        }

        let scanner = SecurityScanner::new();

        // Test scanning
        let _ = scanner.scan(content, "test_file.txt");

        // Test scanning and redaction
        let _ = scanner.scan_and_redact(content, "test_file.txt");
    }
});
