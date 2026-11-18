use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::contracts::LibraryQuery;
use crate::service::{ContentDeleteError, StudioService};

pub fn default_allowed_origins() -> Vec<String> {
    [
        "http://127.0.0.1:3000",
        "http://localhost:3000",
        "http://127.0.0.1:3001",
        "http://localhost:3001",
        "http://localhost:5173",
        "http://127.0.0.1:5173",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

async fn reject_disallowed_origins(
    State(allowed): State<Arc<HashSet<String>>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let state_changing =
        !matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if (state_changing || request.uri().path() == "/mcp")
        && *request.method() != Method::OPTIONS
        && let Some(origin) = request.headers().get(header::ORIGIN)
        && let Ok(origin) = origin.to_str()
        && !allowed.contains(origin)
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": format!("Origin '{origin}' is not allowed") })),
        )
            .into_response();
    }
    next.run(request).await
}

pub fn build_router(service: StudioService, static_dir: Option<PathBuf>) -> Router {
    build_router_with_origins(service, static_dir, &[])
}

pub fn build_router_with_origins(
    service: StudioService,
    static_dir: Option<PathBuf>,
    extra_origins: &[String],
) -> Router {
    let allowed: Arc<HashSet<String>> = Arc::new(
        default_allowed_origins()
            .into_iter()
            .chain(extra_origins.iter().map(|origin| origin.trim().to_string()))
            .filter(|origin| !origin.is_empty())
            .collect(),
    );
    let cors_origins: Vec<axum::http::HeaderValue> = allowed
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(cors_origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health))
        .route("/api/v3/authoring-options", get(get_authoring_options))
        .route(
            "/api/v3/context",
            get(get_interaction_context).post(post_interaction_context),
        )
        .route(
            "/api/v3/project",
            get(get_project_meta).patch(patch_project_meta),
        )
        .route("/api/v3/project-bundle", get(export_bundle))
        .route("/api/v3/project-bundle/validate", post(validate_bundle))
        .route("/api/v3/project-bundle/import", post(import_bundle))
        .route("/api/v3/library", get(query_library))
        .route(
            "/api/v3/content/{entity}",
            get(list_content).post(save_content),
        )
        .route(
            "/api/v3/content/{entity}/{id}",
            get(get_content).put(update_content).delete(delete_content),
        )
        .route("/api/v3/content/{entity}/{id}/move", post(move_content))
        .route(
            "/api/v3/content/{entity}/{id}/references",
            get(get_content_references),
        )
        .route(
            "/api/v3/files",
            post(upload_file).layer(DefaultBodyLimit::max(crate::files::MAX_UPLOAD_BYTES)),
        )
        .route("/api/v3/files/{*path}", get(get_project_file))
        .route("/api/v3/storylets/{id}/approve", post(approve_storylet))
        .route("/api/v3/scenarios", get(list_scenarios).post(save_scenario))
        .route(
            "/api/v3/scenarios/{id}",
            get(get_scenario)
                .put(save_scenario_with_id)
                .delete(delete_scenario),
        )
        .route("/api/v3/scenarios/preview", post(preview_scenario))
        .route("/api/v3/audio/inspect", get(inspect_audio_file))
        .route("/api/v4/run-content", get(crate::runs::run_content))
        .route("/api/v4/coverage", post(crate::runs::coverage))
        .route("/api/v4/runs", get(crate::runs::list).post(crate::runs::start))
        .route("/api/v4/runs/{id}", get(crate::runs::view).delete(crate::runs::delete))
        .route("/api/v4/runs/{id}/act", post(crate::runs::act))
        .route("/api/v4/runs/{id}/rewind", post(crate::runs::rewind))
        .route("/api/v4/runs/{id}/why-not", get(crate::runs::why_not))
        .route("/api/v4/runs/{id}/inspect", get(crate::runs::inspect))
        .route("/api/v4/runs/{id}/reload", post(crate::runs::reload))
        .route("/mcp", post(handle_http_mcp))
        .layer(cors)
        .layer(middleware::from_fn_with_state(
            allowed,
            reject_disallowed_origins,
        ))
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
    let content = std::fs::read_to_string("dist/studio/index.html").unwrap_or_default();
    let mut resp = Response::new(Body::from(content));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/html; charset=utf-8".parse().unwrap(),
    );
    resp
}

async fn health() -> Response {
    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "service": "storylet-studio-rust" })),
    )
        .into_response()
}
async fn get_authoring_options(State(service): State<StudioService>) -> Response {
    (StatusCode::OK, Json(service.get_authoring_options().await)).into_response()
}

async fn post_interaction_context(
    State(service): State<StudioService>,
    Json(body): Json<Value>,
) -> Response {
    match service.save_interaction_context(&body).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn get_interaction_context(State(service): State<StudioService>) -> Response {
    match service.get_interaction_context().await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn get_project_meta(State(service): State<StudioService>) -> Response {
    match service.get_project_metadata().await {
        Ok(m) => (StatusCode::OK, Json(json!(m))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
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
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Request body must be valid JSON" })),
            )
                .into_response();
        }
    };
    match service.update_project_name(&payload.name).await {
        Ok(m) => (StatusCode::OK, Json(json!(m))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn export_bundle(State(service): State<StudioService>) -> Response {
    match service.export_project_bundle().await {
        Ok(bundle) => {
            let filename = format!(
                "{}.storylet.json",
                bundle.project.name.to_lowercase().replace(' ', "-")
            );
            let body = serde_json::to_string_pretty(&bundle).unwrap_or_default();
            let mut resp = Response::new(Body::from(body));
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".parse().unwrap(),
            );
            resp.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\"")
                    .parse()
                    .unwrap(),
            );
            resp
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
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
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Request body must be a JSON object" })),
            )
                .into_response();
        }
    };
    match service.validate_bundle(&payload.bundle).await {
        Ok(issues) => (
            StatusCode::OK,
            Json(json!({ "valid": issues.is_empty(), "issues": issues })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "issues": [e] })),
        )
            .into_response(),
    }
}

async fn import_bundle(
    State(service): State<StudioService>,
    Json(payload): Json<BundlePayload>,
) -> Response {
    if payload.confirm_replace != Some(true) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "Explicit confirmReplace=true is required" })),
        )
            .into_response();
    }
    match service.import_bundle(&payload.bundle).await {
        Ok(revision) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Project replaced successfully",
                "revision": revision
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn query_library(
    State(service): State<StudioService>,
    Query(query): Query<LibraryQuery>,
) -> Response {
    match service.query_library(&query).await {
        Ok(p) => (StatusCode::OK, Json(json!(p))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

fn unknown_kind(entity: &str) -> Option<Response> {
    if crate::contracts::CONTENT_KINDS.contains(&entity) {
        None
    } else {
        Some(
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Unknown content kind '{entity}'") })),
            )
                .into_response(),
        )
    }
}

async fn list_content(
    State(service): State<StudioService>,
    Path(entity): Path<String>,
) -> Response {
    if let Some(response) = unknown_kind(&entity) {
        return response;
    }
    match service.list_content(&entity).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn get_content(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
) -> Response {
    if let Some(response) = unknown_kind(&entity) {
        return response;
    }
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
    if let Some(response) = unknown_kind(&entity) {
        return response;
    }
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
    if let Some(response) = unknown_kind(&entity) {
        return response;
    }
    match service.save_content_item(&entity, &payload).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn delete_content(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
) -> Response {
    if let Some(response) = unknown_kind(&entity) {
        return response;
    }
    match service.delete_content_item(&entity, &id).await {
        Ok(deleted) => (StatusCode::OK, Json(json!({ "deleted": deleted }))).into_response(),
        Err(ContentDeleteError::Conflict(error)) => {
            (StatusCode::CONFLICT, Json(json!({ "error": error }))).into_response()
        }
        Err(ContentDeleteError::Persistence(error)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct MoveBody {
    #[serde(default)]
    pub folder_id: Option<String>,
}

async fn move_content(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
    Json(body): Json<MoveBody>,
) -> Response {
    match service.move_content_item(&entity, &id, body.folder_id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(error) if error.contains("not found") => {
            (StatusCode::NOT_FOUND, Json(json!({ "error": error }))).into_response()
        }
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

async fn get_content_references(
    State(service): State<StudioService>,
    Path((entity, id)): Path<(String, String)>,
) -> Response {
    match service.content_references(&entity, &id).await {
        Ok(references) => {
            (StatusCode::OK, Json(json!({ "references": references }))).into_response()
        }
        Err(error) if error.contains("not found") => {
            (StatusCode::NOT_FOUND, Json(json!({ "error": error }))).into_response()
        }
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    pub name: String,
}

async fn upload_file(
    State(service): State<StudioService>,
    Query(query): Query<UploadQuery>,
    bytes: Bytes,
) -> Response {
    if bytes.len() > crate::files::MAX_UPLOAD_BYTES {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Upload exceeds the 10 MiB limit" })),
        )
            .into_response();
    }
    match crate::files::store_thumbnail(&service.resource_root, &query.name, &bytes) {
        Ok(path) => (StatusCode::CREATED, Json(json!({ "path": path }))).into_response(),
        Err(crate::files::FileError::Invalid(error)) => {
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
        Err(crate::files::FileError::NotFound) => {
            (StatusCode::NOT_FOUND, Json(json!({ "error": "File not found" }))).into_response()
        }
        Err(crate::files::FileError::Io(error)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn get_project_file(
    State(service): State<StudioService>,
    Path(path): Path<String>,
) -> Response {
    let resolved = match crate::files::resolve_project_file(&service.resource_root, &path) {
        Ok(resolved) => resolved,
        Err(crate::files::FileError::Invalid(error)) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response();
        }
        Err(crate::files::FileError::NotFound) => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "File not found" })))
                .into_response();
        }
        Err(crate::files::FileError::Io(error)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error })),
            )
                .into_response();
        }
    };
    let bytes = match tokio::fs::read(&resolved).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "File not found" })))
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };
    let content_type = if path.to_ascii_lowercase().ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        content_type.parse().unwrap(),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    response
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



async fn list_scenarios(State(s): State<StudioService>) -> Response {
    list_content(State(s), Path("scenarios".into())).await
}
async fn get_scenario(State(s): State<StudioService>, Path(id): Path<String>) -> Response {
    get_content(State(s), Path(("scenarios".into(), id))).await
}
async fn save_scenario(State(s): State<StudioService>, Json(p): Json<Value>) -> Response {
    save_content(State(s), Path("scenarios".into()), Json(p)).await
}
async fn save_scenario_with_id(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Json(mut p): Json<Value>,
) -> Response {
    if let Some(obj) = p.as_object_mut() {
        obj.insert("id".into(), Value::String(id));
    }
    save_content(State(s), Path("scenarios".into()), Json(p)).await
}
async fn delete_scenario(State(s): State<StudioService>, Path(id): Path<String>) -> Response {
    delete_content(State(s), Path(("scenarios".into(), id))).await
}

async fn preview_scenario(
    State(service): State<StudioService>,
    Json(payload): Json<Value>,
) -> Response {
    match service.preview_scenario(&payload).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct AudioInspectQuery {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct AudioInspectResponse {
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

fn inspect_wav_duration(path: &std::path::Path) -> Option<f64> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let mut header = [0_u8; 12];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let riff_end = 8_u64.checked_add(u32::from_le_bytes(header[4..8].try_into().ok()?) as u64)?;
    if riff_end < 12 || riff_end > file_len {
        return None;
    }

    let mut byte_rate = None;
    let mut data_size = None;
    while file.stream_position().ok()? + 8 <= riff_end {
        let mut chunk = [0_u8; 8];
        file.read_exact(&mut chunk).ok()?;
        let chunk_size = u32::from_le_bytes(chunk[4..8].try_into().ok()?) as u64;
        let payload_start = file.stream_position().ok()?;
        let payload_end = payload_start.checked_add(chunk_size + chunk_size % 2)?;
        if payload_end > riff_end {
            return None;
        }

        if &chunk[0..4] == b"fmt " {
            if chunk_size < 16 {
                return None;
            }
            let mut format = [0_u8; 16];
            file.read_exact(&mut format).ok()?;
            byte_rate = Some(u32::from_le_bytes(format[8..12].try_into().ok()?) as u64);
        } else if &chunk[0..4] == b"data" {
            data_size = Some(chunk_size);
        }
        file.seek(SeekFrom::Start(payload_end)).ok()?;
        if byte_rate.is_some() && data_size.is_some() {
            break;
        }
    }

    let rate = byte_rate?;
    if rate == 0 {
        return None;
    }
    Some(((data_size? as f64 / rate as f64) * 10.0).round() / 10.0)
}

#[cfg(test)]
mod audio_inspect_tests {
    use super::inspect_wav_duration;
    use std::io::Write;

    #[test]
    fn reports_duration_from_wav_byte_rate_and_data_size() {
        let path =
            std::env::temp_dir().join(format!("storylet-audio-{}.wav", uuid::Uuid::new_v4()));
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&8036_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend(std::iter::repeat_n(0, 8000));
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(&wav).unwrap();
        }

        assert_eq!(inspect_wav_duration(&path), Some(1.0));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_non_wav_files() {
        let path =
            std::env::temp_dir().join(format!("storylet-audio-{}.mp3", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not a wave file").unwrap();
        assert_eq!(inspect_wav_duration(&path), None);
        std::fs::remove_file(path).unwrap();
    }
}

async fn inspect_audio_file(Query(q): Query<AudioInspectQuery>) -> impl IntoResponse {
    let clean_path = q.path.trim();
    let p = std::path::Path::new(clean_path);
    if p.exists() && p.is_file() {
        let size = std::fs::metadata(p).ok().map(|meta| meta.len());
        let duration = inspect_wav_duration(p);
        (
            StatusCode::OK,
            Json(AudioInspectResponse {
                path: clean_path.to_string(),
                exists: true,
                size,
                duration,
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(AudioInspectResponse {
                path: clean_path.to_string(),
                exists: false,
                size: None,
                duration: None,
            }),
        )
    }
}

async fn handle_http_mcp(State(s): State<StudioService>, Json(msg): Json<Value>) -> Response {
    if let Some(resp) = crate::mcp::handle_mcp_message(&s, &msg).await {
        (StatusCode::OK, Json(resp)).into_response()
    } else {
        (StatusCode::NO_CONTENT, Json(Value::Null)).into_response()
    }
}
