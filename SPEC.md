# WebReflex Technical Specification (v0.1)

## 1. Core Architecture

WebReflex operates as a drop-in execution middleware between the AI Agent and the Browser Automation Driver (Playwright / CDP).

```
+-------------------------------------------------------------+
|                        AI Web Agent                         |
|             (Goal: "Search for RTX 4090 and add to cart")   |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                      WebReflex Engine                       |
|                                                             |
|  1. Extract DOM / a11y Skeleton Hash                        |
|  2. Lookup Local Action Graph (SQLite)                      |
|                                                             |
|   [HIT: 95%]                                  [MISS / FAIL] |
|        |                                           |        |
|        v                                           v        |
|  +---------------------+                 +----------------+ |
|  | Fast-Path Replay    |                 | LLM Auto-Heal  | |
|  | (Deterministic 0ms) |                 | (State Hand-off| |
|  +---------------------+                 +----------------+ |
|        |                                           |        |
|        |                                           v        |
|        |                                 +----------------+ |
|        |                                 | Self-Patch DB  | |
|        |                                 +----------------+ |
+--------+-------------------------------------------+--------+
         |                                           |
         +--------------------+----------------------+
                              |
                              v
+-------------------------------------------------------------+
|                   Browser / Target Website                  |
+-------------------------------------------------------------+
```

---

## 2. Data Schema: Action Graph & State Node

### 2.1 Skeleton Hash Generation
1. Sanitize HTML DOM / a11y tree:
   - Strip dynamic CSS classes (e.g. `css-1a2b3c`, Tailwind runtime hashes).
   - Strip text node values, input values, and user identifiers.
   - Retain semantic tags (`button`, `input`, `form`, `a`, `select`), ARIA roles, and structural hierarchy.
2. Generate SHA-256 hash representing the **Page Skeleton State**.

### 2.2 Action Node Schema (JSON)

```json
{
  "graph_id": "shopping_cart_flow_v1",
  "domain_pattern": "example-shop.com/checkout/*",
  "skeleton_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "version": 1,
  "nodes": [
    {
      "step_id": "step_1_input_coupon",
      "action_type": "type",
      "safety_level": "read_or_idempotent",
      "target": {
        "primary_selector": "input#coupon_code",
        "fallback_selectors": [
          "input[name='discountCode']",
          "aria/쿠폰 코드 입력",
          "xpath=//form//input[@type='text']"
        ]
      },
      "value_slot": "$COUPON_CODE",
      "pre_condition": {
        "visible_selector": "input#coupon_code"
      },
      "post_condition": {
        "wait_for_selector": "button#apply_coupon"
      }
    },
    {
      "step_id": "step_2_submit_order",
      "action_type": "click",
      "safety_level": "mutating_write",
      "requires_approval": true,
      "target": {
        "primary_selector": "button.submit-order-btn",
        "fallback_selectors": [
          "button[type='submit']",
          "aria/결제하기"
        ]
      }
    }
  ]
}
```

---

## 3. Self-Healing & Transaction Hand-off Protocol

1. **State Assertion Check**: Before executing Step $N$, verify `pre_condition`.
2. **Partial Failure Interception**: If Step $N$ fails:
   - Record exact checkpoint: Steps $1 \dots N-1$ were completed.
   - Capture current DOM diff and screenshot.
   - Format hand-off context for LLM:
     ```
     Steps [1..N-1] succeeded.
     Step N failed at target: 'input#coupon_code'.
     Current active page DOM snippet: [...]
     Please provide the updated selector for Step N.
     ```
3. **Cache Invalidation & Patch**:
   - LLM verifies the new element.
   - WebReflex updates the fallback selector chain for Step $N$.
   - Version incremented and saved to local SQLite.
