# Supported Tokenizer Models

Infiniloom supports **27 tokenizer models** across 9 providers. Token counts are used for budget management, output sizing, and model-specific optimization.

## Tokenization Methods

| Method | Models | Accuracy | How It Works |
|--------|--------|----------|--------------|
| **Exact (tiktoken)** | All OpenAI models | 100% | Uses the official tiktoken BPE tokenizer library |
| **Calibrated estimation** | All other providers | ~95% prose, ~85% code | Character-count heuristic calibrated per provider |

## OpenAI Models - Exact via tiktoken

### o200k_base encoding (modern)

| Model | CLI Name | Context Window | Notes |
|-------|----------|---------------|-------|
| GPT-5.2 | `gpt52`, `gpt-5.2` | 128K | Latest flagship (Dec 2025) |
| GPT-5.2 Pro | `gpt52pro`, `gpt-5.2-pro` | 128K | Enhanced variant |
| GPT-5.1 | `gpt51`, `gpt-5.1` | 128K | Previous flagship |
| GPT-5.1 Mini | `gpt51-mini`, `gpt-5.1-mini` | 128K | Smaller variant |
| GPT-5.1 Codex | `gpt51-codex`, `gpt-5.1-codex` | 128K | Code-specialized |
| GPT-5 | `gpt5`, `gpt-5` | 128K | Original GPT-5 |
| GPT-5 Mini | `gpt5-mini`, `gpt-5-mini` | 128K | Smaller variant |
| GPT-5 Nano | `gpt5-nano`, `gpt-5-nano` | 128K | Smallest variant |
| O4 Mini | `o4-mini` | 200K | Latest reasoning model |
| O3 | `o3` | 200K | Reasoning model |
| O3 Mini | `o3-mini` | 200K | Smaller O3 variant |
| O1 | `o1` | 128K | Original reasoning model |
| O1 Mini | `o1-mini` | 128K | Smaller O1 variant |
| O1 Preview | `o1-preview` | 128K | O1 preview version |
| GPT-4o | `gpt4o`, `gpt-4o` | 128K | Omni model |
| GPT-4o Mini | `gpt4o-mini`, `gpt-4o-mini` | 128K | Smaller GPT-4o variant |

### cl100k_base encoding (legacy)

| Model | CLI Name | Context Window | Notes |
|-------|----------|---------------|-------|
| GPT-4 / GPT-4 Turbo | `gpt4`, `gpt-4` | 128K | Legacy |
| GPT-3.5 Turbo | `gpt35-turbo`, `gpt-3.5-turbo` | 16K | Legacy |

## Other Providers - Calibrated Estimation

| Provider | Model | CLI Name | Context Window | Chars/Token |
|----------|-------|----------|---------------|-------------|
| **Anthropic** | Claude (all versions) | `claude` | 200K | ~3.5 |
| **Google** | Gemini (all versions) | `gemini` | 1M | ~3.8 |
| **Meta** | Llama 2/3/4 | `llama` | 128K | ~3.5 |
| **Meta** | CodeLlama | `codellama` | 128K | ~3.2 |
| **Mistral AI** | Mistral / Codestral | `mistral` | 128K | ~3.5 |
| **DeepSeek** | DeepSeek V3 / R1 / Coder | `deepseek` | 128K | ~3.5 |
| **Alibaba** | Qwen 2.5 / 3 | `qwen` | 128K | ~3.5 |
| **Cohere** | Command R / R+ | `cohere` | 128K | ~3.6 |
| **xAI** | Grok 2/3/4 | `grok` | 2M | ~3.5 |

## Model Name Aliases

Infiniloom accepts many aliases for each model. Examples:

| You type | Resolves to |
|----------|-------------|
| `claude-sonnet`, `claude-4.6`, `claude-opus` | `claude` |
| `gemini-pro`, `gemini-2.5`, `gemini-3.1-flash` | `gemini` |
| `llama-4`, `llama-3.2`, `llama3` | `llama` |
| `codellama`, `code-llama` | `codellama` |
| `mistral-large`, `codestral`, `devstral` | `mistral` |
| `deepseek-r1`, `deepseek-coder`, `deepseek-v3` | `deepseek` |
| `command-r-plus`, `command-r+` | `cohere` |

## Default Token Budgets

Infiniloom calculates a recommended budget at ~75% of the context window (capped at 500K):

| Model | Context Window | Recommended Budget |
|-------|---------------|-------------------|
| GPT-3.5 Turbo | 16K | 12,000 |
| GPT-4 / GPT-4o | 128K | 96,000 |
| O3 / O4 Mini | 200K | 150,000 |
| Claude | 200K | 150,000 |
| Gemini | 1M | 500,000 (capped) |
| Grok | 2M | 500,000 (capped) |

## Usage

```bash
# Specify model for token counting
infiniloom pack . --model claude
infiniloom scan . --model gpt4o
infiniloom embed . --token-model gpt51-codex

# See all supported models
infiniloom info
```

## Accuracy Notes

- **OpenAI models**: Exact token counts via tiktoken. These match what OpenAI's API reports.
- **Claude**: Estimation is calibrated for English prose (~95% accurate) and code (~85% accurate). For critical budget management, use 90-95% of the context window as your budget.
- **Other providers**: Similar estimation accuracy. The heuristic divides character count by a provider-specific chars-per-token ratio.

## See Also

- [LLM Optimization Guide](guides/llm-optimization.md) - Model-specific tips
- [Configuration Guide](CONFIGURATION.md) - Setting default model
- [info command](commands/info.md) - View all supported models
