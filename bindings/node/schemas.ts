/**
 * Zod schemas for runtime validation of infiniloom-node options
 *
 * These schemas provide runtime validation with helpful error messages.
 * Use them when accepting options from external sources (API calls, config files, etc.)
 *
 * Usage:
 * ```typescript
 * import { PackOptionsSchema, validatePackOptions } from 'infiniloom-node/schemas';
 * import { pack } from 'infiniloom-node';
 *
 * // Validate options from external source
 * const result = PackOptionsSchema.safeParse(userInput);
 * if (!result.success) {
 *   console.error('Invalid options:', result.error.format());
 *   return;
 * }
 *
 * // Or use the helper function
 * const options = validatePackOptions(userInput);
 * const output = pack('./repo', options);
 * ```
 */

import { z } from 'zod';

// ============================================================================
// Enum Schemas
// ============================================================================

/**
 * Output format enum schema
 */
export const OutputFormatSchema = z.enum([
  'xml',
  'markdown',
  'json',
  'yaml',
  'toon',
  'plain',
]);
export type OutputFormat = z.infer<typeof OutputFormatSchema>;

/**
 * Tokenizer model enum schema
 */
export const TokenizerModelSchema = z.enum([
  // OpenAI models (exact tokenization via tiktoken)
  'gpt-5.2',
  'gpt-5.2-pro',
  'gpt-5.1',
  'gpt-5.1-mini',
  'gpt-5.1-codex',
  'gpt-5',
  'gpt-5-mini',
  'gpt-5-nano',
  'o4-mini',
  'o3',
  'o3-mini',
  'o1',
  'o1-mini',
  'o1-preview',
  'gpt-4o',
  'gpt-4o-mini',
  'gpt-4',
  'gpt-4-turbo',
  'gpt-3.5-turbo',
  // Anthropic
  'claude',
  // Google
  'gemini',
  'gemini-1.5',
  'gemini-2.0',
  'gemini-3.1',
  // Meta
  'llama',
  'llama-3',
  'llama-3.1',
  'llama-3.2',
  'codellama',
  // Mistral
  'mistral',
  'mixtral',
  // DeepSeek
  'deepseek',
  'deepseek-v3',
  // Alibaba
  'qwen',
  'qwen-2.5',
  // Cohere
  'cohere',
  'command-r',
  // xAI
  'grok',
]);
export type TokenizerModel = z.infer<typeof TokenizerModelSchema>;

/**
 * Compression level enum schema
 */
export const CompressionLevelSchema = z.enum([
  'none',
  'minimal',
  'balanced',
  'aggressive',
  'extreme',
  'focused',
  'semantic',
]);
export type CompressionLevel = z.infer<typeof CompressionLevelSchema>;

/**
 * Security severity enum schema
 */
export const SecuritySeveritySchema = z.enum([
  'critical',
  'high',
  'medium',
  'low',
  'info',
]);
export type SecuritySeverity = z.infer<typeof SecuritySeveritySchema>;

/**
 * Symbol kind enum schema
 */
export const SymbolKindSchema = z.enum([
  'function',
  'method',
  'class',
  'struct',
  'interface',
  'trait',
  'enum',
  'constant',
  'variable',
  'import',
  'export',
  'type',
  'module',
  'macro',
]);
export type SymbolKind = z.infer<typeof SymbolKindSchema>;

/**
 * Visibility enum schema
 */
export const VisibilitySchema = z.enum([
  'public',
  'private',
  'protected',
  'internal',
]);
export type Visibility = z.infer<typeof VisibilitySchema>;

/**
 * Chunk strategy enum schema
 */
export const ChunkStrategySchema = z.enum([
  'fixed',
  'file',
  'module',
  'symbol',
  'semantic',
  'dependency',
]);
export type ChunkStrategy = z.infer<typeof ChunkStrategySchema>;

/**
 * Impact level enum schema
 */
export const ImpactLevelSchema = z.enum([
  'low',
  'medium',
  'high',
  'critical',
]);
export type ImpactLevel = z.infer<typeof ImpactLevelSchema>;

/**
 * Git status enum schema
 */
export const GitStatusSchema = z.enum([
  'Added',
  'Modified',
  'Deleted',
  'Renamed',
  'Copied',
  'Unknown',
]);
export type GitStatus = z.infer<typeof GitStatusSchema>;

/**
 * Change type enum schema
 */
export const ChangeTypeSchema = z.enum([
  'added',
  'modified',
  'deleted',
]);
export type ChangeType = z.infer<typeof ChangeTypeSchema>;

/**
 * Breaking change severity enum schema
 */
export const ChangeSeveritySchema = z.enum([
  'critical',
  'high',
  'medium',
  'low',
]);
export type ChangeSeverity = z.infer<typeof ChangeSeveritySchema>;

// ============================================================================
// Option Schemas
// ============================================================================

/**
 * Pack options schema with full validation
 */
export const PackOptionsSchema = z.object({
  format: OutputFormatSchema.optional(),
  model: TokenizerModelSchema.optional(),
  compression: CompressionLevelSchema.optional(),
  mapBudget: z.number().int().nonnegative().max(10_000_000).optional(),
  maxSymbols: z.number().int().positive().max(10_000).optional(),
  skipSecurity: z.boolean().optional(),
  redactSecrets: z.boolean().optional(),
  skipSymbols: z.boolean().optional(),
  include: z.array(z.string().max(256)).max(100).optional(),
  exclude: z.array(z.string().max(256)).max(100).optional(),
  includeTests: z.boolean().optional(),
  securityThreshold: SecuritySeveritySchema.optional(),
  tokenBudget: z.number().int().nonnegative().max(10_000_000).optional(),
  changedOnly: z.boolean().optional(),
  baseSha: z.string().regex(/^[a-f0-9]{7,40}$/i).optional(),
  headSha: z.string().regex(/^[a-f0-9]{7,40}$/i).optional(),
  stagedOnly: z.boolean().optional(),
  includeRelated: z.boolean().optional(),
  relatedDepth: z.union([z.literal(1), z.literal(2), z.literal(3)]).optional(),
}).strict();
export type PackOptions = z.infer<typeof PackOptionsSchema>;

/**
 * Scan options schema
 */
export const ScanOptionsSchema = z.object({
  model: TokenizerModelSchema.optional(),
  include: z.array(z.string()).optional(),
  exclude: z.array(z.string()).optional(),
  includeTests: z.boolean().optional(),
  applyDefaultIgnores: z.boolean().optional(),
}).strict();
export type ScanOptions = z.infer<typeof ScanOptionsSchema>;

/**
 * Chunk options schema
 */
export const ChunkOptionsSchema = z.object({
  strategy: ChunkStrategySchema.optional(),
  maxTokens: z.number().int().positive().max(100_000).optional(),
  overlap: z.number().int().nonnegative().max(10_000).optional(),
  model: TokenizerModelSchema.optional(),
  format: OutputFormatSchema.optional(),
  priorityFirst: z.boolean().optional(),
  exclude: z.array(z.string().max(256)).max(100).optional(),
}).strict();
export type ChunkOptions = z.infer<typeof ChunkOptionsSchema>;

/**
 * Index options schema
 */
export const IndexOptionsSchema = z.object({
  force: z.boolean().optional(),
  includeTests: z.boolean().optional(),
  maxFileSize: z.number().int().positive().max(100 * 1024 * 1024).optional(), // 100MB max
  exclude: z.array(z.string().max(256)).max(100).optional(),
  incremental: z.boolean().optional(),
}).strict();
export type IndexOptions = z.infer<typeof IndexOptionsSchema>;

/**
 * Diff context options schema
 */
export const DiffContextOptionsSchema = z.object({
  depth: z.union([z.literal(1), z.literal(2), z.literal(3)]).optional(),
  budget: z.number().int().positive().max(10_000_000).optional(),
  includeDiff: z.boolean().optional(),
  format: OutputFormatSchema.optional(),
  model: TokenizerModelSchema.optional(),
  exclude: z.array(z.string().max(256)).max(100).optional(),
  include: z.array(z.string().max(256)).max(100).optional(),
}).strict();
export type DiffContextOptions = z.infer<typeof DiffContextOptionsSchema>;

/**
 * Impact options schema
 */
export const ImpactOptionsSchema = z.object({
  depth: z.union([z.literal(1), z.literal(2), z.literal(3)]).optional(),
  includeTests: z.boolean().optional(),
  model: TokenizerModelSchema.optional(),
  exclude: z.array(z.string().max(256)).max(100).optional(),
  include: z.array(z.string().max(256)).max(100).optional(),
}).strict();
export type ImpactOptions = z.infer<typeof ImpactOptionsSchema>;

/**
 * Embed options schema
 */
export const EmbedOptionsSchema = z.object({
  maxTokens: z.number().int().positive().max(100_000).optional(),
  minTokens: z.number().int().nonnegative().max(10_000).optional(),
  contextLines: z.number().int().nonnegative().max(1_000).optional(),
  includeImports: z.boolean().optional(),
  includeTopLevel: z.boolean().optional(),
  includeTests: z.boolean().optional(),
  securityScan: z.boolean().optional(),
  includePatterns: z.array(z.string().max(256)).max(100).optional(),
  excludePatterns: z.array(z.string().max(256)).max(100).optional(),
  manifestPath: z.string().max(4096).refine(p => !p.includes('..'), { message: 'Path traversal not allowed' }).optional(),
  diffOnly: z.boolean().optional(),
}).strict();
export type EmbedOptions = z.infer<typeof EmbedOptionsSchema>;

/**
 * Query filter schema
 */
export const QueryFilterSchema = z.object({
  kinds: z.array(SymbolKindSchema).optional(),
  excludeKinds: z.array(SymbolKindSchema).optional(),
}).strict();
export type QueryFilter = z.infer<typeof QueryFilterSchema>;

/**
 * Symbol filter schema
 */
export const SymbolFilterSchema = z.object({
  kind: SymbolKindSchema.optional(),
  visibility: VisibilitySchema.optional(),
}).strict();
export type SymbolFilter = z.infer<typeof SymbolFilterSchema>;

/**
 * Call graph options schema
 */
export const CallGraphOptionsSchema = z.object({
  maxNodes: z.number().int().positive().max(100_000).optional(),
  maxEdges: z.number().int().positive().max(100_000).optional(),
}).strict();
export type CallGraphOptions = z.infer<typeof CallGraphOptionsSchema>;

/**
 * Semantic compress options schema
 */
export const SemanticCompressOptionsSchema = z.object({
  similarityThreshold: z.number().min(0).max(1).optional(),
  budgetRatio: z.number().min(0).max(1).optional(),
  minChunkSize: z.number().int().positive().max(100_000).optional(),
  maxChunkSize: z.number().int().positive().max(100_000).optional(),
}).strict();
export type SemanticCompressOptions = z.infer<typeof SemanticCompressOptionsSchema>;

/**
 * Generate map options schema
 */
export const GenerateMapOptionsSchema = z.object({
  budget: z.number().int().positive().max(10_000_000).optional(),
  maxSymbols: z.number().int().positive().max(10_000).optional(),
}).strict();
export type GenerateMapOptions = z.infer<typeof GenerateMapOptionsSchema>;

/**
 * Transitive callers options schema
 */
export const TransitiveCallersOptionsSchema = z.object({
  maxDepth: z.number().int().positive().max(100).optional(),
  maxResults: z.number().int().positive().max(10_000).optional(),
}).strict();
export type TransitiveCallersOptions = z.infer<typeof TransitiveCallersOptionsSchema>;

/**
 * Call sites context options schema
 */
export const CallSitesContextOptionsSchema = z.object({
  linesBefore: z.number().int().nonnegative().max(1_000).optional(),
  linesAfter: z.number().int().nonnegative().max(1_000).optional(),
}).strict();
export type CallSitesContextOptions = z.infer<typeof CallSitesContextOptionsSchema>;

/**
 * Changed symbols filter schema
 */
export const ChangedSymbolsFilterSchema = z.object({
  kinds: z.array(SymbolKindSchema).optional(),
  excludeKinds: z.array(SymbolKindSchema).optional(),
}).strict();
export type ChangedSymbolsFilter = z.infer<typeof ChangedSymbolsFilterSchema>;

/**
 * Breaking change options schema
 */
export const BreakingChangeOptionsSchema = z.object({
  oldRef: z.string().min(1),
  newRef: z.string().min(1),
}).strict();
export type BreakingChangeOptions = z.infer<typeof BreakingChangeOptionsSchema>;

/**
 * Dead code options schema
 */
export const DeadCodeOptionsSchema = z.object({
  paths: z.array(z.string()).optional(),
  languages: z.array(z.string()).optional(),
}).strict();
export type DeadCodeOptions = z.infer<typeof DeadCodeOptionsSchema>;

/**
 * Multi-repo entry schema
 */
export const MultiRepoEntrySchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  path: z.string().min(1),
}).strict();
export type MultiRepoEntry = z.infer<typeof MultiRepoEntrySchema>;

/**
 * Multi-repo options schema
 */
export const MultiRepoOptionsSchema = z.object({
  repositories: z.array(MultiRepoEntrySchema),
}).strict();
export type MultiRepoOptions = z.infer<typeof MultiRepoOptionsSchema>;

/**
 * Extract documentation options schema
 */
export const ExtractDocOptionsSchema = z.object({
  language: z.string().min(1),
}).strict();
export type ExtractDocOptions = z.infer<typeof ExtractDocOptionsSchema>;

/**
 * Complexity options schema
 */
export const ComplexityOptionsSchema = z.object({
  language: z.string().min(1),
}).strict();
export type ComplexityOptions = z.infer<typeof ComplexityOptionsSchema>;

// ============================================================================
// Validation Helper Functions
// ============================================================================

/**
 * Validate pack options with detailed error messages
 * @throws {z.ZodError} if validation fails
 */
export function validatePackOptions(input: unknown): PackOptions {
  return PackOptionsSchema.parse(input);
}

/**
 * Safely validate pack options, returns result object
 */
export function safeValidatePackOptions(input: unknown): z.ZodSafeParseResult<PackOptions> {
  return PackOptionsSchema.safeParse(input);
}

/**
 * Validate scan options
 * @throws {z.ZodError} if validation fails
 */
export function validateScanOptions(input: unknown): ScanOptions {
  return ScanOptionsSchema.parse(input);
}

/**
 * Safely validate scan options
 */
export function safeValidateScanOptions(input: unknown): z.ZodSafeParseResult<ScanOptions> {
  return ScanOptionsSchema.safeParse(input);
}

/**
 * Validate chunk options
 * @throws {z.ZodError} if validation fails
 */
export function validateChunkOptions(input: unknown): ChunkOptions {
  return ChunkOptionsSchema.parse(input);
}

/**
 * Safely validate chunk options
 */
export function safeValidateChunkOptions(input: unknown): z.ZodSafeParseResult<ChunkOptions> {
  return ChunkOptionsSchema.safeParse(input);
}

/**
 * Validate embed options
 * @throws {z.ZodError} if validation fails
 */
export function validateEmbedOptions(input: unknown): EmbedOptions {
  return EmbedOptionsSchema.parse(input);
}

/**
 * Safely validate embed options
 */
export function safeValidateEmbedOptions(input: unknown): z.ZodSafeParseResult<EmbedOptions> {
  return EmbedOptionsSchema.safeParse(input);
}

/**
 * Validate index options
 * @throws {z.ZodError} if validation fails
 */
export function validateIndexOptions(input: unknown): IndexOptions {
  return IndexOptionsSchema.parse(input);
}

/**
 * Safely validate index options
 */
export function safeValidateIndexOptions(input: unknown): z.ZodSafeParseResult<IndexOptions> {
  return IndexOptionsSchema.safeParse(input);
}

/**
 * Validate diff context options
 * @throws {z.ZodError} if validation fails
 */
export function validateDiffContextOptions(input: unknown): DiffContextOptions {
  return DiffContextOptionsSchema.parse(input);
}

/**
 * Safely validate diff context options
 */
export function safeValidateDiffContextOptions(input: unknown): z.ZodSafeParseResult<DiffContextOptions> {
  return DiffContextOptionsSchema.safeParse(input);
}

/**
 * Validate impact options
 * @throws {z.ZodError} if validation fails
 */
export function validateImpactOptions(input: unknown): ImpactOptions {
  return ImpactOptionsSchema.parse(input);
}

/**
 * Safely validate impact options
 */
export function safeValidateImpactOptions(input: unknown): z.ZodSafeParseResult<ImpactOptions> {
  return ImpactOptionsSchema.safeParse(input);
}

// ============================================================================
// Output Schemas (for validating return values)
// ============================================================================

/**
 * Language stat schema
 */
export const LanguageStatSchema = z.object({
  language: z.string(),
  files: z.number().int().nonnegative(),
  lines: z.number().int().nonnegative(),
  percentage: z.number().min(0).max(100),
});
export type LanguageStat = z.infer<typeof LanguageStatSchema>;

/**
 * Scan stats schema
 */
export const ScanStatsSchema = z.object({
  name: z.string(),
  totalFiles: z.number().int().nonnegative(),
  totalLines: z.number().int().nonnegative(),
  totalTokens: z.number().int().nonnegative(),
  primaryLanguage: z.string().optional(),
  languages: z.array(LanguageStatSchema),
  securityFindings: z.number().int().nonnegative(),
});
export type ScanStats = z.infer<typeof ScanStatsSchema>;

/**
 * Security finding schema
 */
export const SecurityFindingSchema = z.object({
  file: z.string(),
  line: z.number().int().positive(),
  severity: z.string(),
  kind: z.string(),
  pattern: z.string(),
});
export type SecurityFinding = z.infer<typeof SecurityFindingSchema>;

/**
 * Index status schema
 */
export const IndexStatusSchema = z.object({
  exists: z.boolean(),
  fileCount: z.number().int().nonnegative(),
  symbolCount: z.number().int().nonnegative(),
  lastBuilt: z.string().optional(),
  version: z.string().optional(),
  filesUpdated: z.number().int().nonnegative().optional(),
  incremental: z.boolean().optional(),
});
export type IndexStatus = z.infer<typeof IndexStatusSchema>;

/**
 * Symbol info schema
 */
export const SymbolInfoSchema = z.object({
  id: z.number().int().nonnegative(),
  name: z.string(),
  kind: z.string(),
  file: z.string(),
  line: z.number().int().positive(),
  endLine: z.number().int().positive(),
  signature: z.string().optional(),
  visibility: z.string(),
});
export type SymbolInfo = z.infer<typeof SymbolInfoSchema>;

/**
 * Embed chunk schema
 */
export const EmbedChunkSchema = z.object({
  id: z.string().regex(/^ec_[a-f0-9]{32}$/),
  fullHash: z.string(),
  content: z.string(),
  tokens: z.number().int().nonnegative(),
  kind: z.string(),
  source: z.object({
    file: z.string(),
    linesStart: z.number().int().positive(),
    linesEnd: z.number().int().positive(),
    symbol: z.string(),
    fqn: z.string().optional(),
    language: z.string(),
    parent: z.string().optional(),
    visibility: z.string(),
    isTest: z.boolean(),
  }),
  context: z.object({
    docstring: z.string().optional(),
    comments: z.array(z.string()),
    signature: z.string().optional(),
    calls: z.array(z.string()),
    calledBy: z.array(z.string()),
    imports: z.array(z.string()),
    tags: z.array(z.string()),
    linesOfCode: z.number().int().nonnegative(),
    maxNestingDepth: z.number().int().nonnegative(),
  }),
  part: z.object({
    part: z.number().int().positive(),
    of: z.number().int().positive(),
    parentId: z.string(),
    parentSignature: z.string().optional(),
  }).optional(),
});
export type EmbedChunk = z.infer<typeof EmbedChunkSchema>;

/**
 * Embed diff summary schema
 */
export const EmbedDiffSummarySchema = z.object({
  added: z.number().int().nonnegative(),
  modified: z.number().int().nonnegative(),
  removed: z.number().int().nonnegative(),
  unchanged: z.number().int().nonnegative(),
  totalChunks: z.number().int().nonnegative(),
});
export type EmbedDiffSummary = z.infer<typeof EmbedDiffSummarySchema>;

/**
 * Embed result schema
 */
export const EmbedResultSchema = z.object({
  chunks: z.array(EmbedChunkSchema),
  diff: EmbedDiffSummarySchema.optional(),
  manifestVersion: z.number().int().nonnegative(),
  elapsedMs: z.number().int().nonnegative(),
});
export type EmbedResult = z.infer<typeof EmbedResultSchema>;
