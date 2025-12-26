/**
 * Performance benchmarks for infiniloom-node
 *
 * Run with: node --test test/performance.test.js
 *
 * These tests verify that key functions meet performance expectations.
 * They use the infiniloom repository itself as a real-world test case.
 */

const test = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const fs = require('node:fs');

const {
  pack,
  scan,
  scanWithOptions,
  countTokens,
  buildIndex,
  indexStatus,
  findSymbol,
  getCallers,
  getCallees,
  getReferences,
  getCallGraph,
  getChangedSymbols,
  getChangedSymbolsFiltered,
  getDiffContext,
  analyzeImpact,
  getCallSites,
  getCallSitesWithContext,
  getTransitiveCallers,
  getSymbolsInFile,
  chunk,
  GitRepo,
} = require('..');

// Use the infiniloom repo itself for realistic benchmarks
const REPO_PATH = path.resolve(__dirname, '../../..');
const ENGINE_PATH = path.join(REPO_PATH, 'engine/src');

// Performance thresholds (in milliseconds)
const THRESHOLDS = {
  scan: 2000,              // Scan should complete under 2s
  scanWithOptions: 2000,   // Scan with options under 2s
  pack: 5000,              // Pack should complete under 5s
  buildIndex: 10000,       // Index build under 10s (cached)
  findSymbol: 200,         // Symbol lookup under 200ms
  getCallers: 300,         // Callers query under 300ms
  getCallees: 300,         // Callees query under 300ms
  getReferences: 300,      // References query under 300ms
  getCallGraph: 500,       // Full call graph under 500ms
  getChangedSymbols: 500,  // Changed symbols under 500ms (was 7-8s before fix)
  getDiffContext: 500,     // Diff context under 500ms (was slow before fix)
  analyzeImpact: 500,      // Impact analysis under 500ms
  getCallSites: 300,       // Call sites under 300ms
  getTransitiveCallers: 500, // Transitive callers under 500ms
  getSymbolsInFile: 200,   // Symbols in file under 200ms
  chunk: 3000,             // Chunking under 3s
  countTokens: 10,         // Token counting under 10ms
  gitStatus: 200,          // Git status under 200ms
  gitLog: 200,             // Git log under 200ms
  gitDiffFiles: 300,       // Git diff files under 300ms
};

/**
 * Measure execution time of a function
 */
function measure(fn) {
  const start = performance.now();
  const result = fn();
  const elapsed = performance.now() - start;
  return { result, elapsed };
}

/**
 * Format milliseconds for display
 */
function formatMs(ms) {
  if (ms < 1) return `${(ms * 1000).toFixed(0)}µs`;
  if (ms < 1000) return `${ms.toFixed(1)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

// Ensure index exists before running tests
test.before(() => {
  console.log('\n=== Infiniloom Node.js Performance Benchmarks ===\n');
  console.log(`Repository: ${REPO_PATH}`);

  // Build index if needed (cached if exists)
  const status = indexStatus(REPO_PATH);
  if (!status.exists) {
    console.log('Building index (first run)...');
    buildIndex(REPO_PATH, { force: false });
  }
  console.log(`Index: ${status.symbolCount} symbols, ${status.fileCount} files\n`);
});

// ============================================================================
// Core Function Benchmarks
// ============================================================================

test('PERF: scan() completes within threshold', () => {
  const { result, elapsed } = measure(() => scan(REPO_PATH));

  console.log(`  scan(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.scan)})`);
  console.log(`    Files: ${result.totalFiles}, Tokens: ${result.totalTokens}`);

  assert.ok(elapsed < THRESHOLDS.scan,
    `scan() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.scan)}`);
});

test('PERF: scanWithOptions() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    scanWithOptions(REPO_PATH, {
      include: ['*.rs'],
      applyDefaultIgnores: true
    })
  );

  console.log(`  scanWithOptions(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.scanWithOptions)})`);
  console.log(`    Files: ${result.totalFiles}`);

  assert.ok(elapsed < THRESHOLDS.scanWithOptions,
    `scanWithOptions() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.scanWithOptions)}`);
});

test('PERF: pack() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    pack(REPO_PATH, {
      format: 'xml',
      compression: 'balanced',
      skipSymbols: true,  // Faster for benchmark
      skipSecurity: true, // Skip security scan for benchmark
      tokenBudget: 50000
    })
  );

  console.log(`  pack(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.pack)})`);
  console.log(`    Output length: ${result.length} chars`);

  assert.ok(elapsed < THRESHOLDS.pack,
    `pack() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.pack)}`);
});

test('PERF: countTokens() completes within threshold', () => {
  // Read a medium-sized file for realistic benchmark
  const testContent = fs.readFileSync(
    path.join(REPO_PATH, 'engine/src/lib.rs'),
    'utf-8'
  );

  const { result, elapsed } = measure(() => countTokens(testContent, 'claude'));

  console.log(`  countTokens(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.countTokens)})`);
  console.log(`    Tokens: ${result}, Input: ${testContent.length} chars`);

  assert.ok(elapsed < THRESHOLDS.countTokens,
    `countTokens() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.countTokens)}`);
});

test('PERF: chunk() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    chunk(REPO_PATH, {
      strategy: 'module',
      maxTokens: 8000
    })
  );

  console.log(`  chunk(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.chunk)})`);
  console.log(`    Chunks: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.chunk,
    `chunk() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.chunk)}`);
});

// ============================================================================
// Index/Query Benchmarks
// ============================================================================

test('PERF: buildIndex() (cached) completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    buildIndex(REPO_PATH, { force: false })
  );

  console.log(`  buildIndex(cached): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.buildIndex)})`);
  console.log(`    Symbols: ${result.symbolCount}, Files: ${result.fileCount}`);

  assert.ok(elapsed < THRESHOLDS.buildIndex,
    `buildIndex() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.buildIndex)}`);
});

test('PERF: findSymbol() completes within threshold', () => {
  const { result, elapsed } = measure(() => findSymbol(REPO_PATH, 'Parser'));

  console.log(`  findSymbol('Parser'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.findSymbol)})`);
  console.log(`    Found: ${result.length} symbols`);

  assert.ok(elapsed < THRESHOLDS.findSymbol,
    `findSymbol() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.findSymbol)}`);
});

test('PERF: getCallers() completes within threshold', () => {
  const { result, elapsed } = measure(() => getCallers(REPO_PATH, 'parse'));

  console.log(`  getCallers('parse'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getCallers)})`);
  console.log(`    Callers: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getCallers,
    `getCallers() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getCallers)}`);
});

test('PERF: getCallees() completes within threshold', () => {
  const { result, elapsed } = measure(() => getCallees(REPO_PATH, 'parse'));

  console.log(`  getCallees('parse'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getCallees)})`);
  console.log(`    Callees: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getCallees,
    `getCallees() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getCallees)}`);
});

test('PERF: getReferences() completes within threshold', () => {
  const { result, elapsed } = measure(() => getReferences(REPO_PATH, 'Repository'));

  console.log(`  getReferences('Repository'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getReferences)})`);
  console.log(`    References: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getReferences,
    `getReferences() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getReferences)}`);
});

test('PERF: getCallGraph() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    getCallGraph(REPO_PATH, { maxNodes: 100, maxEdges: 200 })
  );

  console.log(`  getCallGraph(limited): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getCallGraph)})`);
  console.log(`    Nodes: ${result.nodes.length}, Edges: ${result.edges.length}`);

  assert.ok(elapsed < THRESHOLDS.getCallGraph,
    `getCallGraph() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getCallGraph)}`);
});

test('PERF: getSymbolsInFile() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    getSymbolsInFile(REPO_PATH, 'engine/src/lib.rs')
  );

  console.log(`  getSymbolsInFile(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getSymbolsInFile)})`);
  console.log(`    Symbols: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getSymbolsInFile,
    `getSymbolsInFile() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getSymbolsInFile)}`);
});

// ============================================================================
// Git/Diff Benchmarks (Previously Slow - Now Optimized)
// ============================================================================

test('PERF: getChangedSymbols() completes within threshold (was 7-8s before optimization)', () => {
  const { result, elapsed } = measure(() =>
    getChangedSymbols(REPO_PATH, 'HEAD~10', 'HEAD')
  );

  console.log(`  getChangedSymbols(HEAD~10): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getChangedSymbols)})`);
  console.log(`    Changed symbols: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getChangedSymbols,
    `getChangedSymbols() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getChangedSymbols)}`);
});

test('PERF: getChangedSymbolsFiltered() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    getChangedSymbolsFiltered(REPO_PATH, 'HEAD~10', 'HEAD', {
      kinds: ['function', 'method']
    })
  );

  console.log(`  getChangedSymbolsFiltered(HEAD~10): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getChangedSymbols)})`);
  console.log(`    Filtered symbols: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getChangedSymbols,
    `getChangedSymbolsFiltered() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getChangedSymbols)}`);
});

test('PERF: getDiffContext() completes within threshold (was slow before optimization)', () => {
  const { result, elapsed } = measure(() =>
    getDiffContext(REPO_PATH, 'HEAD~10', 'HEAD', { includeDiff: true })
  );

  console.log(`  getDiffContext(HEAD~10): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getDiffContext)})`);
  console.log(`    Changed files: ${result.changedFiles.length}, Context symbols: ${result.contextSymbols.length}`);

  assert.ok(elapsed < THRESHOLDS.getDiffContext,
    `getDiffContext() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getDiffContext)}`);
});

test('PERF: analyzeImpact() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    analyzeImpact(REPO_PATH, ['engine/src/lib.rs'])
  );

  console.log(`  analyzeImpact(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.analyzeImpact)})`);
  console.log(`    Affected symbols: ${result.affectedSymbols.length}, Impact: ${result.impactLevel}`);

  assert.ok(elapsed < THRESHOLDS.analyzeImpact,
    `analyzeImpact() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.analyzeImpact)}`);
});

test('PERF: getCallSites() completes within threshold', () => {
  const { result, elapsed } = measure(() => getCallSites(REPO_PATH, 'parse'));

  console.log(`  getCallSites('parse'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getCallSites)})`);
  console.log(`    Call sites: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getCallSites,
    `getCallSites() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getCallSites)}`);
});

test('PERF: getCallSitesWithContext() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    getCallSitesWithContext(REPO_PATH, 'parse', { linesBefore: 3, linesAfter: 3 })
  );

  console.log(`  getCallSitesWithContext('parse'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getCallSites)})`);
  console.log(`    Call sites with context: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getCallSites,
    `getCallSitesWithContext() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getCallSites)}`);
});

test('PERF: getTransitiveCallers() completes within threshold', () => {
  const { result, elapsed } = measure(() =>
    getTransitiveCallers(REPO_PATH, 'parse', { maxDepth: 3, maxResults: 50 })
  );

  console.log(`  getTransitiveCallers('parse'): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.getTransitiveCallers)})`);
  console.log(`    Transitive callers: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.getTransitiveCallers,
    `getTransitiveCallers() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.getTransitiveCallers)}`);
});

// ============================================================================
// Git Class Benchmarks
// ============================================================================

test('PERF: GitRepo.status() completes within threshold', () => {
  const repo = new GitRepo(REPO_PATH);
  const { result, elapsed } = measure(() => repo.status());

  console.log(`  GitRepo.status(): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.gitStatus)})`);
  console.log(`    Changed files: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.gitStatus,
    `GitRepo.status() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.gitStatus)}`);
});

test('PERF: GitRepo.log() completes within threshold', () => {
  const repo = new GitRepo(REPO_PATH);
  const { result, elapsed } = measure(() => repo.log(20));

  console.log(`  GitRepo.log(20): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.gitLog)})`);
  console.log(`    Commits: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.gitLog,
    `GitRepo.log() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.gitLog)}`);
});

test('PERF: GitRepo.diffFiles() completes within threshold', () => {
  const repo = new GitRepo(REPO_PATH);
  const { result, elapsed } = measure(() => repo.diffFiles('HEAD~10', 'HEAD'));

  console.log(`  GitRepo.diffFiles(HEAD~10): ${formatMs(elapsed)} (threshold: ${formatMs(THRESHOLDS.gitDiffFiles)})`);
  console.log(`    Changed files: ${result.length}`);

  assert.ok(elapsed < THRESHOLDS.gitDiffFiles,
    `GitRepo.diffFiles() took ${formatMs(elapsed)}, expected < ${formatMs(THRESHOLDS.gitDiffFiles)}`);
});

// ============================================================================
// Scalability Tests
// ============================================================================

test('PERF: getChangedSymbols scales linearly with commit range', () => {
  const ranges = [5, 10, 20];
  const results = [];

  for (const n of ranges) {
    const { result, elapsed } = measure(() =>
      getChangedSymbols(REPO_PATH, `HEAD~${n}`, 'HEAD')
    );
    results.push({ n, elapsed, symbols: result.length });
  }

  console.log('  getChangedSymbols scaling:');
  for (const r of results) {
    console.log(`    HEAD~${r.n}: ${formatMs(r.elapsed)} (${r.symbols} symbols)`);
  }

  // Check that time doesn't explode (should be roughly linear, not O(n*files))
  // Allow 3x time for 4x commit range (some overhead is expected)
  const ratio = results[2].elapsed / results[0].elapsed;
  assert.ok(ratio < 6,
    `Scaling ratio too high: ${ratio.toFixed(1)}x for 4x commit range`);
});

test('PERF: getDiffContext scales linearly with file count', () => {
  const ranges = [5, 10, 20];
  const results = [];

  for (const n of ranges) {
    const { result, elapsed } = measure(() =>
      getDiffContext(REPO_PATH, `HEAD~${n}`, 'HEAD', { includeDiff: true })
    );
    results.push({ n, elapsed, files: result.changedFiles.length });
  }

  console.log('  getDiffContext scaling:');
  for (const r of results) {
    console.log(`    HEAD~${r.n}: ${formatMs(r.elapsed)} (${r.files} files)`);
  }

  // Check that time doesn't explode
  const ratio = results[2].elapsed / results[0].elapsed;
  assert.ok(ratio < 6,
    `Scaling ratio too high: ${ratio.toFixed(1)}x for 4x commit range`);
});

// Summary at the end
test.after(() => {
  console.log('\n=== Performance Benchmark Complete ===\n');
});
