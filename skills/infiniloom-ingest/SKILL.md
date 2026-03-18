---
name: infiniloom-ingest
version: 1.0.0
description: "Ingest documents (Markdown, HTML, CSV, DOCX) into LLM-optimized format with optional PII redaction."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom ingest

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Convert documents into LLM-friendly formats with content distillation and optional PII detection/redaction. Supports Markdown, HTML, CSV, and DOCX input.

## Usage

```bash
infiniloom ingest [OPTIONS] <PATH>
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--format` | `-f` | `xml` | Output format: xml, markdown, json |
| `--distillation` | `-d` | `balanced` | Compression level: none, minimal, balanced, aggressive, full |
| `--output` | `-o` | stdout | Output file path |
| `--model` | `-m` | `claude` | Target model for token counting display |
| `--max-tokens` | | none | Token budget warning threshold |
| `--pii-scan` | | off | Scan for personally identifiable information |
| `--redact-pii` | | off | Redact detected PII in output |
| `--chunk` | | off | Split document into chunks |
| `--max-chunk-tokens` | | `4000` | Maximum tokens per chunk |
| `--overlap-tokens` | | `200` | Overlap tokens between chunks |
| `--verbose` | `-v` | off | Show detailed output |

## Examples

```bash
# Convert Markdown to XML (Claude-optimized)
infiniloom ingest report.md

# HTML to Markdown for GPT, with aggressive distillation
infiniloom ingest page.html -f markdown -d aggressive

# DOCX with PII redaction, saved to file
infiniloom ingest contract.docx --redact-pii -o clean.xml

# CSV to JSON format
infiniloom ingest data.csv -f json

# Chunk a long document for multi-turn conversation
infiniloom ingest thesis.md --chunk --max-chunk-tokens 8000
```

## Tips

- Use `--distillation aggressive` to strip boilerplate and filler language from verbose documents.
- The `--chunk` flag is useful for documents that exceed a model's context window.
- Combine `--pii-scan` (report only) with `--redact-pii` (replace) depending on your compliance needs.
