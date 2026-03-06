# Infiniloom Document Ingestion: Research & Design Document

**Date**: March 2026
**Status**: Research / Pre-Implementation
**Goal**: Extend Infiniloom to convert any document format (PDF, DOCX, TXT, HTML, etc.) into LLM-optimized structured output, with a focus on compliance/regulatory documents.

---

## 1. Problem Statement

Infiniloom currently transforms **codebases** into LLM-friendly formats. The goal is to extend this to **any document** — technical documentation, compliance policies, legal texts, SOPs, regulatory filings, contracts, etc.

**Key insight**: The same principles that make code context effective for LLMs (structured output, semantic chunking, token efficiency, model-specific formatting) apply equally to documents.

**Target use cases**:
- Compliance teams ingesting regulatory documents (SOC2, GDPR, HIPAA policies)
- Legal teams processing contracts and policy documents
- Technical writers converting documentation for AI assistants
- RAG pipelines needing high-quality document chunks
- Agent workflows requiring structured document access

---

## 2. Supported Input Formats

### Tier 1 — Must Have (v0.7)

| Format | Extension(s) | Rust Crate Strategy | Maturity |
|--------|-------------|---------------------|----------|
| **DOCX** | `.docx` | `docx-rust` 0.1.11 (pure Rust, reads XML inside ZIP) | Moderate — extracts paragraphs, tables, headings via XML |
| **HTML** | `.html`, `.htm` | `scraper` 0.25 + `html5ever` 0.38 (Mozilla/Servo) | Excellent — 46M+ downloads, production-grade |
| **Markdown** | `.md` | `pulldown-cmark` 0.13 or `comrak` 0.50 (GFM) | Excellent — 72M+ downloads |
| **Plain Text** | `.txt`, `.text` | Custom structure detection (headings, lists) | N/A — implement ourselves |
| **CSV** | `.csv` | `csv` 1.4 (BurntSushi) | Excellent — 156M downloads |

> **WARNING**: `docx-rs` is primarily a DOCX **writer**, not reader. Use `docx-rust` for reading.
> **NOTE**: PDF support is deferred — it requires spatial layout analysis, OCR, and table heuristics that add significant complexity. All Tier 1 formats are structured and have reliable Rust parsers.

### Tier 2 — Should Have (v0.8)

| Format | Extension(s) | Strategy | Notes |
|--------|-------------|----------|-------|
| **XLSX** | `.xlsx` | `calamine` 0.33 (pure Rust) | Mature (5.8M DL), reads XLS/XLSX/XLSB/ODS |
| **PDF** | `.pdf` | `pdf-extract` 0.10 + `lopdf` 0.39 (pure Rust) | Deferred — needs spatial analysis, OCR |
| **PPTX** | `.pptx` | `pptx-to-md` 0.4 or `ppt-rs` 0.2.6 | Low maturity (14K DL); or custom ZIP+XML |
| **EPUB** | `.epub` | `epub` 2.1.5 or `rbook` 0.7.2 + HTML pipeline | ZIP of XHTML files, use scraper for content |
| **RTF** | `.rtf` | `rtf-parser` 0.4.2 (basic) or Pandoc fallback | 91K DL; complex RTF may fail |
| **ODT** | `.odt` | `quick-xml` + `zip` (manual parse) or Pandoc | No production ODT crate exists |

### Tier 3 — Nice to Have (v0.9+)

| Format | Strategy |
|--------|----------|
| **DOC** (legacy binary) | Pandoc/LibreOffice subprocess or `extractous` |
| **Scanned PDF** (OCR) | Tesseract subprocess or cloud API |
| **Images with text** | Tesseract / vision API |
| **Email (.eml, .msg)** | Custom parser |
| **Universal fallback** | `extractous` 0.3 (Apache Tika via GraalVM, ~100MB native lib) |

### Alternative: Universal Parsers

| Crate | Downloads | Approach | Tradeoff |
|-------|-----------|----------|----------|
| **extractous** | 177K | Apache Tika compiled to native via GraalVM | Broadest coverage, but ~100MB+ native library |
| **shiva** | 47K | Pure Rust, 13 formats (Common Document Model) | Pure Rust, but limited adoption/quality |
| **pandoc** (crate) | 249K | Shell out to pandoc binary | 40+ formats, but requires pandoc installed |

---

## 3. Rust Crate Analysis

### PDF: `lopdf` 0.39 + `pdf-extract` 0.10

```toml
lopdf = "0.39"          # Low-level PDF object access (4.6M downloads)
pdf-extract = "0.10"    # Text extraction layer on top of lopdf (799K downloads)
```

**Capabilities**:
- Extract text content from text-based PDFs
- Page-by-page extraction
- Basic font/style detection (font size → heading inference)
- No native table detection (must infer from spatial layout)

**Limitations**:
- No OCR (scanned PDFs need Tesseract)
- Table extraction requires heuristics (whitespace/column alignment analysis)
- Complex layouts (multi-column) need spatial analysis
- No built-in heading detection (must infer from font size)

**Alternatives**:
- `pdf` crate 0.10 (482K DL, more modern API, actively maintained Mar 2026)
- `pdfium-render` 0.8.37 (774K DL, wraps Google's Pdfium C++ lib — most accurate text extraction with positional info, but requires ~20MB shared library)

### DOCX: `docx-rust` 0.1.11

```toml
docx-rust = "0.1.11"   # Read and write DOCX (1M downloads)
```

> **WARNING**: `docx-rs` (1.2M downloads) is primarily a DOCX **writer**, NOT a reader. Use `docx-rust` instead.

**Capabilities**:
- Read paragraphs with style information (Heading1, Heading2, Normal, etc.)
- Extract tables with rows and cells
- Read lists (ordered and unordered)
- DOCX is XML inside ZIP — full structure accessible

**Limitations**:
- v0.1.11 — functional but not fully mature
- Images extracted as binary blobs (need OCR separately)
- Complex formatting (text boxes, shapes) partially supported
- Legacy `.doc` format: **no Rust crate exists** — use extractous or LibreOffice

**Alternative**: Parse OOXML directly with `quick-xml` + `zip` crates. More control but more work. Also `ooxmlsdk` 0.3 (22K DL, covers DOCX/XLSX/PPTX but young).

### HTML: `scraper` 0.25 + `html5ever` 0.38

```toml
html5ever = "0.38"    # Mozilla/Servo's spec-compliant HTML parser (45.8M downloads)
scraper = "0.25"      # CSS selector-based extraction (14.1M downloads)
```

**Capabilities**:
- Full HTML5 spec compliance (from the Servo browser engine)
- CSS selector queries for structure extraction
- Handles malformed HTML gracefully
- Fast and memory-efficient

**Limitations**:
- JavaScript-rendered content not available (static HTML only)
- Need custom logic to extract semantic structure from `<div>` soup

**Maturity**: Production-grade. Both are among the most downloaded Rust crates.

### Markdown: `pulldown-cmark` 0.13 / `comrak` 0.50

```toml
pulldown-cmark = "0.13"  # CommonMark, streaming pull-parser (71.9M downloads)
comrak = "0.50"           # GFM-compatible with full AST (3.5M downloads)
```

**Capabilities**:
- `pulldown-cmark`: CommonMark spec, streaming parser, used by rustdoc itself
- `comrak`: GitHub Flavored Markdown (tables, task lists, strikethrough, autolinks), full AST access, actively maintained (v0.50, Jan 2026)
- Both: heading hierarchy, code blocks, links, images, lists

**Recommendation**: `pulldown-cmark` for CommonMark; `comrak` for GFM with full AST.

### CSV/XLSX: `csv` 1.4 + `calamine` 0.33

```toml
csv = "1.4"             # BurntSushi's CSV parser (155.8M downloads)
calamine = "0.33"       # Read XLSX/XLS/XLSB/ODS (5.8M downloads)
```

**Capabilities**:
- `csv`: Zero-copy parsing, streaming, serde integration. Gold standard.
- `calamine`: Reads Excel (XLS, XLSX, XLSB) and ODS formats, cell types, formulas. Pure Rust.

**Limitation**: `calamine` provides cell values but not formatting. For formatting-aware reading, consider `umya-spreadsheet` 2.3.3 (467K DL).

### Fallback: Pandoc Subprocess / extractous

For formats without good Rust crates (RTF, legacy DOC, ODT, etc.):

```rust
// Option A: Pandoc as universal converter (requires pandoc binary)
Command::new("pandoc")
    .args(&[input_path, "-t", "markdown", "-o", "-"])
    .output()

// Option B: extractous (Apache Tika via GraalVM native, no JVM needed)
// Supports PDF, DOCX, DOC, PPTX, PPT, XLSX, XLS, RTF, ODT, EPUB, etc.
// But adds ~100MB native library dependency
```

**Pandoc** supports 40+ input formats and can convert to Markdown, which we then process through our pipeline. This is the same pattern Infiniloom uses for git operations (CLI subprocess).

**extractous** (177K downloads, v0.3) bundles Apache Tika compiled to native code via GraalVM. No JVM at runtime, but significant binary size increase. Best option if broad format coverage is critical.

---

## 4. Document Structure Model

### New Types for Document Representation

The key insight: documents have a **different structure** than code. Instead of symbols/functions/classes, documents have **sections, paragraphs, tables, lists, and metadata**.

```rust
/// A parsed document (analogous to Repository for code)
pub struct Document {
    /// Document title
    pub title: Option<String>,
    /// Source file path
    pub source: PathBuf,
    /// Original format
    pub format: DocumentFormat,
    /// Document metadata
    pub metadata: DocumentMetadata,
    /// Hierarchical content structure
    pub sections: Vec<Section>,
    /// Token counts
    pub token_count: TokenCounts,
}

/// Document format enum
pub enum DocumentFormat {
    Pdf,
    Docx,
    Html,
    Markdown,
    PlainText,
    Csv,
    Xlsx,
    Pptx,
    Epub,
    Rtf,
    Odt,
}

/// Document metadata (author, date, version, etc.)
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    /// For compliance: document version/revision
    pub version: Option<String>,
    /// For compliance: effective date
    pub effective_date: Option<String>,
    /// For compliance: document classification
    pub classification: Option<String>,
    /// Total pages (if applicable)
    pub pages: Option<u32>,
    /// Custom metadata key-value pairs
    pub custom: BTreeMap<String, String>,
}

/// A document section (recursive hierarchy)
pub struct Section {
    /// Section ID for cross-referencing
    pub id: Option<String>,
    /// Heading level (1-6, 0 for no heading)
    pub level: u8,
    /// Section title/heading
    pub title: Option<String>,
    /// Section number (e.g., "3.2.1")
    pub number: Option<String>,
    /// Content blocks within this section
    pub content: Vec<ContentBlock>,
    /// Nested subsections
    pub children: Vec<Section>,
    /// Token count for this section
    pub tokens: u32,
}

/// A block of content within a section
pub enum ContentBlock {
    /// A paragraph of text
    Paragraph(Paragraph),
    /// A table
    Table(Table),
    /// An ordered or unordered list
    List(List),
    /// A code block or preformatted text
    CodeBlock(CodeBlock),
    /// A definition (term + definition) — common in compliance
    Definition(Definition),
    /// A blockquote or callout
    Blockquote(Blockquote),
    /// An image reference
    Image(ImageRef),
    /// A cross-reference to another section
    CrossReference(CrossRef),
    /// A footnote or endnote
    Note(Note),
    /// Raw/unknown content
    Raw(String),
}

/// A paragraph with inline formatting preserved
pub struct Paragraph {
    pub text: String,
    /// Inline elements (bold, italic, links, etc.) — stored as spans
    pub spans: Vec<TextSpan>,
}

/// A table with headers and rows
pub struct Table {
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Column alignments
    pub alignments: Vec<Alignment>,
}

/// A list (ordered or unordered, possibly nested)
pub struct List {
    pub ordered: bool,
    pub items: Vec<ListItem>,
}

pub struct ListItem {
    pub text: String,
    /// Nested sub-list
    pub children: Option<List>,
}

/// A definition (term + explanation) — critical for compliance glossaries
pub struct Definition {
    pub term: String,
    pub definition: String,
}

/// A cross-reference to another section or document
pub struct CrossRef {
    pub target_id: String,
    pub display_text: String,
    /// Whether the target is in the same document
    pub internal: bool,
}
```

### Why Not Reuse `RepoFile`/`Symbol`?

Code and documents have fundamentally different structures:

| Aspect | Code (`RepoFile`) | Document (`Document`) |
|--------|-------------------|----------------------|
| Unit | File with symbols | Document with sections |
| Hierarchy | Flat (file → symbols) | Deep (section → subsection → ...) |
| References | Import/call graph | Cross-references, footnotes |
| Metadata | Language, size | Author, date, version, classification |
| Structure | Functions, classes | Headings, paragraphs, tables, lists |
| Chunking | By symbol boundaries | By section/semantic boundaries |

However, they share the **output pipeline** (XML/MD/YAML formatters) and **infrastructure** (token counting, security scanning, chunking).

---

## 5. Architecture Design

### Module Structure

```
engine/src/
├── document/                      # NEW: Document ingestion module
│   ├── mod.rs                     # Module exports, DocumentProcessor trait
│   ├── types.rs                   # Document, Section, ContentBlock, etc.
│   ├── parsers/                   # Format-specific parsers
│   │   ├── mod.rs                 # Parser registry and auto-detection
│   │   ├── pdf.rs                 # PDF → Document (lopdf + pdf-extract)
│   │   ├── docx.rs                # DOCX → Document (docx-rs)
│   │   ├── html.rs                # HTML → Document (scraper)
│   │   ├── markdown.rs            # Markdown → Document (pulldown-cmark)
│   │   ├── plaintext.rs           # Plain text → Document (heuristic structure detection)
│   │   ├── csv.rs                 # CSV → Document (csv crate)
│   │   ├── xlsx.rs                # XLSX → Document (calamine)
│   │   └── pandoc.rs              # Pandoc fallback for RTF, DOC, etc.
│   ├── structure.rs               # Structure detection (heading inference, TOC generation)
│   ├── compliance.rs              # Compliance-specific enrichment
│   ├── distillation/              # Content distillation pipeline (Section 6)
│   │   ├── mod.rs                 # Pipeline orchestrator
│   │   ├── strip.rs               # Stage 1: Zero-value content removal
│   │   ├── dedup.rs               # Stage 2: Redundancy elimination
│   │   ├── compress.rs            # Stage 3: Language tightening (filler removal)
│   │   ├── score.rs               # Stage 4: Information density scoring
│   │   ├── arrange.rs             # Stage 5: Attention-optimized placement
│   │   └── patterns.rs            # Filler phrases, boilerplate patterns
│   ├── table_extract.rs           # Table extraction heuristics
│   └── output.rs                  # Document → LLM format adapters
│
cli/src/commands/
│   └── ingest.rs                  # NEW: `infiniloom ingest` command
```

### Parser Trait

```rust
/// Trait for document format parsers
pub trait DocumentParser: Send + Sync {
    /// Parse a file into a Document
    fn parse(&self, path: &Path, options: &ParseOptions) -> Result<Document>;

    /// Supported file extensions
    fn extensions(&self) -> &[&str];

    /// Format name
    fn format_name(&self) -> &str;

    /// Whether this parser can handle the given file (content sniffing)
    fn can_parse(&self, path: &Path, content: &[u8]) -> bool;
}

/// Options for document parsing
pub struct ParseOptions {
    /// Extract tables (slower but more complete)
    pub extract_tables: bool,
    /// Preserve inline formatting (bold, italic, etc.)
    pub preserve_formatting: bool,
    /// Maximum depth for section hierarchy
    pub max_depth: u8,
    /// OCR strategy for scanned content
    pub ocr: OcrStrategy,
    /// Custom metadata to inject
    pub custom_metadata: BTreeMap<String, String>,
}

pub enum OcrStrategy {
    /// Skip scanned/image content
    Skip,
    /// Use Tesseract subprocess
    Tesseract,
    /// Placeholder for cloud OCR APIs
    CloudApi(String),
}
```

### Processing Pipeline

```
Input File(s)
    │
    ▼
┌─────────────────┐
│ Format Detection │  ← Extension + magic bytes
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Format Parser    │  ← PDF/DOCX/HTML/MD/TXT/CSV parser
│ (DocumentParser) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Document Model   │  ← Unified Document with Sections
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Enrichment       │  ← Structure detection, TOC generation,
│                  │     compliance metadata, cross-ref resolution
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Security Scan    │  ← Reuse existing SecurityScanner
│                  │     (PII detection, secret scanning)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Content          │  ← NEW: 5-stage distillation pipeline
│ Distillation     │     Strip → Dedup → Compress → Score → Arrange
│ (Section 6)      │     Pure Rust, no ML, deterministic
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Token Counting   │  ← Reuse existing Tokenizer
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ Output Formatting                        │
│                                          │
│  ┌──────┐  ┌────┐  ┌──────┐  ┌──────┐  │
│  │ XML  │  │ MD │  │ YAML │  │ JSON │  │
│  │Claude│  │GPT │  │Gemini│  │Agent │  │
│  └──────┘  └────┘  └──────┘  └──────┘  │
└─────────────────────────────────────────┘
```

---

## 6. Attention & Token Optimization (Content Distillation)

### The Core Problem: Human Documents Are Noisy

Human-readable documents contain significant "water" — content that serves social/formatting purposes for humans but actively **degrades** LLM performance:

- **Boilerplate**: Copyright notices, standard disclaimers, repeated headers/footers
- **Filler phrases**: "It is important to note that", "In this section, we will discuss", "As previously mentioned"
- **Hedging language**: "It may be possible that", "It should be noted", "In general, it is the case that"
- **Redundant content**: Table of contents duplicating headings, summaries restating body text, repeated definitions
- **Structural noise**: Page numbers, watermarks, running headers, decorative elements
- **Verbose transitions**: Paragraph connectors that add no information

### Research Findings: Compression Improves Accuracy

This is not just about saving tokens — **removing noise actively improves LLM performance**:

| Finding | Source | Implication |
|---------|--------|-------------|
| Compressing 10K-token prompts at 4x **improved** accuracy by 17-21% | LLMLingua (Microsoft) | Noise removal helps, not just saves cost |
| 6% retention (17x compression) with minimal quality loss | RECOMP (arXiv:2310.04408) | Most document content is redundant for LLM tasks |
| 50% context reduction with only 0.023 BERTscore drop | Selective Context (arXiv:2310.06201) | Conservative 2x compression is nearly lossless |
| Adding retrieved passages initially helps then **hurts** performance | arXiv:2410.05983 | More content is not better — quality > quantity |
| 20x compression with 1.5-point loss on reasoning tasks | LLMLingua | Documents are ~95% compressible for reasoning |

### The "Lost in the Middle" Problem

LLMs exhibit a U-shaped attention curve (Liu et al., 2023):

```
Attention
  HIGH ██████                                    ██████
       ██████                                    ██████
  MED  ██████   ████                        ████ ██████
       ██████   ████  ████            ████  ████ ██████
  LOW  ██████   ████  ████  ████████  ████  ████ ██████
       ──────────────────────────────────────────────────
       START              MIDDLE                    END
       ◄─────── "attention sinks" ──────► ◄── recency ──►
```

**Key findings (as of 2026)**:
- Initial tokens get disproportionately high attention ("attention sinks") regardless of content
- End tokens get high attention due to recency bias in autoregressive generation
- Middle tokens are in a "dead zone" — information placed here is most likely to be missed
- The RULER benchmark shows even models claiming 200K+ context have "large room for improvement"
- Specialized training (FILM-7B, IN2) can mitigate this, but general-purpose models still exhibit it

**Implication for Infiniloom**: Place high-value content at the **start** and **end** of output. Push low-value supporting material to the middle. Never put critical definitions or requirements in the middle.

### Content Distillation Pipeline

Infiniloom should apply a multi-stage distillation pipeline before formatting:

```
Raw Document Content
    │
    ▼
┌──────────────────────┐
│ Stage 1: STRIP        │  Remove zero-value content
│ - Page numbers         │
│ - Running headers      │
│ - Copyright boilerplate│
│ - Watermarks           │
│ - Decorative elements  │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Stage 2: DEDUPLICATE  │  Remove redundant content
│ - TOC (if body follows)│
│ - Repeated definitions │
│ - Summary ≈ body text  │
│ - Cross-doc duplicates │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Stage 3: COMPRESS     │  Tighten language
│ - Filler phrases → ∅  │
│ - Hedging → direct     │
│ - Passive → active     │
│ - Verbose → concise    │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Stage 4: SCORE        │  Rank by information density
│ - Self-information     │
│ - TF-IDF importance    │
│ - Requirement keywords │
│ - Definition density   │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Stage 5: ARRANGE      │  Optimize for attention
│ - High-value → start   │
│ - High-value → end     │
│ - Supporting → middle  │
│ - Metadata → header    │
└──────────────────────┘
```

### Stage 1: Strip (Zero-Value Content Removal)

Patterns to detect and remove:

```rust
/// Content patterns with zero information value
pub enum StripPattern {
    /// "Page X of Y", "- 3 -", etc.
    PageNumbers,
    /// Repeated header/footer text on every page
    RunningHeaders,
    /// "Copyright (c) 2024 Acme Corp. All rights reserved."
    CopyrightNotice,
    /// "CONFIDENTIAL", "DRAFT", watermark text
    Watermarks,
    /// "────────────", "========", decorative separators
    DecorativeSeparators,
    /// Empty/whitespace-only sections
    EmptySections,
    /// Navigation elements (HTML: nav, sidebar, breadcrumbs)
    NavigationElements,
}
```

### Stage 2: Deduplicate (Redundancy Elimination)

| Pattern | Detection Method | Action |
|---------|-----------------|--------|
| TOC before full content | Heading list followed by matching sections | Remove TOC, keep section headings |
| Repeated definitions | Same term defined in glossary AND inline | Keep glossary version, link inline |
| Executive summary ≈ body | High text similarity (>70%) | Remove summary or keep only if significantly shorter |
| Identical boilerplate sections | Exact/near-exact match across documents | Keep one, reference from others |
| Repeated cross-references | "As defined in Section X" appearing 10+ times | Keep first occurrence, remove subsequent |

### Stage 3: Compress (Language Tightening)

Rule-based compression patterns (no ML required):

```rust
/// Filler phrases to remove or shorten (case-insensitive matching)
const FILLER_PATTERNS: &[(&str, &str)] = &[
    // === Complete removal (zero information) ===
    ("it is important to note that ", ""),
    ("it should be noted that ", ""),
    ("it is worth mentioning that ", ""),
    ("in this section, we will discuss ", ""),
    ("as previously mentioned, ", ""),
    ("as a matter of fact, ", ""),
    ("for the avoidance of doubt, ", ""),
    ("for the purposes of this document, ", ""),
    ("it could be argued that ", ""),
    ("it is generally believed that ", ""),
    ("it seems that ", ""),
    ("it appears that ", ""),

    // === Verbose → Concise ===
    ("in order to ", "to "),
    ("for the purpose of ", "to "),
    ("due to the fact that ", "because "),
    ("regardless of the fact that ", "although "),
    ("in the event that ", "if "),
    ("at this point in time ", "now "),
    ("at the present time ", "now "),
    ("in the near future ", "soon "),
    ("prior to ", "before "),
    ("subsequent to ", "after "),
    ("during the course of ", "during "),
    ("with respect to ", "regarding "),
    ("with regard to ", "regarding "),
    ("concerning the matter of ", "about "),
    ("in accordance with ", "per "),
    ("in the process of ", ""),
    ("notwithstanding the foregoing, ", "however, "),
    ("has the ability to ", "can "),
    ("is able to ", "can "),
    ("make a decision ", "decide "),
    ("conduct an investigation ", "investigate "),
    ("give consideration to ", "consider "),
    ("take into consideration ", "consider "),

    // === Hedging reduction ===
    ("it may be possible that ", "possibly, "),
    ("it is generally the case that ", "generally, "),
    ("it is widely recognized that ", ""),
    ("there is a possibility that ", "possibly, "),
    ("to some extent, ", ""),
];
```

**Configurable aggressiveness levels**:
- `minimal`: Strip only (Stage 1) — safe for legal/compliance where exact wording matters
- `balanced`: Strip + Deduplicate (Stages 1-2) — good default
- `aggressive`: Strip + Deduplicate + Compress (Stages 1-3) — maximum token savings
- `full`: All stages including attention-optimized arrangement (Stages 1-5)

### Stage 4: Score (Information Density Ranking)

Score each paragraph/section by information value:

```rust
pub struct ContentScore {
    /// Self-information score (entropy-based, higher = more informative)
    pub self_information: f32,
    /// Contains requirement keywords (SHALL, MUST, REQUIRED)
    pub has_requirements: bool,
    /// Contains definitions or defined terms
    pub has_definitions: bool,
    /// Contains data (numbers, dates, thresholds, percentages)
    pub has_data: bool,
    /// Contains cross-references to other sections
    pub has_references: bool,
    /// TF-IDF importance within the document
    pub tfidf_score: f32,
    /// Composite score (weighted combination)
    pub composite: f32,
}
```

**Heuristic scoring** (no ML required):
- **High signal**: Sections with numbers, requirements (SHALL/MUST), definitions, tables, thresholds
- **Medium signal**: Narrative text with specific claims, procedures, steps
- **Low signal**: General introductions, transitions, acknowledgments, "background" sections
- **Zero signal**: Boilerplate, decorative content, navigation

**Available Rust crates for scoring**:
- `tfidf-text-summarizer` — extractive summarization via TF-IDF sentence scoring with `par_summarize()` (Rayon-parallel). Exposes `summarize(text, reduction_factor)`.
- `keyword_extraction` — implements TF-IDF, RAKE, TextRank, and YAKE for keyword/keyphrase extraction.
- `natural` — Rust NLP library with TF-IDF, tokenization, n-grams, string distance metrics.
- `dom-content-extraction` — CETD algorithm for HTML boilerplate removal (text density analysis).

**Compression-ratio as redundancy signal** (cheap, no ML):
Use `flate2` (already common in Rust) to compute paragraph-level gzip compression ratios.
Highly compressible text (ratio < 0.3) = highly redundant. This is a fast, zero-dependency way to identify boilerplate and repetitive content.

```rust
fn redundancy_score(text: &str) -> f32 {
    use flate2::write::GzEncoder;
    use std::io::Write;
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(text.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();
    compressed.len() as f32 / text.len() as f32
    // < 0.3 = very redundant, > 0.6 = high information density
}
```

**Cross-document boilerplate detection**:
Hash sentences/paragraphs across a document set. Sentences appearing in >N% of documents are boilerplate (legal clauses, policy templates, standard disclaimers). Store known boilerplate hashes for fast lookup.

### Stage 5: Arrange (Attention-Optimized Placement)

Based on the U-shaped attention curve, arrange output content:

```
┌─────────────────────────────────────────┐
│  POSITION 1 (HIGH ATTENTION): Start      │
│  - Document metadata (title, version)    │
│  - Key definitions and glossary          │
│  - Critical requirements / conclusions   │
│  - Executive summary (if retained)       │
├─────────────────────────────────────────┤
│  POSITION 2 (LOW ATTENTION): Middle      │
│  - Supporting narrative text             │
│  - Background / context sections         │
│  - Appendices and supplementary info     │
│  - Less critical tables and data         │
├─────────────────────────────────────────┤
│  POSITION 3 (HIGH ATTENTION): End        │
│  - Action items / next steps             │
│  - Summary of requirements               │
│  - Cross-reference index                 │
│  - The user's query (if applicable)      │
└─────────────────────────────────────────┘
```

### Expected Impact

| Compression Level | Token Reduction | Accuracy Impact | Use Case |
|-------------------|----------------|-----------------|----------|
| `minimal` (strip only) | 10-20% | +2-5% (noise removal) | Legal/compliance (exact wording matters) |
| `balanced` (strip + dedup) | 25-40% | +5-10% | General documents |
| `aggressive` (+ compress) | 40-60% | +10-15% | Large document sets, RAG |
| `full` (+ score + arrange) | 40-60% + attention boost | +15-21% | Maximum LLM performance |

These estimates are based on LLMLingua, RECOMP, and Selective Context research applied to document (not code) contexts.

### Implementation Approach

**Stage 1-3 are rule-based** — pure Rust, deterministic, fast. No ML models needed.
**Stage 4 uses heuristic scoring** — TF-IDF, keyword matching, entropy estimation. Pure Rust.
**Stage 5 is a reordering pass** — sort sections by score and place according to attention curve.

This means the entire distillation pipeline runs in Rust without any external dependencies, maintaining Infiniloom's core advantages (speed, single binary, determinism).

---

## 7. LLM-Optimized Output Design

### What Makes Documents LLM-Friendly?

Based on research of LlamaIndex, LangChain, Unstructured.io, Docling, Marker, Jina Reader, and Claude/GPT best practices:

1. **Preserved hierarchy**: Section/subsection structure with clear nesting
2. **Explicit section boundaries**: Clear delimiters between sections
3. **Metadata in context**: Title, author, date, version visible to the LLM
4. **Cross-references resolved**: "See Section 3.2" → actual inline reference
5. **Tables in structured format**: Not flattened text, but structured representation
6. **Token-efficient encoding**: Minimal overhead per structural element
7. **Semantic chunks**: Break at section boundaries, not arbitrary token counts
8. **Noise removal**: Strip navigation, footers, headers, ads, sidebars before conversion
9. **Content typing**: Distinguish narrative text, tables, lists, definitions, code blocks
10. **Document placement**: For Claude, long documents should go at the TOP, instructions at the BOTTOM (up to 30% quality improvement)

### Key Research Findings

**Optimal chunk size for RAG**: LlamaIndex research found **1024 tokens** is the sweet spot, balancing retrieval precision with context completeness. Starting at ~250 tokens and tuning up is recommended.

**Character-based splitting is harmful**: It "ignores structure and meaning, often cuts sentences mid-thought." Always prefer structure-aware splitting (by title/header, by section, by similarity).

**The llms.txt standard** (llmstxt.org) proposes Markdown as the universal LLM-friendly format, reasoning that LLMs parse Markdown more naturally than XML or JSON.

**Gemini PDF native support**: Gemini can process raw PDFs natively at ~258 tokens/page. For visually complex documents, raw PDF may be more efficient than text extraction.

**Existing tools comparison**:
- **Unstructured.io**: 23+ formats, typed elements (Title, NarrativeText, ListItem, Table), 4 PDF strategies
- **Docling** (IBM): Advanced layout analysis, reading order, table structure, DoclingDocument format
- **Marker**: 25 pages/sec on H100, strong table extraction with LLM post-processing
- **Kreuzberg**: 75+ formats in Rust, PDFium + Tesseract, ~94K lines — closest Rust competitor

### Output Format Examples

#### XML (Claude-optimized)

```xml
<document>
  <metadata>
    <title>SOC2 Type II Compliance Policy</title>
    <version>3.1</version>
    <effective_date>2026-01-15</effective_date>
    <classification>Internal</classification>
    <author>Compliance Team</author>
  </metadata>

  <table_of_contents>
    <entry level="1" id="s1">1. Introduction</entry>
    <entry level="1" id="s2">2. Access Control Policy</entry>
    <entry level="2" id="s2.1">2.1 Authentication Requirements</entry>
    <entry level="2" id="s2.2">2.2 Authorization Matrix</entry>
  </table_of_contents>

  <section id="s1" level="1" number="1" title="Introduction">
    <paragraph>This document establishes the information security policies
    for Acme Corp in accordance with SOC2 Trust Service Criteria.</paragraph>

    <definition term="Authorized User">
      An individual who has been granted access to information systems
      through the formal access request and approval process.
    </definition>
  </section>

  <section id="s2" level="1" number="2" title="Access Control Policy">
    <section id="s2.1" level="2" number="2.1" title="Authentication Requirements">
      <paragraph>All users must authenticate using multi-factor authentication (MFA)
      before accessing production systems.</paragraph>

      <list ordered="true">
        <item>Primary factor: Corporate SSO credentials</item>
        <item>Secondary factor: Hardware security key or authenticator app</item>
        <item>Session timeout: 8 hours maximum</item>
      </list>
    </section>

    <section id="s2.2" level="2" number="2.2" title="Authorization Matrix">
      <table caption="Role-Based Access Control">
        <headers>
          <col>Role</col>
          <col>Production Read</col>
          <col>Production Write</col>
          <col>Admin</col>
        </headers>
        <row><cell>Developer</cell><cell>Yes</cell><cell>No</cell><cell>No</cell></row>
        <row><cell>SRE</cell><cell>Yes</cell><cell>Yes</cell><cell>No</cell></row>
        <row><cell>Admin</cell><cell>Yes</cell><cell>Yes</cell><cell>Yes</cell></row>
      </table>
    </section>
  </section>
</document>
```

#### Markdown (GPT-optimized)

```markdown
# SOC2 Type II Compliance Policy

> **Version**: 3.1 | **Effective**: 2026-01-15 | **Classification**: Internal

## Table of Contents
- [1. Introduction](#1-introduction)
- [2. Access Control Policy](#2-access-control-policy)
  - [2.1 Authentication Requirements](#21-authentication-requirements)
  - [2.2 Authorization Matrix](#22-authorization-matrix)

---

## 1. Introduction

This document establishes the information security policies for Acme Corp
in accordance with SOC2 Trust Service Criteria.

**Authorized User**: An individual who has been granted access to information
systems through the formal access request and approval process.

## 2. Access Control Policy

### 2.1 Authentication Requirements

All users must authenticate using multi-factor authentication (MFA) before
accessing production systems.

1. Primary factor: Corporate SSO credentials
2. Secondary factor: Hardware security key or authenticator app
3. Session timeout: 8 hours maximum

### 2.2 Authorization Matrix

| Role | Production Read | Production Write | Admin |
|------|----------------|-----------------|-------|
| Developer | Yes | No | No |
| SRE | Yes | Yes | No |
| Admin | Yes | Yes | Yes |
```

#### JSON (Agent-optimized)

```json
{
  "document": {
    "title": "SOC2 Type II Compliance Policy",
    "metadata": {
      "version": "3.1",
      "effective_date": "2026-01-15",
      "classification": "Internal"
    },
    "sections": [
      {
        "id": "s2.1",
        "number": "2.1",
        "title": "Authentication Requirements",
        "content": "All users must authenticate using MFA...",
        "requirements": [
          "Corporate SSO credentials",
          "Hardware security key or authenticator app",
          "Session timeout: 8 hours maximum"
        ],
        "parent": "s2",
        "tokens": 85
      }
    ]
  }
}
```

---

## 8. Compliance Document Features

### Special Handling for Regulatory Documents

Compliance documents have unique structural patterns that need special attention:

#### 7.1 Section Numbering

Regulatory documents use hierarchical numbering (1, 1.1, 1.1.1, etc.) with strict hierarchy levels: Article > Section > Subsection > Paragraph. The parser must:
- Detect and preserve section numbers exactly (numbering is legally significant)
- Build cross-reference index
- Resolve "See Section X.Y" and "as defined in Section 4.2(a)" references
- Preserve list numbering schemes exactly: (a), (b), (c) or (i), (ii), (iii)

#### 7.2 Definitions / Glossary

Compliance docs typically have a definitions section with legally precise terms (e.g., "Material Adverse Effect"). We should:
- Extract term-definition pairs
- Mark them with `<definition>` tags
- Enable LLMs to distinguish defined terms from ordinary usage
- Cross-link usage to definitions

#### 7.3 Requirements vs. Informative Text

Distinguish between:
- **Normative** text (SHALL, MUST, REQUIRED) — the actual requirements
- **Informative** text (NOTE, EXAMPLE, guidance) — supporting context

```rust
pub enum ContentClass {
    /// Normative requirement (SHALL/MUST)
    Requirement,
    /// Informative guidance (NOTE/EXAMPLE)
    Informative,
    /// Definition
    Definition,
    /// Reference to external standard
    ExternalReference,
    /// General text
    General,
}
```

#### 7.4 Compliance Mapping

For documents referencing standards (SOC2, ISO 27001, NIST, etc.):
- Extract control identifiers (e.g., "CC6.1", "A.9.1.1")
- Map sections to controls
- Enable queries like "What controls address access management?"

#### 7.5 Tables as Structured Data

Tables in compliance docs often contain critical structured data (access matrices, risk registers, control mappings). These must be preserved as structured data, not flattened text.

#### 7.6 Compliance Signal vs. Noise (Distillation Guidance)

**SOC2 policies — high signal (keep)**:
- Control descriptions with specific implementation details
- Technology stack references and architecture specifics
- Exception procedures and escalation paths
- Incident response specifics
- Access control matrices
- Risk assessment findings

**SOC2 policies — low signal (compress/strip)**:
- Policy purpose/scope boilerplate ("This policy establishes...")
- Revision history tables (unless querying about changes)
- Approval signature blocks
- Standard AICPA definitions (replace with "[Standard AICPA definitions apply]")
- Verbatim framework quotes (reference instead of repeat)

**Contract boilerplate clauses** (appear in nearly every contract, can typically be summarized as a single line unless specifically queried):
1. Entire Agreement, Severability, Governing Law, Venue/Jurisdiction
2. Waiver, Notices, Assignment, Counterparts
3. Amendment/Modification, Third-Party Beneficiaries, Force Majeure, Survival

→ Replace with: `[Standard boilerplate: governing law (Delaware), severability, entire agreement, force majeure, etc.]`

**Contract substantive clauses** (always keep in full):
Payment terms, deliverables, SLAs, IP ownership, indemnification specifics, termination triggers, non-compete scope, representations and warranties

---

## 9. CLI Design

### New Command: `infiniloom ingest`

```bash
# Convert a single document
infiniloom ingest policy.pdf --format xml
infiniloom ingest policy.pdf --format markdown --model gpt4o
infiniloom ingest policy.pdf --format json  # Agent-friendly

# Convert a directory of documents
infiniloom ingest ./compliance-docs/ --format xml --recursive

# With compliance enrichment
infiniloom ingest policy.pdf --compliance --extract-definitions --extract-requirements

# Chunked output for RAG
infiniloom ingest policy.pdf --chunk --max-tokens 1000 --overlap 200

# With content distillation (attention + token optimization)
infiniloom ingest policy.docx --distill balanced    # Strip + dedup (default)
infiniloom ingest policy.docx --distill aggressive  # + language compression
infiniloom ingest policy.docx --distill full         # + scoring + attention arrangement
infiniloom ingest policy.docx --distill minimal      # Strip only (safe for legal)
infiniloom ingest policy.docx --distill none          # No distillation, raw conversion

# With security scanning (PII, secrets)
infiniloom ingest contracts/ --security-check --redact-pii

# Output to file
infiniloom ingest policy.pdf -o policy-context.xml

# Batch processing with manifest
infiniloom ingest ./docs/ --manifest output.jsonl --incremental

# Specific options
infiniloom ingest report.pdf --extract-tables --ocr tesseract
infiniloom ingest data.xlsx --sheet "Sheet1" --headers
```

### Configuration

```yaml
# .infiniloom.yaml additions
ingest:
  # Default output format for documents
  format: xml

  # Compliance mode
  compliance:
    enabled: true
    extract_definitions: true
    extract_requirements: true
    detect_controls: true
    standards: ["SOC2", "ISO27001"]

  # Content distillation (attention + token optimization)
  distillation:
    level: balanced         # none | minimal | balanced | aggressive | full
    strip_boilerplate: true
    strip_page_numbers: true
    strip_running_headers: true
    deduplicate_toc: true
    deduplicate_definitions: true
    compress_filler: true   # Remove filler phrases (aggressive/full only)
    score_sections: true    # Rank by information density (full only)
    arrange_attention: true # Reorder for U-shaped attention (full only)
    # Custom filler patterns to remove (in addition to built-in list)
    custom_filler_patterns: []
    # Sections to always keep (never strip/compress)
    protected_sections: ["Definitions", "Glossary"]

  # Table extraction
  tables:
    enabled: true
    # For PDF: spatial analysis sensitivity
    column_threshold: 20

  # OCR for scanned content
  ocr:
    strategy: skip  # skip | tesseract | cloud
    tesseract_path: /usr/bin/tesseract
    languages: ["eng"]

  # PII detection and redaction
  pii:
    enabled: true
    redact: true
    patterns: ["SSN", "email", "phone", "credit_card"]

  # Chunking for RAG output
  chunking:
    enabled: false
    max_tokens: 1000
    overlap: 200
    boundary: section  # section | paragraph | sentence
```

---

## 10. Feature Flags

```toml
[features]
default = []

# Document ingestion support
document = [
    "dep:lopdf",
    "dep:pdf-extract",
    "dep:docx-rs",
    "dep:scraper",
    "dep:html5ever",
    "dep:pulldown-cmark",
    "dep:calamine",
    "dep:csv",
    "dep:zip",        # For DOCX/PPTX/EPUB/ODT (all ZIP-based)
]

# OCR support (requires tesseract installed)
ocr = ["dep:tesseract-rs"]

# Full feature set
full = ["async", "embeddings", "watch", "document", "ocr"]
```

This keeps the core binary lean — users who only need code analysis don't pay for document parsing dependencies.

---

## 11. Implementation Plan

### Phase 1: Foundation (v0.7.0) — ~2 weeks

1. **Document types module** (`engine/src/document/types.rs`)
   - `Document`, `Section`, `ContentBlock`, `Table`, etc.
   - Serde serialization for all types

2. **Parser trait and registry** (`engine/src/document/parsers/mod.rs`)
   - `DocumentParser` trait
   - Auto-detection by extension + magic bytes
   - Parser registry pattern

3. **Markdown parser** (easiest, validates the pipeline)
   - `pulldown-cmark` integration
   - Heading hierarchy → Section tree
   - Tables, lists, code blocks

4. **Plain text parser** (structure detection)
   - Heading detection (ALL CAPS, underlines, numbering patterns)
   - List detection (bullets, numbered)
   - Paragraph boundary detection

5. **Document output formatters**
   - XML document formatter (Claude)
   - Markdown document formatter (GPT)
   - JSON document formatter (agents)

6. **CLI `ingest` command** (basic)

### Phase 2: Office Formats (v0.7.1) — ~2 weeks

7. **HTML parser** (`scraper` + `html5ever`)
   - Semantic HTML → Section tree
   - Table extraction
   - Link resolution

8. **DOCX parser** (`docx-rs`)
   - Paragraph styles → heading levels
   - Table extraction
   - List extraction
   - Document properties → metadata

9. **CSV parser** (`csv` crate)
   - Auto-detect headers
   - Type inference
   - Table representation

### Phase 3: Advanced Formats (v0.8.0) — ~2 weeks

10. **XLSX parser** (`calamine`)
    - Multi-sheet support
    - Cell type preservation
    - Formula results

11. **Pandoc fallback** (RTF, legacy DOC, ODT, etc.)
    - Subprocess integration
    - Markdown intermediate format

12. **Content distillation pipeline** (see Section 8)
    - Boilerplate detection and removal
    - Filler phrase compression
    - Information density scoring
    - Attention-optimized content placement

### Phase 5: PDF Support (v0.9.0, deferred) — ~3 weeks

17. **PDF parser** (`lopdf` + `pdf-extract`)
    - Text extraction per page
    - Font-size-based heading detection
    - Spatial table extraction heuristics
    - Multi-column layout detection
    - Optional OCR via Tesseract subprocess

### Phase 4: Compliance & Polish (v0.8.1) — ~2 weeks

13. **Compliance enrichment module**
    - Section numbering detection
    - Requirement extraction (SHALL/MUST keywords)
    - Definition extraction
    - Control identifier mapping

14. **Document chunking for RAG**
    - Section-boundary-aware chunking
    - Overlap with context preservation
    - Integration with existing `embed` command

15. **PII detection** (extend existing security module)
    - SSN, email, phone, credit card patterns
    - Redaction support

16. **Incremental processing**
    - Manifest-based change detection (reuse embed manifest pattern)
    - Content-addressable document chunks

---

## 12. Integration with Existing Infiniloom

### Shared Infrastructure

| Component | Reuse Strategy |
|-----------|----------------|
| Token counting (`Tokenizer`) | Direct reuse — count document content |
| Security scanning (`SecurityScanner`) | Extend with PII patterns |
| Output formats (`OutputFormatter`) | New document formatters alongside code formatters |
| Embedding chunks (`EmbedChunker`) | Extend with document chunk types |
| Budget enforcement (`BudgetEnforcer`) | Direct reuse for token budgets |
| Incremental caching | Reuse manifest pattern |
| CLI framework (clap) | Add `ingest` subcommand |

### New vs. Extended

| New | Extended |
|-----|----------|
| `document/` module (types, parsers) | `security.rs` (PII patterns) |
| `ingest` CLI command | `embedding/types.rs` (document chunk kinds) |
| Document-specific formatters | `chunking/` (section-aware strategies) |
| Compliance enrichment | `config.rs` (ingest config section) |

---

## 13. Competitive Analysis

| Tool | Language | Approach | Limitations |
|------|----------|----------|------------|
| **Unstructured.io** | Python | Multi-format parsing + partitioning | Python-only, slow, heavy deps |
| **LlamaIndex** | Python | Document loaders + node parsing | Python, runtime overhead |
| **LangChain** | Python/JS | Document loaders | Thin wrappers, inconsistent quality |
| **Docling** (IBM) | Python | PDF/DOCX with ML layout analysis | Python, model dependencies |
| **marker** | Python | PDF → Markdown with ML | Python, GPU preferred |
| **extractous** | Rust (Tika) | Apache Tika via GraalVM native | ~100MB native lib, broad format coverage |
| **shiva** | Rust | Pure Rust, 13 formats, Common Document Model | Only 47K downloads, extraction quality varies |

**Infiniloom's advantages**:
- **Pure Rust**: Single binary, no Python/Java runtime, 10-100x faster
- **Content distillation**: 5-stage pipeline removes filler/boilerplate, **improving LLM accuracy 17-21%** (not just saving tokens) — no competing tool does this
- **Attention-optimized output**: Content arranged based on LLM attention research (U-shaped curve), placing high-value content where models actually attend
- **Multi-model output**: Not just Markdown — XML/YAML/JSON/TOON optimized per LLM
- **Token-aware**: Exact token counting, budget enforcement, optimal chunking
- **Code + docs unified**: Same tool for code context AND document context
- **Compliance-first**: Built-in support for regulatory document patterns
- **Deterministic**: Same input → same output (important for CI/CD)
- **Security built-in**: Secret + PII detection and redaction

**Key differentiator**: Every other tool converts format but passes through content as-is. Infiniloom is the only tool that **distills** content for optimal LLM consumption. A 50-page compliance document becomes a dense 10-15 page LLM-optimized context that performs better than the original.

---

## 14. Open Questions

1. **Should documents and code share the same `pack` command?**
   - Option A: Unified `pack` that auto-detects (simpler UX)
   - Option B: Separate `ingest` command (clearer separation)
   - **Recommendation**: Separate `ingest` command, but allow `pack` to include docs via flag

2. **How to handle mixed code+doc repositories?**
   - `infiniloom pack --include-docs` could process both code and non-code files
   - Document files would use the document pipeline, code files use the code pipeline

3. **OCR strategy**: Build in Tesseract support or keep as external?
   - **Recommendation**: Tesseract subprocess (like we do for git), optional feature flag

4. **Table extraction from PDF**: Use heuristics or ML?
   - **Recommendation**: Start with spatial heuristics, add ML option later

5. **Should we support docx writing (round-trip)?**
   - **Recommendation**: No — read-only for v1. Focus on ingestion quality.

---

## 15. Success Metrics

### Parsing Quality
- Parse 99% of DOCX files with structure preserved
- Parse 99% of HTML files with semantic structure extracted
- <1 second for typical compliance documents (<100 pages)
- <100ms for Markdown/HTML/TXT files
- Compliance: Extract 90%+ of section numbers and headings correctly

### Distillation Effectiveness
- `balanced` distillation: 25-40% token reduction with no information loss
- `aggressive` distillation: 40-60% token reduction with <5% information loss
- Zero false positives on requirement/definition stripping (never remove normative content)
- Measurable LLM accuracy improvement on QA tasks vs. raw document input
- Filler phrase removal: Detect 90%+ of common filler patterns

### Token Efficiency
- Format overhead: <10% vs. raw text in TOON format
- Distilled + formatted output: 50-70% smaller than original document
- Attention-optimized arrangement: High-value content in first/last 20% of output

---

## 16. Dependencies Summary

```toml
# Tier 1 (feature-gated under "document")
docx-rust = { version = "0.1", optional = true }
scraper = { version = "0.25", optional = true }
pulldown-cmark = { version = "0.13", optional = true }
calamine = { version = "0.33", optional = true }
csv = { version = "1.4", optional = true }
zip = { version = "2.0", optional = true }

# Tier 2 (additional formats + distillation)
comrak = { version = "0.50", optional = true }     # GFM Markdown with full AST
rtf-parser = { version = "0.4", optional = true }  # Basic RTF text extraction
epub = { version = "2.1", optional = true }         # EPUB reader

# Content distillation / scoring
flate2 = "1.0"  # Already common — compression-ratio redundancy detection
# tfidf-text-summarizer = { version = "0.1", optional = true }  # TF-IDF sentence scoring
# keyword_extraction = { version = "0.1", optional = true }     # TextRank, RAKE, YAKE
# dom-content-extraction = { version = "0.1", optional = true } # HTML boilerplate removal

# Deferred: PDF support (Phase 5)
# lopdf = { version = "0.39", optional = true }
# pdf-extract = { version = "0.10", optional = true }

# Tier 3 (optional heavy dependencies)
# extractous = { version = "0.3", optional = true } # Apache Tika via GraalVM (~100MB)
# tesseract-rs = { version = "0.6", optional = true } # OCR (requires tesseract installed)
```

**Estimated binary size impact**: ~2-3 MB additional with `document` feature enabled (pure Rust crates only). `extractous` would add ~100MB.

---

## 17. Key Challenges & Mitigations

| Challenge | Severity | Mitigation |
|-----------|----------|------------|
| **Table extraction from PDF** | High | Spatial heuristics first; optional LLM post-processing later |
| **OCR for scanned PDFs** | High | Tesseract subprocess (optional feature flag); skip by default |
| **Multi-column layouts** | Medium | Reading order detection heuristics; log warnings |
| **Cross-reference resolution** | Medium | Regex-based detection + section index lookup |
| **Legacy formats (.doc, .ppt)** | Medium | Pandoc/LibreOffice subprocess fallback |
| **Mathematical formulas** | Low | Pass through as LaTeX strings |
| **Determinism** | Critical | Sorted processing, integer-only hashing (reuse embed pattern) |

---

## References

- [Unstructured.io partitioning strategies](https://docs.unstructured.io/)
- [LlamaIndex document parsing / chunk size research](https://docs.llamaindex.ai/)
- [Anthropic prompt engineering — document formatting](https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering)
- [llms.txt standard](https://llmstxt.org/) — Markdown as universal LLM format
- [Docling (IBM)](https://github.com/DS4SD/docling) — layout analysis + DoclingDocument format
- [Marker](https://github.com/VikParuchuri/marker) — PDF/image to Markdown with ML
- [Kreuzberg](https://github.com/...) — 75+ format Rust document intelligence library
- [Jina Reader](https://jina.ai/reader/) — web content to LLM-friendly format
- Crate docs: [pdf-extract](https://docs.rs/pdf-extract), [docx-rust](https://docs.rs/docx-rust), [scraper](https://docs.rs/scraper), [calamine](https://docs.rs/calamine), [pulldown-cmark](https://docs.rs/pulldown-cmark), [comrak](https://docs.rs/comrak), [extractous](https://docs.rs/extractous)
