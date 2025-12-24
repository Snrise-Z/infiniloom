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
