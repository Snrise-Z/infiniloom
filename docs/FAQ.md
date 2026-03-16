# Frequently Asked Questions

## General Questions

### What is Infiniloom?

Infiniloom is a high-performance tool that transforms codebases into optimized context for Large Language Models (LLMs) like Claude, GPT-4o/GPT-5, and Gemini. It extracts code, symbols, and structure from repositories and outputs them in formats compatible with each AI model.

### Why not just copy-paste my code?

Infiniloom provides several advantages:

1. **Token efficiency**: Smart compression reduces context size by 30-80%
2. **Model optimization**: Output formats designed for each LLM's strengths
3. **Security**: Automatic detection and redaction of secrets
4. **Structure**: Maintains file hierarchy and symbol relationships
5. **Filtering**: Include/exclude patterns, test exclusion, etc.
6. **Ranking**: PageRank-based importance scoring for symbols

### Is Infiniloom free?

Yes! Infiniloom is open source under the MIT license. Use it freely for personal and commercial projects.

### What languages does Infiniloom support?

Infiniloom detects 23 languages and provides full AST-based symbol extraction for 21 using Tree-sitter:

- **Systems**: Rust, C, C++, Go, Zig
- **Web**: JavaScript, TypeScript
- **Backend**: Python, Java, Kotlin, C#, Ruby, PHP, Swift
- **Functional**: Haskell, Elixir, OCaml, Scala
- **Scripting**: Lua, R, Bash
- **Mobile**: Dart
- **Infrastructure**: HCL/Terraform
- **Deprecated (no AST support)**: Clojure, F#

All text files are included regardless of language; symbol extraction is language-dependent.

**Note**: Clojure and F# are deprecated as of v0.7.0 (no compatible tree-sitter grammars). Files are still detected but receive text-only processing without AST symbols.

---

## Usage Questions

### Which output format should I use?

| LLM | Recommended Format | Why |
|-----|-------------------|-----|
| Claude | `xml` | Prompt caching hints, CDATA sections |
| GPT-4o/GPT-5 | `markdown` | Tables, code fences, headers |
| Gemini | `yaml` | Query at end, structured hierarchy |
| Limited context | `toon` | ~40% smaller than other formats |
| Programmatic | `json` | Standard parsing |

```bash
infiniloom pack . --format xml --model claude     # Claude
infiniloom pack . --format markdown --model gpt4o # GPT
infiniloom pack . --format toon                   # Maximum efficiency
```

### How do I reduce output size?

Several approaches, combinable:

```bash
# 1. Use compression
infiniloom pack . --compression aggressive  # 50-60% reduction

# 2. Set token budget
infiniloom pack . --max-tokens 50000

# 3. Filter files
infiniloom pack . --include "src/**" --exclude "tests/*"

# 4. Limit to top files
infiniloom pack . --top-files 50

# 5. Use TOON format
infiniloom pack . --format toon
```

### What is the best format for budget-constrained scenarios?

Use the TOON (Token-Optimized Object Notation) format. It achieves approximately 40% token reduction compared to JSON, making it the most token-efficient output format available. TOON works well with any LLM and is especially useful when you need to fit a large codebase into a limited context window.

```bash
# Maximum token savings with TOON format
infiniloom pack . --format toon

# Combine with compression for even greater reduction
infiniloom pack . --format toon --compression aggressive

# Combine with a token budget for strict limits
infiniloom pack . --format toon --max-tokens 50000
```

### How accurate is token counting?

- **OpenAI models**: Exact counting via tiktoken
- **Other models**: Calibrated estimation, ~95% accurate for prose, ~85% for code

For safety, use 90-95% of your context limit as the budget.

### Can I use Infiniloom with private repositories?

Yes. Infiniloom runs locally and never sends data anywhere. It works with any local directory.

### Can I pack remote repositories?

Yes, using GitHub syntax:

```bash
infiniloom pack github:owner/repo
infiniloom pack github:owner/repo --remote-branch develop
infiniloom pack github:owner/repo --sparse-path src  # Large repos
```

### How do I include test files?

Test files are excluded by default. Include them with:

```bash
infiniloom pack . --include-tests
```

### How do I exclude files?

Multiple approaches:

```bash
# 1. Exclude patterns (glob)
infiniloom pack . --exclude "tests/*" --exclude "docs/*"

# 2. Include patterns (only these)
infiniloom pack . --include "src/**/*.rs"

# 3. Config file
# .infiniloom.yaml
scan:
  exclude:
    - "tests/*"
    - "node_modules/*"
```

Files in `.gitignore` are also excluded by default.

---

## Performance Questions

### How fast is Infiniloom?

Very fast. Benchmarks on M2 MacBook Pro:

| Repository | Files | Time |
|------------|-------|------|
| Small (~100 files) | 174 | ~400ms |
| Medium (~1000 files) | 1,200 | ~2s |
| Large (~5000 files) | 5,000 | ~8s |

### Why is symbol extraction slow?

Symbol extraction uses Tree-sitter AST parsing, which is slower but provides:

- PageRank importance ranking
- Better repository maps
- Call graph analysis

Skip it if not needed:

```bash
infiniloom pack .  # Fast (no symbols by default)
infiniloom pack . --symbols  # Slower but more features
```

### How do I speed up repeated runs?

Enable caching:

```bash
infiniloom pack . --cache
```

This caches parsed file data in `.infiniloom/cache.bin`.

---

## Security Questions

### Does Infiniloom send my code anywhere?

No. Infiniloom runs entirely locally. No telemetry, no network requests (except when packing remote repositories).

### How does secret detection work?

Infiniloom scans for patterns matching:

- API keys (AWS, Google, GitHub, etc.)
- Access tokens
- Private keys (RSA, SSH, PGP)
- Database connection strings
- Environment variables with sensitive names

```bash
# Scan and report
infiniloom pack . --security-check

# Redact secrets
infiniloom pack . --redact-secrets
```

### Can I whitelist false positives?

Yes, in your config file:

```yaml
# .infiniloom.yaml
security:
  allowlist:
    - "EXAMPLE_KEY"
    - "test_token"
    - "localhost"
```

### Should I use --redact-secrets in CI?

Yes, recommended for any context that might be shared:

```bash
infiniloom pack . --redact-secrets --output context.xml
```

---

## Configuration Questions

### Where should I put my config file?

Infiniloom looks for config files in this order:

1. `--config <path>` flag (explicit)
2. `.infiniloom.yaml` in current directory
3. `.infiniloom.toml` in current directory
4. `.infiniloom.json` in current directory

### What's the config file format?

YAML (recommended), TOML, or JSON:

```yaml
# .infiniloom.yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000

scan:
  include:
    - "*.rs"
    - "*.py"
  exclude:
    - "tests/*"

security:
  scan_secrets: true
  redact_secrets: true
```

### How do I create a config file?

```bash
infiniloom init                # Creates .infiniloom.yaml
infiniloom init --format toml  # Creates .infiniloom.toml
infiniloom init --template rust  # Rust-optimized template
```

### Do CLI flags override config?

Yes. Order of precedence (highest to lowest):

1. CLI flags
2. Environment variables
3. Config file
4. Built-in defaults

---

## Diff/Impact Questions

### What is the symbol index?

The symbol index stores information about symbols (functions, classes, etc.) and their relationships (calls, imports). It enables fast diff context and impact analysis.

```bash
# Build index (once)
infiniloom index .

# Use for diff/impact
infiniloom diff . --staged
infiniloom impact . src/auth.rs
```

### How often should I rebuild the index?

- After major refactoring
- When adding new files
- Or use `--watch` for automatic updates:

```bash
infiniloom index . --watch
```

### What do diff depth levels mean?

| Depth | Includes |
|-------|----------|
| 1 | Changed files only |
| 2 | + Direct imports/importers (default) |
| 3 | + Second-degree dependencies |

```bash
infiniloom diff . --depth 1  # Minimal
infiniloom diff . --depth 3  # Comprehensive
```

### Can I analyze impact without git?

The `impact` command requires an index but not git:

```bash
infiniloom index .
infiniloom impact . src/main.rs  # What depends on this?
```

---

## Integration Questions

### Can I use Infiniloom in my Python scripts?

Yes:

```python
import infiniloom

# Pack repository
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")

# Scan statistics
stats = infiniloom.scan("/path/to/repo")

# Count tokens
tokens = infiniloom.count_tokens("Hello, world!", model="claude")
```

Install with: `pip install infiniloom`

### Can I use Infiniloom in Node.js?

Yes:

```javascript
const { pack, scan, countTokens } = require('infiniloom-node');

const context = pack('./repo', { format: 'xml', model: 'claude' });
const stats = scan('./repo');
const tokens = countTokens('Hello, world!', 'claude');
```

Install with: `npm install infiniloom-node`

### How do I use Infiniloom in CI/CD?

See the [CI/CD Integration Guide](guides/ci-integration.md). Quick example:

```yaml
# GitHub Actions
- name: Generate context
  run: |
    npm install -g infiniloom
    infiniloom pack . --format xml --output context.xml
```

### Can I pipe output to clipboard?

Yes, on macOS:

```bash
infiniloom pack . | pbcopy
```

Or use the built-in flag:

```bash
infiniloom pack . --copy-to-clipboard
```

---

## Troubleshooting

### Why do I get "command not found"?

See [Troubleshooting: Installation Issues](TROUBLESHOOTING.md#installation-issues)

### Why is my output empty?

See [Troubleshooting: Output is empty](TROUBLESHOOTING.md#output-is-empty-or-very-small)

### Why doesn't my config file work?

See [Troubleshooting: Configuration Issues](TROUBLESHOOTING.md#configuration-issues)

---

## More Questions?

- **Check the docs**: [Documentation Index](README.md)
- **Search issues**: [GitHub Issues](https://github.com/Topos-Labs/infiniloom/issues)
- **Open new issue**: Include version, OS, command, and error message
