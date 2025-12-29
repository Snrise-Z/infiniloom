//! Diff command handler
//!
//! Gets context for git diffs including changed files, dependents, and tests.
//!
//! This module is organized into several submodules:
//! - `git_ops`: Git operations (diff, status, history)
//! - `formatting`: Output formatters for multiple formats
//! - `context`: Context enrichment and token budget management

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;

use infiniloom_engine::git::GitRepo;
use infiniloom_engine::index::{
    BuildOptions, ContextDepth, ContextExpander, IndexBuilder, IndexStorage, LazyContextBuilder,
};
use infiniloom_engine::output::OutputFormat;
use infiniloom_engine::types::TokenizerModel;

use crate::config::load_config_file;

// Submodules
pub mod context;
pub mod formatting;
pub mod git_ops;

#[cfg(test)]
mod tests;

// Re-export commonly used functions and types
pub use context::{apply_diff_budget, enrich_diff_context};
pub use formatting::{format_diff_context, FileHistory};
pub use git_ops::{check_git_available, get_diff_changes, read_file_from_git, resolve_base_ref};

/// Get context for a diff
#[allow(clippy::too_many_arguments)]
pub fn cmd_diff(
    mut path: PathBuf,
    mut reference: Option<String>,
    staged: bool,
    depth: u8,
    budget: u32,
    format: OutputFormat,
    output: Option<PathBuf>,
    include_diff: bool,
    cli_model: Option<TokenizerModel>,
    include_history: bool,
    history_count: usize,
    verbose: bool,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
) -> Result<()> {
    // Check git is available
    check_git_available()?;

    if reference.is_none() && !path.exists() {
        reference = Some(path.to_string_lossy().to_string());
        path = PathBuf::from(".");
    }

    let storage = IndexStorage::new(&path);
    let loaded_config = load_config_file(None, &path);

    let model: TokenizerModel = if let Some(m) = cli_model {
        m
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let base_ref = resolve_base_ref(reference.as_deref(), &path);

    // Always load diff content for accurate change classification
    let mut changes = get_diff_changes(&path, reference.as_deref(), staged, true)?;

    // Apply centralized filtering
    infiniloom_engine::filtering::apply_exclude_patterns(&mut changes, &exclude, |c| &c.file_path);
    infiniloom_engine::filtering::apply_include_patterns(&mut changes, &include_patterns, |c| {
        &c.file_path
    });

    // Exclude test files unless include_tests is true
    if !include_tests {
        use infiniloom_engine::default_ignores::{matches_any, TEST_IGNORES};
        changes.retain(|c| !matches_any(&c.file_path, TEST_IGNORES));
    }

    if changes.is_empty() {
        println!("No changes detected.");
        return Ok(());
    }

    if verbose {
        eprintln!("{} Analyzing {} changed files...", "→".cyan(), changes.len());
    }

    // Convert depth
    let context_depth = match depth {
        1 => ContextDepth::L1,
        2 => ContextDepth::L2,
        _ => ContextDepth::L3,
    };

    let expand_budget = u32::MAX / 2;

    // Try to use pre-built index, fall back to in-memory rebuild or lazy indexing
    let mut context = if storage.exists() {
        let mut use_prebuilt = true;
        if let Ok(meta) = storage.load_meta() {
            if !git_ops::is_index_fresh(&path, &meta) {
                use_prebuilt = false;
            }
        }

        if use_prebuilt {
            let (index, graph) = storage.load_all().context("Failed to load index")?;
            let expander = ContextExpander::new(&index, &graph);
            expander.expand(&changes, context_depth, expand_budget)
        } else {
            eprintln!(
                "{} Index is stale; rebuilding in memory for accurate context...",
                "→".yellow()
            );
            let builder = IndexBuilder::new(&path)
                .with_options(BuildOptions { respect_gitignore: true, ..Default::default() });
            match builder.build() {
                Ok((index, graph)) => {
                    let expander = ContextExpander::new(&index, &graph);
                    expander.expand(&changes, context_depth, expand_budget)
                },
                Err(e) => {
                    eprintln!(
                        "{} Index rebuild failed ({}). Falling back to lazy indexing...",
                        "⚠".yellow(),
                        e
                    );
                    let mut builder = LazyContextBuilder::new(&path);
                    builder
                        .generate_context(&changes, context_depth, expand_budget)
                        .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
                },
            }
        }
    } else {
        // Lazy path: build minimal index on-the-fly
        eprintln!("{} No pre-built index found, using lazy indexing...", "→".yellow());
        let mut builder = LazyContextBuilder::new(&path);
        builder
            .generate_context(&changes, context_depth, expand_budget)
            .map_err(|e| anyhow::anyhow!("Lazy indexing failed: {}", e))?
    };

    if !include_diff {
        for file in &mut context.changed_files {
            file.diff_content = None;
        }
    }

    enrich_diff_context(&path, &changes, base_ref.as_deref(), &mut context, model)?;
    apply_diff_budget(&mut context, budget, model);

    // Fetch commit history for changed files if requested
    let file_history: FileHistory = if include_history && history_count > 0 {
        let mut history_map = HashMap::new();
        if let Ok(repo) = GitRepo::open(&path) {
            for file in &context.changed_files {
                if let Ok(commits) = repo.file_log(&file.path, history_count) {
                    if !commits.is_empty() {
                        history_map.insert(file.path.clone(), commits);
                    }
                }
            }
        }
        history_map
    } else {
        HashMap::new()
    };

    // Format output
    let output_text = format_diff_context(&context, format, &file_history);

    // Write output
    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        println!("Context written to {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    // Print summary
    eprintln!();
    eprintln!(
        "{} Impact: {} ({} files, {} symbols, {} tests)",
        "→".cyan(),
        context.impact_summary.level.name(),
        context.impact_summary.direct_files + context.impact_summary.transitive_files,
        context.impact_summary.affected_symbols,
        context.impact_summary.affected_tests
    );
    eprintln!("  Tokens: ~{}", context.total_tokens);

    Ok(())
}
