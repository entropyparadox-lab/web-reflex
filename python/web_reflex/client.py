import requests
from typing import Any, Dict, Optional, Tuple
from web_reflex.models import ActionGraph


class ReflexClient:
    def __init__(self, endpoint: str = "http://127.0.0.1:9199"):
        self.endpoint = endpoint.rstrip("/")

    def health(self) -> Dict[str, Any]:
        resp = requests.get(f"{self.endpoint}/api/v1/health", timeout=2)
        resp.raise_for_status()
        return resp.json()

    def hash_html(self, html: str) -> str:
        resp = requests.post(f"{self.endpoint}/api/v1/hash", json={"html": html}, timeout=5)
        resp.raise_for_status()
        return resp.json()["skeleton_hash"]

    def inspect(self, html: str, domain: Optional[str] = None) -> Tuple[str, Optional[ActionGraph], Optional[str]]:
        payload = {"html": html, "domain": domain}
        resp = requests.post(f"{self.endpoint}/api/v1/inspect", json=payload, timeout=5)
        resp.raise_for_status()
        data = resp.json()
        status = data.get("status", "miss")
        if status in ("hit", "candidate"):
            graph = ActionGraph.from_dict(data["graph"])
            curr_hash = data.get("current_skeleton_hash")
            return status, graph, curr_hash
        return "miss", None, data.get("skeleton_hash")

    def record(self, graph_dict: Dict[str, Any]) -> Dict[str, Any]:
        resp = requests.post(f"{self.endpoint}/api/v1/record", json={"graph": graph_dict}, timeout=5)
        resp.raise_for_status()
        return resp.json()

    def heal(
        self,
        graph_dict: Dict[str, Any],
        step_id: str,
        new_primary_selector: str,
        new_skeleton_hash: Optional[str] = None,
    ) -> ActionGraph:
        resp = requests.post(
            f"{self.endpoint}/api/v1/heal",
            json={
                "graph": graph_dict,
                "step_id": step_id,
                "new_primary_selector": new_primary_selector,
                "new_skeleton_hash": new_skeleton_hash,
            },
            timeout=5,
        )
        resp.raise_for_status()
        data = resp.json()
        return ActionGraph.from_dict(data["graph"])
