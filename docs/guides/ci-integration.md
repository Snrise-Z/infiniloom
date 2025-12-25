# CI/CD Integration

Use Infiniloom in your CI/CD pipelines for automated context generation and security scanning.

## GitHub Actions

### Basic Setup

```yaml
name: Generate LLM Context

on:
  push:
    branches: [main]
  pull_request:

jobs:
  context:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Infiniloom
        run: npm install -g infiniloom

      - name: Generate context
        run: infiniloom pack . --format xml --output context.xml

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: repo-context
          path: context.xml
```

### Security Scanning

```yaml
name: Security Check

on:
  push:
    branches: [main]
  pull_request:

jobs:
  secrets-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Infiniloom
        run: npm install -g infiniloom

      - name: Scan for secrets
        run: infiniloom pack . --security-check --fail-on-secrets
```

### PR Context Generation

Generate context specifically for changed files in PRs:

```yaml
name: PR Context

on:
  pull_request:

jobs:
  diff-context:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full history for diff

      - name: Install Infiniloom
        run: npm install -g infiniloom

      - name: Build index
        run: infiniloom index .

      - name: Generate diff context
        run: |
          infiniloom diff . origin/${{ github.base_ref }}..HEAD \
            --include-diff \
            --format markdown \
            --output pr-context.md

      - name: Upload context
        uses: actions/upload-artifact@v4
        with:
          name: pr-context
          path: pr-context.md
```

### Matrix Build for Multiple Formats

```yaml
name: Multi-Format Context

on:
  release:
    types: [published]

jobs:
  generate:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        format: [xml, markdown, json, yaml]
    steps:
      - uses: actions/checkout@v4

      - name: Install Infiniloom
        run: npm install -g infiniloom

      - name: Generate ${{ matrix.format }} context
        run: |
          infiniloom pack . \
            --format ${{ matrix.format }} \
            --output context.${{ matrix.format }}

      - name: Upload
        uses: actions/upload-artifact@v4
        with:
          name: context-${{ matrix.format }}
          path: context.${{ matrix.format }}
```

## GitLab CI

### Basic Pipeline

```yaml
# .gitlab-ci.yml
stages:
  - analyze

generate-context:
  stage: analyze
  image: node:20
  script:
    - npm install -g infiniloom
    - infiniloom pack . --format xml --output context.xml
  artifacts:
    paths:
      - context.xml
    expire_in: 1 week

security-scan:
  stage: analyze
  image: node:20
  script:
    - npm install -g infiniloom
    - infiniloom pack . --security-check --fail-on-secrets
  allow_failure: false
```

### Merge Request Context

```yaml
mr-context:
  stage: analyze
  image: node:20
  only:
    - merge_requests
  script:
    - npm install -g infiniloom
    - infiniloom index .
    - infiniloom diff . origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD --include-diff --output mr-context.md
  artifacts:
    paths:
      - mr-context.md
```

## CircleCI

```yaml
# .circleci/config.yml
version: 2.1

jobs:
  generate-context:
    docker:
      - image: cimg/node:20.0
    steps:
      - checkout
      - run:
          name: Install Infiniloom
          command: npm install -g infiniloom
      - run:
          name: Generate context
          command: infiniloom pack . --format xml --output context.xml
      - store_artifacts:
          path: context.xml

  security-check:
    docker:
      - image: cimg/node:20.0
    steps:
      - checkout
      - run:
          name: Install Infiniloom
          command: npm install -g infiniloom
      - run:
          name: Scan for secrets
          command: infiniloom pack . --security-check --fail-on-secrets

workflows:
  main:
    jobs:
      - security-check
      - generate-context:
          requires:
            - security-check
```

## Jenkins

### Jenkinsfile

```groovy
pipeline {
    agent any

    stages {
        stage('Install') {
            steps {
                sh 'npm install -g infiniloom'
            }
        }

        stage('Security Scan') {
            steps {
                sh 'infiniloom pack . --security-check --fail-on-secrets'
            }
        }

        stage('Generate Context') {
            steps {
                sh 'infiniloom pack . --format xml --output context.xml'
                archiveArtifacts artifacts: 'context.xml'
            }
        }
    }
}
```

## Pre-commit Hooks

### Using pre-commit framework

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: infiniloom-secrets
        name: Scan for secrets
        entry: infiniloom pack . --security-check --fail-on-secrets
        language: system
        pass_filenames: false
        always_run: true
```

### Manual Git Hook

```bash
# .git/hooks/pre-commit
#!/bin/bash

echo "Scanning for secrets..."
if ! infiniloom pack . --security-check --fail-on-secrets 2>/dev/null; then
    echo "ERROR: Secrets detected. Please remove them before committing."
    exit 1
fi
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

## Configuration for CI

Create `.infiniloom.yaml` optimized for CI:

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000

scan:
  exclude:
    - "node_modules/*"
    - "vendor/*"
    - ".git/*"
    - "dist/*"
    - "build/*"
    - "target/*"
    - "*.lock"
    - "*.min.js"

security:
  scan_secrets: true
  fail_on_secrets: true  # Fail CI if secrets found
  redact_secrets: true
  allowlist:
    - "EXAMPLE"
    - "placeholder"
    - "localhost"
```

## Environment Variables in CI

Set defaults via environment:

```yaml
# GitHub Actions
env:
  INFINILOOM_OUTPUT__FORMAT: xml
  INFINILOOM_OUTPUT__MODEL: claude
  INFINILOOM_SECURITY__SCAN_SECRETS: true
  INFINILOOM_SECURITY__FAIL_ON_SECRETS: true
```

## Caching

Speed up CI runs by caching the symbol index:

### GitHub Actions

```yaml
- name: Cache Infiniloom index
  uses: actions/cache@v4
  with:
    path: .infiniloom/
    key: infiniloom-index-${{ hashFiles('**/*.rs', '**/*.ts', '**/*.py') }}
    restore-keys: |
      infiniloom-index-
```

### GitLab CI

```yaml
generate-context:
  cache:
    key: infiniloom-index
    paths:
      - .infiniloom/
```

## Scheduled Context Updates

Generate fresh context on a schedule:

```yaml
# GitHub Actions
name: Daily Context

on:
  schedule:
    - cron: '0 6 * * *'  # 6 AM daily

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm install -g infiniloom
      - run: infiniloom pack . --format xml --output context.xml
      - uses: actions/upload-artifact@v4
        with:
          name: daily-context
          path: context.xml
          retention-days: 7
```

## Troubleshooting CI

### "Command not found"

Ensure npm bin is in PATH:
```yaml
- run: export PATH="$(npm bin -g):$PATH"
```

### "Permission denied"

Use `npx` instead of global install:
```yaml
- run: npx infiniloom pack . --format xml
```

### "Timeout"

For large repos, increase timeout and use filtering:
```yaml
- run: |
    infiniloom pack . \
      --format xml \
      --skip-symbols \
      --exclude "tests/*" \
      --max-tokens 50000
  timeout-minutes: 10
```

### "Out of memory"

Use a larger runner or reduce scope:
```yaml
jobs:
  generate:
    runs-on: ubuntu-latest-16-cores  # Larger runner
```

Or:
```yaml
- run: |
    infiniloom pack . \
      --top-files 100 \
      --compression aggressive
```
