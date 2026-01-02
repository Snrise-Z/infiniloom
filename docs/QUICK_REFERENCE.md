# Infiniloom Quick Reference Card

**One-page reference for the most common operations.**

---

## Essential Commands

```bash
# Generate AI-ready context (paste into Claude/GPT)
infiniloom pack .

# Generate chunks for vector databases
infiniloom embed . -o chunks.jsonl

# View repository statistics
infiniloom scan .

# Get context for code changes
infiniloom diff .
```

---

## pack - Generate AI Context

```bash
infiniloom pack [PATH] [OPTIONS]

# Output formats (--format, -f)
-f xml          # Claude (default, prompt caching)
-f markdown     # GPT-4o/GPT-5
-f yaml         # Gemini
-f toon         # Token-efficient (~40% smaller)
-f json         # Programmatic use

# Token models (--model, -m)
-m claude       # Anthropic Claude (default)
-m gpt4o        # OpenAI GPT-4o
-m gpt5         # OpenAI GPT-5
-m gemini       # Google Gemini

# Common options
-o context.xml  # Output to file
--max-tokens N  # Limit output tokens
--compression X # none/minimal/balanced/aggressive/extreme
--security-check       # Scan for secrets
--redact-secrets       # Redact detected secrets
-i "*.rs"       # Include pattern
-e "tests/*"    # Exclude pattern
--include-tests # Include test files
--full          # Enable PageRank + symbols
```

---

## embed - Generate RAG Chunks

```bash
infiniloom embed [PATH] [OPTIONS]

# Output options
-o chunks.jsonl # Output file (default: stdout)
--format jsonl  # JSONL (default) or json
--diff          # Only output changed chunks

# Token options
--max-tokens N  # Max tokens per chunk (default: 1000)
--min-tokens N  # Min tokens per chunk (default: 50)
--context-lines N # Context around symbols (default: 5)

# Common patterns
infiniloom embed . -o chunks.jsonl
infiniloom embed . --diff -o updates.jsonl
infiniloom embed . --max-tokens 1500  # For voyage-code-2
```

**Chunk Output:**
```json
{
  "id": "ec_a1b2c3d4...",
  "content": "fn foo() {...}",
  "tokens": 150,
  "kind": "function",
  "source": { "file": "...", "symbol": "...", "fqn": "..." },
  "context": { "docstring": "...", "calls": [...], "tags": [...] }
}
```

---

## scan - Repository Statistics

```bash
infiniloom scan [PATH] [OPTIONS]

infiniloom scan .           # Basic scan
infiniloom scan . -m gpt4o  # Token count for model
infiniloom scan . --json    # JSON output
infiniloom scan . -v        # Show file list
```

---

## diff - Change Context

```bash
infiniloom diff [PATH] [REF] [OPTIONS]

# Build index first (one time)
infiniloom index .

# Then get diff context
infiniloom diff .           # Unstaged changes
infiniloom diff . --staged  # Staged changes
infiniloom diff . HEAD~1    # Last commit
infiniloom diff . main..feature  # Branch comparison
--include-diff    # Include +/- lines
--depth 1/2/3     # Context depth
```

---

## Filtering (All Commands)

```bash
-i, --include "PATTERN"   # Include files matching glob
-e, --exclude "PATTERN"   # Exclude files matching glob
--include-tests           # Include test files
--include-docs            # Include documentation
--hidden                  # Include hidden files
--no-gitignore            # Ignore .gitignore
```

---

## Output Format by Model

| Model | Format | Command |
|-------|--------|---------|
| Claude | XML | `infiniloom pack . -f xml` |
| GPT-4o/5 | Markdown | `infiniloom pack . -f markdown` |
| Gemini | YAML | `infiniloom pack . -f yaml` |
| Any (minimal) | TOON | `infiniloom pack . -f toon` |

---

## Compression Levels

| Level | Reduction | Use Case |
|-------|-----------|----------|
| `none` | 0% | Full content |
| `minimal` | ~15% | Remove empty lines |
| `balanced` | ~35% | Remove comments (default) |
| `aggressive` | ~60% | Signatures only |
| `extreme` | ~80% | Key symbols only |

---

## Configuration File

Create `.infiniloom.yaml` in project root:

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000

scan:
  include: ["*.rs", "*.py"]
  exclude: ["tests/*", "vendor/*"]
  include_tests: false

security:
  scan_secrets: true
  redact_secrets: true
```

---

## Environment Variables

```bash
INFINILOOM_OUTPUT__MODEL=claude
INFINILOOM_OUTPUT__FORMAT=xml
INFINILOOM_OUTPUT__COMPRESSION=balanced
INFINILOOM_SECURITY__SCAN_SECRETS=true
```

---

## Quick Workflows

**AI Code Review:**
```bash
infiniloom index .                    # One time
infiniloom diff . --staged | pbcopy   # Copy context
```

**Build RAG Pipeline:**
```bash
infiniloom embed . -o chunks.jsonl    # Generate chunks
# Import into Pinecone/Weaviate/Qdrant
```

**Large Repository:**
```bash
infiniloom pack . \
  --include "src/**" \
  --compression aggressive \
  --max-tokens 80000
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |

---

## Help

```bash
infiniloom --help           # General help
infiniloom pack --help      # Command help
infiniloom info             # Version and config
```

---

**More:** [Full Documentation](https://github.com/Topos-Labs/infiniloom/docs) | [Cheat Sheet](CHEATSHEET.md) | [Troubleshooting](TROUBLESHOOTING.md)
