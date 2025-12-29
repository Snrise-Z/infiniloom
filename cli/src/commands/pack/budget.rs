//! Token budget management and file ranking for pack command
//!
//! This module provides functions for:
//! - Token counting and estimation
//! - Token budget enforcement
//! - Fast file ranking by importance
//! - Repository metadata recalculation
//! - Incremental cache management

use infiniloom_engine::{
    tokenizer::{TokenModel, Tokenizer},
    types::{LanguageStats, TokenCounts, TokenizerModel},
};
use std::collections::HashMap;

/// Estimate token count for text using specified model
///
/// # Arguments
///
/// * `text` - Text to count tokens for
/// * `model` - Tokenizer model to use
///
/// # Returns
///
/// Estimated token count.
pub fn estimate_tokens(text: &str, model: TokenizerModel) -> usize {
    let tokenizer = Tokenizer::new();
    tokenizer.count(text, model) as usize
}

/// Truncate text to fit within token budget
///
/// Attempts to truncate at logical boundaries (file end tags, code blocks, etc.).
///
/// # Arguments
///
/// * `text` - Text to truncate
/// * `max_tokens` - Maximum token count
/// * `model` - Tokenizer model to use
///
/// # Returns
///
/// Truncated text with notice appended if truncation occurred.
pub fn truncate_to_tokens(text: &str, max_tokens: usize, model: TokenizerModel) -> String {
    let tokenizer = Tokenizer::new();
    let current = tokenizer.count(text, model) as usize;

    if current <= max_tokens {
        return text.to_owned();
    }

    let truncated = tokenizer.truncate_to_budget(text, model, max_tokens as u32);

    // Try to find a logical boundary to truncate at
    let markers = ["</file>", "```\n\n", "----------------------------------------\n", "\n---\n"];
    let mut best_end = truncated.len();

    for marker in markers {
        if let Some(pos) = truncated.rfind(marker) {
            let end_pos = pos + marker.len();
            // Only use this boundary if it's in the latter half
            if end_pos > truncated.len() / 2 {
                best_end = end_pos;
                break;
            }
        }
    }

    let mut result = truncated[..best_end].to_string();
    result.push_str("\n\n<!-- Output truncated to fit token limit -->\n");
    result
}

/// Fast heuristic-based file ranking
///
/// Ranks files by importance using pattern matching:
/// - Entry points (main.*, index.*, lib.*, etc.) ranked highest
/// - Config files (Cargo.toml, package.json, etc.) ranked high
/// - Source directories (src/, lib/, core/) ranked moderately high
/// - Test files, docs, vendor files ranked lower
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository to rank
pub fn rank_files_fast(repo: &mut infiniloom_engine::Repository) {
    repo.files.sort_by_key(|f| {
        let path = &f.relative_path;
        let mut score: i32 = 1000;

        // Entry points (highest priority)
        let entry_point_patterns = [
            "main.rs",
            "main.go",
            "main.py",
            "main.ts",
            "main.js",
            "main.c",
            "main.cpp",
            "index.ts",
            "index.js",
            "index.tsx",
            "index.jsx",
            "index.py",
            "app.py",
            "app.ts",
            "app.js",
            "app.tsx",
            "app.jsx",
            "app.go",
            "server.py",
            "server.ts",
            "server.js",
            "server.go",
            "mod.rs",
            "lib.rs",
            "lib.py",
            "__main__.py",
            "__init__.py",
        ];
        if entry_point_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 5000;
        }

        // Config files (high priority)
        let config_patterns = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "Gemfile",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tsconfig.json",
            "webpack.config.js",
            "vite.config.ts",
            ".eslintrc",
            "Makefile",
            "CMakeLists.txt",
            "docker-compose.yml",
            "Dockerfile",
        ];
        if config_patterns.iter().any(|p| path.ends_with(p)) {
            score -= 3000;
        }

        // Source directories (moderate priority)
        if path.contains("/src/") || path.starts_with("src/") {
            score -= 1000;
        }
        if path.contains("/lib/") || path.contains("/core/") {
            score -= 800;
        }
        if path.contains("/api/") || path.contains("/handlers/") || path.contains("/routes/") {
            score -= 600;
        }

        // Low priority files
        if path.contains("/test") || path.contains("_test.") || path.contains(".test.") {
            score += 2000;
        }
        if path.contains("/examples/") || path.contains("/docs/") || path.ends_with(".md") {
            score += 1500;
        }
        if path.contains("/vendor/") || path.contains("/node_modules/") {
            score += 3000;
        }

        score
    });
}

/// Recalculate repository metadata after modifications
///
/// Updates:
/// - Total file count
/// - Total line count
/// - Total token counts (all models)
/// - Language statistics
/// - Directory structure
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
pub fn recalculate_metadata(repo: &mut infiniloom_engine::types::Repository) {
    // Update file count
    repo.metadata.total_files = repo.files.len() as u32;

    // Update line count
    repo.metadata.total_lines = repo
        .files
        .iter()
        .map(|f| {
            f.content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| f.size_bytes / 40)
        })
        .sum();

    // Update token counts across all models
    repo.metadata.total_tokens = TokenCounts {
        o200k: repo.files.iter().map(|f| f.token_count.o200k).sum(),
        cl100k: repo.files.iter().map(|f| f.token_count.cl100k).sum(),
        claude: repo.files.iter().map(|f| f.token_count.claude).sum(),
        gemini: repo.files.iter().map(|f| f.token_count.gemini).sum(),
        llama: repo.files.iter().map(|f| f.token_count.llama).sum(),
        mistral: repo.files.iter().map(|f| f.token_count.mistral).sum(),
        deepseek: repo.files.iter().map(|f| f.token_count.deepseek).sum(),
        qwen: repo.files.iter().map(|f| f.token_count.qwen).sum(),
        cohere: repo.files.iter().map(|f| f.token_count.cohere).sum(),
        grok: repo.files.iter().map(|f| f.token_count.grok).sum(),
    };

    // Update language statistics
    let mut language_counts: HashMap<String, u32> = HashMap::new();
    let mut language_lines: HashMap<String, u64> = HashMap::new();

    for file in &repo.files {
        if let Some(ref lang) = file.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
            let lines = file
                .content
                .as_ref()
                .map(|c| c.lines().count() as u64)
                .unwrap_or_else(|| file.size_bytes / 40);
            *language_lines.entry(lang.clone()).or_insert(0) += lines;
        }
    }

    let total_files = repo.metadata.total_files;
    let mut languages: Vec<LanguageStats> = language_counts
        .into_iter()
        .map(|(lang, count)| {
            let lines = language_lines.get(&lang).copied().unwrap_or(0);
            let percentage = if total_files > 0 {
                (count as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            LanguageStats { language: lang, files: count, lines, percentage }
        })
        .collect();

    languages.sort_by(|a, b| b.files.cmp(&a.files));
    repo.metadata.languages = languages;

    // Update directory structure
    let mut paths: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    paths.sort();
    repo.metadata.directory_structure = Some(paths.join("\n"));
}

/// Update incremental cache with repository files
///
/// Updates cache entries for all current files and removes deleted files.
///
/// # Arguments
///
/// * `cache` - Mutable reference to repository cache
/// * `repo` - Repository to cache
/// * `symbols_extracted` - Whether symbols were extracted for files
pub fn update_repo_cache(
    cache: &mut infiniloom_engine::RepoCache,
    repo: &infiniloom_engine::Repository,
    symbols_extracted: bool,
) {
    use infiniloom_engine::incremental::hash_content;

    // Update cache entries for current files
    for file in &repo.files {
        let mtime = std::fs::metadata(&file.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let content_hash = file
            .content
            .as_ref()
            .map(|c| hash_content(c.as_bytes()))
            .unwrap_or(0);

        let cached = cache.get(&file.relative_path);
        let changed = cached.map_or(true, |_| {
            if content_hash != 0 {
                cache.needs_rescan_with_hash(
                    &file.relative_path,
                    mtime,
                    file.size_bytes,
                    content_hash,
                )
            } else {
                cache.needs_rescan(&file.relative_path, mtime, file.size_bytes)
            }
        });

        let symbols_extracted_for_file = if symbols_extracted {
            true
        } else if !changed {
            cached.map(|c| c.symbols_extracted).unwrap_or(false)
        } else {
            false
        };

        cache.update_file(infiniloom_engine::CachedFile {
            path: file.relative_path.clone(),
            mtime,
            size: file.size_bytes,
            hash: content_hash,
            tokens: infiniloom_engine::AccurateTokenCounts {
                o200k: file.token_count.o200k,
                cl100k: file.token_count.cl100k,
                claude: file.token_count.claude,
                gemini: file.token_count.gemini,
                llama: file.token_count.llama,
                mistral: file.token_count.mistral,
                deepseek: file.token_count.deepseek,
                qwen: file.token_count.qwen,
                cohere: file.token_count.cohere,
                grok: file.token_count.grok,
            },
            symbols: file
                .symbols
                .iter()
                .map(infiniloom_engine::CachedSymbol::from)
                .collect(),
            symbols_extracted: symbols_extracted_for_file,
            language: file.language.clone(),
            lines: file
                .content
                .as_ref()
                .map(|c| c.lines().count())
                .unwrap_or(0),
        });
    }

    // Remove deleted files from cache
    let current_files: Vec<&str> = repo
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    for deleted in cache.find_deleted_files(&current_files) {
        cache.remove_file(&deleted);
    }

    cache.recalculate_totals();
}

/// Convert TokenizerModel to TokenModel for budget enforcement
///
/// Maps the 27 TokenizerModel variants to the 10 TokenModel families used by the tokenizer.
pub fn budget_token_model_for(model: TokenizerModel) -> TokenModel {
    match model {
        TokenizerModel::Claude => TokenModel::Claude,
        TokenizerModel::Gpt52
        | TokenizerModel::Gpt52Pro
        | TokenizerModel::Gpt51
        | TokenizerModel::Gpt51Mini
        | TokenizerModel::Gpt51Codex
        | TokenizerModel::Gpt5
        | TokenizerModel::Gpt5Mini
        | TokenizerModel::Gpt5Nano
        | TokenizerModel::O4Mini
        | TokenizerModel::O3
        | TokenizerModel::O3Mini
        | TokenizerModel::O1
        | TokenizerModel::O1Mini
        | TokenizerModel::O1Preview
        | TokenizerModel::Gpt4o
        | TokenizerModel::Gpt4oMini => TokenModel::Gpt4o,
        TokenizerModel::Gpt4 | TokenizerModel::Gpt35Turbo => TokenModel::Gpt4,
        TokenizerModel::Gemini => TokenModel::Gemini,
        TokenizerModel::Llama | TokenizerModel::CodeLlama => TokenModel::Llama,
        TokenizerModel::Mistral => TokenModel::Mistral,
        TokenizerModel::DeepSeek => TokenModel::DeepSeek,
        TokenizerModel::Qwen => TokenModel::Qwen,
        TokenizerModel::Cohere => TokenModel::Cohere,
        TokenizerModel::Grok => TokenModel::Grok,
    }
}

/// Enforce token budget on repository
///
/// Truncates file content to fit within specified token budget using importance-based prioritization.
///
/// # Arguments
///
/// * `repo` - Mutable reference to repository
/// * `max_tokens` - Maximum token budget (0 = no limit)
/// * `model` - Tokenizer model to use for counting
///
/// # Returns
///
/// Returns `Some(EnforcementResult)` if budget was enforced, `None` if no limit.
pub fn enforce_budget(
    repo: &mut infiniloom_engine::Repository,
    max_tokens: u32,
    model: TokenizerModel,
) -> Option<infiniloom_engine::budget::EnforcementResult> {
    if max_tokens == 0 {
        return None;
    }

    use infiniloom_engine::budget::{BudgetConfig, BudgetEnforcer, TruncationStrategy};
    use infiniloom_engine::TokenCount;

    let config = BudgetConfig {
        budget: TokenCount::new(max_tokens),
        model: budget_token_model_for(model),
        strategy: TruncationStrategy::Line,
        overhead_reserve: TokenCount::new(2000),
    };
    let enforcer = BudgetEnforcer::new(config);
    let result = enforcer.enforce(repo);

    // Recalculate metadata after truncation
    recalculate_metadata(repo);

    Some(result)
}
