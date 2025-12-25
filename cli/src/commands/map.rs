//! Map command handler
//!
//! Generates a repository map (symbol index) with PageRank-based ranking.

use anyhow::{Context, Result};
use std::fmt::Write;
use std::path::PathBuf;

use infiniloom_engine::repomap::{RepoMap, RepoMapGenerator};
use infiniloom_engine::types::TokenizerModel;

use crate::config::load_config_file;
use crate::scanner;

/// Generate a repository map with key symbols
pub fn cmd_map(
    path: PathBuf,
    budget: u32,
    cli_model: Option<TokenizerModel>,
    output: Option<PathBuf>,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
    verbose: bool,
) -> Result<()> {
    use std::time::Instant;
    let start = Instant::now();

    let config = scanner::ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024u64,
        skip_symbols: false, // Map command needs symbols for ranking
    };

    let mut repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    // Apply default ignores to filter out build outputs, dependencies, etc.
    // This prevents dist/, node_modules/, etc. from appearing in the map
    {
        use infiniloom_engine::default_ignores::{matches_any, DEFAULT_IGNORES, TEST_IGNORES};
        repo.files.retain(|f| !matches_any(&f.relative_path, DEFAULT_IGNORES));
        // Exclude test files unless include_tests is true
        if !include_tests {
            repo.files.retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
        }
    }

    // Apply custom exclude patterns if provided
    if !exclude.is_empty() {
        repo.files.retain(|f| {
            !exclude.iter().any(|pattern| {
                f.relative_path.contains(pattern)
                    || f.relative_path.starts_with(pattern)
                    || f.relative_path
                        .split('/')
                        .any(|part| part == pattern)
            })
        });
    }

    // Apply include patterns if provided (only keep matching files)
    if !include_patterns.is_empty() {
        repo.files.retain(|f| {
            include_patterns.iter().any(|pattern| {
                // Support glob-like patterns
                if pattern.contains('*') {
                    glob::Pattern::new(pattern)
                        .is_ok_and(|p| p.matches(&f.relative_path))
                } else {
                    f.relative_path.contains(pattern)
                        || f.relative_path.ends_with(pattern)
                }
            })
        });
    }

    if verbose {
        eprintln!("Scanning {} files...", repo.files.len());
    }

    // Count cross-file symbol references (populates Symbol.references field)
    infiniloom_engine::count_symbol_references(&mut repo);

    // Rank files by importance
    infiniloom_engine::rank_files(&mut repo);
    infiniloom_engine::sort_files_by_importance(&mut repo);

    let loaded_config = load_config_file(None, &path);
    let model: TokenizerModel = if let Some(m) = cli_model {
        m
    } else if let Some(ref model_str) = loaded_config.model {
        TokenizerModel::from_model_name(model_str).unwrap_or(TokenizerModel::Claude)
    } else {
        TokenizerModel::Claude
    };

    let map = RepoMapGenerator::builder()
        .token_budget(budget)
        .model(model)
        .build()
        .generate(&repo);

    let output_text = format_repo_map(&map, budget);

    if let Some(output_path) = output {
        std::fs::write(&output_path, &output_text).context("Failed to write output file")?;
        eprintln!("Repository map written to: {}", output_path.display());
    } else {
        println!("{}", output_text);
    }

    Ok(())
}

/// Format a RepoMap for human-readable output
fn format_repo_map(map: &RepoMap, budget: u32) -> String {
    let mut output = String::new();

    let _ = writeln!(
        &mut output,
        "Repository Map (budget: {}, estimated tokens: {})",
        budget, map.token_count
    );
    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Summary");
    let _ = writeln!(&mut output, "-------");
    output.push_str(&map.summary);
    output.push('\n');

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Key Symbols");
    let _ = writeln!(&mut output, "-----------");
    if map.key_symbols.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for sym in &map.key_symbols {
            let _ = writeln!(
                &mut output,
                "{:>2}. {} {} - {}:{} (refs: {}, importance: {:.2})",
                sym.rank, sym.kind, sym.name, sym.file, sym.line, sym.references, sym.importance
            );
            if let Some(ref summary) = sym.summary {
                let _ = writeln!(&mut output, "    summary: {}", summary);
            }
            if let Some(ref sig) = sym.signature {
                if let Some(first_line) = sig.lines().next() {
                    let _ = writeln!(&mut output, "    signature: {}", first_line.trim());
                }
            }
        }
    }

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "Module Graph");
    let _ = writeln!(&mut output, "------------");
    if map.module_graph.nodes.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for node in &map.module_graph.nodes {
            let _ = writeln!(
                &mut output,
                "- {} (files: {}, tokens: {})",
                node.name, node.files, node.tokens
            );
        }
        if !map.module_graph.edges.is_empty() {
            let _ = writeln!(&mut output, "Dependencies:");
            for edge in &map.module_graph.edges {
                let _ = writeln!(
                    &mut output,
                    "  {} -> {} (weight: {})",
                    edge.from, edge.to, edge.weight
                );
            }
        }
    }

    let _ = writeln!(&mut output);
    let _ = writeln!(&mut output, "File Index");
    let _ = writeln!(&mut output, "----------");
    if map.file_index.is_empty() {
        let _ = writeln!(&mut output, "(none)");
    } else {
        for entry in &map.file_index {
            let _ = writeln!(
                &mut output,
                "- {} (tokens: {}, importance: {})",
                entry.path, entry.tokens, entry.importance
            );
        }
    }

    output
}
