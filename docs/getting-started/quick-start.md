# Quick Start

Get productive with Infiniloom in 5 minutes.

## 1. Install

```bash
npm install -g infiniloom
```

## 2. Pack Your First Repository

```bash
# Navigate to your project
cd /path/to/your/project

# Pack for Claude (XML format)
infiniloom pack . --format xml --output context.xml
```

This creates `context.xml` containing your entire codebase in a Claude-optimized format.

## 3. Use with LLMs

### Claude

1. Open [Claude](https://claude.ai)
2. Upload `context.xml` or paste its contents
3. Ask Claude about your code

**Pro tip**: Claude's prompt caching works well with Infiniloom's XML format. Include the context at the start of conversations for best results.

### GPT-4 / ChatGPT

```bash
infiniloom pack . --format markdown --output context.md
```

### Gemini

```bash
infiniloom pack . --format yaml --output context.yaml
```

## 4. Explore Your Repository

### Scan Statistics

```bash
infiniloom scan .
```

Output:

```
Repository: my-project
  Files: 174
  Languages:
    typescript: 89 files (51.1%)
    javascript: 45 files (25.9%)
    json: 20 files (11.5%)
  Token Counts (claude):
    Total: 245,891
```

### Generate Symbol Map

```bash
infiniloom map . --budget 2000
```

Shows the most important symbols in your codebase, ranked by PageRank.

## 5. Common Workflows

### AI Code Review

```bash
# Build symbol index (once)
infiniloom index .

# Get context for staged changes
infiniloom diff . --staged --include-diff

# Pipe to clipboard (macOS)
infiniloom diff . --staged | pbcopy
```

### Focus on Specific Files

```bash
# Only TypeScript files
infiniloom pack . --include "*.ts" --include "*.tsx"

# Exclude tests
infiniloom pack . --exclude "**/*.test.*" --exclude "**/__tests__/*"
```

### Token Budget

```bash
# Limit to 50K tokens (fits GPT-4's context)
infiniloom pack . --max-tokens 50000

# Or limit by file count
infiniloom pack . --top-files 50
```

### Security Check

```bash
# Scan for secrets
infiniloom pack . --security-check

# Redact secrets automatically
infiniloom pack . --redact-secrets
```

## 6. Create a Config File

For repeatable settings:

```bash
infiniloom init
```

Creates `.infiniloom.yaml`:

```yaml
output:
  format: xml
  model: claude
  compression: balanced

scan:
  exclude:
    - "node_modules/*"
    - "dist/*"

security:
  scan_secrets: true
```

Now `infiniloom pack .` uses these defaults.

## Next Steps

- [Configuration Guide](../CONFIGURATION.md) — All config options
- [Command Reference](../commands/) — Detailed command docs
- [LLM Optimization](../guides/llm-optimization.md) — Model-specific tips
