#![no_main]

use libfuzzer_sys::fuzz_target;
use infiniloom_engine::parser::{Language, Parser};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(content) = std::str::from_utf8(data) {
        // Skip very large inputs to avoid timeouts
        if content.len() > 100_000 {
            return;
        }

        let mut parser = Parser::new();
        // We don't care about the result, just that it doesn't panic
        let _ = parser.parse(content, Language::JavaScript);
    }
});
