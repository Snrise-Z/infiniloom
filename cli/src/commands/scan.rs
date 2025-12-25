//! Scan command handler
//!
//! Scans a repository and shows statistics including file counts,
//! sizes, languages, and token estimates.

use anyhow::{Context, Result};
use colored::Colorize;
use humansize::{format_size, BINARY};
use std::path::PathBuf;
use std::time::Instant;

use infiniloom_engine::security::SecurityScanner;
use infiniloom_engine::tokenizer::Tokenizer;
use infiniloom_engine::types::TokenizerModel;

use crate::scanner;

/// Scan repository and show statistics
pub fn cmd_scan(
    path: PathBuf,
    model: TokenizerModel,
    include_hidden: bool,
    verbose: bool,
    json_output: bool,
    security_check: bool,
    sample: Option<usize>,
    sample_percent: Option<f64>,
    exclude: Vec<String>,
    include_patterns: Vec<String>,
    include_tests: bool,
) -> Result<()> {
    use infiniloom_engine::default_ignores::{matches_any, TEST_IGNORES};
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    let start = Instant::now();

    // Always read content to get accurate token counts (not just heuristic estimates)
    let config = scanner::ScanConfig {
        include_hidden,
        respect_gitignore: true,
        read_contents: true, // Read content for accurate token counting
        max_file_size: 50 * 1024 * 1024u64,
        skip_symbols: true, // No need for symbols in scan mode
    };

    let mut repo = scanner::scan_repository(&path, config).context("Failed to scan repository")?;

    // Apply exclude patterns if provided
    if !exclude.is_empty() {
        repo.files.retain(|f| {
            !exclude.iter().any(|pattern| {
                f.relative_path.contains(pattern)
                    || f.relative_path.starts_with(pattern)
                    || f.relative_path.split('/').any(|part| part == pattern)
            })
        });
    }

    // Apply include patterns if provided (only keep matching files)
    if !include_patterns.is_empty() {
        repo.files.retain(|f| {
            include_patterns.iter().any(|pattern| {
                // Support glob-like patterns
                if pattern.contains('*') {
                    glob::Pattern::new(pattern).is_ok_and(|p| p.matches(&f.relative_path))
                } else {
                    f.relative_path.contains(pattern) || f.relative_path.ends_with(pattern)
                }
            })
        });
    }

    // Exclude test files unless include_tests is true
    if !include_tests {
        repo.files
            .retain(|f| !matches_any(&f.relative_path, TEST_IGNORES));
    }

    // Update metadata after filtering
    repo.metadata.total_files = repo.files.len() as u32;

    // Apply sampling if requested
    let (is_sampled, sample_size, original_count) = if let Some(n) = sample {
        let original_count = repo.files.len();
        if n < original_count && n > 0 {
            let mut rng = thread_rng();
            repo.files.shuffle(&mut rng);
            repo.files.truncate(n);
            repo.metadata.total_files = n as u32;
            (true, n, original_count)
        } else {
            (false, repo.files.len(), repo.files.len())
        }
    } else if let Some(p) = sample_percent {
        let original_count = repo.files.len();
        let n = ((original_count as f64) * (p / 100.0)).ceil() as usize;
        if n < original_count && n > 0 {
            let mut rng = thread_rng();
            repo.files.shuffle(&mut rng);
            repo.files.truncate(n);
            repo.metadata.total_files = n as u32;
            (true, n, original_count)
        } else {
            (false, repo.files.len(), repo.files.len())
        }
    } else {
        (false, repo.files.len(), repo.files.len())
    };

    // Extrapolation factor for estimating totals from sample
    let extrapolation_factor = if is_sampled && sample_size > 0 {
        original_count as f64 / sample_size as f64
    } else {
        1.0
    };

    // Compute accurate token counts using the real tokenizer (not heuristics)
    {
        let tokenizer = Tokenizer::new();
        for file in &mut repo.files {
            if let Some(ref content) = file.content {
                let counts = tokenizer.count_all(content);
                file.token_count = infiniloom_engine::types::TokenCounts {
                    o200k: counts.o200k,
                    cl100k: counts.cl100k,
                    claude: counts.claude,
                    gemini: counts.gemini,
                    llama: counts.llama,
                    mistral: counts.mistral,
                    deepseek: counts.deepseek,
                    qwen: counts.qwen,
                    cohere: counts.cohere,
                    grok: counts.grok,
                };
            }
        }
        // Update metadata totals
        repo.metadata.total_tokens = infiniloom_engine::types::TokenCounts {
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
    }

    // Run security scan if requested (parallelized for performance)
    let security_issues = if security_check {
        use rayon::prelude::*;
        let scanner = SecurityScanner::new();
        let all_issues: Vec<_> = repo
            .files
            .par_iter()
            .filter_map(|file| {
                if let Some(ref content) = file.content {
                    let (_, file_issues) = scanner.scan_and_redact(content, &file.relative_path);
                    if file_issues.is_empty() {
                        None
                    } else {
                        Some(file_issues)
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        Some(all_issues)
    } else {
        None
    };

    let elapsed = start.elapsed();

    if json_output {
        output_json(
            &repo,
            &path,
            is_sampled,
            sample_size,
            original_count,
            extrapolation_factor,
            elapsed,
            security_issues.as_ref(),
        )?;
    } else {
        output_human(
            &repo,
            &path,
            model,
            is_sampled,
            sample_size,
            original_count,
            extrapolation_factor,
            elapsed,
            verbose,
            security_issues.as_ref(),
        );
    }

    Ok(())
}

/// Output scan results as JSON
fn output_json(
    repo: &infiniloom_engine::Repository,
    _path: &PathBuf,
    is_sampled: bool,
    sample_size: usize,
    original_count: usize,
    extrapolation_factor: f64,
    elapsed: std::time::Duration,
    security_issues: Option<&Vec<infiniloom_engine::security::SecretFinding>>,
) -> Result<()> {
    // Calculate estimated totals (extrapolate if sampled)
    let sampled_bytes: u64 = repo.files.iter().map(|f| f.size_bytes).sum();
    let estimated_bytes = (sampled_bytes as f64 * extrapolation_factor) as u64;
    let estimated_claude =
        (repo.total_tokens(TokenizerModel::Claude) as f64 * extrapolation_factor) as u32;
    let estimated_gpt4o =
        (repo.total_tokens(TokenizerModel::Gpt4o) as f64 * extrapolation_factor) as u32;
    let estimated_gemini =
        (repo.total_tokens(TokenizerModel::Gemini) as f64 * extrapolation_factor) as u32;

    // JSON output
    let mut stats = serde_json::json!({
        "repository": repo.name,
        "files": if is_sampled { original_count } else { repo.files.len() },
        "total_bytes": estimated_bytes,
        "total_tokens": {
            "claude": estimated_claude,
            "gpt4o": estimated_gpt4o,
            "gemini": estimated_gemini,
        },
        "languages": repo.metadata.languages,
        "scan_time_ms": elapsed.as_millis(),
    });

    // Add sampling metadata if sampled
    if is_sampled {
        stats["sampling"] = serde_json::json!({
            "is_estimated": true,
            "sample_size": sample_size,
            "total_files": original_count,
            "sample_percent": (sample_size as f64 / original_count as f64 * 100.0),
            "extrapolation_factor": extrapolation_factor,
        });
    }

    // Add security issues if scanned
    if let Some(issues) = security_issues {
        let estimated_issues = (issues.len() as f64 * extrapolation_factor) as usize;
        stats["security"] = serde_json::json!({
            "issues_found": if is_sampled { estimated_issues } else { issues.len() },
            "is_estimated": is_sampled,
            "issues": issues.iter().map(|i| serde_json::json!({
                "file": &i.file,
                "line": i.line,
                "kind": i.kind.name(),
                "severity": format!("{:?}", i.severity),
            })).collect::<Vec<_>>(),
        });
    }
    println!("{}", serde_json::to_string_pretty(&stats)?);

    Ok(())
}

/// Output scan results in human-readable format
#[allow(clippy::too_many_arguments)]
fn output_human(
    repo: &infiniloom_engine::Repository,
    path: &PathBuf,
    model: TokenizerModel,
    is_sampled: bool,
    sample_size: usize,
    original_count: usize,
    extrapolation_factor: f64,
    elapsed: std::time::Duration,
    verbose: bool,
    security_issues: Option<&Vec<infiniloom_engine::security::SecretFinding>>,
) {
    println!();
    println!("{}", "━".repeat(50).dimmed());
    if is_sampled {
        println!("  {} {}", "Scan Results".cyan().bold(), "[ESTIMATED]".yellow().bold());
    } else {
        println!("  {}", "Scan Results".cyan().bold());
    }
    println!("{}", "━".repeat(50).dimmed());
    println!();

    println!("  Repository:   {}", repo.name.yellow());
    println!("  Path:         {}", path.display());

    // Show file count with sampling info
    if is_sampled {
        println!(
            "  Files:        {} (sampled {} of {})",
            original_count, sample_size, original_count
        );
    } else {
        println!("  Files:        {}", repo.files.len());
    }

    let sampled_bytes: u64 = repo.files.iter().map(|f| f.size_bytes).sum();
    let estimated_bytes = (sampled_bytes as f64 * extrapolation_factor) as u64;
    if is_sampled {
        println!("  Total Size:   ~{} (estimated)", format_size(estimated_bytes, BINARY));
    } else {
        println!("  Total Size:   {}", format_size(sampled_bytes, BINARY));
    }
    println!("  Scan Time:    {:?}", elapsed);
    println!();

    // Show sampling details
    if is_sampled {
        println!("  {}:", "Sampling".yellow());
        println!("    Sample size:        {} files", sample_size);
        println!(
            "    Sample percentage:  {:.1}%",
            sample_size as f64 / original_count as f64 * 100.0
        );
        println!("    Extrapolation:      {:.2}x", extrapolation_factor);
        println!();
    }

    // Language breakdown
    if !repo.metadata.languages.is_empty() {
        println!("  {}:", "Languages".cyan());
        for lang in &repo.metadata.languages {
            println!("    {}: {} files ({:.1}%)", lang.language, lang.files, lang.percentage);
        }
        println!();
    }

    // Token counts (accurate, using tiktoken for OpenAI models)
    let estimated_tokens = (repo.total_tokens(model) as f64 * extrapolation_factor) as u32;
    if is_sampled {
        println!("  {} ({}) {}:", "Token Counts".cyan(), model.name(), "[ESTIMATED]".yellow());
        println!("    Total: ~{}", estimated_tokens);
    } else {
        println!("  {} ({}):", "Token Counts".cyan(), model.name());
        println!("    Total: {}", repo.total_tokens(model));
    }
    println!();

    // Verbose file list
    if verbose {
        println!("  {}:", "Files".cyan());
        let mut files: Vec<_> = repo.files.iter().collect();
        files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        for file in files.iter().take(20) {
            let lang = file.language.as_deref().unwrap_or("?");
            println!(
                "    {} ({}) - {}",
                file.relative_path,
                lang,
                format_size(file.size_bytes, BINARY)
            );
        }

        if files.len() > 20 {
            println!("    ... and {} more files", files.len() - 20);
        }
        println!();
    }

    // Security scan results
    if let Some(issues) = security_issues {
        if is_sampled {
            println!("  {} {}:", "Security Scan".cyan(), "[ESTIMATED]".yellow());
        } else {
            println!("  {}:", "Security Scan".cyan());
        }
        if issues.is_empty() {
            println!("    {} No secrets detected in sample", "✓".green());
        } else {
            let estimated_issues = (issues.len() as f64 * extrapolation_factor) as usize;
            if is_sampled {
                println!(
                    "    {} Found ~{} potential secrets (estimated from {} in sample):",
                    "⚠".yellow(),
                    estimated_issues,
                    issues.len()
                );
            } else {
                println!("    {} Found {} potential secrets:", "⚠".yellow(), issues.len());
            }
            for issue in issues.iter().take(10) {
                println!(
                    "      • [{:?}] {} in {} (line {})",
                    issue.severity,
                    issue.kind.name(),
                    issue.file,
                    issue.line
                );
            }
            if issues.len() > 10 {
                println!("      ... and {} more in sample", issues.len() - 10);
            }
        }
        println!();
    }
}
