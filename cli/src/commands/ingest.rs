//! Document ingestion command - convert documents to LLM-optimized formats.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

use infiniloom_engine::document::{
    self, distillation, output as doc_output,
    types::{DistillationLevel, DocumentFormat},
    ParseOptions,
};

/// Configuration for the ingest command.
pub(crate) struct IngestConfig {
    pub path: PathBuf,
    pub format: IngestOutputFormat,
    pub distillation: DistillationLevel,
    pub output_file: Option<PathBuf>,
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
    }

    // Format output
    let output = match config.format {
        IngestOutputFormat::Xml => doc_output::format_xml(&doc),
        IngestOutputFormat::Markdown => doc_output::format_markdown(&doc),
        IngestOutputFormat::Json => doc_output::format_json(&doc),
    };

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
