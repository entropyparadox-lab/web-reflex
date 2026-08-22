use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use web_reflex_core::{ActionGraph, SkeletonHasher};
use web_reflex_engine::{FastPathResult, ReplayEngine, SelfHealingManager};
use web_reflex_storage::ActionStorage;

pub struct AppState {
    pub storage: Arc<ActionStorage>,
    pub engine: ReplayEngine,
    pub healing: SelfHealingManager,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Deserialize)]
pub struct HtmlRequest {
    pub html: String,
}

#[derive(Deserialize)]
pub struct InspectRequest {
    pub html: String,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Serialize)]
pub struct HashResponse {
    pub skeleton_hash: String,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InspectResponse {
    Hit {
        graph: ActionGraph,
    },
    Candidate {
        graph: ActionGraph,
        current_skeleton_hash: String,
    },
    Miss {
        skeleton_hash: String,
    },
}

#[derive(Deserialize)]
pub struct RecordRequest {
    pub graph: ActionGraph,
}

#[derive(Serialize)]
pub struct RecordResponse {
    pub status: &'static str,
    pub graph_id: String,
    pub version: u32,
}

#[derive(Deserialize)]
pub struct HealRequest {
    pub graph: ActionGraph,
    pub step_id: String,
    pub new_primary_selector: String,
    #[serde(default)]
    pub new_skeleton_hash: Option<String>,
}

#[derive(Serialize)]
pub struct HealResponse {
    pub status: &'static str,
    pub graph: ActionGraph,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/hash", post(hash_handler))
        .route("/api/v1/inspect", post(inspect_handler))
        .route("/api/v1/record", post(record_handler))
        .route("/api/v1/heal", post(heal_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: "0.1.0",
    })
}

async fn hash_handler(Json(payload): Json<HtmlRequest>) -> Json<HashResponse> {
    let hash = SkeletonHasher::compute_hash(&payload.html);
    Json(HashResponse {
        skeleton_hash: hash,
    })
}

async fn inspect_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InspectRequest>,
) -> Result<Json<InspectResponse>, (StatusCode, String)> {
    match state
        .engine
        .inspect_page_with_domain(&payload.html, payload.domain.as_deref())
    {
        Ok(FastPathResult::Hit(graph)) => Ok(Json(InspectResponse::Hit { graph })),
        Ok(FastPathResult::DomainCandidate {
            graph,
            current_skeleton_hash,
        }) => Ok(Json(InspectResponse::Candidate {
            graph,
            current_skeleton_hash,
        })),
        Ok(FastPathResult::Miss { skeleton_hash }) => {
            Ok(Json(InspectResponse::Miss { skeleton_hash }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn record_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RecordRequest>,
) -> Result<Json<RecordResponse>, (StatusCode, String)> {
    match state.storage.save_graph(&payload.graph) {
        Ok(_) => Ok(Json(RecordResponse {
            status: "saved",
            graph_id: payload.graph.graph_id,
            version: payload.graph.version,
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn heal_handler(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<HealRequest>,
) -> Result<Json<HealResponse>, (StatusCode, String)> {
    if let Some(new_hash) = payload.new_skeleton_hash {
        payload.graph.skeleton_hash = new_hash;
    }

    match state.healing.apply_patch(
        payload.graph,
        &payload.step_id,
        payload.new_primary_selector,
    ) {
        Ok(healed_graph) => Ok(Json(HealResponse {
            status: "healed",
            graph: healed_graph,
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn run_server(addr: SocketAddr, storage: Arc<ActionStorage>) -> anyhow::Result<()> {
    let engine = ReplayEngine::new(storage.clone());
    let healing = SelfHealingManager::new(storage.clone());

    let state = Arc::new(AppState {
        storage,
        engine,
        healing,
    });

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("WebReflex Daemon listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
