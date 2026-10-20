# WebReflex (OpenReflex)

<div align="center">

**Deterministic, Instant, and Self-Healing Action Cache for AI Web Agents**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org)
[![npm](https://img.shields.io/badge/npm-%40entropyparadox%2Fweb--reflex-red.svg)](https://www.npmjs.com)
[![Playwright](https://img.shields.io/badge/playwright-supported-green.svg)](https://playwright.dev)

*Don't think every step. Just reflex.*

</div>

---

## ⚡ Why WebReflex?

AI web agents (like Browser-use, Stagehand, MultiOn) are powerful, but evaluating a full LLM/VLM step on every single navigation makes them **slow (2–5s per click)**, **prohibitively expensive (thousands of tokens)**, and **prone to hallucinations**.

Traditional RPA and Playwright scripts run in **~40ms at $0 cost**, but **break completely whenever a CSS class or button ID changes**.

**WebReflex bridges this gap with a 2-stage execution lifecycle:**

```
[Phase A: Cold Start (1st Run)]
  Agent explores via LLM OR Developer uses `BrowserRecorder` ──> Recipe saved ($SLOTS)

[Phase B: Warm Replay (Next 1,000+ Runs)]
  Page Skeleton Matched ──> Deterministic Playwright Replay (<40ms, $0 Token Cost)

[Phase C: UI Mutation / A/B Test (When Broken)]
  Step Fails ──> LLM Self-Heals only the broken selector ──> Append-only DB v+1 (<40ms next time)
```

| Approach | Latency | Token Cost | Fragility on UI Changes | Multi-Tenancy & Safety |
| :--- | :--- | :--- | :--- | :--- |
| **Traditional RPA / Hardcoded Scripts** | ~50ms | $0 | 💥 **Breaks on 1px CSS change** | ❌ Manual maintenance |
| **VLM / Raw LLM Agents** | 2,000–5,000ms | $$$ (thousands of tokens/step) | 🐢 **Extremely slow & costly** | ⚠️ High hallucination risk |
| **🚀 WebReflex (Cache + Self-Healing)** | **~30–40ms** | **$0** (on warm cached paths) | 🛡️ **Auto-heals via LLM when broken** | ✅ **Physical DB isolation & Auth Gate** |

---

## 🏗️ Architecture

```
+-------------------------------------------------------------+
|                        AI Web Agent                         |
|             (e.g., Goal: "Search for RTX 5090 and order")   |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                      WebReflex Engine                       |
|                                                             |
|  1. Sanitize DOM & Compute Skeleton Hash (SHA-256)          |
|  2. Query Tenant SQLite Action Cache (WAL Mode)             |
|                                                             |
|   [HIT: Warm Path (30-40ms)]                  [MISS / FAIL] |
|        |                                           |        |
|        v                                           v        |
|  +---------------------+                 +----------------+ |
|  | Fast-Path Replay    |                 | LLM Self-Heal  | |
|  | ($0 Token Cost)     |                 | (Ollama/OpenAI)| |
|  +---------------------+                 +----------------+ |
|        |                                           |        |
|        |                                           v        |
|        |                                 +----------------+ |
|        |                                 | Append History | |
|        |                                 | (Version += 1) | |
|        |                                 +----------------+ |
+--------+-------------------------------------------+--------+
         |                                           |
         +--------------------+----------------------+
                              |
                              v
+-------------------------------------------------------------+
|                 Playwright / Target Website                 |
+-------------------------------------------------------------+
```

---

## 🚀 Quick Start

### 1. Build and Run the Local or Multi-Tenant Daemon (Rust)

```bash
# Clone repository
git clone https://github.com/entropyparadox-lab/web-reflex.git
cd web-reflex

# Build release binary
cargo build --release

# Mode A: 1-Click Single-Tenant Local Daemon (Default)
./target/release/web-reflex serve --port 9199 --db reflex.db

# Mode B: Production Multi-Tenant Server with Physical SQLite Isolation & API Key Auth
./target/release/web-reflex serve --port 9199 --db-dir ./tenants --api-key your-secret-token
```

### 2. Python SDK & Playwright Integration

```python
from playwright.sync_api import sync_playwright
from web_reflex import ReflexSession, create_llm_healer

# Automated LLM repair helper (supports Ollama / local vLLM / OpenAI with PII redaction)
llm_healer = create_llm_healer(
    provider="ollama", # or "openai", "local"
    model="llama3.2",
    redact_pii=True,   # Masks emails, phone numbers, and card numbers before healing
)

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page()
    page.goto("https://example-shop.com/checkout")

    session = ReflexSession(
        page,
        endpoint="http://127.0.0.1:9199",
        api_key="your-secret-token", # Optional: for authenticated daemon
        tenant_id="customer_acme",   # Optional: routes to ./tenants/customer_acme.db
        llm_healer=llm_healer,       # Triggered ONLY if a selector breaks
    )

    # Fast-Path Execution (Runs in ~30-40ms on warm cached pages)
    result = session.execute(value_slots={"COUPON": "SAVE50"})
    print(f"Status: {result['status']}, Version: {result['version']}, Elapsed: {result.get('elapsed_ms')}ms")

    browser.close()
```

### 3. TypeScript / Node.js SDK

```typescript
import { chromium } from "playwright";
import { ReflexSession } from "@entropyparadox/web-reflex";

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto("https://example-shop.com/checkout");

const session = new ReflexSession(page, {
  endpoint: "http://127.0.0.1:9199",
  apiKey: "your-secret-token",
  tenantId: "customer_acme",
});

const result = await session.execute({
  valueSlots: { COUPON: "SAVE50" },
});
console.log(`Status: ${result.status}, Version: ${result.version}, Elapsed: ${result.elapsedMs}ms`);
```

---

## 🏢 Enterprise & Production Features

### 1. Database-per-Tenant Physical Isolation
* Instead of mixing customer data in a single table with risky `WHERE tenant_id = ...` clauses, WebReflex creates **separate physical SQLite files** (`./tenants/{tenant_id}.db`).
* **Zero data leakage**: Connection routing occurs at the filesystem level.
* **Concurrency isolation**: SQLite WAL write-locks during self-healing do not block queries from other tenants.
* **Zero-residual offboarding**: Offboard a customer cleanly with `rm ./tenants/{tenant_id}.db`.

### 2. Bearer & API Key Authentication Gate
* Set `--api-key <KEY>` (or `REFLEX_API_KEY` environment variable).
* Mutation and query endpoints (`/inspect`, `/record`, `/heal`, `/rollback`) strictly validate `Authorization: Bearer <KEY>` or `X-API-Key: <KEY>`, returning `401 Unauthorized` on missing or invalid tokens.
* Public health probes (`/api/v1/health`) remain accessible for container orchestration liveness checks.

### 3. Append-Only Version History & Instant Rollback
* Every mutation or self-healing patch preserves the prior version in the `action_graph_history` table.
* **Query History**: `GET /api/v1/graphs/{graph_id}/history`
* **Instant Rollback**: `POST /api/v1/graphs/{graph_id}/rollback` with `{"version": 1}` restores a verified stable recipe without downtime.

### 4. Zero-Downtime Schema Migrations
* Powered by SQLite's native `PRAGMA user_version`.
* **Lazy JIT Migration**: Tenant databases are automatically checked and migrated within <1ms upon their first connection.
* **Batch CLI Migration**:
  ```bash
  web-reflex migrate --db-dir ./tenants
  ```

### 5. Local LLM (Ollama) & Zero-PII Sanitization
* Run self-healing completely on-premise without sending HTML to external third parties:
  ```python
  healer = create_llm_healer(provider="ollama", model="qwen2.5-coder", redact_pii=True)
  ```
* `redact_sensitive_html` automatically masks credit card numbers, emails, phone numbers, and password input fields.

### 6. Real-time Telemetry, Heal Snapshots & Stats Dashboard
* **Zero-I/O Tracing**: Daemon outputs structured one-line telemetry to stdout for instant `journalctl -u web-reflex.service -f` observability:
  ```text
  [INFO] tenant=default domain=shop.com action=INSPECT status=HIT graph=checkout v=1 elapsed=31.20ms
  [WARN] tenant=acme domain=shop.com action=HEAL status=HEALED graph=checkout v=1->2 step=pay_btn new_sel="#pay_v2" elapsed=842.10ms
  ```
* **Database-Native Heal Snapshots**:
  * Instead of polluting the filesystem with unversioned JSON files, self-healing events are captured in the tenant's `heal_snapshots` SQLite ring-buffer (latest 500 per graph) with PII-redacted snippets.
* **Persistent Performance Dashboard**:
  ```bash
  web-reflex stats --db-dir ./tenants
  ```
  Reports cumulative cache hits, total token savings, and recent self-heal events per tenant without losing counts across daemon restarts.

---

## 🛡️ Safety Gate: Read vs Write Isolation

WebReflex enforces strict safety levels on every action node:

* `read_only` / `idempotent`: Data extraction, search inputs, navigating tabs. Auto-executed without interruption.
* `mutating_write`: Checkout submission, deletion, payment, bank transfer.
  * WebReflex triggers `approval_callback(node)` before clicking.
  * If no approval is given, execution is safely blocked with `ReflexSafetyError`.

---

## 🧪 Testing & Benchmark Results

```bash
# 1. Run all Rust core/storage/engine/cli tests
cargo test --workspace

# 2. Run Python Playwright E2E & Enterprise integration tests
pytest -v tests/

# 3. Run TypeScript SDK tests
cd typescript && npm test
```

### Verified Live Benchmark Output

```text
[Run 1] Cold Session Fast-Path: 167.76ms (Browser initialization + cache hit)
[Run 2] Self-Healed to v2:    1,023.25ms (UI Mutation caught -> Selector healed -> DB v2)
[Run 3] Direct v2 Fast-Path:     32.85ms (Direct fast-path replay, 0 LLM tokens)
```

---

## 🗺️ Roadmap & Current Status

- [x] **v0.1.0 Core**: Rust DOM Sanitizer, Skeleton Hasher (SHA-256), SQLite Action Cache (WAL).
- [x] **v0.1.0 Daemon & SDK**: Axum REST server, Python Playwright `ReflexSession`, Self-Healing Hand-off.
- [x] **v0.2.0 Interactive Recorder**: `BrowserRecorder` CLI for 1-click recipe capture.
- [x] **v0.3.0 TypeScript SDK**: `@entropyparadox/web-reflex` native npm package.
- [x] **v0.3.5 Git Sync**: `web-reflex export/import` for Git-versioned action repositories.
- [x] **v0.4.0 Chrome Extension**: MV3 SidePanel visual inspector & replay trigger.
- [x] **v0.5.0 Enterprise & Security**:
  - [x] Multi-tenant physical SQLite file isolation (`--db-dir ./tenants`, `X-Tenant-ID`).
  - [x] Bearer & API Key authentication gate (`--api-key`).
  - [x] Append-only version history table & rollback API (`/history`, `/rollback`).
  - [x] Native `PRAGMA user_version` automated migrations (`web-reflex migrate`).
  - [x] Pluggable Local LLM (Ollama/vLLM) & Zero-PII sanitization engine.

---

## 📄 License

Licensed under the [Apache License, Version 2.0](LICENSE).
