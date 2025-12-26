//! Core tokenizer implementation
//!
//! This module provides the main Tokenizer struct with accurate BPE tokenization
//! for OpenAI models and estimation-based counting for other models.

use super::counts::TokenCounts;
use super::models::TokenModel;
use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

/// Global tokenizer instances (lazy initialized, thread-safe)
static GPT4O_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
static GPT4_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Global token count cache - keyed by (content_hash, model)
/// This provides significant speedup when the same content is tokenized multiple times.
static TOKEN_CACHE: OnceLock<DashMap<(u64, TokenModel), u32>> = OnceLock::new();

/// Maximum number of entries in the token cache before eviction.
/// 100K entries ≈ 2.4MB memory (24 bytes per entry: 8 + 8 + 4 + padding).
/// This prevents unbounded memory growth in long-running processes.
const MAX_CACHE_ENTRIES: usize = 100_000;

/// Get or initialize the global token cache
fn get_token_cache() -> &'static DashMap<(u64, TokenModel), u32> {
    TOKEN_CACHE.get_or_init(DashMap::new)
}

/// Check if cache needs cleanup and clear if it exceeds the limit.
/// Uses a simple strategy: when cache is full, clear it entirely.
/// This is fast and avoids complex LRU tracking overhead.
fn maybe_cleanup_cache(cache: &DashMap<(u64, TokenModel), u32>) {
    if cache.len() >= MAX_CACHE_ENTRIES {
        cache.clear();
    }
}

/// Compute a fast hash of content for cache keys
fn hash_content(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Get or initialize the GPT-4o tokenizer (o200k_base)
fn get_gpt4o_tokenizer() -> &'static CoreBPE {
    GPT4O_TOKENIZER.get_or_init(|| {
        o200k_base().expect("tiktoken o200k_base initialization failed - please report this bug")
    })
}

/// Get or initialize the GPT-4 tokenizer (cl100k_base)
fn get_gpt4_tokenizer() -> &'static CoreBPE {
    GPT4_TOKENIZER.get_or_init(|| {
        cl100k_base().expect("tiktoken cl100k_base initialization failed - please report this bug")
    })
}

/// Accurate token counter with fallback to estimation
///
/// The tokenizer supports caching to avoid re-computing token counts for the same content.
/// This is particularly useful when processing files multiple times or across different
/// operations.
pub struct Tokenizer {
    /// Use exact tokenization when available
    use_exact: bool,
    /// Use global cache for token counts
    use_cache: bool,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer {
    /// Create a new tokenizer with exact mode and caching enabled
    pub fn new() -> Self {
        Self { use_exact: true, use_cache: true }
    }

    /// Create a tokenizer that only uses estimation (faster but less accurate)
    pub fn estimation_only() -> Self {
        Self { use_exact: false, use_cache: true }
    }

    /// Create a tokenizer without caching (useful for benchmarks or one-off counts)
    pub fn without_cache() -> Self {
        Self { use_exact: true, use_cache: false }
    }

    /// Count tokens for a specific model.
    ///
    /// When caching is enabled, results are stored in a global cache keyed by
    /// content hash and model. This provides significant speedup for repeated
    /// tokenization of the same content.
    ///
    /// # Returns
    ///
    /// The token count for the specified model. For OpenAI models (GPT-4o, GPT-4, etc.),
    /// this is exact via tiktoken. For other models, it's a calibrated estimation.
    #[must_use]
    pub fn count(&self, text: &str, model: TokenModel) -> u32 {
        if text.is_empty() {
            return 0;
        }

        if self.use_cache {
            let cache = get_token_cache();
            let content_hash = hash_content(text);
            let key = (content_hash, model);

            // Check cache first
            if let Some(count) = cache.get(&key) {
                return *count;
            }

            // Compute and cache (with size limit enforcement)
            let count = self.count_uncached(text, model);
            maybe_cleanup_cache(cache);
            cache.insert(key, count);
            count
        } else {
            self.count_uncached(text, model)
        }
    }

    /// Count tokens without using cache
    fn count_uncached(&self, text: &str, model: TokenModel) -> u32 {
        if self.use_exact && model.has_exact_tokenizer() {
            self.count_exact(text, model)
        } else {
            self.estimate(text, model)
        }
    }

    /// Count tokens using exact BPE encoding.
    /// Falls back to estimation if tiktoken panics (rare edge cases with unusual byte sequences).
    fn count_exact(&self, text: &str, model: TokenModel) -> u32 {
        if model.uses_o200k() {
            // All modern OpenAI models use o200k_base encoding
            // GPT-5.x, GPT-4o, O1, O3, O4
            let tokenizer = get_gpt4o_tokenizer();
            // Catch panics from tiktoken on malformed input
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tokenizer.encode_ordinary(text).len() as u32
            })) {
                Ok(count) => count,
                Err(_) => self.estimate(text, model), // Fallback to estimation on panic
            }
        } else if model.uses_cl100k() {
            // Legacy OpenAI models use cl100k_base encoding
            // GPT-4, GPT-3.5-turbo
            let tokenizer = get_gpt4_tokenizer();
            // Catch panics from tiktoken on malformed input
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tokenizer.encode_ordinary(text).len() as u32
            })) {
                Ok(count) => count,
                Err(_) => self.estimate(text, model), // Fallback to estimation on panic
            }
        } else {
            // Non-OpenAI models use estimation
            self.estimate(text, model)
        }
    }

    /// Estimate tokens using character-based heuristics
    fn estimate(&self, text: &str, model: TokenModel) -> u32 {
        if text.is_empty() {
            return 0;
        }

        let chars_per_token = model.chars_per_token();
        let len = text.len() as f32;

        // Base estimation
        let mut estimate = len / chars_per_token;

        // Count whitespace (often merged with adjacent tokens)
        let whitespace_count = text.chars().filter(|c| *c == ' ' || *c == '\t').count() as f32;
        estimate -= whitespace_count * 0.3;

        // Count newlines (usually single tokens)
        let newline_count = text.chars().filter(|c| *c == '\n').count() as f32;
        estimate += newline_count * 0.5;

        // Adjust for special characters (often separate tokens)
        let special_chars = text
            .chars()
            .filter(|c| {
                matches!(
                    c,
                    '{' | '}'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | ';'
                        | ':'
                        | ','
                        | '.'
                        | '='
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '<'
                        | '>'
                        | '!'
                        | '&'
                        | '|'
                        | '@'
                        | '#'
                        | '$'
                        | '%'
                        | '^'
                        | '~'
                        | '`'
                        | '"'
                        | '\''
                )
            })
            .count() as f32;

        // Code-focused models handle special chars differently
        if matches!(
            model,
            TokenModel::CodeLlama | TokenModel::Claude | TokenModel::DeepSeek | TokenModel::Mistral
        ) {
            estimate += special_chars * 0.3;
        }

        estimate.ceil().max(1.0) as u32
    }

    /// Count tokens for all supported models at once
    ///
    /// Returns counts for representative models from each encoding family:
    /// - `o200k`: GPT-5.x, GPT-4o, O1/O3/O4 (all use same tokenizer)
    /// - `cl100k`: GPT-4, GPT-3.5-turbo (legacy, same tokenizer)
    /// - Other vendors use estimation
    pub fn count_all(&self, text: &str) -> TokenCounts {
        TokenCounts {
            // OpenAI o200k_base (GPT-5.x, GPT-4o, O-series all share this)
            o200k: self.count(text, TokenModel::Gpt4o),
            // OpenAI cl100k_base (legacy GPT-4, GPT-3.5)
            cl100k: self.count(text, TokenModel::Gpt4),
            // Other vendors (estimation-based)
            claude: self.count(text, TokenModel::Claude),
            gemini: self.count(text, TokenModel::Gemini),
            llama: self.count(text, TokenModel::Llama),
            mistral: self.count(text, TokenModel::Mistral),
            deepseek: self.count(text, TokenModel::DeepSeek),
            qwen: self.count(text, TokenModel::Qwen),
            cohere: self.count(text, TokenModel::Cohere),
            grok: self.count(text, TokenModel::Grok),
        }
    }

    /// Estimate which model will have the lowest token count
    pub fn most_efficient_model(&self, text: &str) -> (TokenModel, u32) {
        let counts = self.count_all(text);
        let models = [
            (TokenModel::Gpt4o, counts.o200k), // GPT-5.x, GPT-4o, O-series
            (TokenModel::Gpt4, counts.cl100k), // Legacy GPT-4
            (TokenModel::Claude, counts.claude),
            (TokenModel::Gemini, counts.gemini),
            (TokenModel::Llama, counts.llama),
            (TokenModel::Mistral, counts.mistral),
            (TokenModel::DeepSeek, counts.deepseek),
            (TokenModel::Qwen, counts.qwen),
            (TokenModel::Cohere, counts.cohere),
            (TokenModel::Grok, counts.grok),
        ];

        // Safe: models array is non-empty, so min_by_key always returns Some
        models
            .into_iter()
            .min_by_key(|(_, count)| *count)
            .unwrap_or((TokenModel::Claude, 0))
    }

    /// Truncate text to fit within a token budget
    pub fn truncate_to_budget<'a>(&self, text: &'a str, model: TokenModel, budget: u32) -> &'a str {
        let current = self.count(text, model);
        if current <= budget {
            return text;
        }

        // Binary search for the right truncation point
        let mut low = 0usize;
        let mut high = text.len();

        while low < high {
            let mid_raw = (low + high).div_ceil(2);
            // Find valid UTF-8 boundary (rounds down)
            let mid = text.floor_char_boundary(mid_raw);

            // CRITICAL: Prevent infinite loop when low and high converge within
            // a multi-byte UTF-8 character. If floor_char_boundary rounds mid
            // back to low, we can't make progress - break out.
            if mid <= low {
                break;
            }

            let count = self.count(&text[..mid], model);

            if count <= budget {
                low = mid;
            } else {
                high = mid.saturating_sub(1);
            }
        }

        // Try to truncate at word boundary
        let mut end = low;
        while end > 0 {
            let c = text.as_bytes().get(end - 1).copied().unwrap_or(0);
            if c == b' ' || c == b'\n' {
                break;
            }
            end -= 1;
        }

        if end > 0 {
            &text[..end]
        } else {
            let low = text.floor_char_boundary(low);
            &text[..low]
        }
    }

    /// Check if text exceeds a token budget
    pub fn exceeds_budget(&self, text: &str, model: TokenModel, budget: u32) -> bool {
        self.count(text, model) > budget
    }
}

/// Quick estimation without creating a Tokenizer instance
pub fn quick_estimate(text: &str, model: TokenModel) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let chars_per_token = model.chars_per_token();
    (text.len() as f32 / chars_per_token).ceil().max(1.0) as u32
}
