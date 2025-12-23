#![no_main]

use libfuzzer_sys::fuzz_target;
use infiniloom_engine::tokenizer::{TokenModel, Tokenizer};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(content) = std::str::from_utf8(data) {
        // Skip very large inputs to avoid timeouts
        if content.len() > 1_000_000 {
            return;
        }

        let tokenizer = Tokenizer::new();

        // Test multiple tokenizer models
        let _ = tokenizer.count(content, TokenModel::Claude);
        let _ = tokenizer.count(content, TokenModel::Gpt4o);
        let _ = tokenizer.count(content, TokenModel::Gemini);

        // Test truncation
        let _ = tokenizer.truncate_to_budget(content, TokenModel::Claude, 1000);
    }
});
