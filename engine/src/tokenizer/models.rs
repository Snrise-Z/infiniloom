//! Token model definitions for various LLM providers
//!
//! This module defines the supported LLM models and their tokenizer properties.

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
