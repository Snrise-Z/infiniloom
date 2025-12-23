# `infiniloom info` Command

## Overview

The `info` command displays version information, supported formats, models, compression levels, and optionally project-specific configuration. It's useful for understanding available options and debugging configuration issues.

## Synopsis

```bash
infiniloom info [PATH]
```

**PATH** (optional): Path to a project to show project-specific configuration

## Description

The `info` command displays:

1. **Version Information**: CLI and engine versions
2. **Supported Formats**: Output format options with descriptions
3. **Supported Models**: All tokenizer models with their characteristics
4. **Compression Levels**: Available compression options with reduction percentages
5. **Project Config** (if path provided): Loaded configuration from project

## Output

### Without Path (General Info)

```
Infiniloom - Repository Context Generator
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Version:      0.1.0
  Engine:       0.1.0

  Supported Formats:
    xml       - Claude-optimized (with cache hints)
    markdown  - GPT-optimized (with code blocks)
    json      - Generic structured format
    yaml      - Gemini-optimized (query at end)
    toon      - Most token-efficient (~40% smaller)
    plain     - Simple plain text (no markup)

  Supported Models:
    claude      - Anthropic Claude (default)
    gpt52       - OpenAI GPT-5.2 (o200k_base encoding)
    gpt51       - OpenAI GPT-5.1 (o200k_base encoding)
    gpt5        - OpenAI GPT-5 (o200k_base encoding)
    o4-mini     - OpenAI O4-mini reasoning model
    o3          - OpenAI O3 reasoning model
    o1          - OpenAI O1 reasoning model
    gpt4o       - OpenAI GPT-4o (o200k_base encoding)
    gpt4o-mini  - OpenAI GPT-4o-mini (o200k_base encoding)
    gpt4        - OpenAI GPT-4/GPT-4 Turbo (cl100k_base, legacy)
    gpt35-turbo - OpenAI GPT-3.5-turbo (cl100k_base, legacy)
    gemini      - Google Gemini
    llama       - Meta Llama 3/4
    codellama   - Meta CodeLlama (optimized for code)
    mistral     - Mistral AI (Large, Medium, Codestral)
    deepseek    - DeepSeek (V3, R1, Coder)
    qwen        - Alibaba Qwen (Qwen3, Qwen2.5)
    cohere      - Cohere (Command R+, Command R)
    grok        - xAI Grok (Grok 2, Grok 3)

  Compression Levels:
    none      - No compression (0%)
    minimal   - Whitespace only (~15%)
    balanced  - Remove comments (~35%)
    aggressive - Signatures only (~60%)
    extreme   - Key symbols only (~80%)
    focused   - Key symbols with context (~75%)
    semantic  - Heuristic chunking (~65%, NOT neural)
```

### With Path (Project Info)

```bash
infiniloom info /path/to/project
```

```
Infiniloom - Repository Context Generator
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Version:      0.1.0
  Engine:       0.1.0

  Project:
    Path:       /path/to/project
    Config:     Found (.infiniloom.yaml)
      Format:     xml
      Model:      claude
      Compression: balanced
      Budget:     100000 tokens

  [... rest of general info ...]
```

## Technical Details

### Format Characteristics

| Format | Tokens per File | Best For | Key Feature |
|--------|-----------------|----------|-------------|
| XML | Baseline | Claude | Cache control hints |
| Markdown | ~10% more | GPT models | Natural code blocks |
| JSON | ~15% more | Tooling | Structured parsing |
| YAML | ~12% more | Gemini | Query placement |
| TOON | ~40% less | Any | Maximum efficiency |
| Plain | ~5% less | Simple use | No overhead |

### Model Tokenization Methods

| Model | Method | Accuracy |
|-------|--------|----------|
| OpenAI (o200k_base) | tiktoken | 100% |
| OpenAI (cl100k_base) | tiktoken | 100% |
| Claude | Calibrated | ~95% |
| Gemini | Calibrated | ~95% |
| Llama | Calibrated | ~95% |
| Others | Calibrated | ~90-95% |

### Compression Level Details

| Level | Transforms Applied | Size Reduction |
|-------|-------------------|----------------|
| None | None | 0% |
| Minimal | Remove empty lines | ~15% |
| Balanced | Minimal + remove comments | ~35% |
| Aggressive | Keep only signatures | ~60% |
| Extreme | Keep only symbol names | ~80% |
| Semantic | Heuristic compression | ~65% |

## Examples

### Basic Info

```bash
# Show general information
infiniloom info
```

### Project Configuration Check

```bash
# Check config for current directory
infiniloom info .

# Check config for specific project
infiniloom info /path/to/myproject
```

### Scripting

```bash
# Get version for scripts
infiniloom info | grep "Version:" | awk '{print $2}'
```

## Best Practices

### Configuration Verification

Before packing, verify configuration is loaded correctly:

```bash
infiniloom info .
# Check that Format, Model, Compression match expectations
```

### Model Selection Guide

Use `info` to understand model options:

```bash
infiniloom info | grep -A 20 "Supported Models:"
```

Then select based on target:

| Target | Recommended Model |
|--------|------------------|
| Claude API | `claude` |
| OpenAI latest | `gpt4o` or `o1` |
| OpenAI legacy | `gpt4` |
| Google | `gemini` |
| Open source | `llama`, `mistral`, `qwen` |

### Format Selection Guide

```bash
infiniloom info | grep -A 10 "Supported Formats:"
```

Then select based on use case:

| Use Case | Recommended Format |
|----------|-------------------|
| Claude | `xml` |
| GPT | `markdown` |
| Gemini | `yaml` |
| CI/CD | `json` |
| Maximum tokens | `toon` |
| Email/docs | `plain` |

## Potential Improvements

### 1. Machine-Readable Output

```bash
# Future: JSON output for tooling
infiniloom info --json
```

### 2. Capability Detection

```bash
# Future: show what's available on this system
infiniloom info --capabilities
# Shows: Available languages, GPU support, etc.
```

### 3. Model Recommendations

```bash
# Future: recommend based on project
infiniloom info /path/to/project --recommend
# Output: "Recommended: claude (Python project, 45K tokens)"
```

### 4. Config Validation

```bash
# Future: validate project config
infiniloom info /path/to/project --validate
# Output: "Config valid" or "Warning: unknown option 'foo'"
```

### 5. Environment Variables Display

```bash
# Future: show active environment overrides
infiniloom info --env
# Shows: INFINILOOM_OUTPUT__FORMAT=xml, etc.
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Invalid path (if path provided) |

## Related Commands

- [`init`](init.md) - Create configuration file
- [`pack`](pack.md) - Use configuration for packing
- [`scan`](scan.md) - Scan with model-specific counting
