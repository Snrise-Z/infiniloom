# Configuration Guide

Infiniloom supports configuration via config files, environment variables, and CLI flags. Settings are applied in this order (later overrides earlier):

1. Default values
2. Config file (`.infiniloom.yaml`, `.infiniloom.toml`, or `.infiniloom.json`)
3. Environment variables
4. CLI flags

---

## Quick Start

Generate a config file:

```bash
infiniloom init                      # Creates .infiniloom.yaml
infiniloom init --format toml        # Creates .infiniloom.toml
infiniloom init --format json        # Creates .infiniloom.json
infiniloom init --template rust      # Pre-configured for Rust projects
infiniloom init --template python    # Pre-configured for Python projects
infiniloom init --template typescript  # Pre-configured for TypeScript
```

---

## Config File Reference

### Full Example (YAML)

```yaml
# Output settings
output:
  format: xml              # xml, markdown, json, yaml, toon, plain
  model: claude            # claude, gpt4o, gpt4, gemini, llama, etc.
  compression: balanced    # none, minimal, balanced, aggressive, extreme
  token_budget: 100000     # 0 = no limit
  line_numbers: true       # Include line numbers in output
  show_file_summary: true  # Include file metadata summary

# Scanning settings
scan:
  include:                 # Glob patterns to include
    - "*.rs"
    - "*.py"
    - "*.ts"
    - "*.js"
  exclude:                 # Glob patterns to exclude
    - "tests/*"
    - "docs/*"
    - "*.test.*"
    - "*.spec.*"
  include_hidden: false    # Include hidden files/directories
  respect_gitignore: true  # Honor .gitignore patterns
  max_file_size: 1048576   # Skip files larger than 1MB

# Security settings
security:
  scan_secrets: true       # Enable secret scanning
  fail_on_secrets: false   # Exit with error if secrets found (CI/CD)
  redact_secrets: true     # Replace secrets with [REDACTED]
  allowlist:               # Patterns to ignore (won't be flagged)
    - "EXAMPLE"
    - "test_key"
    - "localhost"
  custom_patterns:         # Additional regex patterns to detect
    - "MY_SECRET_[A-Z0-9]{32}"
    - "CUSTOM_TOKEN_\\w+"
```

### Minimal Example

```yaml
output:
  format: xml
  model: claude

scan:
  exclude:
    - "node_modules/*"
    - "target/*"
```

### Flat Format (Legacy)

For backward compatibility, flat format is also supported:

```yaml
include:
  - "*.rs"
exclude:
  - "tests/*"
include_tests: false
include_docs: false
```

---

## Environment Variables

Environment variables use double underscore (`__`) to separate nested keys.

### Output Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_OUTPUT__FORMAT` | Output format | `xml` |
| `INFINILOOM_OUTPUT__MODEL` | Tokenizer model | `claude` |
| `INFINILOOM_OUTPUT__COMPRESSION` | Compression level | `none` |
| `INFINILOOM_OUTPUT__TOKEN_BUDGET` | Token budget | `0` (no limit) |
| `INFINILOOM_OUTPUT__LINE_NUMBERS` | Include line numbers | `true` |

### Scan Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_SCAN__INCLUDE_HIDDEN` | Include hidden files | `false` |
| `INFINILOOM_SCAN__RESPECT_GITIGNORE` | Honor .gitignore | `true` |
| `INFINILOOM_SCAN__MAX_FILE_SIZE` | Max file size (bytes) | `1048576` |

### Security Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_SECURITY__SCAN_SECRETS` | Enable secret scanning | `false` |
| `INFINILOOM_SECURITY__FAIL_ON_SECRETS` | Exit on secrets found | `false` |
| `INFINILOOM_SECURITY__REDACT_SECRETS` | Redact detected secrets | `true` |

### Examples

```bash
# Set defaults for Claude with security scanning
export INFINILOOM_OUTPUT__FORMAT=xml
export INFINILOOM_OUTPUT__MODEL=claude
export INFINILOOM_SECURITY__SCAN_SECRETS=true

# Use in command
infiniloom pack .
```

---

## CLI Flags Reference

CLI flags override both config files and environment variables.

### Output Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--format <FORMAT>` | `-f` | Output format |
| `--model <MODEL>` | `-m` | Tokenizer model |
| `--output <PATH>` | `-o` | Output file path |
| `--compression <LEVEL>` | `-c` | Compression level |
| `--max-tokens <N>` | | Token budget |
| `--line-numbers` | `-n` | Include line numbers |

### Filter Flags

| Flag | Description |
|------|-------------|
| `--include <PATTERN>` | Include glob pattern (can repeat) |
| `--exclude <PATTERN>` | Exclude glob pattern (can repeat) |
| `--top-files <N>` | Limit to N most important files |
| `--skip-symbols` | Skip AST parsing (faster) |

### Security Flags

| Flag | Description |
|------|-------------|
| `--security-check` | Scan for secrets, report in metadata |
| `--redact-secrets` | Redact secrets with [REDACTED] |
| `--fail-on-secrets` | Exit with error if secrets found |

### Git Flags

| Flag | Description |
|------|-------------|
| `--include-logs` | Include recent commit history |
| `--logs-count <N>` | Number of commits (default: 10) |
| `--include-diffs` | Include uncommitted changes |

---

## .infiniloomignore

In addition to `.gitignore`, you can create `.infiniloomignore` for Infiniloom-specific exclusions:

```gitignore
# Build artifacts
target/
dist/
build/
out/

# Dependencies
node_modules/
vendor/
.venv/

# Large files
*.bin
*.dat
*.sqlite
data/

# Generated code
*.generated.*
*.g.dart

# IDE
.idea/
.vscode/
*.swp

# Test fixtures that shouldn't be packed
fixtures/
__fixtures__/
```

---

## Language-Specific Templates

### Rust Template

```bash
infiniloom init --template rust
```

```yaml
output:
  format: xml
  model: claude
  compression: balanced

scan:
  include:
    - "*.rs"
    - "*.toml"
  exclude:
    - "target/*"
    - "benches/*"

security:
  scan_secrets: true
```

### Python Template

```bash
infiniloom init --template python
```

```yaml
output:
  format: xml
  model: claude
  compression: balanced

scan:
  include:
    - "*.py"
    - "*.pyi"
    - "pyproject.toml"
    - "setup.py"
  exclude:
    - ".venv/*"
    - "venv/*"
    - "__pycache__/*"
    - "*.egg-info/*"
    - "dist/*"
    - "build/*"

security:
  scan_secrets: true
```

### TypeScript Template

```bash
infiniloom init --template typescript
```

```yaml
output:
  format: markdown
  model: gpt4o
  compression: balanced

scan:
  include:
    - "*.ts"
    - "*.tsx"
    - "*.js"
    - "*.jsx"
    - "package.json"
    - "tsconfig.json"
  exclude:
    - "node_modules/*"
    - "dist/*"
    - "build/*"
    - "coverage/*"
    - "*.d.ts"

security:
  scan_secrets: true
```

---

## CI/CD Integration

### GitHub Actions

```yaml
- name: Pack repository context
  run: |
    npm install -g infiniloom
    infiniloom pack . --format xml --output context.xml --fail-on-secrets

- name: Upload context artifact
  uses: actions/upload-artifact@v4
  with:
    name: repo-context
    path: context.xml
```

### Pre-commit Hook

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: infiniloom-secrets
        name: Check for secrets
        entry: infiniloom pack . --security-check --fail-on-secrets
        language: system
        pass_filenames: false
```

---

## Supported Values

### Output Formats

| Format | Best For | Description |
|--------|----------|-------------|
| `xml` | Claude | Prompt caching hints, CDATA sections |
| `markdown` | GPT-4/GPT-4o | Tables, code fences |
| `yaml` | Gemini | Structured hierarchy |
| `json` | Programmatic | Full metadata |
| `toon` | Any | ~40% smaller than JSON |
| `plain` | Generic | Simple text |

### Tokenizer Models

| Model | Encoding | Notes |
|-------|----------|-------|
| `claude` | Estimation | Default, ~95% accurate |
| `gpt52`, `gpt51`, `gpt5` | o200k_base | Exact via tiktoken |
| `o4-mini`, `o3`, `o1` | o200k_base | Exact via tiktoken |
| `gpt4o`, `gpt4o-mini` | o200k_base | Exact via tiktoken |
| `gpt4`, `gpt35-turbo` | cl100k_base | Exact via tiktoken |
| `gemini` | Estimation | Google models |
| `llama`, `codellama` | Estimation | Meta models |
| `mistral`, `mixtral` | Estimation | Mistral AI |
| `deepseek` | Estimation | DeepSeek V3/R1 |
| `qwen` | Estimation | Alibaba |
| `cohere` | Estimation | Command R+ |
| `grok` | Estimation | xAI |

### Compression Levels

| Level | Token Reduction | Removes |
|-------|-----------------|---------|
| `none` | 0% | Nothing |
| `minimal` | 10-20% | Empty lines, trailing whitespace |
| `balanced` | 30-40% | Comments, redundant whitespace |
| `aggressive` | 50-60% | Docstrings, inline comments |
| `extreme` | 70-80% | Everything except signatures |

---

## Troubleshooting

### Config File Not Found

Infiniloom searches for config files in this order:
1. `.infiniloom.yaml`
2. `.infiniloom.toml`
3. `.infiniloom.json`

Verify the file exists and is in the repository root:

```bash
ls -la .infiniloom.*
```

### Environment Variable Not Working

Ensure double underscores are used correctly:

```bash
# Correct
export INFINILOOM_OUTPUT__FORMAT=xml

# Wrong
export INFINILOOM_OUTPUT_FORMAT=xml
```

### Gitignore Patterns Not Respected

Check that `respect_gitignore` is enabled (default: true):

```yaml
scan:
  respect_gitignore: true
```

Or verify with:

```bash
infiniloom scan . --verbose
```
