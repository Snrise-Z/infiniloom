//! Token count aggregation for multiple models
//!
//! This module provides the TokenCounts struct for storing token counts
//! across multiple LLM model families.

use super::models::TokenModel;

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

    /// Create counts with all fields set to the same value
    /// Useful for testing
    #[cfg(test)]
    pub fn default_with_value(value: u32) -> Self {
        Self {
            o200k: value,
            cl100k: value,
            claude: value,
            gemini: value,
            llama: value,
            mistral: value,
            deepseek: value,
            qwen: value,
            cohere: value,
            grok: value,
        }
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
