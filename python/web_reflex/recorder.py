import json
import time
from typing import Any, Dict, List, Optional
from playwright.sync_api import Page, sync_playwright
from web_reflex.client import ReflexClient
from web_reflex.models import ActionType, SafetyLevel

RECORDER_SCRIPT = """
(function() {
    if (window.__web_reflex_injected) return;
    window.__web_reflex_injected = true;
    window.__web_reflex_events = [];

    function getCleanSelector(el) {
        if (!el || el.nodeType !== Node.ELEMENT_NODE) return '';

        // 1. Static ID
        if (el.id && !el.id.match(/^css-[a-z0-9]+|^tw-|^[0-9a-f]{8,}|[0-9]+/i)) {
            return '#' + el.id;
        }

        // 2. Name attribute
        if (el.name) {
            return `${el.tagName.toLowerCase()}[name="${el.name}"]`;
        }

        // 3. Aria Label
        const aria = el.getAttribute('aria-label') || el.getAttribute('title');
        if (aria) {
            return `${el.tagName.toLowerCase()}[aria-label="${aria}"]`;
        }

        // 4. Input placeholder
        if (el.placeholder) {
            return `${el.tagName.toLowerCase()}[placeholder="${el.placeholder}"]`;
        }

        // 5. Hierarchy path
        let path = el.tagName.toLowerCase();
        if (el.className && typeof el.className === 'string') {
            const cleanClasses = el.className.split(' ')
                .filter(c => !c.match(/^css-[a-z0-9]+|^tw-|^[0-9a-f]{8,}|_[a-z0-9]{4,}/i) && c.length < 25)
                .join('.');
            if (cleanClasses) path += '.' + cleanClasses;
        }
        return path;
    }

    document.addEventListener('input', function(e) {
        const target = e.target;
        const selector = getCleanSelector(target);
        let last = window.__web_reflex_events[window.__web_reflex_events.length - 1];
        if (last && last.type === 'type' && last.target === selector) {
            last.value = target.value;
        } else {
            window.__web_reflex_events.push({
                type: 'type',
                target: selector,
                tag: target.tagName.toLowerCase(),
                value: target.value,
                timestamp: Date.now()
            });
        }
    }, true);

    document.addEventListener('click', function(e) {
        const target = e.target;
        const selector = getCleanSelector(target);
        const isMutating = target.type === 'submit' || 
            (target.textContent && target.textContent.match(/결제|주문|삭제|Submit|Pay|Order|Delete|Transfer/i));

        window.__web_reflex_events.push({
            type: 'click',
            target: selector,
            tag: target.tagName.toLowerCase(),
            text: (target.textContent || '').trim().substring(0, 30),
            is_mutating: !!isMutating,
            timestamp: Date.now()
        });
    }, true);
})();
"""


class BrowserRecorder:
    def __init__(self, endpoint: str = "http://127.0.0.1:9199"):
        self.client = ReflexClient(endpoint)

    def record_interactive(
        self,
        url: str,
        graph_id: Optional[str] = None,
        headless: bool = False,
        timeout_sec: int = 120,
    ) -> Dict[str, Any]:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=headless)
            context = browser.new_context()
            page = context.new_page()

            page.add_init_script(RECORDER_SCRIPT)
            page.goto(url)

            print(f"🎬 WebReflex Recording started on {url}")
            print("👉 Perform actions in the browser window. Close the browser when finished.")

            start = time.time()
            try:
                while time.time() - start < timeout_sec:
                    if page.is_closed():
                        break
                    time.sleep(0.5)
            except Exception:
                pass

            try:
                events = page.evaluate("() => window.__web_reflex_events || []")
                html = page.content()
            except Exception:
                events = []
                html = ""

            browser.close()

            if not events or not html:
                return {"status": "empty", "message": "No actions recorded."}

            skeleton_hash = self.client.hash_html(html)
            domain = url.split("//")[-1].split("/")[0]

            nodes = []
            for idx, ev in enumerate(events, start=1):
                action_type = ActionType.CLICK if ev["type"] == "click" else ActionType.TYPE
                safety = (
                    SafetyLevel.MUTATING_WRITE
                    if ev.get("is_mutating")
                    else SafetyLevel.IDEMPOTENT
                )

                slot = f"$SLOT_{idx}" if ev["type"] == "type" else None

                nodes.append({
                    "step_id": f"step_{idx}_{ev['type']}",
                    "action_type": action_type.value,
                    "safety_level": safety.value,
                    "requires_approval": safety == SafetyLevel.MUTATING_WRITE,
                    "target": {
                        "primary": ev["target"],
                        "fallbacks": [],
                        "aria_name": None,
                    },
                    "value_slot": slot,
                })

            graph_dict = {
                "graph_id": graph_id or f"recipe_{int(time.time())}",
                "domain_pattern": domain,
                "skeleton_hash": skeleton_hash,
                "version": 1,
                "nodes": nodes,
                "created_at": "",
                "updated_at": "",
            }

            self.client.record(graph_dict)
            print(f"✅ Successfully recorded and saved Action Graph '{graph_dict['graph_id']}' with {len(nodes)} steps!")
            return {
                "status": "saved",
                "graph_id": graph_dict["graph_id"],
                "skeleton_hash": skeleton_hash,
                "steps": len(nodes),
            }
