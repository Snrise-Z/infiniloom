# `infiniloom scan` Command

## Overview

The `scan` command analyzes a repository and displays comprehensive statistics including file counts, language distribution, accurate token counts, and optional security findings. It's useful for understanding repository size before packing or for quick security audits.

## Synopsis

```bash
infiniloom scan [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `scan` command performs:

1. **Directory Walking**: Traverses the repository respecting `.gitignore`
2. **Language Detection**: Identifies languages from extensions and filenames
3. **Content Reading**: Reads all files for accurate token counting
4. **Token Counting**: Uses tiktoken (OpenAI) or calibrated estimation for accurate counts
5. **Security Scanning**: Optionally detects potential secrets
6. **Statistics Aggregation**: Computes totals by language and overall

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--model <MODEL>` | `-m` | Target model for token counting | `claude` |
| `--hidden` | | Include hidden files | `false` |
| `--verbose` | `-v` | Show detailed file list (top 20 by size) | `false` |
| `--json` | | Output statistics as JSON | `false` |
| `--security-check` | | Scan for secrets and API keys | `false` |
| `--sample <N>` | | Sample N random files for estimation | (disabled) |
| `--sample-percent <P>` | | Sample P% of files for estimation | (disabled) |
| `--include <PATTERN>` | `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | `-e` | Exclude files/directories matching pattern (repeatable) | none |
| `--include-tests` | | Include test files in scan (normally excluded) | `false` |

## Output

### Human-Readable Output (Default)

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Scan Results
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Repository:   infiniloom
  Path:         /Users/dev/infiniloom
  Files:        127
  Total Size:   1.2 MiB
  Scan Time:    234ms

  Languages:
    rust: 89 files (70.1%)
    toml: 12 files (9.4%)
    markdown: 15 files (11.8%)
    yaml: 8 files (6.3%)
    json: 3 files (2.4%)

  Token Counts (claude):
    Total: 156,432

  Security Scan:
    ✓ No secrets detected
```

### JSON Output (`--json`)

```json
{
  "repository": "infiniloom",
  "files": 127,
  "total_bytes": 1258291,
  "total_tokens": {
    "claude": 156432,
    "gpt4o": 148521,
    "gemini": 152103
  },
  "languages": [
    { "language": "rust", "files": 89, "lines": 15234, "percentage": 70.1 }
  ],
  "scan_time_ms": 234,
  "security": {
    "issues_found": 0,
    "issues": []
  }
}
```

## Technical Implementation

### Token Counting Pipeline

Unlike quick heuristic estimation, the scan command reads actual file content for accurate counting:

```rust
// Uses real tokenizer, not just byte-based estimation
let tokenizer = Tokenizer::new();
for file in &mut repo.files {
    if let Some(ref content) = file.content {
        let counts = tokenizer.count_all(content);
        file.token_count = counts;
    }
}
```

### Accuracy by Model

| Model Family | Method | Accuracy |
|--------------|--------|----------|
| OpenAI (o200k_base) | tiktoken BPE | 100% exact |
| OpenAI (cl100k_base) | tiktoken BPE | 100% exact |
| Claude | Calibrated estimation | ~95% |
| Gemini | Calibrated estimation | ~95% |
| Others | Calibrated estimation | ~90-95% |

### Security Scanner

The security scanner uses pre-compiled regex patterns to detect:

| Pattern Type | Examples |
|--------------|----------|
| AWS Keys | `AKIA*`, `aws_secret_access_key` |
| GitHub Tokens | `ghp_*`, `github_pat_*` |
| Private Keys | `-----BEGIN RSA PRIVATE KEY-----` |
| JWT Tokens | `eyJ*` patterns |
| Generic API Keys | `api_key=*`, `apikey:*` |
| High Entropy Strings | Potential passwords/tokens |

## Examples

### Basic Scan

```bash
# Scan current directory
infiniloom scan

# Scan specific repository
infiniloom scan /path/to/repo
```

### Token Budget Planning

```bash
# Check token counts for GPT-4o
infiniloom scan -m gpt4o

# JSON output for scripting
infiniloom scan --json | jq '.total_tokens.gpt4o'
```

### Security Audit

```bash
# Quick security scan
infiniloom scan --security-check

# JSON security report
infiniloom scan --security-check --json > security-report.json
```

### Verbose Analysis

```bash
# Show largest files
infiniloom scan -v
```

### Sampling Mode (Large Repositories)

For very large repositories with 100K+ files, use sampling to get fast approximate results:

```bash
# Sample 100 random files
infiniloom scan --sample 100

# Sample 1% of files
infiniloom scan --sample-percent 1

# Sample with JSON output for analysis
infiniloom scan --sample 500 --json
```

**Sampled Output Example:**

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Scan Results [ESTIMATED]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Repository:   large-monorepo
  Path:         /path/to/repo
  Files:        ~125,000 (sampled 500)
  Total Size:   ~1.5 GiB
  Scan Time:    1.2s

  Token Counts (claude) [ESTIMATED]:
    Total: ~15,643,200

  Note: Statistics extrapolated from 500-file sample (0.4%)
```

**JSON Output with Sampling:**

```json
{
  "repository": "large-monorepo",
  "files": 125000,
  "total_tokens": {
    "claude": 15643200
  },
  "is_estimated": true,
  "sample_size": 500,
  "extrapolation_factor": 250.0
}
```

## Best Practices for LLM Context

### Pre-Pack Analysis

Before packing a repository, use `scan` to determine:

1. **Total token count** - Ensure it fits within model context window
2. **Language distribution** - Identify dominant languages for format optimization
3. **Security issues** - Address secrets before sharing with LLMs

```bash
# Check if repo fits in Claude's 200K context
infiniloom scan -m claude --json | jq '.total_tokens.claude'
```

### CI/CD Integration

```bash
# Fail if secrets are detected
infiniloom scan --security-check --json | jq -e '.security.issues_found == 0'
```

## Performance Optimizations

### Current Implementation

- **Parallel file reading**: Uses Rayon for concurrent I/O
- **Skip symbols**: No Tree-sitter parsing (faster than `pack --symbols`)
- **Streaming stats**: Aggregates while scanning

### Potential Improvements

1. **Cached token counts**: Reuse counts from previous scans
   ```bash
   # Future: infiniloom scan --cache
   ```

2. **Incremental scanning**: Only rescan changed files
   ```bash
   # Future: infiniloom scan --incremental
   ```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Path not found or I/O error |

## Related Commands

- [`pack`](pack.md) - Generate full LLM context
- [`map`](map.md) - Generate symbol-ranked repository map
- [`info`](info.md) - Show tool configuration
