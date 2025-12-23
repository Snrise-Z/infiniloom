// Example usage of @infiniloom/node

const { pack, scan, countTokens, Infiniloom } = require('./index');

// Example 1: Simple packing
console.log('=== Example 1: Simple Packing ===');
try {
  const context = pack('.', {
    format: 'xml',
    model: 'claude',
    compression: 'balanced',
    mapBudget: 1000,
    maxSymbols: 20
  });
  console.log('Packed context length:', context.length);
} catch (error) {
  console.error('Error:', error.message);
}

// Example 2: Repository scanning
console.log('\n=== Example 2: Repository Scanning ===');
try {
  const stats = scan('.', 'claude');
  console.log(`Repository: ${stats.name}`);
  console.log(`Total files: ${stats.totalFiles}`);
  console.log(`Total lines: ${stats.totalLines}`);
  console.log(`Total tokens: ${stats.totalTokens}`);
  console.log(`Primary language: ${stats.primaryLanguage || 'N/A'}`);
  console.log(`Languages:`, stats.languages);
  console.log(`Security findings: ${stats.securityFindings}`);
} catch (error) {
  console.error('Error:', error.message);
}

// Example 3: Token counting
console.log('\n=== Example 3: Token Counting ===');
const text = 'Hello, world! This is a test of the token counting functionality.';
console.log(`Text: "${text}"`);
console.log(`Tokens (claude): ${countTokens(text, 'claude')}`);
console.log(`Tokens (gpt-4o): ${countTokens(text, 'gpt-4o')}`);
console.log(`Tokens (gemini): ${countTokens(text, 'gemini')}`);

// Example 4: Advanced usage with Infiniloom class
console.log('\n=== Example 4: Infiniloom Class ===');
try {
  const loom = new Infiniloom('.', 'claude');

  // Get statistics
  const stats = loom.getStats();
  console.log(`Stats:`, stats);

  // Generate map
  const map = loom.generateMap(1000, 20);
  console.log(`Map generated, length: ${map.length}`);

  // Pack with options
  const context = loom.pack({
    format: 'markdown',
    compression: 'minimal'
  });
  console.log(`Packed context length: ${context.length}`);

  // Security scan
  const findings = loom.securityScan();
  if (findings.length > 0) {
    console.warn('Security issues found:');
    findings.forEach(finding => console.warn(`  - ${finding}`));
  } else {
    console.log('No security issues found');
  }
} catch (error) {
  console.error('Error:', error.message);
}
