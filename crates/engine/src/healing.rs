use crate::runner::ReplayProgress;
use anyhow::{bail, Result};
use std::sync::Arc;
use web_reflex_core::{ActionGraph, ActionNode, SafetyLevel};
use web_reflex_storage::ActionStorage;

#[derive(Debug, PartialEq, Eq)]
pub enum SafetyVerdict {
    ApprovedForAutoExecution,
    RequiresApproval { reason: String },
}

pub struct SafetyGate;

impl SafetyGate {
    pub fn evaluate(node: &ActionNode) -> SafetyVerdict {
        if node.requires_approval || node.safety_level == SafetyLevel::MutatingWrite {
            SafetyVerdict::RequiresApproval {
                reason: format!(
                    "Step '{}' is a mutating action ({:?}) that modifies state or submits transactions.",
                    node.step_id, node.action_type
                ),
            }
        } else {
            SafetyVerdict::ApprovedForAutoExecution
        }
    }
}

pub struct SelfHealingManager {
    storage: Arc<ActionStorage>,
}

impl SelfHealingManager {
    pub fn new(storage: Arc<ActionStorage>) -> Self {
        Self { storage }
    }

    pub fn prepare_hand_off_payload(
        &self,
        graph: &ActionGraph,
        progress: &ReplayProgress,
        html_snippet: &str,
    ) -> String {
        let failed_id = progress
            .failed_step
            .as_ref()
            .map(|s| s.step_id.as_str())
            .unwrap_or("unknown");

        format!(
            "WebReflex Hand-off Context:
Graph ID: {} (v{})
Completed Steps: {:?}
Failed Step: {}
Failure Reason: {:?}

Active Page DOM Snippet:
{}

Goal: Provide the updated selector for step '{}' to heal the action graph.",
            graph.graph_id,
            graph.version,
            progress.completed_steps,
            failed_id,
            progress.failure_reason,
            html_snippet,
            failed_id
        )
    }

    pub fn apply_patch(
        &self,
        mut graph: ActionGraph,
        step_id: &str,
        new_primary_selector: String,
    ) -> Result<ActionGraph> {
        let mut found = false;
        for node in &mut graph.nodes {
            if node.step_id == step_id {
                let old_primary = node.target.primary.clone();
                node.target.fallbacks.insert(0, old_primary);
                node.target.primary = new_primary_selector.clone();
                found = true;
                break;
            }
        }

        if !found {
            bail!(
                "Step '{}' not found in ActionGraph '{}'",
                step_id,
                graph.graph_id
            );
        }

        graph.version += 1;
        self.storage.save_graph(&graph)?;
        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use web_reflex_core::{ActionType, SelectorChain};

    #[test]
    fn test_safety_gate() {
        let safe_node = ActionNode {
            step_id: "read_title".to_string(),
            action_type: ActionType::Assert,
            safety_level: SafetyLevel::ReadOnly,
            requires_approval: false,
            target: SelectorChain::new("h1"),
            value_slot: None,
            pre_condition: None,
            post_condition: None,
        };
        assert_eq!(
            SafetyGate::evaluate(&safe_node),
            SafetyVerdict::ApprovedForAutoExecution
        );

        let mutating_node = ActionNode {
            step_id: "pay_button".to_string(),
            action_type: ActionType::Click,
            safety_level: SafetyLevel::MutatingWrite,
            requires_approval: true,
            target: SelectorChain::new("button.pay"),
            value_slot: None,
            pre_condition: None,
            post_condition: None,
        };
        assert!(matches!(
            SafetyGate::evaluate(&mutating_node),
            SafetyVerdict::RequiresApproval { .. }
        ));
    }

    #[test]
    fn test_self_healing_patch() -> Result<()> {
        let storage = Arc::new(ActionStorage::in_memory()?);
        let manager = SelfHealingManager::new(storage.clone());

        let graph = ActionGraph {
            graph_id: "checkout".to_string(),
            domain_pattern: "shop.com".to_string(),
            skeleton_hash: "hash_xyz".to_string(),
            version: 1,
            nodes: vec![ActionNode {
                step_id: "coupon_input".to_string(),
                action_type: ActionType::Type,
                safety_level: SafetyLevel::Idempotent,
                requires_approval: false,
                target: SelectorChain::new("input#old_coupon"),
                value_slot: None,
                pre_condition: None,
                post_condition: None,
            }],
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };

        storage.save_graph(&graph)?;

        let healed =
            manager.apply_patch(graph, "coupon_input", "input#new_coupon_v2".to_string())?;
        assert_eq!(healed.version, 2);
        assert_eq!(healed.nodes[0].target.primary, "input#new_coupon_v2");
        assert_eq!(healed.nodes[0].target.fallbacks[0], "input#old_coupon");

        // Verify stored in DB with v2
        let db_graph = storage.find_by_skeleton_hash("hash_xyz")?.unwrap();
        assert_eq!(db_graph.version, 2);
        assert_eq!(db_graph.nodes[0].target.primary, "input#new_coupon_v2");

        Ok(())
    }
}
