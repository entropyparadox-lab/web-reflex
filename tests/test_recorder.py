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
from web_reflex.recorder import BrowserRecorder, RECORDER_SCRIPT

HTML_RECORD_PAGE = """<!DOCTYPE html>
<html>
<head><title>WebReflex Recorder Test</title></head>
<body>
  <h2>Search & Submit</h2>
  <form id="search-form" onsubmit="event.preventDefault()">
    <input type="text" id="query_input" name="q" placeholder="Search..." />
    <button type="submit" id="search_btn">Search</button>
  </form>
</body>
</html>
"""


class RecordHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.end_headers()
        self.wfile.write(HTML_RECORD_PAGE.encode("utf-8"))

    def log_message(self, format, *args):
        pass


def get_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        s.listen(1)
        return s.getsockname()[1]


class ReusableTCPServer(socketserver.TCPServer):
    allow_reuse_address = True


@pytest.fixture(scope="module")
def record_web_server():
    port = get_free_port()
    httpd = ReusableTCPServer(("127.0.0.1", port), RecordHandler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    httpd.shutdown()
    httpd.server_close()


@pytest.fixture(scope="module")
def reflex_daemon():
    port = get_free_port()
    db_path = f"/tmp/test_reflex_rec_{port}.db"
    if os.path.exists(db_path):
        os.remove(db_path)

    binary_path = (
        Path(__file__).parent.parent / "target" / "release" / "web-reflex"
    )

    proc = subprocess.Popen(
        [str(binary_path), "serve", "--port", str(port), "--db", db_path],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    daemon_url = f"http://127.0.0.1:{port}"

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


def test_browser_recorder_auto_extraction(record_web_server, reflex_daemon):
    client = ReflexClient(endpoint=reflex_daemon)

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        page.add_init_script(RECORDER_SCRIPT)
        page.goto(record_web_server)
        page.evaluate(RECORDER_SCRIPT)

        # Simulate typing and clicking
        page.focus("#query_input")
        page.type("#query_input", "RTX 5090")
        page.click("#search_btn")

        events = page.evaluate("() => window.__web_reflex_events || []")
        html = page.content()
        browser.close()

        assert len(events) >= 2
        assert events[0]["type"] == "type"
        assert events[0]["target"] == "#query_input"
        assert events[0]["value"] == "RTX 5090"
        assert events[1]["type"] == "click"
        assert events[1]["target"] == "#search_btn"

        # Save to DB via recorder logic
        skeleton_hash = client.hash_html(html)
        graph_dict = {
            "graph_id": "auto_recorded_search",
            "domain_pattern": record_web_server.replace("http://", ""),
            "skeleton_hash": skeleton_hash,
            "version": 1,
            "nodes": [
                {
                    "step_id": "step_1_type",
                    "action_type": "type",
                    "safety_level": "idempotent",
                    "requires_approval": False,
                    "target": {
                        "primary": events[0]["target"],
                        "fallbacks": [],
                        "aria_name": None,
                    },
                    "value_slot": "$QUERY",
                },
                {
                    "step_id": "step_2_click",
                    "action_type": "click",
                    "safety_level": "mutating_write"
                    if events[1].get("is_mutating")
                    else "idempotent",
                    "requires_approval": False,
                    "target": {
                        "primary": events[1]["target"],
                        "fallbacks": [],
                        "aria_name": None,
                    },
                    "value_slot": None,
                },
            ],
            "created_at": "",
            "updated_at": "",
        }

        res = client.record(graph_dict)
        assert res["status"] == "saved"

        # Verify inspect hit
        status, graph, _ = client.inspect(html)
        assert status == "hit"
        assert graph.graph_id == "auto_recorded_search"
        assert len(graph.nodes) == 2
        print("\n[Recorder Test] Auto-extraction & inspection verified successfully!")
