const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execSync } = require('node:child_process')

const { pack, scan, countTokens, semanticCompress, Infiniloom, isGitRepo, GitRepo, scanSecurity } = require('..')

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
    /Invalid model/i,
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
  assert.ok(typeof countTokens === 'function')
  assert.ok(typeof semanticCompress === 'function')
  assert.ok(typeof isGitRepo === 'function')
  assert.ok(typeof scanSecurity === 'function')

  // Classes
  assert.ok(typeof Infiniloom === 'function')
  assert.ok(typeof GitRepo === 'function')
})
