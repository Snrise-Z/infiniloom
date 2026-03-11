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

### GPT-4o / GPT-5 / ChatGPT

```bash
infiniloom pack . --format markdown --output context.md
```

### Gemini

```bash
infiniloom pack . --format yaml --output context.yaml
```

## 4. Generate RAG Chunks (Vector Databases)

For semantic search and RAG pipelines, use `embed` instead of `pack`:

```bash
# Generate content-addressable chunks
infiniloom embed . -o chunks.jsonl

# Optimized for Voyage embeddings (1500 tokens)
infiniloom embed . --max-tokens 1500 -o chunks.jsonl
```

Each chunk includes:
- **Content-addressable ID** (`ec_a1b2c3...`) - same code = same ID across repos
- **AST boundaries** - never splits mid-function
- **Semantic metadata** - docstrings, call graph, auto-tags

```bash
# After code changes, only get what changed
infiniloom embed . --diff -o updates.jsonl
```

Output format ready for Pinecone, Weaviate, Qdrant, or any vector database.

## 5. Explore Your Repository

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

## 6. Common Workflows

### AI Code Review

```bash
# Build symbol index (once)
infiniloom index .

# Get context for staged changes
infiniloom diff . --staged --include-diff

# Pipe to clipboard (macOS)
infiniloom diff . --staged | pbcopy
```

### RAG Pipeline

```bash
# Initial ingestion
infiniloom embed . -o chunks.jsonl
# Ingest into your vector DB...

# Incremental updates (only changed)
infiniloom embed . --diff -o updates.jsonl
# Upsert/delete in your vector DB...
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
# Limit to 50K tokens (fits smaller context windows)
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

### Document Ingestion

```bash
# Convert a DOCX to LLM-ready XML
infiniloom ingest report.docx -o report.xml

# Process with PII redaction
infiniloom ingest contract.docx --redact-pii -o safe.xml

# Chunk a large document for multi-turn conversation
infiniloom ingest whitepaper.md --chunk --max-chunk-tokens 6000
```

## 7. Create a Config File

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

- [Reference](../REFERENCE.md) — All commands and options at a glance
- [Recipes](../RECIPES.md) — Ready-to-use code patterns
- [Configuration Guide](../CONFIGURATION.md) — All config options
- [Command Reference](../commands/) — Detailed command docs
- [LLM Optimization](../guides/llm-optimization.md) — Model-specific tips
- [Large Repositories](../guides/large-repos.md) — Handling big codebases
- [CI/CD Integration](../guides/ci-integration.md) — Automation workflows
- [FAQ](../FAQ.md) — Frequently asked questions
- [Troubleshooting](../TROUBLESHOOTING.md) — Common issues and solutions
