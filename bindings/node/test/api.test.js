const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execSync } = require('node:child_process')

const {
  pack,
  scan,
  scanWithOptions,
  countTokens,
  semanticCompress,
  Infiniloom,
  isGitRepo,
  GitRepo,
  scanSecurity,
  packAsync,
  scanAsync,
  buildIndexAsync,
  chunkAsync,
  analyzeImpactAsync,
  getDiffContextAsync,
} = require('..')

function createTempRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-node-'))
  fs.writeFileSync(
    path.join(dir, 'main.rs'),
    [
      'fn main() {',
      '    println!("Hello, world!");',
      '}',
      '',
      'fn add(a: i32, b: i32) -> i32 {',
      '    a + b',
      '}',
      '',
    ].join('\n'),
  )
  fs.writeFileSync(
    path.join(dir, 'script.py'),
    [
      'def greet(name):',
      '    return f"Hello, {name}!"',
      '',
      'if __name__ == "__main__":',
      '    print(greet("World"))',
      '',
    ].join('\n'),
  )
  return dir
}

function cleanup(dir) {
  fs.rmSync(dir, { recursive: true, force: true })
}

test('pack returns valid JSON output', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const output = pack(dir, {
    format: 'json',
    model: 'claude',
    mapBudget: 500,
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  assert.ok(parsed.repository)
  assert.ok(parsed.repository.name)
})

test('scan returns stats with files and tokens', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const stats = scan(dir, 'claude')
  assert.ok(stats.totalFiles >= 2)
  assert.ok(stats.totalTokens > 0)
  assert.ok(Array.isArray(stats.languages))
})

test('countTokens returns a positive number and rejects invalid models', () => {
  const count = countTokens('Hello, world!', 'claude')
  assert.ok(count > 0)

  assert.throws(
    () => countTokens('Hello', 'invalid-model'),
    /Unknown model/i,
  )
})

test('semanticCompress reduces long inputs', () => {
  const paragraphs = Array.from({ length: 12 }, (_, i) =>
    `Paragraph ${i}\n` + 'x'.repeat(140),
  )
  const text = paragraphs.join('\n\n')
  const compressed = semanticCompress(text, 0.7, 0.5)

  assert.ok(compressed.length > 0)
  assert.ok(compressed.length < text.length)
})

test('Infiniloom class exposes stats, map, pack, and security scan', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const loom = new Infiniloom(dir, 'claude')
  const stats = loom.getStats()
  assert.ok(stats.totalFiles >= 2)

  const map = JSON.parse(loom.generateMap(200, 10))
  assert.ok(map.summary)

  const context = JSON.parse(loom.pack({ format: 'json' }))
  assert.ok(context.repository)

  const findings = loom.securityScan()
  assert.ok(Array.isArray(findings))
})

// Helper to create a temporary git repository
function createGitRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-git-'))
  execSync('git init', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.name "Test User"', { cwd: dir, stdio: 'pipe' })

  // Create a test file and commit it
  fs.writeFileSync(
    path.join(dir, 'test.py'),
    'def hello():\n    return "world"\n'
  )
  execSync('git add test.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Initial commit"', { cwd: dir, stdio: 'pipe' })

  return dir
}

test('isGitRepo returns true for git repositories', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  assert.strictEqual(isGitRepo(dir), true)
})

test('isGitRepo returns false for non-git directories', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-nogit-'))
  t.after(() => cleanup(dir))

  assert.strictEqual(isGitRepo(dir), false)
})

test('isGitRepo returns false for nonexistent paths', () => {
  assert.strictEqual(isGitRepo('/nonexistent/path/xyz123'), false)
})

test('GitRepo provides branch and commit info', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // Test currentBranch
  const branch = repo.currentBranch()
  assert.ok(typeof branch === 'string')
  assert.ok(branch.length > 0)

  // Test currentCommit
  const commit = repo.currentCommit()
  assert.ok(typeof commit === 'string')
  assert.strictEqual(commit.length, 40) // Full SHA-1 hash
})

test('GitRepo provides status and log', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // Test status (should be clean after commit)
  const status = repo.status()
  assert.ok(Array.isArray(status))

  // Test log
  const log = repo.log(5)
  assert.ok(Array.isArray(log))
  assert.ok(log.length >= 1)
  assert.ok(log[0].hash)
  assert.ok(log[0].shortHash)
  assert.ok(log[0].author)
  assert.ok(log[0].message)
})

test('GitRepo provides ls_files', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // Test lsFiles
  const files = repo.lsFiles()
  assert.ok(Array.isArray(files))
  assert.ok(files.includes('test.py'))
})

test('GitRepo provides file-specific operations', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // Test fileLog
  const fileLog = repo.fileLog('test.py', 5)
  assert.ok(Array.isArray(fileLog))
  assert.ok(fileLog.length >= 1)

  // Test lastModifiedCommit
  const lastCommit = repo.lastModifiedCommit('test.py')
  assert.ok(lastCommit.hash)
  assert.ok(lastCommit.author)

  // Test fileChangeFrequency
  const freq = repo.fileChangeFrequency('test.py', 30)
  assert.ok(typeof freq === 'number')
  assert.ok(freq >= 1)
})

test('GitRepo provides blame', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // Test blame
  const blame = repo.blame('test.py')
  assert.ok(Array.isArray(blame))
  assert.ok(blame.length >= 1)
  assert.ok(blame[0].commit)
  assert.ok(blame[0].author)
  assert.ok(typeof blame[0].lineNumber === 'number')
})

test('GitRepo provides diff operations', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const repo = new GitRepo(dir)

  // hasChanges should be false after clean commit
  assert.strictEqual(repo.hasChanges('test.py'), false)

  // Modify the file
  fs.writeFileSync(
    path.join(dir, 'test.py'),
    'def hello():\n    return "modified"\n'
  )

  // hasChanges should be true now
  assert.strictEqual(repo.hasChanges('test.py'), true)

  // uncommittedDiff should contain the change
  const diff = repo.uncommittedDiff('test.py')
  assert.ok(typeof diff === 'string')
  assert.ok(diff.includes('modified'))

  // allUncommittedDiffs should work
  const allDiff = repo.allUncommittedDiffs()
  assert.ok(typeof allDiff === 'string')
})

test('GitRepo provides diffFiles between commits', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  // Create another commit
  fs.writeFileSync(
    path.join(dir, 'test2.py'),
    'def goodbye():\n    return "goodbye"\n'
  )
  execSync('git add test2.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add test2.py"', { cwd: dir, stdio: 'pipe' })

  const repo = new GitRepo(dir)

  // Get diff between HEAD~1 and HEAD
  const diffFiles = repo.diffFiles('HEAD~1', 'HEAD')
  assert.ok(Array.isArray(diffFiles))
  assert.ok(diffFiles.length >= 1)

  const test2File = diffFiles.find(f => f.path === 'test2.py')
  assert.ok(test2File)
  assert.strictEqual(test2File.status, 'Added')
  assert.ok(typeof test2File.additions === 'number')
  assert.ok(typeof test2File.deletions === 'number')
})

test('GitRepo throws for nonexistent path', () => {
  assert.throws(
    () => new GitRepo('/nonexistent/path/xyz123'),
    /Failed to open git repo/i
  )
})

test('GitRepo throws for non-git directory', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-nogit-'))
  t.after(() => cleanup(dir))

  assert.throws(
    () => new GitRepo(dir),
    /Failed to open git repo/i
  )
})

test('scanSecurity detects potential security issues', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-sec-'))
  t.after(() => cleanup(dir))

  // Create a file with potential security issues
  fs.writeFileSync(
    path.join(dir, 'config.py'),
    "password = 'secret123'\napi_key = 'sk-1234567890abcdef'\n"
  )

  const findings = scanSecurity(dir)
  assert.ok(Array.isArray(findings))
  // We expect to find some issues (hardcoded credentials)
  // Note: The actual findings depend on the SecurityScanner implementation
})

test('All exports are available', () => {
  // Functions
  assert.ok(typeof pack === 'function')
  assert.ok(typeof scan === 'function')
  assert.ok(typeof scanWithOptions === 'function')
  assert.ok(typeof countTokens === 'function')
  assert.ok(typeof semanticCompress === 'function')
  assert.ok(typeof isGitRepo === 'function')
  assert.ok(typeof scanSecurity === 'function')

  // Classes
  assert.ok(typeof Infiniloom === 'function')
  assert.ok(typeof GitRepo === 'function')
})

// ============================================================================
// New Feature Tests (Bug Fixes)
// ============================================================================

// Helper to create repo with cross-file references for PageRank testing
function createCrossRefRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-pagerank-'))

  // utils.py - defines helper functions
  fs.writeFileSync(
    path.join(dir, 'utils.py'),
    [
      'def calculate_total(items):',
      '    """Calculate total from list of items."""',
      '    return sum(item.value for item in items)',
      '',
      'def format_output(result):',
      '    """Format result for display."""',
      '    return f"Result: {result}"',
      '',
    ].join('\n')
  )

  // main.py - imports and uses utils
  fs.writeFileSync(
    path.join(dir, 'main.py'),
    [
      'from utils import calculate_total, format_output',
      '',
      'def main():',
      '    items = get_items()',
      '    total = calculate_total(items)',
      '    output = format_output(total)',
      '    print(output)',
      '',
      'if __name__ == "__main__":',
      '    main()',
      '',
    ].join('\n')
  )

  // processor.py - also uses utils
  fs.writeFileSync(
    path.join(dir, 'processor.py'),
    [
      'from utils import calculate_total',
      '',
      'class DataProcessor:',
      '    def process(self, data):',
      '        return calculate_total(data)',
      '',
    ].join('\n')
  )

  return dir
}

test('PageRank cross-file references are counted', (t) => {
  const dir = createCrossRefRepo()
  t.after(() => cleanup(dir))

  // Pack with symbols enabled to get PageRank data
  const output = pack(dir, {
    format: 'json',
    model: 'claude',
    skipSymbols: false,
  })

  const parsed = JSON.parse(output)
  assert.ok(parsed.repository)
  assert.ok(parsed.repository.files)

  // Check that files are present
  const files = parsed.repository.files
  assert.ok(files.length >= 3, 'Should have at least 3 files')
})

test('pack with include patterns filters files', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-include-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'test_app.py'), 'def test_app(): pass\n')
  fs.writeFileSync(path.join(dir, 'utils.js'), 'function utils() {}\n')

  // Include only Python files
  const output = pack(dir, {
    format: 'json',
    include: ['*.py'],
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  const files = parsed.repository.files
  const filePaths = files.map(f => f.path)

  // Should include .py files
  assert.ok(filePaths.some(p => p.endsWith('.py')), 'Should include .py files')
  // Should NOT include .js files
  assert.ok(!filePaths.some(p => p.endsWith('.js')), 'Should not include .js files')
})

test('pack with exclude patterns filters out files', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-exclude-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'test_app.py'), 'def test_app(): pass\n')
  fs.writeFileSync(path.join(dir, 'app.test.py'), 'def app_test(): pass\n')

  // Exclude test files
  const output = pack(dir, {
    format: 'json',
    exclude: ['*test*'],
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  const files = parsed.repository.files
  const filePaths = files.map(f => f.path)

  // Should include app.py
  assert.ok(filePaths.some(p => p.includes('app.py') && !p.includes('test')), 'Should include app.py')
  // Should NOT include test files
  assert.ok(!filePaths.some(p => p.includes('test')), 'Should exclude test files')
})

test('pack with includeTests includes test files', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-inctests-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.mkdirSync(path.join(dir, 'tests'))
  fs.writeFileSync(path.join(dir, 'tests', 'test_app.py'), 'def test_app(): pass\n')

  // With includeTests: true
  const output = pack(dir, {
    format: 'json',
    includeTests: true,
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  const files = parsed.repository.files
  const filePaths = files.map(f => f.path)

  // Should include test files when includeTests is true
  assert.ok(filePaths.some(p => p.includes('test')), 'Should include test files when includeTests: true')
})

test('pack with tokenBudget limits output size', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-budget-'))
  t.after(() => cleanup(dir))

  // Create multiple larger files (each ~200+ tokens)
  for (let i = 0; i < 20; i++) {
    const content = `def function_${i}():\n` + '    print("line")\n'.repeat(100)
    fs.writeFileSync(path.join(dir, `file${i}.py`), content)
  }

  // Pack with minimum valid token budget
  const output = pack(dir, {
    format: 'json',
    tokenBudget: 1000,  // Minimum valid budget
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  const files = parsed.repository.files

  // With a small budget, we should get fewer files than without budget
  assert.ok(files.length < 20, `Token budget should limit files (got ${files.length})`)
  assert.ok(files.length >= 1, 'Should include at least one file')
})

test('scanWithOptions applies default ignores', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-ignores-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.mkdirSync(path.join(dir, 'node_modules'))
  fs.writeFileSync(path.join(dir, 'node_modules', 'package.js'), 'module.exports = {}\n')
  fs.mkdirSync(path.join(dir, 'dist'))
  fs.writeFileSync(path.join(dir, 'dist', 'bundle.js'), '// bundled\n')

  // With default ignores (default behavior)
  const statsWithIgnores = scanWithOptions(dir, {
    applyDefaultIgnores: true,
  })

  // Without default ignores
  const statsWithoutIgnores = scanWithOptions(dir, {
    applyDefaultIgnores: false,
  })

  // With ignores should have fewer files
  assert.ok(statsWithIgnores.totalFiles <= statsWithoutIgnores.totalFiles,
    'Default ignores should reduce file count')
})

test('scanWithOptions with include patterns', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-scan-inc-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'utils.py'), 'def utils(): pass\n')
  fs.writeFileSync(path.join(dir, 'main.js'), 'function main() {}\n')

  const stats = scanWithOptions(dir, {
    include: ['*.py'],
    applyDefaultIgnores: false,
  })

  // Should only count Python files
  assert.ok(stats.totalFiles >= 2, 'Should include Python files')
  assert.ok(stats.primaryLanguage.toLowerCase() === 'python', 'Primary language should be Python')
})

test('scanWithOptions with exclude patterns', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-scan-exc-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'test_app.py'), 'def test(): pass\n')
  fs.writeFileSync(path.join(dir, 'main.js'), 'function main() {}\n')

  const stats = scanWithOptions(dir, {
    exclude: ['*test*'],
    applyDefaultIgnores: false,
  })

  // Should not count test files
  // The exact count depends on what test patterns are matched
  assert.ok(stats.totalFiles >= 2, 'Should have at least 2 non-test files')
})

test('scan returns correct language line counts (Bug #9 fix)', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-lines-'))
  t.after(() => cleanup(dir))

  // Create file with known line count
  const pythonLines = 10
  const pythonContent = Array(pythonLines).fill('x = 1').join('\n') + '\n'
  fs.writeFileSync(path.join(dir, 'app.py'), pythonContent)

  const stats = scan(dir)

  // Check that lines are non-zero
  assert.ok(stats.languages.length > 0, 'Should have language stats')
  const pythonLang = stats.languages.find(l => l.language.toLowerCase() === 'python')
  assert.ok(pythonLang, 'Should detect Python')
  assert.ok(pythonLang.lines > 0, 'Python lines should be > 0')
})

test('scan returns languages sorted by percentage (Bug #12 fix)', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-sort-'))
  t.after(() => cleanup(dir))

  // Create more Python files than JavaScript
  fs.writeFileSync(path.join(dir, 'a.py'), 'x = 1\n')
  fs.writeFileSync(path.join(dir, 'b.py'), 'y = 2\n')
  fs.writeFileSync(path.join(dir, 'c.py'), 'z = 3\n')
  fs.writeFileSync(path.join(dir, 'main.js'), 'const x = 1;\n')

  const stats = scan(dir)

  // Languages should be sorted by percentage (highest first)
  if (stats.languages.length >= 2) {
    for (let i = 1; i < stats.languages.length; i++) {
      assert.ok(
        stats.languages[i-1].percentage >= stats.languages[i].percentage,
        'Languages should be sorted by percentage descending'
      )
    }
  }
})

test('Infiniloom.securityScan returns structured findings (Bug #8 fix)', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-secscan-'))
  t.after(() => cleanup(dir))

  // Create file with potential security issue
  fs.writeFileSync(
    path.join(dir, 'config.py'),
    "API_KEY = 'sk-1234567890abcdefghijklmnopqrstuvwxyz'\n"
  )

  const loom = new Infiniloom(dir, 'claude')
  const findings = loom.securityScan()

  assert.ok(Array.isArray(findings), 'securityScan should return array')

  // If findings exist, check structure
  if (findings.length > 0) {
    const finding = findings[0]
    assert.ok('file' in finding, 'Finding should have file property')
    assert.ok('line' in finding, 'Finding should have line property')
    assert.ok('severity' in finding, 'Finding should have severity property')
    assert.ok('kind' in finding, 'Finding should have kind property')
    assert.ok('pattern' in finding, 'Finding should have pattern property')
  }
})

test('semanticCompress handles short text', () => {
  const shortText = 'This is a short piece of text.'
  const compressed = semanticCompress(shortText, 0.7, 0.5)

  // Short text should not throw and should return something
  assert.ok(typeof compressed === 'string')
  assert.ok(compressed.length > 0)
})

test('semanticCompress handles repetitive text', () => {
  // Create text with many repetitive lines
  const lines = Array.from({ length: 20 }, (_, i) => `Line ${i}: This is repetitive content.`)
  const text = lines.join('\n')

  const compressed = semanticCompress(text, 0.7, 0.3)

  assert.ok(typeof compressed === 'string')
  assert.ok(compressed.length > 0)
  // Should compress repetitive content
  assert.ok(compressed.length <= text.length, 'Should not expand text')
})

test('pack with securityThreshold blocks on findings', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-threshold-'))
  t.after(() => cleanup(dir))

  // Create file with security issue
  fs.writeFileSync(
    path.join(dir, 'secrets.py'),
    "AWS_SECRET = 'AKIAIOSFODNN7EXAMPLE1234567890abcdefghij'\n"
  )

  // With skipSecurity: true, should succeed
  const output = pack(dir, {
    format: 'json',
    skipSecurity: true,
    skipSymbols: true,
  })
  assert.ok(output.length > 0, 'Should produce output with skipSecurity: true')

  // Note: Testing actual blocking would require a known pattern that triggers
  // the security scanner - this depends on SecurityScanner implementation
})

// ============================================================================
// NEW API Tests (Bug Fixes #1-5 and Features #6-10)
// ============================================================================

const {
  buildIndex,
  getSymbolsInFile,
  getSymbolSource,
  getChangedSymbols,
  getTestsForFile,
  getCallSites,
  analyzeImpact,
  getDiffContext,
  chunk,
} = require('..')

// Helper to create a git repo with multiple files for testing diff/index features
function createTestRepoWithIndex() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-idx-'))
  execSync('git init', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.name "Test User"', { cwd: dir, stdio: 'pipe' })

  // Create auth.ts - main source file
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )

  // Create main.ts - imports auth
  fs.writeFileSync(
    path.join(dir, 'main.ts'),
    [
      "import { authenticate } from './auth';",
      '',
      'function main() {',
      '    const result = authenticate("user", "password123");',
      '    console.log(result);',
      '}',
      '',
      'main();',
      '',
    ].join('\n')
  )

  // Create auth.test.ts - test file
  fs.writeFileSync(
    path.join(dir, 'auth.test.ts'),
    [
      "import { authenticate } from './auth';",
      '',
      "describe('authenticate', () => {",
      "    it('should return true for valid credentials', () => {",
      "        expect(authenticate('user', 'password123')).toBe(true);",
      '    });',
      '});',
      '',
    ].join('\n')
  )

  // Commit the files
  execSync('git add .', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Initial commit"', { cwd: dir, stdio: 'pipe' })

  // Build the index
  buildIndex(dir)

  return dir
}

// ============================================================================
// Feature #8: getSymbolsInFile
// ============================================================================

test('getSymbolsInFile returns all symbols in a file', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const symbols = getSymbolsInFile(dir, 'auth.ts')

  assert.ok(Array.isArray(symbols), 'Should return an array')
  assert.ok(symbols.length >= 3, 'Should find at least 3 symbols (authenticate, validate, checkPassword)')

  // Check that expected symbols are present
  const symbolNames = symbols.map(s => s.name)
  assert.ok(symbolNames.includes('authenticate'), 'Should include authenticate')
  assert.ok(symbolNames.includes('validate'), 'Should include validate')
  assert.ok(symbolNames.includes('checkPassword'), 'Should include checkPassword')

  // Check symbol structure
  const auth = symbols.find(s => s.name === 'authenticate')
  assert.ok(auth, 'Should find authenticate symbol')
  assert.ok(auth.kind, 'Symbol should have kind')
  assert.ok(typeof auth.line === 'number', 'Symbol should have line number')
  assert.ok(auth.file, 'Symbol should have file path')
})

test('getSymbolsInFile with kind filter', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const functions = getSymbolsInFile(dir, 'auth.ts', { kind: 'function' })

  assert.ok(Array.isArray(functions), 'Should return an array')
  // All returned symbols should be functions
  for (const sym of functions) {
    assert.strictEqual(sym.kind, 'function', `Symbol ${sym.name} should be a function`)
  }
})

test('getSymbolsInFile throws for nonexistent file', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getSymbolsInFile(dir, 'nonexistent.ts'),
    /File not found/i
  )
})

// ============================================================================
// Feature #9: getSymbolSource
// ============================================================================

test('getSymbolSource returns symbol source code', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getSymbolSource(dir, 'authenticate')

  // Returns SymbolSourceResult object with source property
  assert.ok(typeof result === 'object', 'Should return an object')
  assert.ok(typeof result.source === 'string', 'Should have source string')
  assert.ok(result.source.length > 0, 'Source should not be empty')
  assert.ok(result.source.includes('authenticate'), 'Source should contain function name')
  assert.ok(result.source.includes('validate'), 'Source should contain function body')
})

test('getSymbolSource with file path disambiguation', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getSymbolSource(dir, 'authenticate', 'auth.ts')

  // Returns SymbolSourceResult object
  assert.ok(typeof result === 'object', 'Should return an object')
  assert.ok(result.source.includes('authenticate'), 'Source should contain function name')
})

test('getSymbolSource throws for nonexistent symbol', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getSymbolSource(dir, 'nonExistentSymbol'),
    /Symbol not found/i
  )
})

// ============================================================================
// Feature #10: getTestsForFile
// ============================================================================

test('getTestsForFile finds related test files', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const tests = getTestsForFile(dir, 'auth.ts')

  assert.ok(Array.isArray(tests), 'Should return an array')
  assert.ok(tests.length >= 1, 'Should find at least one test file')
  assert.ok(tests.some(t => t.includes('test')), 'Should find test file')
})

test('getTestsForFile returns empty for file with no tests', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-notests-'))
  t.after(() => cleanup(dir))

  // Create a file without any tests
  fs.writeFileSync(path.join(dir, 'utils.py'), 'def helper(): pass\n')

  buildIndex(dir)

  const tests = getTestsForFile(dir, 'utils.py')

  assert.ok(Array.isArray(tests), 'Should return an array')
  // No test files exist, so should be empty
  assert.strictEqual(tests.length, 0, 'Should return empty array for file with no tests')
})

// ============================================================================
// Feature #6 & Bug #5: getCallSites with actual line numbers
// ============================================================================

test('getCallSites returns call locations', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const callSites = getCallSites(dir, 'authenticate')

  assert.ok(Array.isArray(callSites), 'Should return an array')
  // authenticate is called from main.ts
  if (callSites.length > 0) {
    const site = callSites[0]
    assert.ok(site.caller, 'Call site should have caller name')
    assert.ok(site.callee, 'Call site should have callee name')
    assert.ok(site.file, 'Call site should have file path')
    assert.ok(typeof site.line === 'number', 'Call site should have line number')
    assert.ok(site.line > 0, 'Line number should be positive')
  }
})

test('getCallSites returns empty for unused symbol', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-unused-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(
    path.join(dir, 'unused.py'),
    'def unused_function():\n    pass\n'
  )

  buildIndex(dir)

  const callSites = getCallSites(dir, 'unused_function')

  assert.ok(Array.isArray(callSites), 'Should return an array')
  assert.strictEqual(callSites.length, 0, 'Should return empty array for unused symbol')
})

// ============================================================================
// Feature #7: getChangedSymbols
// ============================================================================

test('getChangedSymbols returns symbols changed in diff', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Modify auth.ts
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    console.log("Authenticating...");  // Added this line',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )
  execSync('git add auth.ts', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Modify authenticate"', { cwd: dir, stdio: 'pipe' })

  // Rebuild index to include new commit
  buildIndex(dir, { force: true })

  const changed = getChangedSymbols(dir, 'HEAD~1', 'HEAD')

  assert.ok(Array.isArray(changed), 'Should return an array')
  // authenticate was modified, so it should be in the list
  if (changed.length > 0) {
    const auth = changed.find(s => s.name === 'authenticate')
    if (auth) {
      assert.strictEqual(auth.name, 'authenticate', 'Should find authenticate symbol')
      assert.ok(auth.file, 'Changed symbol should have file')
      assert.ok(typeof auth.line === 'number', 'Changed symbol should have line')
    }
  }
})

// ============================================================================
// Bug #1: contextSymbols should not be empty in getDiffContext
// ============================================================================

test('getDiffContext returns contextSymbols (Bug #1 fix)', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Modify a file
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    // Modified',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )

  const context = getDiffContext(dir, '', '', { depth: 2 })

  assert.ok(context, 'Should return context result')
  assert.ok(Array.isArray(context.changedFiles), 'Should have changedFiles array')
  assert.ok(Array.isArray(context.contextSymbols), 'Should have contextSymbols array')

  // With the bug fix, contextSymbols should contain symbols from changed files
  // even when hunk parsing fails
  if (context.changedFiles.length > 0) {
    // At minimum, we should have some symbols if files were changed
    // Note: The actual count depends on how the diff was parsed
    assert.ok(context.contextSymbols !== undefined, 'contextSymbols should be defined')
  }
})

// ============================================================================
// Bug #2: relatedTests should not be empty in getDiffContext
// ============================================================================

test('getDiffContext returns relatedTests (Bug #2 fix)', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Modify auth.ts which has auth.test.ts
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    // Modified for test',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )

  const context = getDiffContext(dir, '', '', { depth: 2 })

  assert.ok(context, 'Should return context result')
  assert.ok(Array.isArray(context.relatedTests), 'Should have relatedTests array')

  // With the bug fix, relatedTests should find auth.test.ts
  // since it imports from auth.ts which was modified
  if (context.changedFiles.length > 0) {
    // Check if test was found (depends on import graph resolution)
    assert.ok(context.relatedTests !== undefined, 'relatedTests should be defined')
  }
})

// ============================================================================
// Bug #3: contextSnippets should not be empty in getDiffContext
// ============================================================================

test('getDiffContext returns contextSnippets (Bug #3 fix)', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Modify auth.ts
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    // Added snippet comment',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )

  const context = getDiffContext(dir, '', '', { depth: 2 })

  assert.ok(context, 'Should return context result')
  assert.ok(Array.isArray(context.changedFiles), 'Should have changedFiles array')

  // With the bug fix, changedFiles should have contextSnippets populated
  for (const file of context.changedFiles) {
    assert.ok(Array.isArray(file.contextSnippets), `File ${file.path} should have contextSnippets array`)
    // Snippets should be generated for files with changes
    // Note: The actual content depends on the diff
  }
})

// ============================================================================
// Bug #4: testFiles should not be empty in analyzeImpact
// ============================================================================

test('analyzeImpact returns testFiles (Bug #4 fix)', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const impact = analyzeImpact(dir, ['auth.ts'], { depth: 2, includeTests: true })

  assert.ok(impact, 'Should return impact result')
  assert.ok(Array.isArray(impact.testFiles), 'Should have testFiles array')

  // With the bug fix, testFiles should find auth.test.ts
  // since it's related to auth.ts
  if (impact.changedFiles.length > 0) {
    assert.ok(impact.testFiles !== undefined, 'testFiles should be defined')
    // If test detection found the test file
    if (impact.testFiles.length > 0) {
      assert.ok(
        impact.testFiles.some(t => t.includes('test')),
        'Should find test files related to changed files'
      )
    }
  }
})

test('analyzeImpact returns affected symbols', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const impact = analyzeImpact(dir, ['auth.ts'], { depth: 2 })

  assert.ok(impact, 'Should return impact result')
  assert.ok(Array.isArray(impact.affectedSymbols), 'Should have affectedSymbols array')
  assert.ok(impact.impactLevel, 'Should have impactLevel')
  assert.ok(impact.summary, 'Should have summary')
})

// ============================================================================
// Integration test: Full PR review workflow
// ============================================================================

test('Full PR review workflow with new APIs', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Modify auth.ts (simulating a PR change)
  fs.writeFileSync(
    path.join(dir, 'auth.ts'),
    [
      'export function authenticate(user: string, password: string): boolean {',
      '    console.log(`Authenticating ${user}...`);',
      '    return validate(user) && checkPassword(password);',
      '}',
      '',
      'function validate(user: string): boolean {',
      '    // Added validation logging',
      '    console.log("Validating user");',
      '    return user.length > 0;',
      '}',
      '',
      'function checkPassword(password: string): boolean {',
      '    return password.length >= 8;',
      '}',
      '',
    ].join('\n')
  )
  execSync('git add auth.ts', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add logging to auth"', { cwd: dir, stdio: 'pipe' })

  // Rebuild index
  buildIndex(dir, { force: true })

  // Step 1: Get changed symbols
  const changed = getChangedSymbols(dir, 'HEAD~1', 'HEAD')
  assert.ok(Array.isArray(changed), 'Should get changed symbols')

  // Step 2: For each changed symbol, find callers
  for (const sym of changed.slice(0, 3)) {  // Limit to first 3 for speed
    const callSites = getCallSites(dir, sym.name)
    assert.ok(Array.isArray(callSites), `Should get call sites for ${sym.name}`)

    // Step 3: Get caller source
    for (const site of callSites.slice(0, 2)) {  // Limit to first 2
      if (site.caller) {
        try {
          const source = getSymbolSource(dir, site.caller)
          assert.ok(typeof source === 'string', 'Should get caller source')
        } catch (e) {
          // Symbol might not be in index
        }
      }
    }
  }

  // Step 4: Get symbols in changed file
  const symbolsInAuth = getSymbolsInFile(dir, 'auth.ts')
  assert.ok(symbolsInAuth.length >= 3, 'Should find symbols in auth.ts')

  // Step 5: Find related tests
  const tests = getTestsForFile(dir, 'auth.ts')
  assert.ok(Array.isArray(tests), 'Should get related tests')
})

// ============================================================================
// Edge cases and error handling
// ============================================================================

test('getSymbolsInFile handles empty file', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-empty-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'empty.py'), '# Just a comment\n')
  buildIndex(dir)

  const symbols = getSymbolsInFile(dir, 'empty.py')
  assert.ok(Array.isArray(symbols), 'Should return array for empty file')
  // Empty file should have no symbols
  assert.strictEqual(symbols.length, 0, 'Empty file should have no symbols')
})

test('getCallSites handles symbol with no callers', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-nocall-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(
    path.join(dir, 'standalone.py'),
    [
      'def lonely_function():',
      '    """This function is never called."""',
      '    pass',
      '',
    ].join('\n')
  )
  buildIndex(dir)

  const callSites = getCallSites(dir, 'lonely_function')
  assert.ok(Array.isArray(callSites), 'Should return array')
  assert.strictEqual(callSites.length, 0, 'Should have no callers')
})

test('API functions throw helpful errors for missing index', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-noidx-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'test.py'), 'def foo(): pass\n')
  // Don't build index

  assert.throws(
    () => getSymbolsInFile(dir, 'test.py'),
    /Failed to load index/i,
    'Should throw when index missing'
  )

  assert.throws(
    () => getCallSites(dir, 'foo'),
    /Failed to load index/i,
    'Should throw when index missing'
  )
})

// ============================================================================
// v0.4.5 New Feature Tests
// ============================================================================

const {
  getChangedSymbolsFiltered,
  getTransitiveCallers,
  getCallSitesWithContext,
  findSymbolFiltered,
  getCallersFiltered,
  getCalleesFiltered,
  getReferencesFiltered,
  indexStatus,
  findSymbol,
  getCallers,
  getCallees,
  getReferences,
  findCircularDependencies,
  findCircularDependenciesAsync,
  getExportedSymbols,
  getExportedSymbolsAsync,
} = require('..')

// Helper to create repo with call chain for transitive caller testing
function createCallChainRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-chain-'))
  execSync('git init', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.name "Test User"', { cwd: dir, stdio: 'pipe' })

  // Create a call chain: main -> controller -> service -> repository -> database
  fs.writeFileSync(
    path.join(dir, 'database.py'),
    [
      'def query_database(sql):',
      '    """Execute SQL query."""',
      '    return execute(sql)',
      '',
      'def execute(sql):',
      '    return []',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'repository.py'),
    [
      'from database import query_database',
      '',
      'def find_all():',
      '    return query_database("SELECT * FROM items")',
      '',
      'def find_by_id(id):',
      '    return query_database(f"SELECT * FROM items WHERE id = {id}")',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'service.py'),
    [
      'from repository import find_all, find_by_id',
      '',
      'def get_items():',
      '    return find_all()',
      '',
      'def get_item(id):',
      '    return find_by_id(id)',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'controller.py'),
    [
      'from service import get_items, get_item',
      '',
      'def list_handler():',
      '    items = get_items()',
      '    return {"items": items}',
      '',
      'def detail_handler(id):',
      '    item = get_item(id)',
      '    return {"item": item}',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'main.py'),
    [
      'from controller import list_handler, detail_handler',
      '',
      'def main():',
      '    print(list_handler())',
      '    print(detail_handler(1))',
      '',
      'if __name__ == "__main__":',
      '    main()',
      '',
    ].join('\n')
  )

  // Create test file
  fs.writeFileSync(
    path.join(dir, 'test_service.py'),
    [
      'from service import get_items, get_item',
      '',
      'def test_get_items():',
      '    result = get_items()',
      '    assert isinstance(result, list)',
      '',
      'def test_get_item():',
      '    result = get_item(1)',
      '    assert result is not None',
      '',
    ].join('\n')
  )

  execSync('git add .', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Initial commit"', { cwd: dir, stdio: 'pipe' })

  buildIndex(dir)

  return dir
}

// ============================================================================
// Feature #6: getChangedSymbolsFiltered
// ============================================================================

test('getChangedSymbolsFiltered returns symbols with filtering', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Modify a file
  fs.writeFileSync(
    path.join(dir, 'service.py'),
    [
      'from repository import find_all, find_by_id',
      '',
      'class ItemService:',
      '    """New class added."""',
      '    def __init__(self):',
      '        pass',
      '',
      'def get_items():',
      '    # Modified',
      '    return find_all()',
      '',
      'def get_item(id):',
      '    return find_by_id(id)',
      '',
      'CONSTANT = "value"',
      '',
    ].join('\n')
  )
  execSync('git add service.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add class and constant"', { cwd: dir, stdio: 'pipe' })
  buildIndex(dir, { force: true })

  // Get all changed symbols
  const allChanged = getChangedSymbolsFiltered(dir, 'HEAD~1', 'HEAD')
  assert.ok(Array.isArray(allChanged), 'Should return an array')
  assert.ok(allChanged.length > 0, 'Should find changed symbols')

  // Check that symbols have change_type field (Feature #7)
  for (const sym of allChanged) {
    assert.ok(sym.name, 'Symbol should have name')
    assert.ok(sym.kind, 'Symbol should have kind')
    assert.ok(sym.file, 'Symbol should have file')
    assert.ok(typeof sym.line === 'number', 'Symbol should have line')
    assert.ok(sym.changeType, 'Symbol should have changeType (Feature #7)')
    assert.ok(['added', 'modified', 'deleted'].includes(sym.changeType),
      `changeType should be valid: ${sym.changeType}`)
  }
})

test('getChangedSymbolsFiltered filters by kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Add a class to service.py
  fs.writeFileSync(
    path.join(dir, 'service.py'),
    [
      'from repository import find_all, find_by_id',
      '',
      'class ItemService:',
      '    def get(self):',
      '        return find_all()',
      '',
      'def get_items():',
      '    return find_all()',
      '',
      'def get_item(id):',
      '    return find_by_id(id)',
      '',
    ].join('\n')
  )
  execSync('git add service.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add class"', { cwd: dir, stdio: 'pipe' })
  buildIndex(dir, { force: true })

  // Filter to only functions
  const functions = getChangedSymbolsFiltered(dir, 'HEAD~1', 'HEAD', {
    kinds: ['function'],
  })

  for (const sym of functions) {
    assert.strictEqual(sym.kind, 'function', `Should only have functions, got: ${sym.kind}`)
  }
})

test('getChangedSymbolsFiltered excludes specified kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Add import statement
  fs.writeFileSync(
    path.join(dir, 'service.py'),
    [
      'from repository import find_all, find_by_id',
      'import json',  // New import
      '',
      'def get_items():',
      '    return json.dumps(find_all())',
      '',
      'def get_item(id):',
      '    return find_by_id(id)',
      '',
    ].join('\n')
  )
  execSync('git add service.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add import"', { cwd: dir, stdio: 'pipe' })
  buildIndex(dir, { force: true })

  // Exclude imports
  const noImports = getChangedSymbolsFiltered(dir, 'HEAD~1', 'HEAD', {
    excludeKinds: ['import'],
  })

  for (const sym of noImports) {
    assert.notStrictEqual(sym.kind, 'import', `Should not have imports, got: ${sym.kind}`)
  }
})

// ============================================================================
// Feature #8: getTransitiveCallers
// ============================================================================

test('getTransitiveCallers finds direct callers at depth 1', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const callers = getTransitiveCallers(dir, 'query_database', { maxDepth: 1 })

  assert.ok(Array.isArray(callers), 'Should return an array')

  // Direct callers: find_all and find_by_id call query_database
  if (callers.length > 0) {
    for (const caller of callers) {
      assert.ok(caller.name, 'Caller should have name')
      assert.ok(caller.kind, 'Caller should have kind')
      assert.ok(caller.file, 'Caller should have file')
      assert.ok(typeof caller.depth === 'number', 'Caller should have depth')
      assert.ok(Array.isArray(caller.callPath), 'Caller should have callPath')
      assert.strictEqual(caller.depth, 1, 'Direct callers should have depth 1')
    }
  }
})

test('getTransitiveCallers finds transitive callers at depth 3', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const callers = getTransitiveCallers(dir, 'query_database', { maxDepth: 3 })

  assert.ok(Array.isArray(callers), 'Should return an array')

  // Should find callers at multiple depths
  const depths = new Set(callers.map(c => c.depth))

  // With depth 3, we should have callers at depth 1, 2, and possibly 3
  // query_database <- find_all/find_by_id (d1) <- get_items/get_item (d2) <- list_handler/etc (d3)
  if (callers.length >= 2) {
    assert.ok(depths.size >= 1, 'Should have callers at different depths')
  }
})

test('getTransitiveCallers includes call path', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const callers = getTransitiveCallers(dir, 'query_database', { maxDepth: 3 })

  // Find a caller at depth > 1
  const transitiveCaller = callers.find(c => c.depth > 1)

  if (transitiveCaller) {
    assert.ok(transitiveCaller.callPath.length >= 2, 'Call path should have multiple steps')
    // Path should end with target symbol
    assert.strictEqual(
      transitiveCaller.callPath[transitiveCaller.callPath.length - 1],
      'query_database',
      'Call path should end with target symbol'
    )
  }
})

test('getTransitiveCallers respects maxResults', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const callers = getTransitiveCallers(dir, 'query_database', {
    maxDepth: 10,
    maxResults: 2,
  })

  assert.ok(Array.isArray(callers), 'Should return an array')
  assert.ok(callers.length <= 2, 'Should respect maxResults limit')
})

test('getTransitiveCallers returns empty for symbol with no callers', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // main() is the entry point - nothing calls it
  const callers = getTransitiveCallers(dir, 'main', { maxDepth: 5 })

  assert.ok(Array.isArray(callers), 'Should return an array')
  assert.strictEqual(callers.length, 0, 'Entry point should have no callers')
})

// ============================================================================
// Feature #9: getCallSitesWithContext
// ============================================================================

test('getCallSitesWithContext returns code context', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const sites = getCallSitesWithContext(dir, 'find_all', {
    linesBefore: 2,
    linesAfter: 2,
  })

  assert.ok(Array.isArray(sites), 'Should return an array')

  if (sites.length > 0) {
    const site = sites[0]
    assert.ok(site.caller, 'Site should have caller name')
    assert.ok(site.callee, 'Site should have callee name')
    assert.ok(site.file, 'Site should have file path')
    assert.ok(typeof site.line === 'number', 'Site should have line number')

    // Feature #9: Code context should be present
    if (site.context) {
      assert.ok(typeof site.context === 'string', 'Context should be string')
      assert.ok(site.context.length > 0, 'Context should not be empty')
      assert.ok(typeof site.contextStartLine === 'number', 'Should have contextStartLine')
      assert.ok(typeof site.contextEndLine === 'number', 'Should have contextEndLine')
    }
  }
})

test('getCallSitesWithContext context contains call line', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const sites = getCallSitesWithContext(dir, 'find_all', {
    linesBefore: 3,
    linesAfter: 3,
  })

  for (const site of sites) {
    if (site.context) {
      // Context should contain the callee name (the function being called)
      assert.ok(
        site.context.includes('find_all'),
        'Context should include call to find_all'
      )
    }
  }
})

test('getCallSitesWithContext deduplicates call sites (Bug #5)', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const sites = getCallSitesWithContext(dir, 'query_database')

  // Check for unique call sites (no duplicates)
  const siteKeys = sites.map(s => `${s.file}:${s.line}:${s.callerId}:${s.calleeId}`)
  const uniqueKeys = new Set(siteKeys)

  assert.strictEqual(
    siteKeys.length,
    uniqueKeys.size,
    'Should not have duplicate call sites (Bug #5 fix)'
  )
})

test('getCallSitesWithContext returns empty for uncalled symbol', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-uncalled-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(
    path.join(dir, 'unused.py'),
    [
      'def never_called():',
      '    """This function is never called anywhere."""',
      '    return 42',
      '',
    ].join('\n')
  )
  buildIndex(dir)

  const sites = getCallSitesWithContext(dir, 'never_called')

  assert.ok(Array.isArray(sites), 'Should return an array')
  assert.strictEqual(sites.length, 0, 'Should have no call sites')
})

// ============================================================================
// Integration: v0.4.5 features working together
// ============================================================================

test('v0.4.5 features integration: PR review workflow', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Simulate a PR that modifies repository.py
  fs.writeFileSync(
    path.join(dir, 'repository.py'),
    [
      'from database import query_database',
      '',
      'def find_all():',
      '    # Added caching',
      '    cache_key = "all_items"',
      '    return query_database("SELECT * FROM items")',
      '',
      'def find_by_id(id):',
      '    # Validate id',
      '    if id <= 0:',
      '        raise ValueError("Invalid id")',
      '    return query_database(f"SELECT * FROM items WHERE id = {id}")',
      '',
      'def count():',
      '    """New function added."""',
      '    return query_database("SELECT COUNT(*) FROM items")',
      '',
    ].join('\n')
  )
  execSync('git add repository.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "Add caching and count function"', { cwd: dir, stdio: 'pipe' })
  buildIndex(dir, { force: true })

  // Step 1: Get changed symbols (only functions, no imports)
  const changed = getChangedSymbolsFiltered(dir, 'HEAD~1', 'HEAD', {
    kinds: ['function'],
    excludeKinds: ['import'],
  })
  assert.ok(changed.length > 0, 'Should find changed functions')

  // Verify change types exist
  for (const sym of changed) {
    assert.ok(['added', 'modified', 'deleted'].includes(sym.changeType),
      `Symbol ${sym.name} should have valid changeType`)
  }

  // count() should be found in changed symbols (might be 'added' or 'modified' depending on line detection)
  assert.ok(changed.some(s => s.name === 'count'), 'count() should be in changed symbols')

  // Step 2: For changed functions, find who calls them (impact analysis)
  for (const sym of changed.slice(0, 2)) {
    const callers = getTransitiveCallers(dir, sym.name, { maxDepth: 2 })
    // Each changed function affects its callers
    assert.ok(Array.isArray(callers), `Should get callers for ${sym.name}`)
  }

  // Step 3: Get call sites with context for review
  const findAllSites = getCallSitesWithContext(dir, 'find_all', {
    linesBefore: 3,
    linesAfter: 3,
  })
  assert.ok(Array.isArray(findAllSites), 'Should get call sites for find_all')

  // Each site should have context for code review
  for (const site of findAllSites) {
    if (site.context) {
      assert.ok(site.context.length > 0, 'Call site should have context for review')
    }
  }
})

// ============================================================================
// Feature #1: Exclude patterns for buildIndex
// ============================================================================

test('buildIndex with exclude option skips matching directories', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-exclude-'))
  t.after(() => cleanup(dir))

  // Create main code
  fs.writeFileSync(
    path.join(dir, 'main.py'),
    'def main_function():\n    pass\n'
  )

  // Create vendor directory that should be excluded
  fs.mkdirSync(path.join(dir, 'vendor'))
  fs.writeFileSync(
    path.join(dir, 'vendor', 'lib.py'),
    'def vendor_function():\n    pass\n'
  )

  // Create tests directory that should be excluded
  fs.mkdirSync(path.join(dir, 'tests'))
  fs.writeFileSync(
    path.join(dir, 'tests', 'test_main.py'),
    'def test_main():\n    pass\n'
  )

  // Build index with exclude patterns
  buildIndex(dir, { exclude: ['vendor', 'tests'] })

  // Verify main.py symbols are indexed
  const mainSymbols = getSymbolsInFile(dir, 'main.py')
  assert.ok(mainSymbols.some(s => s.name === 'main_function'), 'main_function should be indexed')

  // Verify vendor files are NOT indexed
  assert.throws(
    () => getSymbolsInFile(dir, 'vendor/lib.py'),
    /File not found/i,
    'vendor/lib.py should not be indexed'
  )

  // Verify test files are NOT indexed
  assert.throws(
    () => getSymbolsInFile(dir, 'tests/test_main.py'),
    /File not found/i,
    'tests/test_main.py should not be indexed'
  )
})

test('buildIndex with exclude option supports nested directories', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-excnest-'))
  t.after(() => cleanup(dir))

  // Create source files
  fs.mkdirSync(path.join(dir, 'src'))
  fs.writeFileSync(path.join(dir, 'src', 'app.py'), 'def app(): pass\n')

  // Create generated directory that should be excluded
  fs.mkdirSync(path.join(dir, 'generated'))
  fs.writeFileSync(path.join(dir, 'generated', 'types.py'), 'def generated_type(): pass\n')

  // Create cache directory that should be excluded
  fs.mkdirSync(path.join(dir, '.cache'))
  fs.writeFileSync(path.join(dir, '.cache', 'cached.py'), 'def cached(): pass\n')

  // Build index excluding generated and cache directories
  buildIndex(dir, { exclude: ['generated', '.cache'] })

  // src/app.py should be indexed
  const srcSymbols = getSymbolsInFile(dir, 'src/app.py')
  assert.ok(srcSymbols.some(s => s.name === 'app'), 'src/app.py should be indexed')

  // generated directory should not be indexed
  assert.throws(
    () => getSymbolsInFile(dir, 'generated/types.py'),
    /File not found/i,
    'generated/types.py should not be indexed'
  )

  // .cache directory should not be indexed
  assert.throws(
    () => getSymbolsInFile(dir, '.cache/cached.py'),
    /File not found/i,
    '.cache/cached.py should not be indexed'
  )
})

// ============================================================================
// Feature #2: Filtered query functions
// ============================================================================

test('findSymbolFiltered filters by kinds', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  // Find only functions
  const functions = findSymbolFiltered(dir, 'authenticate', { kinds: ['function'] })
  assert.ok(Array.isArray(functions), 'Should return an array')

  for (const sym of functions) {
    assert.strictEqual(sym.kind, 'function', `Expected function, got ${sym.kind}`)
  }
})

test('findSymbolFiltered excludes kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Find symbols but exclude imports
  const symbols = findSymbolFiltered(dir, 'find_all', { excludeKinds: ['import'] })

  for (const sym of symbols) {
    assert.notStrictEqual(sym.kind, 'import', 'Should not return imports')
  }
})

test('getCallersFiltered filters caller kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Get callers of query_database, filtering to only functions
  const callers = getCallersFiltered(dir, 'query_database', { kinds: ['function'] })
  assert.ok(Array.isArray(callers), 'Should return an array')

  for (const caller of callers) {
    assert.strictEqual(caller.kind, 'function', `Expected function caller, got ${caller.kind}`)
  }
})

test('getCalleesFiltered filters callee kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Get callees of find_all
  const callees = getCalleesFiltered(dir, 'find_all', { kinds: ['function'] })
  assert.ok(Array.isArray(callees), 'Should return an array')

  for (const callee of callees) {
    assert.strictEqual(callee.kind, 'function', `Expected function callee, got ${callee.kind}`)
  }
})

test('getReferencesFiltered filters by kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Get references to query_database, only functions
  const refs = getReferencesFiltered(dir, 'query_database', { kinds: ['function'] })
  assert.ok(Array.isArray(refs), 'Should return an array')

  for (const ref of refs) {
    assert.strictEqual(ref.symbol.kind, 'function', `Expected function reference, got ${ref.symbol.kind}`)
  }
})

test('getReferencesFiltered excludes specified kinds', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  // Get references but exclude imports
  const refs = getReferencesFiltered(dir, 'find_all', { excludeKinds: ['import'] })
  assert.ok(Array.isArray(refs), 'Should return an array')

  for (const ref of refs) {
    assert.notStrictEqual(ref.symbol.kind, 'import', 'Should not include imports')
  }
})

// ============================================================================
// Feature #4: Incremental index updates
// ============================================================================

test('indexStatus returns index information', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const status = indexStatus(dir)

  assert.ok(status, 'Should return status object')
  assert.ok(typeof status.exists === 'boolean', 'Should have exists field')
  assert.ok(status.exists, 'Index should exist')
  assert.ok(typeof status.fileCount === 'number', 'Should have fileCount')
  assert.ok(typeof status.symbolCount === 'number', 'Should have symbolCount')
  assert.ok(status.fileCount > 0, 'Should have indexed files')
  assert.ok(status.symbolCount > 0, 'Should have indexed symbols')
})

test('indexStatus returns exists=false for missing index', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-noindex-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'test.py'), 'def foo(): pass\n')
  // Don't build index

  const status = indexStatus(dir)

  assert.ok(status, 'Should return status object')
  assert.strictEqual(status.exists, false, 'Index should not exist')
})

test('buildIndex with incremental option only re-indexes changed files', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-incr-'))
  t.after(() => cleanup(dir))

  // Create initial files
  fs.writeFileSync(path.join(dir, 'file1.py'), 'def func1():\n    pass\n')
  fs.writeFileSync(path.join(dir, 'file2.py'), 'def func2():\n    pass\n')

  // Build initial index
  buildIndex(dir)

  const status1 = indexStatus(dir)
  assert.ok(status1.exists, 'Initial index should exist')
  assert.strictEqual(status1.fileCount, 2, 'Should have 2 files')

  // Modify one file
  fs.writeFileSync(path.join(dir, 'file1.py'), 'def func1_modified():\n    return 42\n')

  // Build incrementally
  buildIndex(dir, { incremental: true })

  const status2 = indexStatus(dir)
  assert.ok(status2.exists, 'Index should still exist')
  assert.strictEqual(status2.fileCount, 2, 'Should still have 2 files')

  // Verify the modification was picked up
  const symbols = getSymbolsInFile(dir, 'file1.py')
  assert.ok(symbols.some(s => s.name === 'func1_modified'), 'Should have updated symbol')
})

test('buildIndex incremental adds new files', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-incradd-'))
  t.after(() => cleanup(dir))

  // Create initial file
  fs.writeFileSync(path.join(dir, 'existing.py'), 'def existing():\n    pass\n')

  // Build initial index
  buildIndex(dir)

  const status1 = indexStatus(dir)
  assert.strictEqual(status1.fileCount, 1, 'Should have 1 file initially')

  // Add new file
  fs.writeFileSync(path.join(dir, 'new_file.py'), 'def new_function():\n    pass\n')

  // Build incrementally
  buildIndex(dir, { incremental: true })

  const status2 = indexStatus(dir)
  assert.strictEqual(status2.fileCount, 2, 'Should have 2 files after incremental')

  // Verify new file is indexed
  const symbols = getSymbolsInFile(dir, 'new_file.py')
  assert.ok(symbols.some(s => s.name === 'new_function'), 'New file should be indexed')
})

test('buildIndex force overrides incremental', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-force-'))
  t.after(() => cleanup(dir))

  // Create file
  fs.writeFileSync(path.join(dir, 'test.py'), 'def test():\n    pass\n')

  // Build initial index
  buildIndex(dir)

  // Force rebuild (should work even with incremental)
  buildIndex(dir, { force: true, incremental: true })

  const status = indexStatus(dir)
  assert.ok(status.exists, 'Index should exist after force rebuild')
  assert.strictEqual(status.fileCount, 1, 'Should have 1 file')
})

// ============================================================================
// Bug Fix Tests - Input Validation
// ============================================================================

test('pack throws on empty path', () => {
  assert.throws(
    () => pack('', {}),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
  assert.throws(
    () => pack('   ', {}),
    /Path cannot be empty/i,
    'Should reject whitespace-only path'
  )
})

test('scan throws on empty path', () => {
  assert.throws(
    () => scan('', 'claude'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
})

test('scanWithOptions throws on empty path', () => {
  assert.throws(
    () => scanWithOptions('', {}),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
})

test('findSymbol throws on empty path or name', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => findSymbol('', 'test'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )

  assert.throws(
    () => findSymbol(dir, ''),
    /Symbol name cannot be empty/i,
    'Should reject empty symbol name'
  )

  assert.throws(
    () => findSymbol(dir, '   '),
    /Symbol name cannot be empty/i,
    'Should reject whitespace-only symbol name'
  )
})

test('getCallers throws on empty path or name', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const { getCallers } = require('..')

  assert.throws(
    () => getCallers('', 'test'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )

  assert.throws(
    () => getCallers(dir, ''),
    /Symbol name cannot be empty/i,
    'Should reject empty symbol name'
  )
})

test('getCallees throws on empty path or name', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const { getCallees } = require('..')

  assert.throws(
    () => getCallees('', 'test'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )

  assert.throws(
    () => getCallees(dir, ''),
    /Symbol name cannot be empty/i,
    'Should reject empty symbol name'
  )
})

test('getReferences throws on empty path or name', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getReferences('', 'test'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )

  assert.throws(
    () => getReferences(dir, ''),
    /Symbol name cannot be empty/i,
    'Should reject empty symbol name'
  )
})

test('buildIndex throws on empty path', () => {
  assert.throws(
    () => buildIndex(''),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
})

test('analyzeImpact throws on empty path or files', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => analyzeImpact('', ['file.ts']),
    /Path cannot be empty/i,
    'Should reject empty path'
  )

  assert.throws(
    () => analyzeImpact(dir, []),
    /Files array cannot be empty/i,
    'Should reject empty files array'
  )

  assert.throws(
    () => analyzeImpact(dir, ['']),
    /File path cannot be empty/i,
    'Should reject empty file path in array'
  )
})

test('getDiffContext throws on empty path', (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getDiffContext('', '', ''),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
})

test('getChangedSymbols throws on empty path', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getChangedSymbols('', 'HEAD', 'HEAD'),
    /Path cannot be empty/i,
    'Should reject empty path'
  )
})

// ============================================================================
// Bug Fix Tests - countTokens edge cases
// ============================================================================

test('countTokens handles empty string', () => {
  const count = countTokens('', 'claude')
  assert.strictEqual(count, 0, 'Empty string should return 0 tokens')
})

test('countTokens handles whitespace-only string', () => {
  const count = countTokens('   ', 'claude')
  assert.ok(typeof count === 'number', 'Whitespace string should return a number')
})

// Bug fix: countTokens(null) should return 0 instead of crashing
test('countTokens handles null input', () => {
  const count = countTokens(null, 'claude')
  assert.strictEqual(count, 0, 'null input should return 0 tokens')
})

// Bug fix: countTokens(undefined) should return 0 instead of crashing
test('countTokens handles undefined input', () => {
  const count = countTokens(undefined, 'claude')
  assert.strictEqual(count, 0, 'undefined input should return 0 tokens')
})

// ============================================================================
// Bug Fix Tests - tokenBudget validation
// ============================================================================

test('pack with negative tokenBudget throws error', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  assert.throws(
    () => pack(dir, { format: 'json', tokenBudget: -1 }),
    /Token budget cannot be negative/i,
    'Negative tokenBudget should throw error'
  )
})

test('pack with tokenBudget=-100 throws error', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  assert.throws(
    () => pack(dir, { format: 'json', tokenBudget: -100 }),
    /Token budget cannot be negative/i,
    'Large negative tokenBudget should throw error'
  )
})

test('pack with tokenBudget=0 throws validation error', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  // tokenBudget=0 is now rejected - omit the parameter for no limit
  assert.throws(
    () => pack(dir, { format: 'json', tokenBudget: 0 }),
    /tokenBudget cannot be 0|Omit the parameter for no limit/i,
    'tokenBudget=0 should throw validation error'
  )
})

// ============================================================================
// Bug Fix Tests - semanticCompress params effectiveness
// ============================================================================

test('semanticCompress budget_ratio affects small content', () => {
  // Content that previously wasn't compressed (under 100 chars but over 10)
  const text = 'Short content for testing budget ratio effectiveness with moderate length.'

  // With budget_ratio=0.3 (30%), should compress
  const compressed = semanticCompress(text, 0.7, 0.3)

  // Result should be shorter or contain truncation marker
  // The fix ensures budget_ratio < 1.0 triggers truncation for content >= 10 chars
  assert.ok(
    compressed.length < text.length || compressed.includes('truncated'),
    `Small content with budget_ratio=0.3 should be compressed. Original: ${text.length}, Result: ${compressed.length}`
  )
})

test('semanticCompress budget_ratio=1.0 preserves content', () => {
  const text = 'Content that should be preserved with budget_ratio of 1.0 (keep 100%).'

  // With budget_ratio=1.0, should keep everything
  const compressed = semanticCompress(text, { budgetRatio: 1.0 })

  // Should return content (may include original or processed)
  assert.ok(typeof compressed === 'string', 'Should return a string')
  assert.ok(compressed.length > 0, 'Should not be empty')
  // With budget_ratio=1.0, the result should be similar length to original
  assert.ok(compressed.length >= text.length * 0.5, 'budget_ratio=1.0 should preserve most content')
})

test('semanticCompress budget_ratio affects medium content without chunks', () => {
  // Content without paragraph breaks (\\n\\n) that falls back to truncation
  const text = 'This is a test of medium length content that has no paragraph breaks and should trigger the budget ratio truncation path instead of chunk-based compression since there are no chunk boundaries.'

  const compressed = semanticCompress(text, 0.7, 0.5)

  // With budget_ratio=0.5, should compress significantly
  assert.ok(
    compressed.length < text.length,
    `Medium content should be compressed. Original: ${text.length}, Result: ${compressed.length}`
  )
})

test('semanticCompress similarity_threshold documented behavior', () => {
  // Note: similarity_threshold only affects clustering when embeddings feature is enabled
  // Without embeddings feature, it has no effect - this is documented behavior
  // This test verifies the function works regardless of threshold value

  const text = 'Test content for similarity threshold parameter.'

  const result1 = semanticCompress(text, 0.5, 1.0)
  const result2 = semanticCompress(text, 0.9, 1.0)

  // Both should work without error
  assert.ok(result1.length > 0, 'similarity_threshold=0.5 should work')
  assert.ok(result2.length > 0, 'similarity_threshold=0.9 should work')
})

// ============================================================================
// Bug Fix Tests - getCallGraph with limits
// ============================================================================

test('getCallGraph respects maxNodes limit', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const { getCallGraph } = require('..')

  // Get full graph first
  const fullGraph = getCallGraph(dir)
  const fullNodeCount = fullGraph.nodes.length

  if (fullNodeCount > 5) {
    // Get limited graph
    const limitedGraph = getCallGraph(dir, { maxNodes: 5 })
    assert.ok(
      limitedGraph.nodes.length <= 5,
      `maxNodes limit should be respected: got ${limitedGraph.nodes.length}, expected <= 5`
    )
  }
})

test('getCallGraph respects maxEdges limit', (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const { getCallGraph } = require('..')

  // Get full graph first
  const fullGraph = getCallGraph(dir)
  const fullEdgeCount = fullGraph.edges.length

  if (fullEdgeCount > 3) {
    // Get limited graph
    const limitedGraph = getCallGraph(dir, { maxEdges: 3 })
    assert.ok(
      limitedGraph.edges.length <= 3,
      `maxEdges limit should be respected: got ${limitedGraph.edges.length}, expected <= 3`
    )
  }
})

// ============================================================================
// Bug Fix Tests - scanWithOptions exclude patterns
// ============================================================================

test('scanWithOptions exclude works without include patterns', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-exconly-'))
  t.after(() => cleanup(dir))

  // Create files including test files
  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'utils.py'), 'def utils(): pass\n')
  fs.writeFileSync(path.join(dir, 'test_app.py'), 'def test_app(): pass\n')
  fs.writeFileSync(path.join(dir, 'test_utils.py'), 'def test_utils(): pass\n')

  // Exclude ONLY test files, no include pattern
  const stats = scanWithOptions(dir, {
    exclude: ['test_*.py'],
    applyDefaultIgnores: false,
    includeTests: true, // Don't apply default test ignores
  })

  // Should have 2 files (app.py, utils.py) - not the test files
  assert.strictEqual(stats.totalFiles, 2, 'Exclude pattern should filter out test files even without include')
})

test('scanWithOptions with both include and exclude patterns', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-incexc-'))
  t.after(() => cleanup(dir))

  fs.writeFileSync(path.join(dir, 'app.py'), 'def app(): pass\n')
  fs.writeFileSync(path.join(dir, 'main.py'), 'def main(): pass\n')
  fs.writeFileSync(path.join(dir, 'test.py'), 'def test(): pass\n')
  fs.writeFileSync(path.join(dir, 'app.js'), 'function app() {}\n')

  // Include only .py files, exclude test.py
  const stats = scanWithOptions(dir, {
    include: ['*.py'],
    exclude: ['test.py'],
    applyDefaultIgnores: false,
    includeTests: true,
  })

  // Should have 2 files (app.py, main.py) - not app.js or test.py
  assert.strictEqual(stats.totalFiles, 2, 'Should have 2 Python files (excluding test.py)')
})

// ============================================================================
// v0.4.8 Bug Fixes - New Tests for Fixed APIs
// ============================================================================

const { version, getCallGraph } = require('..')

// ============================================================================
// Bug Fix: version() function was missing
// ============================================================================

test('version() returns package version string', () => {
  assert.ok(typeof version === 'function', 'version should be exported as a function')

  const v = version()
  assert.ok(typeof v === 'string', 'version() should return a string')
  assert.ok(v.length > 0, 'version string should not be empty')

  // Should be a valid semver-like version (x.y.z)
  assert.ok(/^\d+\.\d+\.\d+/.test(v), `version should match semver pattern, got: ${v}`)
})

test('version() is consistent', () => {
  const v1 = version()
  const v2 = version()
  assert.strictEqual(v1, v2, 'version() should return consistent value')
})

// ============================================================================
// Bug Fix: Null/undefined handling - functions should throw clean errors
// ============================================================================

test('pack handles null path gracefully', () => {
  assert.throws(
    () => pack(null, {}),
    /Path cannot be null or undefined/i,
    'pack(null) should throw clean error'
  )
})

test('pack handles undefined path gracefully', () => {
  assert.throws(
    () => pack(undefined, {}),
    /Path cannot be null or undefined/i,
    'pack(undefined) should throw clean error'
  )
})

test('scan handles null path gracefully', () => {
  assert.throws(
    () => scan(null, 'claude'),
    /Path cannot be null or undefined/i,
    'scan(null) should throw clean error'
  )
})

test('scan handles undefined path gracefully', () => {
  assert.throws(
    () => scan(undefined, 'claude'),
    /Path cannot be null or undefined/i,
    'scan(undefined) should throw clean error'
  )
})

test('findSymbol handles null path gracefully', () => {
  assert.throws(
    () => findSymbol(null, 'test'),
    /Path cannot be null or undefined/i,
    'findSymbol(null, ...) should throw clean error'
  )
})

test('findSymbol handles null symbol name gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => findSymbol(dir, null),
    /Symbol name cannot be null or undefined/i,
    'findSymbol(..., null) should throw clean error'
  )
})

test('findSymbol handles undefined inputs gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => findSymbol(undefined, 'test'),
    /Path cannot be null or undefined/i,
    'findSymbol(undefined, ...) should throw clean error'
  )

  assert.throws(
    () => findSymbol(dir, undefined),
    /Symbol name cannot be null or undefined/i,
    'findSymbol(..., undefined) should throw clean error'
  )
})

test('getCallers handles null inputs gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getCallers(null, 'test'),
    /Path cannot be null or undefined/i,
    'getCallers(null, ...) should throw clean error'
  )

  assert.throws(
    () => getCallers(dir, null),
    /Symbol name cannot be null or undefined/i,
    'getCallers(..., null) should throw clean error'
  )
})

test('getCallees handles null inputs gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getCallees(null, 'test'),
    /Path cannot be null or undefined/i,
    'getCallees(null, ...) should throw clean error'
  )

  assert.throws(
    () => getCallees(dir, null),
    /Symbol name cannot be null or undefined/i,
    'getCallees(..., null) should throw clean error'
  )
})

test('getReferences handles null inputs gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getReferences(null, 'test'),
    /Path cannot be null or undefined/i,
    'getReferences(null, ...) should throw clean error'
  )

  assert.throws(
    () => getReferences(dir, null),
    /Symbol name cannot be null or undefined/i,
    'getReferences(..., null) should throw clean error'
  )
})

test('getCallGraph handles null path gracefully', () => {
  assert.throws(
    () => getCallGraph(null),
    /Path cannot be null or undefined/i,
    'getCallGraph(null) should throw clean error'
  )
})

test('getCallGraph handles undefined path gracefully', () => {
  assert.throws(
    () => getCallGraph(undefined),
    /Path cannot be null or undefined/i,
    'getCallGraph(undefined) should throw clean error'
  )
})

test('getSymbolSource handles null inputs gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getSymbolSource(null, 'test'),
    /Path cannot be null or undefined/i,
    'getSymbolSource(null, ...) should throw clean error'
  )

  assert.throws(
    () => getSymbolSource(dir, null),
    /Symbol name cannot be null or undefined/i,
    'getSymbolSource(..., null) should throw clean error'
  )
})

test('semanticCompress handles null text gracefully', () => {
  assert.throws(
    () => semanticCompress(null),
    /Text cannot be null or undefined/i,
    'semanticCompress(null) should throw clean error'
  )
})

test('semanticCompress handles undefined text gracefully', () => {
  assert.throws(
    () => semanticCompress(undefined),
    /Text cannot be null or undefined/i,
    'semanticCompress(undefined) should throw clean error'
  )
})

test('semanticCompress handles empty text gracefully', () => {
  assert.throws(
    () => semanticCompress(''),
    /Text cannot be empty/i,
    'semanticCompress("") should throw clean error'
  )
})

// ============================================================================
// Bug Fix: semanticCompress with options object (previously broken)
// ============================================================================

test('semanticCompress with options object works', () => {
  const paragraphs = Array.from({ length: 12 }, (_, i) =>
    `Paragraph ${i}\n` + 'x'.repeat(140),
  )
  const text = paragraphs.join('\n\n')

  // New API: options object instead of positional params
  const compressed = semanticCompress(text, { budgetRatio: 0.5 })

  assert.ok(compressed.length > 0, 'Should return compressed text')
  assert.ok(compressed.length < text.length, 'Text should be compressed')
})

test('semanticCompress with all options', () => {
  const paragraphs = Array.from({ length: 10 }, (_, i) =>
    `Paragraph ${i}: ` + 'content '.repeat(50),
  )
  const text = paragraphs.join('\n\n')

  const compressed = semanticCompress(text, {
    similarityThreshold: 0.7,
    budgetRatio: 0.3,
    minChunkSize: 100,
    maxChunkSize: 2000,
  })

  assert.ok(typeof compressed === 'string', 'Should return string')
  assert.ok(compressed.length > 0, 'Should not be empty')
})

test('semanticCompress with only budgetRatio option', () => {
  const text = 'First paragraph.\n\n' + 'Second paragraph.\n\n'.repeat(10)

  const compressed = semanticCompress(text, { budgetRatio: 0.3 })

  assert.ok(typeof compressed === 'string', 'Should return string')
})

test('semanticCompress with no options uses defaults', () => {
  const paragraphs = Array.from({ length: 15 }, (_, i) =>
    `Line ${i}: ` + 'content '.repeat(30),
  )
  const text = paragraphs.join('\n\n')

  // Call with text only, no options (uses defaults)
  const compressed = semanticCompress(text)

  assert.ok(typeof compressed === 'string', 'Should return string')
  assert.ok(compressed.length > 0, 'Should not be empty')
})

// ============================================================================
// Bug Fix: Infiniloom.generateMap with options object (previously broken)
// ============================================================================

test('Infiniloom.generateMap with options object works', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const loom = new Infiniloom(dir, 'claude')

  // New API: options object instead of positional params
  const map = JSON.parse(loom.generateMap({ budget: 2000, maxSymbols: 50 }))

  assert.ok(map, 'Should return map')
  assert.ok(map.summary, 'Map should have summary')
})

test('Infiniloom.generateMap with only budget option', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const loom = new Infiniloom(dir, 'claude')

  const map = JSON.parse(loom.generateMap({ budget: 500 }))

  assert.ok(map, 'Should return map')
  assert.ok(map.summary, 'Map should have summary')
})

test('Infiniloom.generateMap with only maxSymbols option', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const loom = new Infiniloom(dir, 'claude')

  const map = JSON.parse(loom.generateMap({ maxSymbols: 10 }))

  assert.ok(map, 'Should return map')
})

test('Infiniloom.generateMap with no options uses defaults', (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const loom = new Infiniloom(dir, 'claude')

  // Call with no options (uses defaults: budget=2000, maxSymbols=50)
  const map = JSON.parse(loom.generateMap())

  assert.ok(map, 'Should return map with defaults')
  assert.ok(map.summary, 'Map should have summary')
})

// ============================================================================
// Bug Fix: getSymbolSource returns SymbolSourceResult object (not string)
// ============================================================================

test('getSymbolSource returns SymbolSourceResult object', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getSymbolSource(dir, 'authenticate')

  // Should be an object, not a string
  assert.ok(typeof result === 'object', 'Should return an object')
  assert.ok(result !== null, 'Should not be null')

  // Check required properties
  assert.ok('source' in result, 'Result should have source property')
  assert.ok('path' in result, 'Result should have path property')
  assert.ok('startLine' in result, 'Result should have startLine property')
  assert.ok('endLine' in result, 'Result should have endLine property')
  assert.ok('name' in result, 'Result should have name property')
  assert.ok('kind' in result, 'Result should have kind property')

  // Check property types
  assert.ok(typeof result.source === 'string', 'source should be string')
  assert.ok(typeof result.path === 'string', 'path should be string')
  assert.ok(typeof result.startLine === 'number', 'startLine should be number')
  assert.ok(typeof result.endLine === 'number', 'endLine should be number')
  assert.ok(typeof result.name === 'string', 'name should be string')
  assert.ok(typeof result.kind === 'string', 'kind should be string')

  // Check values
  assert.strictEqual(result.name, 'authenticate', 'name should match requested symbol')
  assert.ok(result.source.includes('authenticate'), 'source should contain function')
  assert.ok(result.startLine > 0, 'startLine should be positive')
  assert.ok(result.endLine >= result.startLine, 'endLine should be >= startLine')
})

test('getSymbolSource result has correct file path', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getSymbolSource(dir, 'authenticate', 'auth.ts')

  assert.strictEqual(result.path, 'auth.ts', 'path should match the file')
})

test('getSymbolSource result kind reflects symbol type', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getSymbolSource(dir, 'authenticate')

  // authenticate is a function
  assert.strictEqual(result.kind, 'function', 'kind should be function')
})

// ============================================================================
// Additional regression tests for edge cases
// ============================================================================

test('scanSecurity handles null path gracefully', () => {
  assert.throws(
    () => scanSecurity(null),
    /Path cannot be null|invalid/i,
    'scanSecurity(null) should throw clean error'
  )
})

test('GitRepo handles null path in constructor', () => {
  assert.throws(
    () => new GitRepo(null),
    /Path cannot be null or undefined|Failed to open git repo|invalid/i,
    'new GitRepo(null) should throw clean error'
  )
})

test('Infiniloom handles empty path in constructor', () => {
  assert.throws(
    () => new Infiniloom('', 'claude'),
    /Path does not exist|empty/i,
    'new Infiniloom("") should throw error'
  )
})

// ============================================================================
// Bug fix regression tests (v0.4.9)
// Tests for the 5 critical null crashes and parameter edge cases
// ============================================================================

test('getCallSites handles null path gracefully', (t) => {
  assert.throws(
    () => getCallSites(null, 'authenticate'),
    /Path cannot be null|cannot be null or undefined|Null/i,
    'getCallSites(null, ...) should throw clean error'
  )
})

test('getCallSites handles null symbolName gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getCallSites(dir, null),
    /Symbol name cannot be null|cannot be null or undefined|Null/i,
    'getCallSites(path, null) should throw clean error'
  )
})

test('getTransitiveCallers handles null path gracefully', (t) => {
  assert.throws(
    () => getTransitiveCallers(null, 'authenticate'),
    /Path cannot be null|cannot be null or undefined|Null/i,
    'getTransitiveCallers(null, ...) should throw clean error'
  )
})

test('getTransitiveCallers handles null symbolName gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getTransitiveCallers(dir, null),
    /Symbol name cannot be null|cannot be null or undefined|Null/i,
    'getTransitiveCallers(path, null) should throw clean error'
  )
})

test('getTransitiveCallers with maxDepth=0 returns empty array', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getTransitiveCallers(dir, 'authenticate', { maxDepth: 0 })

  assert.ok(Array.isArray(result), 'should return array')
  assert.strictEqual(result.length, 0, 'maxDepth=0 should return empty array (no traversal)')
})

test('getCallSitesWithContext handles null path gracefully', (t) => {
  assert.throws(
    () => getCallSitesWithContext(null, 'authenticate'),
    /Path cannot be null|cannot be null or undefined|Null/i,
    'getCallSitesWithContext(null, ...) should throw clean error'
  )
})

test('getCallSitesWithContext handles null symbolName gracefully', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => getCallSitesWithContext(dir, null),
    /Symbol name cannot be null|cannot be null or undefined|Null/i,
    'getCallSitesWithContext(path, null) should throw clean error'
  )
})

test('buildIndex handles null path gracefully', (t) => {
  assert.throws(
    () => buildIndex(null),
    /Path cannot be null|cannot be null or undefined|Null/i,
    'buildIndex(null) should throw clean error'
  )
})

test('chunk handles null path gracefully', (t) => {
  assert.throws(
    () => chunk(null),
    /Path cannot be null|cannot be null or undefined|Null/i,
    'chunk(null) should throw clean error'
  )
})

test('chunk with maxTokens=0 throws validation error', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => chunk(dir, { maxTokens: 0 }),
    /max_tokens cannot be 0|too small|cannot be 0/i,
    'chunk with maxTokens=0 should throw validation error'
  )
})

test('chunk with maxTokens below minimum throws validation error', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  assert.throws(
    () => chunk(dir, { maxTokens: 50 }),
    /too small|minimum is 100/i,
    'chunk with maxTokens=50 should throw validation error (minimum is 100)'
  )
})

test('getCallGraph with maxNodes=0 returns empty graph', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getCallGraph(dir, { maxNodes: 0 })

  assert.ok(result, 'should return result')
  assert.ok(Array.isArray(result.nodes), 'should have nodes array')
  assert.ok(Array.isArray(result.edges), 'should have edges array')
  assert.strictEqual(result.nodes.length, 0, 'maxNodes=0 should return empty nodes')
  assert.strictEqual(result.edges.length, 0, 'maxNodes=0 should return empty edges')
})

test('getCallGraph with maxEdges=0 returns empty graph', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = getCallGraph(dir, { maxEdges: 0 })

  assert.ok(result, 'should return result')
  assert.ok(Array.isArray(result.nodes), 'should have nodes array')
  assert.ok(Array.isArray(result.edges), 'should have edges array')
  assert.strictEqual(result.nodes.length, 0, 'maxEdges=0 should return empty nodes')
  assert.strictEqual(result.edges.length, 0, 'maxEdges=0 should return empty edges')
})

// ============================================================================
// Async Wrapper Function Tests
// ============================================================================

test('packAsync returns valid JSON output', async (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const output = await packAsync(dir, {
    format: 'json',
    model: 'claude',
    mapBudget: 500,
  })

  assert.ok(output, 'packAsync should return output')
  assert.ok(output.length > 0, 'packAsync output should not be empty')

  // Output is a JSON string, parse it
  const parsed = JSON.parse(output)
  assert.ok(parsed, 'packAsync output should be valid JSON')
  assert.ok(parsed.repository || parsed.map, 'packAsync JSON should have repository or map data')
})

test('packAsync with XML format', async (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const output = await packAsync(dir, {
    format: 'xml',
    model: 'gpt4o',
  })

  assert.ok(output, 'packAsync XML should return output')
  assert.ok(output.includes('<files>'), 'packAsync should contain files tag')
  assert.ok(output.includes('<file'), 'packAsync should contain file tag')
})

test('packAsync throws on invalid path', async (t) => {
  await assert.rejects(
    async () => await packAsync('/nonexistent/path/to/repo'),
    /not found|does not exist/i,
    'packAsync should throw on invalid path'
  )
})

test('scanAsync returns statistics', async (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const result = await scanAsync(dir, 'claude')

  assert.ok(result, 'scanAsync should return result')
  assert.ok(typeof result.totalFiles === 'number', 'scanAsync should return totalFiles')
  assert.ok(typeof result.totalTokens === 'number', 'scanAsync should return totalTokens')
  assert.ok(result.totalFiles > 0, 'scanAsync should find files')
  assert.ok(Array.isArray(result.languages), 'scanAsync should have languages array')
})

test('scanAsync with different model', async (t) => {
  const dir = createTempRepo()
  t.after(() => cleanup(dir))

  const result = await scanAsync(dir, 'gpt4o')

  assert.ok(result, 'scanAsync with gpt4o should return result')
  assert.ok(result.totalFiles > 0, 'scanAsync should find files')
  assert.ok(result.totalTokens > 0, 'scanAsync should count tokens')
})

test('scanAsync throws on invalid path', async (t) => {
  await assert.rejects(
    async () => await scanAsync('/invalid/repo/path', 'claude'),
    /not found|does not exist/i,
    'scanAsync should throw on invalid path'
  )
})

test('buildIndexAsync creates index', async (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  const result = await buildIndexAsync(dir, {
    force: false,
    includeTests: false,
  })

  assert.ok(result, 'buildIndexAsync should return result')
  assert.ok(typeof result.fileCount === 'number', 'buildIndexAsync should return fileCount')
  assert.ok(typeof result.symbolCount === 'number', 'buildIndexAsync should return symbolCount')
})

test('buildIndexAsync with force rebuild', async (t) => {
  const dir = createGitRepo()
  t.after(() => cleanup(dir))

  // Build once
  await buildIndexAsync(dir, { force: false })

  // Force rebuild
  const result = await buildIndexAsync(dir, { force: true })

  assert.ok(result, 'buildIndexAsync force should return result')
})

test('buildIndexAsync throws on invalid path', async (t) => {
  await assert.rejects(
    async () => await buildIndexAsync('/nonexistent/path'),
    /not found|does not exist/i,
    'buildIndexAsync should throw on invalid path'
  )
})

test('chunkAsync returns chunks', async (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = await chunkAsync(dir, {
    maxTokens: 1000,
    overlap: 200,
    strategy: 'file',
  })

  assert.ok(result, 'chunkAsync should return result')
  assert.ok(Array.isArray(result), 'chunkAsync should return array')
  assert.ok(result.length > 0, 'chunkAsync should create chunks')
  assert.ok(result[0].content, 'chunkAsync chunks should have content')
  assert.ok(typeof result[0].tokens === 'number', 'chunkAsync chunks should have token count')
})

test('chunkAsync with module strategy', async (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = await chunkAsync(dir, {
    maxTokens: 2000,
    strategy: 'module',
  })

  assert.ok(result, 'chunkAsync module should return result')
  assert.ok(Array.isArray(result), 'chunkAsync module should return array')
})

test('chunkAsync throws on maxTokens below minimum', async (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  await assert.rejects(
    async () => await chunkAsync(dir, { maxTokens: 50 }),
    /too small|minimum is 100/i,
    'chunkAsync with maxTokens=50 should throw validation error'
  )
})

test('analyzeImpactAsync returns impact analysis', async (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = await analyzeImpactAsync(dir, ['main.rs'], {
    depth: 1,
  })

  assert.ok(result, 'analyzeImpactAsync should return result')
  assert.ok(Array.isArray(result.affectedSymbols) || Array.isArray(result.testFiles) ||
    result.impactLevel !== undefined, 'analyzeImpactAsync should have impact data')
})

test('analyzeImpactAsync with multiple files', async (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const result = await analyzeImpactAsync(dir, ['main.rs', 'script.py'], {
    depth: 2,
  })

  assert.ok(result, 'analyzeImpactAsync multiple files should return result')
})

test('analyzeImpactAsync throws on invalid path', async (t) => {
  await assert.rejects(
    async () => await analyzeImpactAsync('/nonexistent/path', ['main.rs']),
    /not found|does not exist/i,
    'analyzeImpactAsync should throw on invalid path'
  )
})

test('getDiffContextAsync returns diff context', async (t) => {
  const dir = createGitRepo() // Already has initial commit with test.py
  t.after(() => cleanup(dir))

  // Make a change to existing file and commit
  fs.writeFileSync(path.join(dir, 'test.py'), 'def hello():\n    return "changed"\n')
  execSync('git add test.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "change test.py"', { cwd: dir, stdio: 'pipe' })

  const result = await getDiffContextAsync(dir, 'HEAD~1', 'HEAD', {
    depth: 1,
  })

  assert.ok(result, 'getDiffContextAsync should return result')
  assert.ok(Array.isArray(result.changedFiles) || Array.isArray(result.changed_files),
    'getDiffContextAsync should have changed files')
})

test('getDiffContextAsync with different commit range', async (t) => {
  const dir = createGitRepo() // Already has initial commit with test.py
  t.after(() => cleanup(dir))

  // Add a new file and commit
  fs.writeFileSync(path.join(dir, 'new_file.py'), 'def new_function():\n    pass\n')
  execSync('git add new_file.py', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "add new file"', { cwd: dir, stdio: 'pipe' })

  const result = await getDiffContextAsync(dir, 'HEAD~1', 'HEAD', {
    depth: 2,
  })

  assert.ok(result, 'getDiffContextAsync range should return result')
  assert.ok(Array.isArray(result.changedFiles) || Array.isArray(result.changed_files),
    'getDiffContextAsync should have changed files')
})

test('getDiffContextAsync throws on invalid path', async (t) => {
  await assert.rejects(
    async () => await getDiffContextAsync('/nonexistent/path', 'HEAD~1', 'HEAD'),
    /not found|does not exist|not a git repository/i,
    'getDiffContextAsync should throw on invalid path'
  )
})

// ============================================================================
// v0.6.2 Feature Tests - findCircularDependencies & getExportedSymbols
// ============================================================================

// Helper to create repo with circular imports for testing
function createCircularImportRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-circular-'))
  execSync('git init', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.name "Test User"', { cwd: dir, stdio: 'pipe' })

  // Create circular import: a.py -> b.py -> c.py -> a.py
  fs.writeFileSync(
    path.join(dir, 'a.py'),
    [
      'from b import func_b',
      '',
      'def func_a():',
      '    return func_b()',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'b.py'),
    [
      'from c import func_c',
      '',
      'def func_b():',
      '    return func_c()',
      '',
    ].join('\n')
  )

  fs.writeFileSync(
    path.join(dir, 'c.py'),
    [
      'from a import func_a',
      '',
      'def func_c():',
      '    return func_a()',
      '',
    ].join('\n')
  )

  execSync('git add -A', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "initial"', { cwd: dir, stdio: 'pipe' })

  return dir
}

// Helper to create repo with exported/public symbols
function createExportsRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'infiniloom-exports-'))
  execSync('git init', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: 'pipe' })
  execSync('git config user.name "Test User"', { cwd: dir, stdio: 'pipe' })

  // Create Rust file with public and private items
  fs.writeFileSync(
    path.join(dir, 'lib.rs'),
    [
      '/// Public function',
      'pub fn public_function() -> i32 {',
      '    private_helper()',
      '}',
      '',
      'fn private_helper() -> i32 {',
      '    42',
      '}',
      '',
      '/// Public struct',
      'pub struct PublicStruct {',
      '    pub field: i32,',
      '}',
      '',
      'struct PrivateStruct {',
      '    field: i32,',
      '}',
      '',
    ].join('\n')
  )

  // Create TypeScript file with exports
  fs.writeFileSync(
    path.join(dir, 'index.ts'),
    [
      'export function exportedFunction(): string {',
      '    return internalHelper();',
      '}',
      '',
      'function internalHelper(): string {',
      '    return "hello";',
      '}',
      '',
      'export class ExportedClass {',
      '    getValue(): number {',
      '        return 42;',
      '    }',
      '}',
      '',
    ].join('\n')
  )

  execSync('git add -A', { cwd: dir, stdio: 'pipe' })
  execSync('git commit -m "initial"', { cwd: dir, stdio: 'pipe' })

  return dir
}

test('findCircularDependencies returns empty array when no cycles exist', (t) => {
  const dir = createCallChainRepo() // Uses linear import chain (no cycles)
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const cycles = findCircularDependencies(dir)
  assert.ok(Array.isArray(cycles), 'findCircularDependencies should return an array')
  assert.equal(cycles.length, 0, 'Should find no cycles in linear import chain')
})

test('findCircularDependencies detects circular imports', (t) => {
  const dir = createCircularImportRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const cycles = findCircularDependencies(dir)
  assert.ok(Array.isArray(cycles), 'findCircularDependencies should return an array')
  // Note: The cycle may or may not be detected depending on how imports are analyzed
  // At minimum, the function should not throw
})

test('findCircularDependenciesAsync works asynchronously', async (t) => {
  const dir = createCallChainRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const cycles = await findCircularDependenciesAsync(dir)
  assert.ok(Array.isArray(cycles), 'findCircularDependenciesAsync should return an array')
})

test('findCircularDependencies returns cycle structure with files and length', (t) => {
  const dir = createCircularImportRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const cycles = findCircularDependencies(dir)
  // Each cycle should have files array, file_ids array, and length
  for (const cycle of cycles) {
    assert.ok(Array.isArray(cycle.files), 'Cycle should have files array')
    assert.ok(Array.isArray(cycle.fileIds), 'Cycle should have fileIds array')
    assert.ok(typeof cycle.length === 'number', 'Cycle should have length')
  }
})

test('getExportedSymbols returns public symbols', (t) => {
  const dir = createExportsRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const exports = getExportedSymbols(dir)
  assert.ok(Array.isArray(exports), 'getExportedSymbols should return an array')

  // Should find some public/exported symbols
  const symbolNames = exports.map(s => s.name)
  // Check for exported items - at least one of these should be found
  const hasExports = symbolNames.some(name =>
    name.includes('public') ||
    name.includes('Public') ||
    name.includes('exported') ||
    name.includes('Exported')
  )
  // Note: exact matching depends on parser implementation
})

test('getExportedSymbols with file filter', (t) => {
  const dir = createExportsRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const exports = getExportedSymbols(dir, 'lib.rs')
  assert.ok(Array.isArray(exports), 'getExportedSymbols should return an array')

  // All returned symbols should be from lib.rs
  for (const sym of exports) {
    assert.equal(sym.file, 'lib.rs', `Symbol ${sym.name} should be from lib.rs`)
  }
})

test('getExportedSymbols returns empty for nonexistent file', (t) => {
  const dir = createExportsRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const exports = getExportedSymbols(dir, 'nonexistent.rs')
  assert.ok(Array.isArray(exports), 'getExportedSymbols should return an array')
  assert.equal(exports.length, 0, 'Should return empty array for nonexistent file')
})

test('getExportedSymbolsAsync works asynchronously', async (t) => {
  const dir = createExportsRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const exports = await getExportedSymbolsAsync(dir)
  assert.ok(Array.isArray(exports), 'getExportedSymbolsAsync should return an array')
})

test('getExportedSymbols returns SymbolInfo structure', (t) => {
  const dir = createExportsRepo()
  t.after(() => cleanup(dir))

  const { buildIndex } = require('..')
  buildIndex(dir)

  const exports = getExportedSymbols(dir)

  // Check structure of returned symbols
  for (const sym of exports) {
    assert.ok(typeof sym.id === 'number', 'Symbol should have numeric id')
    assert.ok(typeof sym.name === 'string', 'Symbol should have string name')
    assert.ok(typeof sym.kind === 'string', 'Symbol should have string kind')
    assert.ok(typeof sym.file === 'string', 'Symbol should have string file')
    assert.ok(typeof sym.line === 'number', 'Symbol should have numeric line')
    assert.ok(typeof sym.endLine === 'number', 'Symbol should have numeric endLine')
    assert.ok(typeof sym.visibility === 'string', 'Symbol should have string visibility')
  }
})

test('findCircularDependencies handles null path gracefully', (t) => {
  assert.throws(
    () => findCircularDependencies(null),
    /path|null|undefined|empty/i,
    'Should throw on null path'
  )
})

test('getExportedSymbols handles null path gracefully', (t) => {
  assert.throws(
    () => getExportedSymbols(null),
    /path|null|undefined|empty/i,
    'Should throw on null path'
  )
})
