# Infiniloom CLI Specifications

This document defines the expected behavior for every CLI command. It is the
source of truth for the black-box tests in `cli/tests/specification_tests.rs`.

## Scope

- Commands: `pack`, `scan`, `map`, `info`, `init`, `index`, `diff`, `impact`, `chunk`
- Outputs must remain valid for the selected format (XML/JSON/YAML/Markdown/TOON/Plain)
- Exit status: non-zero on error; zero on success
- Output: human-readable on stdout unless `--output` is provided (then write file)

## Common Behavior

### Config resolution
- Config files are loaded from the repository root in this order:
  - `.infiniloomrc` (format auto-detected)
  - `.infiniloom.yaml`, `.infiniloom.yml`, `.infiniloom.toml`, `.infiniloom.json`
- `--config` overrides the lookup path.
- `.infiniloomignore` is always applied (independent of config file selection).
- Precedence: CLI flags > config > defaults.
- `--max-tokens` is only taken from config when CLI value is zero (default).

### Path handling
- If a path is omitted, commands default to the current directory.
- `pack` accepts local paths and remote URLs (GitHub/GitLab/Bitbucket/SSH).

### Output validity
- XML must be well-formed.
- JSON must parse as a single JSON value.
- YAML must parse as a single YAML document.
- Markdown, TOON, and Plain are free-form but must be readable.

---

## Command Specs

### `pack`

**Purpose:** Generate LLM-optimized repository context.

**Syntax:** `infiniloom pack [path] [options]`

**Core options:**
- `--format <xml|markdown|json|yaml|toon|plain>` (default: xml)
- `--model <claude|gpt52|gpt51|gpt5|o4-mini|o3|o1|gpt4o|gpt4o-mini|gpt4|gpt35-turbo|gemini|llama|codellama|mistral|deepseek|qwen|cohere|grok>` (default: claude)
- `--compression <none|minimal|balanced|aggressive|extreme|semantic>` (default: balanced)
- `--max-tokens <u32>` (alias `--budget`, default: 0 = unlimited)
- `--output <path>` (default: stdout)

**Filtering options:**
- `--hidden` to include hidden files (default: excluded)
- `--no-gitignore` to ignore `.gitignore` rules (default: respect)
- `--include <glob>` / `--exclude <glob>` (repeatable; applied after scan)
- `--include-tests` / `--include-docs` (default: excluded)
- `--no-default-ignores` disables built-in ignores (node_modules, dist, etc.)
- `--stdin` reads a newline-delimited file list and filters to those paths

**Content options:**
- `--no-content` excludes file contents (metadata only)
- `--remove-empty-lines`, `--remove-comments`
- `--line-numbers` / `--no-line-numbers` (default: enabled)
- `--top-files <N>` limits output to the most important files
- `--truncate-base64` truncates long base64 blobs

**Git context options:**
- `--include-logs`, `--logs-count <N>`
- `--include-diffs` (uncommitted changes)
- `--sort-by-changes` (sort by git change frequency)
  - XML and JSON include git history in structured metadata; Markdown/Plain/TOON/YAML
    append a Git Context section when requested.

**Extras and safety:**
- `--header-text <text>` adds a header section
- `--instruction-file <path>` adds an instructions section
- `--token-tree` adds per-file token counts
- `--security-check` scans for secrets and reports findings
- `--redact-secrets` redacts detected secrets in output
- `--watch` regenerates on file changes (requires `--output`)
- `--cache` enables incremental scan caching
- `--map-budget <u32>` sets repo-map token budget (default: 2000)

**Behavior:**
- Scans the repo, applies default ignores (unless disabled), then apply
  stdin filter, include patterns, and exclude patterns.
- `--full` enables symbol extraction and PageRank ranking.
- `--symbols` enables symbol extraction without full PageRank ranking.
- `--no-symbols` disables symbol extraction even when `--full` is set.
- Token budget is enforced before formatting; output is truncated only if still
  above `--max-tokens`.
- Watch mode re-applies all filters, compression, security, and budgeting.

**Format-specific extras (header/instructions/token-tree/security):**
- XML: extras are injected inside `<repository>` using dedicated elements.
- JSON/YAML: extras are added as top-level fields (`header_text`,
  `instructions`, `token_tree`, `security_scan`).
- Markdown/Plain/TOON: `header_text` is prepended; other extras are appended
  as sections.

---

### `scan`

**Purpose:** Show repository statistics and token counts.

**Syntax:** `infiniloom scan [path] [options]`

**Options:**
- `--model <...>` (default: claude)
- `--hidden`
- `--verbose`
- `--json`
- `--security-check`

**Behavior:**
- Respects `.gitignore` by default.
- Reads file contents to compute accurate token counts.
- JSON output includes fixed token totals for `claude`, `gpt4o`, and `gemini`,
  plus language breakdown and optional security findings.

---

### `map`

**Purpose:** Generate a repository map with key symbols.

**Syntax:** `infiniloom map [path] [options]`

**Options:**
- `--budget <u32>` (default: 2000)
- `--output <path>`

**Behavior:**
- Runs a full symbol scan and PageRank ranking.
- Output is a human-readable map including:
  - Summary (repo stats, primary language, key modules)
  - Key symbols (ranked list with file and line)
  - Module graph (module dependencies)
  - File index (top entries by importance)

---

### `info`

**Purpose:** Show version and configuration information.

**Syntax:** `infiniloom info [path]`

**Behavior:**
- Prints CLI/engine versions, supported formats/models, and compression levels.
- If a path is provided, reports config presence and key settings.

---

### `init`

**Purpose:** Create a default config file.

**Syntax:** `infiniloom init [path] [--format yaml|toml|json] [--output <path>] [--force]`

**Behavior:**
- Writes `.infiniloom.<ext>` in the target directory unless `--output` is set.
- Fails if the file exists and `--force` is not provided.

---

### `index`

**Purpose:** Build or update the symbol index for diff/impact.

**Syntax:** `infiniloom index [path] [--force] [--status] [--verbose]`

**Behavior:**
- `--status` shows index metadata without rebuilding.
- Skips rebuild if the index is less than 5 minutes old unless `--force` is set.
- Stores index data under `.infiniloom/`.

---

### `diff`

**Purpose:** Provide semantic context for changes.

**Syntax:** `infiniloom diff [path] [reference] [options]`

**Options:**
- `--staged`
- `--depth <1|2|3>` (default: 2)
- `--budget <u32>` (default: 50000)
- `--format <xml|json|markdown|yaml|toon|plain>` (default: xml)
- `--include-diff`
- `--output <path>`

**Behavior:**
- Uses git diffs for unstaged/staged/reference comparisons.
- Uses prebuilt index when available; falls back to lazy indexing.
- Outputs a formatted diff context and prints an impact summary to stderr.

---

### `impact`

**Purpose:** Analyze impact of a file or symbol.

**Syntax:** `infiniloom impact [path] <target> [--symbol] [--call-graph] [--json]`

**Behavior:**
- Requires a prebuilt index (`infiniloom index`).
- Without `--symbol`, `target` is treated as a file path.
- With `--symbol`, `target` is treated as a symbol name.
- `--call-graph` includes callees in output (symbol mode only).

---

### `chunk`

**Purpose:** Split repository into chunks for multi-turn workflows.

**Syntax:** `infiniloom chunk [path] [options]`

**Options:**
- `--strategy <fixed|file|module|semantic|dependency>` (default: semantic)
- `--max-tokens <u32>` (default: 8000)
- `--model <...>` (default: claude)
- `--format <xml|json|yaml|markdown|toon|plain>` (default: xml)
- `--output <dir>`
- `--verbose`

**Behavior:**
- Dependency strategy requires symbol extraction.
- If `--output` is set, each chunk is written to a file with a numbered name.
- JSON/YAML outputs are wrapped with chunk metadata.

---

## Test Cases (Black-Box)

### Pack

- **TC-PACK-001:** Basic XML output is well-formed and includes content.
- **TC-PACK-002:** Markdown output includes headers and code fences.
- **TC-PACK-004:** `.gitignore` is respected (ignored file contents excluded).
- **TC-PACK-005:** Binary file contents are excluded.
- **TC-PACK-006:** Symbol names appear when symbols are enabled or inferred.
