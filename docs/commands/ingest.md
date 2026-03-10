# ingest - Document Ingestion

Convert documents into LLM-optimized formats with optional content distillation, PII detection/redaction, and chunking for multi-turn conversations.

## Overview

The `ingest` command transforms non-code documents (Markdown, HTML, CSV, DOCX, XLSX) into structured formats that LLMs can process efficiently. This is useful for feeding documentation, reports, spreadsheets, and other business documents into AI workflows.

**Note**: This command requires the `document` feature flag (enabled by default). XLSX support requires the additional `document-xlsx` feature.

## Basic Usage

```bash
infiniloom ingest report.md                     # Markdown → XML (stdout)
infiniloom ingest page.html -f markdown         # HTML → Markdown
infiniloom ingest data.csv -f json              # CSV → JSON
infiniloom ingest report.docx -o output.xml     # DOCX → XML file
infiniloom ingest spreadsheet.xlsx -f json      # XLSX → JSON (requires document-xlsx)
```

## Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--format` | `-f` | `xml` | Output format: `xml`, `markdown`, `json` |
| `--distillation` | `-d` | `balanced` | Distillation level: `minimal`, `balanced`, `aggressive` |
| `--output` | `-o` | stdout | Output file path |
| `--model` | `-m` | — | Target model for token counting display |
| `--max-tokens` | — | — | Token budget warning threshold |
| `--pii-scan` | — | false | Scan for personally identifiable information |
| `--redact-pii` | — | false | Redact detected PII in output |
| `--chunk` | — | false | Split document into chunks |
| `--max-chunk-tokens` | — | 4000 | Maximum tokens per chunk |
| `--overlap-tokens` | — | 200 | Overlap tokens between chunks |
| `--verbose` | `-v` | false | Verbose output to stderr |

## Supported Document Formats

| Format | Extensions | Parser | Notes |
|--------|-----------|--------|-------|
| Markdown | `.md`, `.markdown` | Built-in | CommonMark + GFM tables, YAML frontmatter stripped |
| HTML | `.html`, `.htm` | Built-in | Tag stripping, heading/list/table extraction |
| CSV | `.csv`, `.tsv` | Built-in | Auto-detects delimiter (comma, tab, semicolon, pipe) |
| DOCX | `.docx` | ZIP + XML | Microsoft Word, requires `document` feature |
| XLSX | `.xlsx` | Calamine | Microsoft Excel, requires `document-xlsx` feature |

## Distillation Levels

Distillation compresses document content by removing filler phrases and low-information-density content:

| Level | Description | Use Case |
|-------|-------------|----------|
| `minimal` | Light cleanup, preserves most content | When full fidelity is needed |
| `balanced` | Removes filler phrases, moderate compression | Default, good for most use cases |
| `aggressive` | Heavy compression, keeps only high-density content | Tight token budgets |

## PII Detection

The PII scanner detects and optionally redacts:

- **Social Security Numbers** — Validated format (excludes invalid area codes 000, 666, 900+)
- **Credit Card Numbers** — Luhn-validated (Visa, Mastercard, Amex, Discover)
- **Email Addresses** — Standard email patterns
- **Phone Numbers** — US formats with separators (avoids false positives from version numbers)
- **IP Addresses** — IPv4 with octet validation (0-255)

```bash
# Scan only (reports findings to stderr)
infiniloom ingest doc.md --pii-scan

# Scan and redact
infiniloom ingest doc.md --redact-pii -o clean.xml
```

## Chunking

Split large documents into chunks for multi-turn LLM conversations:

```bash
# Default chunking (4000 tokens per chunk, 200 token overlap)
infiniloom ingest doc.md --chunk

# Custom chunk sizes
infiniloom ingest doc.md --chunk --max-chunk-tokens 8000 --overlap-tokens 500

# Chunked output to file
infiniloom ingest doc.md --chunk -o chunks.xml
```

## Output Formats

### XML (default)

Claude-optimized XML with structured sections:

```xml
<document title="Report">
  <section level="1" title="Introduction">
    <paragraph>Content here...</paragraph>
  </section>
</document>
```

### Markdown

GPT-optimized Markdown preserving heading hierarchy:

```markdown
# Introduction

Content here...
```

### JSON

Structured JSON for programmatic consumption:

```json
{
  "title": "Report",
  "sections": [
    {
      "level": 1,
      "title": "Introduction",
      "content": [{"type": "paragraph", "text": "Content here..."}]
    }
  ]
}
```

## Feature Flags

The document ingestion module is gated behind Cargo feature flags:

| Feature | Default | Dependencies | Description |
|---------|---------|-------------|-------------|
| `document` | Yes | `zip`, `quick-xml` | Core document ingestion (MD, HTML, CSV, DOCX) |
| `document-xlsx` | No | `calamine` | XLSX spreadsheet support |

To disable document support entirely, build without default features:

```bash
cargo build --release --no-default-features
```

To enable XLSX support:

```bash
cargo build --release --features document-xlsx
```

## Examples

### Convert a project's README for AI analysis

```bash
infiniloom ingest README.md -f xml -o readme-context.xml
```

### Process a business document with PII protection

```bash
infiniloom ingest report.docx --redact-pii -m claude -o safe-report.xml
```

### Chunk a large document for multi-turn conversation

```bash
infiniloom ingest whitepaper.md --chunk --max-chunk-tokens 6000 -f markdown
```

### Process a CSV data file

```bash
infiniloom ingest data.csv -f json -d aggressive -o data-summary.json
```

## See Also

- [pack](pack.md) — Generate context from code repositories
- [embed](embed.md) — Generate chunks for vector databases
- [Configuration Guide](../CONFIGURATION.md) — Feature flag configuration
