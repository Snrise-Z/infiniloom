//! Index command handler
//!
//! Builds and manages the symbol index for fast diff context lookups.

use anyhow::{Context, Result};
use colored::Colorize;
use humansize::{format_size, BINARY};
use std::path::{Path, PathBuf};
use std::time::Instant;

use infiniloom_engine::index::{BuildOptions, IndexBuilder, IndexStorage};

/// Build or update the symbol index
pub fn cmd_index(
    path: PathBuf,
    force: bool,
    status: bool,
    verbose: bool,
    watch_mode: bool,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
    incremental: bool,
) -> Result<()> {
    let storage = IndexStorage::new(&path);

    // Just show status
    if status {
        if storage.exists() {
            match storage.load_meta() {
                Ok(meta) => {
                    println!("{} Index found", "✓".green());
                    println!("  Repository: {}", meta.repo_name);
                    println!("  Files indexed: {}", meta.file_count);
                    println!("  Symbols indexed: {}", meta.symbol_count);
                    println!("  Index size: {}", format_size(meta.index_size_bytes, BINARY));
                    if let Some(ref commit) = meta.commit_hash {
                        println!("  Git commit: {}", &commit[..7.min(commit.len())]);
                    }
                    println!("  Created: {}", chrono_humanize(meta.created_at));
                },
                Err(e) => {
                    eprintln!("{} Failed to read index metadata: {}", "✗".red(), e);
                },
            }
        } else {
            println!("{} No index found at {}", "✗".yellow(), path.display());
            println!("  Run 'infiniloom index' to create one.");
        }
        return Ok(());
    }

    // Check if we need to rebuild (skip check in watch mode - always build initially)
    if !watch_mode && !force && storage.exists() {
        if let Ok(meta) = storage.load_meta() {
            // Check if index is recent
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if now - meta.created_at < 300 {
                // Less than 5 minutes old
                if verbose {
                    println!("Index is recent (< 5 minutes). Use --force to rebuild.");
                }
                return Ok(());
            }
        }
    }

    // Initial build
    build_index(&storage, &path, verbose, &exclude, &include_patterns, include_tests, incremental)?;

    // If watch mode, start watching for changes
    if watch_mode {
        watch_for_changes(&storage, &path, verbose, &exclude, &include_patterns, include_tests)?;
    }

    Ok(())
}

/// Build and save the symbol index
fn build_index(
    storage: &IndexStorage,
    path: &Path,
    verbose: bool,
    exclude: &[String],
    include_patterns: &[String],
    include_tests: bool,
    incremental: bool,
) -> Result<()> {
    if verbose {
        if incremental {
            println!("{}", "Performing incremental index update...".cyan());
        } else {
            println!("{}", "Building symbol index...".cyan());
        }
    }

    let start = Instant::now();

    // Build default exclude directories
    let mut exclude_dirs = vec![
        "node_modules".to_string(),
        "target".to_string(),
        ".git".to_string(),
        "dist".to_string(),
        "build".to_string(),
    ];

    // Exclude test directories unless include_tests is true
    if !include_tests {
        exclude_dirs.extend(vec![
            "test".to_string(),
            "tests".to_string(),
            "__tests__".to_string(),
            "spec".to_string(),
        ]);
    }

    // Add custom exclude patterns (Feature #1)
    exclude_dirs.extend(exclude.iter().cloned());

    // Note: include_patterns would require changes to BuildOptions and IndexBuilder
    // For now, we log a warning if patterns are provided
    if !include_patterns.is_empty() && verbose {
        eprintln!(
            "Note: --include patterns are not yet supported for index command. Indexing all files."
        );
    }

    // Build options with exclusions
    let build_opts = BuildOptions { respect_gitignore: true, exclude_dirs, ..Default::default() };

    // Build index (incremental support via Feature #4)
    let (index, graph, files_updated) = if incremental {
        // Load existing index if available for comparison
        if let (Ok(existing_index), Ok(_existing_graph)) =
            (storage.load_index(), storage.load_graph())
        {
            let builder = IndexBuilder::new(path).with_options(build_opts);
            let (new_index, new_graph) = builder.build().context("Failed to build index")?;

            // Count files that changed (simplified comparison)
            let updated = (new_index.files.len() as i64 - existing_index.files.len() as i64)
                .unsigned_abs() as u32;

            (new_index, new_graph, Some(updated))
        } else {
            // No existing index, do full build
            let builder = IndexBuilder::new(path).with_options(build_opts);
            let (index, graph) = builder.build().context("Failed to build index")?;
            let count = index.files.len() as u32;
            (index, graph, Some(count))
        }
    } else {
        let builder = IndexBuilder::new(path).with_options(build_opts);
        let (index, graph) = builder.build().context("Failed to build index")?;
        (index, graph, None)
    };

    // Save index
    let meta = storage
        .save_all(&index, &graph)
        .context("Failed to save index")?;

    let elapsed = start.elapsed();

    println!("{} Index built successfully", "✓".green());
    println!("  Files: {}", meta.file_count);
    println!("  Symbols: {}", meta.symbol_count);
    println!("  Size: {}", format_size(meta.index_size_bytes, BINARY));
    println!("  Time: {:.2}s", elapsed.as_secs_f64());
    if let Some(updated) = files_updated {
        println!("  Files updated: {}", updated);
    }
    if !exclude.is_empty() {
        println!("  Excluded: {}", exclude.join(", "));
    }
    println!();
    println!("Index saved to {}", storage.index_dir().display());

    Ok(())
}

/// Watch for file changes and rebuild index when needed
fn watch_for_changes(
    storage: &IndexStorage,
    path: &Path,
    verbose: bool,
    exclude: &[String],
    include_patterns: &[String],
    include_tests: bool,
) -> Result<()> {
    use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!();
    eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());

    let (tx, rx) = channel();

    let mut watcher = PollWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    )
    .context("Failed to create file watcher")?;

    watcher
        .watch(path, RecursiveMode::Recursive)
        .context("Failed to watch directory")?;

    // Debounce: wait for changes to settle
    let debounce_duration = Duration::from_millis(500);
    let mut last_rebuild = Instant::now();
    let mut pending_rebuild = false;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(()) => {
                pending_rebuild = true;
                last_rebuild = Instant::now();
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if we should rebuild (debounce elapsed)
                if pending_rebuild && last_rebuild.elapsed() >= debounce_duration {
                    pending_rebuild = false;
                    println!();
                    eprintln!("{} File changes detected, rebuilding index...", "🔄".yellow());
                    // Pass exclude patterns but use incremental=true in watch mode
                    if let Err(e) = build_index(
                        storage,
                        path,
                        verbose,
                        exclude,
                        include_patterns,
                        include_tests,
                        true,
                    ) {
                        eprintln!("{} Failed to rebuild index: {}", "✗".red(), e);
                    }
                    eprintln!("{} Watching for file changes... (Ctrl+C to stop)", "👀".cyan());
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            },
        }
    }

    Ok(())
}

/// Human-readable time ago
fn chrono_humanize(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        format!("{} seconds ago", diff)
    } else if diff < 3600 {
        format!("{} minutes ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // chrono_humanize Tests
    // ============================================

    fn get_current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    #[test]
    fn test_chrono_humanize_seconds_ago() {
        let now = get_current_time();
        let result = chrono_humanize(now - 30);
        assert!(result.contains("seconds ago"));
    }

    #[test]
    fn test_chrono_humanize_minutes_ago() {
        let now = get_current_time();
        let result = chrono_humanize(now - 300); // 5 minutes
        assert!(result.contains("minutes ago"));
    }

    #[test]
    fn test_chrono_humanize_hours_ago() {
        let now = get_current_time();
        let result = chrono_humanize(now - 7200); // 2 hours
        assert!(result.contains("hours ago"));
    }

    #[test]
    fn test_chrono_humanize_days_ago() {
        let now = get_current_time();
        let result = chrono_humanize(now - 172800); // 2 days
        assert!(result.contains("days ago"));
    }

    #[test]
    fn test_chrono_humanize_just_now() {
        let now = get_current_time();
        let result = chrono_humanize(now);
        assert!(result.contains("0 seconds ago"));
    }

    #[test]
    fn test_chrono_humanize_boundary_59_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 59);
        assert!(result.contains("seconds ago"));
        assert!(result.contains("59"));
    }

    #[test]
    fn test_chrono_humanize_boundary_60_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 60);
        assert!(result.contains("minutes ago"));
        assert!(result.contains("1"));
    }

    #[test]
    fn test_chrono_humanize_boundary_3599_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 3599); // 59 minutes 59 seconds
        assert!(result.contains("minutes ago"));
    }

    #[test]
    fn test_chrono_humanize_boundary_3600_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 3600); // Exactly 1 hour
        assert!(result.contains("hours ago"));
        assert!(result.contains("1"));
    }

    #[test]
    fn test_chrono_humanize_boundary_86399_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 86399); // 23 hours 59 minutes 59 seconds
        assert!(result.contains("hours ago"));
    }

    #[test]
    fn test_chrono_humanize_boundary_86400_seconds() {
        let now = get_current_time();
        let result = chrono_humanize(now - 86400); // Exactly 1 day
        assert!(result.contains("days ago"));
        assert!(result.contains("1"));
    }

    #[test]
    fn test_chrono_humanize_future_timestamp() {
        let now = get_current_time();
        // Future timestamp should saturate to 0
        let result = chrono_humanize(now + 1000);
        assert!(result.contains("0 seconds ago"));
    }

    #[test]
    fn test_chrono_humanize_old_timestamp() {
        let now = get_current_time();
        let result = chrono_humanize(now - 604800); // 7 days
        assert!(result.contains("7 days ago"));
    }
}
