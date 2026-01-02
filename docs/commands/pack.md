# `infiniloom pack` Command

## Overview

The `pack` command is the primary command for transforming a repository into LLM-optimized context. It scans a codebase, extracts metadata and content, applies transformations, and outputs in a format optimized for specific AI models.

## Synopsis

```bash
infiniloom pack [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

---

## Quick Examples

### Basic Usage

```bash
# Pack current directory to stdout (Claude-optimized XML)
infiniloom pack

# Pack specific directory to file
infiniloom pack /path/to/repo -o context.xml

# Pack with Markdown format for GPT
infiniloom pack -f markdown -m gpt4o -o context.md
```

### Token Budget Control

```bash
# Limit to 50,000 tokens
infiniloom pack --max-tokens 50000

# Use TOON format for maximum efficiency
infiniloom pack -f toon --max-tokens 30000
```

### Security-Conscious Usage

```bash
# Scan and report secrets
infiniloom pack --security-check

# Redact secrets in output
infiniloom pack --redact-secrets -o safe-context.xml
```

### Remote Repository

```bash
# Pack a GitHub repository
infiniloom pack github:user/repo

# Pack specific branch
infiniloom pack github:user/repo --remote-branch develop

# Sparse checkout for monorepo
infiniloom pack github:large/monorepo --sparse-path packages/core
```

### Advanced Analysis

```bash
# Full analysis with PageRank ranking
infiniloom pack --full -o context.xml

# Include git history
infiniloom pack --include-logs --logs-count 100
```

### Incremental Workflow

```bash
# First run (builds cache)
infiniloom pack --cache -o context.xml

# Subsequent runs (fast, uses cache)
infiniloom pack --cache -o context.xml

# Watch mode for continuous updates
infiniloom pack --cache --watch -o context.xml
```

### Filtering Files

```bash
# Only Rust and Python files
infiniloom pack --include "*.rs" --include "*.py"

# Exclude tests and vendor
infiniloom pack --exclude "tests/*" --exclude "vendor/*"

# Include test files (excluded by default)
infiniloom pack --include-tests
```

---

## Description

The `pack` command performs the following operations:

1. **Repository Scanning**: Walks the directory tree respecting `.gitignore` patterns
2. **Language Detection**: Identifies programming languages from file extensions and special filenames
3. **Content Processing**: Reads file contents, optionally extracts symbols via Tree-sitter AST parsing
4. **Token Counting**: Calculates accurate token counts using tiktoken (OpenAI) or calibrated estimation (other models)
5. **Security Scanning**: Optionally detects and redacts secrets (API keys, tokens, private keys)
6. **Compression**: Applies content transformations based on compression level
7. **Output Generation**: Formats the repository in model-optimized format (XML, Markdown, JSON, YAML, TOON, Plain)
8. **Budget Enforcement**: Truncates output to stay within token budget if specified

---

## Options

### Input/Output Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--output <PATH>` | `-o` | Write output to file instead of stdout | stdout |
| `--format <FORMAT>` | `-f` | Output format: `xml`, `markdown`, `json`, `yaml`, `toon`, `plain` | `xml` |
| `--model <MODEL>` | `-m` | Target model for token counting optimization | `claude` |
| `--config <PATH>` | | Path to custom config file | `.infiniloom.yaml` |

### Token Budget Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--max-tokens <N>` | `-t`, `-b` | Maximum output tokens (0 = unlimited) | `0` |
| `--map-budget <N>` | | Token budget for repository map section | `2000` |

### Content Filtering Options

| Option | Description | Default |
|--------|-------------|---------|
| `--hidden` | Include hidden files (starting with `.`) | `false` |
| `--no-gitignore` | Don't respect `.gitignore` patterns | `false` |
| `--include <PATTERN>` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | Exclude files matching glob pattern (repeatable) | none |
| `--include-tests` | Include test files (normally excluded) | `false` |
| `--include-docs` | Include documentation files (normally excluded) | `false` |
| `--no-default-ignores` | Disable default ignore patterns (`node_modules`, `dist`, etc.) | `false` |
| `--stdin` | Read file paths from stdin (one per line) | `false` |
| `--top-files <N>` | Limit files in summary (0 = all) | `0` |

### Symbol & Analysis Options

| Option | Description | Default |
|--------|-------------|---------|
| `--symbols` | Enable Tree-sitter symbol extraction | `false` |
| `--full` | Enable full analysis (symbols + repo map + PageRank) | `false` |
| `--no-symbols` | Explicitly skip symbol extraction (overrides `--full`) | `false` |
| `--no-content` | Exclude file contents (metadata only) | `false` |

### Compression Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--compression <LEVEL>` | `-c` | Compression level (see below) | `balanced` |
| `--remove-empty-lines` | | Remove empty lines from code | `false` |
| `--remove-comments` | | Remove comments from code | `false` |
| `--truncate-base64` | | Truncate base64-encoded content | `false` |

**Compression Levels:**

| Level | Description | Reduction |
|-------|-------------|-----------|
| `none` | No compression, full content | 0% |
| `minimal` | Remove empty lines only | ~15% |
| `balanced` | Remove comments | ~35% |
| `aggressive` | Keep only function/class signatures | ~60% |
| `extreme` | Keep only key symbol names | ~80% |
| `focused` | Key symbols with small surrounding context | ~75% |
| `semantic` | Heuristic-based chunking (char frequency analysis) | ~65% |

### Output Formatting Options

| Option | Description | Default |
|--------|-------------|---------|
| `--line-numbers` | Enable line numbers in output | `true` |
| `--no-line-numbers` | Disable line numbers in output | `false` |
| `--no-directory-structure` | Hide directory tree from output | `false` |
| `--no-file-summary` | Hide file summary section | `false` |
| `--header-text <TEXT>` | Custom header text at top of output | none |
| `--instruction-file <PATH>` | File containing custom instructions to embed | none |
| `--token-tree` | Show token count breakdown by file | `false` |

### Security Options

| Option | Description | Default |
|--------|-------------|---------|
| `--security-check` | Scan for secrets and report findings | `false` |
| `--redact-secrets` | Replace detected secrets with `[REDACTED]` | `false` |

### Git Integration Options

| Option | Description | Default |
|--------|-------------|---------|
| `--include-logs` | Include git commit history | `false` |
| `--logs-count <N>` | Number of log entries to include | `50` |
| `--include-diffs` | Include git diff content | `false` |
| `--sort-by-changes` | Sort files by git change frequency | `false` |

### Remote Repository Options

| Option | Description | Default |
|--------|-------------|---------|
| `--remote-branch <BRANCH>` | Branch to checkout for remote repos | default |
| `--sparse-path <PATH>` | Sparse checkout paths (repeatable, for large monorepos) | all |

### Performance Options

| Option | Description | Default |
|--------|-------------|---------|
| `--cache` | Enable incremental caching | `false` |
| `--watch` | Watch for file changes and regenerate | `false` |
| `--copy-to-clipboard` | Copy output to system clipboard | `false` |
| `--verbose` | Show progress and detailed output | `false` |

## Supported Models

### OpenAI (Exact via tiktoken)

| Model | Encoding | Description |
|-------|----------|-------------|
| `gpt52` | o200k_base | GPT-5.2 |
| `gpt51` | o200k_base | GPT-5.1 |
| `gpt5` | o200k_base | GPT-5 |
| `o4-mini` | o200k_base | O4-mini reasoning model |
| `o3` | o200k_base | O3 reasoning model |
| `o1` | o200k_base | O1 reasoning model |
| `gpt4o` | o200k_base | GPT-4o |
| `gpt4o-mini` | o200k_base | GPT-4o-mini |
| `gpt4` | cl100k_base | GPT-4/GPT-4 Turbo (legacy) |
| `gpt35-turbo` | cl100k_base | GPT-3.5-turbo (legacy) |

### Other Vendors (Calibrated Estimation ~95% accuracy)

| Model | Vendor |
|-------|--------|
| `claude` | Anthropic |
| `gemini` | Google |
| `llama` | Meta |
| `codellama` | Meta |
| `mistral` | Mistral AI |
| `deepseek` | DeepSeek |
| `qwen` | Alibaba |
| `cohere` | Cohere |
| `grok` | xAI |

## Output Formats

### XML (Claude-Optimized)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<repository name="myproject" total_files="42" total_tokens="15000">
  <metadata>
    <languages>
      <language name="rust" files="20" percentage="47.6"/>
    </languages>
    <directory_structure>...</directory_structure>
  </metadata>
  <files>
    <file path="src/main.rs" language="rust" tokens="500">
      <content><![CDATA[
1:fn main() {
2:    println!("Hello");
3:}
      ]]></content>
    </file>
  </files>
</repository>
```

### Markdown (GPT-Optimized)

````markdown
# Repository: myproject

## Metadata
- Files: 42
- Tokens: 15,000

## Files

### src/main.rs
```rust
fn main() {
    println!("Hello");
}
```
````

### TOON (Token-Efficient)

Most compact format, ~40% smaller than XML:

```
@repo myproject
@files 42
@tokens 15000

@file src/main.rs
@lang rust
@tok 500
---
1:fn main() {
2:    println!("Hello");
3:}
---
```

### JSON/YAML

Structured data format with full metadata, suitable for programmatic consumption.

## Technical Implementation

### Scanning Pipeline

```
Directory Walk (ignore crate)
    ↓
Language Detection (extension + filename patterns)
    ↓
Binary File Filtering (check first 8KB for null bytes)
    ↓
Content Reading (std::fs or mmap for files ≥1MB)
    ↓
Parallel Processing (Rayon with thread-local parsers)
    ↓
Symbol Extraction (Tree-sitter AST, if enabled)
    ↓
Token Counting (tiktoken or estimation)
```

### Thread-Local Parser Architecture

To avoid mutex contention during parallel file processing, each Rayon worker thread maintains its own Tree-sitter parser instance:

```rust
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}
```

### Incremental Caching

When `--cache` is enabled:

1. Cache stored in `.infiniloom/cache.bin` (bincode format)
2. File entries keyed by relative path
3. Cache invalidation via mtime + size comparison (fast path)
4. Content hash comparison for accurate change detection (BLAKE3)
5. Symbols re-extracted only when `--symbols` flag changes

### Watch Mode Architecture

When `--watch` is enabled:

1. Uses `notify` crate for filesystem events
2. Debounces events (500ms threshold)
3. Full rescan on detected changes
4. Outputs to specified file (required for watch mode)
5. Skips changes to output file to avoid self-triggering

## Configuration File

Settings can be specified in `.infiniloom.yaml`:

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
  exclude:
    - "tests/*"
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  redact_secrets: true
  fail_on_secrets: false
  allowlist:
    - "EXAMPLE_KEY"
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error (invalid path, I/O error, etc.) |
| 1 | Secrets detected with `fail_on_secrets: true` in config |

## Performance Notes

- **Default mode** (no `--symbols`): ~80x faster, suitable for most use cases
- **Symbol mode** (`--symbols`): Enables PageRank ranking and better repo maps
- **Cache mode** (`--cache`): Significantly faster for repeated runs
- **Parallel processing**: Automatically scales to available CPU cores
- **Memory-mapped I/O**: Used for files ≥1MB for better performance

## See Also

- [`embed`](embed.md) - Generate chunks for vector databases (RAG)
- [`scan`](scan.md) - View repository statistics
- [`diff`](diff.md) - Get context for code changes
- [`chunk`](chunk.md) - Split repository for multi-turn conversations
- [Reference](../REFERENCE.md) - Complete command reference
- [Recipes](../RECIPES.md) - Ready-to-use code patterns
