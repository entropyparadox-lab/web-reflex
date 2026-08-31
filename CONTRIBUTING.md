# Contributing to WebReflex ⚡

Thank you for your interest in contributing to WebReflex! 🎉

WebReflex is an open-source, deterministic, self-healing action cache and execution engine for AI browser agents, maintained by [EntropyParadox Lab](https://github.com/entropyparadox-lab). We welcome contributions across the core Rust engine, Python/TS SDKs, browser extensions, and documentation.

---

## 1. Branch Strategy & PR Workflow

We follow **GitHub Flow** with a protected `main` branch:

* **`main` (Protected)**: Production release branch. Direct push to `main` is prohibited; changes land only via reviewed Pull Requests.
* **`feat/<name>` / `fix/<name>`**: Feature and bugfix branches (e.g. `feat/dom-sanitizer`, `fix/slot-inference`).
* **`docs/<name>` / `perf/<name>`**: Documentation updates and performance optimizations.

---

## 2. Fast Local Git Hooks

Install the local pre-commit and pre-push validation hooks:
```bash
./scripts/setup-hooks.sh
```

---

## 3. Development Setup & Testing

### Prerequisites
1. **Rust**: 1.80+ (stable toolchain)
2. **Python**: 3.10+ (with `uv` or `pip`)
3. **Playwright**: Installed for live browser integration testing

### Quality & Verification Commands
Before submitting a Pull Request, ensure all tests and linter checks pass cleanly:

```bash
# 1. Rust Format & Clippy
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# 2. Rust Unit & Integration Tests
cargo test --workspace

# 3. Python Live Playwright E2E Tests
source .venv/bin/activate
pytest -v tests/test_live_playwright.py
```

---

## 4. Architecture Principles & Guidelines

When writing code for WebReflex, please adhere to these core tenets:

1. **Zero-PII Architecture**: Never store sensitive values, input text, tokens, or emails into the Action Graph database. Always parameterize variable inputs into `$SLOT` placeholders.
2. **Deterministic Fast-Path**: The core replay path must execute in under 50ms without invoking network LLM calls on warm cache hits.
3. **Fail-Closed Safety Gate**: Any action that modifies data (`mutating_write`, checkout, deletion, transfers) must default to requiring explicit confirmation.
4. **Clean Hand-off on Failure**: When a selector fails, do not repeat completed steps. Provide a minimal bounded context snippet for the LLM to heal only the broken node.

---

## 5. Release & SemVer Policy

* **Semantic Versioning (SemVer 2.0.0)**:
  * `PATCH`: Bug fixes, DOM healing edge cases, documentation.
  * `MINOR`: New SDK bindings, engine capabilities, backwards-compatible additions.
  * `MAJOR`: Breaking schema or Action Graph format changes.
* **Tag Immutability**:
  * Never delete or rewrite a published tag (`vX.Y.Z`).

---

## 6. Commit Message Format

We strictly enforce **Conventional Commits**:
```
<type>(<scope>): <subject>

Examples:
  feat(engine): add subtree skeleton hashing
  fix(storage): optimize WAL journal mode checkpoint
  docs: add Python SDK quickstart recipe
```
