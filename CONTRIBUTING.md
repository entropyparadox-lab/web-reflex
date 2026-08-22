# Contributing to WebReflex

Thank you for your interest in contributing to WebReflex! 🎉

WebReflex is an open-source, deterministic, self-healing action cache and execution engine for AI browser agents, maintained by [EntropyParadox Lab](https://github.com/entropyparadox-lab). We welcome contributions across the core Rust engine, Python/TS SDKs, browser extensions, and documentation.

---

## 🛠️ Development Setup

### Prerequisites

1. **Rust**: 1.80+ (stable toolchain)
2. **Python**: 3.10+ (with `uv` or `pip`)
3. **Playwright**: Installed for live browser integration testing

### 1. Clone and Build Core Engine

```bash
git clone https://github.com/entropyparadox-lab/web-reflex.git
cd web-reflex

# Check and build Rust workspace
cargo check --workspace
cargo test --workspace
```

### 2. Setup Python SDK Environment

We recommend using `uv` or `venv`:

```bash
# Create virtual environment and install dependencies
uv venv .venv
source .venv/bin/activate
uv pip install -e "python[dev]"
playwright install chromium
```

---

## 📁 Repository Organization

```
web-reflex/
├── crates/
│   ├── core/         # DOM Sanitizer, Skeleton Hasher (SHA-256), Models
│   ├── storage/      # SQLite Action Graph Storage (WAL mode, rusqlite)
│   ├── engine/       # Replay Engine, Safety Gate, Self-Healing Manager
│   └── cli/          # web-reflex CLI & Local Axum HTTP Server
├── python/           # Python SDK (web_reflex.ReflexSession, client, healers)
├── tests/            # Live Playwright browser E2E test suite
├── SPEC.md           # Architecture specifications & schema
├── README.md         # Project documentation
├── CONTRIBUTING.md   # Contribution guide
└── LICENSE           # Apache-2.0
```

---

## 🧪 Running Tests & Quality Checks

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

## 📐 Architecture Principles & Guidelines

When writing code for WebReflex, please adhere to these core tenets:

1. **Zero-PII Architecture**: Never store sensitive values, input text, tokens, or emails into the Action Graph database. Always parameterize variable inputs into `$SLOT` placeholders.
2. **Deterministic Fast-Path**: The core replay path must execute in under 50ms without invoking network LLM calls on warm cache hits.
3. **Fail-Closed Safety Gate**: Any action that modifies data (`mutating_write`, checkout, deletion, transfers) must default to requiring explicit confirmation.
4. **Clean Hand-off on Failure**: When a selector fails, do not repeat completed steps. Provide a minimal bounded context snippet for the LLM to heal only the broken node.

---

## 🚀 Submitting a Pull Request

1. **Fork the repository** and create a feature branch:
   ```bash
   git checkout -b feat/your-feature-name
   ```
2. **Commit your changes** with descriptive commit messages following Conventional Commits (e.g. `feat:`, `fix:`, `docs:`, `perf:`).
3. **Push to your fork** and open a Pull Request against `main`.
4. Include a clear summary of what changes were made and how they were tested.

---

## 🔒 Reporting Security Issues

If you discover a security vulnerability or potential PII leak vector, please do **not** open a public GitHub issue. Instead, report it privately to the maintainers at `security@entropyparadox.com` or via GitHub Private Vulnerability Reporting.
