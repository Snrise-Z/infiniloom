# Infiniloom Quick Start Guide

**Last Updated**: 2026-01-02
**Version**: 0.6.0

This guide will get you started with Infiniloom in 10 minutes. Perfect for first-time users!

---

## Table of Contents

- [Installation](#installation)
- [Your First Command](#your-first-command)
- [Common Workflows](#common-workflows)
- [Tutorial 1: Basic Repository Context](#tutorial-1-basic-repository-context)
- [Tutorial 2: Git-Aware Diff Context](#tutorial-2-git-aware-diff-context)
- [Tutorial 3: Token Budget Management](#tutorial-3-token-budget-management)
- [Tutorial 4: Security Scanning](#tutorial-4-security-scanning)
- [Tutorial 5: Symbol Index and Call Graph](#tutorial-5-symbol-index-and-call-graph)
- [Tutorial 6: Large Repository Handling](#tutorial-6-large-repository-handling)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)

---

## Installation

Choose your preferred method:

### npm (Recommended - Cross-Platform)

```bash
npm install -g infiniloom
infiniloom --version
```

### Homebrew (macOS)

```bash
brew tap Topos-Labs/infiniloom
brew install --cask infiniloom
infiniloom --version
```

### Cargo (Rust Users)

```bash
cargo install infiniloom
infiniloom --version
```

### From Source

```bash
git clone https://github.com/Topos-Labs/infiniloom
cd infiniloom
cargo build --release
./target/release/infiniloom --version
```

---

## Your First Command

Let's start with the simplest command - scanning your current directory:

```bash
# Navigate to any code repository
cd /path/to/your/project

# Scan and show statistics
infiniloom scan .
```

**Output:**
```
Repository: my-project
Total files: 42
Total lines: 3,214
Total tokens (Claude): 8,453

Languages:
  Rust: 28 files (67%)
  Python: 10 files (24%)
  YAML: 4 files (9%)

Top files by tokens:
  1. src/main.rs: 1,234 tokens
  2. src/lib.rs: 987 tokens
  3. src/parser.rs: 654 tokens
```

**What just happened?**
- Infiniloom walked your directory tree (respecting .gitignore)
- Detected file languages by extension
- Counted tokens for Claude's tokenizer
- Showed you a summary

---

## Common Workflows

### Workflow 1: Quick Context for Claude

```bash
# Generate XML context (Claude-optimized)
infiniloom pack . --format xml > context.xml

# Copy to clipboard and paste into Claude
cat context.xml | pbcopy  # macOS
cat context.xml | xclip    # Linux
```

### Workflow 2: GPT-4 Context with Token Limit

```bash
# Generate Markdown with 50K token limit
infiniloom pack . \
  --format markdown \
  --model gpt4o \
  --max-tokens 50000 \
  --output context.md
```

### Workflow 3: Review Recent Changes

```bash
# Get context for uncommitted changes
infiniloom diff

# Get context for last commit
infiniloom diff HEAD~1

# Get context for feature branch
infiniloom diff main..feature
```

### Workflow 4: Security Check Before Commit

```bash
# Scan for secrets and API keys
infiniloom scan . --security-check

# Pack with automatic redaction
infiniloom pack . --security-check --redact-secrets
```

---

## Tutorial 1: Basic Repository Context

**Goal**: Generate context for a Python project and use it with Claude.

### Step 1: Navigate to Your Project

```bash
cd ~/projects/my-python-app
```

### Step 2: Quick Scan

```bash
infiniloom scan . --verbose
```

**Output shows**:
- 87 Python files
- 12,453 lines of code
- 32,145 tokens (Claude)
- Top files by importance

### Step 3: Generate Context

```bash
infiniloom pack . \
  --format xml \
  --model claude \
  --output context.xml
```

**Options explained**:
- `--format xml`: Claude-optimized format
- `--model claude`: Use Claude's tokenizer
- `--output context.xml`: Save to file

### Step 4: Use with Claude

```bash
# macOS
cat context.xml | pbcopy

# Linux
cat context.xml | xclip -selection clipboard

# Windows
type context.xml | clip
```

Then paste into Claude and ask:
```
Please review this Python application and suggest improvements to the
authentication system. Focus on security best practices.
```

### Step 5: Filter to Specific Files

```bash
# Only include src/ directory
infiniloom pack . \
  --include "src/**/*.py" \
  --format xml \
  --output context.xml

# Exclude tests and docs
infiniloom pack . \
  --exclude "tests/*" \
  --exclude "docs/*" \
  --format xml \
  --output context.xml
```

---

## Tutorial 2: Git-Aware Diff Context

**Goal**: Get context for code review with git integration.

### Step 1: Create a Symbol Index

First, build an index of your codebase symbols (one-time setup):

```bash
infiniloom index .
```

**Output:**
```
Building symbol index...
Parsed 87 files, found 342 symbols
Index saved to: .infiniloom/index.bin
Build time: 2.3s
```

**What's in the index?**
- All functions, classes, methods
- Import relationships (who calls whom)
- Dependency graph
- File metadata

### Step 2: Get Diff Context for Uncommitted Changes

```bash
infiniloom diff
```

**Output:**
```
Changed files: 3
  - src/auth.py (modified)
  - src/validators.py (new file)
  - tests/test_auth.py (modified)

Context files: 8
  - src/auth.py (changed)
  - src/validators.py (changed)
  - src/database.py (calls auth)
  - src/api.py (calls auth)
  - src/user_service.py (calls validators)
  - tests/test_auth.py (changed)
  - tests/test_validators.py (tests validators)
  - tests/conftest.py (test fixture)

Total context tokens: 4,523
```

### Step 3: Get Diff Context for Specific Commit

```bash
# Last commit
infiniloom diff HEAD~1 --format xml --output review.xml

# Last 3 commits
infiniloom diff HEAD~3 --format xml --output review.xml

# Specific commit range
infiniloom diff abc123..def456 --format xml --output review.xml
```

### Step 4: Include Actual Diff Content

```bash
# Add +/- lines to context
infiniloom diff HEAD~1 \
  --include-diff \
  --format xml \
  --output review.xml
```

**Result**: XML includes both changed files AND the actual diff hunks (+/- lines).

### Step 5: Control Context Depth

```bash
# Depth 1: Only changed files
infiniloom diff --depth 1

# Depth 2: Changed files + direct dependencies (default)
infiniloom diff --depth 2

# Depth 3: Changed files + transitive dependencies
infiniloom diff --depth 3
```

**Depth Examples**:
```
auth.py changed
├─ Depth 1: auth.py only
├─ Depth 2: + api.py (calls auth), + database.py (calls auth)
└─ Depth 3: + user_service.py (calls api), + middleware.py (calls api)
```

---

## Tutorial 3: Token Budget Management

**Goal**: Generate context that fits within LLM context windows.

### Step 1: Check Current Token Count

```bash
infiniloom scan . --model gpt4o
```

**Output:**
```
Total tokens (GPT-4o): 127,453
```

**Problem**: This exceeds GPT-4o's 128K context window!

### Step 2: Apply Token Budget

```bash
# Limit to 50K tokens
infiniloom pack . \
  --model gpt4o \
  --max-tokens 50000 \
  --format markdown \
  --output context.md
```

**What happens?**
- Infiniloom ranks files by importance (PageRank)
- Includes most important files first
- Stops when budget reached
- Includes summary of excluded files

### Step 3: Use Compression

```bash
# Remove comments and empty lines
infiniloom pack . \
  --model gpt4o \
  --max-tokens 50000 \
  --compression balanced \
  --format markdown \
  --output context.md
```

**Compression Levels**:
- `none`: Full source code
- `minimal`: Remove empty lines (5-10% savings)
- `balanced`: Remove comments (20-30% savings)
- `aggressive`: Extract signatures only (50-70% savings)
- `extreme`: Key symbols only (80-90% savings)
- `focused`: Key symbols with 2-line context (75-85% savings)

### Step 4: Use TOON Format

TOON (Token-Oriented Object Notation) is 30-40% more efficient than XML/Markdown:

```bash
infiniloom pack . \
  --model gpt4o \
  --format toon \
  --output context.toon
```

**Before (XML)**: 127,453 tokens
**After (TOON)**: 76,472 tokens (40% reduction!)

### Step 5: Combine Strategies

```bash
infiniloom pack . \
  --model gpt4o \
  --max-tokens 50000 \
  --compression balanced \
  --format toon \
  --exclude "tests/*" \
  --exclude "docs/*" \
  --output context.toon
```

**Result**: Fits in 50K token budget with maximum information density.

---

## Tutorial 4: Security Scanning

**Goal**: Detect and redact secrets before sharing code with AI.

### Step 1: Scan for Secrets

```bash
infiniloom scan . --security-check
```

**Output:**
```
Security scan results:
Found 3 potential secrets:

  1. config/prod.env:12
     Pattern: AWS_ACCESS_KEY
     Matched: AKIAIOSFODNN7EXAMPLE

  2. src/database.py:45
     Pattern: PASSWORD_IN_URL
     Matched: postgres://user:pass@localhost/db

  3. scripts/deploy.sh:23
     Pattern: PRIVATE_KEY
     Matched: -----BEGIN RSA PRIVATE KEY-----
```

### Step 2: Pack with Redaction

```bash
infiniloom pack . \
  --security-check \
  --redact-secrets \
  --output safe-context.xml
```

**Result**:
```xml
<!-- config/prod.env -->
AWS_ACCESS_KEY=[REDACTED_AWS_ACCESS_KEY]
AWS_SECRET=[REDACTED_AWS_SECRET_KEY]

<!-- src/database.py -->
db_url = "postgres://user:[REDACTED]@localhost/db"

<!-- scripts/deploy.sh -->
ssh_key = """[REDACTED_PRIVATE_KEY]"""
```

### Step 3: Configure Allowlist

Some "secrets" are actually test data. Create `.infiniloom.yaml`:

```yaml
security:
  scan_secrets: true
  redact_secrets: true
  allowlist:
    - "EXAMPLE"
    - "test_"
    - "mock_"
```

Now run again:

```bash
infiniloom pack . \
  --security-check \
  --redact-secrets \
  --config .infiniloom.yaml \
  --output safe-context.xml
```

**Result**: Test keys like `AWS_KEY=test_12345` are not redacted.

### Step 4: Custom Secret Patterns

Add custom patterns to detect organization-specific secrets:

```yaml
security:
  scan_secrets: true
  redact_secrets: true
  custom_patterns:
    - "MYCOMPANY_API_[A-Z0-9]{32}"
    - "INTERNAL_TOKEN_[a-f0-9]{64}"
```

### Step 5: Fail CI on Secrets

In CI/CD pipelines, exit with error if secrets found:

```yaml
# .github/workflows/ci.yml
- name: Check for secrets
  run: |
    infiniloom scan . --security-check || exit 1
```

Or in `.infiniloom.yaml`:

```yaml
security:
  scan_secrets: true
  fail_on_secrets: true  # Exit code 4 if secrets found
```

---

## Tutorial 5: Symbol Index and Call Graph

**Goal**: Understand code relationships and find what depends on your changes.

### Step 1: Build Index

```bash
infiniloom index . --verbose
```

**Output:**
```
Scanning files: 87 files
Parsing symbols: [==============================] 100%
Found symbols:
  - Functions: 234
  - Classes: 42
  - Methods: 312
  - Total: 588

Building dependency graph...
Found 1,234 call relationships

Index saved to: .infiniloom/index.bin (142 KB)
Build time: 3.2s
```

### Step 2: Check Index Status

```bash
infiniloom index --status
```

**Output:**
```
Index status: ✓ UP TO DATE

  Files indexed: 87
  Symbols: 588
  Dependencies: 1,234
  Last built: 2025-12-28 10:23:15
  Index size: 142 KB

Recent changes: None
Next action: Index is current
```

### Step 3: Incremental Update

After editing files:

```bash
infiniloom index --incremental
```

**Output:**
```
Checking for changes...
Changed files: 3
  - src/auth.py (modified)
  - src/validators.py (new file)
  - tests/test_auth.py (modified)

Re-indexing changed files: [====] 100% (3/3)
Updated 23 symbols
Updated 47 dependencies

Build time: 0.8s (4x faster than full rebuild!)
```

### Step 4: Analyze Impact of Changes

```bash
# What depends on auth.py?
infiniloom impact src/auth.py
```

**Output:**
```
Impact analysis: src/auth.py

Direct dependents: 5 files
  - src/api.py (calls login, logout)
  - src/middleware.py (calls check_auth)
  - src/database.py (calls validate_token)
  - tests/test_auth.py (tests all functions)
  - tests/test_api.py (tests login flow)

Transitive dependents: 12 files
  - src/user_service.py (calls api → auth)
  - src/admin.py (calls api → auth)
  - ... (10 more)

Total affected: 17 files
```

### Step 5: Find Symbol Usages

```bash
# Find all callers of a specific function
infiniloom impact --symbol "validate_token"
```

**Output:**
```
Symbol: validate_token (src/auth.py:45)

Callers: 7 locations
  1. src/api.py:23 → def protected_endpoint()
  2. src/api.py:67 → def admin_endpoint()
  3. src/middleware.py:12 → def auth_middleware()
  4. src/database.py:34 → def check_session()
  5. tests/test_auth.py:89 → def test_validation()
  6. tests/test_auth.py:102 → def test_expired_token()
  7. tests/test_api.py:45 → def test_protected()

Estimated tokens to show all callers: 2,341
```

---

## Tutorial 6: Large Repository Handling

**Goal**: Work with large repositories (10K+ files, 1M+ lines).

### Step 1: Fast Scan with Sampling

For initial exploration:

```bash
# Sample 10% of files
infiniloom scan . --sample-percent 10
```

**Output:**
```
Sampled 432 files (10% of 4,320 files)
Estimated total: 342,145 tokens
Estimated time for full scan: 45s
```

### Step 2: Exclude Generated Code

```bash
infiniloom pack . \
  --exclude "node_modules/*" \
  --exclude "dist/*" \
  --exclude "build/*" \
  --exclude "vendor/*" \
  --exclude "*.min.js" \
  --output context.xml
```

### Step 3: Use Repository Chunking

Split large repo into manageable chunks for multi-turn conversations:

```bash
infiniloom chunk . \
  --strategy semantic \
  --max-tokens 8000 \
  --output chunks/
```

**Output:**
```
Created 12 chunks:
  chunks/chunk_001.xml (7,845 tokens) - Core authentication
  chunks/chunk_002.xml (7,923 tokens) - API endpoints
  chunks/chunk_003.xml (7,654 tokens) - Database models
  ... (9 more)

Total: 95,432 tokens across 12 chunks
Average: 7,953 tokens per chunk
```

**Chunking Strategies**:
- `semantic`: Group related code (default, best for understanding)
- `module`: Group by directory structure
- `file`: One file per chunk (simple)
- `symbol`: Group by function/class (granular)
- `dependency`: Group by import relationships

### Step 4: Focus on Specific Areas

Use include patterns for surgical precision:

```bash
# Only authentication module
infiniloom pack . \
  --include "src/auth/**" \
  --include "tests/auth/**" \
  --output auth-context.xml

# Only API layer
infiniloom pack . \
  --include "src/api/**" \
  --include "src/routes/**" \
  --output api-context.xml
```

### Step 5: Incremental Caching

Enable caching for repeated operations:

```bash
infiniloom pack . \
  --cache \
  --output context.xml
```

**First run**: 45 seconds (full parse)
**Second run**: 2 seconds (cache hit!)
**After editing 3 files**: 5 seconds (incremental re-parse)

**Cache location**: `.infiniloom/cache/`

---

## Configuration

### Create Configuration File

```bash
infiniloom init
```

**Prompts**:
```
Output format? [xml/markdown/yaml/json/toon] xml
Target model? [claude/gpt4o/gemini] claude
Compression level? [none/minimal/balanced/aggressive] balanced
Token budget? (0 = no limit) 100000
Include hidden files? [y/n] n
Include test files? [y/n] n
```

**Result**: `.infiniloom.yaml` created:

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000
  line_numbers: true

scan:
  include_hidden: false
  respect_gitignore: true
  include_tests: false

security:
  scan_secrets: true
  redact_secrets: true
```

### Use Configuration

```bash
# Auto-loads .infiniloom.yaml
infiniloom pack .

# Use specific config
infiniloom pack . --config custom-config.yaml

# Override config values
infiniloom pack . --model gpt4o --max-tokens 50000
```

### Environment Variables

Override config with environment variables:

```bash
export INFINILOOM_OUTPUT__MODEL=gpt4o
export INFINILOOM_OUTPUT__FORMAT=markdown
export INFINILOOM_SCAN__INCLUDE_HIDDEN=false

infiniloom pack .  # Uses env vars
```

---

## Troubleshooting

### "No files found" Error

**Problem**: Infiniloom didn't find any code files.

**Solutions**:
```bash
# Check if directory is correct
pwd

# Include hidden files if needed
infiniloom scan . --hidden

# Disable gitignore if needed
infiniloom scan . --no-gitignore

# Check ignore patterns
infiniloom scan . --verbose
```

### "Binary file detected" Warning

**Problem**: Some files are skipped as binary.

**Solution**: This is expected behavior. Binary files (images, PDFs, etc.) are automatically excluded. If a text file is incorrectly detected as binary, file an issue.

### "Token limit exceeded" Error

**Problem**: Output exceeds token budget.

**Solutions**:
```bash
# Increase budget
infiniloom pack . --max-tokens 200000

# Use compression
infiniloom pack . --compression balanced

# Use TOON format
infiniloom pack . --format toon

# Exclude files
infiniloom pack . --exclude "tests/*" --exclude "docs/*"
```

### "Index not found" Error

**Problem**: Running `infiniloom diff` without building index first.

**Solution**:
```bash
# Build index
infiniloom index .

# Then run diff
infiniloom diff
```

### Slow Parsing on Large Repos

**Problem**: Scanning takes too long.

**Solutions**:
```bash
# Skip symbol extraction (80x faster)
infiniloom scan . --no-symbols

# Use sampling
infiniloom scan . --sample-percent 10

# Exclude large directories
infiniloom scan . --exclude "vendor/*" --exclude "node_modules/*"
```

### "Git not found" Error

**Problem**: Git commands fail.

**Solution**:
```bash
# Install git
brew install git  # macOS
apt-get install git  # Ubuntu

# Or skip git features
infiniloom pack . --no-logs --no-diffs
```

---

## Next Steps

### Learn More

- **[Architecture Guide](ARCHITECTURE.md)** - Deep dive into system design
- **[Configuration Guide](CONFIGURATION.md)** - All config options
- **[FAQ](FAQ.md)** - Common questions answered
- **[Command Reference](commands/)** - Detailed command docs

### Advanced Topics

- **Symbol Ranking**: How PageRank determines importance
- **Token Counting**: Exact vs estimated counting strategies
- **Output Formats**: Format-specific optimization techniques
- **Security Patterns**: Custom secret detection patterns
- **Performance Tuning**: Parallel processing and caching

### Get Help

- **GitHub Issues**: [Report bugs](https://github.com/Topos-Labs/infiniloom/issues)
- **Discussions**: [Ask questions](https://github.com/Topos-Labs/infiniloom/discussions)
- **Documentation**: [Read the docs](https://github.com/Topos-Labs/infiniloom/tree/main/docs)

### Contribute

- **[Contributing Guide](../CONTRIBUTING.md)** - How to contribute code
- **[Development Setup](../README.md#development)** - Build from source
- **[Test Specification](TEST_SPECIFICATION.md)** - Testing guidelines

---

**Happy coding with Infiniloom!** 🚀

