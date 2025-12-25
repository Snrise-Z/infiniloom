//! Impact command handler
//!
//! Analyzes the impact of changes to a file or symbol.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use infiniloom_engine::index::IndexStorage;
use infiniloom_engine::types::TokenizerModel;

/// Analyze impact of changes to a file or symbol
#[allow(clippy::too_many_arguments)]
pub fn cmd_impact(
    path: PathBuf,
    target: Option<String>,
    is_symbol: bool,
    show_call_graph: bool,
    json_output: bool,
    verbose: bool,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
    model: TokenizerModel,
    depth: u8,
) -> Result<()> {
    // Note: exclude, include_patterns, include_tests would require filtering at index build time
    // For now, log a note if patterns are specified but keep the parameters for API consistency
    if verbose && (!exclude.is_empty() || !include_patterns.is_empty()) {
        eprintln!(
            "Note: --exclude and --include patterns apply to index building, not impact analysis."
        );
        eprintln!("      Run 'infiniloom index --exclude <pattern>' to rebuild the index with exclusions.");
    }

    // Depth controls the analysis depth (1=direct, 2=transitive, 3=full)
    // Currently used for display limits, could be extended for deeper graph traversal
    let display_limit = match depth {
        1 => 5,
        2 => 10,
        3 => 20,
        _ => 10,
    };

    let (path, target) = match target {
        Some(value) => (path, value),
        None => {
            if path.is_dir() {
                anyhow::bail!("Target is required. Use: infiniloom impact <target>");
            }
            (PathBuf::from("."), path.to_string_lossy().to_string())
        },
    };

    let storage = IndexStorage::new(&path);

    // Check if index exists
    if !storage.exists() {
        eprintln!("{} No index found. Run 'infiniloom index' first.", "Error:".red());
        std::process::exit(1);
    }

    if verbose {
        eprintln!("{} Loading symbol index...", "→".cyan());
    }

    // Load index
    let (index, graph) = storage.load_all().context("Failed to load index")?;

    if verbose {
        eprintln!(
            "{} Index loaded: {} files, {} symbols",
            "→".cyan(),
            index.files.len(),
            index.symbols.len()
        );
    }

    // Store parameters for potential future use
    let _ = (&exclude, &include_patterns, include_tests, model);

    if is_symbol {
        analyze_symbol_impact(&index, &graph, &target, show_call_graph, json_output, display_limit)?;
    } else {
        analyze_file_impact(&index, &graph, &target, json_output, display_limit)?;
    }

    Ok(())
}

/// Analyze impact of a symbol
fn analyze_symbol_impact(
    index: &infiniloom_engine::index::SymbolIndex,
    graph: &infiniloom_engine::index::DepGraph,
    target: &str,
    show_call_graph: bool,
    json_output: bool,
    display_limit: usize,
) -> Result<()> {
    // Find symbol
    let symbols = index.find_symbols(target);
    if symbols.is_empty() {
        eprintln!("{} Symbol '{}' not found in index.", "Error:".red(), target);
        std::process::exit(1);
    }

    for symbol in &symbols {
        let file = index.get_file_by_id(symbol.file_id.as_u32());
        let file_path = file.map(|f| f.path.as_str()).unwrap_or("unknown");

        if json_output {
            let callers = graph.get_callers(symbol.id.as_u32());
            let callees = graph.get_callees(symbol.id.as_u32());

            let output = serde_json::json!({
                "symbol": {
                    "name": symbol.name,
                    "kind": symbol.kind.name(),
                    "file": file_path,
                    "line": symbol.span.start_line,
                },
                "callers": callers.len(),
                "callees": callees.len(),
                "impact": callers.len() + callees.len(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!(
                "{} {} ({}) in {}:{}",
                "→".cyan(),
                symbol.name,
                symbol.kind.name(),
                file_path,
                symbol.span.start_line
            );

            let callers = graph.get_callers(symbol.id.as_u32());
            let callees = graph.get_callees(symbol.id.as_u32());

            if !callers.is_empty() {
                println!("  Called by ({}):", callers.len());
                for &caller_id in callers.iter().take(display_limit) {
                    if let Some(caller) = index.get_symbol(caller_id) {
                        let caller_file = index.get_file_by_id(caller.file_id.as_u32());
                        let caller_path = caller_file.map(|f| f.path.as_str()).unwrap_or("unknown");
                        println!("    • {} ({})", caller.name, caller_path);
                    }
                }
                if callers.len() > display_limit {
                    println!("    ... and {} more", callers.len() - display_limit);
                }
            }

            if show_call_graph && !callees.is_empty() {
                println!("  Calls ({}):", callees.len());
                for &callee_id in callees.iter().take(display_limit) {
                    if let Some(callee) = index.get_symbol(callee_id) {
                        let callee_file = index.get_file_by_id(callee.file_id.as_u32());
                        let callee_path = callee_file.map(|f| f.path.as_str()).unwrap_or("unknown");
                        println!("    • {} ({})", callee.name, callee_path);
                    }
                }
                if callees.len() > display_limit {
                    println!("    ... and {} more", callees.len() - display_limit);
                }
            }
        }
    }

    Ok(())
}

/// Analyze impact of a file
fn analyze_file_impact(
    index: &infiniloom_engine::index::SymbolIndex,
    graph: &infiniloom_engine::index::DepGraph,
    target: &str,
    json_output: bool,
    display_limit: usize,
) -> Result<()> {
    // Find file
    let file = index.get_file(target);
    if file.is_none() {
        eprintln!("{} File '{}' not found in index.", "Error:".red(), target);
        std::process::exit(1);
    }

    let file = file.unwrap();
    let importers = graph.get_importers(file.id.as_u32());

    if json_output {
        let output = serde_json::json!({
            "file": {
                "path": file.path,
                "language": file.language.name(),
                "lines": file.lines,
                "tokens": file.tokens,
            },
            "imported_by": importers.len(),
            "symbols": file.symbols.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} {} ({}, {} lines, ~{} tokens)",
            "→".cyan(),
            file.path,
            file.language.name(),
            file.lines,
            file.tokens
        );

        let symbols = index.get_file_symbols(file.id);
        if !symbols.is_empty() {
            println!("  Symbols ({}):", symbols.len());
            for symbol in symbols.iter().take(display_limit) {
                println!(
                    "    • {} ({}) L{}",
                    symbol.name,
                    symbol.kind.name(),
                    symbol.span.start_line
                );
            }
            if symbols.len() > display_limit {
                println!("    ... and {} more", symbols.len() - display_limit);
            }
        }

        if !importers.is_empty() {
            println!("  Imported by ({}):", importers.len());
            for &importer_id in importers.iter().take(display_limit) {
                if let Some(importer) = index.get_file_by_id(importer_id) {
                    println!("    • {}", importer.path);
                }
            }
            if importers.len() > display_limit {
                println!("    ... and {} more", importers.len() - display_limit);
            }
        }
    }

    Ok(())
}
