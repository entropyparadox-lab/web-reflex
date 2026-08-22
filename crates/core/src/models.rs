use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    ReadOnly,
    Idempotent,
    MutatingWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Click,
    Type,
    Select,
    Navigate,
    WaitFor,
    Assert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectorChain {
    pub primary: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aria_name: Option<String>,
}

impl SelectorChain {
    pub fn new(primary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
            fallbacks: Vec::new(),
            aria_name: None,
        }
    }

    pub fn with_fallback(mut self, fallback: impl Into<String>) -> Self {
        self.fallbacks.push(fallback.into());
        self
    }

    pub fn with_aria(mut self, aria: impl Into<String>) -> Self {
        self.aria_name = Some(aria.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreCondition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostCondition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_for_selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionNode {
    pub step_id: String,
    pub action_type: ActionType,
    pub safety_level: SafetyLevel,
    #[serde(default)]
    pub requires_approval: bool,
    pub target: SelectorChain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_condition: Option<PreCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_condition: Option<PostCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionGraph {
    pub graph_id: String,
    pub domain_pattern: String,
    pub skeleton_hash: String,
    pub version: u32,
    pub nodes: Vec<ActionNode>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}
