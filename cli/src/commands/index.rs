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
                }
                Err(e) => {
                    eprintln!("{} Failed to read index metadata: {}", "✗".red(), e);
                }
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
    build_index(&storage, &path, verbose)?;

    // If watch mode, start watching for changes
    if watch_mode {
        watch_for_changes(&storage, &path, verbose)?;
    }

    Ok(())
}

/// Build and save the symbol index
fn build_index(storage: &IndexStorage, path: &Path, verbose: bool) -> Result<()> {
    if verbose {
        println!("{}", "Building symbol index...".cyan());
    }

    let start = Instant::now();

    // Build index
    let builder = IndexBuilder::new(path)
        .with_options(BuildOptions { respect_gitignore: true, ..Default::default() });

    let (index, graph) = builder.build().context("Failed to build index")?;

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
    println!();
    println!("Index saved to {}", storage.index_dir().display());

    Ok(())
}

/// Watch for file changes and rebuild index when needed
fn watch_for_changes(storage: &IndexStorage, path: &Path, verbose: bool) -> Result<()> {
    use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!();
    eprintln!(
        "{} Watching for file changes... (Ctrl+C to stop)",
        "👀".cyan()
    );

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
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if we should rebuild (debounce elapsed)
                if pending_rebuild && last_rebuild.elapsed() >= debounce_duration {
                    pending_rebuild = false;
                    println!();
                    eprintln!(
                        "{} File changes detected, rebuilding index...",
                        "🔄".yellow()
                    );
                    if let Err(e) = build_index(storage, path, verbose) {
                        eprintln!("{} Failed to rebuild index: {}", "✗".red(), e);
                    }
                    eprintln!(
                        "{} Watching for file changes... (Ctrl+C to stop)",
                        "👀".cyan()
                    );
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
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
