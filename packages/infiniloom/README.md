# infiniloom

High-performance repository context generator for LLMs.

Transform your codebase into optimized context for Claude, GPT-4o/GPT-5, Gemini, and other Large Language Models.

## Installation

```bash
npm install -g infiniloom
```

## Quick Start

```bash
# Pack repository into Claude-optimized XML
infiniloom pack /path/to/repo --format xml

# Scan repository and show statistics
infiniloom scan /path/to/repo

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000
```

## Commands

| Command | Description |
|---------|-------------|
| `pack` | Transform repository into LLM-optimized context |
| `scan` | Analyze repository statistics |
| `map` | Generate PageRank-based symbol map |
| `index` | Build symbol index for fast diff context |
| `diff` | Get context for code changes |
| `impact` | Analyze change impact |
| `chunk` | Split repository into manageable pieces |
| `init` | Create configuration file |
| `info` | Show version and configuration info |

## Supported Models

- **OpenAI**: GPT-5.x, GPT-4o, O3, O1
- **Anthropic**: Claude
- **Google**: Gemini
- **Meta**: Llama, CodeLlama
- **Others**: Mistral, DeepSeek, Qwen, Cohere, Grok

## Output Formats

- **XML** - Optimized for Claude (prompt caching)
- **Markdown** - Optimized for GPT models
- **YAML** - Compatible with Gemini and other models
- **JSON** - For programmatic access
- **TOON** - Most token-efficient (~40% smaller)

## Alternative Installation

If npm installation fails, you can install via:

```bash
# Homebrew Cask (macOS - fast, pre-built binary)
brew tap Topos-Labs/infiniloom
brew install --cask infiniloom

# Homebrew Formula (builds from source)
brew tap Topos-Labs/infiniloom
brew install infiniloom

# Cargo (Rust)
cargo install infiniloom
```

## Documentation

- [Documentation](https://toposlabs.ai/infiniloom/)
- [GitHub Repository](https://github.com/Topos-Labs/infiniloom)

## License

MIT
