export type SafetyLevel = "read_only" | "idempotent" | "mutating_write";

export type ActionType =
  | "click"
  | "type"
  | "select"
  | "navigate"
  | "wait_for"
  | "assert";

export interface SelectorChain {
  primary: string;
  fallbacks?: string[];
  aria_name?: string;
}

export interface ActionNode {
  step_id: string;
  action_type: ActionType;
  safety_level: SafetyLevel;
  requires_approval?: boolean;
  target: SelectorChain;
  value_slot?: string;
  pre_condition?: {
    visible_selector?: string;
    url_pattern?: string;
    timeout_ms?: number;
  };
  post_condition?: {
    wait_for_selector?: string;
    expected_url?: string;
    state_hash?: string;
  };
}

export interface ActionGraph {
  graph_id: string;
  domain_pattern: string;
  skeleton_hash: string;
  version: number;
  nodes: ActionNode[];
  created_at?: string;
  updated_at?: string;
}

export type InspectStatus = "hit" | "candidate" | "miss";

export interface InspectResponse {
  status: InspectStatus;
  graph?: ActionGraph;
  current_skeleton_hash?: string;
  skeleton_hash?: string;
}

export interface HealResponse {
  status: "healed";
  graph: ActionGraph;
}
