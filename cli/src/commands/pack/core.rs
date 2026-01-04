//! Core pack command implementation
//!
//! This module contains the main pack command logic, refactored to use
//! PackConfig instead of individual parameters.

use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::time::Instant;

use infiniloom_engine::{
    output::OutputFormat,
    remote::RemoteRepo,
    types::{CompressionLevel, TokenizerModel},
};

use super::PackConfig;
use crate::config::load_config_file;
use crate::watch::{run_watch_mode, WatchConfig};

/// Execute the pack command with the given configuration
///
/// This is the main entry point for packing a repository into LLM-friendly format.
///
/// # Arguments
///
/// * `config` - Pack configuration with all options
///
/// # Errors
///
/// Returns error if:
/// - Repository cannot be scanned
/// - Output cannot be generated
/// - Files cannot be written
pub(super) fn execute(config: PackConfig) -> Result<()> {
    let start = Instant::now();

    // Handle stdin mode - read file paths from stdin
    let stdin_paths: Option<Vec<String>> = if config.scan.stdin {
        let stdin_handle = io::stdin();
        let paths: Vec<String> = stdin_handle
            .lock()
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    } else {
        None
    };

    if config.verbose {
        eprintln!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
        eprintln!();
    }

    // Create progress bar
    let pb = if config.verbose {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("Scanning repository...");
        Some(pb)
    } else {
        None
    };

    // Handle remote URL - clone if needed
    let (repo_path, _temp_dir) =
        if RemoteRepo::is_remote_url(config.path.to_string_lossy().as_ref()) {
            handle_remote_repository(&config, &pb)?
        } else {
            (config.path.clone(), None)
        };

    // Load and merge configuration from file
    let merged_config = merge_with_file_config(config, &repo_path)?;

    // Handle watch mode
    if merged_config.watch.enabled {
        return handle_watch_mode(merged_config, repo_path);
    }

    // Execute normal pack mode
    execute_pack(&merged_config, &repo_path, stdin_paths, pb, start)
}

/// Handle remote repository cloning
fn handle_remote_repository(
    config: &PackConfig,
    pb: &Option<ProgressBar>,
) -> Result<(PathBuf, Option<tempfile::TempDir>)> {
    if let Some(pb) = pb {
        pb.set_message("Cloning remote repository...");
    }

    let mut remote = RemoteRepo::parse(config.path.to_string_lossy().as_ref())
        .map_err(|e| anyhow::anyhow!("Invalid remote URL: {}", e))?;

    if let Some(ref branch) = config.git.remote_branch {
        remote.branch = Some(branch.clone());
    }

    if config.verbose {
        let branch_info = remote.branch.as_deref().unwrap_or("default");
        if config.git.sparse_paths.is_empty() {
            eprintln!(
                "  Cloning {} from {:?} (branch: {})...",
                remote.name, remote.provider, branch_info
            );
        } else {
            eprintln!(
                "  Sparse cloning {} from {:?} (branch: {}, paths: {:?})...",
                remote.name, remote.provider, branch_info, config.git.sparse_paths
            );
        }
    }

    let (cloned_path, temp_dir) = if !config.git.sparse_paths.is_empty() {
        let temp_dir = tempfile::TempDir::with_prefix("infiniloom-sparse-")
            .map_err(|e| anyhow::anyhow!("Failed to create temp dir: {}", e))?;
        let paths_refs: Vec<&str> = config.git.sparse_paths.iter().map(|s| s.as_str()).collect();
        let cloned = remote
            .sparse_clone(&paths_refs, Some(temp_dir.path()))
            .map_err(|e| anyhow::anyhow!("Failed to sparse clone repository: {}", e))?;
        (cloned, temp_dir)
    } else {
        remote
            .clone_with_cleanup()
            .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?
    };

    let uses_history_features =
        config.git.include_logs || config.git.sort_by_changes || config.git.include_diffs;
    if uses_history_features {
        eprintln!(
            "{} Remote repositories are cloned with --depth 1 (shallow clone).",
            "Warning:".yellow().bold()
        );
        eprintln!("         History-dependent options may return incomplete results.");
        eprintln!();
    }

    Ok((cloned_path, Some(temp_dir)))
}

/// Merge CLI config with file-based configuration
fn merge_with_file_config(mut config: PackConfig, repo_path: &Path) -> Result<PackConfig> {
    let loaded_config = load_config_file(config.config_path.as_ref(), repo_path);

    // Apply config file defaults (CLI values take precedence)
    if !config.scan.include_tests {
        config.scan.include_tests = loaded_config.include_tests.unwrap_or(false);
    }
    if !config.scan.include_docs {
        config.scan.include_docs = loaded_config.include_docs.unwrap_or(false);
    }
    if !config.security.security_check {
        config.security.security_check = loaded_config.security_check.unwrap_or(false);
    }
    if !config.scan.include_hidden {
        config.scan.include_hidden = loaded_config.include_hidden.unwrap_or(false);
    }

    // Apply boolean flags from config file
    if let Some(ln) = loaded_config.line_numbers {
        if !config.output.show_line_numbers {
            config.output.show_line_numbers = ln;
        }
    }
    if let Some(ds) = loaded_config.show_directory_structure {
        if !config.output.show_directory_structure {
            config.output.show_directory_structure = ds;
        }
    }
    if let Some(fs) = loaded_config.show_file_summary {
        if !config.output.show_file_summary {
            config.output.show_file_summary = fs;
        }
    }
    if !config.scan.remove_empty_lines {
        config.scan.remove_empty_lines = loaded_config.remove_empty_lines.unwrap_or(false);
    }
    if !config.scan.remove_comments {
        config.scan.remove_comments = loaded_config.remove_comments.unwrap_or(false);
    }

    // Apply token budget from config file if not set
    if config.output.max_tokens == 0 {
        if let Some(budget) = loaded_config.token_budget {
            config.output.max_tokens = budget;
        }
    }

    // Resolve format/model/compression with config file defaults
    if config.output.format.is_none() {
        if let Some(ref fmt_str) = loaded_config.format {
            config.output.format = Some(parse_output_format(fmt_str));
        } else {
            config.output.format = Some(OutputFormat::Xml);
        }
    }

    if config.output.model.is_none() {
        if let Some(ref model_str) = loaded_config.model {
            config.output.model =
                Some(TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude));
        } else {
            config.output.model = Some(TokenizerModel::Claude);
        }
    }

    if config.output.compression.is_none() {
        if let Some(ref comp_str) = loaded_config.compression {
            config.output.compression =
                Some(CompressionLevel::from_str(comp_str).unwrap_or(CompressionLevel::Balanced));
        } else {
            config.output.compression = Some(CompressionLevel::Balanced);
        }
    }

    Ok(config)
}

/// Parse output format from string
fn parse_output_format(fmt_str: &str) -> OutputFormat {
    match fmt_str.to_lowercase().as_str() {
        "markdown" | "md" => OutputFormat::Markdown,
        "json" => OutputFormat::Json,
        "yaml" | "yml" => OutputFormat::Yaml,
        "plain" | "text" | "txt" => OutputFormat::Plain,
        "toon" => OutputFormat::Toon,
        _ => OutputFormat::Xml,
    }
}

/// Handle watch mode execution
fn handle_watch_mode(config: PackConfig, repo_path: PathBuf) -> Result<()> {
    let output_path = config
        .output
        .output_file
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Watch mode requires --output to be specified"))?;

    let watch_config = WatchConfig::new(repo_path, output_path, config);
    run_watch_mode(watch_config)
}

/// Execute the pack command in normal (non-watch) mode
fn execute_pack(
    config: &PackConfig,
    repo_path: &Path,
    stdin_paths: Option<Vec<String>>,
    pb: Option<ProgressBar>,
    start: Instant,
) -> Result<()> {
    // This would contain the rest of the pack logic
    // For now, this is a placeholder - the full implementation would go here
    // Including scanning, filtering, transformations, output generation, etc.

    Ok(())
}
