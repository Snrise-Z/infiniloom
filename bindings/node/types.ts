/**
 * Strict TypeScript types for infiniloom-node
 *
 * These types provide string literal unions for better type safety
 * compared to the auto-generated index.d.ts which uses generic strings.
 *
 * Usage:
 * ```typescript
 * import type { StrictPackOptions, OutputFormat, TokenizerModel } from 'infiniloom-node/types';
 * import { pack } from 'infiniloom-node';
 *
 * const options: StrictPackOptions = {
 *   format: 'xml',  // Type-checked!
 *   model: 'claude',
 *   compression: 'balanced'
 * };
 *
 * const output = pack('./repo', options);
 * ```
 */

// Re-export base types from auto-generated index
export type {
  ScanStats,
  LanguageStat,
  GitFileStatus,
  GitChangedFile,
  GitCommit,
  GitBlameLine,
  GitDiffLine,
  GitDiffHunk,
  SecurityFinding,
  IndexStatus,
  SymbolInfo,
  ReferenceInfo,
  CallGraphEdge,
  CallGraphStats,
  CallGraph,
  DependencyCycle,
  SymbolSourceResult,
  RepoChunk,
  AffectedSymbol,
  DiffFileContext,
  ContextSymbolInfo,
  CallSite,
  CallSiteWithContext,
  ChangedSymbolInfo,
  TransitiveCallerInfo,
  EmbedChunkSource,
  EmbedChunkContext,
  EmbedChunkPart,
  EmbedChunk,
  EmbedDiffSummary,
  EmbedResult,
  EmbedManifestStatus,
  JsTypeInfo,
  JsParameterInfo,
  JsGenericParam,
  JsTypeSignature,
  JsParamDoc,
  JsReturnDoc,
  JsThrowsDoc,
  JsExample,
  JsDocumentation,
  JsAncestorInfo,
  JsTypeHierarchy,
  JsHalsteadMetrics,
  JsLocMetrics,
  JsComplexityMetrics,
  JsUnusedExport,
  JsUnreachableCode,
  JsUnusedSymbol,
  JsUnusedImport,
  JsUnusedVariable,
  JsDeadCodeInfo,
  JsBreakingChange,
  JsBreakingChangeSummary,
  JsBreakingChangeReport,
  JsRepoEntry,
  JsCrossRepoLink,
  JsUnifiedSymbolRef,
  JsMultiRepoStats,
  CheckComplexityOptions,
  JsComplexityViolation,
  JsComplexityCheckResult,
} from './index';

// ============================================================================
// String Literal Union Types
// ============================================================================

/**
 * Output format for pack/chunk operations
 * - xml: Optimized for Claude (CDATA sections, structured XML)
 * - markdown: Optimized for GPT models (fenced code blocks)
 * - json: Machine-readable JSON format
 * - yaml: YAML format compatible with Gemini and other models (query at end)
 * - toon: Token-efficient format (30-40% fewer tokens)
 * - plain: Plain text format
 */
export type OutputFormat = 'xml' | 'markdown' | 'json' | 'yaml' | 'toon' | 'plain';

/**
 * Supported LLM tokenizer models
 *
 * OpenAI models (exact via tiktoken):
 * - gpt-5.2, gpt-5.1, gpt-5, o4-mini, o3, o1, gpt-4o, gpt-4o-mini: o200k_base encoding
 * - gpt-4, gpt-4-turbo, gpt-3.5-turbo: cl100k_base encoding
 *
 * Other models (calibrated estimation):
 * - claude: Anthropic Claude models
 * - gemini, gemini-1.5, gemini-2.0, gemini-3.1: Google Gemini models
 * - llama, llama-3, llama-3.1, llama-3.2, codellama: Meta Llama models
 * - mistral, mixtral: Mistral AI models
 * - deepseek, deepseek-v3: DeepSeek models
 * - qwen, qwen-2.5: Alibaba Qwen models
 * - cohere, command-r: Cohere models
 * - grok: xAI Grok models
 */
export type TokenizerModel =
  // OpenAI models (exact tokenization via tiktoken)
  | 'gpt-5.2'
  | 'gpt-5.2-pro'
  | 'gpt-5.1'
  | 'gpt-5.1-mini'
  | 'gpt-5.1-codex'
  | 'gpt-5'
  | 'gpt-5-mini'
  | 'gpt-5-nano'
  | 'o4-mini'
  | 'o3'
  | 'o3-mini'
  | 'o1'
  | 'o1-mini'
  | 'o1-preview'
  | 'gpt-4o'
  | 'gpt-4o-mini'
  | 'gpt-4'
  | 'gpt-4-turbo'
  | 'gpt-3.5-turbo'
  // Anthropic
  | 'claude'
  // Google
  | 'gemini'
  | 'gemini-1.5'
  | 'gemini-2.0'
  | 'gemini-3.1'
  // Meta
  | 'llama'
  | 'llama-3'
  | 'llama-3.1'
  | 'llama-3.2'
  | 'codellama'
  // Mistral
  | 'mistral'
  | 'mixtral'
  // DeepSeek
  | 'deepseek'
  | 'deepseek-v3'
  // Alibaba
  | 'qwen'
  | 'qwen-2.5'
  // Cohere
  | 'cohere'
  | 'command-r'
  // xAI
  | 'grok';

/**
 * Compression level for output
 * - none: No compression, full output
 * - minimal: Light compression (remove blank lines)
 * - balanced: Moderate compression (recommended)
 * - aggressive: Heavy compression (remove comments, simplify)
 * - extreme: Maximum compression (signatures only)
 * - focused: Focus on key symbols
 * - semantic: AI-aware semantic compression
 */
export type CompressionLevel =
  | 'none'
  | 'minimal'
  | 'balanced'
  | 'aggressive'
  | 'extreme'
  | 'focused'
  | 'semantic';

/**
 * Security severity levels
 */
export type SecuritySeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';

/**
 * Git file status
 */
export type GitStatus = 'Added' | 'Modified' | 'Deleted' | 'Renamed' | 'Copied' | 'Unknown';

/**
 * Diff line change type
 */
export type DiffChangeType = 'add' | 'remove' | 'context';

/**
 * Symbol kinds (matches Rust SymbolKind enum)
 */
export type SymbolKind =
  | 'function'
  | 'method'
  | 'class'
  | 'struct'
  | 'interface'
  | 'trait'
  | 'enum'
  | 'constant'
  | 'variable'
  | 'import'
  | 'export'
  | 'type'
  | 'module'
  | 'macro';

/**
 * Symbol visibility
 */
export type Visibility = 'public' | 'private' | 'protected' | 'internal';

/**
 * Impact type for affected symbols
 */
export type ImpactType = 'direct' | 'caller' | 'callee' | 'dependent';

/**
 * Impact level for analysis results
 */
export type ImpactLevel = 'low' | 'medium' | 'high' | 'critical';

/**
 * Reference kind
 */
export type ReferenceKind = 'call' | 'import' | 'inherit' | 'implement';

/**
 * Context symbol reason
 */
export type ContextReason = 'changed' | 'caller' | 'callee' | 'dependent';

/**
 * Change type for symbols
 */
export type ChangeType = 'added' | 'modified' | 'deleted';

/**
 * Chunking strategy
 */
export type ChunkStrategy =
  | 'fixed'
  | 'file'
  | 'module'
  | 'symbol'
  | 'semantic'
  | 'dependency';

/**
 * Embed chunk kind (matches Rust ChunkKind enum)
 */
export type EmbedChunkKind =
  | 'function'
  | 'method'
  | 'class'
  | 'struct'
  | 'enum'
  | 'interface'
  | 'trait'
  | 'module'
  | 'constant'
  | 'variable'
  | 'imports'
  | 'top_level'
  | 'function_part'
  | 'class_part';

/**
 * Breaking change type
 */
export type BreakingChangeType =
  | 'removed'
  | 'signature_changed'
  | 'type_changed'
  | 'visibility_reduced'
  | 'parameter_added'
  | 'parameter_removed'
  | 'return_type_changed';

/**
 * Breaking change severity
 */
export type ChangeSeverity = 'critical' | 'high' | 'medium' | 'low';

/**
 * Cross-repo link type
 */
export type CrossRepoLinkType = 'import' | 'call' | 'inherit' | 'implement';

/**
 * Variance for generic parameters
 */
export type GenericVariance = 'invariant' | 'covariant' | 'contravariant' | 'bivariant';

/**
 * Parameter kind
 */
export type ParameterKind = 'positional' | 'named' | 'rest' | 'keyword_only' | 'positional_only';

// ============================================================================
// Strict Option Interfaces
// ============================================================================

/**
 * Strict pack options with literal union types
 */
export interface StrictPackOptions {
  /** Output format with strict typing */
  format?: OutputFormat;
  /** Target model with strict typing */
  model?: TokenizerModel;
  /** Compression level with strict typing */
  compression?: CompressionLevel;
  /** Token budget for repository map */
  mapBudget?: number;
  /** Maximum number of symbols in map */
  maxSymbols?: number;
  /** Skip security scanning */
  skipSecurity?: boolean;
  /** Redact detected secrets in output */
  redactSecrets?: boolean;
  /** Skip symbol extraction */
  skipSymbols?: boolean;
  /** Glob patterns to include */
  include?: readonly string[];
  /** Glob patterns to exclude */
  exclude?: readonly string[];
  /** Include test files */
  includeTests?: boolean;
  /** Minimum security severity to block on */
  securityThreshold?: SecuritySeverity;
  /** Token budget for total output (0 = no limit) */
  tokenBudget?: number;
  /** Only include files changed in git */
  changedOnly?: boolean;
  /** Base SHA/ref for diff comparison */
  baseSha?: string;
  /** Head SHA/ref for diff comparison */
  headSha?: string;
  /** Include staged changes only */
  stagedOnly?: boolean;
  /** Include related files */
  includeRelated?: boolean;
  /** Depth for related file traversal (1-3) */
  relatedDepth?: 1 | 2 | 3;
}

/**
 * Strict scan options with literal union types
 */
export interface StrictScanOptions {
  /** Target model for token counting */
  model?: TokenizerModel;
  /** Glob patterns to include */
  include?: readonly string[];
  /** Glob patterns to exclude */
  exclude?: readonly string[];
  /** Include test files */
  includeTests?: boolean;
  /** Apply default ignores */
  applyDefaultIgnores?: boolean;
}

/**
 * Strict chunk options with literal union types
 */
export interface StrictChunkOptions {
  /** Chunking strategy */
  strategy?: ChunkStrategy;
  /** Maximum tokens per chunk */
  maxTokens?: number;
  /** Token overlap between chunks */
  overlap?: number;
  /** Target model for token counting */
  model?: TokenizerModel;
  /** Output format */
  format?: OutputFormat;
  /** Sort chunks by priority */
  priorityFirst?: boolean;
  /** Directories/patterns to exclude */
  exclude?: readonly string[];
}

/**
 * Strict diff context options
 */
export interface StrictDiffContextOptions {
  /** Depth of context expansion (1-3) */
  depth?: 1 | 2 | 3;
  /** Token budget for context */
  budget?: number;
  /** Include the actual diff content */
  includeDiff?: boolean;
  /** Output format */
  format?: OutputFormat;
  /** Target model for token counting */
  model?: TokenizerModel;
  /** Glob patterns to exclude */
  exclude?: readonly string[];
  /** Glob patterns to include */
  include?: readonly string[];
}

/**
 * Strict impact options
 */
export interface StrictImpactOptions {
  /** Depth of dependency traversal (1-3) */
  depth?: 1 | 2 | 3;
  /** Include test files in analysis */
  includeTests?: boolean;
  /** Target model for token counting */
  model?: TokenizerModel;
  /** Glob patterns to exclude */
  exclude?: readonly string[];
  /** Glob patterns to include */
  include?: readonly string[];
}

/**
 * Strict embed options
 */
export interface StrictEmbedOptions {
  /** Maximum tokens per chunk */
  maxTokens?: number;
  /** Minimum tokens for a chunk */
  minTokens?: number;
  /** Lines of context around symbols */
  contextLines?: number;
  /** Include imports in chunks */
  includeImports?: boolean;
  /** Include top-level code */
  includeTopLevel?: boolean;
  /** Include test files */
  includeTests?: boolean;
  /** Enable secret scanning */
  securityScan?: boolean;
  /** Include patterns (glob) */
  includePatterns?: readonly string[];
  /** Exclude patterns (glob) */
  excludePatterns?: readonly string[];
  /** Path to manifest file */
  manifestPath?: string;
  /** Only return changed chunks (diff mode) */
  diffOnly?: boolean;
}

/**
 * Strict query filter
 */
export interface StrictQueryFilter {
  /** Filter by symbol kinds */
  kinds?: readonly SymbolKind[];
  /** Exclude specific kinds */
  excludeKinds?: readonly SymbolKind[];
}

/**
 * Strict symbol filter
 */
export interface StrictSymbolFilter {
  /** Filter by symbol kind */
  kind?: SymbolKind;
  /** Filter by visibility */
  visibility?: Visibility;
}

/**
 * Strict index options
 */
export interface StrictIndexOptions {
  /** Force full rebuild */
  force?: boolean;
  /** Include test files */
  includeTests?: boolean;
  /** Maximum file size to index (bytes) */
  maxFileSize?: number;
  /** Directories/patterns to exclude */
  exclude?: readonly string[];
  /** Incremental update */
  incremental?: boolean;
}

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Type guard for OutputFormat
 */
export function isOutputFormat(value: unknown): value is OutputFormat {
  return (
    typeof value === 'string' &&
    ['xml', 'markdown', 'json', 'yaml', 'toon', 'plain'].includes(value)
  );
}

/**
 * Type guard for TokenizerModel
 */
export function isTokenizerModel(value: unknown): value is TokenizerModel {
  const models: TokenizerModel[] = [
    // OpenAI models (exact tokenization via tiktoken)
    'gpt-5.2', 'gpt-5.2-pro', 'gpt-5.1', 'gpt-5.1-mini', 'gpt-5.1-codex',
    'gpt-5', 'gpt-5-mini', 'gpt-5-nano',
    'o4-mini', 'o3', 'o3-mini', 'o1', 'o1-mini', 'o1-preview',
    'gpt-4o', 'gpt-4o-mini', 'gpt-4', 'gpt-4-turbo', 'gpt-3.5-turbo',
    // Anthropic
    'claude',
    // Google
    'gemini', 'gemini-1.5', 'gemini-2.0', 'gemini-3.1',
    // Meta
    'llama', 'llama-3', 'llama-3.1', 'llama-3.2', 'codellama',
    // Mistral
    'mistral', 'mixtral',
    // DeepSeek
    'deepseek', 'deepseek-v3',
    // Alibaba
    'qwen', 'qwen-2.5',
    // Cohere
    'cohere', 'command-r',
    // xAI
    'grok',
  ];
  return typeof value === 'string' && models.includes(value as TokenizerModel);
}

/**
 * Type guard for CompressionLevel
 */
export function isCompressionLevel(value: unknown): value is CompressionLevel {
  return (
    typeof value === 'string' &&
    ['none', 'minimal', 'balanced', 'aggressive', 'extreme', 'focused', 'semantic'].includes(value)
  );
}

/**
 * Type guard for SymbolKind
 */
export function isSymbolKind(value: unknown): value is SymbolKind {
  const kinds: SymbolKind[] = [
    'function', 'method', 'class', 'struct', 'interface', 'trait',
    'enum', 'constant', 'variable', 'import', 'export', 'type', 'module', 'macro',
  ];
  return typeof value === 'string' && kinds.includes(value as SymbolKind);
}

/**
 * Type guard for SecuritySeverity
 */
export function isSecuritySeverity(value: unknown): value is SecuritySeverity {
  return (
    typeof value === 'string' &&
    ['critical', 'high', 'medium', 'low', 'info'].includes(value)
  );
}

// ============================================================================
// Utility Types
// ============================================================================

/**
 * Make specific properties required
 */
export type WithRequired<T, K extends keyof T> = T & { [P in K]-?: T[P] };

/**
 * Make all properties readonly and non-nullable
 */
export type Strict<T> = {
  readonly [P in keyof T]-?: NonNullable<T[P]>;
};

/**
 * Extract the element type from an array type
 */
export type ElementOf<T> = T extends readonly (infer E)[] ? E : never;
