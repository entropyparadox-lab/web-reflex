from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional


class SafetyLevel(str, Enum):
    READ_ONLY = "read_only"
    IDEMPOTENT = "idempotent"
    MUTATING_WRITE = "mutating_write"


class ActionType(str, Enum):
    CLICK = "click"
    TYPE = "type"
    SELECT = "select"
    NAVIGATE = "navigate"
    WAIT_FOR = "wait_for"
    ASSERT = "assert"


@dataclass
class SelectorChain:
    primary: str
    fallbacks: List[str] = field(default_factory=list)
    aria_name: Optional[str] = None

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SelectorChain":
        return cls(
            primary=data.get("primary", ""),
            fallbacks=data.get("fallbacks", []),
            aria_name=data.get("aria_name"),
        )


@dataclass
class ActionNode:
    step_id: str
    action_type: ActionType
    safety_level: SafetyLevel
    target: SelectorChain
    requires_approval: bool = False
    value_slot: Optional[str] = None

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ActionNode":
        return cls(
            step_id=data["step_id"],
            action_type=ActionType(data["action_type"]),
            safety_level=SafetyLevel(data["safety_level"]),
            target=SelectorChain.from_dict(data["target"]),
            requires_approval=data.get("requires_approval", False),
            value_slot=data.get("value_slot"),
        )


@dataclass
class ActionGraph:
    graph_id: str
    domain_pattern: str
    skeleton_hash: str
    version: int
    nodes: List[ActionNode]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ActionGraph":
        return cls(
            graph_id=data["graph_id"],
            domain_pattern=data["domain_pattern"],
            skeleton_hash=data["skeleton_hash"],
            version=data["version"],
            nodes=[ActionNode.from_dict(n) for n in data["nodes"]],
        )
