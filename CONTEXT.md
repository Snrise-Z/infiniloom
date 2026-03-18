# CONTEXT.md — Runtime Rules for AI Agents Using Infiniloom

This document is a runtime reference for AI agents that invoke Infiniloom to gather codebase context. It is not a development guide for Infiniloom itself.

## Rules of Engagement

1. **Scan before you pack.** Always run `infiniloom scan` first to understand a repository's size, language breakdown, and token counts. This prevents blowing your context window with a blind `pack`.

2. **Protect the context window.** Use `--max-tokens` (alias `--budget`) and `--compression` on every `pack` call. A 200K-token repo packed without limits will waste context or fail silently when truncated by the model.

3. **Match the format to the model.** Use `--format xml` for Claude, `--format markdown` for GPT models, `--format yaml` for Gemini and other models. The wrong format costs tokens and hurts comprehension.

## Core Syntax

```
infiniloom <command> [options] [path]
```

Path defaults to `.` (current directory) for all commands.

## Command Decision Matrix

| If you need... | Use this command |
|---|---|
| Full repo context for an LLM | `infiniloom pack` |
| Quick stats and token counts | `infiniloom scan` |
| Key symbols overview (PageRank-ranked) | `infiniloom map` |
| RAG / vector DB chunks | `infiniloom embed` |
| Multi-turn conversation chunks | `infiniloom chunk` |
| Context for a code change (diff + dependents) | `infiniloom diff` |
| Impact analysis of a file or symbol | `infiniloom impact` |
| Document ingestion (Markdown, HTML, CSV, DOCX) | `infiniloom ingest` |
| Build symbol index for fast diff/impact queries | `infiniloom index` |
| Create a config file | `infiniloom init` |
| Show version and config info | `infiniloom info` |

## Key Flags Reference

### Universal flags (available on most commands)

| Flag | Short | Description |
|---|---|---|
| `--include <glob>` | `-i` | Include only files matching pattern (repeatable) |
| `--exclude <glob>` | `-e` | Exclude files matching pattern (repeatable) |
| `--include-tests` | | Include test files (excluded by default) |
| `--verbose` | `-v` | Verbose output |
| `--model <name>` | `-m` | Target model for token counting |

### Pack-specific flags

| Flag | Short | Description |
|---|---|---|
| `--format <fmt>` | `-f` | Output format: `xml`, `markdown`, `yaml`, `json`, `toon`, `plain` |
| `--compression <level>` | `-c` | `none`, `minimal`, `balanced`, `aggressive`, `extreme`, `focused`, `semantic` |
| `--max-tokens <n>` | `-t` | Token budget (0 = no limit). Alias: `--budget` / `-b` |
| `--output <path>` | `-o` | Write to file instead of stdout |
| `--no-symbols` | | Skip symbol extraction (faster) |
| `--full` | | Enable symbols + repo map + PageRank ranking |
| `--no-content` | | Metadata only, exclude file contents |
| `--redact-secrets` | | Replace detected secrets with `[REDACTED]` |
| `--security-check` | | Scan and report secrets without redacting |
| `--remove-comments` | | Strip comments from code |
| `--remove-empty-lines` | | Strip blank lines |
| `--copy-to-clipboard` | | Copy output to system clipboard |
| `--cache` | | Enable incremental caching for repeated scans |
| `--watch` | | Re-generate on file changes |

### Scan-specific flags

| Flag | Description |
|---|---|
| `--json` | Output as JSON (machine-readable) |
| `--security-check` | Report detected secrets |
| `--sample <n>` | Sample N random files (for large repos) |
| `--sample-percent <p>` | Sample P% of files |

### Diff-specific flags

| Flag | Short | Description |
|---|---|---|
| `--staged` | | Use staged changes instead of unstaged |
| `--depth <1-3>` | `-d` | Context depth: 1=containing, 2=direct deps, 3=transitive |
| `--budget <n>` | `-b` | Token budget (default: 50000) |
| `--include-diff` | | Include the actual +/- diff lines |

### Embed-specific flags

| Flag | Description |
|---|---|
| `--diff` | Only output changed chunks (incremental) |
| `--max-tokens <n>` | Max tokens per chunk (default: 1000) |
| `--hierarchy` | Generate summary chunks for classes/structs |
| `--streaming` | Low-memory streaming mode for large repos |
| `--no-security-scan` | Disable secret scanning |

## Supported Models

`claude`, `gpt52`, `gpt51`, `gpt5`, `o4-mini`, `o3`, `o1`, `gpt4o`, `gpt4o-mini`, `gpt4`, `gpt35-turbo`, `gemini`, `llama`, `codellama`, `mistral`, `deepseek`, `qwen`, `cohere`, `grok`

## Compression Levels

| Level | Effect |
|---|---|
| `none` | Full source, no changes |
| `minimal` | Remove empty lines |
| `balanced` | Remove comments |
| `aggressive` | Signatures only |
| `extreme` | Key symbols only |
| `focused` | Key symbols with small context |
| `semantic` | Heuristic chunking |

## Usage Examples

### 1. Assess a repo before packing

```bash
infiniloom scan /path/to/repo --model claude --json
```

Check the token count in the output. If it exceeds your budget, use `--compression` or `--include` patterns when packing.

### 2. Pack a repo for Claude with a token budget

```bash
infiniloom pack /path/to/repo -f xml -m claude -t 80000 -c balanced
```

Produces Claude-optimized XML, capped at 80K tokens, with comments stripped.

### 3. Get context for a pull request review

```bash
infiniloom diff /path/to/repo main..feature-branch -f xml --include-diff --depth 2 -b 50000
```

Returns changed files, their direct dependents, and the actual diff lines, all within a 50K token budget.

### 4. Generate a symbol map for architecture questions

```bash
infiniloom map /path/to/repo --budget 3000 -i "src/**"
```

Returns the top PageRank-scored symbols from `src/`, useful for answering "what are the key abstractions?" questions.

### 5. Incremental RAG pipeline update

```bash
infiniloom embed /path/to/repo --diff -o updates.jsonl --max-tokens 512
```

Outputs only chunks that changed since the last manifest, ready to upsert into a vector database.

## Performance Tips

- **Skip symbols for speed**: Omit `--full` and use `--no-symbols` when you only need file contents. Symbol extraction is the slowest step.
- **Use compression**: `--compression aggressive` reduces output by 60-80% by emitting signatures only. Good for large repos that exceed your budget.
- **Scope with patterns**: `-i "src/**/*.rs" -e "vendor/*"` focuses on relevant code and avoids noise.
- **Use `--cache`**: On repeated `pack` calls against the same repo, `--cache` skips re-parsing unchanged files.
- **Sample large repos**: `infiniloom scan --sample 200` gives a quick estimate without scanning every file.
- **Stream large embeds**: `infiniloom embed --streaming` processes files in batches to keep memory usage low.

## Output Format Quick Reference

| Format | Flag | Best for | Token efficiency |
|---|---|---|---|
| XML | `--format xml` | Claude | Baseline |
| Markdown | `--format markdown` | GPT-4o, GPT-5 | Similar to XML |
| YAML | `--format yaml` | Gemini, others | Similar to XML |
| TOON | `--format toon` | Any (when tokens are scarce) | ~40% smaller |
| JSON | `--format json` | Programmatic consumption | Larger |
| Plain | `--format plain` | Simple pipelines | Smallest |
