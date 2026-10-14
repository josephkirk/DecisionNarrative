//! Loopback HTTP service and durable session storage for the storylet runtime.

use std::{convert::Infallible, error::Error, fmt};
use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path as RoutePath, Query, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use storylet_engine::{
    EngineError, ProjectBundle, RuntimeError, RuntimeSessionState, SemanticRankingRequest,
    StoryletRuntime, WorldCommand, WorldEvent,
};

#[derive(Clone)]
pub struct RuntimeService {
    state: Arc<Mutex<ServiceState>>,
    updates: broadcast::Sender<SessionChange>,
}

#[derive(Clone)]
struct SessionChange {
    session_id: String,
    revision: u64,
    snapshot: Value,
}

struct ServiceState {
    runtime: StoryletRuntime,
    connection: Connection,
}

impl RuntimeService {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ServiceError> {
        let connection = Connection::open(path).map_err(ServiceError::database)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, ServiceError> {
        let connection = Connection::open_in_memory().map_err(ServiceError::database)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, ServiceError> {
        initialize_schema(&connection)?;
        let mut runtime = StoryletRuntime::default();
        let mut statement = connection
            .prepare("SELECT state_json FROM runtime_sessions ORDER BY session_id")
            .map_err(ServiceError::database)?;
        let states = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(ServiceError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ServiceError::database)?;
        drop(statement);
        for encoded in states {
            let state: RuntimeSessionState = serde_json::from_str(&encoded).map_err(|error| {
                ServiceError::startup(format!("saved session JSON is invalid: {error}"))
            })?;
            validate_event_history(&connection, &state)?;
            runtime.restore_session(state).map_err(|error| {
                ServiceError::startup(format!("saved session cannot be restored: {error}"))
            })?;
        }
        let (updates, _) = broadcast::channel(256);
        let service = Self {
            state: Arc::new(Mutex::new(ServiceState {
                runtime,
                connection,
            })),
            updates,
        };
        Ok(service)
    }

    fn notify_session(&self, runtime: &StoryletRuntime, session_id: &str) {
        if let Ok(snapshot) = runtime.snapshot(session_id) {
            let _ = self.updates.send(SessionChange {
                session_id: session_id.to_owned(),
                revision: snapshot.world.revision,
                snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
            });
        }
    }
}

#[derive(Debug)]
pub struct ServiceError {
    status: StatusCode,
    message: String,
}

impl ServiceError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn database(error: rusqlite::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }

    fn startup(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn runtime(error: RuntimeError) -> Self {
        let status = match error {
            RuntimeError::SessionNotFound { .. } => StatusCode::NOT_FOUND,
            RuntimeError::SessionAlreadyExists { .. } => StatusCode::CONFLICT,
            RuntimeError::Engine(EngineError::RevisionConflict { .. })
            | RuntimeError::Engine(EngineError::IdempotencyKeyReuse) => StatusCode::CONFLICT,
            RuntimeError::InvalidProject(_)
            | RuntimeError::UnknownLocation { .. }
            | RuntimeError::UnknownQuality { .. }
            | RuntimeError::Engine(_) => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeError::EmptySessionId | RuntimeError::InvalidSessionState(_) => {
                StatusCode::BAD_REQUEST
            }
        };
        Self::new(status, error.to_string())
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ServiceError {}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), ServiceError> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS runtime_sessions (
             session_id TEXT PRIMARY KEY,
             project_id TEXT NOT NULL,
             revision TEXT NOT NULL,
             turn TEXT NOT NULL,
             state_json TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS runtime_events (
             session_id TEXT NOT NULL,
             event_id TEXT NOT NULL,
             revision TEXT NOT NULL,
             event_json TEXT NOT NULL,
             PRIMARY KEY (session_id, event_id)
         );",
        )
        .map_err(ServiceError::database)
}

fn validate_event_history(
    connection: &Connection,
    saved: &RuntimeSessionState,
) -> Result<(), ServiceError> {
    let mut statement = connection
        .prepare("SELECT event_json FROM runtime_events WHERE session_id = ?1 ORDER BY rowid")
        .map_err(ServiceError::database)?;
    let rows = statement
        .query_map([&saved.session_id], |row| row.get::<_, String>(0))
        .map_err(ServiceError::database)?;
    let mut events = Vec::new();
    for row in rows {
        let encoded = row.map_err(ServiceError::database)?;
        events.push(
            serde_json::from_str::<WorldEvent>(&encoded).map_err(|error| {
                ServiceError::startup(format!("saved event JSON is invalid: {error}"))
            })?,
        );
    }
    if events != saved.world.events {
        return Err(ServiceError::startup(format!(
            "saved session '{}' event journal does not match its snapshot",
            saved.session_id
        )));
    }
    Ok(())
}

fn ensure_session_id_has_no_archive(
    connection: &Connection,
    session_id: &str,
) -> Result<(), ServiceError> {
    let archived: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM runtime_events WHERE session_id = ?1 LIMIT 1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ServiceError::database)?;
    if archived.is_some() {
        return Err(ServiceError::new(
            StatusCode::CONFLICT,
            format!("session '{session_id}' has archived event history and cannot be reused"),
        ));
    }
    Ok(())
}

fn persist_session(state: &mut ServiceState, session_id: &str) -> Result<(), ServiceError> {
    let saved = state
        .runtime
        .export_session(session_id)
        .map_err(ServiceError::runtime)?;
    let state_json =
        serde_json::to_string(&saved).map_err(|error| ServiceError::startup(error.to_string()))?;
    let tx = state
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(ServiceError::database)?;
    let previous_revision: Option<String> = tx
        .query_row(
            "SELECT revision FROM runtime_sessions WHERE session_id = ?1",
            [&saved.session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ServiceError::database)?;
    let previous_revision = previous_revision
        .map(|revision| {
            revision.parse::<u64>().map_err(|error| {
                ServiceError::startup(format!("saved session revision is invalid: {error}"))
            })
        })
        .transpose()?
        .unwrap_or(0);
    tx.execute(
        "INSERT INTO runtime_sessions (session_id, project_id, revision, turn, state_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(session_id) DO UPDATE SET project_id=excluded.project_id,
             revision=excluded.revision, turn=excluded.turn, state_json=excluded.state_json,
             updated_at=excluded.updated_at",
        params![saved.session_id, saved.project_id, saved.world.revision.to_string(), saved.world.turn.to_string(), state_json, now_millis()],
    ).map_err(ServiceError::database)?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO runtime_events (session_id, event_id, revision, event_json) VALUES (?1, ?2, ?3, ?4)",
        ).map_err(ServiceError::database)?;
        for event in &saved.world.events {
            if event.revision <= previous_revision {
                continue;
            }
            let event_json = serde_json::to_string(event)
                .map_err(|error| ServiceError::startup(error.to_string()))?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT event_json FROM runtime_events WHERE session_id = ?1 AND event_id = ?2",
                    params![saved.session_id, event.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(ServiceError::database)?;
            match existing {
                Some(existing) if existing != event_json => {
                    return Err(ServiceError::new(
                        StatusCode::CONFLICT,
                        format!(
                            "event '{}' conflicts with archived session history",
                            event.id
                        ),
                    ));
                }
                Some(_) => continue,
                None => {}
            }
            insert
                .execute(params![
                    saved.session_id,
                    event.id,
                    event.revision.to_string(),
                    event_json
                ])
                .map_err(ServiceError::database)?;
        }
    }
    let persisted_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM runtime_events WHERE session_id = ?1",
            [&saved.session_id],
            |row| row.get(0),
        )
        .map_err(ServiceError::database)?;
    if persisted_count != saved.world.events.len() as i64 {
        return Err(ServiceError::new(
            StatusCode::CONFLICT,
            format!(
                "session '{}' event history conflicts with its archive",
                saved.session_id
            ),
        ));
    }
    tx.commit().map_err(ServiceError::database)
}

fn restore_prior(state: &mut ServiceState, prior: RuntimeSessionState) {
    state.runtime.delete_session(&prior.session_id);
    // This state came from a successful export of the same runtime and has already passed validation.
    state
        .runtime
        .restore_session(prior)
        .expect("previous runtime state must be restorable");
}

fn now_millis() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlaytest {
    #[serde(default)]
    session_id: Option<String>,
    project: ProjectBundle,
    scenario_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkPlaytest {
    branch_session_id: Option<String>,
    event_id: String,
}

#[derive(Deserialize)]
struct RestorePlaytest {
    state: RuntimeSessionState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectorQuery {
    #[serde(default = "default_tension_slope")]
    target_tension_slope: f64,
}

fn default_tension_slope() -> f64 {
    0.15
}

pub fn router(service: RuntimeService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/playtests", get(list_sessions).post(create_session))
        .route("/api/v1/playtests/restore", post(restore_session))
        .route(
            "/api/v1/playtests/{session_id}",
            get(snapshot).delete(delete_session),
        )
        .route("/api/v1/playtests/{session_id}/events", get(events))
        .route("/api/v1/playtests/{session_id}/stream", get(session_stream))
        .route("/api/v1/playtests/{session_id}/director", get(evaluate))
        .route(
            "/api/v1/playtests/{session_id}/director/decision",
            post(apply_decision),
        )
        .route(
            "/api/v1/playtests/{session_id}/director/semantic-rank",
            post(apply_semantic_rank),
        )
        .route("/api/v1/playtests/{session_id}/commands", post(command))
        .route("/api/v1/playtests/{session_id}/choices", post(choice))
        .route("/api/v1/playtests/{session_id}/fork", post(fork_session))
        .route("/api/v1/playtests/{session_id}/export", get(export_session))
        .route(
            "/api/v1/playtests/{session_id}/restore",
            post(restore_session),
        )
        .with_state(service)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn list_sessions(State(service): State<RuntimeService>) -> Result<Json<Value>, ServiceError> {
    let state = service.state.lock().await;
    let mut statement = state.connection.prepare(
        "SELECT session_id, project_id, revision, turn, state_json FROM runtime_sessions ORDER BY session_id",
    ).map_err(ServiceError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(ServiceError::database)?;
    let mut sessions = Vec::new();
    for row in rows {
        let (session_id, project_id, revision, turn, encoded) =
            row.map_err(ServiceError::database)?;
        let saved: RuntimeSessionState = serde_json::from_str(&encoded)
            .map_err(|error| ServiceError::startup(error.to_string()))?;
        sessions.push(json!({
            "session_id": session_id, "project_id": project_id, "scenario_id": saved.scenario_id,
            "revision": revision.parse::<u64>().unwrap_or_default(), "turn": turn.parse::<u64>().unwrap_or_default(),
            "current_location": saved.world.current_location, "event_count": saved.world.events.len()
        }));
    }
    Ok(Json(Value::Array(sessions)))
}

async fn create_session(
    State(service): State<RuntimeService>,
    Json(body): Json<CreatePlaytest>,
) -> Result<(StatusCode, Json<Value>), ServiceError> {
    let session_id = body
        .session_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| uuid());
    let mut state = service.state.lock().await;
    ensure_session_id_has_no_archive(&state.connection, &session_id)?;
    let snapshot = state
        .runtime
        .create_session(session_id.clone(), body.project, &body.scenario_id)
        .map_err(ServiceError::runtime)?;
    if let Err(error) = persist_session(&mut state, &session_id) {
        state.runtime.delete_session(&session_id);
        return Err(error);
    }
    service.notify_session(&state.runtime, &session_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "sessionId": session_id, "snapshot": snapshot })),
    ))
}

async fn session_stream(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ServiceError> {
    let receiver = service.updates.subscribe();
    let initial = {
        let state = service.state.lock().await;
        state
            .runtime
            .snapshot(&session_id)
            .map(|snapshot| SessionChange {
                session_id: session_id.clone(),
                revision: snapshot.world.revision,
                snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
            })
            .map_err(ServiceError::runtime)?
    };
    let initial = tokio_stream::iter([Ok(session_change_event("playtest.snapshot", initial))]);
    let updates = BroadcastStream::new(receiver).filter_map(move |item| match item {
        Ok(change) if change.session_id == session_id => {
            Some(Ok(session_change_event("playtest.updated", change)))
        }
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
            Some(Ok(Event::default().event("playtest.resync").data("{}")))
        }
        _ => None,
    });
    Ok(Sse::new(initial.chain(updates)).keep_alive(KeepAlive::default()))
}

fn session_change_event(name: &str, change: SessionChange) -> Event {
    Event::default()
        .event(name)
        .id(change.revision.to_string())
        .data(change.snapshot.to_string())
}

async fn snapshot(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
) -> Result<Json<Value>, ServiceError> {
    let state = service.state.lock().await;
    state
        .runtime
        .snapshot(&session_id)
        .map(|result| Json(json!(result)))
        .map_err(ServiceError::runtime)
}

async fn events(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
) -> Result<Json<Value>, ServiceError> {
    let state = service.state.lock().await;
    let snapshot = state
        .runtime
        .snapshot(&session_id)
        .map_err(ServiceError::runtime)?;
    Ok(Json(
        json!({ "sessionId": session_id, "turn": snapshot.world.turn, "events": snapshot.world.events }),
    ))
}

async fn evaluate(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Query(query): Query<DirectorQuery>,
) -> Result<Json<Value>, ServiceError> {
    if !query.target_tension_slope.is_finite() {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "targetTensionSlope must be finite",
        ));
    }
    let state = service.state.lock().await;
    state
        .runtime
        .evaluate(&session_id, query.target_tension_slope)
        .map(|result| Json(json!(result)))
        .map_err(ServiceError::runtime)
}

async fn apply_decision(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ServiceError> {
    let decision = serde_json::from_value(body.get("decision").cloned().unwrap_or(body))
        .map_err(|error| ServiceError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let state = service.state.lock().await;
    state
        .runtime
        .apply_decision(&session_id, 0.15, decision)
        .map(|result| Json(json!(result)))
        .map_err(ServiceError::runtime)
}

async fn apply_semantic_rank(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ServiceError> {
    let request = serde_json::from_value::<SemanticRankingRequest>(
        body.get("request").cloned().unwrap_or(body),
    )
    .map_err(|error| ServiceError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let state = service.state.lock().await;
    state
        .runtime
        .rank_semantic(&session_id, 0.15, request)
        .map(|result| Json(json!(result)))
        .map_err(ServiceError::runtime)
}

async fn command(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ServiceError> {
    let command: WorldCommand =
        serde_json::from_value(body.get("command").cloned().unwrap_or(body))
            .map_err(|error| ServiceError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let mut state = service.state.lock().await;
    let prior = state
        .runtime
        .export_session(&session_id)
        .map_err(ServiceError::runtime)?;
    let result = state
        .runtime
        .submit_command(&session_id, command)
        .map_err(ServiceError::runtime)?;
    if let Err(error) = persist_session(&mut state, &session_id) {
        restore_prior(&mut state, prior);
        return Err(error);
    }
    if result.world.revision > prior.world.revision {
        service.notify_session(&state.runtime, &session_id);
    }
    Ok(Json(json!(result)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChoiceRequest {
    storylet_id: String,
    choice_id: String,
    idempotency_key: Option<String>,
    expected_revision: u64,
}

async fn choice(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Json(body): Json<ChoiceRequest>,
) -> Result<Json<Value>, ServiceError> {
    let mut state = service.state.lock().await;
    let prior = state
        .runtime
        .export_session(&session_id)
        .map_err(ServiceError::runtime)?;
    let result = state
        .runtime
        .execute_choice(
            &session_id,
            &body.storylet_id,
            &body.choice_id,
            body.idempotency_key.as_deref().unwrap_or(""),
            body.expected_revision,
        )
        .map_err(ServiceError::runtime)?;
    if let Err(error) = persist_session(&mut state, &session_id) {
        restore_prior(&mut state, prior);
        return Err(error);
    }
    if result.world.revision > prior.world.revision {
        service.notify_session(&state.runtime, &session_id);
    }
    Ok(Json(json!(result)))
}

async fn fork_session(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
    Json(body): Json<ForkPlaytest>,
) -> Result<(StatusCode, Json<Value>), ServiceError> {
    let branch_id = body
        .branch_session_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(uuid);
    let mut state = service.state.lock().await;
    ensure_session_id_has_no_archive(&state.connection, &branch_id)?;
    let snapshot = state
        .runtime
        .fork_session(&session_id, branch_id.clone(), &body.event_id)
        .map_err(ServiceError::runtime)?;
    if let Err(error) = persist_session(&mut state, &branch_id) {
        state.runtime.delete_session(&branch_id);
        return Err(error);
    }
    service.notify_session(&state.runtime, &branch_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "sessionId": branch_id, "snapshot": snapshot })),
    ))
}

async fn delete_session(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
) -> Result<Json<Value>, ServiceError> {
    let mut state = service.state.lock().await;
    state
        .runtime
        .snapshot(&session_id)
        .map_err(ServiceError::runtime)?;
    let tx = state
        .connection
        .transaction()
        .map_err(ServiceError::database)?;
    tx.execute(
        "DELETE FROM runtime_sessions WHERE session_id = ?1",
        [&session_id],
    )
    .map_err(ServiceError::database)?;
    tx.commit().map_err(ServiceError::database)?;
    Ok(Json(
        json!({ "deleted": state.runtime.delete_session(&session_id) }),
    ))
}

async fn export_session(
    State(service): State<RuntimeService>,
    RoutePath(session_id): RoutePath<String>,
) -> Result<Json<Value>, ServiceError> {
    let state = service.state.lock().await;
    state
        .runtime
        .export_session(&session_id)
        .map(|result| Json(json!(result)))
        .map_err(ServiceError::runtime)
}

async fn restore_session(
    State(service): State<RuntimeService>,
    Json(body): Json<RestorePlaytest>,
) -> Result<(StatusCode, Json<Value>), ServiceError> {
    let mut state = service.state.lock().await;
    let session_id = body.state.session_id.clone();
    let snapshot = state
        .runtime
        .restore_session(body.state)
        .map_err(ServiceError::runtime)?;
    if let Err(error) = persist_session(&mut state, &session_id) {
        state.runtime.delete_session(&session_id);
        return Err(error);
    }
    service.notify_session(&state.runtime, &session_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({ "sessionId": session_id, "snapshot": snapshot })),
    ))
}

fn uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    format!(
        "session-{}-{}",
        now_millis(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}
