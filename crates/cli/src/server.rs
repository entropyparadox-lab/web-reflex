use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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
use web_reflex_storage::{ActionStorage, GraphHistoryEntry, TenantStoragePool};

pub struct AppState {
    pub pool: Arc<TenantStoragePool>,
    pub api_key: Option<String>,
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

#[derive(Serialize)]
pub struct HistoryResponse {
    pub graph_id: String,
    pub history: Vec<GraphHistoryEntry>,
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    pub version: u32,
}

#[derive(Serialize)]
pub struct RollbackResponse {
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
        .route("/api/v1/graphs/{graph_id}/history", get(history_handler))
        .route("/api/v1/graphs/{graph_id}/rollback", post(rollback_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

fn check_auth(headers: &HeaderMap, expected_key: Option<&str>) -> Result<(), (StatusCode, String)> {
    if let Some(expected) = expected_key {
        let auth_header = headers.get("authorization").and_then(|h| h.to_str().ok());
        let x_api_key = headers.get("x-api-key").and_then(|h| h.to_str().ok());

        let token = if let Some(h) = auth_header {
            if let Some(bearer) = h.strip_prefix("Bearer ") {
                bearer.trim()
            } else {
                h.trim()
            }
        } else if let Some(k) = x_api_key {
            k.trim()
        } else {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Unauthorized: missing Authorization or X-API-Key header".to_string(),
            ));
        };

        if token != expected {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Unauthorized: invalid API key".to_string(),
            ));
        }
    }
    Ok(())
}

fn get_storage_for_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Arc<ActionStorage>, (StatusCode, String)> {
    let tenant = headers
        .get("x-tenant-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    state
        .pool
        .get_storage(tenant)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
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
    headers: HeaderMap,
    Json(payload): Json<InspectRequest>,
) -> Result<Json<InspectResponse>, (StatusCode, String)> {
    check_auth(&headers, state.api_key.as_deref())?;
    let storage = get_storage_for_request(&state, &headers)?;
    let engine = ReplayEngine::new(storage);

    match engine.inspect_page_with_domain(&payload.html, payload.domain.as_deref()) {
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
    headers: HeaderMap,
    Json(payload): Json<RecordRequest>,
) -> Result<Json<RecordResponse>, (StatusCode, String)> {
    check_auth(&headers, state.api_key.as_deref())?;
    let storage = get_storage_for_request(&state, &headers)?;

    match storage.save_graph(&payload.graph) {
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
    headers: HeaderMap,
    Json(mut payload): Json<HealRequest>,
) -> Result<Json<HealResponse>, (StatusCode, String)> {
    check_auth(&headers, state.api_key.as_deref())?;
    let storage = get_storage_for_request(&state, &headers)?;
    let healing = SelfHealingManager::new(storage);

    if let Some(new_hash) = payload.new_skeleton_hash {
        payload.graph.skeleton_hash = new_hash;
    }

    match healing.apply_patch(
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

async fn history_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(graph_id): Path<String>,
) -> Result<Json<HistoryResponse>, (StatusCode, String)> {
    check_auth(&headers, state.api_key.as_deref())?;
    let storage = get_storage_for_request(&state, &headers)?;

    match storage.get_history(&graph_id) {
        Ok(history) => Ok(Json(HistoryResponse { graph_id, history })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn rollback_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(graph_id): Path<String>,
    Json(payload): Json<RollbackRequest>,
) -> Result<Json<RollbackResponse>, (StatusCode, String)> {
    check_auth(&headers, state.api_key.as_deref())?;
    let storage = get_storage_for_request(&state, &headers)?;

    match storage.rollback_graph(&graph_id, payload.version) {
        Ok(graph) => Ok(Json(RollbackResponse {
            status: "restored",
            graph,
        })),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

pub async fn run_server(
    addr: SocketAddr,
    pool: Arc<TenantStoragePool>,
    api_key: Option<String>,
) -> anyhow::Result<()> {
    let state = Arc::new(AppState { pool, api_key });
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("WebReflex Daemon listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
