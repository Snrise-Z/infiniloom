# Infiniloom Reference

**Complete reference for all commands, options, and workflows.**

---

## Quick Start (30 seconds)

```bash
# Generate AI-ready context (paste into Claude/GPT)
infiniloom pack .

# Generate chunks for vector databases (RAG)
infiniloom embed . -o chunks.jsonl

# View repository statistics
infiniloom scan .

# Get context for code changes
infiniloom index . && infiniloom diff .
```

---

## Commands at a Glance

| Command | Description | Primary Use |
|---------|-------------|-------------|
| `pack` | Transform repo into LLM context | AI assistants, code review |
| `embed` | Generate chunks for vector DBs | RAG pipelines, semantic search |
| `scan` | Show repository statistics | Pre-analysis, security audit |
| `map` | Generate PageRank symbol map | Understand codebase architecture |
| `index` | Build symbol index | Enable fast diff/impact queries |
| `diff` | Get context for code changes | Code review, debugging |
| `impact` | Analyze change impact | Pre-change analysis |
| `chunk` | Split repo for multi-turn | Large codebases |
| `ingest` | Convert documents to LLM format | Document ingestion (MD, HTML, CSV, DOCX, XLSX) |
| `init` | Create configuration file | Project setup |
| `info` | Show version and config | Debug, discovery |

---

## pack - Generate AI Context

Transform repository into LLM-optimized format.

### Basic Usage

```bash
infiniloom pack .                              # Current directory, defaults
infiniloom pack /path/to/repo                  # Specific path
infiniloom pack . --output context.xml         # Save to file
infiniloom pack . | pbcopy                     # Copy to clipboard (macOS)
infiniloom pack github:facebook/react          # Remote repository
```

### Output Formats (`--format`, `-f`)

```bash
infiniloom pack . --format xml         # Claude (default, prompt caching)
infiniloom pack . --format markdown    # GPT-4o/GPT-5
infiniloom pack . --format yaml        # Gemini
infiniloom pack . --format json        # Programmatic use
infiniloom pack . --format toon        # Token-efficient (~40% smaller)
infiniloom pack . --format plain       # Simple plain text
```

### Model Selection (`--model`, `-m`)

```bash
infiniloom pack . --model claude       # Anthropic Claude (default)
infiniloom pack . --model gpt4o        # OpenAI GPT-4o
infiniloom pack . --model gpt5         # OpenAI GPT-5
infiniloom pack . --model o3           # OpenAI O3
infiniloom pack . --model gemini       # Google Gemini
infiniloom pack . --model llama        # Meta Llama
infiniloom pack . --model deepseek     # DeepSeek
```

### Compression Levels (`--compression`, `-c`)

| Level | Reduction | Description |
|-------|-----------|-------------|
| `none` | 0% | Full content |
| `minimal` | ~15% | Remove empty lines |
| `balanced` | ~35% | Remove comments (default) |
| `aggressive` | ~60% | Signatures only |
| `extreme` | ~80% | Key symbols only |
| `focused` | ~75% | Key symbols + context |
| `semantic` | ~65% | Smart compression |

```bash
infiniloom pack . --compression balanced   # Default
infiniloom pack . --compression aggressive # For tight budgets
```

### Filtering

```bash
# Include/exclude patterns
infiniloom pack . --include "*.rs"
infiniloom pack . --include "src/**/*.ts" --include "lib/**/*.ts"
infiniloom pack . --exclude "tests/*" --exclude "docs/*"
infiniloom pack . --exclude "**/*.test.*" --exclude "**/*.spec.*"

# Token limits
infiniloom pack . --max-tokens 50000       # Limit output tokens
infiniloom pack . --top-files 50           # Limit to N most important files

# Content types
infiniloom pack . --include-tests          # Include test files
infiniloom pack . --include-docs           # Include documentation
infiniloom pack . --hidden                 # Include hidden files
infiniloom pack . --no-gitignore           # Ignore .gitignore
```

### Git Integration

```bash
infiniloom pack . --include-logs           # Include commit history
infiniloom pack . --include-logs --logs-count 20
infiniloom pack . --include-diffs          # Include uncommitted changes
infiniloom pack . --sort-by-changes        # Sort by change frequency
```

### Security

```bash
infiniloom pack . --security-check         # Scan for secrets
infiniloom pack . --redact-secrets         # Redact detected secrets
```

### Advanced Options

```bash
infiniloom pack . --symbols                # Extract AST symbols
infiniloom pack . --full                   # Full analysis (symbols + map + ranking)
infiniloom pack . --no-content             # Metadata only, no file contents
infiniloom pack . --no-line-numbers        # Disable line numbers
infiniloom pack . --remove-comments        # Strip all comments
infiniloom pack . --remove-empty-lines     # Remove blank lines
infiniloom pack . --header-text "Context"  # Add custom header
infiniloom pack . --watch                  # Regenerate on file changes
infiniloom pack . --cache                  # Enable incremental caching
```

### Remote Repositories

```bash
infiniloom pack github:owner/repo
infiniloom pack github:owner/repo --remote-branch develop
infiniloom pack github:owner/repo --sparse-path src --sparse-path lib
```

---

## embed - Generate RAG Chunks

Generate content-addressable chunks for vector databases and RAG systems.

### Basic Usage

```bash
infiniloom embed .                             # Current directory, jsonl output
infiniloom embed /path/to/repo -o chunks.json  # Specific path, JSON output
infiniloom embed . --format json               # JSON array format
infiniloom embed . -v                          # Verbose output
```

### Incremental Updates

```bash
# First run creates manifest (.infiniloom-embed.bin)
infiniloom embed -o chunks.json

# Subsequent runs only output changed chunks
infiniloom embed --diff -o changes.json
```

### Token Control

```bash
infiniloom embed --max-tokens 1500             # For voyage-code-2/3
infiniloom embed --max-tokens 800              # For openai embeddings
infiniloom embed --min-tokens 100              # Merge small chunks
infiniloom embed --context-lines 10            # More context around symbols
infiniloom embed --token-model gpt4o           # Token counting model
```

### Filtering

```bash
infiniloom embed -i "*.py" -o python.json      # Python only
infiniloom embed -e "tests/*" -e "docs/*"      # Exclude patterns
infiniloom embed --include-tests               # Include test files
infiniloom embed --no-imports                  # Exclude import chunks
infiniloom embed --no-top-level                # Exclude top-level code
```

### Chunk Output Format

```json
{
  "id": "ec_a1b2c3d4e5f6...",
  "content": "fn foo() {...}",
  "tokens": 150,
  "kind": "function",
  "source": {
    "file": "src/main.rs",
    "symbol": "foo",
    "fqn": "src::main::foo",
    "lines": [10, 25],
    "language": "Rust"
  },
  "context": {
    "docstring": "Does something...",
    "calls": ["bar", "baz"],
    "called_by": ["main"],
    "tags": ["async", "public-api"]
  }
}
```

---

## scan - Repository Statistics

Show repository statistics and token counts.

```bash
infiniloom scan .                          # Basic scan
infiniloom scan . --model gpt4o            # Token count for specific model
infiniloom scan . --verbose                # Show file list
infiniloom scan . --json                   # JSON output
infiniloom scan . --security-check         # Include security scan
infiniloom scan . --sample 500             # Sample N random files
infiniloom scan . --include "src/**"       # Filter files
infiniloom scan . --exclude "vendor/*"     # Exclude patterns
infiniloom scan . --include-tests          # Include test files
```

---

## map - PageRank Symbol Map

Generate PageRank-ranked symbol map.

```bash
infiniloom map .                           # Default 2000 token budget
infiniloom map . --budget 5000             # Larger budget
infiniloom map . --model gpt4o             # Specific model
infiniloom map . --output map.txt          # Save to file
infiniloom map . --verbose                 # Detailed output
infiniloom map . --include "src/**"        # Filter files
infiniloom map . --include-tests           # Include test files
```

---

## index - Build Symbol Index

Build symbol index for fast diff/impact queries.

```bash
infiniloom index .                         # Build index
infiniloom index . --status                # Show index status
infiniloom index . --force                 # Force full rebuild
infiniloom index . --incremental           # Only re-index changed files
infiniloom index . --watch                 # Auto-rebuild on changes
infiniloom index . --verbose               # Detailed output
infiniloom index . --include "src/**"      # Filter files
infiniloom index . --include-tests         # Include test files
```

---

## diff - Change Context

Get context for code changes (requires index).

```bash
# Change sources
infiniloom diff .                          # Unstaged changes
infiniloom diff . --staged                 # Staged changes only
infiniloom diff . HEAD~1                   # Last commit
infiniloom diff . HEAD~5                   # Last 5 commits
infiniloom diff . main..feature            # Branch comparison
infiniloom diff . abc123                   # Specific commit

# Options
infiniloom diff . --include-diff           # Include +/- lines
infiniloom diff . --depth 1                # Containing files only
infiniloom diff . --depth 2                # + Direct deps (default)
infiniloom diff . --depth 3                # + Transitive deps
infiniloom diff . --budget 80000           # Token budget
infiniloom diff . --format markdown        # Output format
infiniloom diff . --output diff.xml        # Save to file
infiniloom diff . --include-history        # Include file commit history
infiniloom diff . --include-tests          # Include test files
```

---

## impact - Change Impact Analysis

Analyze change impact (requires index).

```bash
# File impact
infiniloom impact . src/auth.rs            # What depends on this file?
infiniloom impact . src/core/engine.py     # Full path

# Symbol impact
infiniloom impact . --symbol "authenticate"
infiniloom impact . --symbol "UserService"

# Options
infiniloom impact . src/auth.rs --depth 1  # Direct deps only
infiniloom impact . src/auth.rs --depth 2  # Default
infiniloom impact . src/auth.rs --depth 3  # Full transitive
infiniloom impact . --call-graph           # Show call graph
infiniloom impact . src/auth.rs --json     # JSON output
infiniloom impact . --include-tests        # Include test files
```

---

## chunk - Multi-Turn Splitting

Split repository for multi-turn conversations.

```bash
# Strategies
infiniloom chunk . --strategy semantic     # Group by similarity (default)
infiniloom chunk . --strategy module       # Group by directory
infiniloom chunk . --strategy dependency   # Group by imports
infiniloom chunk . --strategy file         # One file per chunk
infiniloom chunk . --strategy symbol       # Group by AST symbols
infiniloom chunk . --strategy fixed        # Fixed token size

# Options
infiniloom chunk . --max-tokens 50000      # Tokens per chunk
infiniloom chunk . --overlap 2000          # Token overlap between chunks
infiniloom chunk . --format markdown       # Output format
infiniloom chunk . --output chunks/        # Save to directory
infiniloom chunk . --priority-first        # Most important chunks first
infiniloom chunk . --include-tests         # Include test files
```

---

## ingest - Document Ingestion

Convert documents (Markdown, HTML, CSV, DOCX, XLSX) to LLM-optimized formats with optional PII detection, distillation, and chunking.

```bash
# Basic usage
infiniloom ingest report.md                    # Markdown → XML (default)
infiniloom ingest page.html -f markdown        # HTML → Markdown
infiniloom ingest data.csv -f json             # CSV → JSON
infiniloom ingest report.docx -o output.xml    # DOCX → XML file

# Distillation levels
infiniloom ingest doc.md -d minimal            # Light compression
infiniloom ingest doc.md -d balanced           # Default
infiniloom ingest doc.md -d aggressive         # Heavy compression

# PII detection and redaction
infiniloom ingest doc.md --pii-scan            # Scan and report PII
infiniloom ingest doc.md --redact-pii          # Redact PII in output

# Chunking for multi-turn conversations
infiniloom ingest doc.md --chunk               # Split into chunks
infiniloom ingest doc.md --chunk --max-chunk-tokens 8000
infiniloom ingest doc.md --chunk --overlap-tokens 500

# Token budget warning
infiniloom ingest doc.md --max-tokens 50000 -m claude

# Verbose output
infiniloom ingest doc.md -v
```

### Supported Formats

| Format | Extensions | Notes |
|--------|-----------|-------|
| Markdown | `.md`, `.markdown` | CommonMark + GFM tables |
| HTML | `.html`, `.htm` | Strips tags, preserves structure |
| CSV | `.csv`, `.tsv` | Auto-detects delimiter |
| DOCX | `.docx` | Microsoft Word (requires `document` feature) |
| XLSX | `.xlsx` | Microsoft Excel (requires `document-xlsx` feature) |

---

## init & info

```bash
# Create configuration file
infiniloom init                            # Create .infiniloom.yaml
infiniloom init --format toml              # Create .infiniloom.toml
infiniloom init --template rust            # Rust project template
infiniloom init --template python          # Python project template
infiniloom init --force                    # Overwrite existing

# Show version and configuration
infiniloom info                            # General info
infiniloom info .                          # Include project-specific info
infiniloom --version                       # Version only
infiniloom pack --help                     # Help for specific command
```

---

## Common Workflows

### AI Code Review

```bash
# One-time: Build index
infiniloom index .

# Before PR: Get context for staged changes
infiniloom diff . --staged --include-diff --format markdown > review.md

# Or pipe directly
infiniloom diff . --staged --include-diff | pbcopy
```

### RAG Pipeline

```bash
# Generate chunks for vector database
infiniloom embed . -o chunks.jsonl

# Incremental updates (only changed)
infiniloom embed . --diff -o updates.jsonl

# Optimized for Voyage embeddings
infiniloom embed . --max-tokens 1500 -o chunks.jsonl
```

### Large Repository

```bash
# Step 1: Scan to understand size
infiniloom scan .

# Step 2: Generate map for overview
infiniloom map . --budget 5000

# Step 3: Pack focused content
infiniloom pack . \
  --include "src/**" \
  --exclude "tests/*" \
  --compression balanced \
  --max-tokens 80000
```

### Multi-Turn Conversation

```bash
# Split into digestible chunks
infiniloom chunk . --strategy module --max-tokens 50000

# Send chunk 1, then chunk 2, etc.
```

### Security Audit

```bash
# Scan only
infiniloom scan . --security-check

# Pack with secrets redacted
infiniloom pack . --redact-secrets --output context.xml
```

---

## Configuration

### Config File (`.infiniloom.yaml`)

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000
  line_numbers: true
  show_file_summary: true

scan:
  include:
    - "*.rs"
    - "*.py"
    - "*.ts"
  exclude:
    - "tests/*"
    - "docs/*"
    - "node_modules/*"
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  redact_secrets: true
  fail_on_secrets: false
  allowlist:
    - "EXAMPLE"
    - "placeholder"
```

### Environment Variables

```bash
INFINILOOM_OUTPUT__MODEL=claude
INFINILOOM_OUTPUT__FORMAT=xml
INFINILOOM_OUTPUT__COMPRESSION=balanced
INFINILOOM_OUTPUT__TOKEN_BUDGET=100000
INFINILOOM_SCAN__INCLUDE_HIDDEN=false
INFINILOOM_SCAN__RESPECT_GITIGNORE=true
INFINILOOM_SECURITY__SCAN_SECRETS=true
INFINILOOM_SECURITY__REDACT_SECRETS=true
```

---

## Output Format Summary

| Format | Best For | Token Efficiency |
|--------|----------|------------------|
| `xml` | Claude | Baseline (prompt caching) |
| `markdown` | GPT-4o/GPT-5 | ~10% more tokens |
| `yaml` | Gemini | ~12% more tokens |
| `json` | Programmatic | ~15% more tokens |
| `toon` | Any (max efficiency) | ~40% fewer tokens |
| `plain` | Simple use | ~5% fewer tokens |

---

## Model Aliases

| CLI Value | Full Model | Token Counting |
|-----------|------------|----------------|
| `claude` | Claude 3.5/4 | Estimated (~95%) |
| `gpt52` | GPT-5.2 | Exact (tiktoken) |
| `gpt5` | GPT-5 | Exact (tiktoken) |
| `gpt4o` | GPT-4o | Exact (tiktoken) |
| `gpt4` | GPT-4 | Exact (tiktoken) |
| `o3` | O3 | Exact (tiktoken) |
| `o1` | O1 | Exact (tiktoken) |
| `gemini` | Gemini (all versions including 3.1) | Estimated (~95%) |
| `llama` | Llama 3/4 | Estimated (~95%) |
| `deepseek` | DeepSeek V3/R1 | Estimated (~95%) |
| `mistral` | Mistral | Estimated (~95%) |
| `qwen` | Qwen | Estimated (~95%) |
| `cohere` | Command R+ | Estimated (~95%) |
| `grok` | Grok | Estimated (~95%) |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (invalid path, I/O error, validation failure) |

---

## See Also

- [Command Documentation](commands/) - Detailed docs for each command
- [Configuration Guide](CONFIGURATION.md) - All config options
- [Recipes](RECIPES.md) - Ready-to-use code patterns
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues
- [FAQ](FAQ.md) - Frequently asked questions
