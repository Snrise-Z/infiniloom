//! Accurate token counting using actual BPE tokenizers
//!
//! This module provides accurate token counts using tiktoken for OpenAI models
//! and estimation-based counting for other models.
//!
//! # Supported Models
//!
//! ## OpenAI (Exact tokenization via tiktoken)
//! - **o200k_base**: GPT-5.2, GPT-5.1, GPT-5, GPT-4o, O1, O3, O4 (all latest models)
//! - **cl100k_base**: GPT-4, GPT-3.5-turbo (legacy models)
//!
//! ## Other Vendors (Estimation-based)
//! - Claude (Anthropic): ~3.5 chars/token
//! - Gemini (Google): ~3.8 chars/token
//! - Llama (Meta): ~3.5 chars/token
//! - Mistral: ~3.5 chars/token
//! - DeepSeek: ~3.5 chars/token
//! - Qwen (Alibaba): ~3.5 chars/token
//! - Cohere: ~3.6 chars/token
//! - Grok (xAI): ~3.5 chars/token

use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

/// Supported LLM models for token counting
///
/// Models are grouped by their tokenizer encoding family. Use [`TokenModel::from_model_name`]
/// to parse user-friendly model names like "gpt-5.2", "o3", "claude-sonnet", etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TokenModel {
    // =========================================================================
    // OpenAI Models - o200k_base encoding (EXACT tokenization)
    // =========================================================================
    /// GPT-5.2 - Latest flagship model (Dec 2025), uses o200k_base
    Gpt52,
    /// GPT-5.2 Pro - Enhanced GPT-5.2 variant, uses o200k_base
    Gpt52Pro,
    /// GPT-5.1 - Previous flagship (Nov 2025), uses o200k_base
    Gpt51,
    /// GPT-5.1 Mini - Smaller GPT-5.1 variant, uses o200k_base
    Gpt51Mini,
    /// GPT-5.1 Codex - Code-specialized variant, uses o200k_base
    Gpt51Codex,
    /// GPT-5 - Original GPT-5 (Aug 2025), uses o200k_base
    Gpt5,
    /// GPT-5 Mini - Smaller GPT-5 variant, uses o200k_base
    Gpt5Mini,
    /// GPT-5 Nano - Smallest GPT-5 variant, uses o200k_base
    Gpt5Nano,
    /// O4 Mini - Latest reasoning model, uses o200k_base
    O4Mini,
    /// O3 - Reasoning model, uses o200k_base
    O3,
    /// O3 Mini - Smaller O3 variant, uses o200k_base
    O3Mini,
    /// O1 - Original reasoning model, uses o200k_base
    O1,
    /// O1 Mini - Smaller O1 variant, uses o200k_base
    O1Mini,
    /// O1 Preview - O1 preview version, uses o200k_base
    O1Preview,
    /// GPT-4o - Omni model, uses o200k_base encoding (most efficient)
    Gpt4o,
    /// GPT-4o Mini - Smaller GPT-4o variant, uses o200k_base encoding
    Gpt4oMini,

    // =========================================================================
    // OpenAI Models - cl100k_base encoding (EXACT tokenization, legacy)
    // =========================================================================
    /// GPT-4/GPT-4 Turbo - uses cl100k_base encoding (legacy)
    Gpt4,
    /// GPT-3.5-turbo - uses cl100k_base encoding (legacy)
    Gpt35Turbo,

    // =========================================================================
    // Anthropic Claude - Estimation (~3.5 chars/token)
    // =========================================================================
    /// Claude (all versions) - uses estimation based on ~3.5 chars/token
    Claude,

    // =========================================================================
    // Google Gemini - Estimation (~3.8 chars/token)
    // =========================================================================
    /// Gemini (all versions including 3, 2.5, 1.5) - estimation ~3.8 chars/token
    Gemini,

    // =========================================================================
    // Meta Llama - Estimation (~3.5 chars/token)
    // =========================================================================
    /// Llama 3/4 - estimation based on ~3.5 chars/token
    Llama,
    /// CodeLlama - more granular for code (~3.2 chars/token)
    CodeLlama,

    // =========================================================================
    // Mistral AI - Estimation (~3.5 chars/token)
    // =========================================================================
    /// Mistral (Large, Medium, Small, Codestral) - estimation ~3.5 chars/token
    Mistral,

    // =========================================================================
    // DeepSeek - Estimation (~3.5 chars/token)
    // =========================================================================
    /// DeepSeek (V3, R1, Coder) - estimation ~3.5 chars/token
    DeepSeek,

    // =========================================================================
    // Qwen (Alibaba) - Estimation (~3.5 chars/token)
    // =========================================================================
    /// Qwen (Qwen3, Qwen2.5) - estimation ~3.5 chars/token
    Qwen,

    // =========================================================================
    // Cohere - Estimation (~3.6 chars/token)
    // =========================================================================
    /// Cohere (Command R+, Command R) - estimation ~3.6 chars/token
    Cohere,

    // =========================================================================
    // xAI Grok - Estimation (~3.5 chars/token)
    // =========================================================================
    /// Grok (Grok 2, Grok 3) - estimation ~3.5 chars/token
    Grok,
}

impl TokenModel {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            // OpenAI o200k_base models
            Self::Gpt52 => "gpt-5.2",
            Self::Gpt52Pro => "gpt-5.2-pro",
            Self::Gpt51 => "gpt-5.1",
            Self::Gpt51Mini => "gpt-5.1-mini",
            Self::Gpt51Codex => "gpt-5.1-codex",
            Self::Gpt5 => "gpt-5",
            Self::Gpt5Mini => "gpt-5-mini",
            Self::Gpt5Nano => "gpt-5-nano",
            Self::O4Mini => "o4-mini",
            Self::O3 => "o3",
            Self::O3Mini => "o3-mini",
            Self::O1 => "o1",
            Self::O1Mini => "o1-mini",
            Self::O1Preview => "o1-preview",
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4oMini => "gpt-4o-mini",
            // OpenAI cl100k_base models (legacy)
            Self::Gpt4 => "gpt-4",
            Self::Gpt35Turbo => "gpt-3.5-turbo",
            // Other vendors
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Llama => "llama",
            Self::CodeLlama => "codellama",
            Self::Mistral => "mistral",
            Self::DeepSeek => "deepseek",
            Self::Qwen => "qwen",
            Self::Cohere => "cohere",
            Self::Grok => "grok",
        }
    }

    /// Get average characters per token (for estimation fallback)
    pub fn chars_per_token(&self) -> f32 {
        match self {
            // OpenAI o200k_base models - most efficient encoding (~4.0 chars/token)
            Self::Gpt52
            | Self::Gpt52Pro
            | Self::Gpt51
            | Self::Gpt51Mini
            | Self::Gpt51Codex
            | Self::Gpt5
            | Self::Gpt5Mini
            | Self::Gpt5Nano
            | Self::O4Mini
            | Self::O3
            | Self::O3Mini
            | Self::O1
            | Self::O1Mini
            | Self::O1Preview
            | Self::Gpt4o
            | Self::Gpt4oMini => 4.0,
            // OpenAI cl100k_base models (legacy) - slightly less efficient
            Self::Gpt4 | Self::Gpt35Turbo => 3.7,
            // Anthropic Claude
            Self::Claude => 3.5,
            // Google Gemini - slightly more verbose
            Self::Gemini => 3.8,
            // Meta Llama
            Self::Llama => 3.5,
            Self::CodeLlama => 3.2, // Code-focused, more granular
            // Mistral AI
            Self::Mistral => 3.5,
            // DeepSeek
            Self::DeepSeek => 3.5,
            // Qwen (Alibaba)
            Self::Qwen => 3.5,
            // Cohere - slightly more verbose
            Self::Cohere => 3.6,
            // xAI Grok
            Self::Grok => 3.5,
        }
    }

    /// Whether this model has an exact tokenizer available (via tiktoken)
    pub fn has_exact_tokenizer(&self) -> bool {
        matches!(
            self,
            // All OpenAI models have exact tokenizers
            Self::Gpt52
                | Self::Gpt52Pro
                | Self::Gpt51
                | Self::Gpt51Mini
                | Self::Gpt51Codex
                | Self::Gpt5
                | Self::Gpt5Mini
                | Self::Gpt5Nano
                | Self::O4Mini
                | Self::O3
                | Self::O3Mini
                | Self::O1
                | Self::O1Mini
                | Self::O1Preview
                | Self::Gpt4o
                | Self::Gpt4oMini
                | Self::Gpt4
                | Self::Gpt35Turbo
        )
    }

    /// Whether this model uses the o200k_base encoding
    pub fn uses_o200k(&self) -> bool {
        matches!(
            self,
            Self::Gpt52
                | Self::Gpt52Pro
                | Self::Gpt51
                | Self::Gpt51Mini
                | Self::Gpt51Codex
                | Self::Gpt5
                | Self::Gpt5Mini
                | Self::Gpt5Nano
                | Self::O4Mini
                | Self::O3
                | Self::O3Mini
                | Self::O1
                | Self::O1Mini
                | Self::O1Preview
                | Self::Gpt4o
                | Self::Gpt4oMini
        )
    }

    /// Whether this model uses the cl100k_base encoding (legacy)
    pub fn uses_cl100k(&self) -> bool {
        matches!(self, Self::Gpt4 | Self::Gpt35Turbo)
    }

    /// Parse a model name string into a TokenModel
    ///
    /// Supports various formats:
    /// - OpenAI: "gpt-5.2", "gpt-5.2-pro", "gpt-5.1", "gpt-5", "o3", "o1", "gpt-4o", etc.
    /// - Claude: "claude", "claude-3", "claude-4", "claude-opus", "claude-sonnet", "claude-haiku"
    /// - Gemini: "gemini", "gemini-pro", "gemini-flash", "gemini-2.5", "gemini-3"
    /// - Llama: "llama", "llama-3", "llama-4", "codellama"
    /// - Others: "mistral", "deepseek", "qwen", "cohere", "grok"
    ///
    /// # Examples
    ///
    /// ```
    /// use infiniloom_engine::tokenizer::TokenModel;
    ///
    /// assert_eq!(TokenModel::from_model_name("gpt-5.2"), Some(TokenModel::Gpt52));
    /// assert_eq!(TokenModel::from_model_name("o3"), Some(TokenModel::O3));
    /// assert_eq!(TokenModel::from_model_name("claude-sonnet"), Some(TokenModel::Claude));
    /// assert_eq!(TokenModel::from_model_name("unknown-model"), None);
    /// ```
    pub fn from_model_name(name: &str) -> Option<Self> {
        let name_lower = name.to_lowercase();
        let name_lower = name_lower.as_str();

        match name_lower {
            // =================================================================
            // OpenAI GPT-5.2 family
            // =================================================================
            "gpt-5.2" | "gpt5.2" | "gpt-52" | "gpt52" => Some(Self::Gpt52),
            "gpt-5.2-pro" | "gpt5.2-pro" | "gpt-52-pro" | "gpt52pro" => Some(Self::Gpt52Pro),
            s if s.starts_with("gpt-5.2-") || s.starts_with("gpt5.2-") => Some(Self::Gpt52),

            // =================================================================
            // OpenAI GPT-5.1 family
            // =================================================================
            "gpt-5.1" | "gpt5.1" | "gpt-51" | "gpt51" => Some(Self::Gpt51),
            "gpt-5.1-mini" | "gpt5.1-mini" | "gpt-51-mini" => Some(Self::Gpt51Mini),
            "gpt-5.1-codex" | "gpt5.1-codex" | "gpt-51-codex" => Some(Self::Gpt51Codex),
            s if s.starts_with("gpt-5.1-") || s.starts_with("gpt5.1-") => Some(Self::Gpt51),

            // =================================================================
            // OpenAI GPT-5 family
            // =================================================================
            "gpt-5" | "gpt5" => Some(Self::Gpt5),
            "gpt-5-mini" | "gpt5-mini" => Some(Self::Gpt5Mini),
            "gpt-5-nano" | "gpt5-nano" => Some(Self::Gpt5Nano),
            s if s.starts_with("gpt-5-") || s.starts_with("gpt5-") => Some(Self::Gpt5),

            // =================================================================
            // OpenAI O-series reasoning models
            // =================================================================
            "o4-mini" | "o4mini" => Some(Self::O4Mini),
            "o3" => Some(Self::O3),
            "o3-mini" | "o3mini" => Some(Self::O3Mini),
            s if s.starts_with("o3-") => Some(Self::O3),
            "o1" => Some(Self::O1),
            "o1-mini" | "o1mini" => Some(Self::O1Mini),
            "o1-preview" | "o1preview" => Some(Self::O1Preview),
            s if s.starts_with("o1-") => Some(Self::O1),

            // =================================================================
            // OpenAI GPT-4o family
            // =================================================================
            "gpt-4o" | "gpt4o" => Some(Self::Gpt4o),
            "gpt-4o-mini" | "gpt4o-mini" | "gpt-4o-mini-2024-07-18" => Some(Self::Gpt4oMini),
            s if s.starts_with("gpt-4o-") || s.starts_with("gpt4o-") => Some(Self::Gpt4o),

            // =================================================================
            // OpenAI GPT-4 family (legacy, cl100k_base)
            // =================================================================
            "gpt-4" | "gpt4" | "gpt-4-turbo" | "gpt4-turbo" | "gpt-4-turbo-preview" => {
                Some(Self::Gpt4)
            },
            s if s.starts_with("gpt-4-") && !s.contains("4o") => Some(Self::Gpt4),

            // =================================================================
            // OpenAI GPT-3.5 family (legacy, cl100k_base)
            // =================================================================
            "gpt-3.5-turbo" | "gpt-35-turbo" | "gpt3.5-turbo" | "gpt35-turbo" | "gpt-3.5" => {
                Some(Self::Gpt35Turbo)
            },
            s if s.starts_with("gpt-3.5-") || s.starts_with("gpt-35-") => Some(Self::Gpt35Turbo),

            // =================================================================
            // Anthropic Claude (all versions map to Claude)
            // =================================================================
            "claude" | "claude-3" | "claude-3.5" | "claude-4" | "claude-4.5" | "claude-opus"
            | "claude-opus-4" | "claude-opus-4.5" | "claude-sonnet" | "claude-sonnet-4"
            | "claude-sonnet-4.5" | "claude-haiku" | "claude-haiku-4" | "claude-haiku-4.5"
            | "claude-instant" => Some(Self::Claude),
            s if s.starts_with("claude") => Some(Self::Claude),

            // =================================================================
            // Google Gemini (all versions map to Gemini)
            // =================================================================
            "gemini" | "gemini-pro" | "gemini-flash" | "gemini-ultra" | "gemini-1.5"
            | "gemini-1.5-pro" | "gemini-1.5-flash" | "gemini-2" | "gemini-2.5"
            | "gemini-2.5-pro" | "gemini-2.5-flash" | "gemini-3" | "gemini-3-pro" => {
                Some(Self::Gemini)
            },
            s if s.starts_with("gemini") => Some(Self::Gemini),

            // =================================================================
            // Meta Llama
            // =================================================================
            "llama" | "llama-2" | "llama-3" | "llama-3.1" | "llama-3.2" | "llama-4" | "llama2"
            | "llama3" | "llama4" => Some(Self::Llama),
            "codellama" | "code-llama" | "llama-code" => Some(Self::CodeLlama),
            s if s.starts_with("llama") && !s.contains("code") => Some(Self::Llama),
            s if s.contains("codellama") || s.contains("code-llama") => Some(Self::CodeLlama),

            // =================================================================
            // Mistral AI
            // =================================================================
            "mistral" | "mistral-large" | "mistral-large-3" | "mistral-medium"
            | "mistral-medium-3" | "mistral-small" | "mistral-small-3" | "codestral"
            | "devstral" | "ministral" => Some(Self::Mistral),
            s if s.starts_with("mistral") || s.contains("stral") => Some(Self::Mistral),

            // =================================================================
            // DeepSeek
            // =================================================================
            "deepseek" | "deepseek-v3" | "deepseek-v3.2" | "deepseek-r1" | "deepseek-coder"
            | "deepseek-chat" | "deepseek-reasoner" => Some(Self::DeepSeek),
            s if s.starts_with("deepseek") => Some(Self::DeepSeek),

            // =================================================================
            // Qwen (Alibaba)
            // =================================================================
            "qwen" | "qwen2" | "qwen2.5" | "qwen3" | "qwen-72b" | "qwen-7b" | "qwen-coder" => {
                Some(Self::Qwen)
            },
            s if s.starts_with("qwen") => Some(Self::Qwen),

            // =================================================================
            // Cohere
            // =================================================================
            "cohere" | "command-r" | "command-r-plus" | "command-r+" | "command" => {
                Some(Self::Cohere)
            },
            s if s.starts_with("cohere") || s.starts_with("command") => Some(Self::Cohere),

            // =================================================================
            // xAI Grok
            // =================================================================
            "grok" | "grok-1" | "grok-2" | "grok-3" | "grok-beta" => Some(Self::Grok),
            s if s.starts_with("grok") => Some(Self::Grok),

            // Unknown model
            _ => None,
        }
    }

    /// Get all available models
    pub fn all() -> &'static [Self] {
        &[
            Self::Gpt52,
            Self::Gpt52Pro,
            Self::Gpt51,
            Self::Gpt51Mini,
            Self::Gpt51Codex,
            Self::Gpt5,
            Self::Gpt5Mini,
            Self::Gpt5Nano,
            Self::O4Mini,
            Self::O3,
            Self::O3Mini,
            Self::O1,
            Self::O1Mini,
            Self::O1Preview,
            Self::Gpt4o,
            Self::Gpt4oMini,
            Self::Gpt4,
            Self::Gpt35Turbo,
            Self::Claude,
            Self::Gemini,
            Self::Llama,
            Self::CodeLlama,
            Self::Mistral,
            Self::DeepSeek,
            Self::Qwen,
            Self::Cohere,
            Self::Grok,
        ]
    }

    /// Get the vendor/provider name for this model
    pub fn vendor(&self) -> &'static str {
        match self {
            Self::Gpt52
            | Self::Gpt52Pro
            | Self::Gpt51
            | Self::Gpt51Mini
            | Self::Gpt51Codex
            | Self::Gpt5
            | Self::Gpt5Mini
            | Self::Gpt5Nano
            | Self::O4Mini
            | Self::O3
            | Self::O3Mini
            | Self::O1
            | Self::O1Mini
            | Self::O1Preview
            | Self::Gpt4o
            | Self::Gpt4oMini
            | Self::Gpt4
            | Self::Gpt35Turbo => "OpenAI",
            Self::Claude => "Anthropic",
            Self::Gemini => "Google",
            Self::Llama | Self::CodeLlama => "Meta",
            Self::Mistral => "Mistral AI",
            Self::DeepSeek => "DeepSeek",
            Self::Qwen => "Alibaba",
            Self::Cohere => "Cohere",
            Self::Grok => "xAI",
        }
    }
}

/// Global tokenizer instances (lazy initialized, thread-safe)
static GPT4O_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
static GPT4_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Global token count cache - keyed by (content_hash, model)
/// This provides significant speedup when the same content is tokenized multiple times.
static TOKEN_CACHE: OnceLock<DashMap<(u64, TokenModel), u32>> = OnceLock::new();

/// Get or initialize the global token cache
fn get_token_cache() -> &'static DashMap<(u64, TokenModel), u32> {
    TOKEN_CACHE.get_or_init(DashMap::new)
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

            // Compute and cache
            let count = self.count_uncached(text, model);
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

    /// Count tokens using exact BPE encoding
    fn count_exact(&self, text: &str, model: TokenModel) -> u32 {
        if model.uses_o200k() {
            // All modern OpenAI models use o200k_base encoding
            // GPT-5.x, GPT-4o, O1, O3, O4
            let tokenizer = get_gpt4o_tokenizer();
            tokenizer.encode_ordinary(text).len() as u32
        } else if model.uses_cl100k() {
            // Legacy OpenAI models use cl100k_base encoding
            // GPT-4, GPT-3.5-turbo
            let tokenizer = get_gpt4_tokenizer();
            tokenizer.encode_ordinary(text).len() as u32
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

/// Token counts for multiple models
///
/// Counts are grouped by tokenizer encoding family:
/// - `o200k`: OpenAI modern models (GPT-5.x, GPT-4o, O1/O3/O4) - EXACT
/// - `cl100k`: OpenAI legacy models (GPT-4, GPT-3.5-turbo) - EXACT
/// - Other fields: Estimation-based counts for non-OpenAI vendors
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenCounts {
    /// OpenAI o200k_base encoding (GPT-5.2, GPT-5.1, GPT-5, GPT-4o, O1, O3, O4)
    pub o200k: u32,
    /// OpenAI cl100k_base encoding (GPT-4, GPT-3.5-turbo) - legacy
    pub cl100k: u32,
    /// Anthropic Claude (all versions)
    pub claude: u32,
    /// Google Gemini (all versions)
    pub gemini: u32,
    /// Meta Llama (3, 4, CodeLlama)
    pub llama: u32,
    /// Mistral AI (Large, Medium, Small, Codestral)
    pub mistral: u32,
    /// DeepSeek (V3, R1, Coder)
    pub deepseek: u32,
    /// Alibaba Qwen (Qwen3, Qwen2.5)
    pub qwen: u32,
    /// Cohere (Command R+, Command R)
    pub cohere: u32,
    /// xAI Grok (Grok 2, Grok 3)
    pub grok: u32,
}

impl TokenCounts {
    /// Create zero counts
    pub fn zero() -> Self {
        Self::default()
    }

    /// Get count for a specific model
    pub fn get(&self, model: TokenModel) -> u32 {
        match model {
            // OpenAI o200k_base models (all share same encoding)
            TokenModel::Gpt52
            | TokenModel::Gpt52Pro
            | TokenModel::Gpt51
            | TokenModel::Gpt51Mini
            | TokenModel::Gpt51Codex
            | TokenModel::Gpt5
            | TokenModel::Gpt5Mini
            | TokenModel::Gpt5Nano
            | TokenModel::O4Mini
            | TokenModel::O3
            | TokenModel::O3Mini
            | TokenModel::O1
            | TokenModel::O1Mini
            | TokenModel::O1Preview
            | TokenModel::Gpt4o
            | TokenModel::Gpt4oMini => self.o200k,
            // OpenAI cl100k_base models (legacy, same encoding)
            TokenModel::Gpt4 | TokenModel::Gpt35Turbo => self.cl100k,
            // Other vendors
            TokenModel::Claude => self.claude,
            TokenModel::Gemini => self.gemini,
            TokenModel::Llama | TokenModel::CodeLlama => self.llama,
            TokenModel::Mistral => self.mistral,
            TokenModel::DeepSeek => self.deepseek,
            TokenModel::Qwen => self.qwen,
            TokenModel::Cohere => self.cohere,
            TokenModel::Grok => self.grok,
        }
    }

    /// Set count for a specific model
    pub fn set(&mut self, model: TokenModel, count: u32) {
        match model {
            // OpenAI o200k_base models
            TokenModel::Gpt52
            | TokenModel::Gpt52Pro
            | TokenModel::Gpt51
            | TokenModel::Gpt51Mini
            | TokenModel::Gpt51Codex
            | TokenModel::Gpt5
            | TokenModel::Gpt5Mini
            | TokenModel::Gpt5Nano
            | TokenModel::O4Mini
            | TokenModel::O3
            | TokenModel::O3Mini
            | TokenModel::O1
            | TokenModel::O1Mini
            | TokenModel::O1Preview
            | TokenModel::Gpt4o
            | TokenModel::Gpt4oMini => self.o200k = count,
            // OpenAI cl100k_base models (legacy)
            TokenModel::Gpt4 | TokenModel::Gpt35Turbo => self.cl100k = count,
            // Other vendors
            TokenModel::Claude => self.claude = count,
            TokenModel::Gemini => self.gemini = count,
            TokenModel::Llama | TokenModel::CodeLlama => self.llama = count,
            TokenModel::Mistral => self.mistral = count,
            TokenModel::DeepSeek => self.deepseek = count,
            TokenModel::Qwen => self.qwen = count,
            TokenModel::Cohere => self.cohere = count,
            TokenModel::Grok => self.grok = count,
        }
    }

    /// Sum all counts (useful for aggregate statistics)
    pub fn total(&self) -> u64 {
        self.o200k as u64
            + self.cl100k as u64
            + self.claude as u64
            + self.gemini as u64
            + self.llama as u64
            + self.mistral as u64
            + self.deepseek as u64
            + self.qwen as u64
            + self.cohere as u64
            + self.grok as u64
    }

    /// Add counts from another TokenCounts
    pub fn add(&mut self, other: &TokenCounts) {
        self.o200k += other.o200k;
        self.cl100k += other.cl100k;
        self.claude += other.claude;
        self.gemini += other.gemini;
        self.llama += other.llama;
        self.mistral += other.mistral;
        self.deepseek += other.deepseek;
        self.qwen += other.qwen;
        self.cohere += other.cohere;
        self.grok += other.grok;
    }

    /// Get the minimum token count across all models
    pub fn min(&self) -> u32 {
        [
            self.o200k,
            self.cl100k,
            self.claude,
            self.gemini,
            self.llama,
            self.mistral,
            self.deepseek,
            self.qwen,
            self.cohere,
            self.grok,
        ]
        .into_iter()
        .min()
        .unwrap_or(0)
    }

    /// Get the maximum token count across all models
    pub fn max(&self) -> u32 {
        [
            self.o200k,
            self.cl100k,
            self.claude,
            self.gemini,
            self.llama,
            self.mistral,
            self.deepseek,
            self.qwen,
            self.cohere,
            self.grok,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

impl std::ops::Add for TokenCounts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            o200k: self.o200k + rhs.o200k,
            cl100k: self.cl100k + rhs.cl100k,
            claude: self.claude + rhs.claude,
            gemini: self.gemini + rhs.gemini,
            llama: self.llama + rhs.llama,
            mistral: self.mistral + rhs.mistral,
            deepseek: self.deepseek + rhs.deepseek,
            qwen: self.qwen + rhs.qwen,
            cohere: self.cohere + rhs.cohere,
            grok: self.grok + rhs.grok,
        }
    }
}

impl std::iter::Sum for TokenCounts {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
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
