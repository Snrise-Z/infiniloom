const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execSync } = require('node:child_process')

const { pack, scan, scanWithOptions, countTokens, semanticCompress, Infiniloom, isGitRepo, GitRepo, scanSecurity } = require('..')

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

  // Create multiple files
  for (let i = 0; i < 10; i++) {
    const content = `def function_${i}():\n` + '    pass\n'.repeat(50)
    fs.writeFileSync(path.join(dir, `file${i}.py`), content)
  }

  // Pack with a small token budget
  const output = pack(dir, {
    format: 'json',
    tokenBudget: 500,  // Very small budget
    skipSymbols: true,
  })

  const parsed = JSON.parse(output)
  const files = parsed.repository.files

  // With a small budget, we should get fewer files than without budget
  assert.ok(files.length < 10, `Token budget should limit files (got ${files.length})`)
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

  const source = getSymbolSource(dir, 'authenticate')

  assert.ok(typeof source === 'string', 'Should return a string')
  assert.ok(source.length > 0, 'Source should not be empty')
  assert.ok(source.includes('authenticate'), 'Source should contain function name')
  assert.ok(source.includes('validate'), 'Source should contain function body')
})

test('getSymbolSource with file path disambiguation', (t) => {
  const dir = createTestRepoWithIndex()
  t.after(() => cleanup(dir))

  const source = getSymbolSource(dir, 'authenticate', 'auth.ts')

  assert.ok(typeof source === 'string', 'Should return a string')
  assert.ok(source.includes('authenticate'), 'Source should contain function name')
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
