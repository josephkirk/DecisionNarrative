//! Axum HTTP router and request handlers for Storylet Studio.

use std::path::PathBuf;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::contracts::LibraryQuery;
use crate::service::StudioService;

pub fn build_router(service: StudioService, static_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/v2/authoring-options", get(get_authoring_options))

        // Project metadata & bundles
        .route("/api/v2/project", get(get_project_meta).patch(patch_project_meta))
        .route("/api/v1/project", get(get_project_meta).patch(patch_project_meta))
        .route("/api/v2/project-bundle", get(export_bundle))
        .route("/api/v1/project-bundle", get(export_bundle))
        .route("/api/v2/project-bundle/validate", post(validate_bundle))
        .route("/api/v1/project-bundle/validate", post(validate_bundle))
        .route("/api/v2/project-bundle/import", post(import_bundle))
        .route("/api/v1/project-bundle/import", post(import_bundle))

        // Library query
        .route("/api/v2/library", get(query_library))

        // Content CRUD
        .route("/api/v2/content/{entity}", get(list_content).post(save_content))
        .route("/api/v2/content/{entity}/{id}", get(get_content).put(update_content).delete(delete_content))
        .route("/api/v2/storylets/{id}/approve", post(approve_storylet))

        // Direct legacy aliases
        .route("/api/qualities", get(list_qualities).post(save_quality))
        .route("/api/qualities/{id}", delete(delete_quality))
        .route("/api/characters", get(list_characters).post(save_character))
        .route("/api/characters/{id}", delete(delete_character))
        .route("/api/locations", get(list_locations).post(save_location))
        .route("/api/locations/{id}", delete(delete_location))
        .route("/api/npc-behaviors", get(list_npc_behaviors).post(save_npc_behavior))
        .route("/api/npc-behaviors/{id}", delete(delete_npc_behavior))
        .route("/api/storylets", get(list_storylets).post(save_storylet))
        .route("/api/storylets/{id}", delete(delete_storylet))
        .route("/api/storylets/{id}/approve", post(approve_storylet))
        .route("/api/events", get(list_events).post(save_event))
        .route("/api/events/{id}", delete(delete_event))

        // Scenarios
        .route("/api/v2/scenarios", get(list_scenarios).post(save_scenario))
        .route("/api/v1/scenarios", get(list_scenarios).post(save_scenario))
        .route("/api/v2/scenarios/{id}", get(get_scenario).put(save_scenario_with_id).delete(delete_scenario))
        .route("/api/v1/scenarios/{id}", get(get_scenario).put(save_scenario_with_id).delete(delete_scenario))
        .route("/api/v2/scenarios/preview", post(preview_scenario))

        // Generation
        .route("/api/v2/generation-jobs", get(list_gen_jobs).post(create_gen_job))
        .route("/api/v1/generation-jobs", get(list_gen_jobs).post(create_gen_job))
        .route("/api/v2/generation-jobs/{id}", get(get_gen_job).delete(delete_gen_job))
        .route("/api/v1/generation-jobs/{id}", get(get_gen_job).delete(delete_gen_job))
        .route("/api/v2/generation-jobs/{id}/advance", post(advance_gen_job))
        .route("/api/v1/generation-jobs/{id}/advance", post(advance_gen_job))
        .route("/api/v2/generation-jobs/{id}/stages/{stage}", put(edit_gen_stage))
        .route("/api/v1/generation-jobs/{id}/stages/{stage}", put(edit_gen_stage))
        .route("/api/v2/generation-jobs/{id}/draft", post(save_gen_draft))
        .route("/api/v1/generation-jobs/{id}/draft", post(save_gen_draft))
        .route("/api/v2/generation-jobs/{id}/events", get(gen_job_stream))
        .route("/api/v1/generation-jobs/{id}/events", get(gen_job_stream))
        .route("/api/v2/craft-resources", get(list_craft_resources))
        .route("/api/v1/craft-resources", get(list_craft_resources))

        // Playtest sessions
        .route("/api/v1/playtests/from-workspace", post(create_playtest_from_workspace))
        .route("/api/v1/playtests", get(list_playtests).post(create_playtest))
        .route("/api/v1/playtests/{id}", get(get_playtest_snapshot).delete(delete_playtest))
        .route("/api/v1/playtests/{id}/commands", post(apply_playtest_command))
        .route("/api/v1/playtests/{id}/choices", post(apply_playtest_choice))
        .route("/api/v1/playtests/{id}/director", get(evaluate_director))
        .route("/api/v1/playtests/{id}/director/decision", post(apply_director_decision))
        .route("/api/v1/playtests/{id}/director/semantic-rank", post(apply_semantic_rank))
        .route("/api/v1/playtests/{id}/director/rank", post(rank_local_encoder))
        .route("/api/v1/playtests/{id}/events", get(get_playtest_events))
        .route("/api/v1/playtests/{id}/stream", get(playtest_stream))
        .route("/api/v1/playtests/{id}/fork", post(fork_playtest))
        .route("/api/v1/playtests/{id}/export", get(export_playtest))
        .route("/api/v1/playtests/restore", post(restore_playtest))
        .route("/api/v1/playtests/{id}/restore", post(restore_playtest))

        // MCP JSON-RPC
        .route("/mcp", post(handle_http_mcp))

        .layer(cors)
        .with_state(service);

    if let Some(dir) = static_dir {
        if dir.exists() {
            let serve_dir = ServeDir::new(&dir).fallback(ServeDir::new(&dir));
            router = router.fallback_service(serve_dir);
        }
    }

    router
}

async fn index_handler() -> Response {
    let content = match std::fs::read_to_string("dist/studio/index.html") {
        Ok(c) => c,
        Err(_) => std::fs::read_to_string("public/index.html").unwrap_or_default(),
    };
    let mut resp = Response::new(Body::from(content));
    resp.headers_mut().insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());
    resp
}

async fn health() -> Response {
    (StatusCode::OK, Json(json!({ "status": "ok", "service": "storylet-studio-rust" }))).into_response()
}

async fn get_authoring_options(State(service): State<StudioService>) -> Response {
    (StatusCode::OK, Json(service.get_authoring_options().await)).into_response()
}

async fn get_project_meta(State(service): State<StudioService>) -> Response {
    match service.get_project_metadata().await {
        Ok(m) => (StatusCode::OK, Json(json!(m))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct PatchProjectName {
    pub name: String,
}

async fn patch_project_meta(
    State(service): State<StudioService>,
    bytes: axum::body::Bytes,
) -> Response {
    let payload: PatchProjectName = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Request body must be valid JSON" }))).into_response(),
    };
    match service.update_project_name(&payload.name).await {
        Ok(m) => (StatusCode::OK, Json(json!(m))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn export_bundle(State(service): State<StudioService>) -> Response {
    match service.export_project_bundle().await {
        Ok(bundle) => {
            let filename = format!("{}.storylet.json", bundle.project.name.to_lowercase().replace(' ', "-"));
            let body = serde_json::to_string_pretty(&bundle).unwrap_or_default();
            let mut resp = Response::new(Body::from(body));
            resp.headers_mut().insert(header::CONTENT_TYPE, "application/json; charset=utf-8".parse().unwrap());
            resp.headers_mut().insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"").parse().unwrap());
            resp
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct BundlePayload {
    pub bundle: Value,
    #[serde(rename = "confirmReplace")]
    pub confirm_replace: Option<bool>,
}

async fn validate_bundle(
    State(service): State<StudioService>,
    bytes: axum::body::Bytes,
) -> Response {
    let payload: BundlePayload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Request body must be a JSON object" }))).into_response(),
    };
    match service.validate_bundle(&payload.bundle).await {
        Ok(issues) => (StatusCode::OK, Json(json!({ "valid": issues.is_empty(), "issues": issues }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "valid": false, "issues": [e] }))).into_response(),
    }
}

async fn import_bundle(
    State(service): State<StudioService>,
    Json(payload): Json<BundlePayload>,
) -> Response {
    if payload.confirm_replace != Some(true) {
        return (StatusCode::CONFLICT, Json(json!({ "error": "Explicit confirmReplace=true is required" }))).into_response();
    }
    match service.import_bundle(&payload.bundle).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true, "message": "Project replaced successfully" }))).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

async fn query_library(
    State(service): State<StudioService>,
    Query(query): Query<LibraryQuery>,
) -> Response {
    match service.query_library(&query).await {
        Ok(p) => (StatusCode::OK, Json(json!(p))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

// Content CRUD
async fn list_content(
    State(service): State<StudioService>,
    Path(entity): Path<String>,
) -> Response {
    match service.list_content(&entity).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn get_content(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
) -> Response {
    match service.get_content_item(&entity, &id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}

async fn save_content(
    State(service): State<StudioService>,
    Path(entity): Path<String>,
    Json(payload): Json<Value>,
) -> Response {
    match service.save_content_item(&entity, &payload).await {
        Ok(data) => (StatusCode::CREATED, Json(data)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn update_content(
    State(service): State<StudioService>,
    Path((entity, _id)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Response {
    match service.save_content_item(&entity, &payload).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn delete_content(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
) -> Response {
    match service.delete_content_item(&entity, &id).await {
        Ok(deleted) => (StatusCode::OK, Json(json!({ "deleted": deleted }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct ApproveBody {
    #[serde(rename = "expectedRevision")]
    pub expected_revision: u64,
}

async fn approve_storylet(
    State(service): State<StudioService>,
    Path(id): Path<String>,
    Json(body): Json<ApproveBody>,
) -> Response {
    match service.approve_storylet(&id, body.expected_revision).await {
        Ok(st) => (StatusCode::OK, Json(json!(st))).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

// Aliases
async fn list_qualities(State(s): State<StudioService>) -> Response { list_content(State(s), Path("qualities".into())).await }
async fn save_quality(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("qualities".into()), Json(p)).await }
async fn delete_quality(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("qualities".into(), id))).await }

async fn list_characters(State(s): State<StudioService>) -> Response { list_content(State(s), Path("characters".into())).await }
async fn save_character(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("characters".into()), Json(p)).await }
async fn delete_character(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("characters".into(), id))).await }

async fn list_locations(State(s): State<StudioService>) -> Response { list_content(State(s), Path("locations".into())).await }
async fn save_location(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("locations".into()), Json(p)).await }
async fn delete_location(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("locations".into(), id))).await }

async fn list_npc_behaviors(State(s): State<StudioService>) -> Response { list_content(State(s), Path("npc_behaviors".into())).await }
async fn save_npc_behavior(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("npc_behaviors".into()), Json(p)).await }
async fn delete_npc_behavior(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("npc_behaviors".into(), id))).await }

async fn list_storylets(State(s): State<StudioService>) -> Response { list_content(State(s), Path("storylets".into())).await }
async fn save_storylet(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("storylets".into()), Json(p)).await }
async fn delete_storylet(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("storylets".into(), id))).await }

async fn list_events(State(s): State<StudioService>) -> Response { list_content(State(s), Path("events".into())).await }
async fn save_event(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("events".into()), Json(p)).await }
async fn delete_event(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("events".into(), id))).await }

// Scenarios
async fn list_scenarios(State(s): State<StudioService>) -> Response { list_content(State(s), Path("scenarios".into())).await }
async fn get_scenario(State(s): State<StudioService>, Path(id): Path<String>) -> Response { get_content(State(s), Path(("scenarios".into(), id))).await }
async fn save_scenario(State(s): State<StudioService>, Json(p): Json<Value>) -> Response { save_content(State(s), Path("scenarios".into()), Json(p)).await }
async fn save_scenario_with_id(State(s): State<StudioService>, Path(id): Path<String>, Json(mut p): Json<Value>) -> Response {
    if let Some(obj) = p.as_object_mut() { obj.insert("id".into(), Value::String(id)); }
    save_content(State(s), Path("scenarios".into()), Json(p)).await
}
async fn delete_scenario(State(s): State<StudioService>, Path(id): Path<String>) -> Response { delete_content(State(s), Path(("scenarios".into(), id))).await }

async fn preview_scenario(
    State(service): State<StudioService>,
    Json(payload): Json<Value>,
) -> Response {
    match service.preview_scenario(&payload).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

// Generation
async fn list_gen_jobs(State(s): State<StudioService>) -> Response {
    match s.list_generation_jobs().await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn get_gen_job(State(s): State<StudioService>, Path(id): Path<String>) -> Response {
    match s.get_generation_job(&id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}

async fn create_gen_job(State(s): State<StudioService>, Json(p): Json<Value>) -> Response {
    match s.create_generation_job(p).await {
        Ok(job) => (StatusCode::CREATED, Json(json!(job))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct AdvanceGenBody {
    #[serde(rename = "expectedRevision")]
    pub expected_revision: u64,
}

async fn advance_gen_job(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(b): Json<AdvanceGenBody>,
) -> Response {
    match s.advance_generation_job(&id, b.expected_revision).await {
        Ok(job) => (StatusCode::OK, Json(json!(job))).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct EditStageBody {
    #[serde(rename = "expectedRevision")]
    pub expected_revision: u64,
    pub artifact: Value,
}

async fn edit_gen_stage(
    State(s): State<StudioService>,
    Path((id, stage)): Path<(String, String)>,
    Json(b): Json<EditStageBody>,
) -> Response {
    match s.edit_generation_stage(&id, &stage, b.expected_revision, b.artifact).await {
        Ok(job) => (StatusCode::OK, Json(json!(job))).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

async fn save_gen_draft(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.save_generation_draft(&id).await {
        Ok(st) => (StatusCode::OK, Json(json!(st))).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

async fn delete_gen_job(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.delete_generation_job(&id).await {
        Ok(del) => (StatusCode::OK, Json(json!({ "deleted": del }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn gen_job_stream(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = s.generation.job_events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |item| {
        if let Ok((job_id, payload)) = item {
            if job_id == id {
                let data = serde_json::to_string(&payload).unwrap_or_default();
                return Some(Ok(Event::default().event("generation").data(data)));
            }
        }
        None
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list_craft_resources(State(s): State<StudioService>) -> Response {
    match s.list_craft_resources().await {
        Ok(list) => (StatusCode::OK, Json(json!({ "resources": list }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

// Playtests
#[derive(Deserialize)]
struct WorkspacePlaytestRequest {
    #[serde(rename = "startLocationId")]
    pub start_location_id: Option<String>,
    #[serde(rename = "scenarioId")]
    pub scenario_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlaytestBody {
    #[serde(default)]
    pub session_id: Option<String>,
    pub scenario_id: String,
    pub project: storylet_engine::ProjectBundle,
}

async fn create_playtest(
    State(s): State<StudioService>,
    Json(body): Json<CreatePlaytestBody>,
) -> Response {
    let session_id = body.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    match s.create_playtest_session(&session_id, body.project, &body.scenario_id).await {
        Ok(data) => (StatusCode::CREATED, Json(data)).into_response(),
        Err(e) => {
            let code = if e.contains("already exists") || e.contains("cannot be reused") {
                StatusCode::CONFLICT
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            (code, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn create_playtest_from_workspace(
    State(s): State<StudioService>,
    Json(req): Json<WorkspacePlaytestRequest>,
) -> Response {
    match s.create_playtest_from_workspace(req.start_location_id.as_deref(), req.scenario_id.as_deref()).await {
        Ok(data) => (StatusCode::CREATED, Json(data)).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

async fn list_playtests(State(s): State<StudioService>) -> Response {
    match s.list_playtest_sessions().await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn get_playtest_snapshot(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.get_playtest_snapshot(&id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}

async fn delete_playtest(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.delete_playtest(&id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CommandPayload {
    Wrapped { command: storylet_engine::WorldCommand },
    Direct(storylet_engine::WorldCommand),
}

async fn apply_playtest_command(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let payload: CommandPayload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": format!("Invalid command JSON: {e}") }))).into_response(),
    };

    let cmd = match payload {
        CommandPayload::Wrapped { command } => command,
        CommandPayload::Direct(command) => command,
    };

    let cmd_val = match serde_json::to_value(&cmd) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    };

    match s.apply_playtest_command(&id, &cmd_val).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            let code = if e.contains("Failed to persist") || e.contains("write failure") {
                StatusCode::INTERNAL_SERVER_ERROR
            } else if e.contains("revision conflict") || e.contains("idempotency key") {
                StatusCode::CONFLICT
            } else if e.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            (code, Json(json!({ "error": e }))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct ChoiceBody {
    #[serde(rename = "storyletId")]
    pub storylet_id: String,
    #[serde(rename = "choiceId")]
    pub choice_id: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: u64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

async fn apply_playtest_choice(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(b): Json<ChoiceBody>,
) -> Response {
    match s.apply_playtest_choice(&id, &b.storylet_id, &b.choice_id, b.expected_revision, &b.idempotency_key).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct DirectorQuery {
    #[serde(rename = "targetTensionSlope", default = "default_slope")]
    pub target_tension_slope: f64,
}
fn default_slope() -> f64 { 0.15 }

#[derive(Deserialize)]
struct DecisionBody {
    pub decision: storylet_engine::DecisionResult,
    #[serde(rename = "targetTensionSlope", default = "default_slope")]
    pub target_tension_slope: f64,
}

async fn apply_director_decision(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Response {
    match s.apply_director_decision(&id, body.target_tension_slope, body.decision).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

async fn evaluate_director(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<DirectorQuery>,
) -> Response {
    match s.evaluate_playtest_director(&id, q.target_tension_slope).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn rank_local_encoder(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<DirectorQuery>,
) -> Response {
    match s.rank_playtest_with_local_encoder(&id, q.target_tension_slope).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn get_playtest_events(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.get_playtest_events(&id).await {
        Ok(data) => (StatusCode::OK, Json(json!({ "events": data }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

async fn playtest_stream(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    let initial_snapshot = match s.get_playtest_snapshot(&id).await {
        Ok(snap) => snap,
        Err(e) => return (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    };

    let initial: Result<Event, std::convert::Infallible> = Ok(Event::default()
        .event("playtest.snapshot")
        .data(json!({ "sessionId": id, "snapshot": initial_snapshot }).to_string()));

    let target_id = id.clone();
    let rx = s.updates.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |item| {
        if let Ok(change) = item {
            if change.session_id == target_id {
                let data = serde_json::to_string(&change.snapshot).unwrap_or_default();
                let ev: Result<Event, std::convert::Infallible> = Ok(Event::default().event("playtest.updated").data(data));
                return Some(ev);
            }
        }
        None
    });

    let combined = tokio_stream::once(initial).chain(stream);
    Sse::new(combined).keep_alive(KeepAlive::default()).into_response()
}

#[derive(Deserialize)]
struct ForkBody {
    #[serde(rename = "branchSessionId")]
    pub branch_session_id: Option<String>,
    #[serde(rename = "eventId")]
    pub event_id: String,
}

async fn fork_playtest(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(b): Json<ForkBody>,
) -> Response {
    match s.fork_playtest(&id, b.branch_session_id.as_deref(), &b.event_id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

// HTTP MCP
async fn export_playtest(
    State(s): State<StudioService>,
    Path(id): Path<String>,
) -> Response {
    match s.export_playtest_session(&id).await {
        Ok(state) => (StatusCode::OK, Json(json!(state))).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RestoreBody {
    Wrapped { state: storylet_engine::RuntimeSessionState },
    Direct(storylet_engine::RuntimeSessionState),
}

async fn restore_playtest(
    State(s): State<StudioService>,
    bytes: axum::body::Bytes,
) -> Response {
    let body: RestoreBody = match serde_json::from_slice(&bytes) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    };

    let state = match body {
        RestoreBody::Wrapped { state } => state,
        RestoreBody::Direct(state) => state,
    };

    match s.restore_playtest_session(state).await {
        Ok(data) => (StatusCode::CREATED, Json(data)).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct SemanticRankBody {
    pub request: storylet_engine::SemanticRankingRequest,
    #[serde(rename = "targetTensionSlope", default = "default_slope")]
    pub target_tension_slope: f64,
}

async fn apply_semantic_rank(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(body): Json<SemanticRankBody>,
) -> Response {
    match s.apply_semantic_rank(&id, body.target_tension_slope, body.request).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

async fn handle_http_mcp(
    State(s): State<StudioService>,
    Json(msg): Json<Value>,
) -> Response {
    if let Some(resp) = crate::mcp::handle_mcp_message(&s, &msg).await {
        (StatusCode::OK, Json(resp)).into_response()
    } else {
        (StatusCode::NO_CONTENT, Json(Value::Null)).into_response()
    }
}
