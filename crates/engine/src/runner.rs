use anyhow::Result;
use std::sync::Arc;
use web_reflex_core::{ActionGraph, ActionNode, SkeletonHasher};
use web_reflex_storage::ActionStorage;

#[derive(Debug)]
pub enum FastPathResult {
    Hit(ActionGraph),
    Miss { skeleton_hash: String },
}

#[derive(Debug)]
pub struct ReplayProgress {
    pub completed_steps: Vec<String>,
    pub failed_step: Option<ActionNode>,
    pub failure_reason: Option<String>,
}

pub struct ReplayEngine {
    storage: Arc<ActionStorage>,
}

impl ReplayEngine {
    pub fn new(storage: Arc<ActionStorage>) -> Self {
        Self { storage }
    }

    pub fn inspect_page(&self, html: &str) -> Result<FastPathResult> {
        let skeleton_hash = SkeletonHasher::compute_hash(html);
        if let Some(graph) = self.storage.find_by_skeleton_hash(&skeleton_hash)? {
            Ok(FastPathResult::Hit(graph))
        } else {
            Ok(FastPathResult::Miss { skeleton_hash })
        }
    }
}
