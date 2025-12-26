# Infiniloom Cheat Sheet

Quick reference for all Infiniloom commands, options, and common workflows.

## Commands at a Glance

```
infiniloom pack    - Transform repository into LLM context
infiniloom scan    - Show repository statistics
infiniloom map     - Generate PageRank symbol map
infiniloom index   - Build symbol index for fast queries
infiniloom diff    - Get context for code changes
infiniloom impact  - Analyze change impact
infiniloom chunk   - Split repository for multi-turn
infiniloom init    - Create configuration file
infiniloom info    - Show version and config
```

---

## pack

Transform repository into LLM-optimized format.

### Basic Usage

```bash
infiniloom pack .                              # Current directory, defaults
infiniloom pack /path/to/repo                  # Specific path
infiniloom pack . --output context.xml         # Save to file
infiniloom pack . | pbcopy                     # Copy to clipboard (macOS)
infiniloom pack github:facebook/react          # Remote repository
```

### Output Formats

```bash
infiniloom pack . --format xml         # Claude (default, prompt caching)
infiniloom pack . --format markdown    # GPT-4o/GPT-5
infiniloom pack . --format yaml        # Gemini
infiniloom pack . --format json        # Programmatic use
infiniloom pack . --format toon        # Token-efficient (~40% smaller)
infiniloom pack . --format plain       # Simple plain text
```

### Model Selection

```bash
infiniloom pack . --model claude       # Anthropic Claude (default)
infiniloom pack . --model gpt4o        # OpenAI GPT-4o
infiniloom pack . --model gpt5         # OpenAI GPT-5
infiniloom pack . --model o3           # OpenAI O3
infiniloom pack . --model gemini       # Google Gemini
infiniloom pack . --model llama        # Meta Llama
infiniloom pack . --model deepseek     # DeepSeek
```

### Compression Levels

```bash
infiniloom pack . --compression none       # Full content (0%)
infiniloom pack . --compression minimal    # Remove empty lines (10-20%)
infiniloom pack . --compression balanced   # Remove comments (30-40%)
infiniloom pack . --compression aggressive # Remove docstrings (50-60%)
infiniloom pack . --compression extreme    # Signatures only (70-80%)
infiniloom pack . --compression focused    # Key symbols + context (~75%)
infiniloom pack . --compression semantic   # Smart compression (60-70%)
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

## scan

Show repository statistics and token counts.

```bash
infiniloom scan .                          # Basic scan
infiniloom scan . --model gpt4o            # Token count for specific model
infiniloom scan . --verbose                # Show file list
infiniloom scan . --json                   # JSON output
infiniloom scan . --security-check         # Include security scan
infiniloom scan . --sample 500             # Sample N random files
infiniloom scan . --sample-percent 5       # Sample N% of files
infiniloom scan . --include "src/**"       # Filter files
infiniloom scan . --exclude "vendor/*"     # Exclude patterns
infiniloom scan . --include-tests          # Include test files
```

---

## map

Generate PageRank-ranked symbol map.

```bash
infiniloom map .                           # Default 2000 token budget
infiniloom map . --budget 5000             # Larger budget
infiniloom map . --model gpt4o             # Specific model
infiniloom map . --output map.txt          # Save to file
infiniloom map . --verbose                 # Detailed output
infiniloom map . --include "src/**"        # Filter files
infiniloom map . --exclude "tests/*"       # Exclude patterns
infiniloom map . --include-tests           # Include test files
```

---

## index

Build symbol index for fast diff/impact queries.

```bash
infiniloom index .                         # Build index
infiniloom index . --status                # Show index status
infiniloom index . --force                 # Force full rebuild
infiniloom index . --incremental           # Only re-index changed files
infiniloom index . --watch                 # Auto-rebuild on changes
infiniloom index . --verbose               # Detailed output
infiniloom index . --include "src/**"      # Filter files
infiniloom index . --exclude "vendor/*"    # Exclude patterns
infiniloom index . --include-tests         # Include test files
```

---

## diff

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
infiniloom diff . --model gpt4o            # Token counting model
infiniloom diff . --include-history        # Include file commit history
infiniloom diff . --history-count 5        # Number of commits per file
infiniloom diff . --verbose                # Detailed output
infiniloom diff . --include "src/**"       # Filter files
infiniloom diff . --exclude "vendor/*"     # Exclude patterns
infiniloom diff . --include-tests          # Include test files
```

---

## impact

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
infiniloom impact . --verbose              # Detailed output
infiniloom impact . --model gpt4o          # Token counting model
infiniloom impact . --include "src/**"     # Filter files
infiniloom impact . --exclude "vendor/*"   # Exclude patterns
infiniloom impact . --include-tests        # Include test files
```

---

## chunk

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
infiniloom chunk . --model gpt4o           # Token counting model
infiniloom chunk . --format markdown       # Output format
infiniloom chunk . --output chunks/        # Save to directory
infiniloom chunk . --priority-first        # Most important chunks first
infiniloom chunk . --no-chunk-summary      # Disable chunk summaries
infiniloom chunk . --verbose               # Detailed output
infiniloom chunk . --include "src/**"      # Filter files
infiniloom chunk . --exclude "vendor/*"    # Exclude patterns
infiniloom chunk . --include-tests         # Include test files
```

---

## init

Create configuration file.

```bash
infiniloom init                            # Create .infiniloom.yaml
infiniloom init --format toml              # Create .infiniloom.toml
infiniloom init --format json              # Create .infiniloom.json
infiniloom init --template rust            # Rust project template
infiniloom init --template python          # Python project template
infiniloom init --template node            # Node.js project template
infiniloom init --template generic         # Generic template (default)
infiniloom init --output custom.yaml       # Custom output path
infiniloom init --force                    # Overwrite existing
```

---

## info

Show version and configuration.

```bash
infiniloom info                            # General info
infiniloom info .                          # Include project-specific info
infiniloom --version                       # Version only
infiniloom --help                          # Help for all commands
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

### Remote Repository Analysis

```bash
# Sparse checkout for large repos
infiniloom pack github:torvalds/linux --sparse-path kernel/sched

# Specific branch
infiniloom pack github:owner/repo --remote-branch develop
```

---

## Environment Variables

```bash
INFINILOOM_OUTPUT__MODEL=claude            # Default model
INFINILOOM_OUTPUT__FORMAT=xml              # Default format
INFINILOOM_OUTPUT__COMPRESSION=balanced    # Default compression
INFINILOOM_OUTPUT__TOKEN_BUDGET=100000     # Default token budget
INFINILOOM_SCAN__INCLUDE_HIDDEN=false      # Include hidden files
INFINILOOM_SCAN__RESPECT_GITIGNORE=true    # Respect .gitignore
INFINILOOM_SECURITY__SCAN_SECRETS=true     # Enable security scanning
INFINILOOM_SECURITY__REDACT_SECRETS=true   # Redact secrets in output
```

---

## Configuration File

```yaml
# .infiniloom.yaml
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
  custom_patterns:
    - "MY_SECRET_[A-Z0-9]{32}"
```

---

## Output Format Summary

| Format | Best For | Optimizations |
|--------|----------|---------------|
| `xml` | Claude | Prompt caching, CDATA sections |
| `markdown` | GPT-4o/GPT-5 | Tables, headers, code fences |
| `yaml` | Gemini | Structured hierarchy |
| `json` | Programmatic | Standard parsing |
| `toon` | Limited context | ~40% smaller |
| `plain` | Simple use | No markup |

---

## Model Aliases

| CLI Value | Full Model |
|-----------|------------|
| `claude` | Claude 3.5 Sonnet / Opus |
| `gpt52` | GPT-5.2 |
| `gpt5` | GPT-5 |
| `gpt4o` | GPT-4o |
| `gpt4` | GPT-4 |
| `o3` | O3 |
| `o1` | O1 |
| `gemini` | Gemini 1.5 Pro |
| `llama` | Llama 3/4 |
| `codellama` | CodeLlama |
| `deepseek` | DeepSeek V3/R1 |
| `mistral` | Mistral |
| `qwen` | Qwen |
| `cohere` | Command R+ |
| `grok` | Grok |

---

## See Also

- [Command Reference](commands/) - Detailed documentation for each command
- [Configuration Guide](CONFIGURATION.md) - All configuration options
- [LLM Optimization](guides/llm-optimization.md) - Model-specific tips
- [Large Repos Guide](guides/large-repos.md) - Scaling strategies
- [CI Integration](guides/ci-integration.md) - Automation workflows
