use anyhow::Result;
use std::sync::Arc;
use web_reflex_core::{
    ActionGraph, ActionNode, ActionType, SafetyLevel, SelectorChain, SkeletonHasher,
};
use web_reflex_engine::{
    FastPathResult, ReplayEngine, ReplayProgress, SafetyGate, SafetyVerdict, SelfHealingManager,
};
use web_reflex_storage::ActionStorage;

#[test]
fn test_end_to_end_fast_path_and_self_healing() -> Result<()> {
    // 1. Initial Web Page HTML (Checkout page)
    let initial_html = r#"
        <div class="shop-container css-1a2b3c">
            <header><h2>Checkout</h2></header>
            <form id="order-form">
                <input type="text" id="coupon_input" placeholder="Coupon" />
                <button type="submit" id="pay_btn" class="btn-primary">Pay Now</button>
            </form>
        </div>
    "#;

    let storage = Arc::new(ActionStorage::in_memory()?);
    let initial_hash = SkeletonHasher::compute_hash(initial_html);

    // 2. Pre-seed Action Graph (Simulating recorded user recipe)
    let initial_graph = ActionGraph {
        graph_id: "checkout_flow".to_string(),
        domain_pattern: "shop.com/checkout".to_string(),
        skeleton_hash: initial_hash.clone(),
        version: 1,
        nodes: vec![
            ActionNode {
                step_id: "step_1_coupon".to_string(),
                action_type: ActionType::Type,
                safety_level: SafetyLevel::Idempotent,
                requires_approval: false,
                target: SelectorChain::new("#coupon_input"),
                value_slot: Some("$COUPON".to_string()),
                pre_condition: None,
                post_condition: None,
            },
            ActionNode {
                step_id: "step_2_pay".to_string(),
                action_type: ActionType::Click,
                safety_level: SafetyLevel::MutatingWrite,
                requires_approval: true,
                target: SelectorChain::new("#pay_btn"),
                value_slot: None,
                pre_condition: None,
                post_condition: None,
            },
        ],
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };
    storage.save_graph(&initial_graph)?;

    let engine = ReplayEngine::new(storage.clone());
    let healing = SelfHealingManager::new(storage.clone());

    // 3. Fast-Path Inspection on unchanged page -> CACHE HIT!
    let inspection = engine.inspect_page(initial_html)?;
    match inspection {
        FastPathResult::Hit(graph) => {
            assert_eq!(graph.graph_id, "checkout_flow");
            assert_eq!(graph.version, 1);
            // Verify Step 1 is auto-approved, Step 2 requires gate
            assert_eq!(
                SafetyGate::evaluate(&graph.nodes[0]),
                SafetyVerdict::ApprovedForAutoExecution
            );
            assert!(matches!(
                SafetyGate::evaluate(&graph.nodes[1]),
                SafetyVerdict::RequiresApproval { .. }
            ));
        }
        FastPathResult::Miss { .. } => panic!("Expected cache HIT on initial page"),
    }

    // 4. Simulate A/B Testing / UI Update: CSS ID changed to #pay_btn_v2
    let updated_html = r#"
        <div class="shop-container css-9z8y7x">
            <header><h2>Checkout</h2></header>
            <form id="order-form">
                <input type="text" id="coupon_input" placeholder="Coupon" />
                <button type="submit" id="pay_btn_v2" class="btn-primary-new">Pay Now</button>
            </form>
        </div>
    "#;

    // Notice: DomSanitizer strips dynamic classes, but #pay_btn_v2 is a semantic structural change.
    // If selector fails during replay, Self-Healing kicks in:
    let simulated_progress = ReplayProgress {
        completed_steps: vec!["step_1_coupon".to_string()],
        failed_step: Some(initial_graph.nodes[1].clone()),
        failure_reason: Some("Element '#pay_btn' not found on page".to_string()),
    };

    let hand_off =
        healing.prepare_hand_off_payload(&initial_graph, &simulated_progress, updated_html);
    assert!(hand_off.contains("step_2_pay"));
    assert!(hand_off.contains("step_1_coupon"));

    // 5. LLM / Healer resolves new selector '#pay_btn_v2' and applies patch
    let healed_graph =
        healing.apply_patch(initial_graph, "step_2_pay", "#pay_btn_v2".to_string())?;
    assert_eq!(healed_graph.version, 2);
    assert_eq!(healed_graph.nodes[1].target.primary, "#pay_btn_v2");
    assert_eq!(healed_graph.nodes[1].target.fallbacks[0], "#pay_btn");

    println!(
        "E2E Simulation: Fast-Path + Safety Gate + Self-Healing Hand-off verified successfully!"
    );
    Ok(())
}
