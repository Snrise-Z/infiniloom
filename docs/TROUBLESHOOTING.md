# Troubleshooting Guide

Solutions to common issues when using Infiniloom.

## Installation Issues

### "command not found: infiniloom"

The binary isn't in your PATH.

**npm installation:**
```bash
# Check npm global bin location
npm bin -g

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$(npm bin -g):$PATH"

# Or use npx
npx infiniloom pack .
```

**Cargo installation:**
```bash
# Ensure ~/.cargo/bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

**Homebrew installation:**
```bash
# Apple Silicon
export PATH="/opt/homebrew/bin:$PATH"

# Intel Mac
export PATH="/usr/local/bin:$PATH"
```

### macOS Gatekeeper blocking binary

If macOS says the app is from an unidentified developer:

```bash
# Remove quarantine attribute
xattr -d com.apple.quarantine $(which infiniloom)
```

Or: System Preferences → Security & Privacy → "Allow Anyway"

### Build errors from source

```bash
# Ensure Rust is up to date
rustup update stable

# Check Rust version (requires 1.91+)
rustc --version

# Clean and rebuild
cargo clean
cargo build --release
```

### Missing dependencies on Linux

```bash
# Ubuntu/Debian
sudo apt-get install build-essential pkg-config libssl-dev

# Fedora/RHEL
sudo dnf install gcc pkg-config openssl-devel

# Arch
sudo pacman -S base-devel openssl
```

---

## Runtime Issues

### "Not a valid repository path"

Infiniloom expects a directory containing files. Common causes:

```bash
# Wrong: File path instead of directory
infiniloom pack ./main.rs  # Error!

# Right: Directory path
infiniloom pack .
infiniloom pack ./src
```

### "No files found"

The filters might be too restrictive:

```bash
# Check what's being filtered
infiniloom scan . --verbose

# Common issues:
# 1. All files in .gitignore
infiniloom pack . --no-gitignore

# 2. Wrong include pattern
infiniloom pack . --include "*.rs"   # Right
infiniloom pack . --include "rs"     # Wrong (no glob)

# 3. Tests excluded by default
infiniloom pack . --include-tests
```

### "Binary file detected, skipping"

Infiniloom skips binary files by default. To include specific file types:

```bash
# Force include specific extension (not recommended for actual binaries)
infiniloom pack . --include "*.wasm"
```

### Output is empty or very small

```bash
# Check repository statistics
infiniloom scan .

# Common causes:
# 1. Token budget too small
infiniloom pack . --max-tokens 100000  # Increase budget

# 2. Compression too aggressive
infiniloom pack . --compression none

# 3. Wrong include patterns
infiniloom pack . --verbose  # See what's included
```

### Processing is slow

```bash
# Skip symbol extraction (80x faster)
# Default is already fast; --symbols makes it slower
infiniloom pack .  # Fast
infiniloom pack . --symbols  # Slower but more features

# For very large repos, use filtering
infiniloom pack . --include "src/**" --exclude "vendor/*"

# Use sampling for quick stats
infiniloom scan . --sample 500
```

### Out of memory

For very large repositories:

```bash
# Filter to specific directories
infiniloom pack . --include "src/**"

# Use chunking
infiniloom chunk . --max-tokens 50000

# Limit files
infiniloom pack . --top-files 100

# Use compression
infiniloom pack . --compression aggressive
```

---

## Output Issues

### "Context too long" from LLM

Your output exceeds the model's context window:

```bash
# Reduce token budget
infiniloom pack . --max-tokens 50000

# Increase compression
infiniloom pack . --compression aggressive

# Focus on specific files
infiniloom pack . --include "src/**" --exclude "tests/*"

# Limit to most important files
infiniloom pack . --top-files 50

# Use TOON format (~40% smaller)
infiniloom pack . --format toon
```

### Token count doesn't match LLM's count

Token counting accuracy varies by model:

```bash
# OpenAI models use exact tiktoken tokenization
infiniloom scan . --model gpt4o      # Exact
infiniloom scan . --model gpt5       # Exact

# Other models use calibrated estimation (~95% accurate)
infiniloom scan . --model claude     # Estimation
infiniloom scan . --model gemini     # Estimation
```

For precise budgeting, use a slightly lower budget than your limit.

### Wrong output format

Make sure format and model match:

```bash
# Claude - use XML
infiniloom pack . --format xml --model claude

# GPT - use Markdown
infiniloom pack . --format markdown --model gpt4o

# Gemini - use YAML
infiniloom pack . --format yaml --model gemini
```

### Missing symbols in output

Symbol extraction requires the `--symbols` or `--full` flag:

```bash
# Enable symbol extraction
infiniloom pack . --symbols

# Or full analysis mode
infiniloom pack . --full
```

---

## Security Issues

### "Secrets detected" error

Infiniloom found potential secrets in your code:

```bash
# View what was detected
infiniloom pack . --security-check

# Redact secrets instead of failing
infiniloom pack . --redact-secrets

# Or disable security checking (not recommended)
# Add to .infiniloom.yaml:
# security:
#   scan_secrets: false
```

### False positive secret detection

Add to allowlist in config:

```yaml
# .infiniloom.yaml
security:
  scan_secrets: true
  allowlist:
    - "EXAMPLE_KEY"
    - "test_token"
    - "placeholder"
    - "localhost"
```

---

## Git Integration Issues

### "Not a git repository"

Git features require a git repository:

```bash
# Check if git repo
git status

# Initialize if needed
git init
```

### "Index not found" for diff/impact commands

Build the index first:

```bash
infiniloom index .

# Then run diff/impact
infiniloom diff . --staged
infiniloom impact . src/main.rs
```

### Index is stale

Rebuild after significant changes:

```bash
# Force rebuild
infiniloom index . --force

# Or use incremental update
infiniloom index . --incremental

# Or enable watch mode
infiniloom index . --watch
```

### Diff shows wrong files

Check your reference format:

```bash
# Unstaged changes (working tree vs HEAD)
infiniloom diff .

# Staged changes only
infiniloom diff . --staged

# Last commit
infiniloom diff . HEAD~1

# Branch comparison
infiniloom diff . main..feature-branch
```

---

## Configuration Issues

### Config file not loading

Check file location and format:

```bash
# Config file locations (in order of precedence):
# 1. --config flag
# 2. .infiniloom.yaml in current directory
# 3. .infiniloom.toml in current directory
# 4. .infiniloom.json in current directory

# Verify config is being read
infiniloom info .
```

### Config values being ignored

CLI flags override config file values:

```bash
# This uses markdown even if config says xml
infiniloom pack . --format markdown
```

Environment variables also override config:

```bash
# This overrides config file
INFINILOOM_OUTPUT__FORMAT=json infiniloom pack .
```

### Invalid configuration error

Check YAML/TOML syntax:

```yaml
# Wrong - missing colon
output
  format: xml

# Right
output:
  format: xml
```

```yaml
# Wrong - tabs instead of spaces
output:
	format: xml

# Right - use spaces
output:
  format: xml
```

---

## CI/CD Issues

### "Permission denied" in CI

Use npx instead of global install:

```yaml
# Instead of:
- run: npm install -g infiniloom && infiniloom pack .

# Use:
- run: npx infiniloom pack .
```

### Timeout in CI

Large repos may need longer timeout and filtering:

```yaml
- run: |
    infiniloom pack . \
      --include "src/**" \
      --exclude "vendor/*" \
      --compression balanced
  timeout-minutes: 10
```

### Failing on security check

Configure security behavior:

```yaml
# .infiniloom.yaml
security:
  scan_secrets: true
  fail_on_secrets: false  # Don't fail, just report
  redact_secrets: true    # Redact in output
```

Or explicitly handle:

```yaml
- name: Generate context (redact secrets)
  run: infiniloom pack . --redact-secrets --output context.xml
```

---

## Language Binding Issues

### Python: "Module not found"

```bash
# Ensure pip installed to correct Python
which python
pip install infiniloom

# Or use virtual environment
python -m venv .venv
source .venv/bin/activate
pip install infiniloom
```

### Node.js: "Cannot find module"

```bash
# Check installation
npm list infiniloom-node

# Reinstall
npm uninstall infiniloom-node
npm install infiniloom-node
```

### "Invalid model" error

Check model name spelling:

```python
# Wrong
infiniloom.pack("/path", model="GPT-4")

# Right
infiniloom.pack("/path", model="gpt4")
infiniloom.pack("/path", model="gpt4o")
```

---

## Recently Fixed Issues

These issues have been fixed in recent versions. If you encounter them, upgrade to the latest version.

### Stack overflow on large repositories (Fixed in v0.4.8)

**Symptom:** Process crashes with stack overflow when processing repositories with 75,000+ files.

**Solution:** Upgrade to v0.4.8+ which uses non-recursive file traversal.

```bash
# Upgrade via npm
npm update infiniloom-node

# Upgrade via pip
pip install --upgrade infiniloom

# Upgrade via Homebrew
brew upgrade infiniloom
```

### Stack overflow on non-git directories (Fixed in v0.4.8)

**Symptom:** `scan()` or `pack()` crashes on very large standalone directories (not git repositories).

**Solution:** Same fix as above - upgrade to v0.4.8+.

### tiktoken panic on certain files (Fixed in v0.4.8)

**Symptom:** Crash with panic message related to tiktoken when counting tokens for certain unusual file contents.

**Solution:** Upgrade to v0.4.8+ which wraps tiktoken calls with panic recovery and falls back to estimation.

### countTokens crashes on null/undefined (Fixed in v0.4.8)

**Symptom:** Node.js `countTokens(null, 'claude')` throws TypeError instead of returning 0.

**Solution:** Upgrade to v0.4.8+ where null/undefined input returns 0.

### semanticCompress parameters not effective on small content (Fixed in v0.4.8)

**Symptom:** `semanticCompress()` with `budgetRatio < 1.0` doesn't compress text shorter than 100 characters.

**Solution:** Upgrade to v0.4.8+ where `budgetRatio` affects content as small as 10 characters.

---

## Getting Help

If your issue isn't listed here:

1. **Check the FAQ**: [FAQ.md](FAQ.md)
2. **Search issues**: [GitHub Issues](https://github.com/Topos-Labs/infiniloom/issues)
3. **Ask for help**: Open a new issue with:
   - Infiniloom version (`infiniloom --version`)
   - OS and version
   - Complete command you ran
   - Error message
   - Minimal reproduction steps

---

## Diagnostic Commands

```bash
# Show version and configuration
infiniloom info

# Show project-specific info
infiniloom info .

# Verbose scan for debugging
infiniloom scan . --verbose

# Check what's in .gitignore
git check-ignore *

# Test with minimal options
infiniloom pack . --format plain --compression none
```
