//! Document ingestion command - convert documents to LLM-optimized formats.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

use infiniloom_engine::document::{
    self,
    chunking::{self, ChunkConfig},
    distillation, output as doc_output, pii,
    types::{DistillationLevel, DocumentFormat},
    ParseOptions,
};
use infiniloom_engine::tokenizer::TokenModel as TokenizerModel;

/// Configuration for the ingest command.
pub(crate) struct IngestConfig {
    pub path: PathBuf,
    pub format: IngestOutputFormat,
    pub distillation: DistillationLevel,
    pub output_file: Option<PathBuf>,
    pub model: Option<TokenizerModel>,
    pub max_tokens: Option<usize>,
    pub pii_scan: bool,
    pub redact_pii: bool,
    pub chunk: bool,
    pub max_chunk_tokens: usize,
    pub overlap_tokens: usize,
    pub verbose: bool,
}

/// Output format for ingested documents.
#[derive(Clone, Copy, Default)]
pub(crate) enum IngestOutputFormat {
    /// Claude-optimized XML (default)
    #[default]
    Xml,
    /// GPT-optimized Markdown
    Markdown,
    /// Agent-friendly JSON
    Json,
}

pub(crate) fn cmd_ingest(config: IngestConfig) -> Result<()> {
    let path = &config.path;

    // Validate file exists
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    if !path.is_file() {
        anyhow::bail!("Not a file: {}", path.display());
    }

    // Detect format from extension
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let doc_format = DocumentFormat::from_extension(ext)
        .with_context(|| format!("Unsupported document format: .{ext}"))?;

    if config.verbose {
        eprintln!("Ingesting: {}", path.display());
        eprintln!("Format detected: {}", doc_format.name());
        eprintln!("Distillation: {:?}", config.distillation);
    }

    // Parse the document
    let options = ParseOptions { distillation: config.distillation, ..ParseOptions::default() };
    let mut doc = document::parse_document(path, &options)
        .with_context(|| format!("Failed to parse: {}", path.display()))?;

    if config.verbose {
        eprintln!("Parsed: {} sections, {} content blocks", doc.section_count(), doc.block_count());
    }

    // Apply distillation pipeline
    distillation::distill(&mut doc, config.distillation);

    if config.verbose {
        eprintln!(
            "After distillation: {} sections, {} content blocks",
            doc.section_count(),
            doc.block_count()
        );

        let tc = &doc.token_count;
        eprintln!(
            "Token counts (raw): claude={}, gpt4o={}, gemini={}",
            tc.claude, tc.o200k, tc.gemini
        );

        if let Some(model) = config.model {
            eprintln!("  {:?} = {}", model, tc.get(model));
        }
    }

    // PII scanning and redaction
    if config.pii_scan || config.redact_pii {
        let findings = pii::scan_document(&doc);

        if config.verbose {
            for f in &findings {
                eprintln!(
                    "  PII: {:?} at [{}] line ~{}: {}",
                    f.kind, f.location, f.line_approx, f.text
                );
            }
        }

        eprintln!("{}", pii::summarize(&findings));

        if config.redact_pii {
            pii::redact_document(&mut doc);
            if config.verbose {
                eprintln!("PII redaction applied.");
            }
        }
    }

    // Chunking path: split document into chunks for multi-turn conversations
    if config.chunk {
        let chunk_config = ChunkConfig {
            max_tokens: config.max_chunk_tokens,
            overlap_tokens: config.overlap_tokens,
        };
        let chunks = chunking::chunk_document(&doc, &chunk_config);

        if config.verbose {
            let avg_tokens = if chunks.is_empty() {
                0
            } else {
                chunks.iter().map(|c| c.token_count).sum::<usize>() / chunks.len()
            };
            eprintln!("Split into {} chunks (avg ~{} tokens each)", chunks.len(), avg_tokens);
        }

        if let Some(out_path) = &config.output_file {
            let json = chunking::format_chunks_json(&chunks);
            std::fs::write(out_path, &json)
                .with_context(|| format!("Failed to write: {}", out_path.display()))?;
            if config.verbose {
                eprintln!("Written to: {}", out_path.display());
            }
        } else {
            for chunk in &chunks {
                let text = chunking::format_chunk_text(chunk);
                std::io::stdout()
                    .write_all(text.as_bytes())
                    .context("Failed to write to stdout")?;
            }
        }

        return Ok(());
    }

    // Format output
    let format_name = match config.format {
        IngestOutputFormat::Xml => "XML",
        IngestOutputFormat::Markdown => "Markdown",
        IngestOutputFormat::Json => "JSON",
    };
    let output = match config.format {
        IngestOutputFormat::Xml => doc_output::format_xml(&doc),
        IngestOutputFormat::Markdown => doc_output::format_markdown(&doc),
        IngestOutputFormat::Json => doc_output::format_json(&doc),
    };

    // Count tokens on the formatted output
    let output_tokens = document::count_output_tokens(&output);

    if config.verbose {
        eprintln!(
            "Token counts (formatted {}): claude={}, gpt4o={}, gemini={}",
            format_name, output_tokens.claude, output_tokens.o200k, output_tokens.gemini
        );

        if let Some(model) = config.model {
            eprintln!("  {:?} = {}", model, output_tokens.get(model));
        }
    }

    // Warn if output exceeds token budget
    if let Some(budget) = config.max_tokens {
        let model = config.model.unwrap_or(TokenizerModel::Claude);
        let count = output_tokens.get(model) as usize;
        if count > budget {
            eprintln!(
                "Warning: Output exceeds token budget ({} > {} tokens for {:?})",
                count, budget, model
            );
        }
    }

    // Write output
    if let Some(out_path) = &config.output_file {
        std::fs::write(out_path, &output)
            .with_context(|| format!("Failed to write: {}", out_path.display()))?;
        if config.verbose {
            eprintln!("Written to: {}", out_path.display());
        }
    } else {
        std::io::stdout()
            .write_all(output.as_bytes())
            .context("Failed to write to stdout")?;
    }

    Ok(())
}
