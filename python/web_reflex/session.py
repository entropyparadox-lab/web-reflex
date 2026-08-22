import time
from urllib.parse import urlparse
from typing import Any, Callable, Dict, Optional
from web_reflex.client import ReflexClient
from web_reflex.models import ActionGraph, ActionNode, ActionType, SafetyLevel


class ReflexSafetyError(Exception):
    pass


class ReflexSession:
    def __init__(
        self,
        page: Any,
        endpoint: str = "http://127.0.0.1:9199",
        approval_callback: Optional[Callable[[ActionNode], bool]] = None,
        llm_healer: Optional[Callable[[Dict[str, Any]], str]] = None,
    ):
        self.page = page
        self.client = ReflexClient(endpoint)
        self.approval_callback = approval_callback
        self.llm_healer = llm_healer

    def execute(self, value_slots: Optional[Dict[str, str]] = None) -> Dict[str, Any]:
        slots = value_slots or {}
        html = self.page.content()
        parsed_url = urlparse(self.page.url)
        domain = parsed_url.netloc or parsed_url.path

        status, graph, curr_hash = self.client.inspect(html, domain=domain)

        if not graph:
            return {
                "status": "miss",
                "message": "No cached action graph found for this page skeleton.",
            }

        start_time = time.perf_counter()
        completed_steps = []

        for node in graph.nodes:
            # 1. Safety Gate Evaluation
            if node.requires_approval or node.safety_level == SafetyLevel.MUTATING_WRITE:
                if self.approval_callback:
                    approved = self.approval_callback(node)
                    if not approved:
                        raise ReflexSafetyError(
                            f"Execution blocked: Step '{node.step_id}' was rejected by human gate."
                        )
                else:
                    raise ReflexSafetyError(
                        f"Execution blocked: Step '{node.step_id}' requires human approval."
                    )

            # 2. Resolve slot value if applicable
            resolved_value = None
            if node.value_slot:
                slot_key = node.value_slot.lstrip("$")
                resolved_value = slots.get(slot_key, slots.get(node.value_slot, ""))

            # 3. Execute with Fallback & Self-Healing
            success = self._execute_node(node, resolved_value)
            if not success:
                # Trigger Self-Healing if LLM healer is configured
                if self.llm_healer:
                    context = {
                        "graph_id": graph.graph_id,
                        "version": graph.version,
                        "failed_step_id": node.step_id,
                        "html_snippet": self.page.content()[:3000],
                    }
                    new_selector = self.llm_healer(context)
                    if new_selector:
                        # Patch graph in daemon with updated selector and new skeleton hash if changed
                        graph_dict = {
                            "graph_id": graph.graph_id,
                            "domain_pattern": graph.domain_pattern,
                            "skeleton_hash": graph.skeleton_hash,
                            "version": graph.version,
                            "nodes": [
                                {
                                    "step_id": n.step_id,
                                    "action_type": n.action_type.value,
                                    "safety_level": n.safety_level.value,
                                    "requires_approval": n.requires_approval,
                                    "target": {
                                        "primary": n.target.primary,
                                        "fallbacks": n.target.fallbacks,
                                        "aria_name": n.target.aria_name,
                                    },
                                    "value_slot": n.value_slot,
                                }
                                for n in graph.nodes
                            ],
                        }
                        healed_graph = self.client.heal(
                            graph_dict,
                            node.step_id,
                            new_selector,
                            new_skeleton_hash=curr_hash,
                        )
                        graph = healed_graph
                        # Retry execution with healed selector
                        node.target.primary = new_selector
                        success = self._execute_node(node, resolved_value)

                if not success:
                    return {
                        "status": "failed",
                        "failed_step": node.step_id,
                        "completed_steps": completed_steps,
                    }

            completed_steps.append(node.step_id)

        elapsed_ms = (time.perf_counter() - start_time) * 1000.0
        return {
            "status": "success",
            "graph_id": graph.graph_id,
            "version": graph.version,
            "completed_steps": completed_steps,
            "elapsed_ms": round(elapsed_ms, 2),
        }

    def _execute_node(self, node: ActionNode, value: Optional[str]) -> bool:
        selectors = [node.target.primary] + node.target.fallbacks
        for sel in selectors:
            try:
                if node.action_type == ActionType.CLICK:
                    self.page.click(sel, timeout=1000)
                    return True
                elif node.action_type == ActionType.TYPE:
                    self.page.fill(sel, value or "", timeout=1000)
                    return True
                elif node.action_type == ActionType.WAIT_FOR:
                    self.page.wait_for_selector(sel, timeout=1000)
                    return True
            except Exception:
                continue
        return False
