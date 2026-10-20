import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
import pytest
import requests

# Add python SDK to sys.path
sys.path.insert(0, str(Path(__file__).parent.parent / "python"))

from web_reflex.client import ReflexClient
from web_reflex.healers import redact_sensitive_html


def get_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        s.listen(1)
        return s.getsockname()[1]


@pytest.fixture
def enterprise_daemon():
    temp_dir = tempfile.mkdtemp(prefix="reflex_ent_test_")
    port = get_free_port()
    api_key = "super-secret-token-1234"

    binary_path = (
        Path(__file__).parent.parent / "target" / "release" / "web-reflex"
    )
    if not binary_path.exists():
        binary_path = (
            Path(__file__).parent.parent / "target" / "debug" / "web-reflex"
        )

    proc = subprocess.Popen(
        [
            str(binary_path),
            "serve",
            "--port",
            str(port),
            "--db-dir",
            temp_dir,
            "--api-key",
            api_key,
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    endpoint = f"http://127.0.0.1:{port}"
    ready = False
    for _ in range(50):
        try:
            r = requests.get(f"{endpoint}/api/v1/health", timeout=1)
            if r.status_code == 200:
                ready = True
                break
        except Exception:
            time.sleep(0.1)

    assert ready, "Enterprise Daemon failed to start"

    yield {
        "endpoint": endpoint,
        "api_key": api_key,
        "temp_dir": temp_dir,
        "binary_path": binary_path,
    }

    proc.terminate()
    proc.wait()
    shutil.rmtree(temp_dir, ignore_errors=True)


def test_pii_redaction():
    raw_html = """
    <div class="user-card">
        <span>Email: ceo@entropyparadox.com</span>
        <span>Phone: +82-10-9876-5432</span>
        <span>Card: 4111-2222-3333-4444</span>
        <input type="password" name="password" value="my_secret_pw123" />
        <button id="pay_now">Submit</button>
    </div>
    """
    cleaned = redact_sensitive_html(raw_html)
    assert "ceo@entropyparadox.com" not in cleaned
    assert "[EMAIL_REDACTED]" in cleaned
    assert "+82-10-9876-5432" not in cleaned
    assert "[PHONE_REDACTED]" in cleaned
    assert "4111-2222-3333-4444" not in cleaned
    assert "[CARD_REDACTED]" in cleaned
    assert "my_secret_pw123" not in cleaned
    assert 'value="[REDACTED]"' in cleaned
    assert '<button id="pay_now">Submit</button>' in cleaned


def test_api_key_auth(enterprise_daemon):
    endpoint = enterprise_daemon["endpoint"]
    api_key = enterprise_daemon["api_key"]

    # 1. Unauthenticated request to /inspect should return 401
    r_unauth = requests.post(f"{endpoint}/api/v1/inspect", json={"html": "<html></html>"})
    assert r_unauth.status_code == 401

    # 2. Invalid API key should return 401
    r_bad_auth = requests.post(
        f"{endpoint}/api/v1/inspect",
        json={"html": "<html></html>"},
        headers={"Authorization": "Bearer wrong-key"},
    )
    assert r_bad_auth.status_code == 401

    # 3. Valid Bearer token should succeed (200)
    client = ReflexClient(endpoint=endpoint, api_key=api_key)
    health = client.health()
    assert health["status"] == "ok"

    status, graph, _ = client.inspect("<html><body>test</body></html>")
    assert status == "miss"


def test_tenant_database_file_isolation(enterprise_daemon):
    endpoint = enterprise_daemon["endpoint"]
    api_key = enterprise_daemon["api_key"]
    temp_dir = Path(enterprise_daemon["temp_dir"])

    client_acme = ReflexClient(endpoint=endpoint, api_key=api_key, tenant_id="acme")
    client_globex = ReflexClient(endpoint=endpoint, api_key=api_key, tenant_id="globex")

    acme_graph = {
        "graph_id": "flow_login",
        "domain_pattern": "acme.corp",
        "skeleton_hash": "hash_acme_1",
        "version": 1,
        "nodes": [
            {
                "step_id": "click_btn",
                "action_type": "click",
                "safety_level": "read_only",
                "requires_approval": False,
                "target": {"primary": "#acme_login", "fallbacks": []},
            }
        ],
    }
    client_acme.record(acme_graph)

    # Verify physical file acme.db was created
    assert (temp_dir / "acme.db").exists()

    # Globex should not have access or see acme's flow
    globex_status, globex_graph, _ = client_globex.inspect("<html></html>", domain="acme.corp")
    assert globex_graph is None

    # Globex records its own
    globex_graph_data = {
        "graph_id": "flow_login",
        "domain_pattern": "globex.com",
        "skeleton_hash": "hash_globex_1",
        "version": 1,
        "nodes": [
            {
                "step_id": "click_btn",
                "action_type": "click",
                "safety_level": "read_only",
                "requires_approval": False,
                "target": {"primary": "#globex_btn", "fallbacks": []},
            }
        ],
    }
    client_globex.record(globex_graph_data)
    assert (temp_dir / "globex.db").exists()

    # Acme reads flow_login -> gets #acme_login
    _, acme_found, _ = client_acme.inspect("<html></html>", domain="acme.corp")
    assert acme_found is not None
    assert acme_found.nodes[0].target.primary == "#acme_login"

    # Globex reads flow_login -> gets #globex_btn
    _, globex_found, _ = client_globex.inspect("<html></html>", domain="globex.com")
    assert globex_found is not None
    assert globex_found.nodes[0].target.primary == "#globex_btn"


def test_version_history_and_rollback(enterprise_daemon):
    endpoint = enterprise_daemon["endpoint"]
    api_key = enterprise_daemon["api_key"]
    client = ReflexClient(endpoint=endpoint, api_key=api_key, tenant_id="versioning_tenant")

    graph = {
        "graph_id": "checkout_flow",
        "domain_pattern": "store.com",
        "skeleton_hash": "hash_checkout_v1",
        "version": 1,
        "nodes": [
            {
                "step_id": "pay_btn",
                "action_type": "click",
                "safety_level": "read_only",
                "requires_approval": False,
                "target": {"primary": "#pay_v1", "fallbacks": []},
            }
        ],
    }
    client.record(graph)

    # History should be empty at v1
    history = client.get_history("checkout_flow")
    assert len(history) == 0

    # Self-heal to v2
    v2_graph = client.heal(
        graph,
        step_id="pay_btn",
        new_primary_selector="#pay_v2",
        new_skeleton_hash="hash_checkout_v2",
    )
    assert v2_graph.version == 2
    assert v2_graph.nodes[0].target.primary == "#pay_v2"

    # History must contain archived v1
    history = client.get_history("checkout_flow")
    assert len(history) == 1
    assert history[0]["version"] == 1
    assert "#pay_v1" in history[0]["graph_json"]

    # Rollback to v1
    restored = client.rollback("checkout_flow", version=1)
    assert restored.version == 1
    assert restored.nodes[0].target.primary == "#pay_v1"

    # Check active graph is v1
    _, current, _ = client.inspect("<html></html>", domain="store.com")
    assert current is not None
    assert current.version == 1
    assert current.nodes[0].target.primary == "#pay_v1"

    # History should now also contain archived v2
    history_after = client.get_history("checkout_flow")
    assert len(history_after) == 2
    assert history_after[0]["version"] == 2


def test_cli_migrate_command(enterprise_daemon):
    binary_path = enterprise_daemon["binary_path"]
    temp_dir = enterprise_daemon["temp_dir"]

    # Run migrate command via CLI
    res = subprocess.run(
        [str(binary_path), "migrate", "--db-dir", temp_dir],
        capture_output=True,
        text=True,
        check=True,
    )
    assert "Running SQLite schema migrations to v3" in res.stdout
    assert "Migrated" in res.stdout


def test_telemetry_stats_and_heal_snapshot(enterprise_daemon):
    endpoint = enterprise_daemon["endpoint"]
    api_key = enterprise_daemon["api_key"]
    binary_path = enterprise_daemon["binary_path"]
    temp_dir = enterprise_daemon["temp_dir"]

    client = ReflexClient(endpoint=endpoint, api_key=api_key, tenant_id="stats_tenant")

    # 1. Record recipe with exact hash
    skeleton_hash = client.hash_html("<html><body>test</body></html>")
    graph = {
        "graph_id": "stats_flow",
        "domain_pattern": "stats.com",
        "skeleton_hash": skeleton_hash,
        "version": 1,
        "nodes": [
            {
                "step_id": "click_btn",
                "action_type": "click",
                "safety_level": "read_only",
                "requires_approval": False,
                "target": {"primary": "#btn_v1", "fallbacks": []},
            }
        ],
    }
    client.record(graph)

    # 2. Inspect hit (should record hit_count)
    status, _, _ = client.inspect("<html><body>test</body></html>", domain="stats.com")
    assert status == "hit"

    # 3. Heal (should record heal snapshot)
    healed = client.heal(
        graph,
        step_id="click_btn",
        new_primary_selector="#btn_v2",
        new_skeleton_hash="hash_stats_v2",
    )
    assert healed.version == 2

    # 4. Check stats via API
    stats = client.get_stats()
    assert stats["tenant_id"] == "stats_tenant"
    assert stats["total_graphs"] == 1
    assert stats["total_hits"] >= 1
    assert stats["total_heals"] == 1
    assert len(stats["recent_heals"]) == 1
    assert stats["recent_heals"][0]["new_selector"] == "#btn_v2"

    # 5. Check stats CLI command
    stats_res = subprocess.run(
        [str(binary_path), "stats", "--db-dir", temp_dir],
        capture_output=True,
        text=True,
        check=True,
    )
    assert "WebReflex Telemetry & Action Cache Performance Stats" in stats_res.stdout
    assert "stats_tenant" in stats_res.stdout
    assert "Cumulative Cache Hits" in stats_res.stdout
    assert "Total Self-Heal Events" in stats_res.stdout

