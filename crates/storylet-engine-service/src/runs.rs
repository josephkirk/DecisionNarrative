use crate::{
    run_content::{compile_run_content, compile_run_content_in_transaction},
    service::{StudioInner, StudioService, chrono_now_ms},
    store::Store,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use storylet_engine::{
    RewindRequest as KernelRewindRequest, RunOperation,
    actions::{ActionProposal, Authority, DecisionProvenance},
    chrono_participants::Controller,
    run::{RunAction, RunContent, RunError, RunKernel, RunStart},
};

pub(crate) struct CachedRun {
    content: RunContent,
    kernel: RunKernel,
    stalled: Option<RunError>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartConfig {
    start: RunStart,
    scenario_id: Option<String>,
    #[serde(default)]
    storylet_id: Option<String>,
    #[serde(default)]
    participant_config: Option<storylet_engine::chrono_explore::ParticipantConfig>,
    content: RunContent,
}
#[derive(Clone)]
struct Row {
    id: String,
    start_key: String,
    start_request: String,
    project_revision: u64,
    seed: u64,
    start: StartConfig,
    actions: Vec<RunOperation>,
    created_at: i64,
    updated_at: i64,
}
#[derive(Debug)]
pub struct RunApiError {
    status: StatusCode,
    message: String,
}
impl RunApiError {
    fn invalid(s: impl ToString) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: s.to_string(),
        }
    }
    fn internal(s: impl ToString) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: s.to_string(),
        }
    }
    fn missing() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: "run not found".into(),
        }
    }
    fn conflict(s: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: s.into(),
        }
    }
}
impl std::fmt::Display for RunApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl IntoResponse for RunApiError {
    fn into_response(self) -> Response {
        let body = if self.status == StatusCode::UNPROCESSABLE_ENTITY {
            json!({"error":self.message,"issues":[]})
        } else {
            json!({"error":self.message})
        };
        (self.status, Json(body)).into_response()
    }
}
fn kernel_error(e: RunError) -> RunApiError {
    RunApiError::invalid(format!("{e:?}"))
}
fn encode<T: Serialize + ?Sized>(v: &T) -> Result<String, RunApiError> {
    serde_json::to_string(v).map_err(RunApiError::internal)
}
fn load(store: &Store, id: &str) -> Result<Row, RunApiError> {
    type RawRow = (String, String, String, i64, i64, String, String, i64, i64);
    let raw: Option<RawRow> = store.conn.query_row(
        "SELECT id, start_key, start_request, project_revision, seed, start_json,          actions_json, created_at, updated_at FROM runs WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
    ).optional().map_err(RunApiError::internal)?;
    let (
        id,
        start_key,
        start_request,
        project_revision,
        seed,
        start,
        actions,
        created_at,
        updated_at,
    ) = raw.ok_or_else(RunApiError::missing)?;
    Ok(Row {
        id,
        start_key,
        start_request,
        project_revision: project_revision as u64,
        seed: seed as u64,
        start: serde_json::from_str(&start).map_err(RunApiError::internal)?,
        actions: serde_json::from_str(&actions).map_err(RunApiError::internal)?,
        created_at,
        updated_at,
    })
}
fn save(store: &Store, row: &Row) -> Result<(), RunApiError> {
    store.conn.execute(
        "INSERT INTO runs(id, start_key, start_request, project_revision, seed, start_json,          actions_json, created_at, updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)          ON CONFLICT(id) DO UPDATE SET project_revision=excluded.project_revision,          start_json=excluded.start_json, actions_json=excluded.actions_json, updated_at=excluded.updated_at",
        params![row.id, row.start_key, row.start_request, row.project_revision as i64,
            row.seed as i64, encode(&row.start)?, encode(&row.actions)?, row.created_at, row.updated_at],
    ).map_err(RunApiError::internal)?;
    Ok(())
}
fn current_revision(store: &Store) -> Result<u64, RunApiError> {
    Ok(store
        .get_project_metadata()
        .map_err(RunApiError::internal)?
        .revision)
}
fn human_authority(kernel: &RunKernel, actor: u16) -> Result<Authority, RunApiError> {
    match kernel.world().participants.controller.get(actor as usize) {
        Some(Controller::Human { seat }) => Ok(Authority::Human { seat: *seat }),
        Some(Controller::Rule) => Err(RunApiError::invalid(
            "rule participants cannot be controlled by callers",
        )),
        None => Err(RunApiError::invalid("unknown participant")),
    }
}
fn proposal(
    actor: u16,
    action: RunAction,
    expected_revision: u64,
    key: String,
    provenance: Option<DecisionProvenance>,
) -> ActionProposal {
    ActionProposal {
        schema_version: 1,
        actor,
        action,
        expected_revision,
        idempotency_key: key,
        provenance,
    }
}
fn rejection(e: storylet_engine::actions::ActionRejection) -> RunApiError {
    if e.reason.contains("stale revision") || e.reason.contains("idempotency key reused") {
        RunApiError::conflict(&e.reason)
    } else {
        RunApiError::invalid(e.reason)
    }
}
fn rebuild(row: &Row) -> Result<CachedRun, RunApiError> {
    let content = row.start.content.clone();
    let (mut kernel, mut stalled) =
        RunKernel::start_preserving_stall(&content, row.start.start.clone(), row.seed)
            .map_err(kernel_error)?;
    for op in &row.actions {
        kernel
            .apply_operation(&content, op.clone())
            .map_err(rejection)?;
        stalled = kernel.stalled();
    }
    Ok(CachedRun {
        kernel,
        content,
        stalled,
    })
}
fn matches_row(c: &CachedRun, row: &Row) -> bool {
    c.content.content_revision == row.project_revision && c.kernel.operations() == row.actions
}
fn cached<'a>(
    runs: &'a mut BTreeMap<String, CachedRun>,
    row: &Row,
) -> Result<&'a CachedRun, RunApiError> {
    if !runs.get(&row.id).is_some_and(|c| matches_row(c, row)) {
        runs.remove(&row.id);
        runs.insert(row.id.clone(), rebuild(row)?);
    }
    Ok(&runs[&row.id])
}
fn envelope(row: &Row, c: &CachedRun, participant: u16, current: u64) -> Value {
    let mut value = json!({"id":row.id,"revision":c.kernel.revision(),"stale":row.project_revision != current,"contentRevision":row.project_revision,"view":c.kernel.architect_view(&c.content,participant)});
    if let Some(RunError::Stalled {
        storylet,
        node_guid,
    }) = &c.stalled
    {
        value["stalled"] = json!({"storyletId":storylet,"nodeGuid":node_guid});
    }
    value
}
fn validate_participant(c: &CachedRun, participant: u16) -> Result<(), RunApiError> {
    if participant as usize >= c.kernel.world().participants.len() {
        Err(RunApiError::invalid("unknown participant"))
    } else {
        Ok(())
    }
}
fn require_key(key: &str) -> Result<(), RunApiError> {
    if key.trim().is_empty() {
        Err(RunApiError::invalid("idempotencyKey is required"))
    } else {
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    start: RunStart,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    storylet_id: Option<String>,
    scenario_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_seed")]
    seed: Option<u64>,
    #[serde(default)]
    actions: Vec<(u16, RunAction)>,
    #[serde(default)]
    participant_config: Option<storylet_engine::chrono_explore::ParticipantConfig>,
    expected_revision: u64,
    idempotency_key: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActRequest {
    participant: u16,
    action: RunAction,
    expected_revision: u64,
    idempotency_key: String,
    #[serde(default)]
    provenance: Option<DecisionProvenance>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionRequest {
    expected_revision: u64,
}
#[derive(Default, Deserialize)]
pub struct ParticipantQuery {
    participant: Option<u16>,
}
impl StudioService {
    pub async fn run_start(&self, req: StartRequest) -> Result<Value, RunApiError> {
        require_key(&req.idempotency_key)?;
        let request = encode(&req)?;
        let mut inner = self.inner.lock().await;
        let StudioInner { store, runs } = &mut *inner;
        let tx = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(RunApiError::internal)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,start_request FROM runs WHERE start_key=?1",
                [&req.idempotency_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(RunApiError::internal)?;
        if let Some((id, stored)) = existing {
            if stored != request {
                return Err(RunApiError::conflict(
                    "idempotency key reused with different request",
                ));
            }
            let row = load(store, &id)?;
            let c = match runs.remove(&id) {
                Some(c) if matches_row(&c, &row) => c,
                _ => rebuild(&row)?,
            };
            let result = envelope(&row, &c, 0, current_revision(store)?);
            tx.commit().map_err(RunApiError::internal)?;
            runs.insert(id, c);
            return Ok(result);
        }
        let (content, default_seed) = compile_run_content_in_transaction(
            store,
            req.scenario_id.as_deref(),
            Some(req.expected_revision),
            req.storylet_id.as_deref(),
        )
        .map_err(|e| {
            if e == "stale revision" {
                RunApiError::conflict("stale revision")
            } else {
                RunApiError::invalid(e)
            }
        })?;
        let seed = req.seed.unwrap_or(default_seed);
        let content = match req.participant_config {
            Some(config) => storylet_engine::chrono_explore::configured_content(&content, config),
            None => content,
        };
        let (mut kernel, mut stalled) =
            RunKernel::start_preserving_stall(&content, req.start.clone(), seed)
                .map_err(kernel_error)?;
        let mut recorded = Vec::with_capacity(req.actions.len());
        for (participant, action) in &req.actions {
            let authority = human_authority(&kernel, *participant)?;
            let p = proposal(
                *participant,
                action.clone(),
                kernel.revision(),
                format!("start-{}", recorded.len()),
                None,
            );
            kernel
                .propose(&content, authority, p.clone())
                .map_err(rejection)?;
            stalled = kernel.stalled();
            recorded.push(RunOperation::Action {
                authority,
                proposal: p,
            });
        }
        let now = chrono_now_ms();
        let row = Row {
            id: uuid::Uuid::new_v4().to_string(),
            start_key: req.idempotency_key,
            start_request: request,
            project_revision: content.content_revision,
            seed,
            start: StartConfig {
                start: req.start,
                scenario_id: req.scenario_id,
                storylet_id: req.storylet_id,
                participant_config: req.participant_config,
                content: content.clone(),
            },
            actions: recorded,
            created_at: now,
            updated_at: now,
        };
        let c = CachedRun {
            content,
            kernel,
            stalled,
        };
        let result = envelope(&row, &c, 0, current_revision(store)?);
        save(store, &row)?;
        tx.commit().map_err(RunApiError::internal)?;
        runs.insert(row.id.clone(), c);
        Ok(result)
    }
    pub async fn run_list(&self) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let ids = {
            let mut stmt = inner
                .store
                .conn
                .prepare("SELECT id FROM runs ORDER BY updated_at DESC,id")
                .map_err(RunApiError::internal)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(RunApiError::internal)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(RunApiError::internal)?
        };
        let rev = current_revision(&inner.store)?;
        let mut out = Vec::new();
        for id in ids {
            let row = load(&inner.store, &id)?;
            let c = cached(&mut inner.runs, &row)?;
            let v = c.kernel.architect_view(&c.content, 0);
            out.push(json!({"id":id,"title":v.scene.as_ref().map(|s|s.storylet.title.clone()).unwrap_or_else(||"Run".into()),"steps":v.current_step,"ending":v.ending,"stale":row.project_revision!=rev,"updatedAt":row.updated_at}));
        }
        Ok(json!(out))
    }
    pub async fn run_view(&self, id: &str, participant: u16) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let row = load(&inner.store, id)?;
        let rev = current_revision(&inner.store)?;
        let c = cached(&mut inner.runs, &row)?;
        validate_participant(c, participant)?;
        Ok(envelope(&row, c, participant, rev))
    }
    pub async fn run_observe(&self, id: &str, participant: u16) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let row = load(&inner.store, id)?;
        let c = cached(&mut inner.runs, &row)?;
        validate_participant(c, participant)?;
        Ok(json!(c.kernel.observe(&c.content, participant)))
    }
    pub async fn run_explain(&self, id: &str, event: u32) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let row = load(&inner.store, id)?;
        let c = cached(&mut inner.runs, &row)?;
        c.kernel
            .explain(event)
            .map(|events| json!(events))
            .ok_or_else(RunApiError::missing)
    }
    pub async fn run_act(&self, id: &str, req: ActRequest) -> Result<Value, RunApiError> {
        require_key(&req.idempotency_key)?;
        let participant = req.participant;
        self.run_operation(id, participant, move |kernel| {
            Ok(RunOperation::Action {
                authority: human_authority(kernel, participant)?,
                proposal: proposal(
                    participant,
                    req.action,
                    req.expected_revision,
                    req.idempotency_key,
                    req.provenance,
                ),
            })
        })
        .await
    }
    pub async fn run_rewind(
        &self,
        id: &str,
        req: KernelRewindRequest,
    ) -> Result<Value, RunApiError> {
        require_key(&req.idempotency_key)?;
        self.run_operation(id, 0, move |_| {
            Ok(RunOperation::Rewind {
                authority: Authority::Architect,
                request: req,
            })
        })
        .await
    }
    async fn run_operation(
        &self,
        id: &str,
        participant: u16,
        make_op: impl FnOnce(&RunKernel) -> Result<RunOperation, RunApiError>,
    ) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let StudioInner { store, runs } = &mut *inner;
        let prior = runs.remove(id);
        let tx = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(RunApiError::internal)?;
        let mut row = load(store, id)?;
        let current = current_revision(store)?;
        if row.project_revision != current {
            return Err(RunApiError::conflict("run is stale; reload required"));
        }
        let mut c = match prior {
            Some(c) if matches_row(&c, &row) => c,
            _ => rebuild(&row)?,
        };
        let op = make_op(&c.kernel)?;
        let receipt = c
            .kernel
            .apply_operation(&c.content, op.clone())
            .map_err(rejection)?;
        if !receipt.duplicate {
            row.actions.push(op);
            row.updated_at = chrono_now_ms();
            c.stalled = c.kernel.stalled();
            save(store, &row)?;
        }
        let mut result = envelope(&row, &c, participant, current);
        result["receipt"] = json!(receipt);
        tx.commit().map_err(RunApiError::internal)?;
        runs.insert(row.id, c);
        Ok(result)
    }
    pub async fn run_why_not(&self, id: &str, participant: u16) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let row = load(&inner.store, id)?;
        let c = cached(&mut inner.runs, &row)?;
        validate_participant(c, participant)?;
        Ok(json!({"nearMisses":c.kernel.why_not(&c.content,participant,10)}))
    }
    pub async fn run_inspect(&self, id: &str, participant: u16) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let row = load(&inner.store, id)?;
        let c = cached(&mut inner.runs, &row)?;
        validate_participant(c, participant)?;
        Ok(json!(c.kernel.inspect(&c.content, participant)))
    }
    pub async fn run_reload(&self, id: &str, req: RevisionRequest) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let StudioInner { store, runs } = &mut *inner;
        let prior = runs.remove(id);
        let tx = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(RunApiError::internal)?;
        let mut row = load(store, id)?;
        let c0 = match prior {
            Some(c) if matches_row(&c, &row) => c,
            _ => rebuild(&row)?,
        };
        let current = current_revision(store)?;
        if row.project_revision == current {
            let result = envelope(&row, &c0, 0, current);
            tx.commit().map_err(RunApiError::internal)?;
            runs.insert(row.id, c0);
            return Ok(result);
        }
        if c0.kernel.revision() != req.expected_revision {
            return Err(RunApiError::conflict("stale revision"));
        }
        let (content, _) = compile_run_content_in_transaction(
            store,
            row.start.scenario_id.as_deref(),
            None,
            row.start.storylet_id.as_deref(),
        )
        .map_err(RunApiError::invalid)?;
        let content = match row.start.participant_config {
            Some(config) => storylet_engine::chrono_explore::configured_content(&content, config),
            None => content,
        };

        if row.start.content.catalog.space() != content.catalog.space()
            || row.start.content.registry != content.registry
            || row.start.content.participant_indices != content.participant_indices
            || row.start.content.implicit_participant != content.implicit_participant
            || row
                .start
                .content
                .initial_world
                .vertices
                .iter()
                .map(|v| (&v.vertex_id, v.entity_index))
                .collect::<Vec<_>>()
                != content
                    .initial_world
                    .vertices
                    .iter()
                    .map(|v| (&v.vertex_id, v.entity_index))
                    .collect::<Vec<_>>()
        {
            return Err(RunApiError::conflict(
                "compiled identity changed; cannot safely reload",
            ));
        }
        let (mut kernel, mut stalled) =
            RunKernel::start_preserving_stall(&content, row.start.start.clone(), row.seed)
                .map_err(kernel_error)?;
        let mut replayed = 0usize;
        let mut kept = 0usize;
        let mut diverged = None;
        for op in &row.actions {
            let result = kernel.apply_operation(&content, op.clone());
            stalled = kernel.stalled();
            if result.is_err() {
                diverged = Some((replayed + 1) as u32);
                break;
            }
            if matches!(op, RunOperation::Action { .. }) {
                replayed += 1;
            }
            kept += 1;
        }
        row.actions.truncate(kept);
        row.project_revision = content.content_revision;
        row.start.content = content.clone();
        row.updated_at = chrono_now_ms();
        let c = CachedRun {
            content,
            kernel,
            stalled,
        };
        let mut result = envelope(&row, &c, 0, current_revision(store)?);
        result["replayed"] = json!(replayed);
        result["divergedAt"] = json!(diverged);
        save(store, &row)?;
        tx.commit().map_err(RunApiError::internal)?;
        runs.insert(row.id.clone(), c);
        Ok(result)
    }
    pub async fn run_delete(&self, id: &str, req: RevisionRequest) -> Result<Value, RunApiError> {
        let mut inner = self.inner.lock().await;
        let StudioInner { store, runs } = &mut *inner;
        let prior = runs.remove(id);
        let tx = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(RunApiError::internal)?;
        match load(store, id) {
            Ok(row) => {
                let c = match prior {
                    Some(c) if matches_row(&c, &row) => c,
                    _ => rebuild(&row)?,
                };
                if c.kernel.revision() != req.expected_revision {
                    return Err(RunApiError::conflict("stale revision"));
                }
                tx.execute("DELETE FROM runs WHERE id=?1", [id])
                    .map_err(RunApiError::internal)?;
            }
            Err(e) if e.status == StatusCode::NOT_FOUND => {}
            Err(e) => return Err(e),
        }
        tx.commit().map_err(RunApiError::internal)?;
        Ok(json!({"ok":true}))
    }
}
macro_rules! handler {
    ($name:ident,$method:ident,$arg:ty) => {
        pub async fn $name(
            State(s): State<StudioService>,
            Path(id): Path<String>,
            Json(req): Json<$arg>,
        ) -> Response {
            match s.$method(&id, req).await {
                Ok(v) => Json(v).into_response(),
                Err(e) => e.into_response(),
            }
        }
    };
}
pub async fn start(State(s): State<StudioService>, Json(req): Json<StartRequest>) -> Response {
    match s.run_start(req).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}
pub async fn list(State(s): State<StudioService>) -> Response {
    match s.run_list().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}
pub async fn view(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<ParticipantQuery>,
) -> Response {
    match s.run_view(&id, q.participant.unwrap_or(0)).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}
#[derive(Default, Deserialize)]
pub struct EventQuery {
    event: Option<u32>,
}
pub async fn observe(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<ParticipantQuery>,
) -> Response {
    match q.participant {
        Some(participant) => match s.run_observe(&id, participant).await {
            Ok(v) => Json(v).into_response(),
            Err(e) => e.into_response(),
        },
        None => RunApiError::invalid("participant is required").into_response(),
    }
}
pub async fn explain(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<EventQuery>,
) -> Response {
    match q.event {
        Some(event) => match s.run_explain(&id, event).await {
            Ok(v) => Json(v).into_response(),
            Err(e) => e.into_response(),
        },
        None => RunApiError::invalid("event is required").into_response(),
    }
}
pub async fn why_not(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<ParticipantQuery>,
) -> Response {
    match s.run_why_not(&id, q.participant.unwrap_or(0)).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}
pub async fn inspect(
    State(s): State<StudioService>,
    Path(id): Path<String>,
    Query(q): Query<ParticipantQuery>,
) -> Response {
    match s.run_inspect(&id, q.participant.unwrap_or(0)).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}
handler!(act, run_act, ActRequest);
handler!(rewind, run_rewind, KernelRewindRequest);
handler!(reload, run_reload, RevisionRequest);
handler!(delete, run_delete, RevisionRequest);

fn deserialize_optional_seed<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("seed must be an unsigned integer")),
        Some(Value::String(text)) => text
            .parse::<u64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(
            "seed must be an unsigned integer or decimal string",
        )),
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CoverageRequest {
    pub runs: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_optional_seed")]
    pub seed: Option<u64>,
}

struct CoveragePermit(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for CoveragePermit {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

fn compile_coverage_config(
    world: &crate::contracts::EntityRecord,
    content: &RunContent,
    req: &CoverageRequest,
) -> Result<storylet_engine::chrono_explore::ExploreConfig, RunApiError> {
    use storylet_engine::chrono_explore::{
        ExploreConfig, IntentProbe, Milestone, ParticipantConfig,
    };
    let mut cfg = ExploreConfig {
        runs: req.runs.unwrap_or(2000),
        seed: req.seed.unwrap_or(0),
        ..Default::default()
    };
    let text = |v: &std::collections::BTreeMap<String, Value>, key: &str| {
        v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
    };
    let strings = |v: &std::collections::BTreeMap<String, Value>,
                   key: &str|
     -> Result<Vec<String>, RunApiError> {
        v.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.as_str().map(str::to_owned).ok_or_else(|| {
                            RunApiError::invalid(format!("{key} must contain strings"))
                        })
                    })
                    .collect()
            })
            .unwrap_or_else(|| Ok(Vec::new()))
    };
    for trait_instance in &world.traits {
        let values = &trait_instance.values;
        let routes = |key: &str| -> Result<Vec<u16>, RunApiError> {
            strings(values, key)?
                .iter()
                .map(|id| {
                    content
                        .storylet_index(id)
                        .ok_or_else(|| RunApiError::invalid(format!("unknown storylet {id}")))
                })
                .collect()
        };
        match trait_instance.template_id.as_str() {
            "system.milestone" => {
                let reachable_for = strings(values, "reachable_for")?
                    .iter()
                    .map(|name| match name.as_str() {
                        "solo" => Ok(ParticipantConfig::Solo),
                        "coop" => Ok(ParticipantConfig::AllHumans),
                        _ => Err(RunApiError::invalid(format!(
                            "unknown participant configuration {name}"
                        ))),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let raw_deadline = values
                    .get("deadline_slot")
                    .and_then(Value::as_i64)
                    .unwrap_or(-1);
                let deadline_slot = if raw_deadline == -1 {
                    None
                } else {
                    Some(
                        u32::try_from(raw_deadline)
                            .map_err(|_| RunApiError::invalid("invalid milestone deadline"))?,
                    )
                };
                cfg.milestones.push(Milestone {
                    key: text(values, "key"),
                    name: text(values, "name"),
                    routes: routes("routes")?,
                    reachable_for,
                    deadline_slot,
                });
            }
            "system.intent_probe" => {
                let holder_knows = strings(values, "holder_knows")?
                    .iter()
                    .map(|fact| {
                        content
                            .registry
                            .id(fact)
                            .ok_or_else(|| RunApiError::invalid(format!("unknown fact {fact}")))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let colocated_with = values
                    .get("colocated_with")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(|id| {
                        let p = content.participant_indices.get(id).ok_or_else(|| {
                            RunApiError::invalid(format!("unknown participant {id}"))
                        })?;
                        content
                            .initial_world
                            .participants
                            .entity
                            .get(*p as usize)
                            .copied()
                            .ok_or_else(|| {
                                RunApiError::invalid(format!("unknown participant {id}"))
                            })
                    })
                    .transpose()?;
                cfg.probes.push(IntentProbe {
                    name: text(values, "name"),
                    holder_knows,
                    colocated_with,
                    expected_routes: routes("expected_routes")?,
                });
            }
            _ => {}
        }
    }
    Ok(cfg)
}

impl StudioService {
    pub async fn run_coverage(&self, req: CoverageRequest) -> Result<Value, RunApiError> {
        if req.runs.is_some_and(|runs| runs == 0 || runs > 50_000) {
            return Err(RunApiError::invalid("runs must be between 1 and 50000"));
        }
        if self
            .coverage_running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            return Err(RunApiError::conflict("coverage already running"));
        }
        let permit = CoveragePermit(self.coverage_running.clone());
        let (content, config) = {
            let inner = self.inner.lock().await;
            let (content, scenario_seed) = compile_run_content(&inner.store, None, None, None)
                .map_err(RunApiError::invalid)?;
            let world = inner
                .store
                .list_entities()
                .map_err(RunApiError::internal)?
                .into_iter()
                .find(|e| e.id == crate::entities::WORLD_ENTITY_ID)
                .ok_or_else(|| RunApiError::invalid("missing World entity"))?;
            if inner
                .store
                .get_project_metadata()
                .map_err(RunApiError::internal)?
                .revision
                != content.content_revision
            {
                return Err(RunApiError::conflict(
                    "project changed while compiling coverage",
                ));
            }
            let mut config = compile_coverage_config(&world, &content, &req)?;
            if req.seed.is_none() {
                config.seed = scenario_seed;
            }
            (content, config)
        };
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut report =
                serde_json::to_value(storylet_engine::chrono_explore::explore(&content, &config))
                    .map_err(RunApiError::internal)?;
            report["content_revision"] = json!(content.content_revision);
            report["runs"] = json!(config.runs);
            report["seed"] = json!(config.seed.to_string());
            for field in ["violations", "errors"] {
                if let Some(rows) = report[field].as_array_mut() {
                    for row in rows {
                        if let Some(seed) = row["seed"].as_u64() {
                            row["seed"] = json!(seed.to_string());
                        }
                    }
                }
            }
            report["milestones"] = json!(config.milestones);
            report["probes"] = json!(config.probes);
            report["storylets"] = Value::Object(
                content
                    .storylet_titles
                    .iter()
                    .map(|(id, title)| {
                        (
                            id.to_string(),
                            json!({"title":title,"key":content.storylet_key(*id)}),
                        )
                    })
                    .collect(),
            );
            report["locations"] = Value::Object(
                content
                    .entity_names
                    .iter()
                    .map(|(id, name)| (id.to_string(), json!(name)))
                    .collect(),
            );
            Ok(report)
        })
        .await
        .map_err(RunApiError::internal)?
    }
}
pub async fn coverage(
    State(s): State<StudioService>,
    Json(req): Json<CoverageRequest>,
) -> Response {
    match s.run_coverage(req).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn run_content(State(service): State<StudioService>) -> Response {
    let inner = service.inner.lock().await;
    match compile_run_content(&inner.store, None, None, None) {
        Ok((content, _)) => Json(content).into_response(),
        Err(error) => RunApiError::invalid(error).into_response(),
    }
}
