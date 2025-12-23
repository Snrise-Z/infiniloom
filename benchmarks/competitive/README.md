# Competitive Benchmarks

Compare Infiniloom against Repomix and Gitingest across performance, features, and LLM effectiveness.

## Quick Start

```bash
# Install Python dependencies
pip install -r requirements.txt

# Check tool installation status
python install_tools.py status

# Install all tools
python install_tools.py install

# Clone test repositories
python repos.py clone

# Run quick benchmark (1 run, no warmup)
python runner.py --quick --repos fastapi lodash

# Run full benchmark (3 runs + warmup)
python runner.py --repos fastapi deno lodash

# Generate report
python report.py
```

## Components

### `install_tools.py` - Tool Management

Manages installation and verification of benchmark tools.

```bash
# Check what's installed
python install_tools.py status

# Install specific tool
python install_tools.py install --tool repomix

# Install all tools
python install_tools.py install
```

### `repos.py` - Repository Management

Clones and manages test repositories.

```bash
# List available repos
python repos.py list

# Clone all repos (shallow clone)
python repos.py clone

# Clone specific repo
python repos.py clone --repo fastapi

# Force reclone
python repos.py clone --repo fastapi --force

# Clean all cloned repos
python repos.py clean
```

**Available Repositories:**
| Repo | Size | Language | Description |
|------|------|----------|-------------|
| fastapi | ~500 files | Python | Popular web framework |
| deno | ~2000 files | Rust/TS | Large mixed codebase |
| lodash | ~600 files | JavaScript | Utility library |
| rust-analyzer | ~1500 files | Rust | Complex Rust project |
| TypeScript | ~3000 files | TypeScript | Large TS project |
| infiniloom | ~100 files | Rust | This project (baseline) |

### `runner.py` - Benchmark Runner

Runs performance benchmarks across all tools.

```bash
# Quick mode (fast, less accurate)
python runner.py --quick --repos fastapi

# Standard mode (3 runs + warmup)
python runner.py --repos fastapi lodash

# Full benchmark (all repos)
python runner.py --repos fastapi deno lodash rust-analyzer TypeScript

# Specific tools only
python runner.py --tools infiniloom repomix --repos fastapi

# Custom output directory
python runner.py --output-dir ./my_results --repos fastapi
```

**Metrics Measured:**
- Execution time (seconds)
- Peak memory usage (MB)
- Output size (bytes)
- Token count (GPT-4 encoding)
- Success rate

### `llm_eval.py` - LLM Effectiveness Evaluation

Tests how well each tool's output works with Claude and GPT-4.

```bash
# Run evaluation (requires API keys)
export ANTHROPIC_API_KEY="your-key"
export OPENAI_API_KEY="your-key"

# Basic evaluation
python llm_eval.py --repo fastapi

# Specific tools
python llm_eval.py --repo fastapi --tools infiniloom repomix

# Specific tests
python llm_eval.py --repo fastapi --tests code_understanding symbol_location

# Specific LLMs
python llm_eval.py --repo fastapi --llms claude
```

**Test Types:**
- `code_understanding`: Explain project architecture
- `symbol_location`: Find specific functions/classes
- `bug_finding`: Identify code issues
- `code_generation`: Generate new code matching patterns

### `report.py` - Report Generation

Generates comprehensive markdown reports.

```bash
# Generate report from results
python report.py

# Custom paths
python report.py \
  --results-dir ./benchmark_results \
  --llm-results ./llm_eval_results.json \
  --output ./report.md
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key for Claude (LLM evaluation) |
| `OPENAI_API_KEY` | API key for GPT-4 (LLM evaluation) |

## Output Files

```
benchmark_results/
├── results_YYYYMMDD_HHMMSS.json    # Raw benchmark data
├── summary_YYYYMMDD_HHMMSS.json    # Summary statistics
└── ...

llm_eval_results.json                # LLM evaluation data
benchmark_report.md                  # Comprehensive report
```

## Example Results Format

```json
{
  "tool": "infiniloom",
  "repo": "fastapi",
  "execution_time_seconds": 1.23,
  "peak_memory_mb": 45.6,
  "output_size_bytes": 234567,
  "token_count": 58642,
  "tokens_per_second": 47675.6,
  "exit_code": 0
}
```

## Adding New Tools

Edit `install_tools.py`:

```python
TOOLS["new_tool"] = Tool(
    name="new_tool",
    description="Description here",
    install_command=["pip", "install", "new-tool"],
    check_command=["new-tool", "--version"],
    version_command=["new-tool", "--version"],
    pack_command_template="new-tool pack {repo_path}",
)
```

## Adding New Test Repos

Edit `repos.py`:

```python
TEST_REPOS.append(TestRepo(
    name="new-repo",
    url="https://github.com/owner/new-repo",
    description="Description here",
    estimated_files=500,
    primary_language="Python",
))
```
