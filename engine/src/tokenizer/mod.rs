//! Accurate token counting using actual BPE tokenizers
//!
//! This module provides accurate token counts using tiktoken for OpenAI models
//! and estimation-based counting for other models (~95% accuracy).
//!
//! # Quick Start
//!
//! ```rust
//! use infiniloom_engine::tokenizer::{Tokenizer, TokenModel};
//!
//! let tokenizer = Tokenizer::new();
//! let code = "fn main() { println!(\"Hello, world!\"); }";
//!
//! // Exact counting for OpenAI models (via tiktoken)
//! let gpt4o_tokens = tokenizer.count(code, TokenModel::Gpt4o);
//! println!("GPT-4o tokens: {}", gpt4o_tokens);
//!
//! // Calibrated estimation for other models
//! let claude_tokens = tokenizer.count(code, TokenModel::Claude);
//! println!("Claude tokens: {}", claude_tokens);
//! ```
//!
//! # Multi-Model Token Counting
//!
//! Count tokens for all supported models at once:
//!
//! ```rust,no_run
//! use infiniloom_engine::tokenizer::Tokenizer;
//!
//! let tokenizer = Tokenizer::new();
//! let source_code = std::fs::read_to_string("main.rs")?;
//!
//! // Returns TokenCounts struct with all models
//! let counts = tokenizer.count_all(&source_code);
//!
//! println!("OpenAI modern (o200k): {}", counts.o200k);    // GPT-5, GPT-4o, O1/O3/O4
//! println!("OpenAI legacy (cl100k): {}", counts.cl100k);  // GPT-4, GPT-3.5-turbo
//! println!("Claude: {}", counts.claude);
//! println!("Gemini: {}", counts.gemini);
//! println!("Llama: {}", counts.llama);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Repository-Wide Counting
//!
//! ```rust
//! use infiniloom_engine::{Repository, tokenizer::TokenModel};
//!
//! let repo = Repository::new("my-project", "/path/to/project");
//! // ... scan and populate repo.files
//!
//! // Get total tokens for specific model
//! let total_tokens = repo.total_tokens(TokenModel::Gpt4o);
//! println!("Total GPT-4o tokens: {}", total_tokens);
//!
//! // Check if within budget
//! let budget = 100_000;
//! if total_tokens > budget {
//!     eprintln!("Repository exceeds {} token budget!", budget);
//! }
//! ```
//!
//! # Performance Optimization
//!
//! The tokenizer is thread-safe and uses lazy initialization:
//!
//! ```rust,no_run
//! use infiniloom_engine::tokenizer::Tokenizer;
//! use rayon::prelude::*;
//!
//! let tokenizer = Tokenizer::new(); // Clone is cheap (Arc internally)
//! let files = vec!["file1.rs", "file2.rs", "file3.rs"];
//!
//! // Parallel token counting across multiple files
//! let token_counts: Vec<_> = files.par_iter()
//!     .map(|file| {
//!         let content = std::fs::read_to_string(file)?;
//!         Ok(tokenizer.count(&content, infiniloom_engine::tokenizer::TokenModel::Gpt4o))
//!     })
//!     .collect::<Result<Vec<_>, std::io::Error>>()?;
//!
//! let total: u32 = token_counts.iter().sum();
//! println!("Total tokens across all files: {}", total);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Quick Estimation (No Tokenizer Instance)
//!
//! For rough estimates without tiktoken overhead:
//!
//! ```rust
//! use infiniloom_engine::tokenizer::{quick_estimate, TokenModel};
//!
//! let text = "Some text to estimate";
//! let estimated_tokens = quick_estimate(text, TokenModel::Claude);
//!
//! // Uses model-specific estimation
//! println!("Estimated tokens: {}", estimated_tokens);
//! ```
//!
//! # Supported Models
//!
//! ## OpenAI (Exact tokenization via tiktoken)
//! - **o200k_base**: GPT-5.2, GPT-5.1, GPT-5, GPT-4o, O1, O3, O4-mini (all latest models)
//! - **cl100k_base**: GPT-4, GPT-3.5-turbo (legacy models)
//!
//! ## Other Vendors (Estimation-based, ~95% accuracy)
//! - Claude (Anthropic): ~3.5 chars/token
//! - Gemini (Google): ~3.8 chars/token
//! - Llama (Meta): ~3.5 chars/token
//! - Mistral: ~3.5 chars/token
//! - DeepSeek: ~3.5 chars/token
//! - Qwen (Alibaba): ~3.5 chars/token
//! - Cohere: ~3.6 chars/token
//! - Grok (xAI): ~3.5 chars/token
//!
//! # Why Estimation for Non-OpenAI Models?
//!
//! Most LLM vendors don't provide public tokenizers. We use calibrated
//! character-to-token ratios based on empirical testing. Accuracy is ~95%
//! for typical source code, which is sufficient for budget planning.
//!
//! OpenAI models use exact tiktoken-based counting because the tokenizers
//! are open-source and officially supported.

mod core;
mod counts;
mod models;

pub use core::{quick_estimate, Tokenizer};
pub use counts::TokenCounts;
pub use models::TokenModel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_gpt4o_counting() {
        let tokenizer = Tokenizer::new();
        let text = "Hello, world!";
        let count = tokenizer.count(text, TokenModel::Gpt4o);

        // o200k_base should give exact count
        assert!(count > 0);
        assert!(count < 10); // Should be around 3-4 tokens
    }

    #[test]
    fn test_exact_gpt5_counting() {
        let tokenizer = Tokenizer::new();
        let text = "def hello():\n    print('Hello, World!')\n";

        // All GPT-5 variants should use o200k_base and give same count
        let count_52 = tokenizer.count(text, TokenModel::Gpt52);
        let count_51 = tokenizer.count(text, TokenModel::Gpt51);
        let count_5 = tokenizer.count(text, TokenModel::Gpt5);
        let count_4o = tokenizer.count(text, TokenModel::Gpt4o);

        assert_eq!(count_52, count_51);
        assert_eq!(count_51, count_5);
        assert_eq!(count_5, count_4o);
        assert!(count_52 > 5);
        assert!(count_52 < 30);
    }

    #[test]
    fn test_exact_o_series_counting() {
        let tokenizer = Tokenizer::new();
        let text = "Solve this math problem: 2 + 2 = ?";

        // All O-series models should use o200k_base
        let count_o4 = tokenizer.count(text, TokenModel::O4Mini);
        let count_o3 = tokenizer.count(text, TokenModel::O3);
        let count_o1 = tokenizer.count(text, TokenModel::O1);
        let count_4o = tokenizer.count(text, TokenModel::Gpt4o);

        assert_eq!(count_o4, count_o3);
        assert_eq!(count_o3, count_o1);
        assert_eq!(count_o1, count_4o);
    }

    #[test]
    fn test_exact_gpt4_counting() {
        let tokenizer = Tokenizer::new();
        let text = "def hello():\n    print('Hello, World!')\n";
        let count = tokenizer.count(text, TokenModel::Gpt4);

        // cl100k_base should give exact count
        assert!(count > 5);
        assert!(count < 30);
    }

    #[test]
    fn test_estimation_claude() {
        let tokenizer = Tokenizer::new();
        let text = "This is a test string for token estimation.";
        let count = tokenizer.count(text, TokenModel::Claude);

        // Estimation should be reasonable
        assert!(count > 5);
        assert!(count < 30);
    }

    #[test]
    fn test_estimation_new_vendors() {
        let tokenizer = Tokenizer::new();
        let text = "This is a test string for new vendor token estimation.";

        // All estimation-based models should return reasonable counts
        let mistral = tokenizer.count(text, TokenModel::Mistral);
        let deepseek = tokenizer.count(text, TokenModel::DeepSeek);
        let qwen = tokenizer.count(text, TokenModel::Qwen);
        let cohere = tokenizer.count(text, TokenModel::Cohere);
        let grok = tokenizer.count(text, TokenModel::Grok);

        assert!(mistral > 5 && mistral < 50);
        assert!(deepseek > 5 && deepseek < 50);
        assert!(qwen > 5 && qwen < 50);
        assert!(cohere > 5 && cohere < 50);
        assert!(grok > 5 && grok < 50);
    }

    #[test]
    fn test_count_all() {
        let tokenizer = Tokenizer::new();
        let text = "function hello() { console.log('hello'); }";
        let counts = tokenizer.count_all(text);

        assert!(counts.o200k > 0);
        assert!(counts.cl100k > 0);
        assert!(counts.claude > 0);
        assert!(counts.gemini > 0);
        assert!(counts.llama > 0);
        assert!(counts.mistral > 0);
        assert!(counts.deepseek > 0);
        assert!(counts.qwen > 0);
        assert!(counts.cohere > 0);
        assert!(counts.grok > 0);
    }

    #[test]
    fn test_empty_string() {
        let tokenizer = Tokenizer::new();
        assert_eq!(tokenizer.count("", TokenModel::Claude), 0);
        assert_eq!(tokenizer.count("", TokenModel::Gpt4o), 0);
        assert_eq!(tokenizer.count("", TokenModel::Gpt52), 0);
        assert_eq!(tokenizer.count("", TokenModel::O3), 0);
    }

    #[test]
    fn test_truncate_to_budget() {
        let tokenizer = Tokenizer::new();
        let text = "This is a fairly long string that we want to truncate to fit within a smaller token budget for testing purposes.";

        let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 10);
        let count = tokenizer.count(truncated, TokenModel::Gpt4);

        assert!(count <= 10);
        assert!(truncated.len() < text.len());
    }

    #[test]
    fn test_quick_estimate() {
        let count = quick_estimate("Hello world", TokenModel::Claude);
        assert!(count > 0);
        assert!(count < 10);
    }

    #[test]
    fn test_token_counts_add() {
        let a = TokenCounts {
            o200k: 8,
            cl100k: 9,
            claude: 10,
            gemini: 8,
            llama: 10,
            mistral: 10,
            deepseek: 10,
            qwen: 10,
            cohere: 10,
            grok: 10,
        };
        let b = TokenCounts {
            o200k: 4,
            cl100k: 5,
            claude: 5,
            gemini: 4,
            llama: 5,
            mistral: 5,
            deepseek: 5,
            qwen: 5,
            cohere: 5,
            grok: 5,
        };
        let sum = a + b;

        assert_eq!(sum.o200k, 12);
        assert_eq!(sum.cl100k, 14);
        assert_eq!(sum.claude, 15);
    }

    #[test]
    fn test_token_counts_min_max() {
        let counts = TokenCounts {
            o200k: 100,
            cl100k: 110,
            claude: 95,
            gemini: 105,
            llama: 98,
            mistral: 97,
            deepseek: 96,
            qwen: 99,
            cohere: 102,
            grok: 101,
        };

        assert_eq!(counts.min(), 95);
        assert_eq!(counts.max(), 110);
    }

    #[test]
    fn test_most_efficient_model() {
        let tokenizer = Tokenizer::new();
        let text = "const x = 42;";
        let (_model, count) = tokenizer.most_efficient_model(text);

        // GPT-4o with o200k should usually be most efficient
        assert!(count > 0);
    }

    #[test]
    fn test_from_model_name_openai() {
        // GPT-5.2 variants
        assert_eq!(TokenModel::from_model_name("gpt-5.2"), Some(TokenModel::Gpt52));
        assert_eq!(TokenModel::from_model_name("GPT-5.2"), Some(TokenModel::Gpt52));
        assert_eq!(TokenModel::from_model_name("gpt-5.2-pro"), Some(TokenModel::Gpt52Pro));
        assert_eq!(TokenModel::from_model_name("gpt-5.2-2025-12-11"), Some(TokenModel::Gpt52));

        // GPT-5.1 variants
        assert_eq!(TokenModel::from_model_name("gpt-5.1"), Some(TokenModel::Gpt51));
        assert_eq!(TokenModel::from_model_name("gpt-5.1-mini"), Some(TokenModel::Gpt51Mini));
        assert_eq!(TokenModel::from_model_name("gpt-5.1-codex"), Some(TokenModel::Gpt51Codex));

        // GPT-5 variants
        assert_eq!(TokenModel::from_model_name("gpt-5"), Some(TokenModel::Gpt5));
        assert_eq!(TokenModel::from_model_name("gpt-5-mini"), Some(TokenModel::Gpt5Mini));
        assert_eq!(TokenModel::from_model_name("gpt-5-nano"), Some(TokenModel::Gpt5Nano));

        // O-series
        assert_eq!(TokenModel::from_model_name("o4-mini"), Some(TokenModel::O4Mini));
        assert_eq!(TokenModel::from_model_name("o3"), Some(TokenModel::O3));
        assert_eq!(TokenModel::from_model_name("o3-mini"), Some(TokenModel::O3Mini));
        assert_eq!(TokenModel::from_model_name("o1"), Some(TokenModel::O1));
        assert_eq!(TokenModel::from_model_name("o1-mini"), Some(TokenModel::O1Mini));
        assert_eq!(TokenModel::from_model_name("o1-preview"), Some(TokenModel::O1Preview));

        // GPT-4o
        assert_eq!(TokenModel::from_model_name("gpt-4o"), Some(TokenModel::Gpt4o));
        assert_eq!(TokenModel::from_model_name("gpt-4o-mini"), Some(TokenModel::Gpt4oMini));

        // Legacy
        assert_eq!(TokenModel::from_model_name("gpt-4"), Some(TokenModel::Gpt4));
        assert_eq!(TokenModel::from_model_name("gpt-3.5-turbo"), Some(TokenModel::Gpt35Turbo));
    }

    #[test]
    fn test_from_model_name_other_vendors() {
        // Claude
        assert_eq!(TokenModel::from_model_name("claude"), Some(TokenModel::Claude));
        assert_eq!(TokenModel::from_model_name("claude-sonnet"), Some(TokenModel::Claude));
        assert_eq!(TokenModel::from_model_name("claude-opus-4.5"), Some(TokenModel::Claude));

        // Gemini
        assert_eq!(TokenModel::from_model_name("gemini"), Some(TokenModel::Gemini));
        assert_eq!(TokenModel::from_model_name("gemini-2.5-pro"), Some(TokenModel::Gemini));

        // Llama
        assert_eq!(TokenModel::from_model_name("llama-4"), Some(TokenModel::Llama));
        assert_eq!(TokenModel::from_model_name("codellama"), Some(TokenModel::CodeLlama));

        // Mistral
        assert_eq!(TokenModel::from_model_name("mistral"), Some(TokenModel::Mistral));
        assert_eq!(TokenModel::from_model_name("codestral"), Some(TokenModel::Mistral));

        // DeepSeek
        assert_eq!(TokenModel::from_model_name("deepseek"), Some(TokenModel::DeepSeek));
        assert_eq!(TokenModel::from_model_name("deepseek-r1"), Some(TokenModel::DeepSeek));

        // Qwen
        assert_eq!(TokenModel::from_model_name("qwen3"), Some(TokenModel::Qwen));

        // Cohere
        assert_eq!(TokenModel::from_model_name("cohere"), Some(TokenModel::Cohere));
        assert_eq!(TokenModel::from_model_name("command-r+"), Some(TokenModel::Cohere));

        // Grok
        assert_eq!(TokenModel::from_model_name("grok-3"), Some(TokenModel::Grok));
    }

    #[test]
    fn test_from_model_name_unknown() {
        assert_eq!(TokenModel::from_model_name("unknown-model"), None);
        assert_eq!(TokenModel::from_model_name(""), None);
        assert_eq!(TokenModel::from_model_name("random"), None);
    }

    #[test]
    fn test_model_properties() {
        // Test uses_o200k
        assert!(TokenModel::Gpt52.uses_o200k());
        assert!(TokenModel::O3.uses_o200k());
        assert!(TokenModel::Gpt4o.uses_o200k());
        assert!(!TokenModel::Gpt4.uses_o200k());
        assert!(!TokenModel::Claude.uses_o200k());

        // Test uses_cl100k
        assert!(TokenModel::Gpt4.uses_cl100k());
        assert!(TokenModel::Gpt35Turbo.uses_cl100k());
        assert!(!TokenModel::Gpt52.uses_cl100k());
        assert!(!TokenModel::Claude.uses_cl100k());

        // Test has_exact_tokenizer
        assert!(TokenModel::Gpt52.has_exact_tokenizer());
        assert!(TokenModel::Gpt4.has_exact_tokenizer());
        assert!(!TokenModel::Claude.has_exact_tokenizer());
        assert!(!TokenModel::Mistral.has_exact_tokenizer());

        // Test vendor
        assert_eq!(TokenModel::Gpt52.vendor(), "OpenAI");
        assert_eq!(TokenModel::Claude.vendor(), "Anthropic");
        assert_eq!(TokenModel::Gemini.vendor(), "Google");
        assert_eq!(TokenModel::Llama.vendor(), "Meta");
        assert_eq!(TokenModel::Mistral.vendor(), "Mistral AI");
        assert_eq!(TokenModel::DeepSeek.vendor(), "DeepSeek");
        assert_eq!(TokenModel::Qwen.vendor(), "Alibaba");
        assert_eq!(TokenModel::Cohere.vendor(), "Cohere");
        assert_eq!(TokenModel::Grok.vendor(), "xAI");
    }

    #[test]
    fn test_all_models() {
        let all = TokenModel::all();
        assert_eq!(all.len(), 27); // 18 OpenAI (16 o200k_base + 2 cl100k_base) + 9 other vendors
        assert!(all.contains(&TokenModel::Gpt52));
        assert!(all.contains(&TokenModel::O3));
        assert!(all.contains(&TokenModel::Claude));
        assert!(all.contains(&TokenModel::Mistral));
    }

    #[test]
    fn test_tokenizer_caching() {
        let tokenizer = Tokenizer::new();
        let text = "This is a test string for caching verification.";

        // First call - computes and caches
        let count1 = tokenizer.count(text, TokenModel::Gpt4o);

        // Second call - should return cached value
        let count2 = tokenizer.count(text, TokenModel::Gpt4o);

        // Both should be equal
        assert_eq!(count1, count2);
        assert!(count1 > 0);

        // Different model should have different cache entry
        let count_claude = tokenizer.count(text, TokenModel::Claude);
        assert!(count_claude > 0);
    }

    #[test]
    fn test_tokenizer_without_cache() {
        let tokenizer = Tokenizer::without_cache();
        let text = "Test text for uncached counting.";

        // Should still work correctly, just without caching
        let count = tokenizer.count(text, TokenModel::Gpt4o);
        assert!(count > 0);
        assert!(count < 20);
    }

    // =========================================================================
    // Additional edge case tests for comprehensive coverage
    // =========================================================================

    #[test]
    fn test_all_models_return_nonzero_for_content() {
        let tokenizer = Tokenizer::new();
        let content = "fn main() { println!(\"Hello, world!\"); }";

        // Test every single model returns a non-zero count
        for model in TokenModel::all() {
            let count = tokenizer.count(content, *model);
            assert!(count > 0, "Model {:?} returned 0 tokens for non-empty content", model);
        }
    }

    #[test]
    fn test_unicode_content_handling() {
        let tokenizer = Tokenizer::new();

        // Test various Unicode content
        let unicode_samples = [
            "Hello, 世界! 🌍",         // Mixed ASCII, CJK, emoji
            "Привет мир",              // Cyrillic
            "مرحبا بالعالم",           // Arabic (RTL)
            "🦀🦀🦀 Rust 🦀🦀🦀",      // Emoji-heavy
            "const λ = (x) => x * 2;", // Greek letters in code
        ];

        for sample in unicode_samples {
            let count = tokenizer.count(sample, TokenModel::Gpt4o);
            assert!(count > 0, "Unicode sample '{}' returned 0 tokens", sample);

            // Verify truncation doesn't break UTF-8
            let truncated = tokenizer.truncate_to_budget(sample, TokenModel::Gpt4o, 3);
            assert!(truncated.is_char_boundary(truncated.len()));
        }
    }

    #[test]
    fn test_very_long_content() {
        let tokenizer = Tokenizer::new();

        // Generate ~100KB of content
        let long_content: String = (0..10000)
            .map(|i| format!("Line {}: some repeated content here\n", i))
            .collect();

        // Should handle large content without panicking
        let count = tokenizer.count(&long_content, TokenModel::Claude);
        assert!(count > 1000, "Long content should have many tokens");

        // Truncation should work efficiently
        let truncated = tokenizer.truncate_to_budget(&long_content, TokenModel::Claude, 100);
        let truncated_count = tokenizer.count(truncated, TokenModel::Claude);
        assert!(truncated_count <= 100, "Truncation should respect budget");
    }

    #[test]
    fn test_whitespace_only_content() {
        let tokenizer = Tokenizer::new();

        let whitespace_samples = [
            "   ",        // Spaces
            "\t\t\t",     // Tabs
            "\n\n\n",     // Newlines
            "  \t  \n  ", // Mixed
        ];

        for sample in whitespace_samples {
            // Should not panic and should return some count (even if small)
            let _count = tokenizer.count(sample, TokenModel::Gpt4o);
        }
    }

    #[test]
    fn test_special_characters_heavy_code() {
        let tokenizer = Tokenizer::new();

        // Code-heavy content with many special characters
        let code = r#"
            fn process<T: Clone + Debug>(items: &[T]) -> Result<Vec<T>, Error> {
                items.iter()
                    .filter(|x| x.is_valid())
                    .map(|x| x.clone())
                    .collect::<Result<Vec<_>, _>>()
            }
        "#;

        let count = tokenizer.count(code, TokenModel::CodeLlama);
        assert!(count > 10, "Code content should have meaningful token count");

        // CodeLlama should handle code differently than general models
        let claude_count = tokenizer.count(code, TokenModel::Claude);
        // Both should be reasonable but may differ
        assert!(claude_count > 10);
    }

    #[test]
    fn test_model_get_consistency() {
        // Verify TokenCounts.get() returns correct values for all model families
        let counts = TokenCounts {
            o200k: 100,
            cl100k: 110,
            claude: 95,
            gemini: 105,
            llama: 98,
            mistral: 97,
            deepseek: 96,
            qwen: 99,
            cohere: 102,
            grok: 101,
        };

        // All o200k models should return the same count
        assert_eq!(counts.get(TokenModel::Gpt52), 100);
        assert_eq!(counts.get(TokenModel::Gpt4o), 100);
        assert_eq!(counts.get(TokenModel::O3), 100);

        // cl100k models
        assert_eq!(counts.get(TokenModel::Gpt4), 110);
        assert_eq!(counts.get(TokenModel::Gpt35Turbo), 110);

        // Individual vendors
        assert_eq!(counts.get(TokenModel::Claude), 95);
        assert_eq!(counts.get(TokenModel::Gemini), 105);
        assert_eq!(counts.get(TokenModel::Llama), 98);
        assert_eq!(counts.get(TokenModel::CodeLlama), 98); // Same as Llama
        assert_eq!(counts.get(TokenModel::Mistral), 97);
        assert_eq!(counts.get(TokenModel::DeepSeek), 96);
        assert_eq!(counts.get(TokenModel::Qwen), 99);
        assert_eq!(counts.get(TokenModel::Cohere), 102);
        assert_eq!(counts.get(TokenModel::Grok), 101);
    }

    #[test]
    fn test_budget_exactly_met() {
        let tokenizer = Tokenizer::new();
        let text = "Hello world!";
        let exact_budget = tokenizer.count(text, TokenModel::Gpt4o);

        // Content that exactly meets budget should not be truncated
        let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4o, exact_budget);
        assert_eq!(truncated, text);
    }

    #[test]
    fn test_exceeds_budget_check() {
        let tokenizer = Tokenizer::new();
        let text = "A fairly long string that should have a decent number of tokens.";

        assert!(tokenizer.exceeds_budget(text, TokenModel::Claude, 1));
        assert!(!tokenizer.exceeds_budget(text, TokenModel::Claude, 1000));
        assert!(!tokenizer.exceeds_budget("", TokenModel::Claude, 0));
    }
}
