# Infiniloom Recipes

**Copy-paste solutions for common tasks.**

---

## Table of Contents

1. [AI Code Review with Claude](#1-ai-code-review-with-claude)
2. [AI Code Review with GPT-4o](#2-ai-code-review-with-gpt-4o)
3. [Build a RAG Pipeline with Pinecone](#3-build-a-rag-pipeline-with-pinecone)
4. [Incremental Vector DB Updates](#4-incremental-vector-db-updates)
5. [Security Audit Before AI](#5-security-audit-before-ai)
6. [Process Large Monorepo](#6-process-large-monorepo)
7. [Multi-Turn Conversation Chunking](#7-multi-turn-conversation-chunking)
8. [Python Project Context](#8-python-project-context)
9. [TypeScript/Node.js Project Context](#9-typescriptnodejs-project-context)
10. [Rust Project Context](#10-rust-project-context)
11. [PR Context for Review](#11-pr-context-for-review)
12. [Impact Analysis Before Refactoring](#12-impact-analysis-before-refactoring)
13. [CI/CD Integration](#13-cicd-integration)
14. [Watch Mode for Development](#14-watch-mode-for-development)
15. [Remote Repository Analysis](#15-remote-repository-analysis)

---

## 1. AI Code Review with Claude

**Goal:** Generate XML context optimized for Claude, copy to clipboard.

```bash
# macOS
infiniloom pack . -f xml -o context.xml && cat context.xml | pbcopy

# Linux (with xclip)
infiniloom pack . -f xml -o context.xml && cat context.xml | xclip -selection clipboard

# Windows (PowerShell)
infiniloom pack . -f xml -o context.xml; Get-Content context.xml | Set-Clipboard
```

**With security scanning:**
```bash
infiniloom pack . -f xml --redact-secrets -o context.xml
```

---

## 2. AI Code Review with GPT-4o

**Goal:** Generate Markdown context optimized for GPT-4o with accurate token count.

```bash
infiniloom pack . -f markdown -m gpt4o -o context.md

# Limit to 100K tokens
infiniloom pack . -f markdown -m gpt4o --max-tokens 100000 -o context.md
```

---

## 3. Build a RAG Pipeline with Pinecone

**Goal:** Generate chunks for Pinecone import.

```bash
# Generate chunks
infiniloom embed . --max-tokens 1500 -o chunks.jsonl
```

**Python import script:**
```python
import json
from pinecone import Pinecone
from openai import OpenAI

# Initialize clients
pc = Pinecone(api_key="your-api-key")
index = pc.Index("code-embeddings")
openai = OpenAI()

# Load and embed chunks
with open("chunks.jsonl") as f:
    for line in f:
        chunk = json.loads(line)

        # Get embedding
        response = openai.embeddings.create(
            model="text-embedding-3-small",
            input=chunk["content"]
        )
        embedding = response.data[0].embedding

        # Upsert to Pinecone
        index.upsert(vectors=[{
            "id": chunk["id"],
            "values": embedding,
            "metadata": {
                "file": chunk["source"]["file"],
                "symbol": chunk["source"]["symbol"],
                "language": chunk["source"]["language"],
                "kind": chunk["kind"],
                "tokens": chunk["tokens"],
                "content": chunk["content"][:1000]  # Truncate for metadata
            }
        }])
```

---

## 4. Incremental Vector DB Updates

**Goal:** Only re-embed changed code.

```bash
# First run - generates manifest + all chunks
infiniloom embed . -o chunks.jsonl

# After code changes - only outputs changed chunks
infiniloom embed . --diff -o updates.jsonl
```

**Python update script:**
```python
import json

with open("updates.jsonl") as f:
    updates = [json.loads(line) for line in f]

# updates contains only added/modified chunks
# Use chunk["id"] to upsert (add new or update existing)
for chunk in updates:
    # Embed and upsert to vector DB
    pass
```

---

## 5. Security Audit Before AI

**Goal:** Scan for secrets before sharing code with AI.

```bash
# Scan only (report findings)
infiniloom pack . --security-check 2>&1 | head -50

# Generate context with secrets redacted
infiniloom pack . --redact-secrets -o safe-context.xml
```

**For embed command (security scan is default):**
```bash
# Default: security scan enabled
infiniloom embed . -o chunks.jsonl

# Explicit: fail if secrets found
infiniloom embed . --fail-on-secrets -o chunks.jsonl
```

---

## 6. Process Large Monorepo

**Goal:** Handle 100K+ file repository efficiently.

```bash
# Step 1: Scan to understand size
infiniloom scan . --json | jq '.total_files, .total_tokens'

# Step 2: Focus on specific packages
infiniloom pack . \
  --include "packages/core/**" \
  --include "packages/api/**" \
  --exclude "**/node_modules/**" \
  --exclude "**/dist/**" \
  --compression balanced \
  --max-tokens 80000 \
  -o context.xml

# Step 3: Or use sparse checkout for remotes
infiniloom pack github:large/monorepo \
  --sparse-path packages/core \
  --sparse-path packages/shared
```

---

## 7. Multi-Turn Conversation Chunking

**Goal:** Split large repo into digestible chunks for multi-turn AI conversations.

```bash
# Group by module/directory
infiniloom chunk . --strategy module --max-tokens 50000 --output chunks/

# Result: chunks/chunk_001.xml, chunks/chunk_002.xml, etc.
```

**Usage:**
1. Send chunk_001.xml: "Here's part 1 of my codebase..."
2. AI responds with analysis
3. Send chunk_002.xml: "Here's part 2..."
4. Continue conversation with full context

---

## 8. Python Project Context

**Goal:** Generate context for a Python project.

```bash
infiniloom pack . \
  --include "**/*.py" \
  --include "pyproject.toml" \
  --include "requirements.txt" \
  --exclude "**/__pycache__/**" \
  --exclude "**/venv/**" \
  --exclude "**/.venv/**" \
  --exclude "**/site-packages/**" \
  --include-tests \
  -o context.xml
```

---

## 9. TypeScript/Node.js Project Context

**Goal:** Generate context for a TypeScript/Node.js project.

```bash
infiniloom pack . \
  --include "**/*.ts" \
  --include "**/*.tsx" \
  --include "**/*.js" \
  --include "**/*.jsx" \
  --include "package.json" \
  --include "tsconfig.json" \
  --exclude "**/node_modules/**" \
  --exclude "**/dist/**" \
  --exclude "**/build/**" \
  --exclude "**/*.d.ts" \
  -o context.xml
```

---

## 10. Rust Project Context

**Goal:** Generate context for a Rust project.

```bash
infiniloom pack . \
  --include "**/*.rs" \
  --include "Cargo.toml" \
  --include "Cargo.lock" \
  --exclude "**/target/**" \
  --full \
  -o context.xml
```

---

## 11. PR Context for Review

**Goal:** Get context for reviewing a pull request.

```bash
# Build index (one time per repo)
infiniloom index .

# Get context for staged changes (before commit)
infiniloom diff . --staged --include-diff -f markdown -o pr-context.md

# Get context for branch comparison
infiniloom diff . main..feature-branch --include-diff -o pr-context.xml

# Include file history
infiniloom diff . main..feature-branch --include-history --history-count 5
```

---

## 12. Impact Analysis Before Refactoring

**Goal:** Understand what will break if you change a file or function.

```bash
# Build index first
infiniloom index .

# What depends on this file?
infiniloom impact . src/auth/login.rs

# What calls this function?
infiniloom impact . --symbol "authenticate"

# Full transitive analysis
infiniloom impact . src/core/database.py --depth 3 --json
```

---

## 13. CI/CD Integration

**Goal:** Generate context as part of CI/CD pipeline.

**GitHub Actions:**
```yaml
name: Generate AI Context

on:
  pull_request:
    branches: [main]

jobs:
  context:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Infiniloom
        run: npm install -g infiniloom

      - name: Generate PR Context
        run: |
          infiniloom index .
          infiniloom diff . origin/main..HEAD \
            --include-diff \
            --redact-secrets \
            -o pr-context.xml

      - name: Upload Context
        uses: actions/upload-artifact@v4
        with:
          name: pr-context
          path: pr-context.xml
```

---

## 14. Watch Mode for Development

**Goal:** Automatically regenerate context when files change.

```bash
# Watch and regenerate on changes
infiniloom pack . --watch -o context.xml

# With caching for faster regeneration
infiniloom pack . --watch --cache -o context.xml
```

**Note:** Watch mode requires output file (`-o`).

---

## 15. Remote Repository Analysis

**Goal:** Analyze a GitHub repository without cloning.

```bash
# Analyze public repository
infiniloom pack github:facebook/react -o react-context.xml

# Specific branch
infiniloom pack github:owner/repo --remote-branch develop

# Sparse checkout (for large monorepos)
infiniloom pack github:kubernetes/kubernetes \
  --sparse-path pkg/api \
  --sparse-path pkg/controller \
  -o k8s-api-context.xml
```

---

## Bonus: Common Aliases

Add to your shell profile (`~/.bashrc`, `~/.zshrc`):

```bash
# Quick context generation
alias ctx='infiniloom pack . -o context.xml && echo "Context saved to context.xml"'
alias ctxcp='infiniloom pack . | pbcopy && echo "Context copied to clipboard"'

# Quick diff context
alias ctxdiff='infiniloom index . && infiniloom diff . --staged --include-diff'

# Quick embed
alias embed='infiniloom embed . -o chunks.jsonl && echo "Chunks saved to chunks.jsonl"'

# Scan stats
alias repostats='infiniloom scan . --json | jq'
```

---

## See Also

- [Cheat Sheet](CHEATSHEET.md) - Command reference
- [Quick Reference](QUICK_REFERENCE.md) - One-page printable
- [Command Reference](commands/) - Full documentation
- [Configuration Guide](CONFIGURATION.md) - Config options
