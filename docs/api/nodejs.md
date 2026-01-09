# Node.js API Reference

**Infiniloom Node.js bindings for RAG pipelines and vector database integration.**

```bash
npm install infiniloom-node
```

---

## Quick Start

```javascript
const {
  pack,
  scan,
  embed,
  countTokens,
  buildIndex,
  getCallers,
  GitRepo
} = require('infiniloom-node');

// Pack a repository into Claude-optimized XML
const context = pack('./my-repo', { format: 'xml', model: 'claude' });

// Generate embedding chunks for vector databases
const result = embed('./my-repo', { maxTokens: 1000 });
for (const chunk of result.chunks) {
  console.log(`${chunk.id}: ${chunk.source.symbol}`);
}

// Scan repository statistics
const stats = scan('./my-repo');
console.log(`Files: ${stats.totalFiles}, Tokens: ${stats.totalTokens}`);

// Count tokens in text
const tokens = countTokens('Hello, world!', 'claude');
```

---

## Core Functions

### `pack()`

Pack a repository into an LLM-optimized format.

```typescript
function pack(path?: string, options?: PackOptions): string
```

**Options:**
```typescript
interface PackOptions {
  format?: string;         // "xml", "markdown", "json", "yaml", "toon", "plain"
  model?: string;          // "claude", "gpt4o", "gpt5", "gemini", etc.
  compression?: string;    // "none", "minimal", "balanced", "aggressive", "extreme"
  mapBudget?: number;      // Token budget for repository map (default: 2000)
  maxSymbols?: number;     // Maximum symbols in map (default: 50)
  skipSecurity?: boolean;  // Skip security scanning (default: false)
  redactSecrets?: boolean; // Redact detected secrets (default: true)
  skipSymbols?: boolean;   // Skip symbol extraction (default: false)
  include?: string[];      // Glob patterns to include
  exclude?: string[];      // Glob patterns to exclude
  includeTests?: boolean;  // Include test files (default: false)
  tokenBudget?: number;    // Max output tokens (0 = unlimited)
  changedOnly?: boolean;   // Only include git-changed files
  baseSha?: string;        // Base ref for diff comparison
  headSha?: string;        // Head ref for diff comparison
  stagedOnly?: boolean;    // Include only staged changes
  includeRelated?: boolean; // Include related files
  relatedDepth?: number;   // Depth for related file traversal (1-3)
}
```

**Example:**
```javascript
// Claude-optimized XML
const context = pack('./my-repo', { format: 'xml', model: 'claude' });

// GPT-4o Markdown with aggressive compression
const gptContext = pack('./my-repo', {
  format: 'markdown',
  model: 'gpt4o',
  compression: 'aggressive'
});

// Only changed files since main branch
const diffContext = pack('./my-repo', {
  changedOnly: true,
  baseSha: 'main',
  includeRelated: true
});
```

---

### `scan()`

Scan a repository and return statistics.

```typescript
function scan(path?: string, model?: string): ScanStats
function scanWithOptions(path: string, options?: ScanOptions): ScanStats
```

**Returns:**
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

interface LanguageStat {
  language: string;
  files: number;
  lines: number;
  percentage: number;
}
```

**Example:**
```javascript
const stats = scan('./my-repo', 'claude');
console.log(`Repository: ${stats.name}`);
console.log(`Total files: ${stats.totalFiles}`);
console.log(`Total tokens: ${stats.totalTokens}`);

for (const lang of stats.languages) {
  console.log(`  ${lang.language}: ${lang.files} files (${lang.percentage.toFixed(1)}%)`);
}
```

---

### `embed()`

Generate embedding chunks for vector databases.

```typescript
function embed(path: string, options?: EmbedOptions): EmbedResult
```

**Options:**
```typescript
interface EmbedOptions {
  maxTokens?: number;       // Max tokens per chunk (default: 1000)
  minTokens?: number;       // Min tokens per chunk (default: 50)
  contextLines?: number;    // Context lines around symbols (default: 5)
  includeImports?: boolean; // Include import statements (default: true)
  includeTopLevel?: boolean; // Include top-level code (default: true)
  includeTests?: boolean;   // Include test files (default: false)
  securityScan?: boolean;   // Enable secret scanning (default: true)
  includePatterns?: string[]; // Glob patterns to include
  excludePatterns?: string[]; // Glob patterns to exclude
  manifestPath?: string;    // Custom manifest path
  diffOnly?: boolean;       // Only return changed chunks (default: false)
}
```

**Returns:**
```typescript
interface EmbedResult {
  version: number;
  settings: EmbedSettings;
  chunks: EmbedChunk[];
  summary: {
    totalChunks: number;
    totalTokens: number;
    added?: number;
    modified?: number;
    removed?: number;
    unchanged?: number;
  };
  diff?: EmbedDiff;
}

interface EmbedChunk {
  id: string;              // Content-addressable ID (ec_...)
  fullHash: string;        // Full BLAKE3 hash
  content: string;         // Chunk content
  tokens: number;          // Token count
  kind: string;            // "function", "class", "method", etc.
  source: {
    file: string;
    lines: [number, number];
    symbol: string;
    fqn?: string;          // Fully qualified name
    language: string;
    parent?: string;
    visibility: string;
    isTest: boolean;
  };
  context: {
    docstring?: string;
    signature?: string;
    calls?: string[];      // Functions this calls
    calledBy?: string[];   // Functions that call this
    imports?: string[];
    tags?: string[];       // Auto-generated semantic tags
  };
}
```

**Example:**
```javascript
// Generate chunks for RAG pipeline
const result = embed('./my-repo', { maxTokens: 1500 });

for (const chunk of result.chunks) {
  // Embed and upsert to vector DB
  const embedding = await getEmbedding(chunk.content);
  await vectorDb.upsert({
    id: chunk.id,
    vector: embedding,
    metadata: {
      file: chunk.source.file,
      symbol: chunk.source.symbol,
      kind: chunk.kind,
      tags: chunk.context.tags,
    }
  });
}

// Incremental updates
const changes = embed('./my-repo', { diffOnly: true });
console.log(`Added: ${changes.summary.added}, Modified: ${changes.summary.modified}`);
```

---

### `countTokens()`

Count tokens in text for a specific model.

```typescript
function countTokens(text?: string, model?: string): number
```

**Example:**
```javascript
const tokens = countTokens('Hello, world!', 'claude');
console.log(`Tokens: ${tokens}`);

// Exact counting for OpenAI models
const gpt4oTokens = countTokens(code, 'gpt4o');
```

---

### `scanSecurity()`

Scan repository for security issues.

```typescript
function scanSecurity(path?: string): SecurityFinding[]
```

**Returns:**
```typescript
interface SecurityFinding {
  file: string;
  line: number;
  severity: string;  // "Critical", "High", "Medium", "Low"
  kind: string;
  pattern: string;
}
```

**Example:**
```javascript
const findings = scanSecurity('./my-repo');
for (const finding of findings) {
  if (finding.severity === 'Critical') {
    console.log(`CRITICAL: ${finding.kind} in ${finding.file}:${finding.line}`);
  }
}
```

---

### `semanticCompress()`

Compress text while preserving meaning.

```typescript
function semanticCompress(text?: string, options?: SemanticCompressOptions): string

interface SemanticCompressOptions {
  similarityThreshold?: number;  // 0.0-1.0 (default: 0.7)
  budgetRatio?: number;          // 0.0-1.0 (default: 0.5)
  minChunkSize?: number;         // Minimum chunk size (default: 100)
  maxChunkSize?: number;         // Maximum chunk size (default: 2000)
}
```

**Example:**
```javascript
const longText = '...'; // Your long text
const compressed = semanticCompress(longText, { budgetRatio: 0.3 });
console.log(`Reduced from ${longText.length} to ${compressed.length} chars`);
```

---

## Index & Call Graph API

### `buildIndex()`

Build or update the symbol index for fast queries.

```typescript
function buildIndex(path?: string, options?: IndexOptions): IndexStatus

interface IndexOptions {
  force?: boolean;        // Force full rebuild
  includeTests?: boolean; // Include test files
  maxFileSize?: number;   // Max file size in bytes
  exclude?: string[];     // Patterns to exclude
  incremental?: boolean;  // Only re-index changed files
}

interface IndexStatus {
  exists: boolean;
  fileCount: number;
  symbolCount: number;
  lastBuilt?: string;     // ISO 8601
  version?: string;
  filesUpdated?: number;
  incremental?: boolean;
}
```

**Example:**
```javascript
// Build index
const status = buildIndex('./my-repo');
console.log(`Indexed ${status.symbolCount} symbols`);

// Incremental update
const updated = buildIndex('./my-repo', { incremental: true });
console.log(`Updated ${updated.filesUpdated} files`);
```

---

### `findSymbol()`

Find a symbol by name.

```typescript
function findSymbol(path?: string, name?: string): SymbolInfo[]

interface SymbolInfo {
  id: number;
  name: string;
  kind: string;           // "function", "class", "method", etc.
  file: string;
  line: number;           // Start line (1-indexed)
  endLine: number;
  signature?: string;
  visibility: string;     // "public", "private", etc.
}
```

**Example:**
```javascript
const symbols = findSymbol('./my-repo', 'authenticate');
for (const sym of symbols) {
  console.log(`${sym.kind}: ${sym.name} at ${sym.file}:${sym.line}`);
}
```

---

### `getCallers()`

Get all functions that call a symbol.

```typescript
function getCallers(path?: string, symbolName?: string): SymbolInfo[]
```

**Example:**
```javascript
const callers = getCallers('./my-repo', 'validateInput');
console.log(`validateInput is called by ${callers.length} functions`);
for (const c of callers) {
  console.log(`  ${c.name} at ${c.file}:${c.line}`);
}
```

---

### `getCallees()`

Get all functions that a symbol calls.

```typescript
function getCallees(path?: string, symbolName?: string): SymbolInfo[]
```

---

### `getReferences()`

Get all references to a symbol.

```typescript
function getReferences(path?: string, symbolName?: string): ReferenceInfo[]

interface ReferenceInfo {
  symbol: SymbolInfo;
  kind: string;           // "call", "import", "inherit", "implement"
  file: string;
  line: number;
}
```

---

### `getCallGraph()`

Get the complete call graph.

```typescript
function getCallGraph(path?: string, options?: CallGraphOptions): CallGraph

interface CallGraphOptions {
  maxNodes?: number;
  maxEdges?: number;
}

interface CallGraph {
  nodes: SymbolInfo[];
  edges: CallGraphEdge[];
  stats: CallGraphStats;
}

interface CallGraphEdge {
  callerId: number;
  calleeId: number;
  caller: string;
  callee: string;
  file: string;
  line: number;
}

interface CallGraphStats {
  totalSymbols: number;
  totalCalls: number;
  functions: number;
  classes: number;
}
```

**Example:**
```javascript
const graph = getCallGraph('./my-repo');
console.log(`Call graph: ${graph.stats.totalSymbols} symbols, ${graph.stats.totalCalls} calls`);

// Find most called functions
const callCounts = new Map();
for (const edge of graph.edges) {
  callCounts.set(edge.callee, (callCounts.get(edge.callee) || 0) + 1);
}
const sorted = [...callCounts.entries()].sort((a, b) => b[1] - a[1]);
console.log('Most called functions:', sorted.slice(0, 10));
```

---

### `getTransitiveCallers()`

Get all functions that eventually call a symbol (up to max depth).

```typescript
function getTransitiveCallers(
  path?: string,
  symbolName?: string,
  options?: TransitiveCallersOptions
): TransitiveCallerInfo[]

interface TransitiveCallersOptions {
  maxDepth?: number;     // Default: 3
  maxResults?: number;   // Default: 100
}

interface TransitiveCallerInfo {
  name: string;
  kind: string;
  file: string;
  line: number;
  depth: number;
  callPath: string[];    // e.g., ["main", "process", "validate", "target"]
}
```

**Example:**
```javascript
const callers = getTransitiveCallers('./my-repo', 'dangerousFunction', { maxDepth: 5 });
for (const c of callers) {
  console.log(`Depth ${c.depth}: ${c.callPath.join(' -> ')}`);
}
```

---

### `getCallSites()`

Get locations where a symbol is called.

```typescript
function getCallSites(path?: string, symbolName?: string): CallSite[]

interface CallSite {
  caller: string;
  callee: string;
  file: string;
  line: number;
  column?: number;
  callerId: number;
  calleeId: number;
}
```

---

### `getCallSitesWithContext()`

Get call sites with surrounding code context.

```typescript
function getCallSitesWithContext(
  path?: string,
  symbolName?: string,
  options?: CallSitesContextOptions
): CallSiteWithContext[]

interface CallSitesContextOptions {
  linesBefore?: number;  // Default: 3
  linesAfter?: number;   // Default: 3
}

interface CallSiteWithContext extends CallSite {
  context?: string;
  contextStartLine?: number;
  contextEndLine?: number;
}
```

**Example:**
```javascript
const sites = getCallSitesWithContext('./my-repo', 'authenticate', {
  linesBefore: 5,
  linesAfter: 5
});
for (const site of sites) {
  console.log(`Call in ${site.file}:${site.line}`);
  console.log(site.context);
}
```

---

### `findCircularDependencies()`

Detect circular import/dependency cycles in the codebase.

```typescript
function findCircularDependencies(path?: string): DependencyCycle[]

interface DependencyCycle {
  files: string[];      // File paths forming the cycle
  fileIds: number[];    // Internal file IDs
  length: number;       // Number of files in the cycle
}
```

**Example:**
```javascript
const { findCircularDependencies, buildIndex } = require('infiniloom-node');

// Build index first
buildIndex('./my-repo');

// Find circular dependencies
const cycles = findCircularDependencies('./my-repo');
if (cycles.length > 0) {
  console.log(`Found ${cycles.length} circular dependency cycles:`);
  for (const cycle of cycles) {
    console.log(`  Cycle of ${cycle.length} files: ${cycle.files.join(' -> ')}`);
  }
} else {
  console.log('No circular dependencies found');
}
```

**Async version:** `findCircularDependenciesAsync(path)`

---

### `getExportedSymbols()`

Get all public/exported symbols in the repository or a specific file.

```typescript
function getExportedSymbols(path?: string, filePath?: string): SymbolInfo[]
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `string` | Path to repository |
| `filePath` | `string` | Optional file path to filter symbols |

**Returns:** Array of `SymbolInfo` objects for public/exported symbols only.

**Example:**
```javascript
const { getExportedSymbols, buildIndex } = require('infiniloom-node');

// Build index first
buildIndex('./my-repo');

// Get all exported symbols
const exports = getExportedSymbols('./my-repo');
console.log(`Found ${exports.length} exported symbols`);

for (const sym of exports) {
  console.log(`  ${sym.visibility} ${sym.kind}: ${sym.name} at ${sym.file}:${sym.line}`);
}

// Get exports from a specific file
const fileExports = getExportedSymbols('./my-repo', 'src/lib.rs');
console.log(`Exports from src/lib.rs: ${fileExports.map(s => s.name).join(', ')}`);
```

**Async version:** `getExportedSymbolsAsync(path, filePath)`

---

## Diff Context API

### `getDiffContext()`

Get context-aware diff with surrounding symbols.

```typescript
function getDiffContext(
  path: string,
  fromRef: string,
  toRef: string,
  options?: DiffContextOptions
): DiffContextResult

interface DiffContextOptions {
  depth?: number;         // 1-3 (default: 2)
  budget?: number;        // Token budget (default: 50000)
  includeDiff?: boolean;  // Include diff content (default: false)
  format?: string;        // "xml", "markdown", "json"
  model?: string;
  exclude?: string[];
  include?: string[];
}

interface DiffContextResult {
  changedFiles: DiffFileContext[];
  contextSymbols: ContextSymbolInfo[];
  relatedTests: string[];
  formattedOutput?: string;
  totalTokens: number;
}
```

**Example:**
```javascript
// Context for last commit
const context = getDiffContext('./my-repo', 'HEAD~1', 'HEAD', {
  depth: 2,
  budget: 50000,
  includeDiff: true
});

console.log(`Changed: ${context.changedFiles.length} files`);
console.log(`Related symbols: ${context.contextSymbols.length}`);
console.log(`Related tests: ${context.relatedTests.length}`);
```

---

### `analyzeImpact()`

Analyze the impact of changes to files.

```typescript
function analyzeImpact(
  path: string,
  files: string[],
  options?: ImpactOptions
): ImpactResult

interface ImpactOptions {
  depth?: number;         // 1-3 (default: 2)
  includeTests?: boolean;
  model?: string;
  exclude?: string[];
  include?: string[];
}

interface ImpactResult {
  changedFiles: string[];
  dependentFiles: string[];
  testFiles: string[];
  affectedSymbols: AffectedSymbol[];
  impactLevel: string;    // "low", "medium", "high", "critical"
  summary: string;
}
```

**Example:**
```javascript
const impact = analyzeImpact('./my-repo', ['src/auth.ts']);
console.log(`Impact level: ${impact.impactLevel}`);
console.log(`Dependent files: ${impact.dependentFiles.length}`);
console.log(`Test files to run: ${impact.testFiles.join(', ')}`);
```

---

## Chunk API

### `chunk()`

Split a repository into chunks for incremental processing.

```typescript
function chunk(path?: string, options?: ChunkOptions): RepoChunk[]

interface ChunkOptions {
  strategy?: string;      // "fixed", "file", "module", "symbol", "semantic", "dependency"
  maxTokens?: number;     // Default: 8000
  overlap?: number;       // Default: 0
  model?: string;         // Default: "claude"
  format?: string;        // "xml", "markdown", "json"
  priorityFirst?: boolean;
  exclude?: string[];
}

interface RepoChunk {
  index: number;
  total: number;
  focus: string;
  tokens: number;
  files: string[];
  content: string;
}
```

**Example:**
```javascript
const chunks = chunk('./large-repo', {
  strategy: 'module',
  maxTokens: 50000,
  model: 'claude'
});

for (const c of chunks) {
  console.log(`Chunk ${c.index}/${c.total}: ${c.focus} (${c.tokens} tokens)`);
  // Process c.content with LLM
}
```

---

## Analysis API

### `extractDocumentation()`

Extract structured documentation from a docstring/comment.

```typescript
function extractDocumentation(rawDoc: string, language: string): Documentation

interface Documentation {
  summary?: string;
  description?: string;
  params: DocParam[];
  returns?: {
    typeInfo?: string;
    description?: string;
  };
  throws: DocThrows[];
  examples: DocExample[];
  isDeprecated: boolean;
  deprecationMessage?: string;
  since?: string;
  seeAlso: string[];
  tags: Record<string, string[]>;
  raw?: string;
}

interface DocParam {
  name: string;
  typeInfo?: string;
  description?: string;
  isOptional: boolean;
  defaultValue?: string;
}

interface DocThrows {
  exceptionType: string;
  description?: string;
}

interface DocExample {
  code: string;
  title?: string;
  language?: string;
}
```

**Example:**
```javascript
const { extractDocumentation } = require('infiniloom-node');

const doc = `
@param name - The user's name
@param age - The user's age
@returns The greeting message
@throws ValueError If name is empty
`;
const result = extractDocumentation(doc, 'javascript');
console.log(`Params: ${result.params.length}`);
for (const param of result.params) {
  console.log(`  ${param.name}: ${param.description}`);
}
```

---

### `calculateComplexity()`

Calculate complexity metrics for source code.

```typescript
function calculateComplexity(source: string, language: string): ComplexityMetrics

interface ComplexityMetrics {
  cyclomatic: number;           // Cyclomatic complexity
  cognitive: number;            // Cognitive complexity
  halstead: HalsteadMetrics;
  loc: LocMetrics;
  maintainabilityIndex?: number;
  maxNestingDepth: number;
  parameterCount: number;
  returnCount: number;
}

interface HalsteadMetrics {
  distinctOperators: number;
  distinctOperands: number;
  totalOperators: number;
  totalOperands: number;
  vocabulary: number;
  length: number;
  calculatedLength: number;
  volume: number;
  difficulty: number;
  effort: number;
  time: number;
  bugs: number;
}

interface LocMetrics {
  total: number;
  source: number;
  comments: number;
  blank: number;
}
```

**Example:**
```javascript
const { calculateComplexity } = require('infiniloom-node');

const code = `
function complexFunction(a, b, c) {
  if (a > 0) {
    if (b > 0) {
      return a + b;
    } else {
      return a - b;
    }
  }
  return c;
}
`;
const metrics = calculateComplexity(code, 'javascript');
console.log(`Cyclomatic: ${metrics.cyclomatic}`);
console.log(`Cognitive: ${metrics.cognitive}`);
console.log(`Maintainability: ${metrics.maintainabilityIndex?.toFixed(1)}`);
```

---

### `checkComplexity()`

Check code complexity against thresholds.

```typescript
function checkComplexity(
  source: string,
  language: string,
  options?: ComplexityThresholds
): ComplexityIssue[]

interface ComplexityThresholds {
  maxCyclomatic?: number;       // Default: 10
  maxCognitive?: number;        // Default: 15
  maxNesting?: number;          // Default: 4
  maxParams?: number;           // Default: 5
  minMaintainability?: number;  // Default: 40.0
}

interface ComplexityIssue {
  message: string;
  severity: string;  // "warning" or "error"
}
```

**Example:**
```javascript
const { checkComplexity } = require('infiniloom-node');

const issues = checkComplexity(code, 'javascript', { maxCyclomatic: 5 });
for (const issue of issues) {
  console.log(`[${issue.severity}] ${issue.message}`);
}
```

---

### `detectDeadCode()`

Detect dead code in a repository.

```typescript
function detectDeadCode(path: string, options?: DeadCodeOptions): DeadCodeInfo

interface DeadCodeOptions {
  languages?: string[];
}

interface DeadCodeInfo {
  unusedExports: UnusedExport[];
  unreachableCode: UnreachableCode[];
  unusedPrivate: UnusedPrivate[];
  unusedImports: UnusedImport[];
  unusedVariables: UnusedVariable[];
}

interface UnusedExport {
  name: string;
  kind: string;
  filePath: string;
  line: number;
  confidence: number;
  reason: string;
}

interface UnreachableCode {
  filePath: string;
  startLine: number;
  endLine: number;
  reason: string;
  snippet: string;
}

interface UnusedPrivate {
  name: string;
  kind: string;
  filePath: string;
  line: number;
}

interface UnusedImport {
  name: string;
  importPath: string;
  filePath: string;
  line: number;
}

interface UnusedVariable {
  name: string;
  filePath: string;
  line: number;
  scope: string;
}
```

**Example:**
```javascript
const { detectDeadCode } = require('infiniloom-node');

const result = detectDeadCode('./my-repo');
console.log(`Unused exports: ${result.unusedExports.length}`);
console.log(`Unreachable code: ${result.unreachableCode.length}`);
for (const exp of result.unusedExports) {
  console.log(`  ${exp.name} in ${exp.filePath}:${exp.line}`);
}
```

---

### `detectBreakingChanges()`

Detect breaking changes between two versions.

```typescript
function detectBreakingChanges(
  path: string,
  oldRef: string,
  newRef: string
): BreakingChangeReport

interface BreakingChangeReport {
  changes: BreakingChange[];
  summary: {
    totalChanges: number;
    bySeverity: Record<string, number>;
    byType: Record<string, number>;
  };
  oldRef: string;
  newRef: string;
}

interface BreakingChange {
  symbolName: string;
  changeType: string;        // "removed", "signature_changed", "visibility_changed"
  severity: string;          // "breaking", "warning", "info"
  filePath: string;
  oldLine?: number;
  newLine?: number;
  oldValue?: string;
  newValue?: string;
  description: string;
  suggestion?: string;
}
```

**Example:**
```javascript
const { detectBreakingChanges } = require('infiniloom-node');

const report = detectBreakingChanges('./my-repo', 'v1.0.0', 'v2.0.0');
console.log(`Breaking changes: ${report.summary.bySeverity.breaking || 0}`);
for (const change of report.changes) {
  if (change.severity === 'breaking') {
    console.log(`  BREAKING: ${change.symbolName} - ${change.description}`);
  }
}
```

---

### `getTypeHierarchy()`

Get the type hierarchy for a class/interface.

```typescript
function getTypeHierarchy(path: string, symbolName: string): TypeHierarchy

interface TypeHierarchy {
  name: string;
  kind: string;
  file: string;
  line: number;
  ancestors: AncestorInfo[];
  descendants: AncestorInfo[];
  interfaces: string[];
}

interface AncestorInfo {
  name: string;
  file: string;
  line: number;
  kind: string;
  depth: number;
  isDirect: boolean;
}
```

**Example:**
```javascript
const { getTypeHierarchy } = require('infiniloom-node');

const hierarchy = getTypeHierarchy('./my-repo', 'BaseController');
console.log(`Ancestors: ${hierarchy.ancestors.map(a => a.name).join(', ')}`);
console.log(`Descendants: ${hierarchy.descendants.map(d => d.name).join(', ')}`);
```

---

### `getTypeAncestors()`

Get all ancestors (parent classes/interfaces) of a type.

```typescript
function getTypeAncestors(path: string, symbolName: string): AncestorInfo[]
```

---

### `getTypeDescendants()`

Get all descendants (child classes) of a type.

```typescript
function getTypeDescendants(path: string, symbolName: string): AncestorInfo[]
```

---

### `getImplementors()`

Get all classes that implement an interface.

```typescript
function getImplementors(path: string, interfaceName: string): SymbolInfo[]
```

**Example:**
```javascript
const { getImplementors } = require('infiniloom-node');

const implementors = getImplementors('./my-repo', 'Serializable');
console.log(`Classes implementing Serializable: ${implementors.length}`);
for (const impl of implementors) {
  console.log(`  ${impl.name} at ${impl.file}:${impl.line}`);
}
```

---

## Classes

### `Infiniloom`

Object-oriented interface for repository operations.

```typescript
class Infiniloom {
  constructor(path: string, model?: string)
  getStats(): ScanStats
  generateMap(options?: GenerateMapOptions): string
  pack(options?: PackOptions): string
  securityScan(): SecurityFinding[]
}
```

**Example:**
```javascript
const { Infiniloom } = require('infiniloom-node');

const loom = new Infiniloom('./my-repo');
const stats = loom.getStats();
console.log(`Repository: ${stats.name}, Files: ${stats.totalFiles}`);

const context = loom.pack({ format: 'xml', model: 'claude' });
const findings = loom.securityScan();
```

---

### `GitRepo`

Git repository operations.

```typescript
class GitRepo {
  constructor(path?: string)
  currentBranch(): string
  currentCommit(): string
  status(): GitFileStatus[]
  diffFiles(fromRef: string, toRef: string): GitChangedFile[]
  log(count?: number): GitCommit[]
  fileLog(path: string, count?: number): GitCommit[]
  blame(path: string): GitBlameLine[]
  lsFiles(): string[]
  diffContent(fromRef: string, toRef: string, path: string): string
  uncommittedDiff(path: string): string
  allUncommittedDiffs(): string
  hasChanges(path: string): boolean
  lastModifiedCommit(path: string): GitCommit
  fileChangeFrequency(path: string, days?: number): number
  fileAtRef(path: string, gitRef: string): string
  diffHunks(fromRef: string, toRef: string, path?: string): GitDiffHunk[]
  uncommittedHunks(path?: string): GitDiffHunk[]
  stagedHunks(path?: string): GitDiffHunk[]
}
```

**Example:**
```javascript
const { GitRepo, isGitRepo } = require('infiniloom-node');

if (isGitRepo('./my-project')) {
  const repo = new GitRepo('./my-project');
  console.log(`Branch: ${repo.currentBranch()}`);
  console.log(`Commit: ${repo.currentCommit()}`);

  for (const file of repo.status()) {
    console.log(`${file.status}: ${file.path}`);
  }

  for (const commit of repo.log(5)) {
    console.log(`${commit.shortHash}: ${commit.message}`);
  }
}
```

---

## Async Functions

All core functions have async versions:

```javascript
const {
  packAsync,
  scanAsync,
  embedAsync,
  buildIndexAsync,
  chunkAsync,
  analyzeImpactAsync,
  getDiffContextAsync,
  findSymbolAsync,
  getCallersAsync,
  getCalleesAsync,
  getReferencesAsync,
  getCallGraphAsync,
} = require('infiniloom-node');

async function main() {
  const context = await packAsync('./my-repo', { format: 'xml' });
  const stats = await scanAsync('./my-repo');
  const result = await embedAsync('./my-repo');
}
```

---

## Filtered Query Functions

Filter symbols by kind in queries:

```typescript
interface QueryFilter {
  kinds?: string[];       // Include only these kinds
  excludeKinds?: string[]; // Exclude these kinds
}

function findSymbolFiltered(path: string, name: string, filter?: QueryFilter): SymbolInfo[]
function getCallersFiltered(path: string, symbolName: string, filter?: QueryFilter): SymbolInfo[]
function getCalleesFiltered(path: string, symbolName: string, filter?: QueryFilter): SymbolInfo[]
function getReferencesFiltered(path: string, symbolName: string, filter?: QueryFilter): ReferenceInfo[]
```

**Example:**
```javascript
const { findSymbolFiltered, getCallersFiltered } = require('infiniloom-node');

// Find only functions named "process"
const funcs = findSymbolFiltered('./my-repo', 'process', {
  kinds: ['function', 'method']
});

// Get callers excluding imports
const callers = getCallersFiltered('./my-repo', 'authenticate', {
  excludeKinds: ['import']
});
```

---

## Supported Models

| Model | Accuracy | Description |
|-------|----------|-------------|
| `claude` | ~95% | Anthropic Claude (default) |
| `gpt52`, `gpt51`, `gpt5` | Exact | OpenAI GPT-5 series |
| `o4-mini`, `o3`, `o1` | Exact | OpenAI reasoning models |
| `gpt4o`, `gpt4o-mini` | Exact | OpenAI GPT-4o |
| `gpt4`, `gpt35-turbo` | Exact | OpenAI legacy |
| `gemini` | ~95% | Google Gemini |
| `llama`, `codellama` | ~95% | Meta Llama |
| `mistral` | ~95% | Mistral AI |
| `deepseek` | ~95% | DeepSeek |
| `qwen` | ~95% | Alibaba Qwen |
| `cohere` | ~95% | Cohere |
| `grok` | ~95% | xAI Grok |

---

## Integration Examples

### Pinecone

```javascript
const { embed } = require('infiniloom-node');
const { Pinecone } = require('@pinecone-database/pinecone');
const { OpenAI } = require('openai');

const pc = new Pinecone({ apiKey: '...' });
const index = pc.index('code-embeddings');
const openai = new OpenAI();

// Generate chunks
const result = embed('./my-repo', { maxTokens: 1500 });

// Embed and upsert
for (const chunk of result.chunks) {
  const response = await openai.embeddings.create({
    model: 'text-embedding-3-small',
    input: chunk.content
  });

  await index.upsert([{
    id: chunk.id,
    values: response.data[0].embedding,
    metadata: {
      file: chunk.source.file,
      symbol: chunk.source.symbol,
      language: chunk.source.language,
      kind: chunk.kind,
      tags: chunk.context.tags,
    }
  }]);
}
```

### Express Middleware

```javascript
const express = require('express');
const { pack, scan, embed } = require('infiniloom-node');

const app = express();

app.get('/api/context/:repo', (req, res) => {
  try {
    const context = pack(req.params.repo, {
      format: req.query.format || 'xml',
      model: req.query.model || 'claude'
    });
    res.type('text/plain').send(context);
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

app.get('/api/stats/:repo', (req, res) => {
  const stats = scan(req.params.repo);
  res.json(stats);
});

app.get('/api/chunks/:repo', (req, res) => {
  const result = embed(req.params.repo, {
    maxTokens: parseInt(req.query.maxTokens) || 1000
  });
  res.json(result);
});
```

---

## TypeScript Support

The package includes full TypeScript definitions:

```typescript
import {
  pack,
  scan,
  embed,
  PackOptions,
  ScanStats,
  EmbedResult,
  EmbedChunk,
  SymbolInfo,
  CallGraph,
  GitRepo,
} from 'infiniloom-node';

const options: PackOptions = {
  format: 'xml',
  model: 'claude',
  compression: 'balanced',
};

const context: string = pack('./my-repo', options);
const stats: ScanStats = scan('./my-repo');
const result: EmbedResult = embed('./my-repo');
```

---

## See Also

- [Python API Reference](python.md)
- [CLI Reference](../commands/pack.md)
- [Recipes Cookbook](../RECIPES.md)
- [Reference](../REFERENCE.md)
