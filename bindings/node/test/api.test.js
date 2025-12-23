const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')

const { pack, scan, countTokens, semanticCompress, Infiniloom } = require('..')

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
