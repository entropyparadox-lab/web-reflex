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
  Page Skeleton Matched ──> Deterministic Playwright Replay (<45ms, $0 Token Cost)

[Phase C: UI Mutation / A/B Test (When Broken)]
  Step Fails ──> LLM Self-Heals only the broken selector ──> Cache updated to v+1 (<45ms next time)
```

| Approach | Latency | Token Cost | Fragility on UI Changes |
| :--- | :--- | :--- | :--- |
| **Traditional RPA / Hardcoded Scripts** | ~50ms | $0 | 💥 **Breaks on 1px CSS change** |
| **VLM / Raw LLM Agents** | 2,000–5,000ms | $$$ (thousands of tokens/step) | 🐢 **Extremely slow & costly** |
| **🚀 WebReflex (Cache + Self-Healing)** | **~40ms** | **$0** (on warm cached paths) | 🛡️ **Auto-heals via LLM when broken** |

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
|  2. Query Local SQLite Action Cache (WAL Mode)              |
|                                                             |
|   [HIT: Warm Path (40ms)]                     [MISS / FAIL] |
|        |                                           |        |
|        v                                           v        |
|  +---------------------+                 +----------------+ |
|  | Fast-Path Replay    |                 | LLM Self-Heal  | |
|  | ($0 Token Cost)     |                 | (Hand-off ctx) | |
|  +---------------------+                 +----------------+ |
|        |                                           |        |
|        |                                           v        |
|        |                                 +----------------+ |
|        |                                 | Auto-Patch DB  | |
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

### 1. Build and Run the Local Daemon (Rust)

```bash
# Clone repository
git clone https://github.com/entropyparadox-lab/web-reflex.git
cd web-reflex

# Build release binary
cargo build --release

# Start local HTTP/REST daemon with SQLite WAL mode
./target/release/web-reflex serve --port 9199 --db reflex.db
```

### 2. Python SDK & Playwright Integration

```python
from playwright.sync_api import sync_playwright
from web_reflex import ReflexSession, create_llm_healer

# Automated LLM repair helper (supports OpenAI / Anthropic)
llm_healer = create_llm_healer(model="gpt-4o-mini", provider="openai")

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page()
    page.goto("https://example-shop.com/checkout")

    session = ReflexSession(
        page,
        endpoint="http://127.0.0.1:9199",
        llm_healer=llm_healer, # Triggered ONLY if a selector breaks
    )

    # Fast-Path Execution (Runs in ~40ms on warm cached pages)
    result = session.execute(value_slots={"COUPON": "SAVE50"})
    print(f"Status: {result['status']}, Elapsed: {result.get('elapsed_ms')}ms")

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
});

const result = await session.execute({
  valueSlots: { COUPON: "SAVE50" },
});
console.log(`Status: ${result.status}, Elapsed: ${result.elapsedMs}ms`);
```

### 4. Git-Backed Team Recipe Sync

Export and version-control your team's action graphs directly in Git:

```bash
# Export SQLite cache to pretty JSON files for Git commits
./target/release/web-reflex export --out ./recipes

# Import updated recipes into local SQLite on other team machines
./target/release/web-reflex import --input ./recipes
```

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
# 1. Run all Rust core/storage/engine tests
cargo test --workspace

# 2. Run Python Playwright E2E & Browser Recorder tests
pytest -v tests/

# 3. Run TypeScript SDK tests
cd typescript && node --test test/sdk.test.mjs
```

### Verified Benchmark Output

```text
[Run 1] Fast-Path Hit:        40.06ms   (0 LLM tokens, 100% deterministic)
[Run 2] Self-Healed to v2:   1,058.41ms (UI Mutation caught -> Healed via LLM hook -> DB v2)
[Run 3] Direct v2 Hit:        40.80ms   (Direct fast-path replay on new layout)
```

---

## 🗺️ Roadmap & Current Status

- [x] **v0.1.0 Core**: Rust DOM Sanitizer, Skeleton Hasher (SHA-256), SQLite Action Cache (WAL).
- [x] **v0.1.0 Daemon & SDK**: Axum REST server, Python Playwright `ReflexSession`, Self-Healing Hand-off.
- [x] **v0.2.0 Interactive Recorder**: `BrowserRecorder` CLI for 1-click recipe capture.
- [x] **v0.3.0 TypeScript SDK**: `@entropyparadox/web-reflex` native npm package.
- [x] **v0.3.5 Git Sync**: `web-reflex export/import` for Git-versioned action repositories.
- [x] **v0.4.0 Chrome Extension**: MV3 SidePanel visual inspector & replay trigger.

---

## 📄 License

Licensed under the [Apache License, Version 2.0](LICENSE).
