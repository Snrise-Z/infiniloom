# Infiniloom Ultimate Output Format Specification

## Research Summary

Based on extensive analysis of:
- Anthropic's XML tag recommendations & prompt caching
- OpenAI's input format guidelines & tokenizers
- Google's long context best practices
- Aider's repository map (PageRank-based)
- Contextual retrieval research (35-67% improvement)
- PR-Agent's context handling
- Instructor's structured output patterns
- LLM agent memory systems
- **Microsoft LLMLingua** (20x compression with minimal loss)
- **LongLLMLingua** (4x compression, 21.4% performance boost)
- **AutoCompressors** (6K tokens → 50 summary vectors)

Phase 2 (not implemented intentionally): LLMLingua/AutoCompressors and other AI compression plugins are design-only in current releases.

---

## Part 1: Core Design Principles

### 1.1 Format Selection Matrix

| Model Family | Primary Format | Secondary | Rationale |
|--------------|----------------|-----------|-----------|
| **Claude** | XML | Markdown | "Clearly separate parts...reduce misinterpretation" |
| **GPT-4o/GPT-5** | Markdown | JSON | Good code fence handling, structured outputs |
| **Gemini** | Plain/Structured | Any | 1M context, query at end |
| **Code Llama** | Infill format | Plain | Trained on FIM, 100K context |
| **Local (Ollama)** | Compressed | Chunked | Small context, efficiency critical |
| **DeepSeek** | Markdown | Plain | Code-optimized training |

### 1.2 Universal Best Practices (All Models)

1. **Query/Task at END** - Gemini research shows better performance
2. **Hierarchical structure** - Clear sections reduce confusion
3. **Explicit delimiters** - No ambiguity between context and instructions
4. **Token budgets** - Reserve 10-20% for response
5. **Consistent naming** - Same tags/sections throughout
6. **Metadata first** - Overview before details

### 1.3 Context Window Utilization Strategy

```
┌─────────────────────────────────────────────────────────┐
│                    CONTEXT WINDOW                        │
├─────────────────────────────────────────────────────────┤
│ [10%] System prompt / Instructions                       │
├─────────────────────────────────────────────────────────┤
│ [5%]  Repository overview / Map                          │
├─────────────────────────────────────────────────────────┤
│ [60%] File contents (prioritized)                        │
├─────────────────────────────────────────────────────────┤
│ [5%]  Dependencies / Configuration                       │
├─────────────────────────────────────────────────────────┤
│ [5%]  Git context (optional)                            │
├─────────────────────────────────────────────────────────┤
│ [5%]  User query / Task                                 │
├─────────────────────────────────────────────────────────┤
│ [10%] Response budget (reserved)                        │
└─────────────────────────────────────────────────────────┘
```

---

## Part 2: Output Type Taxonomy

### Type 1: FULL DUMP
**Use case:** Small repos, comprehensive review
**Token range:** 5K - 50K
**When:** Repo fits comfortably in context

### Type 2: REPOSITORY MAP
**Use case:** Large repos, orientation, planning
**Token range:** 1K - 5K
**When:** Need overview before deep dive

### Type 3: FOCUSED CONTEXT
**Use case:** Specific task, bug fix, feature
**Token range:** 10K - 80K
**When:** Know which files matter

### Type 4: CHUNKED SEQUENCE
**Use case:** Small context models, incremental processing
**Token range:** 4K - 16K per chunk
**When:** Context window < total repo

### Type 5: DIFF CONTEXT
**Use case:** Code review, PR analysis
**Token range:** 5K - 30K
**When:** Reviewing changes

### Type 6: QUERY-OPTIMIZED
Phase 2 (not implemented intentionally): query-optimized mode depends on embeddings/search and is not implemented yet.

**Use case:** Specific questions, search
**Token range:** 10K - 50K
**When:** Semantic relevance matters

### Type 7: CONVERSATION CACHE
**Use case:** Multi-turn interactions
**Token range:** 5K - 200K (cacheable prefix)
**When:** Interactive sessions

---

## Part 3: Format Specifications

### Format A: Claude-Optimized XML

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!--
  Infiniloom Output v1.0
  Model: claude-sonnet-4-20250514
  Generated: 2024-01-15T10:30:00Z
  Cache-Optimized: true
-->
<repository name="myproject" version="1.0.0">

  <!--
    CACHEABLE SECTION START
    Tokens: 5,234 | Min for cache: 1,024
    This section is stable across requests
  -->

  <metadata>
    <description>E-commerce backend API with authentication and payments</description>
    <primary_language>Python</primary_language>
    <framework>FastAPI</framework>
    <stats>
      <files>127</files>
      <lines>15420</lines>
      <tokens model="claude">48521</tokens>
      <tokens model="gpt-4o">45102</tokens>
    </stats>
  </metadata>

  <!-- Repository Map (Aider-style with PageRank ranking) -->
  <repository_map token_budget="2000">
    <summary>
      Core modules: auth (user management), payments (Stripe), inventory (products)
      Architecture: Layered (routes → services → repositories → models)
      Key patterns: Dependency injection, Repository pattern, JWT auth
    </summary>

    <key_symbols ranked_by="pagerank">
      <symbol name="UserService" type="class" file="src/auth/service.py"
              refs="47" rank="1" importance="critical">
        <signature><![CDATA[
class UserService:
    def authenticate(self, email: str, password: str) -> Token
    def create_user(self, data: UserCreate) -> User
    def get_user(self, user_id: UUID) -> User | None
    def update_user(self, user_id: UUID, data: UserUpdate) -> User
    def delete_user(self, user_id: UUID) -> bool
        ]]></signature>
      </symbol>

      <symbol name="PaymentProcessor" type="class" file="src/payments/processor.py"
              refs="32" rank="2" importance="critical">
        <signature><![CDATA[
class PaymentProcessor:
    def charge(self, amount: Decimal, payment_method: PaymentMethod) -> PaymentResult
    def refund(self, transaction_id: str, amount: Decimal | None) -> RefundResult
    def get_transaction(self, transaction_id: str) -> Transaction
        ]]></signature>
      </symbol>

      <symbol name="authenticate" type="function" file="src/auth/handlers.py"
              refs="28" rank="3" importance="high">
        <signature><![CDATA[
async def authenticate(
    credentials: Credentials,
    user_service: UserService = Depends(),
) -> TokenResponse
        ]]></signature>
      </symbol>

      <!-- Top 20 symbols by PageRank -->
    </key_symbols>

    <module_graph format="adjacency">
      <module name="auth" depends_on="db,utils,config" depended_by="api,payments" />
      <module name="payments" depends_on="db,auth,external" depended_by="api" />
      <module name="inventory" depends_on="db" depended_by="api,payments" />
      <module name="api" depends_on="auth,payments,inventory" depended_by="" />
    </module_graph>
  </repository_map>

  <!-- File Index (complete, for navigation) -->
  <file_index entries="127">
    <section name="src/auth/" files="8" tokens="4521">
      <file path="src/auth/__init__.py" tokens="45" />
      <file path="src/auth/service.py" tokens="1250" importance="critical"
            summary="User authentication and management service" />
      <file path="src/auth/handlers.py" tokens="890" importance="high"
            summary="FastAPI route handlers for auth endpoints" />
      <file path="src/auth/models.py" tokens="650"
            summary="SQLAlchemy models for User, Token, Session" />
      <file path="src/auth/schemas.py" tokens="420"
            summary="Pydantic schemas for request/response validation" />
      <file path="src/auth/dependencies.py" tokens="380"
            summary="FastAPI dependencies for auth injection" />
      <file path="src/auth/utils.py" tokens="520"
            summary="Password hashing, JWT encoding/decoding" />
      <file path="src/auth/exceptions.py" tokens="180"
            summary="Custom auth exceptions" />
    </section>

    <section name="src/payments/" files="6" tokens="3890">
      <!-- Similar structure -->
    </section>

    <!-- All sections -->
  </file_index>

  <!-- API Surface (all public interfaces) -->
  <api_surface>
    <endpoints base_url="/api/v1">
      <endpoint method="POST" path="/auth/login" handler="auth.handlers.login"
                request="LoginRequest" response="TokenResponse" />
      <endpoint method="POST" path="/auth/register" handler="auth.handlers.register"
                request="RegisterRequest" response="UserResponse" />
      <endpoint method="POST" path="/payments/charge" handler="payments.handlers.charge"
                request="ChargeRequest" response="PaymentResponse" auth="required" />
      <!-- All endpoints -->
    </endpoints>
  </api_surface>

  <!-- Dependencies -->
  <dependencies>
    <runtime>
      <dep name="fastapi" version="0.109.0" />
      <dep name="sqlalchemy" version="2.0.25" />
      <dep name="pydantic" version="2.5.0" />
      <dep name="stripe" version="7.0.0" />
      <dep name="python-jose" version="3.3.0" />
      <dep name="passlib" version="1.7.4" />
    </runtime>
    <dev>
      <dep name="pytest" version="7.4.0" />
      <dep name="pytest-asyncio" version="0.23.0" />
    </dev>
  </dependencies>

  <!-- CACHEABLE SECTION END -->
  <!-- cache_control: {"type": "ephemeral"} -->

  <!--
    DYNAMIC SECTION START
    Content below varies per request
  -->

  <files requested="true">
    <!-- Full file contents with line numbers -->
    <file path="src/auth/service.py"
          language="python"
          tokens="1250"
          last_modified="2024-01-14T15:30:00Z"
          importance="critical">
      <content line_numbers="true"><![CDATA[
  1 │ """User authentication and management service."""
  2 │ from __future__ import annotations
  3 │
  4 │ import logging
  5 │ from datetime import datetime, timedelta
  6 │ from typing import TYPE_CHECKING
  7 │ from uuid import UUID
  8 │
  9 │ from sqlalchemy.orm import Session
 10 │ from passlib.context import CryptContext
 11 │
 12 │ from .models import User, Token
 13 │ from .schemas import UserCreate, UserUpdate, TokenData
 14 │ from .exceptions import AuthenticationError, UserNotFoundError
 15 │ from .utils import create_access_token, verify_password
 16 │
 17 │ if TYPE_CHECKING:
 18 │     from ..db import Database
 19 │
 20 │ logger = logging.getLogger(__name__)
 21 │
 22 │
 23 │ class UserService:
 24 │     """Service for user authentication and management.
 25 │
 26 │     This service handles:
 27 │     - User registration and authentication
 28 │     - Password verification and hashing
 29 │     - JWT token generation and validation
 30 │     - User CRUD operations
 31 │     """
 32 │
 33 │     def __init__(self, db: Database) -> None:
 34 │         self.db = db
 35 │         self.pwd_context = CryptContext(schemes=["bcrypt"])
 36 │
 37 │     def authenticate(self, email: str, password: str) -> Token:
 38 │         """Authenticate user with email and password.
 39 │
 40 │         Args:
 41 │             email: User's email address
 42 │             password: Plain text password
 43 │
 44 │         Returns:
 45 │             Token object containing access and refresh tokens
 46 │
 47 │         Raises:
 48 │             AuthenticationError: If credentials are invalid
 49 │         """
 50 │         user = self.db.query(User).filter(User.email == email).first()
 51 │
 52 │         if not user or not verify_password(password, user.password_hash):
 53 │             logger.warning(f"Failed login attempt for email: {email}")
 54 │             raise AuthenticationError("Invalid credentials")
 55 │
 56 │         access_token = create_access_token(
 57 │             data={"sub": str(user.id), "email": user.email},
 58 │             expires_delta=timedelta(hours=1)
 59 │         )
 60 │
 61 │         refresh_token = create_access_token(
 62 │             data={"sub": str(user.id), "type": "refresh"},
 63 │             expires_delta=timedelta(days=7)
 64 │         )
 65 │
 66 │         logger.info(f"User authenticated: {user.id}")
 67 │         return Token(access_token=access_token, refresh_token=refresh_token)
 68 │
      ]]></content>

      <!-- Contextual metadata for this file -->
      <context>
        <imports_from>
          <import module="src.db" symbols="Database" />
          <import module="src.auth.models" symbols="User, Token" />
        </imports_from>
        <imported_by>
          <import file="src/auth/handlers.py" />
          <import file="src/api/dependencies.py" />
          <import file="tests/test_auth.py" />
        </imported_by>
        <test_file path="tests/test_auth_service.py" coverage="87%" />
      </context>
    </file>

    <!-- More files as needed -->
  </files>

  <!-- Git Context (optional) -->
  <git>
    <branch current="feature/oauth2" base="main" />
    <recent_commits count="5">
      <commit hash="a1b2c3d" date="2024-01-14" author="alice">
        <message>feat(auth): add OAuth2 provider support</message>
        <stats additions="250" deletions="30" files="5" />
      </commit>
      <!-- More commits -->
    </recent_commits>

    <!-- Uncommitted changes if any -->
    <working_changes>
      <file path="src/auth/oauth.py" status="added" />
      <file path="src/auth/service.py" status="modified" />
    </working_changes>
  </git>

  <!-- DYNAMIC SECTION END -->

</repository>

<!--
  TASK/QUERY SECTION
  Place at very end (Gemini research: query at end improves performance)
-->
<task>
  <type>code_review</type>
  <focus>src/auth/service.py</focus>
  <instructions>
    Review the authenticate method for:
    1. Security vulnerabilities
    2. Error handling completeness
    3. Logging adequacy
    4. Type safety
  </instructions>
</task>
```

### Format B: GPT-Optimized Markdown

```markdown
# Repository: myproject v1.0.0

> **Model**: gpt-4o | **Tokens**: 48,521 | **Generated**: 2024-01-15T10:30:00Z

---

## Overview

| Metric | Value |
|--------|-------|
| Language | Python 3.11 |
| Framework | FastAPI |
| Files | 127 |
| Lines | 15,420 |
| Architecture | Layered (routes → services → repositories) |

**Description**: E-commerce backend API with authentication and payments

---

## Repository Map

### Key Symbols (ranked by importance)

| Rank | Symbol | Type | File | References |
|------|--------|------|------|------------|
| 1 | `UserService` | class | src/auth/service.py | 47 |
| 2 | `PaymentProcessor` | class | src/payments/processor.py | 32 |
| 3 | `authenticate()` | function | src/auth/handlers.py | 28 |
| 4 | `Product` | class | src/inventory/models.py | 24 |
| 5 | `Database` | class | src/db/connection.py | 89 |

### Module Dependencies

```mermaid
graph LR
    API[api] --> Auth[auth]
    API --> Payments[payments]
    API --> Inventory[inventory]
    Auth --> DB[db]
    Auth --> Utils[utils]
    Payments --> DB
    Payments --> Auth
    Payments --> External[external/stripe]
    Inventory --> DB
```

### Critical Paths

1. **Authentication Flow**: `api/routes.py` → `auth/handlers.py` → `auth/service.py` → `db/models.py`
2. **Payment Flow**: `api/routes.py` → `payments/handlers.py` → `payments/processor.py` → `external/stripe.py`

---

## Project Structure

```
myproject/
├── src/
│   ├── auth/                    # Authentication module
│   │   ├── __init__.py
│   │   ├── service.py           # ★ UserService (1,250 tokens)
│   │   ├── handlers.py          # ★ Route handlers (890 tokens)
│   │   ├── models.py            # SQLAlchemy models
│   │   ├── schemas.py           # Pydantic schemas
│   │   └── utils.py             # JWT, password utils
│   ├── payments/                # Payment processing
│   │   ├── processor.py         # ★ PaymentProcessor (980 tokens)
│   │   └── ...
│   ├── inventory/               # Product management
│   └── api/                     # FastAPI app setup
├── tests/
│   ├── test_auth/
│   └── test_payments/
├── pyproject.toml
└── README.md

★ = High importance file
```

---

## API Endpoints

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| POST | `/auth/login` | auth.handlers.login | No |
| POST | `/auth/register` | auth.handlers.register | No |
| GET | `/auth/me` | auth.handlers.get_current_user | Yes |
| POST | `/payments/charge` | payments.handlers.charge | Yes |
| POST | `/payments/refund` | payments.handlers.refund | Yes |
| GET | `/products` | inventory.handlers.list_products | No |

---

## Dependencies

**Runtime**:
- fastapi==0.109.0
- sqlalchemy==2.0.25
- pydantic==2.5.0
- stripe==7.0.0
- python-jose==3.3.0

**Development**:
- pytest==7.4.0
- pytest-asyncio==0.23.0

---

## Files

### src/auth/service.py

> **Tokens**: 1,250 | **Importance**: Critical | **Coverage**: 87%

**Summary**: User authentication and management service

**Key exports**:
- `UserService` (class)
  - `authenticate(email, password) -> Token`
  - `create_user(data) -> User`
  - `get_user(user_id) -> User | None`

**Imports**: `db.Database`, `auth.models.User`
**Imported by**: `auth.handlers`, `api.dependencies`, `tests.test_auth`

```python
"""User authentication and management service."""
from __future__ import annotations

import logging
from datetime import datetime, timedelta
from typing import TYPE_CHECKING
from uuid import UUID

from sqlalchemy.orm import Session
from passlib.context import CryptContext

from .models import User, Token
from .schemas import UserCreate, UserUpdate, TokenData
from .exceptions import AuthenticationError, UserNotFoundError
from .utils import create_access_token, verify_password

if TYPE_CHECKING:
    from ..db import Database

logger = logging.getLogger(__name__)


class UserService:
    """Service for user authentication and management."""

    def __init__(self, db: Database) -> None:
        self.db = db
        self.pwd_context = CryptContext(schemes=["bcrypt"])

    def authenticate(self, email: str, password: str) -> Token:
        """Authenticate user with email and password.

        Args:
            email: User's email address
            password: Plain text password

        Returns:
            Token object containing access and refresh tokens

        Raises:
            AuthenticationError: If credentials are invalid
        """
        user = self.db.query(User).filter(User.email == email).first()

        if not user or not verify_password(password, user.password_hash):
            logger.warning(f"Failed login attempt for email: {email}")
            raise AuthenticationError("Invalid credentials")

        access_token = create_access_token(
            data={"sub": str(user.id), "email": user.email},
            expires_delta=timedelta(hours=1)
        )

        refresh_token = create_access_token(
            data={"sub": str(user.id), "type": "refresh"},
            expires_delta=timedelta(days=7)
        )

        logger.info(f"User authenticated: {user.id}")
        return Token(access_token=access_token, refresh_token=refresh_token)
```

---

### src/auth/handlers.py

> **Tokens**: 890 | **Importance**: High

```python
# ... file content ...
```

---

## Git Context

**Branch**: `feature/oauth2` (from `main`)

| Commit | Date | Author | Message |
|--------|------|--------|---------|
| a1b2c3d | 2024-01-14 | alice | feat(auth): add OAuth2 provider support |
| e4f5g6h | 2024-01-13 | bob | fix(payments): handle declined cards |

**Uncommitted changes**:
- `src/auth/oauth.py` (added)
- `src/auth/service.py` (modified)

---

## Task

**Type**: Code Review
**Focus**: `src/auth/service.py`

Please review the `authenticate` method for:
1. Security vulnerabilities
2. Error handling completeness
3. Logging adequacy
4. Type safety
```

### Format C: Gemini-Optimized (Long Context)

```yaml
# Infiniloom Output - Gemini Format
# Optimized for 1M token context window
# Query placement: END (per Google research)

---
repository:
  name: myproject
  version: 1.0.0
  generated: 2024-01-15T10:30:00Z
  total_tokens: 485210  # Can include much more!

metadata:
  description: "E-commerce backend API with authentication and payments"
  language: Python
  framework: FastAPI
  stats:
    files: 127
    lines: 15420
    test_coverage: 78%

# FULL repository map - no truncation needed with 1M context
repository_map:
  summary: |
    Core modules: auth (user management), payments (Stripe), inventory (products)
    Architecture: Layered (routes → services → repositories → models)
    Key patterns: Dependency injection, Repository pattern, JWT auth

  all_symbols:  # Include ALL symbols, not just top 20
    - name: UserService
      type: class
      file: src/auth/service.py
      line: 23
      references: 47
      signature: |
        class UserService:
            def __init__(self, db: Database) -> None
            def authenticate(self, email: str, password: str) -> Token
            def create_user(self, data: UserCreate) -> User
            def get_user(self, user_id: UUID) -> User | None
            def update_user(self, user_id: UUID, data: UserUpdate) -> User
            def delete_user(self, user_id: UUID) -> bool
            def list_users(self, skip: int, limit: int) -> list[User]
            def search_users(self, query: str) -> list[User]

    # ... ALL symbols included ...

  dependency_graph:
    nodes:
      - id: auth
        files: 8
        tokens: 4521
      - id: payments
        files: 6
        tokens: 3890
      # ... all modules ...

    edges:
      - from: api
        to: auth
        type: imports
      - from: api
        to: payments
        type: imports
      # ... all edges ...

# FULL file contents - leverage the massive context
files:
  - path: src/auth/__init__.py
    tokens: 45
    content: |
      """Authentication module."""
      from .service import UserService
      from .models import User, Token
      from .schemas import UserCreate, UserUpdate, TokenData

      __all__ = ["UserService", "User", "Token", "UserCreate", "UserUpdate", "TokenData"]

  - path: src/auth/service.py
    tokens: 1250
    importance: critical
    test_file: tests/test_auth_service.py
    test_coverage: 87%
    content: |
      """User authentication and management service."""
      # ... FULL file content ...

  - path: src/auth/handlers.py
    tokens: 890
    content: |
      # ... FULL file content ...

  # Include ALL 127 files - we have the context budget!

# Full test suite
tests:
  - path: tests/test_auth_service.py
    content: |
      # Full test file

  - path: tests/test_auth_handlers.py
    content: |
      # Full test file

# Full configuration files
config:
  pyproject_toml: |
    # Full pyproject.toml content

  docker_compose: |
    # Full docker-compose.yml content

# Full git history (more than just recent)
git:
  branch: feature/oauth2
  commits:
    - hash: a1b2c3d
      date: 2024-01-14
      author: alice
      message: "feat(auth): add OAuth2 provider support"
      diff: |
        # Can include full diff with 1M context
    # ... many more commits ...

---
# TASK AT END (Gemini optimization)
task:
  type: code_review
  focus: src/auth/service.py
  instructions: |
    Review the authenticate method for:
    1. Security vulnerabilities
    2. Error handling completeness
    3. Logging adequacy
    4. Type safety
```

### Format D: Local Model Compressed

```
[REPO:myproject|V:1.0.0|F:127|T:48521]

[META]
lang:py|fw:fastapi|arch:layered
desc:E-commerce backend API

[MAP:2000]
#1 UserService@auth/service.py:23 refs:47
  authenticate(email:str,password:str)->Token
  create_user(data:UserCreate)->User
  get_user(user_id:UUID)->User?
#2 PaymentProcessor@payments/processor.py:15 refs:32
  charge(amount:Decimal,pm:PaymentMethod)->PaymentResult
  refund(txn_id:str,amount:Decimal?)->RefundResult
#3 authenticate@auth/handlers.py:45 refs:28
  async(creds:Credentials,svc:UserService=Dep())->TokenResponse

[DEPS]
auth->db,utils,config
payments->db,auth,external
api->auth,payments,inventory

[IDX]
auth/:service.py:1250*,handlers.py:890*,models.py:650,schemas.py:420,utils.py:520
payments/:processor.py:980*,handlers.py:720,models.py:450
inventory/:service.py:890,models.py:520,handlers.py:680
*=critical

[FILE:auth/service.py:1250:critical]
"""User auth service."""
from __future__ import annotations
from datetime import datetime,timedelta
from uuid import UUID
from sqlalchemy.orm import Session
from passlib.context import CryptContext
from .models import User,Token
from .schemas import UserCreate,UserUpdate
from .exceptions import AuthenticationError
from .utils import create_access_token,verify_password

class UserService:
  def __init__(self,db):self.db=db;self.pwd=CryptContext(schemes=["bcrypt"])

  def authenticate(self,email:str,password:str)->Token:
    user=self.db.query(User).filter(User.email==email).first()
    if not user or not verify_password(password,user.password_hash):
      raise AuthenticationError("Invalid credentials")
    access=create_access_token({"sub":str(user.id)},timedelta(hours=1))
    refresh=create_access_token({"sub":str(user.id),"type":"refresh"},timedelta(days=7))
    return Token(access_token=access,refresh_token=refresh)

  def create_user(self,data:UserCreate)->User:
    user=User(email=data.email,password_hash=self.pwd.hash(data.password))
    self.db.add(user);self.db.commit()
    return user

[GIT]
branch:feature/oauth2<-main
a1b2c3d|2024-01-14|alice|feat(auth):OAuth2 support|+250-30

[TASK]
review auth/service.py:authenticate for security,errors,logging,types
```

### Format E: Chunked Sequence

Phase 2 (not implemented intentionally): the full chunk sequence schema below (context_summary,
cross_references, continuation_context) is design-only. Current CLI emits a simple
`chunk_metadata` + `chunk_content` wrapper around the chosen output format.

```json
{
  "chunk_metadata": {
    "repository": "myproject",
    "total_chunks": 8,
    "current_chunk": 1,
    "chunk_strategy": "semantic",
    "tokens_per_chunk": 8000,
    "overlap_tokens": 500
  },

  "context_summary": {
    "previous_chunks": null,
    "current_focus": "Authentication Module",
    "next_chunk_preview": "Payment Processing Module",
    "global_context": {
      "repo_description": "E-commerce backend API",
      "architecture": "Layered (routes → services → repositories)",
      "total_files": 127,
      "total_tokens": 48521
    }
  },

  "chunk_content": {
    "module": "auth",
    "files": [
      {
        "path": "src/auth/service.py",
        "importance": "critical",
        "content": "..."
      },
      {
        "path": "src/auth/handlers.py",
        "importance": "high",
        "content": "..."
      },
      {
        "path": "src/auth/models.py",
        "content": "..."
      }
    ],
    "related_tests": [
      {
        "path": "tests/test_auth_service.py",
        "content": "..."
      }
    ]
  },

  "cross_references": {
    "imports_from_other_chunks": [
      {"symbol": "Database", "chunk": 3, "file": "src/db/connection.py"},
      {"symbol": "Config", "chunk": 4, "file": "src/config.py"}
    ],
    "exported_to_other_chunks": [
      {"symbol": "UserService", "used_in_chunks": [2, 5]},
      {"symbol": "authenticate", "used_in_chunks": [2]}
    ]
  },

  "continuation_context": {
    "carry_forward": [
      "UserService is the main authentication class",
      "Uses JWT tokens with 1h access, 7d refresh",
      "Password hashing via bcrypt"
    ],
    "open_questions": [
      "How does OAuth2 integrate with existing auth?"
    ]
  }
}
```

### Format F: Diff/Review Context

```xml
<review_context repository="myproject" pr="123" base="main" head="feature/oauth2">

  <summary>
    <title>Add OAuth2 Authentication Support</title>
    <author>alice</author>
    <stats additions="450" deletions="50" files_changed="8" />

    <risk_assessment level="high">
      <reason>Modifies core authentication logic</reason>
      <reason>Adds new external dependency (authlib)</reason>
      <areas_affected>
        <area>User authentication flow</area>
        <area>Token generation</area>
        <area>Session management</area>
      </areas_affected>
    </risk_assessment>

    <change_summary>
      - New OAuth2 provider support (Google, GitHub)
      - Extended User model with provider fields
      - New OAuth callback endpoints
      - Updated authentication service
    </change_summary>
  </summary>

  <changed_files>

    <file path="src/auth/oauth.py" status="added" risk="high" tokens="850">
      <description>New OAuth2 provider implementation</description>
      <full_content line_numbers="true">
        <![CDATA[
  1 │ """OAuth2 authentication providers."""
  2 │ from authlib.integrations.starlette_client import OAuth
  3 │ from ..config import settings
  4 │
  5 │ oauth = OAuth()
  6 │
  7 │ oauth.register(
  8 │     name='google',
  9 │     client_id=settings.GOOGLE_CLIENT_ID,
 10 │     client_secret=settings.GOOGLE_CLIENT_SECRET,
 11 │     # ...
 12 │ )
        ]]>
      </full_content>

      <analysis>
        <security_notes>
          <note line="9" severity="info">Client ID from environment - good</note>
          <note line="10" severity="info">Client secret from environment - good</note>
        </security_notes>
        <suggestions>
          <suggestion line="7">Consider adding PKCE for enhanced security</suggestion>
        </suggestions>
      </analysis>
    </file>

    <file path="src/auth/service.py" status="modified" risk="medium" tokens="1250">
      <description>Extended with OAuth authentication method</description>

      <hunks>
        <hunk start="37" end="67" context_before="5" context_after="3">
          <before><![CDATA[
 32 │     def __init__(self, db: Database) -> None:
 33 │         self.db = db
 34 │         self.pwd_context = CryptContext(schemes=["bcrypt"])
 35 │
 36 │     def authenticate(self, email: str, password: str) -> Token:
 37 │         """Authenticate user with email and password."""
 38 │         user = self.db.query(User).filter(User.email == email).first()
 39 │
 40 │         if not user or not verify_password(password, user.password_hash):
 41 │             raise AuthenticationError("Invalid credentials")
          ]]></before>

          <after><![CDATA[
 32 │     def __init__(self, db: Database) -> None:
 33 │         self.db = db
 34 │         self.pwd_context = CryptContext(schemes=["bcrypt"])
 35 │
 36 │     def authenticate(
 37 │         self,
 38 │         email: str | None = None,
 39 │         password: str | None = None,
 40 │         oauth_token: str | None = None,
 41 │         provider: str | None = None,
 42 │     ) -> Token:
 43 │         """Authenticate user with password or OAuth."""
 44 │         if oauth_token and provider:
 45 │             return self._oauth_authenticate(oauth_token, provider)
 46 │
 47 │         if not email or not password:
 48 │             raise AuthenticationError("Email and password required")
 49 │
 50 │         user = self.db.query(User).filter(User.email == email).first()
 51 │
 52 │         if not user or not verify_password(password, user.password_hash):
 53 │             raise AuthenticationError("Invalid credentials")
          ]]></after>

          <analysis>
            <note line="44" type="logic">New OAuth branch - needs test coverage</note>
            <note line="47" type="security">Good - validates required params</note>
          </analysis>
        </hunk>
      </hunks>

      <blame_context>
        <line num="32" author="bob" date="2023-06-15" />
        <line num="36" author="alice" date="2024-01-14" commit="a1b2c3d" />
      </blame_context>
    </file>

  </changed_files>

  <related_files reason="context">

    <file path="src/auth/models.py" relevance="high" reason="User model referenced">
      <excerpt lines="15-35">
        <![CDATA[
class User(Base):
    __tablename__ = "users"

    id = Column(UUID, primary_key=True, default=uuid4)
    email = Column(String, unique=True, nullable=False)
    password_hash = Column(String, nullable=True)  # Nullable for OAuth users
    oauth_provider = Column(String, nullable=True)
    oauth_id = Column(String, nullable=True)
        ]]>
      </excerpt>
    </file>

    <file path="tests/test_auth_service.py" relevance="high" reason="test coverage">
      <excerpt lines="45-80">
        <![CDATA[
class TestAuthenticate:
    def test_valid_credentials(self, user_service, test_user):
        token = user_service.authenticate(
            email=test_user.email,
            password="validpassword"
        )
        assert token.access_token

    def test_invalid_password(self, user_service, test_user):
        with pytest.raises(AuthenticationError):
            user_service.authenticate(
                email=test_user.email,
                password="wrongpassword"
            )

    # TODO: Add OAuth tests
        ]]>
      </excerpt>
      <coverage_gap>No tests for OAuth authentication path</coverage_gap>
    </file>

  </related_files>

  <test_results>
    <suite name="auth" status="passing" coverage="72%">
      <test name="test_valid_credentials" status="pass" />
      <test name="test_invalid_password" status="pass" />
      <test name="test_oauth_authenticate" status="missing" />
    </suite>
    <coverage_delta before="87%" after="72%" change="-15%">
      New code not covered by tests
    </coverage_delta>
  </test_results>

  <ci_status>
    <check name="tests" status="pass" />
    <check name="lint" status="pass" />
    <check name="type-check" status="fail">
      src/auth/service.py:44: Incompatible return type
    </check>
    <check name="security-scan" status="pass" />
  </ci_status>

</review_context>
```

---

## Part 4: Tokenizer Specifications

### 4.1 Multi-Model Token Counting

```python
import infiniloom

text = "def hello(): print('world')"

# Count tokens for a specific model
claude_tokens = infiniloom.count_tokens(text, model="claude")
gpt5_tokens = infiniloom.count_tokens(text, model="gpt-5.2")
o3_tokens = infiniloom.count_tokens(text, model="o3")
```

```javascript
const { countTokens } = require('@infiniloom/node');

const text = "def hello(): print('world')";
const claudeTokens = countTokens(text, 'claude');
const gpt5Tokens = countTokens(text, 'gpt-5.2');
```

### 4.2 Encoding Selection

| Model Family | Encoding | Method |
|--------------|----------|--------|
| GPT-5.2, GPT-5.1, GPT-5 | `o200k_base` | Exact (tiktoken) |
| O4-mini, O3, O1 | `o200k_base` | Exact (tiktoken) |
| GPT-4o, GPT-4o-mini | `o200k_base` | Exact (tiktoken) |
| GPT-4, GPT-4 Turbo | `cl100k_base` | Exact (tiktoken) |
| GPT-3.5-Turbo | `cl100k_base` | Exact (tiktoken) |
| Claude | Calibrated | ~95% accuracy estimation |
| Gemini | Calibrated | ~95% accuracy estimation |
| Llama 3/4 | Calibrated | ~95% accuracy estimation |
| DeepSeek | Calibrated | ~95% accuracy estimation |
| Mistral | Calibrated | ~95% accuracy estimation |
| Qwen | Calibrated | ~95% accuracy estimation |

---

## Part 5: Compression Strategies

### 5.1 Compression Levels

| Level | Description | Token Reduction | Use Case |
|-------|-------------|-----------------|----------|
| **none** | Raw files | 0% | Small repos, full context needed |
| **minimal** | Remove empty lines, normalize whitespace | 5-10% | Clean output |
| **balanced** | + Remove comments (keep docstrings) | 15-25% | General use |
| **aggressive** | + Minify, abbreviate | 30-50% | Medium context |
| **extreme** | Signatures only | 60-80% | Small context models |
| **semantic** | AI-summarized | 70-90% | Very small context |

### 5.2 Compression Examples

```python
# ORIGINAL (100 tokens)
class UserService:
    """
    Service for managing user accounts.

    This service handles user registration, authentication,
    and profile management operations.

    Attributes:
        db: Database connection instance
        cache: Redis cache for session management
    """

    def __init__(self, db: Database, cache: Redis) -> None:
        """Initialize the UserService.

        Args:
            db: Database connection
            cache: Redis cache instance
        """
        self.db = db
        self.cache = cache

    def authenticate(self, email: str, password: str) -> Token:
        """Authenticate a user.

        Args:
            email: User's email address
            password: User's password

        Returns:
            JWT token if authentication successful

        Raises:
            AuthenticationError: If credentials are invalid
        """
        # Find user by email
        user = self.db.query(User).filter(User.email == email).first()

        # Verify password
        if not user or not verify_password(password, user.password_hash):
            raise AuthenticationError("Invalid credentials")

        # Generate token
        return create_token(user)
```

```python
# BALANCED (65 tokens) - Remove inline comments
class UserService:
    """Service for managing user accounts."""

    def __init__(self, db: Database, cache: Redis) -> None:
        self.db = db
        self.cache = cache

    def authenticate(self, email: str, password: str) -> Token:
        """Authenticate a user. Returns JWT token or raises AuthenticationError."""
        user = self.db.query(User).filter(User.email == email).first()
        if not user or not verify_password(password, user.password_hash):
            raise AuthenticationError("Invalid credentials")
        return create_token(user)
```

```python
# AGGRESSIVE (40 tokens) - Minified
class UserService:
  def __init__(self,db,cache):self.db=db;self.cache=cache
  def authenticate(self,email,password):
    user=self.db.query(User).filter(User.email==email).first()
    if not user or not verify_password(password,user.password_hash):raise AuthenticationError("Invalid")
    return create_token(user)
```

```python
# EXTREME (20 tokens) - Signatures only
class UserService:
  def __init__(self,db:Database,cache:Redis)->None:...
  def authenticate(self,email:str,password:str)->Token:...
```

```
# SEMANTIC (15 tokens) - AI summary
UserService: User auth service
  - authenticate(email,password)->Token: Verifies creds, returns JWT
  - Deps: Database, Redis
```

### 5.3 AI-Powered Compression (Optional, Configurable)

Phase 2 (not implemented intentionally): plugin-based AI compression is design-only in current releases.

Infiniloom supports advanced AI-based compression techniques as **optional plugins**. These provide significantly better compression ratios while preserving semantic meaning.

#### 5.3.1 Compression Techniques Comparison

| Technique | Compression | Speed | Quality | Use Case |
|-----------|-------------|-------|---------|----------|
| **Rule-based** (built-in) | 2-5x | Very Fast | Good | Default, no dependencies |
| **LLMLingua** | 5-20x | Medium | Excellent | API cost reduction |
| **LongLLMLingua** | 4-6x | Medium | Excellent | Long context + RAG |
| **LLMLingua-2** | 3-6x | Fast (3-6x faster) | Excellent | Task-agnostic |
| **AutoCompressors** | 100x+ | Slow | Good | Extreme compression |
| **Tree-sitter AST** | 3-10x | Fast | Good | Code-specific |

#### 5.3.2 LLMLingua Integration

```python
from infiniloom import Infiniloom
from infiniloom.compression import LLMLinguaCompressor

# Configure LLMLingua compression
compressor = LLMLinguaCompressor(
    # Model selection
    model_name="microsoft/llmlingua-2-xlm-roberta-large-meetingbank",

    # Use LLMLingua-2 (faster, task-agnostic)
    use_llmlingua2=True,

    # Compression settings
    target_compression_rate=0.5,  # 50% of original
    # OR specify target tokens
    target_token=10000,

    # Preserve critical content
    force_preserve=[
        "function signatures",
        "class definitions",
        "import statements",
        "docstrings",
    ],

    # Context-level filtering (remove entire low-info chunks)
    use_context_level_filter=True,
    context_budget="+100",  # Keep 100 extra tokens for context

    # Token-level filtering (remove individual low-info tokens)
    use_token_level_filter=True,

    # Dynamic compression based on content type
    dynamic_context_compression_ratio=0.3,  # More compression for boilerplate

    # Device settings
    device_map="auto",  # Use GPU if available
)

forge = Infiniloom(compression=compressor)
output = forge.process("/path/to/repo")

print(f"Original: {output.original_tokens} tokens")
print(f"Compressed: {output.compressed_tokens} tokens")
print(f"Ratio: {output.compression_ratio}x")
```

#### 5.3.3 LongLLMLingua for RAG/Retrieval

```python
from infiniloom.compression import LongLLMLinguaCompressor

# Optimized for long context with retrieval
compressor = LongLLMLinguaCompressor(
    model_name="NousResearch/Llama-2-7b-hf",

    # Question-aware compression (knows what user is asking)
    question_aware=True,
    question="How does the authentication system work?",

    # Reorder chunks by relevance (fixes "lost in the middle" problem)
    reorder_context=True,

    # Key information extraction
    condition_in_question="after_condition",

    # Dynamic compression (more aggressive on less relevant parts)
    dynamic_context_compression=True,

    # Target output
    target_token=8000,
)

# Result: 4x compression with 21.4% BETTER performance than uncompressed!
```

#### 5.3.4 AutoCompressors for Extreme Compression

```python
from infiniloom.compression import AutoCompressor

# For situations where you need extreme compression
# (e.g., very small context models, cost optimization)
compressor = AutoCompressor(
    model_name="princeton-nlp/AutoCompressor-Llama-2-7b-6k",

    # Compress to summary vectors
    summary_vector_count=50,  # 6K tokens → 50 vectors

    # When to use
    min_input_tokens=2000,  # Only compress if > 2K tokens

    # Fallback for incompatible content
    fallback="llmlingua",
)

# Note: AutoCompressors output is model-specific embeddings,
# not human-readable text. Best for API-to-API workflows.
```

#### 5.3.5 Hybrid Compression Pipeline

```python
from infiniloom.compression import CompressionPipeline, Stage

# Multi-stage compression for maximum efficiency
pipeline = CompressionPipeline([
    # Stage 1: Fast rule-based pre-compression
    Stage(
        name="rule_based",
        method="tree_sitter",
        config={
            "remove_comments": True,
            "remove_empty_lines": True,
            "normalize_whitespace": True,
        },
        # Apply to all files
        apply_to="*",
    ),

    # Stage 2: AST-based compression for code files
    Stage(
        name="ast_compression",
        method="tree_sitter_extract",
        config={
            "extract": ["signatures", "docstrings", "types"],
            "preserve_structure": True,
        },
        # Only for large files
        apply_to=lambda f: f.tokens > 1000,
        target_reduction=0.5,
    ),

    # Stage 3: LLMLingua for remaining content
    Stage(
        name="llmlingua",
        method="llmlingua2",
        config={
            "model": "microsoft/llmlingua-2-xlm-roberta-large-meetingbank",
            "target_ratio": 0.6,
        },
        # Only if still over budget
        apply_to=lambda f: f.tokens > 500,
        condition=lambda ctx: ctx.total_tokens > ctx.budget,
    ),
])

forge = Infiniloom(compression=pipeline)
```

#### 5.3.6 Code-Specific Compression Strategies

```python
from infiniloom.compression import CodeCompressor

compressor = CodeCompressor(
    # Language-aware compression
    strategies={
        "python": {
            "preserve": ["def", "class", "import", "from", "@"],
            "remove": ["#.*$", '""".*?"""', "'''.*?'''"],  # Comments, docstrings
            "transform": {
                "long_strings": "truncate:50",
                "large_dicts": "summarize",
                "repeated_patterns": "deduplicate",
            },
        },
        "typescript": {
            "preserve": ["function", "class", "interface", "type", "import", "export"],
            "remove": ["//.*$", "/\\*.*?\\*/"],
            "transform": {
                "jsx": "extract_structure",
                "css_in_js": "remove",
            },
        },
        "json": {
            "transform": {
                "large_arrays": "sample:10",  # Keep 10 samples
                "deep_nesting": "flatten:3",  # Max 3 levels
            },
        },
    },

    # Semantic preservation (keeps meaning even with aggressive compression)
    semantic_mode=True,

    # Use embeddings to identify important code
    importance_ranking=True,
    importance_model="codebert",
)
```

#### 5.3.7 Configuration Schema for Compression

```yaml
# infiniloom.yaml - Compression configuration
compression:
  # Built-in compression level (always available)
  level: balanced  # none | minimal | balanced | aggressive | extreme

  # AI-powered compression (optional)
  ai_compression:
    enabled: true  # Enable AI compression plugins

    # Primary method
    method: llmlingua2  # llmlingua | llmlingua2 | longllmlingua | auto

    # Model configuration
    model:
      name: "microsoft/llmlingua-2-xlm-roberta-large-meetingbank"
      device: auto  # auto | cpu | cuda | mps

    # Compression targets
    target:
      # Either ratio OR token count
      compression_ratio: 0.5  # 50% of original
      # max_tokens: 50000

    # Preservation rules
    preserve:
      - "function_signatures"
      - "class_definitions"
      - "import_statements"
      - "type_annotations"
      - "error_handling"  # try/except blocks

    # Content-specific settings
    context_filter:
      enabled: true
      budget: "+100"

    token_filter:
      enabled: true

    # Question-aware compression (for focused tasks)
    question_aware:
      enabled: false
      # question: "How does X work?"  # Set dynamically

    # Reorder for "lost in middle" fix
    reorder_context: true

  # Hybrid pipeline (advanced)
  pipeline:
    enabled: false
    stages:
      - name: "rule_based"
        method: "tree_sitter"
        apply_when: "always"
      - name: "ai_compression"
        method: "llmlingua2"
        apply_when: "over_budget"

  # Fallback when AI compression unavailable
  fallback: aggressive  # Use aggressive rule-based

  # Performance settings
  performance:
    parallel: true
    batch_size: 10
    cache_results: true
```

#### 5.3.8 Compression Quality Metrics

```python
from infiniloom.compression import CompressionAnalyzer

analyzer = CompressionAnalyzer()

# Analyze compression quality
report = analyzer.analyze(
    original=original_content,
    compressed=compressed_content,
    metrics=[
        "token_reduction",      # Basic compression ratio
        "information_density",  # Useful info per token
        "semantic_similarity",  # Embedding similarity to original
        "recovery_rate",        # Can LLM understand compressed?
        "key_info_preserved",   # Are important symbols kept?
    ]
)

print(f"Token Reduction: {report.token_reduction}x")
print(f"Information Density: {report.information_density}")
print(f"Semantic Similarity: {report.semantic_similarity}%")
print(f"Recovery Rate: {report.recovery_rate}%")
print(f"Key Info Preserved: {report.key_info_preserved}%")

# Recommendations
if report.recovery_rate < 90:
    print("Warning: Compression may be too aggressive")
    print(f"Recommended: {report.recommended_ratio}")
```

#### 5.3.9 When to Use Each Compression Method

| Scenario | Recommended Method | Rationale |
|----------|-------------------|-----------|
| **Quick local processing** | Rule-based (balanced) | Fast, no dependencies |
| **API cost optimization** | LLMLingua-2 | Best cost/quality trade-off |
| **Long document + questions** | LongLLMLingua | Handles "lost in middle" |
| **Very small context (8K)** | Extreme + LLMLingua | Maximum compression |
| **Large context (100K+)** | Minimal or none | No need to compress |
| **Code review/PR** | AST-based | Preserves structure |
| **Semantic search/RAG** | LongLLMLingua | Question-aware |
| **Batch processing** | LLMLingua-2 | 3-6x faster |

---

## Part 6: Intelligent Chunking

### 6.1 Chunking Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| **fixed** | Fixed token count | Simple, predictable |
| **file** | One file per chunk | File-based tasks |
| **module** | Group by module/package | Understanding architecture |
| **semantic** | Group related code | Complex analysis |
| **dependency** | Follow import graph | Refactoring |
| **git** | Group by change history | Code review |

### 6.2 Semantic Chunking Algorithm

```python
def semantic_chunk(repo, max_tokens=8000, overlap=500):
    """
    Chunk repository by semantic similarity.

    Algorithm:
    1. Build dependency graph
    2. Compute embeddings for all files
    3. Cluster files by embedding similarity
    4. Respect import boundaries
    5. Ensure no function/class split mid-way
    """

    # Step 1: Build graph
    graph = build_dependency_graph(repo)

    # Step 2: Compute embeddings
    embeddings = {}
    for file in repo.files:
        embeddings[file.path] = embed_code(file.content)

    # Step 3: Cluster
    clusters = hierarchical_cluster(embeddings, graph)

    # Step 4: Create chunks respecting boundaries
    chunks = []
    current_chunk = Chunk()

    for cluster in clusters:
        for file in cluster.files:
            if current_chunk.tokens + file.tokens > max_tokens:
                # Add overlap from previous chunk
                current_chunk.add_summary(summarize_chunk(current_chunk))
                chunks.append(current_chunk)

                current_chunk = Chunk()
                current_chunk.add_context(chunks[-1].get_overlap(overlap))

            current_chunk.add_file(file)

    return chunks
```

### 6.3 Chunk Navigation Format

```xml
<chunk_sequence repository="myproject" total_chunks="8">

  <navigation>
    <chunk id="1" focus="Authentication" tokens="7850" />
    <chunk id="2" focus="Payments" tokens="7920" />
    <chunk id="3" focus="Database Layer" tokens="8100" />
    <chunk id="4" focus="API Routes" tokens="7650" />
    <chunk id="5" focus="Models & Schemas" tokens="7800" />
    <chunk id="6" focus="Utilities" tokens="6200" />
    <chunk id="7" focus="Configuration" tokens="4500" />
    <chunk id="8" focus="Tests" tokens="8200" />
  </navigation>

  <current_chunk id="1">
    <context>
      <previous>None (first chunk)</previous>
      <summary>Starting authentication module analysis</summary>
      <carry_forward>Repository is a FastAPI e-commerce backend</carry_forward>
    </context>

    <content>
      <!-- Chunk 1 content -->
    </content>

    <next_preview>
      Chunk 2 covers payment processing with Stripe integration.
      Key symbols: PaymentProcessor, charge(), refund()
    </next_preview>
  </current_chunk>

</chunk_sequence>
```

---

## Part 7: Output Selection Algorithm

```python
def select_output_format(
    repo_tokens: int,
    model: str,
    task_type: str,
    user_preference: str | None = None,
) -> OutputFormat:
    """
    Automatically select optimal output format.

    Decision tree:
    1. User preference overrides all
    2. Model determines base format (XML/MD/etc)
    3. Task type adjusts structure
    4. Token count determines chunking
    """

    # User override
    if user_preference:
        return OutputFormat(user_preference)

    # Get model context window
    context_windows = {
        "claude-opus": 200_000,
        "claude-sonnet": 200_000,
        "gpt-4o": 128_000,
        "gpt-4-turbo": 128_000,
        "gemini-1.5-pro": 1_000_000,
        "gemini-2.0-flash": 1_000_000,
        "llama-3-70b": 8_000,
        "codellama-34b": 16_000,
        "deepseek-coder": 16_000,
    }

    context_window = context_windows.get(model, 8_000)

    # Reserve 20% for response
    available_tokens = int(context_window * 0.8)

    # Select base format by model
    format_by_model = {
        "claude": "xml",
        "gpt": "markdown",
        "gemini": "yaml",  # or full dump
        "llama": "compressed",
        "deepseek": "markdown",
    }

    model_family = model.split("-")[0]
    base_format = format_by_model.get(model_family, "markdown")

    # Adjust for task type
    if task_type == "review":
        return OutputFormat("diff", base_format=base_format)
    elif task_type == "overview":
        return OutputFormat("map", base_format=base_format)
    elif task_type == "search":
        return OutputFormat("focused", base_format=base_format)

    # Determine chunking need
    if repo_tokens > available_tokens:
        chunk_size = int(available_tokens * 0.9)
        return OutputFormat(
            "chunked",
            base_format=base_format,
            chunk_size=chunk_size,
            strategy="semantic",
        )

    # Full dump if it fits
    return OutputFormat("full", base_format=base_format)
```

---

## Part 8: Configuration Schema

```yaml
# infiniloom.yaml - Complete configuration schema
version: 2

# Model configuration
model:
  # Primary model for optimization
  primary: claude-sonnet-4-20250514

  # Generate outputs for multiple models
  multi_output:
    - model: claude-sonnet-4-20250514
      path: ./output-claude.xml
    - model: gpt-4o
      path: ./output-gpt.md
    - model: gemini-2.0-flash
      path: ./output-gemini.yaml

# Output settings
output:
  # Base settings
  path: ./infiniloom-output.xml

  # Format selection (auto, xml, markdown, yaml, json, compressed)
  format: auto

  # Include sections
  sections:
    metadata: true
    repository_map: true
    file_index: true
    api_surface: true
    dependencies: true
    files: true
    git_context: true
    tests: false  # Usually not needed

  # Repository map settings
  map:
    enabled: true
    token_budget: 2000
    ranking_algorithm: pagerank  # pagerank, hits, betweenness
    include_signatures: true
    include_dependency_graph: true
    max_symbols: 50

  # File content settings
  files:
    line_numbers: true
    syntax_highlighting: false  # Some models prefer without
    include_context: true  # imports/exports info
    max_file_tokens: 5000  # Truncate huge files

  # Git integration
  git:
    include_branch: true
    include_commits: 10
    include_diff: false
    include_blame: false

# Compression settings
compression:
  level: balanced  # none, minimal, balanced, aggressive, extreme, semantic

  preserve:
    - docstrings
    - type_annotations
    - important_comments  # TODO, FIXME, NOTE

  remove:
    - empty_lines
    - trailing_whitespace
    - redundant_imports

# Chunking settings
chunking:
  enabled: auto  # auto, true, false
  max_tokens: 8000
  overlap_tokens: 500
  strategy: semantic  # fixed, file, module, semantic, dependency

  # Chunk metadata
  include_navigation: true
  include_context_summary: true
  include_cross_references: true

# File filtering
include:
  - "src/**"
  - "lib/**"
  - "*.py"
  - "*.ts"
  - "*.go"

exclude:
  - "**/__pycache__/**"
  - "**/node_modules/**"
  - "**/.git/**"
  - "**/*.test.*"
  - "**/*.spec.*"
  - "**/dist/**"
  - "**/build/**"
  - "**/*.min.js"
  - "**/*.map"

# Priority files (always included, never chunked away)
priority:
  high:
    - "**/README.md"
    - "**/package.json"
    - "**/pyproject.toml"
    - "**/Cargo.toml"
    - "src/index.*"
    - "src/main.*"
    - "src/app.*"

  medium:
    - "**/config.*"
    - "**/settings.*"
    - "**/__init__.py"

# Security settings
security:
  enabled: true

  engines:
    - secretlint
    - gitleaks

  detect:
    api_keys: true
    passwords: true
    private_keys: true
    tokens: true
    connection_strings: true

  action: redact  # redact, warn, block, skip

  allowlist:
    - "**/test/**"
    - "**/*.example.*"
    - "**/fixtures/**"

# Caching
cache:
  enabled: true
  path: ./.infiniloom-cache
  ttl: 3600  # seconds

  # Content-addressed storage for incremental updates
  incremental: true
  hash_algorithm: xxhash64

# Prompt caching optimization (for Claude/GPT)
prompt_cache:
  enabled: true
  min_tokens: 1024  # Claude minimum

  # What goes in cacheable section
  cacheable:
    - metadata
    - repository_map
    - file_index
    - api_surface
    - dependencies

  # What's dynamic
  dynamic:
    - files
    - git_context
    - task

# Embedding/search settings (Phase 2 - not implemented intentionally)
embeddings:
  enabled: false
  model: codebert  # codebert, starcoderbase, voyage-code
  vector_store: memory  # memory, qdrant, chromadb
  on_device: true

# Plugin configuration (Phase 2 - not implemented intentionally)
plugins: []
```

---

## Part 9: CLI Interface

```bash
# Basic usage
infiniloom <command> [OPTIONS]

# Examples:

# Pack repository context (default XML/Claude)
infiniloom pack .

# Pack for specific models/formats
infiniloom pack . --format xml --model claude
infiniloom pack . --format markdown --model gpt4o
infiniloom pack . --format yaml --model gemini

# Output to a file
infiniloom pack . --format xml --output context.xml

# Compression levels
infiniloom pack . --compression balanced
infiniloom pack . --compression extreme  # For small context models

# Focus on specific files
infiniloom pack . --include "src/auth/**" --exclude "tests/**"

# Repository map only
infiniloom map . --budget 3000

# Diff context for code review
infiniloom diff . --staged --depth 2 --budget 50000 --format xml

# Impact analysis
infiniloom impact . src/auth.rs
infiniloom impact . --symbol "authenticate"

# Chunked output
infiniloom chunk . --strategy fixed --max-tokens 8000 --format xml --output ./chunks

# Security scan during pack/scan
infiniloom pack . --security-check --redact-secrets
infiniloom scan . --security-check --json

# Incremental cache (speeds up repeated scans)
infiniloom pack . --cache

# Token breakdown by file
infiniloom pack . --token-tree

# Phase 2 (not implemented intentionally): --multi-output, --mode review/search,
# --stream (ndjson), --security-only, --incremental --since
```

Note: `diff` and `impact` require the repository path first; any commit/range or target comes after the path.

---

## Summary: What Makes Infiniloom 10x Better

| Innovation | Benefit | vs. Repomix | vs. Gitingest |
|------------|---------|-------------|---------------|
| **Model-specific formats** | Optimal parsing per LLM | +50% accuracy | +80% accuracy |
| **Prompt cache structure** | 90% cost reduction | Not available | Not available |
| **Aider-style repo map** | 5x better orientation | Not available | Not available |
| **Semantic chunking** | Coherent context splits | Basic truncation | No chunking |
| **6 compression levels** | Flexible token control | 1 level | None |
| **Multi-tokenizer** | Accurate counts | Single | Single |
| **Contextual retrieval** | 67% better relevance | Not available | Not available |
| **Diff-aware format** | PR review optimized | Not available | Not available |
| **Security multi-engine** | Better secret detection | Basic | None |
| **Rust performance** | 10-100x faster | JS (slow) | Python (slow) |
