import http.server
import os
import socket
import socketserver
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest
import requests
from playwright.sync_api import sync_playwright

# Add python SDK to sys.path
sys.path.insert(0, str(Path(__file__).parent.parent / "python"))

from web_reflex.client import ReflexClient
from web_reflex.session import ReflexSession

HTML_V1 = """<!DOCTYPE html>
<html>
<head><title>WebReflex Live Shop</title></head>
<body>
  <h2>Checkout Page</h2>
  <form id="checkout-form">
    <input type="text" id="coupon_input" placeholder="Coupon" />
    <button type="button" id="pay_btn" onclick="document.body.setAttribute('data-paid', 'true')">Pay</button>
  </form>
</body>
</html>
"""

HTML_V2 = """<!DOCTYPE html>
<html>
<head><title>WebReflex Live Shop</title></head>
<body>
  <h2>Checkout Page</h2>
  <form id="checkout-form">
    <input type="text" id="coupon_input" placeholder="Coupon" />
    <button type="button" id="pay_btn_v2" onclick="document.body.setAttribute('data-paid', 'true')">Pay</button>
  </form>
</body>
</html>
"""

current_html = HTML_V1


class DynamicHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        global current_html
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.end_headers()
        self.wfile.write(current_html.encode("utf-8"))

    def log_message(self, format, *args):
        pass  # Quiet logs


def get_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        s.listen(1)
        port = s.getsockname()[1]
    return port


class ReusableTCPServer(socketserver.TCPServer):
    allow_reuse_address = True


@pytest.fixture(scope="module")
def local_web_server():
    port = get_free_port()
    httpd = ReusableTCPServer(("127.0.0.1", port), DynamicHandler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    httpd.shutdown()
    httpd.server_close()


@pytest.fixture(scope="module")
def reflex_daemon():
    port = get_free_port()
    db_path = f"/tmp/test_reflex_live_{port}.db"
    if os.path.exists(db_path):
        os.remove(db_path)

    binary_path = (
        Path(__file__).parent.parent / "target" / "release" / "web-reflex"
    )
    if not binary_path.exists():
        binary_path = (
            Path(__file__).parent.parent / "target" / "debug" / "web-reflex"
        )

    proc = subprocess.Popen(
        [str(binary_path), "serve", "--port", str(port), "--db", db_path],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    daemon_url = f"http://127.0.0.1:{port}"

    # Wait for daemon ready
    ready = False
    for _ in range(50):
        try:
            r = requests.get(f"{daemon_url}/api/v1/health", timeout=1)
            if r.status_code == 200:
                ready = True
                break
        except Exception:
            time.sleep(0.1)

    assert ready, "WebReflex Daemon failed to start"
    yield daemon_url

    proc.terminate()
    proc.wait()
    if os.path.exists(db_path):
        os.remove(db_path)


def test_live_playwright_fastpath_and_self_healing(
    local_web_server, reflex_daemon
):
    global current_html
    current_html = HTML_V1
    client = ReflexClient(reflex_daemon)

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        page.goto(local_web_server)

        # 1. Compute Skeleton Hash and Record Action Recipe
        html = page.content()
        hash_val = client.hash_html(html)

        graph_dict = {
            "graph_id": "live_checkout_flow",
            "domain_pattern": local_web_server.replace("http://", ""),
            "skeleton_hash": hash_val,
            "version": 1,
            "nodes": [
                {
                    "step_id": "fill_coupon",
                    "action_type": "type",
                    "safety_level": "idempotent",
                    "requires_approval": False,
                    "target": {
                        "primary": "#coupon_input",
                        "fallbacks": [],
                        "aria_name": None,
                    },
                    "value_slot": "$COUPON",
                },
                {
                    "step_id": "click_pay",
                    "action_type": "click",
                    "safety_level": "idempotent",
                    "requires_approval": False,
                    "target": {
                        "primary": "#pay_btn",
                        "fallbacks": [],
                        "aria_name": None,
                    },
                    "value_slot": None,
                },
            ],
            "created_at": "",
            "updated_at": "",
        }
        client.record(graph_dict)

        # 2. Run 1: Deterministic Fast-Path Execution (0ms token cost)
        session = ReflexSession(page, endpoint=reflex_daemon)
        res1 = session.execute(value_slots={"COUPON": "DISCOUNT2026"})

        assert res1["status"] == "success"
        assert res1["version"] == 1
        assert "fill_coupon" in res1["completed_steps"]
        assert "click_pay" in res1["completed_steps"]
        assert page.evaluate("() => document.body.getAttribute('data-paid')") == "true"
        print(f"\n[Run 1] Fast-Path Hit Elapsed: {res1['elapsed_ms']}ms")

        # 3. Simulate UI Mutation (A/B test or site update -> #pay_btn_v2)
        current_html = HTML_V2
        page.goto(local_web_server)
        assert page.evaluate("() => document.body.getAttribute('data-paid')") is None

        # 4. Run 2: Self-Healing Triggered via LLM mock healer
        healing_invoked = False

        def mock_llm_healer(ctx):
            nonlocal healing_invoked
            healing_invoked = True
            assert ctx["failed_step_id"] == "click_pay"
            return "#pay_btn_v2"

        healed_session = ReflexSession(
            page, endpoint=reflex_daemon, llm_healer=mock_llm_healer
        )
        res2 = healed_session.execute(value_slots={"COUPON": "DISCOUNT2026"})

        assert healing_invoked, "Self-Healing LLM hook should have been triggered"
        assert res2["status"] == "success"
        assert res2["version"] == 2
        assert page.evaluate("() => document.body.getAttribute('data-paid')") == "true"
        print(f"[Run 2] Self-Healed to v2 Elapsed: {res2['elapsed_ms']}ms")

        # 5. Run 3: Next execution on v2 page uses Fast-Path directly (No healer called)
        page.goto(local_web_server)
        healing_invoked = False
        res3 = healed_session.execute(value_slots={"COUPON": "DISCOUNT2026"})

        assert not healing_invoked, "Healer should NOT be called on cached v2 page"
        assert res3["status"] == "success"
        assert res3["version"] == 2
        assert page.evaluate("() => document.body.getAttribute('data-paid')") == "true"
        print(f"[Run 3] Direct v2 Fast-Path Hit Elapsed: {res3['elapsed_ms']}ms")

        browser.close()
