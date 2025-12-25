# Infiniloom Design Document

> **Note:** This document describes the design decisions and architecture of Infiniloom.
> For planned but unimplemented features, see [ROADMAP.md](planning/ROADMAP.md).
> For current implementation status, see [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md).

## Executive Summary

This document describes the design and architecture of **Infiniloom** - a high-performance repository context tool that leverages Tree-sitter parsing, PageRank ranking, and model-specific output formats to provide optimized context for LLM-assisted code understanding.

---

## Part 1: Technology Stack Evaluation

### Language Options Evaluated

*Note: After evaluation, **Rust** was chosen for the entire implementation.*

| Language | Performance | Memory Safety | Cross-Platform | Bindings | Build Time | Ecosystem |
|----------|-------------|---------------|----------------|----------|------------|-----------|
| **Rust** ✓ | Excellent | Excellent | Excellent | Good (PyO3, NAPI-RS) | Slow | Large |
| Go | Good | Good | Excellent | CGo overhead | Fast | Large |
| C++ | Excellent | Poor | Good | Excellent | Medium | Massive |

### Chosen: Pure Rust Architecture

#### Why Rust?

1. **Memory Safety Without GC**
   - Zero-cost abstractions
   - Compile-time memory safety guarantees
   - No runtime overhead

2. **Excellent Ecosystem**
   - Tree-sitter Rust bindings
   - tiktoken-rs for accurate token counting
   - Rayon for parallel processing
   - memmap2 for memory-mapped I/O

3. **Cross-Platform Support**
   ```bash
   # Build for all platforms
   cargo build --release --target x86_64-unknown-linux-gnu
   cargo build --release --target aarch64-apple-darwin
   cargo build --release --target x86_64-pc-windows-msvc
   ```

4. **Performance**
   - Thread-local parsers for lock-free parallelism
   - Memory-mapped file processing
   - Zero-copy where possible

#### Architecture

```
infiniloom/
├── cli/                        # Rust CLI application
│   └── src/
│       ├── main.rs             # Entry point
│       └── scanner.rs          # Parallel file scanning
│
├── engine/                     # Core Rust engine
│   ├── src/
│   │   ├── parser.rs           # Tree-sitter AST parsing
│   │   ├── repomap/            # PageRank symbol ranking
│   │   ├── output/             # Format generators
│   │   ├── security.rs         # Secret detection
│   │   └── config.rs           # Configuration
│   └── Cargo.toml
│
├── bindings/
│   ├── python/                 # PyO3 bindings
│   └── node/                   # NAPI-RS bindings
```

---

## Part 2: Advanced Features Deep Dive

### 2.1 Aider-Style Repository Map

Inspired by Aider's repository mapping, implement a **semantic code graph**:

```python
from infiniloom import Infiniloom
from infiniloom.repomap import RepoMap, RankingConfig

# Create intelligent repository map
repo_map = RepoMap(
    config=RankingConfig(
        # Graph-based importance ranking
        algorithm="pagerank",  # pagerank | hits | betweenness

        # What to extract
        extract={
            "functions": True,
            "classes": True,
            "interfaces": True,
            "types": True,
            "constants": True,
            "imports": True,
        },

        # Token budget for map
        token_budget=2000,

        # Ranking factors
        factors={
            "reference_count": 0.4,      # How often referenced
            "import_centrality": 0.2,    # Import graph position
            "modification_recency": 0.2, # Recent changes
            "file_size": 0.1,            # Larger = more important
            "test_coverage": 0.1,        # Has tests = important
        }
    )
)

# Generate map
map_output = repo_map.generate("/path/to/repo")
print(map_output.summary)
# Output: "Repository: myproject (45 files, 12,345 lines)
#          Key symbols: UserService, DatabaseConnection, authenticate()
#          Hot files: src/auth.py (25 refs), src/db.py (18 refs)"
```

#### Repository Map Output Format

```xml
<repository_map tokens="1847">
  <overview>
    <stats files="45" lines="12345" languages="3" />
    <description>E-commerce backend with authentication and payment processing</description>
  </overview>

  <key_symbols>
    <symbol name="UserService" type="class" file="src/auth/service.py" refs="25" />
    <symbol name="authenticate" type="function" file="src/auth/handlers.py" refs="18" />
    <symbol name="PaymentProcessor" type="class" file="src/payments/processor.py" refs="15" />
  </key_symbols>

  <file_signatures>
    <file path="src/auth/service.py" importance="high">
      <![CDATA[
class UserService:
    def create_user(self, email: str, password: str) -> User: ...
    def authenticate(self, email: str, password: str) -> Optional[Token]: ...
    def refresh_token(self, token: Token) -> Token: ...
      ]]>
    </file>
    <file path="src/payments/processor.py" importance="high">
      <![CDATA[
class PaymentProcessor:
    def charge(self, amount: Decimal, card: Card) -> PaymentResult: ...
    def refund(self, transaction_id: str) -> RefundResult: ...
      ]]>
    </file>
  </file_signatures>

  <dependency_graph format="mermaid">
    <![CDATA[
graph TD
    A[auth/service.py] --> B[db/models.py]
    A --> C[utils/crypto.py]
    D[payments/processor.py] --> B
    D --> E[external/stripe.py]
    ]]>
  </dependency_graph>
</repository_map>
```

### 2.2 Semantic Code Embeddings (CodeBERT/StarCoder)

Phase 2 (not implemented intentionally): embeddings-backed search and vector stores are design-only in current releases.

Integrate code embeddings for semantic search and similarity:

```python
from infiniloom.embeddings import CodeEmbedder, EmbeddingConfig

embedder = CodeEmbedder(
    config=EmbeddingConfig(
        # Model selection
        model="starcoderbase",  # codebert | starcoderbase | codellama | local

        # Embedding dimensions
        dimensions=768,

        # What to embed
        granularity="function",  # file | class | function | chunk

        # Storage
        vector_store="qdrant",  # qdrant | chromadb | pinecone | memory

        # On-device for privacy
        on_device=True,
    )
)

# Index repository
embedder.index("/path/to/repo")

# Semantic search
results = embedder.search(
    query="function that handles user authentication",
    top_k=5,
    threshold=0.7
)

for result in results:
    print(f"{result.file}:{result.line} - {result.name} (score: {result.score})")
```

#### Embedding-Enhanced Context Selection

```python
from infiniloom import Infiniloom
from infiniloom.context import SmartContextSelector

selector = SmartContextSelector(
    # Combine multiple signals
    strategies=[
        "embedding_similarity",  # Semantic similarity to query
        "graph_centrality",      # Code graph importance
        "recency",               # Recent modifications
        "test_association",      # Include related tests
    ],

    # Context budget
    max_tokens=50000,

    # Reserve tokens for response
    response_budget=10000,
)

# User query
query = "How does the payment processing work?"

# Get optimized context
context = selector.select(
    repo="/path/to/repo",
    query=query,
)

print(f"Selected {len(context.files)} files ({context.tokens} tokens)")
print(f"Relevance score: {context.average_relevance}")
```

### 2.3 Zoekt-Style Trigram Search Integration

Phase 2 (not implemented intentionally): trigram/Zoekt search is a planned extension and is not implemented yet.

Implement fast code search for interactive use:

```python
from infiniloom.search import TrigramIndex, SearchConfig

# Build trigram index
index = TrigramIndex(
    config=SearchConfig(
        # Index configuration
        languages=["python", "typescript", "rust"],

        # Search features
        features={
            "regex": True,
            "literal": True,
            "symbol": True,
            "definition": True,
        },

        # Ranking
        ranking="bm25",  # bm25 | tfidf | code_aware

        # Performance
        parallel=True,
        cache_queries=True,
    )
)

# Index repository
index.build("/path/to/repo")

# Fast search
results = index.search(
    query="def authenticate",
    filters={
        "language": "python",
        "path": "src/**",
    },
    limit=20
)
```

### 2.4 SCIP-Based Semantic Navigation

Phase 2 (not implemented intentionally): SCIP indexing and navigation are design-only in current releases.

Integrate Sourcegraph's SCIP for precise code intelligence:

```python
from infiniloom.scip import SCIPIndexer, NavigationQuery

indexer = SCIPIndexer(
    # Languages to index
    languages=["python", "typescript", "go", "rust"],

    # Index type
    include_references=True,
    include_implementations=True,
    include_hover_docs=True,
)

# Build SCIP index
index = indexer.index("/path/to/repo")

# Precise navigation
definitions = index.find_definitions("UserService")
references = index.find_references("authenticate")
implementations = index.find_implementations("PaymentProcessor")

# Include in context with full navigation info
context = repo.pack(
    scip_annotations=True,  # Add go-to-definition hints
    include_hover_docs=True,  # Add documentation on hover
)
```

### 2.5 Prompt Caching Optimization

Optimize output structure for LLM prompt caching:

```python
from infiniloom.cache import CacheOptimizer, CacheStrategy

optimizer = CacheOptimizer(
    strategy=CacheStrategy(
        # Target LLM
        model="claude-sonnet-4-20250514",

        # Minimum cacheable prefix (Claude requires 1024+ tokens)
        min_cache_tokens=1024,

        # Structure for caching
        layout="hierarchical",  # hierarchical | flat | chunked

        # Stable prefix content (rarely changes)
        stable_sections=[
            "repository_metadata",
            "directory_structure",
            "dependency_graph",
            "file_index",
        ],

        # Volatile content (changes often)
        volatile_sections=[
            "file_contents",
            "git_diff",
            "recent_changes",
        ],
    )
)

# Generate cache-optimized output
output = optimizer.generate("/path/to/repo")

# Output structure:
# [CACHEABLE PREFIX - 5000 tokens]
#   - Repository metadata
#   - Complete file index with summaries
#   - Dependency graph
#   - API surface area
# [CACHE BREAK POINT]
# [VOLATILE CONTENT - varies]
#   - Requested file contents
#   - Recent changes
#   - User-specific context
```

#### Cache-Aware Output Format

```xml
<?xml version="1.0" encoding="UTF-8"?>
<repository cache_version="1" generated="2024-01-01T00:00:00Z">

  <!-- CACHEABLE SECTION START (tokens: 5234) -->
  <!-- This section is stable and should be cached by the LLM -->
  <cache_section id="stable_context">

    <metadata>
      <name>myproject</name>
      <description>E-commerce platform with microservices architecture</description>
      <languages>
        <language name="Python" files="45" lines="12000" percentage="60" />
        <language name="TypeScript" files="30" lines="8000" percentage="40" />
      </languages>
    </metadata>

    <file_index entries="75">
      <!-- Complete index for all files -->
      <entry path="src/auth/service.py" tokens="450" summary="User authentication service" />
      <entry path="src/payments/processor.py" tokens="380" summary="Payment processing" />
      <!-- ... all files ... -->
    </file_index>

    <api_surface>
      <!-- All public interfaces -->
      <interface file="src/auth/service.py">
        <method name="authenticate" signature="(email: str, password: str) -> Token" />
        <method name="create_user" signature="(data: UserCreate) -> User" />
      </interface>
    </api_surface>

    <dependency_graph>
      <!-- Module dependencies -->
    </dependency_graph>

  </cache_section>
  <!-- CACHEABLE SECTION END -->

  <!-- CACHE BREAK POINT - Content below varies per request -->

  <dynamic_content request_id="abc123">
    <requested_files>
      <file path="src/auth/service.py">
        <content><![CDATA[
        # Full file content here
        ]]></content>
      </file>
    </requested_files>

    <recent_changes since="2024-01-01">
      <!-- Git changes -->
    </recent_changes>
  </dynamic_content>

</repository>
```

### 2.6 Incremental Processing with Content Addressing

Implement Git-like content addressing for efficient updates:

```python
from infiniloom.incremental import ContentAddressedStore, DeltaProcessor

store = ContentAddressedStore(
    # Storage backend
    backend="sqlite",  # sqlite | rocksdb | memory

    # Content hashing
    hash_algorithm="xxhash64",  # xxhash64 | blake3 | sha256

    # Compression
    compression="zstd",  # zstd | lz4 | none
)

processor = DeltaProcessor(store)

# Initial full index
result = processor.index("/path/to/repo")
print(f"Indexed {result.files} files, {result.tokens} tokens")

# Later: incremental update (only changed files)
delta = processor.update("/path/to/repo")
print(f"Updated {delta.changed} files, {delta.added} new, {delta.removed} deleted")
print(f"Processing time: {delta.time_ms}ms (vs {result.time_ms}ms full)")

# Generate output with cached content
output = processor.generate(
    changed_only=False,  # Include all files
    highlight_changes=True,  # Mark changed sections
)
```

### 2.7 Real-Time Streaming with Backpressure

Phase 2 (not implemented intentionally): streaming/backpressure APIs are planned but not implemented yet.

Handle massive repositories with streaming:

```python
from infiniloom.stream import StreamingProcessor, BackpressureConfig
import asyncio

processor = StreamingProcessor(
    config=BackpressureConfig(
        # Chunk configuration
        chunk_tokens=1000,

        # Backpressure handling
        max_buffered_chunks=10,
        pause_threshold=8,
        resume_threshold=4,

        # Memory limits
        max_memory_mb=512,
    )
)

async def process_large_repo():
    async for chunk in processor.stream("/path/to/huge/repo"):
        # Process each chunk as it arrives
        match chunk.type:
            case "metadata":
                print(f"Repository: {chunk.data.name}")
            case "file":
                print(f"File: {chunk.data.path} ({chunk.data.tokens} tokens)")
                # Send to LLM API
                await send_to_api(chunk.data)
            case "summary":
                print(f"Complete: {chunk.data.total_tokens} tokens")

        # Yield control for backpressure
        await asyncio.sleep(0)

asyncio.run(process_large_repo())
```

### 2.9 Multi-Modal Context (Diagrams, Images)

Phase 2 (not implemented intentionally): multimodal (images/diagrams) context is planned but not implemented yet.

Include visual context for multi-modal LLMs:

```python
from infiniloom.multimodal import MultiModalProcessor, ImageConfig

processor = MultiModalProcessor(
    image_config=ImageConfig(
        # What to include
        include={
            "architecture_diagrams": True,  # .png, .svg in docs/
            "screenshots": True,            # UI screenshots
            "uml_diagrams": True,           # Generated from code
        },

        # Image processing
        max_dimension=1024,
        format="base64",  # base64 | url | description

        # For models without vision
        fallback="description",  # Generate text description
    )
)

output = processor.pack("/path/to/repo")

# Output includes:
# - Code as usual
# - Architecture diagrams as base64 or URLs
# - Auto-generated UML class diagrams
# - Screenshot context for UI code
```

### 2.10 Diff-Aware Context for Code Review

Optimized output for code review workflows:

```python
from infiniloom.review import ReviewContext, DiffMode

review = ReviewContext(
    # Diff source
    diff_source="pr",  # pr | commit | branch | working

    # What to include
    mode=DiffMode(
        # Changed files
        include_changed=True,

        # Context around changes
        context_lines=10,

        # Related files (imports, tests)
        include_related=True,
        related_depth=2,

        # Full file for small changes
        full_file_threshold=50,  # lines changed
    ),

    # Annotations
    annotations={
        "line_blame": True,      # Who last modified
        "test_status": True,     # Test pass/fail
        "lint_warnings": True,   # Static analysis
        "complexity": True,      # Cyclomatic complexity
    }
)

# Generate review-optimized context
output = review.generate(
    repo="/path/to/repo",
    pr_number=123,
)
```

#### Review Context Output

```xml
<review_context pr="123" base="main" head="feature/auth">
  <summary>
    <stats files_changed="5" additions="150" deletions="30" />
    <description>Add OAuth2 authentication flow</description>
    <risk_assessment level="medium">
      Modifies authentication logic, requires security review
    </risk_assessment>
  </summary>

  <changed_files>
    <file path="src/auth/oauth.py" status="added" risk="high">
      <full_content>
        <![CDATA[
# New file - OAuth2 implementation
class OAuth2Provider:
    """OAuth2 authentication provider."""

    def authenticate(self, code: str) -> Token:
        # Implementation
        pass
        ]]>
      </full_content>
      <annotations>
        <lint line="15" level="warning">Consider adding rate limiting</lint>
        <security line="23" level="info">Ensure PKCE is enabled</security>
      </annotations>
    </file>

    <file path="src/auth/service.py" status="modified">
      <hunks>
        <hunk start_line="45" context="10">
          <before><![CDATA[
    def authenticate(self, email: str, password: str) -> Token:
        user = self.find_user(email)
        if user and self.verify_password(password, user.password_hash):
            return self.create_token(user)
        return None
          ]]></before>
          <after><![CDATA[
    def authenticate(
        self,
        email: str = None,
        password: str = None,
        oauth_code: str = None,
        provider: str = None,
    ) -> Token:
        if oauth_code and provider:
            return self.oauth_authenticate(oauth_code, provider)

        user = self.find_user(email)
        if user and self.verify_password(password, user.password_hash):
            return self.create_token(user)
        return None
          ]]></after>
          <blame>
            <line num="45" author="alice" date="2024-01-01" />
            <line num="46" author="bob" date="2023-12-15" />
          </blame>
        </hunk>
      </hunks>
    </file>
  </changed_files>

  <related_files reason="imported_by_changed">
    <file path="src/auth/handlers.py" relevance="high">
      <!-- File that imports changed module -->
    </file>
    <file path="tests/test_auth.py" relevance="high">
      <!-- Related test file -->
    </file>
  </related_files>

  <test_results>
    <suite name="auth" status="passing" coverage="85%">
      <test name="test_oauth_flow" status="new" />
      <test name="test_password_auth" status="passing" />
    </suite>
  </test_results>
</review_context>
```

---

## Part 3: Plugin Architecture

Phase 2 (not implemented intentionally): the plugin SDK and related examples are design-only in current releases.

### 3.1 Nickel-Inspired Configuration Language

Create a powerful, typed configuration system:

```nickel
# infiniloom.ncl - Nickel configuration file

let InfiniloomConfig = {
  # Type contracts for validation
  model | String | default = "claude-sonnet-4-20250514",
  context_window | Number | default = 200000,

  output | {
    format | [| 'xml, 'markdown, 'json, 'compressed |] | default = 'xml,
    path | String | default = "infiniloom-output.xml",
  },

  compression | {
    level | [| 'minimal, 'balanced, 'aggressive, 'extreme |] | default = 'balanced,
    preserve | Array String | default = ["docstrings", "types"],
  },

  # Custom transformers
  transformers | Array {
    name | String,
    pattern | String,
    action | [| 'include, 'exclude, 'transform |],
    transform | String | optional,
  } | default = [],
}

# Actual configuration
{
  model = "claude-sonnet-4-20250514",

  output = {
    format = 'xml,
    path = "./context.xml",
  },

  compression = {
    level = 'balanced,
    preserve = ["docstrings", "types", "comments_with_todo"],
  },

  transformers = [
    {
      name = "redact_secrets",
      pattern = "**/*.env",
      action = 'exclude,
    },
    {
      name = "summarize_tests",
      pattern = "**/test_*.py",
      action = 'transform,
      transform = "extract_test_names",
    },
  ],
}
```

### 3.2 Plugin SDK

```python
from infiniloom.plugins import Plugin, hook, HookPriority
from infiniloom.types import File, Context, Output

class MyCustomPlugin(Plugin):
    """Example custom plugin with full lifecycle hooks."""

    name = "my-custom-plugin"
    version = "1.0.0"

    # Plugin configuration schema
    config_schema = {
        "api_key": {"type": "string", "required": True},
        "threshold": {"type": "number", "default": 0.8},
    }

    def __init__(self, config):
        self.api_key = config["api_key"]
        self.threshold = config["threshold"]

    @hook("file.discover", priority=HookPriority.EARLY)
    def filter_files(self, files: list[str]) -> list[str]:
        """Filter files before processing."""
        return [f for f in files if not f.endswith('.generated.py')]

    @hook("file.read")
    def process_file(self, file: File) -> File:
        """Transform file content after reading."""
        if file.language == "python":
            file.content = self.add_type_hints(file.content)
        return file

    @hook("file.compress")
    def custom_compression(self, file: File, context: Context) -> File:
        """Custom compression logic."""
        if file.tokens > 1000:
            file.content = self.smart_summarize(file.content)
        return file

    @hook("output.format", priority=HookPriority.LATE)
    def add_metadata(self, output: Output) -> Output:
        """Add custom metadata to output."""
        output.metadata["custom_analysis"] = self.analyze(output)
        return output

    @hook("output.post")
    async def upload_to_api(self, output: Output):
        """Post-processing hook (async supported)."""
        await self.upload(output, self.api_key)


# Register plugin
from infiniloom import Infiniloom

forge = Infiniloom()
forge.register_plugin(MyCustomPlugin, config={
    "api_key": "xxx",
    "threshold": 0.9,
})
```

### 3.3 Built-in Plugin Library

```python
# Language-specific plugins
from infiniloom.plugins.languages import (
    PythonEnhancer,      # Type inference, docstring extraction
    TypeScriptEnhancer,  # Type extraction, JSDoc parsing
    RustEnhancer,        # Cargo.toml parsing, trait extraction
    GoEnhancer,          # Go mod parsing, interface extraction
)

# Output plugins
from infiniloom.plugins.output import (
    NotionExporter,      # Export to Notion pages
    ConfluenceExporter,  # Export to Confluence
    GitHubGistExporter,  # Create GitHub Gists
)

# Analysis plugins
from infiniloom.plugins.analysis import (
    ComplexityAnalyzer,  # Cyclomatic complexity
    DuplicationFinder,   # Code clone detection
    DependencyGrapher,   # Visualize dependencies
    SecurityScanner,     # Vulnerability detection
)

# Integration plugins
from infiniloom.plugins.integrations import (
    JiraLinker,          # Link to Jira tickets from comments
    SlackNotifier,       # Send notifications
    LinearIntegration,   # Linear issue tracking
)
```

---

## Part 4: Model-Specific Optimizations Deep Dive

### 4.1 Claude Optimization Details

```python
from infiniloom.models.claude import ClaudeOptimizer, ClaudeConfig

optimizer = ClaudeOptimizer(
    config=ClaudeConfig(
        # Model selection
        model="claude-sonnet-4-20250514",  # opus, sonnet, haiku

        # Context optimization
        context_window=200000,
        target_utilization=0.8,  # Use 80% of context

        # Prompt caching
        cache_optimization=True,
        min_cache_tokens=1024,  # Claude minimum
        cache_ttl=300,  # 5 minute cache

        # Output format
        format="xml",  # Claude prefers XML
        xml_options={
            "use_cdata": True,  # Better parsing
            "include_attributes": True,  # Metadata in attributes
            "hierarchical": True,  # Nested structure
        },

        # Extended thinking support
        thinking_hints=True,
        artifact_hints=True,

        # Tool use optimization
        tool_context={
            "include_schemas": True,
            "function_signatures": True,
            "example_calls": True,
        },
    )
)

# Generate Claude-optimized output
output = optimizer.pack("/path/to/repo")
```

#### Claude-Specific Output Enhancements

```xml
<?xml version="1.0" encoding="UTF-8"?>
<repository xmlns:claude="http://anthropic.com/claude/context">

  <!-- Hint for extended thinking -->
  <claude:thinking_context>
    This repository contains complex authentication logic.
    Consider the security implications carefully.
    Key areas requiring analysis: OAuth flow, token validation, rate limiting.
  </claude:thinking_context>

  <!-- Artifact-ready code sections -->
  <files>
    <file path="src/auth/service.py"
          claude:artifact_type="code"
          claude:artifact_language="python"
          claude:artifact_title="Authentication Service">
      <content><![CDATA[
class AuthService:
    """User authentication service with OAuth2 support."""

    def authenticate(self, credentials: Credentials) -> Token:
        """Authenticate user and return JWT token."""
        # Implementation
        pass
      ]]></content>
    </file>
  </files>

  <!-- Tool use context -->
  <claude:tool_context>
    <available_tools>
      <tool name="run_tests" description="Execute test suite">
        <parameter name="path" type="string" description="Test file path" />
        <parameter name="verbose" type="boolean" default="false" />
      </tool>
      <tool name="lint_code" description="Run linter on code">
        <parameter name="files" type="array" description="Files to lint" />
      </tool>
    </available_tools>
  </claude:tool_context>

</repository>
```

### 4.2 GPT-4 Optimization Details

```python
from infiniloom.models.openai import OpenAIOptimizer, OpenAIConfig

optimizer = OpenAIOptimizer(
    config=OpenAIConfig(
        model="gpt-4o",
        context_window=128000,

        # Format preferences
        format="markdown",
        markdown_options={
            "use_tables": True,        # GPT handles tables well
            "code_fence_language": True,
            "task_lists": True,        # Checkbox lists
        },

        # Function calling optimization
        function_calling={
            "include_schemas": True,
            "json_mode_ready": True,
            "structured_output": True,
        },

        # Token optimization (GPT tokenizer differs)
        tokenizer="cl100k_base",

        # Response format hints
        response_hints={
            "prefer_json": True,
            "structured_reasoning": True,
        },
    )
)
```

### 4.3 Gemini Optimization Details

```python
from infiniloom.models.google import GeminiOptimizer, GeminiConfig

optimizer = GeminiOptimizer(
    config=GeminiConfig(
        model="gemini-2.0-flash",
        context_window=1000000,  # 1M tokens!

        # Leverage massive context
        include_full_files=True,  # No need to truncate
        include_all_tests=True,
        include_documentation=True,

        # Grounding context
        grounding={
            "include_urls": True,
            "include_citations": True,
            "factual_context": True,
        },

        # Multimodal support
        multimodal={
            "include_diagrams": True,
            "include_screenshots": True,
            "image_format": "inline",  # Inline base64
        },

        # Code execution hints
        code_execution={
            "runnable_snippets": True,
            "execution_context": True,
        },
    )
)
```

### 4.4 Local Model Optimization (Ollama/vLLM)

```python
from infiniloom.models.local import LocalOptimizer, LocalConfig

optimizer = LocalOptimizer(
    config=LocalConfig(
        # Model detection
        model="deepseek-coder:33b",
        context_window=16384,  # Detect or specify

        # Aggressive optimization for small context
        compression={
            "level": "extreme",
            "strategy": "signatures_only",  # Only function signatures
            "max_tokens_per_file": 500,
        },

        # Smart chunking for multi-turn
        chunking={
            "enabled": True,
            "strategy": "semantic",
            "chunk_size": 6000,
            "overlap": 500,
            "prioritize_by": "relevance",
        },

        # Format for code models
        format="compressed",
        compressed_options={
            "remove_comments": True,
            "minify_whitespace": True,
            "abbreviate_names": False,  # Keep readable
        },

        # Context windowing for chat
        sliding_window={
            "enabled": True,
            "window_size": 12000,
            "summary_budget": 2000,  # Keep summary of previous context
        },
    )
)
```

---

## Part 5: Performance Architecture

### 5.1 Parallel Processing Pipeline

```
                    ┌─────────────────┐
                    │   File Scanner  │
                    │     (Rayon)     │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
       ┌──────────┐   ┌──────────┐   ┌──────────┐
       │ Worker 1 │   │ Worker 2 │   │ Worker N │
       │ (Parser) │   │ (Parser) │   │ (Parser) │
       └────┬─────┘   └────┬─────┘   └────┬─────┘
            │              │              │
            └──────────────┼──────────────┘
                           │
                    ┌──────┴──────┐
                    │   Merger    │
                    │  (Reducer)  │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
       ┌──────────┐ ┌──────────┐ ┌──────────┐
       │ Tokenizer│ │Compressor│ │ Ranker   │
       │  (GPU?)  │ │(Tree-sit)│ │(PageRank)│
       └────┬─────┘ └────┬─────┘ └────┬─────┘
            │            │            │
            └────────────┼────────────┘
                         │
                  ┌──────┴──────┐
                  │  Formatter  │
                  │ (Streaming) │
                  └──────┬──────┘
                         │
                  ┌──────┴──────┐
                  │   Output    │
                  │ (File/API)  │
                  └─────────────┘
```

### 5.2 Memory-Mapped File Processing

```rust
// Rust implementation for memory-efficient file processing
use memmap2::Mmap;
use std::fs::File;
use std::io::Result;

pub struct FileProcessor;

impl FileProcessor {
    pub fn process_large_file(path: &str) -> Result<ProcessedFile> {
        // Memory-map the file instead of loading into RAM
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // Process in streaming fashion
        let mut result = ProcessedFile::new();

        for line in mmap.split(|&b| b == b'\n') {
            result.process_line(line);
        }

        Ok(result)
    }
}
```

### 5.3 GPU-Accelerated Token Counting

```python
from infiniloom.tokenizer import GPUTokenizer

# Use GPU for batch token counting
tokenizer = GPUTokenizer(
    model="claude",
    device="cuda",  # or "mps" for Apple Silicon
    batch_size=1000,
)

# Count tokens for many files in parallel
token_counts = tokenizer.count_batch(files)
```

### 5.4 Benchmarks and Targets

| Repository Size | Files | Current Tools | Infiniloom Target |
|-----------------|-------|---------------|------------------|
| Small (<100 files) | 50 | 2-5s | <0.5s |
| Medium (100-1K) | 500 | 10-30s | <2s |
| Large (1K-10K) | 5000 | 1-5min | <15s |
| Huge (10K+) | 50000 | 10min+ | <60s (streaming) |

---

## Part 6: Distribution Strategy

### 6.1 Multi-Platform Binaries

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          # Native binaries
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    steps:
      - uses: actions/checkout@v4

      # Rust build for native targets
      - name: Build with Cargo
        run: cargo build --release --target ${{ matrix.target }}

      # Upload artifacts
      - uses: actions/upload-artifact@v4
        with:
          name: infiniloom-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/infiniloom*
```

### 6.2 Package Distribution

```bash
# PyPI (Python)
pip install infiniloom

# npm (Node.js)
npm install -g infiniloom

# Homebrew (macOS/Linux)
brew install infiniloom

# Cargo (Rust users)
cargo install infiniloom

# Nix
nix profile install nixpkgs#infiniloom

# Docker
docker run -v $(pwd):/repo ghcr.io/infiniloom/infiniloom /repo

# Direct binary
curl -fsSL https://toposlabs.ai/infiniloom/install.sh | sh
```

### 6.3 IDE Extensions

Phase 2 (not implemented intentionally): IDE integrations are planned but not implemented yet.

```json
// VSCode extension package.json
{
  "name": "infiniloom",
  "displayName": "Infiniloom",
  "description": "Pack repository context for LLMs",
  "version": "1.0.0",
  "engines": { "vscode": "^1.80.0" },
  "categories": ["Other"],
  "activationEvents": [
    "onCommand:infiniloom.pack",
    "onCommand:infiniloom.packSelection"
  ],
  "contributes": {
    "commands": [
      {
        "command": "infiniloom.pack",
        "title": "Infiniloom: Pack Repository"
      },
      {
        "command": "infiniloom.packSelection",
        "title": "Infiniloom: Pack Selected Files"
      },
      {
        "command": "infiniloom.configure",
        "title": "Infiniloom: Configure"
      }
    ],
    "configuration": {
      "title": "Infiniloom",
      "properties": {
        "infiniloom.defaultModel": {
          "type": "string",
          "default": "claude",
          "enum": ["claude", "gpt-4", "gemini", "local"],
          "description": "Default LLM model for output optimization"
        },
        "infiniloom.autoTokenCount": {
          "type": "boolean",
          "default": true,
          "description": "Show token count in status bar"
        }
      }
    }
  }
}
```

---

## Part 7: Implementation Status

The project has been fully implemented in **pure Rust** for maximum performance and maintainability. The original Zig hybrid architecture was evaluated but Rust was chosen for the entire stack.

### Core Features (Complete)

- [x] **File System Scanner**: Parallel scanning with gitignore support using `ignore` crate
- [x] **Tree-sitter AST Parsing**: 21+ languages with thread-local parsers for parallelism
- [x] **Multi-model Tokenizers**: Accurate counting via tiktoken-rs (OpenAI) + calibrated estimation (others)
- [x] **Compression Algorithms**: 6 levels (none → extreme) with semantic chunking
- [x] **PageRank Ranking**: Symbol/file importance scoring with graph analysis

### Model Optimization (Complete)

- [x] **Claude**: XML format with prompt caching hints
- [x] **GPT-4o/GPT-5**: Markdown format optimized for code blocks
- [x] **Gemini**: YAML format with query-at-end optimization
- [x] **TOON Format**: Token-efficient format (~40% smaller)
- [x] **Plain Text**: Simple format for any model

### Advanced Features (Complete)

- [x] **Incremental Processing**: File-level caching with content hash invalidation
- [x] **Security Scanning**: Secret detection with configurable patterns and redaction
- [x] **Git Context Engine**: Symbol index, diff context expansion, impact analysis
- [x] **Token Budgeting**: Smart truncation at semantic boundaries
- [x] **Watch Mode**: File system monitoring with automatic regeneration

### Bindings & CLI (Complete)

- [x] **CLI Tool**: Full-featured with all commands (pack, scan, map, index, diff, impact, chunk)
- [x] **Python Bindings**: PyO3 + Maturin
- [x] **Node.js Bindings**: NAPI-RS

### Future Enhancements

See [ROADMAP.md](planning/ROADMAP.md) for planned features including:
- IDE extensions (VS Code, JetBrains)
- MCP server integration
- Neural embeddings (CodeBERT/StarCoder)
- Plugin SDK for custom analyzers

---

## Part 8: Summary of Innovations

### What Makes Infiniloom Superior

| Innovation | Benefit | Implementation |
|------------|---------|----------------|
| **Pure Rust** | 10-100x faster than JS/Python | Rust + tree-sitter + Rayon |
| **Model Flavors** | Optimized for each LLM | Config-driven formatters |
| **Repo Map** | Efficient overview | Graph ranking algorithms |
| **Embeddings** | Semantic search | Phase 2 (not implemented intentionally) |
| **Prompt Caching** | 90% cost reduction | Structure optimization |
| **Incremental** | Fast updates | Content addressing |
| **Streaming** | Handle any size | Phase 2 (not implemented intentionally) |
| **Plugins** | Extensible | Phase 2 (not implemented intentionally) |
| **Multi-modal** | Images + code | Phase 2 (not implemented intentionally) |

### Infiniloom Competitive Moat

1. **Performance**: Pure Rust with Rayon parallelism is unmatched
2. **Intelligence**: Embeddings (Phase 2) + ranking = smart context
3. **Flexibility**: Plugin system (Phase 2) for any use case
4. **Model-Aware**: First tool optimized per-LLM
5. **Developer Experience**: Great CLI, IDE, API

---

## Appendix: Alternative Technology Deep Dives

### A.1 Why Not Pure Go?

**Pros:**
- Fast compilation
- Simple concurrency (goroutines)
- Good cross-compilation
- Large ecosystem

**Cons:**
- CGo overhead for C libraries (tree-sitter)
- GC pauses (unpredictable latency)
- Larger binary sizes
- Less control over memory layout

**Verdict:** Good for services, not ideal for performance-critical parsing.

### A.2 Why Not C++?

**Pros:**
- Maximum performance
- Tree-sitter is C (native interop)
- Mature ecosystem
- Full control

**Cons:**
- Memory safety issues
- Complex build systems
- Slow compilation
- Cross-platform challenges

**Verdict:** Too risky for a tool that processes untrusted code.

### A.3 Why Not Odin?

**Pros:**
- Clean C alternative
- Data-oriented design
- Simple syntax
- Fast compilation

**Cons:**
- Small ecosystem
- Fewer bindings
- Still in development

**Verdict:** Interesting but too immature for production.

### A.4 Why Pure Rust?

After evaluating hybrid approaches, we chose pure Rust for simplicity and performance:

**Pure Rust provides:**
- File I/O (memmap2 for mmap)
- AST parsing (tree-sitter Rust bindings)
- Token counting (tiktoken-rs for accurate BPE)
- Complex algorithms (ranking, embeddings)
- Plugin system (trait-based)
- Async networking (tokio)
- Safety-critical logic
- Parallel processing (Rayon)

**Benefits of pure Rust:**
- No FFI overhead or complexity
- Single toolchain (cargo)
- Memory safety throughout
- Excellent ecosystem
- Simpler builds and CI/CD
- Better debugging experience

This pure Rust approach gives us:
- Maximum performance (zero-cost abstractions)
- Safety guarantees (borrow checker)
- Rich ecosystem (crates.io)
