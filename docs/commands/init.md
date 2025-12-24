# `infiniloom init` Command

## Overview

The `init` command creates a configuration file for a project. This file customizes Infiniloom's behavior, including output format, target model, compression level, file filters, and security settings.

## Synopsis

```bash
infiniloom init [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `init` command:

1. **Checks for Existing Config**: Prevents accidental overwrites
2. **Generates Default Config**: Creates a well-documented configuration file
3. **Supports Multiple Formats**: YAML (default), TOML, or JSON

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--format <FORMAT>` | `-f` | Configuration format: `yaml`, `toml`, `json` | `yaml` |
| `--template <TYPE>` | `-t` | Project template with pre-configured settings | `generic` |
| `--output <PATH>` | `-o` | Custom output path | `.infiniloom.{format}` |
| `--force` | | Overwrite existing config file | `false` |

### Available Templates

| Template | Description | Include Patterns | Exclude Patterns |
|----------|-------------|------------------|------------------|
| `generic` | Default generic template | All files | None |
| `rust` | Rust/Cargo projects | `*.rs`, `Cargo.toml`, `Cargo.lock` | `target/` |
| `python` | Python projects | `*.py`, `requirements.txt`, `pyproject.toml`, `setup.py` | `venv/`, `__pycache__/`, `.pytest_cache/`, `*.egg-info/` |
| `typescript` | TypeScript/Node.js projects | `*.ts`, `*.tsx`, `*.js`, `*.jsx`, `package.json`, `tsconfig.json` | `node_modules/`, `dist/`, `build/`, `coverage/` |
| `go` | Go projects | `*.go`, `go.mod`, `go.sum` | `vendor/` |
| `java` | Java/Maven/Gradle projects | `*.java`, `pom.xml`, `build.gradle`, `*.gradle.kts` | `target/`, `build/`, `.gradle/` |

## Generated Configuration

### YAML Format (Default)

```yaml
# Infiniloom Configuration
# Documentation: https://github.com/Topos-Labs/infiniloom

# Output settings
output:
  # Output format: xml, markdown, json, yaml, toon, plain
  format: xml

  # Target model for token counting optimization
  # Options: claude, gpt52, gpt51, gpt5, o4-mini, o3, o1, gpt4o, gpt4o-mini,
  #          gpt4, gpt35-turbo, gemini, llama, codellama, mistral, deepseek,
  #          qwen, cohere, grok
  model: claude

  # Compression level: none, minimal, balanced, aggressive, extreme, focused, semantic
  compression: balanced

  # Maximum output tokens (0 = no limit)
  token_budget: 0

  # Include line numbers in output
  line_numbers: true

  # Show file summary section
  show_file_summary: true

  # Show directory structure
  show_directory_structure: true

# Scanning settings
scan:
  # Include patterns (glob syntax)
  include: []
  #  - "*.rs"
  #  - "*.py"
  #  - "src/**/*.ts"

  # Exclude patterns (glob syntax)
  exclude: []
  #  - "tests/*"
  #  - "docs/*"
  #  - "*.test.*"
  #  - "*.spec.*"

  # Include hidden files (starting with .)
  include_hidden: false

  # Include test files
  include_tests: false

  # Include documentation files
  include_docs: false

  # Maximum file size in bytes (default: 50MB)
  # Supports: "10MB", "1GB", 52428800
  max_file_size: 52428800

# Security settings
security:
  # Scan for secrets (API keys, tokens, private keys)
  scan_secrets: false

  # Exit with error if secrets are found (for CI/CD)
  fail_on_secrets: false

  # Replace detected secrets with [REDACTED]
  redact_secrets: false

  # Patterns to ignore (won't be flagged as secrets)
  allowlist: []
  #  - "EXAMPLE_KEY"
  #  - "test_token"
  #  - "localhost"

  # Additional regex patterns to detect
  custom_patterns: []
  #  - "MY_SECRET_[A-Z0-9]{32}"
  #  - "internal_key_[a-z]+"

# Content transformation
transform:
  # Remove empty lines from code
  remove_empty_lines: false

  # Remove comments from code
  remove_comments: false

  # Truncate base64-encoded content
  truncate_base64: false
```

### TOML Format

```toml
# Infiniloom Configuration

[output]
format = "xml"
model = "claude"
compression = "balanced"
token_budget = 0
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = []
exclude = []
include_hidden = false
include_tests = false
include_docs = false
max_file_size = 52428800

[security]
scan_secrets = false
fail_on_secrets = false
redact_secrets = false
allowlist = []
custom_patterns = []

[transform]
remove_empty_lines = false
remove_comments = false
truncate_base64 = false
```

### JSON Format

```json
{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "token_budget": 0,
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": [],
    "exclude": [],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false,
    "max_file_size": 52428800
  },
  "security": {
    "scan_secrets": false,
    "fail_on_secrets": false,
    "redact_secrets": false,
    "allowlist": [],
    "custom_patterns": []
  },
  "transform": {
    "remove_empty_lines": false,
    "remove_comments": false,
    "truncate_base64": false
  }
}
```

## Configuration Precedence

Settings are applied in this order (later overrides earlier):

1. **Default values** (built into Infiniloom)
2. **Config file** (`.infiniloom.yaml`, `.infiniloom.toml`, or `.infiniloom.json`)
3. **Environment variables** (`INFINILOOM_OUTPUT__FORMAT=xml`)
4. **CLI arguments** (`--format xml`)

### Environment Variable Format

Use double underscore (`__`) for nested keys:

```bash
export INFINILOOM_OUTPUT__FORMAT=markdown
export INFINILOOM_OUTPUT__MODEL=gpt4o
export INFINILOOM_SECURITY__SCAN_SECRETS=true
```

## Examples

### Basic Initialization

```bash
# Create .infiniloom.yaml in current directory
infiniloom init

# Create in specific directory
infiniloom init /path/to/project
```

### Format Selection

```bash
# YAML (default, most readable)
infiniloom init --format yaml

# TOML (Rust ecosystem standard)
infiniloom init --format toml

# JSON (machine-readable)
infiniloom init --format json
```

### Project Templates

```bash
# Initialize with Rust template
infiniloom init --template rust

# Initialize with Python template
infiniloom init --template python

# Initialize with TypeScript template
infiniloom init --template typescript

# Initialize with Go template
infiniloom init --template go

# Initialize with Java template
infiniloom init --template java

# Combine template with format
infiniloom init --template rust --format toml
```

### Custom Output Path

```bash
# Custom filename
infiniloom init --output config/infiniloom-config.yaml

# Hidden file in home directory (global config)
infiniloom init --output ~/.infiniloomrc.yaml
```

### Overwrite Existing

```bash
# Force overwrite
infiniloom init --force
```

## Best Practices

### Project-Specific Configuration

Create config for each project type:

**Rust Project:**
```yaml
output:
  format: xml
  model: claude
scan:
  include:
    - "*.rs"
    - "Cargo.toml"
  exclude:
    - "target/*"
```

**Python Project:**
```yaml
output:
  format: markdown
  model: gpt4o
scan:
  include:
    - "*.py"
    - "requirements.txt"
    - "pyproject.toml"
  exclude:
    - "venv/*"
    - "__pycache__/*"
    - ".pytest_cache/*"
```

**TypeScript Project:**
```yaml
output:
  format: xml
  model: claude
scan:
  include:
    - "*.ts"
    - "*.tsx"
    - "package.json"
  exclude:
    - "node_modules/*"
    - "dist/*"
    - "*.test.ts"
    - "*.spec.ts"
```

### Security Configuration for CI/CD

```yaml
security:
  scan_secrets: true
  fail_on_secrets: true  # Block pipeline if secrets found
  redact_secrets: true   # Redact in output anyway
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
```

### Token Budget Optimization

```yaml
output:
  # Leave room for LLM response
  token_budget: 100000  # For Claude with 200K context

  # Use efficient format
  format: toon  # 40% smaller than XML

  # Aggressive compression for large repos
  compression: aggressive
```

## Configuration Loading Order

Infiniloom searches for config in this order:

1. `--config` CLI argument (if specified)
2. `.infiniloom.yaml` in project root
3. `.infiniloom.toml` in project root
4. `.infiniloom.json` in project root
5. `.infiniloomrc.yaml` in project root (legacy)
6. `.infiniloomrc.json` in project root (legacy)

First found file is used.

## Potential Improvements

### 1. Interactive Initialization

```bash
# Future: guided setup
infiniloom init --interactive
# Prompts: "What's your target model?", "Include tests?", etc.
```

### 2. Config Validation

```bash
# Future: validate existing config
infiniloom init --validate
# Output: "Config is valid" or lists errors
```

### 3. Config Migration

```bash
# Future: upgrade config to new format
infiniloom init --migrate
# Upgrades old config format to latest
```

### 4. Merge Configurations

```bash
# Future: merge with existing
infiniloom init --merge
# Adds missing fields without overwriting existing
```

### 5. Config Diff

```bash
# Future: compare with defaults
infiniloom init --diff
# Shows what's different from defaults
```

### 6. Remote Config Templates

```bash
# Future: fetch from URL
infiniloom init --from https://example.com/infiniloom-config.yaml
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | File exists and `--force` not specified |
| 1 | Cannot write to path |

## Related Commands

- [`info`](info.md) - View available options and current config
- [`pack`](pack.md) - Use configuration for packing
- [`scan`](scan.md) - Use configuration for scanning
