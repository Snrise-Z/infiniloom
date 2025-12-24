# infiniloom-node

Node.js bindings for Infiniloom - Repository context engine for LLMs.

## Features

- **Pack repositories** into optimized LLM context with configurable compression
- **Scan repositories** for statistics and metadata
- **Count tokens** for different LLM models
- **Security scanning** to detect secrets and sensitive data
- **Fast native performance** powered by Rust and NAPI-RS
- **Cross-platform** support (macOS, Linux, Windows)

## Installation

```bash
npm install infiniloom-node
```

### Building from Source

```bash
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom/bindings/node
npm install
npm run build
```

## Quick Start

### Simple Packing

```javascript
const { pack } = require('infiniloom-node');

// Pack a repository with default settings
const context = pack('./my-repo');
console.log(context);
```

### With Options

```javascript
const { pack } = require('infiniloom-node');

const context = pack('./my-repo', {
  format: 'xml',           // Output format: 'xml', 'markdown', 'json', 'yaml', 'toon', or 'plain'
  model: 'claude',         // Target model: 'gpt-5.2', 'gpt-5.1', 'gpt-5', 'o3', 'gpt-4o', 'claude', 'gemini', 'llama', etc.
  compression: 'balanced', // Compression: 'none', 'minimal', 'balanced', 'aggressive', 'extreme', 'focused', 'semantic'
  mapBudget: 2000,         // Token budget for repository map
  maxSymbols: 50,          // Maximum symbols to include in map
  skipSecurity: false,     // Skip security scanning
  redactSecrets: true,     // Redact detected secrets in output (default true)
  skipSymbols: false,      // Skip symbol extraction for faster scans
  include: ['src/**/*.ts'],    // Glob patterns to include
  exclude: ['**/*.test.ts'],   // Glob patterns to exclude
  includeTests: false,     // Include test files (default: false)
  securityThreshold: 'critical', // Minimum severity to block: 'critical', 'high', 'medium', 'low'
  tokenBudget: 50000       // Limit total output tokens (0 = no limit)
});
```

### Repository Scanning

```javascript
const { scan, scanWithOptions } = require('infiniloom-node');

// Simple scan with model
const stats = scan('./my-repo', 'claude');
console.log(`Repository: ${stats.name}`);
console.log(`Total files: ${stats.totalFiles}`);
console.log(`Total lines: ${stats.totalLines}`);
console.log(`Total tokens: ${stats.totalTokens}`);
console.log(`Primary language: ${stats.primaryLanguage}`);
console.log(`Languages:`, stats.languages);

// Advanced scan with options
const detailedStats = scanWithOptions('./my-repo', {
  model: 'gpt-4o',
  include: ['src/**/*.ts'],       // Include only TypeScript source files
  exclude: ['**/*.test.ts'],      // Exclude test files
  includeTests: false,            // Exclude test directories
  applyDefaultIgnores: true       // Apply default ignores (node_modules, dist, etc.)
});
```

### Token Counting

```javascript
const { countTokens } = require('infiniloom-node');

const count = countTokens('Hello, world!', 'claude');
console.log(`Tokens: ${count}`);
```

### Semantic Compression

```javascript
const { semanticCompress } = require('infiniloom-node');

// Compress text while preserving important content
const longText = '... your long text content ...';
const compressed = semanticCompress(longText, 0.7, 0.5);
// Parameters:
//   - text: The text to compress
//   - similarityThreshold (optional): 0.0-1.0, default 0.7
//   - budgetRatio (optional): 0.0-1.0, default 0.5
console.log(compressed);
```

### Advanced Usage with Infiniloom Class

```javascript
const { Infiniloom } = require('infiniloom-node');

// Create an Infiniloom instance
const loom = new Infiniloom('./my-repo', 'claude');

// Get statistics
const stats = loom.getStats();
console.log(stats);

// Generate repository map
const map = loom.generateMap(2000, 50);
console.log(map);

// Pack with options (including new features)
const context = loom.pack({
  format: 'xml',
  compression: 'balanced',
  tokenBudget: 50000,       // Limit output size
  include: ['src/**/*.ts'], // Filter files
  exclude: ['**/*.test.ts']
});
console.log(context);

// Security scan - returns structured findings
const findings = loom.securityScan();
if (findings.length > 0) {
  console.warn('Security issues found:');
  for (const finding of findings) {
    console.warn(`${finding.severity}: ${finding.kind} in ${finding.file}:${finding.line}`);
  }
}

// Legacy formatted output (if needed)
const formattedFindings = loom.securityScanFormatted();
formattedFindings.forEach(f => console.log(f));
```

## API Reference

### Functions

#### `pack(path: string, options?: PackOptions): string`

Pack a repository into optimized LLM context.

**Parameters:**
- `path` - Path to repository root
- `options` - Optional packing options

**Returns:** Formatted repository context as a string

#### `scan(path: string, model?: string): ScanStats`

Scan a repository and return statistics. Applies default ignores automatically.

**Parameters:**
- `path` - Path to repository root
- `model` - Optional target model (default: "claude")

**Returns:** Repository statistics

#### `scanWithOptions(path: string, options?: ScanOptions): ScanStats`

Scan a repository with full configuration options.

**Parameters:**
- `path` - Path to repository root
- `options` - Optional scan options

**Returns:** Repository statistics

#### `countTokens(text: string, model?: string): number`

Count tokens in text for a specific model.

**Parameters:**
- `text` - Text to tokenize
- `model` - Optional model name (default: "claude")

**Returns:** Token count

#### `semanticCompress(text: string, similarityThreshold?: number, budgetRatio?: number): string`

Compress text using semantic compression while preserving important content.

**Parameters:**
- `text` - Text to compress
- `similarityThreshold` - Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
- `budgetRatio` - Target size as ratio of original (0.0-1.0, default: 0.5)

**Returns:** Compressed text

#### `scanSecurity(path: string): SecurityFinding[]`

Scan a repository for security issues (hardcoded secrets, API keys, etc.).

**Parameters:**
- `path` - Path to repository root

**Returns:** Array of security findings

```javascript
const { scanSecurity } = require('infiniloom-node');

const findings = scanSecurity('./my-repo');
for (const finding of findings) {
  console.log(`${finding.severity}: ${finding.kind} in ${finding.file}:${finding.line}`);
}
```

### Call Graph API

Query caller/callee relationships and navigate your codebase programmatically.

#### `buildIndex(path: string, options?: IndexOptions): IndexStatus`

Build or update the symbol index for a repository (required for call graph queries).

```javascript
const { buildIndex } = require('infiniloom-node');

const status = buildIndex('./my-repo');
console.log(`Indexed ${status.symbolCount} symbols`);
```

#### `findSymbol(path: string, name: string): SymbolInfo[]`

Find symbols by name.

```javascript
const { findSymbol, buildIndex } = require('infiniloom-node');

buildIndex('./my-repo');
const symbols = findSymbol('./my-repo', 'processRequest');
for (const s of symbols) {
  console.log(`${s.name} (${s.kind}) at ${s.file}:${s.line}`);
}
```

#### `getCallers(path: string, symbolName: string): SymbolInfo[]`

Get all functions/methods that call the target symbol.

```javascript
const { getCallers, buildIndex } = require('infiniloom-node');

buildIndex('./my-repo');
const callers = getCallers('./my-repo', 'authenticate');
console.log(`authenticate is called by ${callers.length} functions`);
for (const c of callers) {
  console.log(`  ${c.name} at ${c.file}:${c.line}`);
}
```

#### `getCallees(path: string, symbolName: string): SymbolInfo[]`

Get all functions/methods that the target symbol calls.

```javascript
const { getCallees, buildIndex } = require('infiniloom-node');

buildIndex('./my-repo');
const callees = getCallees('./my-repo', 'main');
console.log(`main calls ${callees.length} functions`);
```

#### `getReferences(path: string, symbolName: string): ReferenceInfo[]`

Get all references to a symbol (calls, imports, inheritance).

```javascript
const { getReferences, buildIndex } = require('infiniloom-node');

buildIndex('./my-repo');
const refs = getReferences('./my-repo', 'UserService');
for (const r of refs) {
  console.log(`${r.kind}: ${r.symbol.name} at ${r.symbol.file}:${r.symbol.line}`);
}
```

#### `getCallGraph(path: string, options?: CallGraphOptions): CallGraph`

Get the complete call graph with all symbols and call relationships.

```javascript
const { getCallGraph, buildIndex } = require('infiniloom-node');

buildIndex('./my-repo');
const graph = getCallGraph('./my-repo');
console.log(`${graph.stats.totalSymbols} symbols, ${graph.stats.totalCalls} calls`);

// Find most called functions
const callCounts = new Map();
for (const edge of graph.edges) {
  callCounts.set(edge.callee, (callCounts.get(edge.callee) || 0) + 1);
}
const sorted = [...callCounts.entries()].sort((a, b) => b[1] - a[1]);
console.log('Most called:', sorted.slice(0, 5));
```

#### Async versions

All call graph functions have async versions:
- `findSymbolAsync(path, name)`
- `getCallersAsync(path, symbolName)`
- `getCalleesAsync(path, symbolName)`
- `getReferencesAsync(path, symbolName)`
- `getCallGraphAsync(path, options)`

#### `isGitRepo(path: string): boolean`

Check if a path is a git repository.

**Parameters:**
- `path` - Path to check

**Returns:** `true` if path is a git repository, `false` otherwise

```javascript
const { isGitRepo } = require('infiniloom-node');

if (isGitRepo('./my-project')) {
  console.log('This is a git repository');
}
```

### Types

#### `PackOptions`

```typescript
interface PackOptions {
  format?: string;           // "xml", "markdown", "json", "yaml", "toon", or "plain"
  model?: string;            // "gpt-5.2", "gpt-5.1", "gpt-5", "o3", "gpt-4o", "claude", "gemini", "llama", etc.
  compression?: string;      // "none", "minimal", "balanced", "aggressive", "extreme", "focused", "semantic"
  mapBudget?: number;        // Token budget for repository map
  maxSymbols?: number;       // Maximum number of symbols in map
  skipSecurity?: boolean;    // Skip security scanning
  redactSecrets?: boolean;   // Redact detected secrets (default: true)
  skipSymbols?: boolean;     // Skip symbol extraction for faster scanning
  include?: string[];        // Glob patterns to include (e.g., ["src/**/*.ts"])
  exclude?: string[];        // Glob patterns to exclude (e.g., ["**/*.test.ts"])
  includeTests?: boolean;    // Include test files (default: false)
  securityThreshold?: string;// Minimum severity to block: "critical", "high", "medium", "low"
  tokenBudget?: number;      // Limit total output tokens (0 = no limit)
  // Git-based filtering options (PR review integration)
  changedOnly?: boolean;     // Only include files changed in git
  baseSha?: string;          // Base SHA/ref for diff comparison (e.g., "main", "HEAD~5")
  headSha?: string;          // Head SHA/ref for diff comparison (default: working tree)
  stagedOnly?: boolean;      // Include staged changes only
  includeRelated?: boolean;  // Include related files (importers/dependencies)
  relatedDepth?: number;     // Depth for related file traversal (1-3, default: 1)
}
```

#### `ScanOptions`

```typescript
interface ScanOptions {
  model?: string;              // Target model for token counting (default: "claude")
  include?: string[];          // Glob patterns to include
  exclude?: string[];          // Glob patterns to exclude
  includeTests?: boolean;      // Include test files (default: false)
  applyDefaultIgnores?: boolean; // Apply default ignores for dist/, node_modules/, etc. (default: true)
}
```

#### `ScanStats`

```typescript
interface ScanStats {
  name: string;
  totalFiles: number;
  totalLines: number;
  totalTokens: number;
  primaryLanguage?: string;
  languages: LanguageStat[];
  securityFindings: number;
}
```

#### `LanguageStat`

```typescript
interface LanguageStat {
  language: string;
  files: number;
  lines: number;
  percentage: number;
}
```

#### `SecurityFinding`

```typescript
interface SecurityFinding {
  file: string;       // File where the finding was detected
  line: number;       // Line number (1-indexed)
  severity: string;   // "Critical", "High", "Medium", "Low", "Info"
  kind: string;       // Type of finding (e.g., "aws_access_key", "github_token")
  pattern: string;    // The matched pattern
}
```

#### `GitCommit`

```typescript
interface GitCommit {
  hash: string;       // Full commit hash (40 characters)
  shortHash: string;  // Short commit hash (7 characters)
  author: string;     // Author name
  email: string;      // Author email
  date: string;       // Commit date (ISO 8601 format)
  message: string;    // Commit message (first line)
}
```

#### `GitFileStatus`

```typescript
interface GitFileStatus {
  path: string;       // File path
  oldPath?: string;   // Old path (for renames)
  status: string;     // "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
}
```

#### `GitChangedFile`

```typescript
interface GitChangedFile {
  path: string;       // File path
  oldPath?: string;   // Old path (for renames)
  status: string;     // "Added", "Modified", "Deleted", "Renamed", "Copied"
  additions: number;  // Number of lines added
  deletions: number;  // Number of lines deleted
}
```

#### `GitBlameLine`

```typescript
interface GitBlameLine {
  commit: string;     // Commit hash that introduced the line
  author: string;     // Author who wrote the line
  date: string;       // Date when line was written
  lineNumber: number; // Line number (1-indexed)
}
```

#### `GitDiffLine`

```typescript
interface GitDiffLine {
  changeType: string; // "add", "remove", or "context"
  oldLine?: number;   // Line number in old file (for remove/context)
  newLine?: number;   // Line number in new file (for add/context)
  content: string;    // Line content (without +/- prefix)
}
```

#### `GitDiffHunk`

```typescript
interface GitDiffHunk {
  oldStart: number;   // Starting line in old file
  oldCount: number;   // Number of lines in old file
  newStart: number;   // Starting line in new file
  newCount: number;   // Number of lines in new file
  header: string;     // Hunk header (e.g., "@@ -1,5 +1,6 @@ function name")
  lines: GitDiffLine[]; // Lines in this hunk
}
```

### Infiniloom Class

#### `new Infiniloom(path: string, model?: string)`

Create a new Infiniloom instance.

#### `getStats(): ScanStats`

Get repository statistics.

#### `generateMap(budget?: number, maxSymbols?: number): string`

Generate a repository map.

#### `pack(options?: PackOptions): string`

Pack repository with specific options.

#### `securityScan(): SecurityFinding[]`

Check for security issues and return structured findings.

#### `securityScanFormatted(): string[]`

Check for security issues and return formatted strings (legacy format).

### GitRepo Class

Git repository wrapper for accessing git operations like status, diff, log, and blame.

#### `new GitRepo(path: string)`

Open a git repository.

**Parameters:**
- `path` - Path to the repository

**Throws:** Error if path is not a git repository

#### `currentBranch(): string`

Get the current branch name.

**Returns:** Current branch name (e.g., "main", "feature/xyz")

#### `currentCommit(): string`

Get the current commit hash.

**Returns:** Full SHA-1 hash of HEAD commit (40 characters)

#### `status(): GitFileStatus[]`

Get working tree status (both staged and unstaged changes).

**Returns:** Array of file status objects

#### `log(count?: number): GitCommit[]`

Get recent commits.

**Parameters:**
- `count` - Maximum number of commits to return (default: 10)

**Returns:** Array of commit objects

#### `fileLog(path: string, count?: number): GitCommit[]`

Get commits that modified a specific file.

**Parameters:**
- `path` - File path relative to repo root
- `count` - Maximum number of commits (default: 10)

**Returns:** Array of commits that modified the file

#### `blame(path: string): GitBlameLine[]`

Get blame information for a file.

**Parameters:**
- `path` - File path relative to repo root

**Returns:** Array of blame line objects

#### `lsFiles(): string[]`

Get list of files tracked by git.

**Returns:** Array of file paths

#### `diffFiles(fromRef: string, toRef: string): GitChangedFile[]`

Get files changed between two commits.

**Parameters:**
- `fromRef` - Starting commit/branch/tag
- `toRef` - Ending commit/branch/tag

**Returns:** Array of changed files with diff stats

#### `uncommittedDiff(path: string): string`

Get diff content for uncommitted changes in a file.

**Parameters:**
- `path` - File path relative to repo root

**Returns:** Unified diff content

#### `allUncommittedDiffs(): string`

Get diff for all uncommitted changes.

**Returns:** Combined unified diff for all changed files

#### `hasChanges(path: string): boolean`

Check if a file has uncommitted changes.

**Parameters:**
- `path` - File path relative to repo root

**Returns:** `true` if file has changes

#### `lastModifiedCommit(path: string): GitCommit`

Get the last commit that modified a file.

**Parameters:**
- `path` - File path relative to repo root

**Returns:** Commit information object

#### `fileChangeFrequency(path: string, days?: number): number`

Get file change frequency in recent days.

**Parameters:**
- `path` - File path relative to repo root
- `days` - Number of days to look back (default: 30)

**Returns:** Number of commits that modified the file in the period

#### `fileAtRef(path: string, gitRef: string): string`

Get file content at a specific git ref (commit, branch, tag).

**Parameters:**
- `path` - File path relative to repo root
- `gitRef` - Git ref (commit hash, branch name, tag, HEAD~n, etc.)

**Returns:** File content as string

```javascript
const repo = new GitRepo('./my-project');
const oldVersion = repo.fileAtRef('src/main.js', 'HEAD~5');
const mainVersion = repo.fileAtRef('src/main.js', 'main');
```

#### `diffHunks(fromRef: string, toRef: string, path?: string): GitDiffHunk[]`

Parse diff between two refs into structured hunks with line-level changes.
Useful for PR review tools that need to post comments at specific lines.

**Parameters:**
- `fromRef` - Starting ref (e.g., "main", "HEAD~5", commit hash)
- `toRef` - Ending ref (e.g., "HEAD", "feature-branch")
- `path` - Optional file path to filter to a single file

**Returns:** Array of diff hunks with line-level change information

```javascript
const repo = new GitRepo('./my-project');
const hunks = repo.diffHunks('main', 'HEAD', 'src/index.js');
for (const hunk of hunks) {
  console.log(`Hunk at old:${hunk.oldStart} new:${hunk.newStart}`);
  for (const line of hunk.lines) {
    console.log(`${line.changeType}: ${line.content}`);
  }
}
```

#### `uncommittedHunks(path?: string): GitDiffHunk[]`

Parse uncommitted changes (working tree vs HEAD) into structured hunks.

**Parameters:**
- `path` - Optional file path to filter to a single file

**Returns:** Array of diff hunks for uncommitted changes

```javascript
const repo = new GitRepo('./my-project');
const hunks = repo.uncommittedHunks('src/index.js');
console.log(`${hunks.length} hunks with uncommitted changes`);
```

#### `stagedHunks(path?: string): GitDiffHunk[]`

Parse staged changes into structured hunks.

**Parameters:**
- `path` - Optional file path to filter to a single file

**Returns:** Array of diff hunks for staged changes only

```javascript
const repo = new GitRepo('./my-project');
const hunks = repo.stagedHunks('src/index.js');
console.log(`${hunks.length} hunks staged for commit`);
```

**Example:**

```javascript
const { GitRepo, isGitRepo } = require('infiniloom-node');

// Check if path is a git repo first
if (isGitRepo('./my-project')) {
  const repo = new GitRepo('./my-project');

  // Get current state
  console.log(`Branch: ${repo.currentBranch()}`);
  console.log(`Commit: ${repo.currentCommit()}`);

  // Get recent commits
  for (const commit of repo.log(5)) {
    console.log(`${commit.shortHash}: ${commit.message}`);
  }

  // Get file history
  for (const commit of repo.fileLog('src/index.js', 3)) {
    console.log(`${commit.date}: ${commit.message}`);
  }

  // Get blame information
  for (const line of repo.blame('src/index.js').slice(0, 10)) {
    console.log(`Line ${line.lineNumber}: ${line.author}`);
  }

  // Check for uncommitted changes
  if (repo.hasChanges('src/index.js')) {
    const diff = repo.uncommittedDiff('src/index.js');
    console.log(diff);
  }

  // Compare branches
  const changes = repo.diffFiles('main', 'feature');
  for (const file of changes) {
    console.log(`${file.status}: ${file.path} (+${file.additions}/-${file.deletions})`);
  }
}
```

## Supported Models

### OpenAI GPT-5.x Series (Latest)
- **GPT-5.2** / **GPT-5.2-Pro** - Latest GPT-5.2 models (o200k_base tokenizer)
- **GPT-5.1** / **GPT-5.1-Mini** / **GPT-5.1-Codex** - GPT-5.1 series
- **GPT-5** / **GPT-5-Mini** / **GPT-5-Nano** - GPT-5 base series

### OpenAI O-Series (Reasoning Models)
- **O4-mini** - Latest O4 reasoning model
- **O3** / **O3-mini** - O3 reasoning models
- **O1** / **O1-mini** / **O1-preview** - O1 reasoning models

### OpenAI GPT-4 Series
- **GPT-4o** / **GPT-4o-mini** - GPT-4o models (o200k_base tokenizer)
- **GPT-4** - GPT-4 (cl100k_base tokenizer)
- **GPT-3.5-turbo** - GPT-3.5

### Other Providers
- **Claude** - Anthropic's Claude models (default)
- **Gemini** - Google's Gemini
- **Llama** / **CodeLlama** - Meta's Llama models
- **Mistral** - Mistral AI
- **DeepSeek** - DeepSeek
- **Qwen** - Alibaba Qwen
- **Cohere** - Cohere Command R+
- **Grok** - xAI Grok

## Compression Levels

- **none** - No compression (0% reduction)
- **minimal** - Remove empty lines, trim whitespace (~15% reduction)
- **balanced** - Remove comments, normalize whitespace (~35% reduction)
- **aggressive** - Remove docstrings, keep signatures only (~60% reduction)
- **extreme** - Key symbols only (~80% reduction)
- **focused** - Key symbols with small context (~75% reduction)
- **semantic** - Heuristic semantic compression (~60-70% reduction)

## Output Formats

- **xml** - XML format optimized for Claude
- **markdown** - Markdown format for GPT models
- **json** - JSON format for programmatic access
- **yaml** - YAML format optimized for Gemini
- **toon** - Token-efficient format (~40% smaller than JSON)
- **plain** - Simple plain text format

## Security Scanning

Infiniloom automatically scans for sensitive data including:

- API keys
- Access tokens
- Private keys
- Passwords
- Database connection strings
- AWS credentials
- GitHub tokens

Critical security issues will prevent packing unless `skipSecurity: true` is set.

## Building from Source

```bash
# Install dependencies
npm install

# Build native addon
npm run build

# Build for release
npm run build --release
```

## Requirements

- Node.js >= 16
- Rust >= 1.91 (for building from source)

## License

MIT

## Links

- [GitHub Repository](https://github.com/Topos-Labs/infiniloom)
- [Documentation](https://toposlabs.ai/infiniloom/)
- [npm Package](https://www.npmjs.com/package/infiniloom-node)
