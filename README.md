# WebReflex (OpenReflex)

> **Deterministic, Instant, and Self-Healing Action Engine for AI Browser Agents**

WebReflex is a high-performance semantic action cache and self-healing execution layer for AI web agents (Playwright, Puppeteer, Browser-use).

---

## 🎯 The Core Problem

1. **AI Browser Agents (LLMs/VLMs)**: Think on every single step. They cost thousands of tokens, take 2–5 seconds per step, and suffer from occasional hallucinations.
2. **Traditional RPA/Playwright Scripts**: Run in 0.01 seconds at 0 cost, but break completely whenever a website modifies a single CSS class or DOM structure.

---

## 💡 The WebReflex Solution

- **Instant Reflex (95% of runs)**: If the action path is cached, execute deterministically via pure Playwright / CDP in <50ms at **$0 token cost**.
- **Self-Healing (5% on UI changes)**: If a selector or step breaks due to UI changes / A/B tests, the LLM intercepts the dirty state, fixes the broken selector chain, and updates the local cache.
- **Local-First & Zero-Leak**: Action graphs are stored in local SQLite as parameterized templates without sensitive user data (PII).
- **Safety Gate**: Strict isolation between `Read-Only` (safe auto-healing) and `Mutating` (human approval gate required for destructive actions).

---

## 📁 Repository Structure (Phase 1)

```
web-reflex/
├── README.md
├── SPEC.md
├── crates/ (or src/)
│   ├── core/         # a11y DOM Sanitizer & Skeleton Hash Engine
│   ├── storage/      # Local SQLite Action Graph Cache
│   ├── engine/       # Deterministic Replay & Hand-off State Machine
│   └── healing/      # LLM Self-Healing & Patch Resolver
└── tests/            # E2E Dynamic UI Mutation Tests
```
