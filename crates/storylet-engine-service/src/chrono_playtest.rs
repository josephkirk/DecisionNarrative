//! Persisted ChronoFlow v5 playtest lane (REQ-CH-02).
//!
//! A ChronoFlow session persists only its `CallHistoryEntry` arrays — the global
//! call history plus the per-mission scoped call histories — the random seed, and
//! the pinned authoring metadata. No `FlowGraphInstance` and no `WorldStateTable`
//! snapshot is stored. Loading a session rebuilds the runtime instance and world
//! from the current project compilation and reconstructs state by replaying the
//! recorded history (REQ-CH-03).
//!
//! The pinned ID space is stored as session metadata, not as game state.
//! `rebase_call_history` only checks that a recorded node or storylet still
//! exists; it cannot detect that a surviving id was remapped onto a different
//! storylet or metric. Without the pinned ID space, a replay after a service
//! restart would silently reinterpret the persisted history.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use storylet_engine::chrono_dispatch::UnifiedDispatcher;
use storylet_engine::chrono_runtime::{
    CallHistoryEntry, FlowGraphAsset, FlowGraphInstance, GraphError, NodeCallData,
    NodeExecutionState, NodeKind, PassionThresholdTriggerEvent, RuntimeContext, SignalToken,
    rebase_call_history,
};
use storylet_engine::chrono_state::WorldStateTable;

use crate::chrono_adapter::{
    CompiledChronoDispatch, compile_chrono_dispatch, initialize_chrono_world,
};
use crate::chrono_graph_adapter::{bind_storylet_start_nodes, compile_narrative_graphs};
use crate::contracts::{
    CharacterRecord, LocationRecord, QualityRecord, ScenarioRecord, StoryletRecord,
};
use crate::scenario::resolve_scenario_start;
use crate::service::{StudioInner, StudioService, chrono_now_ms};
use crate::store::{ChronoPlaytestRow, Store};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChronoPlaytestError {
    InvalidInput(String),
    SessionNotFound(String),
    Conflict(String),
    RevisionConflict(String),
    Runtime(String),
    Persistence(String),
}

impl std::fmt::Display for ChronoPlaytestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::Conflict(message)
            | Self::RevisionConflict(message)
            | Self::Runtime(message)
            | Self::Persistence(message) => formatter.write_str(message),
            Self::SessionNotFound(session_id) => {
                write!(
                    formatter,
                    "chrono playtest session '{session_id}' was not found"
                )
            }
        }
    }
}

fn runtime_error(error: GraphError) -> ChronoPlaytestError {
    ChronoPlaytestError::Runtime(format!("{error}"))
}

fn persistence_error(error: rusqlite::Error) -> ChronoPlaytestError {
    ChronoPlaytestError::Persistence(format!("{error}"))
}

fn compile_error(error: String) -> ChronoPlaytestError {
    ChronoPlaytestError::Runtime(error)
}

// --- Requests --------------------------------------------------------------

pub trait PlaytestRequest {
    fn validate(&self) -> Result<(), String>;
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateChronoPlaytestRequest {
    pub session_id: Option<String>,
    pub scenario_id: Option<String>,
    pub start_location_id: Option<String>,
    pub storylet_id: Option<String>,
    pub seed: Option<u64>,
}

impl PlaytestRequest for CreateChronoPlaytestRequest {
    fn validate(&self) -> Result<(), String> {
        if self
            .session_id
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("Missing sessionId".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestTickRequest {
    pub frame_budget_ms: Option<i64>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestTickRequest {
    fn validate(&self) -> Result<(), String> {
        let Some(budget) = self.frame_budget_ms else {
            return Err("Missing frameBudgetMs".into());
        };
        if budget < 0 {
            return Err("frameBudgetMs must not be negative".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestSignalRequest {
    pub event_name: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestSignalRequest {
    fn validate(&self) -> Result<(), String> {
        if self
            .event_name
            .as_deref()
            .is_none_or(|event| event.trim().is_empty())
        {
            return Err("Missing eventName".into());
        }
        Ok(())
    }
}

/// Resolve a dynamic branch: exactly one of `outputPin` (a Choice node) or
/// `storyletId` (a QBN hub or salience selector).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestChooseRequest {
    pub node_guid: Option<u32>,
    pub output_pin: Option<String>,
    pub storylet_id: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestChooseRequest {
    fn validate(&self) -> Result<(), String> {
        let Some(guid) = self.node_guid else {
            return Err("Missing nodeGuid".into());
        };
        if guid == 0 {
            return Err("nodeGuid must not be zero".into());
        }
        let pin = self
            .output_pin
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let storylet = self
            .storylet_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        if pin == storylet {
            return Err("Missing exactly one of outputPin or storyletId".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestDivertRequest {
    pub node_guid: Option<u32>,
    pub edge_id: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestDivertRequest {
    fn validate(&self) -> Result<(), String> {
        if self.node_guid.is_none_or(|guid| guid == 0) {
            return Err("Missing valid nodeGuid".into());
        }
        if self
            .edge_id
            .as_deref()
            .is_none_or(|edge| edge.trim().is_empty())
        {
            return Err("Missing edgeId".into());
        }
        Ok(())
    }
}

/// Scrub a scene-section timeline forward or backward (REQ-SC-04).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestScrubRequest {
    pub node_guid: Option<u32>,
    pub delta_ms: Option<i64>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestScrubRequest {
    fn validate(&self) -> Result<(), String> {
        if self.node_guid.is_none_or(|guid| guid == 0) {
            return Err("Missing valid nodeGuid".into());
        }
        match self.delta_ms {
            None | Some(0) => Err("deltaMs must be a non-zero millisecond delta".into()),
            Some(_) => Ok(()),
        }
    }
}

/// Rewind one mission subgraph to its start (REQ-CH-04).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestRollbackRequest {
    pub node_guid: Option<u32>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestRollbackRequest {
    fn validate(&self) -> Result<(), String> {
        if self.node_guid.is_none_or(|guid| guid == 0) {
            return Err("Missing valid nodeGuid".into());
        }
        Ok(())
    }
}

/// Invoke a story to a POI node. `declaredRoutes` maps a node GUID to the input
/// pin `seek_backward` should walk back through when that node has several
/// incoming edges. (It selects an input pin, not the node's chosen output.)
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestInvokeRequest {
    pub node_guid: Option<u32>,
    pub declared_routes: BTreeMap<String, String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestInvokeRequest {
    fn validate(&self) -> Result<(), String> {
        let Some(guid) = self.node_guid else {
            return Err("Missing nodeGuid".into());
        };
        if guid == 0 {
            return Err("nodeGuid must not be zero".into());
        }
        if self.declared_routes.is_empty() {
            return Err("Missing declaredRoutes".into());
        }
        for (key, pin) in &self.declared_routes {
            if key
                .parse::<u32>()
                .ok()
                .filter(|value| *value != 0)
                .is_none()
            {
                return Err(format!("declaredRoutes key '{key}' is not a node GUID"));
            }
            if pin.trim().is_empty() {
                return Err(format!("declaredRoutes pin for '{key}' is empty"));
            }
        }
        Ok(())
    }
}

/// Reconcile with an external history (REQ-CH-06). Entries use the engine's
/// persisted record shape, which is also the shape this lane returns.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChronoPlaytestReconcileRequest {
    pub history: Vec<CallHistoryEntry>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_mutation_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl PlaytestRequest for ChronoPlaytestReconcileRequest {
    fn validate(&self) -> Result<(), String> {
        // An empty history is the valid rollback target for a fresh-world
        // reconcile; only validate entries that are present.
        for entry in &self.history {
            if entry.node_guid == 0 {
                return Err("history entry nodeGuid must not be zero".into());
            }
        }
        Ok(())
    }
}

pub fn parse_request<T>(args: &Value) -> Result<T, ChronoPlaytestError>
where
    T: PlaytestRequest + DeserializeOwned,
{
    // The MCP envelope carries the operation name and session id alongside the
    // request fields; strip them before strict deserialization.
    let mut payload = args.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("operation");
        object.remove("id");
    }
    let request: T = serde_json::from_value(payload)
        .map_err(|error| ChronoPlaytestError::InvalidInput(format!("{error}")))?;
    request
        .validate()
        .map_err(ChronoPlaytestError::InvalidInput)?;
    Ok(request)
}

// --- Pinned ID space -------------------------------------------------------

/// The deterministic name→id mapping a compilation produced. `passions` is keyed
/// `"{character}.{passion}"` because the adapter's passion indices are tuples.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledIdSpace {
    pub storylets: BTreeMap<String, u16>,
    pub metrics: BTreeMap<String, u16>,
    pub passions: BTreeMap<String, u16>,
    pub entities: BTreeMap<String, u16>,
    pub tags: BTreeMap<String, u16>,
}

impl CompiledIdSpace {
    fn fingerprint(
        compiled: &CompiledChronoDispatch,
        entity_indices: &BTreeMap<String, u16>,
    ) -> CompiledIdSpace {
        let passions = compiled
            .passion_indices
            .iter()
            .map(|(key, index)| (format!("{}.{}", key.0, key.1), *index))
            .collect();
        let tags = (0..compiled.registry.tag_count() as u16)
            .filter_map(|index| compiled.registry.tag(index).map(|tag| tag.to_string()))
            .zip(0..compiled.registry.tag_count() as u16)
            .collect();
        CompiledIdSpace {
            storylets: compiled
                .storylet_catalog()
                .space()
                .iter()
                .map(|(index, key)| (key.to_owned(), index))
                .collect(),
            metrics: compiled.metric_indices.clone(),
            passions,
            entities: entity_indices.clone(),
            tags,
        }
    }
}

/// Reverse lookups for stable payloads.
#[derive(Clone, Debug, Default)]
struct ReverseIndex {
    storylets: BTreeMap<u16, String>,
    metrics: BTreeMap<u16, String>,
    passions: BTreeMap<u16, String>,
    entities: BTreeMap<u16, String>,
    tags: BTreeMap<u16, String>,
}

impl ReverseIndex {
    fn build(space: &CompiledIdSpace) -> ReverseIndex {
        fn reverse(map: &BTreeMap<String, u16>) -> BTreeMap<u16, String> {
            map.iter()
                .map(|(name, index)| (*index, name.clone()))
                .collect()
        }
        ReverseIndex {
            storylets: reverse(&space.storylets),
            metrics: reverse(&space.metrics),
            passions: reverse(&space.passions),
            entities: reverse(&space.entities),
            tags: reverse(&space.tags),
        }
    }
}

// --- Session model ---------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ChronoPlaytestSession {
    pub session_id: String,
    pub project_id: String,
    pub pinned_project_revision: u64,
    pub scenario_id: String,
    pub start_location_id: String,
    pub start_storylet_id: String,
    pub start_node_guid: u32,
    pub seed: u64,
    pub id_space: CompiledIdSpace,
    pub instance: FlowGraphInstance,
    pub world: WorldStateTable,
    /// True when this session was rebuilt by replaying persisted history rather
    /// than created in this process. The clock, turn, and cooldown table come
    /// from the pending-state column, not from the call history.
    pub reconstructed: bool,
    /// Last idempotency key and its exact response, so a retried mutation
    /// returns the original payload instead of advancing twice.
    pub last_idempotency_key: String,
    pub last_result_json: String,
    /// Monotonic per-session mutation counter for optimistic concurrency.
    pub mutation_revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Mission-scoped histories, keyed by mission node GUID (REQ-CH-04).
fn scoped_histories(session: &ChronoPlaytestSession) -> BTreeMap<u32, Vec<CallHistoryEntry>> {
    fn collect(instance: &FlowGraphInstance, histories: &mut BTreeMap<u32, Vec<CallHistoryEntry>>) {
        for node in &instance.nodes {
            if !node.local_call_history.is_empty() {
                histories.insert(node.node_guid, node.local_call_history.clone());
            }
            if let Some(child) = instance.mission_instance(node.node_guid) {
                collect(child, histories);
            }
        }
    }

    let mut histories = BTreeMap::new();
    collect(&session.instance, &mut histories);
    histories
}

/// Stable instance id derived from the session id, so a rebuild reproduces it.
fn deterministic_instance_id(session_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in session_id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x1000000000000000);
    }
    hash
}

fn to_json<T: Serialize>(value: &T) -> Result<String, ChronoPlaytestError> {
    serde_json::to_string(value)
        .map_err(|error| ChronoPlaytestError::Persistence(format!("{error}")))
}

fn from_json<T: DeserializeOwned>(text: &str) -> Result<T, ChronoPlaytestError> {
    serde_json::from_str(text).map_err(|error| ChronoPlaytestError::Persistence(format!("{error}")))
}

#[derive(Clone, Debug, Default)]
struct WorkspaceContent {
    qualities: Vec<QualityRecord>,
    locations: Vec<LocationRecord>,
    characters: Vec<CharacterRecord>,
    storylets: Vec<StoryletRecord>,
    scenarios: Vec<ScenarioRecord>,
}

fn read_content(store: &Store) -> Result<WorkspaceContent, ChronoPlaytestError> {
    Ok(WorkspaceContent {
        qualities: store.list_qualities().map_err(persistence_error)?,
        locations: store.list_locations().map_err(persistence_error)?,
        characters: store.list_characters().map_err(persistence_error)?,
        storylets: store.list_storylets().map_err(persistence_error)?,
        scenarios: store.list_scenarios().map_err(persistence_error)?,
    })
}

// --- Revision pin (REQ-CH-07) ---------------------------------------------

/// Map each mission node GUID to the asset that owns its child scope. A scoped
/// history's entries carry child-asset GUIDs, so rebasing it against the root
/// asset drops or misreads them.
fn mission_asset_map(
    root: &FlowGraphAsset,
    mission_assets: &BTreeMap<String, FlowGraphAsset>,
) -> Result<BTreeMap<u32, FlowGraphAsset>, ChronoPlaytestError> {
    fn collect(
        asset: &FlowGraphAsset,
        all: &BTreeMap<String, FlowGraphAsset>,
        out: &mut BTreeMap<u32, FlowGraphAsset>,
    ) -> Result<(), ChronoPlaytestError> {
        for node in &asset.nodes {
            let NodeKind::MissionSubGraph {
                sub_graph_asset_id,
            } = &node.kind
            else {
                continue;
            };
            let Some(child) = all.get(sub_graph_asset_id) else {
                continue;
            };
            if let Some(existing) = out.get(&node.node_guid) {
                if existing.asset_id != child.asset_id {
                    return Err(ChronoPlaytestError::RevisionConflict(format!(
                        "mission node GUID {} is defined by both asset '{}' and asset '{}'",
                        node.node_guid, existing.asset_id, child.asset_id
                    )));
                }
            } else {
                out.insert(node.node_guid, child.clone());
            }
            collect(child, all, out)?;
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    collect(root, mission_assets, &mut out)?;
    for asset in mission_assets.values() {
        collect(asset, mission_assets, &mut out)?;
    }
    Ok(out)
}

/// A session is only operable against the compilation it was created from. If
/// the authored revision moved, the freshly compiled ID space must still match —
/// otherwise the persisted u16 ids would be reinterpreted — and every history is
/// rebased onto the new asset. Dropped nodes become tombstones; removed
/// connections are bridged. Returns the adopted history when a rebase happened.
#[allow(clippy::too_many_arguments)]
fn enforce_revision_pin(
    pinned_revision: u64,
    current_revision: u64,
    pinned_space: &CompiledIdSpace,
    current_space: &CompiledIdSpace,
    asset: &FlowGraphAsset,
    mission_assets: &BTreeMap<String, FlowGraphAsset>,
    dispatcher: &UnifiedDispatcher,
    global: &[CallHistoryEntry],
    scoped: &BTreeMap<u32, Vec<CallHistoryEntry>>,
) -> Result<Option<RebasedHistory>, ChronoPlaytestError> {
    if pinned_revision == current_revision {
        // `import_bundle` can rewrite content at the same revision counter, so
        // the compiled ID space is the real identity check.
        if pinned_space != current_space {
            return Err(ChronoPlaytestError::RevisionConflict(format!(
                "project revision {current_revision} matches the session's pinned revision, but the compiled ID space changed"
            )));
        }
        return Ok(None);
    }
    if pinned_space != current_space {
        return Err(ChronoPlaytestError::RevisionConflict(format!(
            "project revision {current_revision} is not the session's pinned revision {pinned_revision}, and the compiled ID space changed"
        )));
    }
    let global = rebase_call_history(asset, dispatcher, global).map_err(runtime_error)?;
    let mission_assets_by_node = mission_asset_map(asset, mission_assets)?;
    let mut scoped_rebased = BTreeMap::new();
    for (node_guid, entries) in scoped {
        // No asset owns this GUID anymore: the mission was deleted, so drop
        // the scope instead of rebasing it against the wrong graph.
        let Some(scope_asset) = mission_assets_by_node.get(node_guid) else {
            continue;
        };
        scoped_rebased.insert(
            *node_guid,
            rebase_call_history(scope_asset, dispatcher, entries).map_err(runtime_error)?,
        );
    }
    Ok(Some(RebasedHistory {
        global,
        scoped: scoped_rebased,
    }))
}

struct RebasedHistory {
    global: Vec<CallHistoryEntry>,
    scoped: BTreeMap<u32, Vec<CallHistoryEntry>>,
}

/// One operation's freshly compiled registries, content, and ID space.
struct Prepared {
    current_revision: u64,
    content: WorkspaceContent,
    compiled: CompiledChronoDispatch,
    current_id_space: CompiledIdSpace,
    rebased: bool,
}

fn prepare(
    inner: &StudioInner,
    session: &mut ChronoPlaytestSession,
) -> Result<Prepared, ChronoPlaytestError> {
    let content = read_content(&inner.store)?;
    let meta = inner
        .store
        .get_project_metadata()
        .map_err(persistence_error)?;
    let mut compiled = compile_chrono_dispatch(
        &content.storylets,
        &content.qualities,
        &content.locations,
        &content.characters,
    )
    .map_err(compile_error)?;
    let scenario = content
        .scenarios
        .iter()
        .find(|scenario| scenario.id == session.scenario_id);
    let seed = initialize_chrono_world(
        &compiled,
        &content.qualities,
        &content.locations,
        &content.characters,
        scenario,
        &session.start_location_id,
    )
    .map_err(compile_error)?;
    let current_id_space = CompiledIdSpace::fingerprint(&compiled, &seed.entity_indices);
    let graphs = compile_narrative_graphs(
        &session.project_id,
        &content.storylets,
        &compiled,
        &seed.entity_indices,
    )
    .map_err(compile_error)?;
    bind_storylet_start_nodes(&mut compiled, &graphs).map_err(compile_error)?;

    let scoped = scoped_histories(session);
    let rebased = enforce_revision_pin(
        session.pinned_project_revision,
        meta.revision,
        &session.id_space,
        &current_id_space,
        &graphs.asset,
        &graphs.mission_assets,
        &compiled.dispatcher,
        &session.instance.global_call_history,
        &scoped,
    )?;
    let was_rebased = rebased.is_some();
    if let Some(history) = rebased {
        // Adopt the recompiled asset and the rebased history. The world must
        // start from the seed again: replaying into the live post-history
        // world would apply every delta twice.
        let pending = pending_state(session);
        session.world = seed.state.clone();
        let mut rebuilt = FlowGraphInstance::new(
            session.instance.instance_id,
            graphs.asset.clone(),
            session.seed,
        )
        .map_err(runtime_error)?;
        bind_catalog_keys(&mut rebuilt, &compiled);
        rebuilt
            .attach_mission_assets(&graphs.mission_assets, session.seed)
            .map_err(runtime_error)?;
        let mut ctx = RuntimeContext::with_registries(
            &mut session.world,
            &compiled.dispatcher,
            &compiled.registry,
            &compiled.passion_registry,
        );
        rebuilt
            .replay_history_with_missions(&mut ctx, &history.global, &history.scoped)
            .map_err(runtime_error)?;
        drop(ctx);
        restore_pending(&mut rebuilt, &pending, PinMismatch::Drop)?;
        session.instance = rebuilt;
        session.pinned_project_revision = meta.revision;
        session.id_space = current_id_space.clone();
    }

    Ok(Prepared {
        current_revision: meta.revision,
        content,
        compiled,
        current_id_space,
        rebased: was_rebased,
    })
}

/// Load a session: the live instance when this process already has one, otherwise
/// rebuild it from the persisted history (REQ-CH-03).
fn load_session(
    inner: &StudioInner,
    session_id: &str,
) -> Result<ChronoPlaytestSession, ChronoPlaytestError> {
    if let Some(cached) = inner.chrono_sessions.get(session_id) {
        return Ok(cached.clone());
    }
    let row = inner
        .store
        .get_chrono_playtest_row(session_id)
        .map_err(persistence_error)?
        .ok_or_else(|| ChronoPlaytestError::SessionNotFound(session_id.to_string()))?;

    let pinned_space: CompiledIdSpace = from_json(&row.pinned_id_space_json)?;
    let global: Vec<CallHistoryEntry> = from_json(&row.global_call_history_json)?;
    let scoped: BTreeMap<u32, Vec<CallHistoryEntry>> = from_json(&row.scoped_call_histories_json)?;
    let pending: PendingState = from_json(&row.pending_state_json)?;

    let content = read_content(&inner.store)?;
    let meta = inner
        .store
        .get_project_metadata()
        .map_err(persistence_error)?;
    let mut compiled = compile_chrono_dispatch(
        &content.storylets,
        &content.qualities,
        &content.locations,
        &content.characters,
    )
    .map_err(compile_error)?;
    let scenario = content
        .scenarios
        .iter()
        .find(|scenario| scenario.id == row.scenario_id);
    let seed = initialize_chrono_world(
        &compiled,
        &content.qualities,
        &content.locations,
        &content.characters,
        scenario,
        &row.start_location_id,
    )
    .map_err(compile_error)?;
    let current_id_space = CompiledIdSpace::fingerprint(&compiled, &seed.entity_indices);
    let graphs = compile_narrative_graphs(
        &row.project_id,
        &content.storylets,
        &compiled,
        &seed.entity_indices,
    )
    .map_err(compile_error)?;
    bind_storylet_start_nodes(&mut compiled, &graphs).map_err(compile_error)?;

    let pinned_revision = row.pinned_project_revision as u64;
    let rebased = enforce_revision_pin(
        pinned_revision,
        meta.revision,
        &pinned_space,
        &current_id_space,
        &graphs.asset,
        &graphs.mission_assets,
        &compiled.dispatcher,
        &global,
        &scoped,
    )?;
    let was_rebased = rebased.is_some();
    let (global, scoped) = match rebased {
        Some(history) => (history.global, history.scoped),
        None => (global, scoped),
    };

    let mut instance = FlowGraphInstance::new(
        deterministic_instance_id(&row.session_id),
        graphs.asset.clone(),
        row.seed as u64,
    )
    .map_err(runtime_error)?;
    bind_catalog_keys(&mut instance, &compiled);
    instance
        .attach_mission_assets(&graphs.mission_assets, row.seed as u64)
        .map_err(runtime_error)?;
    let mut session = ChronoPlaytestSession {
        session_id: row.session_id.clone(),
        project_id: row.project_id.clone(),
        pinned_project_revision: if was_rebased {
            meta.revision
        } else {
            pinned_revision
        },
        scenario_id: row.scenario_id.clone(),
        start_location_id: row.start_location_id.clone(),
        start_storylet_id: row.start_storylet_id.clone(),
        start_node_guid: row.start_node_guid as u32,
        seed: row.seed as u64,
        id_space: if was_rebased {
            current_id_space
        } else {
            pinned_space
        },
        instance,
        world: seed.state,
        reconstructed: true,
        last_idempotency_key: row.last_idempotency_key.clone(),
        last_result_json: row.last_result_json.clone(),
        mutation_revision: row.mutation_revision,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    let mut ctx = RuntimeContext::with_registries(
        &mut session.world,
        &compiled.dispatcher,
        &compiled.registry,
        &compiled.passion_registry,
    );
    session
        .instance
        .replay_history_with_missions(&mut ctx, &global, &scoped)
        .map_err(runtime_error)?;
    drop(ctx);
    restore_pending(
        &mut session.instance,
        &pending,
        if was_rebased {
            PinMismatch::Drop
        } else {
            PinMismatch::Error
        },
    )?;
    Ok(session)
}

// --- Payloads ------------------------------------------------------------

fn call_history_value(entries: &[CallHistoryEntry]) -> Value {
    serde_json::to_value(entries).unwrap_or(Value::Null)
}

fn bind_catalog_keys(
    instance: &mut FlowGraphInstance,
    compiled: &crate::chrono_adapter::CompiledChronoDispatch,
) {
    instance.bind_storylet_keys(
        compiled
            .storylet_catalog()
            .space()
            .iter()
            .map(|(index, key)| (index, key.to_owned())),
    );
}

fn owning_instance(instance: &FlowGraphInstance, instance_id: u64) -> &FlowGraphInstance {
    if instance.instance_id == instance_id {
        return instance;
    }
    for node in &instance.nodes {
        if let Some(child) = node.sub_graph.as_deref() {
            if child.instance_id == instance_id {
                return child;
            }
            let nested = owning_instance(child, instance_id);
            if nested.instance_id == instance_id {
                return nested;
            }
        }
    }
    instance
}

fn tokens_value(instance: &FlowGraphInstance, tokens: &[SignalToken]) -> Value {
    Value::Array(
        tokens
            .iter()
            .map(|token| {
                let owner = owning_instance(instance, token.graph_instance_id);
                json!({
                    "token_id": token.token_id.to_string(),
                    "graph_instance_id": token.graph_instance_id.to_string(),
                    "target_node_guid": token.target_node_guid,
                    "target_pin_name": owner
                        .asset
                        .input_pin_name(token.target_node_guid, token.target_pin_index)
                        .unwrap_or_default(),
                    "time_budget_ms": i64::from(token.time_budget_ms),
                    "is_active": token.is_active(),
                    "deferred_until_ms": i64::from(token.deferred_until_ms),
                    "is_deferred": token.is_deferred(),
                })
            })
            .collect(),
    )
}

/// REQ-BB-03 display metadata comes from authored Chrono storylet settings.
fn storylet_display(storylet: &StoryletRecord) -> Value {
    let settings = storylet.chrono.as_ref();
    json!({
        "id": storylet.id,
        "title": storylet.title,
        "clusterType": storylet.cluster_type,
        "displayPriority": settings.map(|settings| settings.display_priority).unwrap_or(0),
        "urgencyColorTag": settings.map(|settings| settings.urgency_color_tag.as_str()).unwrap_or("Standard")
    })
}

fn session_descriptor(
    session: &ChronoPlaytestSession,
    current_revision: u64,
    rebased: bool,
) -> Value {
    json!({
        "sessionId": session.session_id,
        "projectId": session.project_id,
        "pinnedProjectRevision": session.pinned_project_revision,
        "currentProjectRevision": current_revision,
        "mutationRevision": session.mutation_revision,
        "scenarioId": session.scenario_id,
        "startLocationId": session.start_location_id,
        "startStoryletId": session.start_storylet_id,
        "startNodeGuid": session.start_node_guid,
        "seed": session.seed,
        "rebased": rebased,
        "createdAt": session.created_at,
        "updatedAt": session.updated_at
    })
}

/// Storylets referenced by the current context, in deterministic order.
fn storylets_in_context(session: &ChronoPlaytestSession, prepared: &Prepared) -> Vec<Value> {
    let reverse = ReverseIndex::build(&session.id_space);
    let mut referenced = BTreeSet::from([session.start_storylet_id.clone()]);
    for entry in &session.instance.global_call_history {
        if let Some(index) = entry.chosen_storylet_id {
            if let Some(id) = reverse.storylets.get(&index) {
                referenced.insert(id.clone());
            }
        }
    }
    for node in &session.instance.nodes {
        if let Some(index) = node.chosen_storylet_id {
            if let Some(id) = reverse.storylets.get(&index) {
                referenced.insert(id.clone());
            }
        }
    }
    prepared
        .content
        .storylets
        .iter()
        .filter(|storylet| referenced.contains(&storylet.id))
        .map(storylet_display)
        .collect()
}

fn passion_transition_events(instance: &FlowGraphInstance, reverse: &ReverseIndex) -> Vec<Value> {
    let mut events = Vec::new();
    for node in &instance.nodes {
        for call in &node.call_data {
            let Some(passion_delta) = call.delta().and_then(|delta| delta.passion_delta.as_ref())
            else {
                continue;
            };
            for event in &passion_delta.threshold_events {
                events.push(json!({
                    "nodeGuid": node.node_guid,
                    "npcId": event.npc_id,
                    "characterId": reverse.entities.get(&event.npc_id),
                    "thresholdTag": reverse.tags.get(&event.threshold_tag_bit),
                    "linkedGraphAssetId": event.linked_graph_asset_id,
                    "activated": event.activated,
                    "action": if event.activated { "activate" } else { "cancel" }
                }));
            }
        }
        if let Some(child) = node.sub_graph.as_deref() {
            events.extend(passion_transition_events(child, reverse));
        }
    }
    events
}

fn context_view(session: &ChronoPlaytestSession, prepared: &Prepared) -> Value {
    let reverse = ReverseIndex::build(&session.id_space);
    let instance = &session.instance;

    let nodes = instance
        .nodes
        .iter()
        .map(|node| {
            let def = instance.asset.node(node.node_guid);
            json!({
                "nodeGuid": node.node_guid,
                "nodeTypeId": def.map(|def| def.node_type_id.clone()).unwrap_or_default(),
                "category": serde_json::to_value(def.map(|def| def.category)).unwrap_or(Value::Null),
                "state": serde_json::to_value(&node.state).unwrap_or(Value::Null),
                "storyletId": node
                    .chosen_storylet_id
                    .and_then(|index| reverse.storylets.get(&index).cloned()),
                "outputPins": def.map(|def| def.output_pins.iter().map(|pin| pin.pin_name.clone()).collect::<Vec<_>>()).unwrap_or_default(),
                "payload": def.map(|def| def.payload.clone()).unwrap_or(Value::Null),
                "localCallHistory": call_history_value(&node.local_call_history),
            })
        })
        .collect::<Vec<_>>();

    let active_interactions = instance
        .nodes
        .iter()
        .filter(|node| node.state == NodeExecutionState::ActiveLatent)
        .filter_map(|node| {
            let def = instance.asset.node(node.node_guid)?;
            match &def.kind {
                NodeKind::Choice { .. } => {
                    let choices = def
                        .payload
                        .get("nodeData")
                        .and_then(|data| data.get("choices"))
                        .and_then(Value::as_array);
                    let options = def
                        .output_pins
                        .iter()
                        .filter(|pin| pin.pin_name.starts_with("choice_"))
                        .map(|pin| {
                            let index = pin.pin_name["choice_".len()..]
                                .parse::<usize>()
                                .ok();
                            let choice = index
                                .and_then(|index| choices.and_then(|choices| choices.get(index)));
                            json!({
                                "outputPin": pin.pin_name,
                                "choiceId": choice.and_then(|value| value.get("id")).cloned().unwrap_or(Value::Null),
                                "label": choice.and_then(|value| value.get("text")).and_then(Value::as_str).unwrap_or(&pin.pin_name)
                            })
                        })
                        .collect::<Vec<_>>();
                    Some(json!({
                        "nodeGuid": node.node_guid,
                        "kind": "choice",
                        "options": options
                    }))
                }
                NodeKind::QbnHub { deck, .. } => {
                    let eligible_storylets = prepared
                        .compiled
                        .dispatcher
                        .try_query_qbn_deck_bitset(deck, &session.world, &instance.completed_storylets)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|storylet_id| {
                            prepared
                                .compiled
                                .dispatcher
                                .storylet(*storylet_id)
                                .is_none_or(|storylet| {
                                    instance
                                        .last_completed_ms
                                        .get(storylet_id)
                                        .is_none_or(|last| {
                                            UnifiedDispatcher::cooldown_remaining_ms(
                                                storylet,
                                                *last,
                                                instance.clock_ms,
                                            ) == 0
                                        })
                                })
                        })
                        .filter_map(|id| reverse.storylets.get(&id).cloned())
                        .filter_map(|id| {
                            let storylet = prepared.content.storylets.iter().find(|item| item.id == id)?;
                            let mut display = storylet_display(storylet);
                            if let Some(object) = display.as_object_mut() {
                                object.insert("nodeGuid".into(), json!(node.node_guid));
                            }
                            Some(display)
                        })
                        .collect::<Vec<_>>();
                    Some(json!({
                        "nodeGuid": node.node_guid,
                        "kind": "qbn",
                        "eligibleStorylets": eligible_storylets
                    }))
                }
                NodeKind::WaypointConversation { graph, initial_topic_id, .. } => {
                    let current_topic_id = if node.current_topic_id.is_empty() {
                        initial_topic_id
                    } else {
                        &node.current_topic_id
                    };
                    let topics = graph
                        .edges
                        .iter()
                        .filter(|edge| edge.from_topic_id == *current_topic_id)
                        .filter_map(|edge| {
                            let target = graph.nodes.iter().find(|topic| topic.topic_id == edge.to_topic_id)?;
                            Some(json!({
                                "edgeId": edge.edge_id,
                                "topicId": target.topic_id,
                                "playerOnly": target.is_player_only_trigger,
                                "triggersSceneExit": target.triggers_scene_exit,
                                "revealedClueQualityId": target.revealed_clue_quality_id
                            }))
                        })
                        .collect::<Vec<_>>();
                    Some(json!({
                        "nodeGuid": node.node_guid,
                        "kind": "waypoint",
                        "currentTopicId": current_topic_id,
                        "topics": topics
                    }))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    let metrics = (0..session.world.metrics.len() as u16)
        .map(|index| {
            (
                reverse.metrics.get(&index).cloned().unwrap_or_default(),
                session.world.metrics[index as usize],
            )
        })
        .collect::<BTreeMap<_, _>>();

    let global_tags = (0..session.id_space.tags.len() as u16)
        .filter(|index| session.world.global_tags.bit(*index))
        .filter_map(|index| reverse.tags.get(&index).cloned())
        .collect::<Vec<_>>();

    let entities = (0..session.id_space.entities.len() as u16)
        .filter_map(|index| {
            let row = session.world.entity_tags.get(index as usize)?;
            let owned = (0..session.id_space.tags.len() as u16)
                .filter(|bit| row.bit(*bit))
                .filter_map(|bit| reverse.tags.get(&bit).cloned())
                .collect::<Vec<_>>();
            Some((
                reverse.entities.get(&index)?.clone(),
                Value::Array(owned.into_iter().map(Value::String).collect()),
            ))
        })
        .collect::<BTreeMap<_, _>>();

    let micro = (0..session.world.passions.micro.len() as u16)
        .map(|index| {
            (
                reverse.passions.get(&index).cloned().unwrap_or_default(),
                session.world.passions.micro[index as usize],
            )
        })
        .collect::<BTreeMap<_, _>>();

    let active = (0..session.world.passions.active.len() as u16)
        .filter(|index| session.world.passions.active[*index as usize])
        .filter_map(|index| reverse.passions.get(&index).cloned())
        .collect::<Vec<_>>();

    // `macro_values` is indexed by Star-NPC id, which the adapter assigns as the
    // character's entity index, so the entity reverse map resolves it to a name.
    let macro_values = (0..session.world.passions.macro_values.len() as u16)
        .map(|index| {
            (
                reverse
                    .entities
                    .get(&index)
                    .cloned()
                    .unwrap_or_else(|| index.to_string()),
                session.world.passions.macro_values[index as usize],
            )
        })
        .collect::<BTreeMap<_, _>>();

    let passion_thresholds = prepared
        .compiled
        .passion_registry
        .threshold_rules()
        .iter()
        .map(|rule| {
            json!({
                "npcId": rule.npc_id,
                "characterId": reverse.entities.get(&rule.npc_id).cloned(),
                "thresholdTag": reverse.tags.get(&rule.threshold_tag_bit).cloned(),
                "thresholdTagBit": rule.threshold_tag_bit,
                "triggerValue": rule.trigger_value,
                "direction": serde_json::to_value(&rule.direction).unwrap_or(Value::Null),
                "linkedGraphAssetId": rule.linked_graph_asset_id,
                "activated": session.world.global_tags.bit(rule.threshold_tag_bit)
            })
        })
        .collect::<Vec<_>>();
    let passion_events = passion_transition_events(instance, &reverse);

    json!({
        "instanceId": instance.instance_id.to_string(),
        "assetId": instance.asset.asset_id,
        "turn": instance.turn,
        "clockMs": instance.clock_ms,
        "reconstructedFromHistory": session.reconstructed,
        "nodes": nodes,
        "tokens": tokens_value(instance, &instance.tokens),
        "activeInteractions": active_interactions,
        "world": {
            "currentLocation": session
                .world
                .current_location_entity
                .and_then(|index| reverse.entities.get(&index).cloned()),
            "metrics": metrics,
            "globalTags": global_tags,
            "entities": entities,
            "passions": {
                "micro": micro,
                "macro": macro_values,
                "active": active
            }
        },
        "storylets": storylets_in_context(session, prepared),
        "globalCallHistory": call_history_value(&instance.global_call_history),
        "scopedCallHistories": scoped_histories(session)
            .iter()
            .map(|(guid, entries)| (guid.to_string(), call_history_value(entries)))
            .collect::<serde_json::Map<String, Value>>(),
        "passionEvents": passion_events,
        "passionThresholds": passion_thresholds
    })
}

/// Latent node data that the call history cannot express: a Choice, QBN, or
/// Waiter pauses after its token is consumed and records no call, so the paused
/// node and the live token queue are persisted separately (REQ-CH-02/03).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PendingState {
    #[serde(default)]
    nodes: Vec<PendingNode>,
    /// Persisted with the authored pin name so a stored session survives a
    /// change to the in-memory pin representation.
    #[serde(default)]
    tokens: Vec<PersistedToken>,
    /// Runtime clock. Replay stamps completions at zero, so the real clock and
    /// cooldown table must be restored with the latent nodes.
    #[serde(default)]
    clock_ms: i64,
    #[serde(default)]
    turn: u64,
    #[serde(default)]
    last_completed_ms: BTreeMap<u16, i64>,
    /// Paused mission children, keyed by the mission node that owns them.
    #[serde(default)]
    missions: Vec<PendingMission>,
    /// Actor poses for in-progress scenes. The append-only applied/conflict
    /// logs stay transient so the session row does not grow every frame.
    #[serde(default)]
    actor_pose: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingMission {
    node_guid: u32,
    instance: PendingState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedToken {
    token_id: u64,
    graph_instance_id: u64,
    target_node_guid: u32,
    target_pin_name: String,
    time_budget_ms: i64,
    is_active: bool,
    deferred_until_ms: i64,
    is_deferred: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingNode {
    node_guid: u32,
    executed_input_pin: Option<String>,
    chosen_output_pin: Option<String>,
    chosen_storylet_id: Option<u16>,
    section_clock_ms: i64,
    waiter_wake_ms: i64,
    current_topic_id: String,
    arrived_input_pins: BTreeSet<String>,
    /// Inverse cache, so a restored latent node can still be rolled back.
    #[serde(default)]
    call_data: Vec<NodeCallData>,
}

fn pending_state(session: &ChronoPlaytestSession) -> PendingState {
    instance_pending_state(&session.instance)
}

fn instance_pending_state(instance: &FlowGraphInstance) -> PendingState {
    PendingState {
        nodes: instance
            .nodes
            .iter()
            .filter(|node| node.state == NodeExecutionState::ActiveLatent)
            .map(|node| PendingNode {
                node_guid: node.node_guid,
                executed_input_pin: node.executed_input_pin.clone(),
                chosen_output_pin: node.chosen_output_pin.clone(),
                chosen_storylet_id: node.chosen_storylet_id,
                section_clock_ms: node.section_clock_ms,
                waiter_wake_ms: node.waiter_wake_ms,
                current_topic_id: node.current_topic_id.clone(),
                arrived_input_pins: node.arrived_input_pins.clone(),
                call_data: node.call_data.clone(),
            })
            .collect(),
        tokens: instance
            .tokens
            .iter()
            .filter(|token| token.is_active() || token.is_deferred())
            .map(|token| PersistedToken {
                token_id: token.token_id,
                graph_instance_id: token.graph_instance_id,
                target_node_guid: token.target_node_guid,
                target_pin_name: instance
                    .asset
                    .input_pin_name(token.target_node_guid, token.target_pin_index)
                    .unwrap_or_default()
                    .to_owned(),
                time_budget_ms: i64::from(token.time_budget_ms),
                is_active: token.is_active(),
                deferred_until_ms: i64::from(token.deferred_until_ms),
                is_deferred: token.is_deferred(),
            })
            .collect(),
        clock_ms: instance.clock_ms,
        turn: instance.turn,
        last_completed_ms: instance.last_completed_ms.clone(),
        missions: instance
            .nodes
            .iter()
            .filter_map(|node| {
                node.sub_graph.as_deref().map(|child| PendingMission {
                    node_guid: node.node_guid,
                    instance: instance_pending_state(child),
                })
            })
            .collect(),
        actor_pose: instance.composer.actor_pose.clone(),
    }
}

/// How a rebuilt session treats a pending token whose pin no longer resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinMismatch {
    Error,
    Drop,
}

fn restore_pending(
    instance: &mut FlowGraphInstance,
    pending: &PendingState,
    pin_mismatch: PinMismatch,
) -> Result<(), ChronoPlaytestError> {
    for descriptor in &pending.nodes {
        if let Some(node) = instance.node_mut(descriptor.node_guid) {
            node.state = NodeExecutionState::ActiveLatent;
            node.executed_input_pin
                .clone_from(&descriptor.executed_input_pin);
            node.chosen_output_pin.clone_from(&descriptor.chosen_output_pin);
            node.chosen_storylet_id = descriptor.chosen_storylet_id;
            node.section_clock_ms = descriptor.section_clock_ms;
            node.waiter_wake_ms = descriptor.waiter_wake_ms;
            node.current_topic_id
                .clone_from(&descriptor.current_topic_id);
            node.arrived_input_pins
                .clone_from(&descriptor.arrived_input_pins);
            node.call_data.clone_from(&descriptor.call_data);
        }
    }
    let mut restored_tokens = Vec::new();
    for token in &pending.tokens {
        if token.graph_instance_id != instance.instance_id {
            continue;
        }
        let target_pin_index = match instance
            .asset
            .input_pin_index(token.target_node_guid, &token.target_pin_name)
        {
            Some(index) => index,
            None if pin_mismatch == PinMismatch::Drop => continue,
            None => {
                return Err(ChronoPlaytestError::Persistence(format!(
                    "pending token {} targets node {} pin '{}', which no longer exists",
                    token.token_id, token.target_node_guid, token.target_pin_name
                )))
            }
        };
        let mut restored = SignalToken {
            token_id: token.token_id,
            graph_instance_id: token.graph_instance_id,
            target_node_guid: token.target_node_guid,
            target_pin_index,
            flags: 0,
            time_budget_ms: token
                .time_budget_ms
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            deferred_until_ms: token
                .deferred_until_ms
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        };
        restored.set_active(token.is_active);
        restored.set_deferred(token.is_deferred);
        restored_tokens.push(restored);
    }
    instance.tokens = restored_tokens;
    instance.next_token_id = instance
        .tokens
        .iter()
        .map(|token| token.token_id.saturating_add(1))
        .max()
        .unwrap_or(0);
    instance.clock_ms = pending.clock_ms;
    instance.turn = pending.turn;
    instance
        .last_completed_ms
        .clone_from(&pending.last_completed_ms);
    instance
        .composer
        .actor_pose
        .clone_from(&pending.actor_pose);
    for mission in &pending.missions {
        let Some(child_index) = instance.node_index(mission.node_guid) else {
            continue;
        };
        if let Some(child) = instance.nodes[child_index].sub_graph.as_mut() {
            restore_pending(child, &mission.instance, pin_mismatch)?;
        }
    }
    Ok(())
}

fn trigger_events_value(
    events: Vec<PassionThresholdTriggerEvent>,
    id_space: &CompiledIdSpace,
) -> Vec<Value> {
    let reverse = ReverseIndex::build(id_space);
    events
        .into_iter()
        .map(|event| {
            json!({
                "eventType": "narrative_graph_trigger",
                "nodeGuid": event.node_guid,
                "action": if event.activated { "activate" } else { "cancel" },
                "graphAssetId": event.linked_graph_asset_id,
                "npcId": event.npc_id,
                "characterId": reverse.entities.get(&event.npc_id),
                "thresholdTagBit": event.threshold_tag_bit,
                "thresholdTag": reverse.tags.get(&event.threshold_tag_bit),
                "activated": event.activated
            })
        })
        .collect()
}

fn payload(
    session: &ChronoPlaytestSession,
    prepared: &Prepared,
    operation: &str,
    result: Value,
    trigger_events: Vec<Value>,
) -> Value {
    json!({
        "operation": operation,
        "session": session_descriptor(session, prepared.current_revision, prepared.rebased),
        "context": context_view(session, prepared),
        "result": result,
        "triggerEvents": trigger_events
    })
}

// --- Persistence -----------------------------------------------------------

fn persist_session(
    store: &Store,
    session: &ChronoPlaytestSession,
) -> Result<(), ChronoPlaytestError> {
    let row = ChronoPlaytestRow {
        session_id: session.session_id.clone(),
        project_id: session.project_id.clone(),
        pinned_project_revision: session.pinned_project_revision as i64,
        scenario_id: session.scenario_id.clone(),
        start_location_id: session.start_location_id.clone(),
        start_storylet_id: session.start_storylet_id.clone(),
        start_node_guid: session.start_node_guid as i64,
        seed: session.seed as i64,
        pinned_id_space_json: to_json(&session.id_space)?,
        global_call_history_json: to_json(&session.instance.global_call_history)?,
        scoped_call_histories_json: to_json(&scoped_histories(session))?,
        pending_state_json: to_json(&pending_state(session))?,
        last_idempotency_key: session.last_idempotency_key.clone(),
        last_result_json: session.last_result_json.clone(),
        mutation_revision: session.mutation_revision,
        created_at: session.created_at,
        updated_at: session.updated_at,
    };
    store
        .save_chrono_playtest_row(&row)
        .map_err(persistence_error)
}

/// Persist one operation transactionally, then publish the live session. On a
/// persistence failure the in-memory session is restored to its prior state.
fn finish(
    inner: &mut StudioInner,
    session: &mut ChronoPlaytestSession,
    prepared: &Prepared,
    operation: &str,
    result: Value,
    idempotency_key: Option<&str>,
) -> Result<Value, ChronoPlaytestError> {
    let prior = inner.chrono_sessions.get(&session.session_id).cloned();
    session.updated_at = chrono_now_ms();
    session.mutation_revision = session.mutation_revision.saturating_add(1);
    let trigger_events = trigger_events_value(
        session.instance.drain_passion_threshold_events(),
        &session.id_space,
    );
    let response = payload(session, prepared, operation, result, trigger_events);
    if let Some(key) = idempotency_key.filter(|key| !key.is_empty()) {
        session.last_idempotency_key = key.to_owned();
        session.last_result_json = to_json(&response)?;
    }
    if let Err(error) = persist_session(&inner.store, session) {
        if let Some(prior) = prior {
            inner
                .chrono_sessions
                .insert(session.session_id.clone(), prior);
        }
        return Err(error);
    }
    inner
        .chrono_sessions
        .insert(session.session_id.clone(), session.clone());
    Ok(response)
}

fn check_expected_revision(
    session: &ChronoPlaytestSession,
    expected_project: Option<u64>,
    expected_mutation: Option<u64>,
) -> Result<(), ChronoPlaytestError> {
    if let Some(expected) = expected_project {
        if expected != session.pinned_project_revision {
            return Err(ChronoPlaytestError::RevisionConflict(format!(
                "session is at project revision {} but expectedRevision is {expected}",
                session.pinned_project_revision
            )));
        }
    }
    if let Some(expected) = expected_mutation {
        if expected != session.mutation_revision {
            return Err(ChronoPlaytestError::RevisionConflict(format!(
                "session mutation revision is {} but expectedMutationRevision is {expected}",
                session.mutation_revision
            )));
        }
    }
    Ok(())
}

/// Reject stale clients and short-circuit retried mutations in one call.
fn begin_mutation(
    session: &ChronoPlaytestSession,
    expected_revision: Option<u64>,
    expected_mutation_revision: Option<u64>,
    idempotency_key: Option<&str>,
) -> Result<Option<Value>, ChronoPlaytestError> {
    // An exact retry key wins: the stored response is authoritative even
    // though the mutation revision advanced when the first attempt committed.
    if let Some(response) = replay_idempotent(session, idempotency_key)? {
        return Ok(Some(response));
    }
    check_expected_revision(session, expected_revision, expected_mutation_revision)?;
    Ok(None)
}

/// Return the stored response when this exact mutation key already ran.
fn replay_idempotent(
    session: &ChronoPlaytestSession,
    key: Option<&str>,
) -> Result<Option<Value>, ChronoPlaytestError> {
    let Some(key) = key.filter(|key| !key.is_empty()) else {
        return Ok(None);
    };
    if session.last_idempotency_key == key && !session.last_result_json.is_empty() {
        return from_json(&session.last_result_json).map(Some);
    }
    Ok(None)
}

// --- Service operations ----------------------------------------------------

impl StudioService {
    pub async fn create_chrono_playtest(
        &self,
        request: &CreateChronoPlaytestRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        // Read the interaction context before taking the service lock.
        let interaction = self
            .get_interaction_context()
            .await
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let session_id = request.session_id.as_ref().unwrap().trim().to_string();
        if inner.chrono_sessions.contains_key(&session_id)
            || inner
                .store
                .get_chrono_playtest_row(&session_id)
                .map_err(persistence_error)?
                .is_some()
        {
            return Err(ChronoPlaytestError::Conflict(format!(
                "chrono playtest session '{session_id}' already exists"
            )));
        }

        let content = read_content(&inner.store)?;
        let meta = inner
            .store
            .get_project_metadata()
            .map_err(persistence_error)?;
        if content.storylets.is_empty() {
            return Err(ChronoPlaytestError::InvalidInput(
                "Missing storylets: the project has no storylets to compile".into(),
            ));
        }
        let mut compiled = compile_chrono_dispatch(
            &content.storylets,
            &content.qualities,
            &content.locations,
            &content.characters,
        )
        .map_err(compile_error)?;

        // Scenario: explicit request, otherwise the project's first scenario.
        let (scenario, scenario_source) = match &request.scenario_id {
            Some(id) => {
                let scenario = content
                    .scenarios
                    .iter()
                    .find(|scenario| scenario.id == *id)
                    .ok_or_else(|| {
                        ChronoPlaytestError::InvalidInput(format!("Missing scenario '{id}'"))
                    })?;
                (scenario.clone(), "request")
            }
            None => {
                let scenario = content.scenarios.first().ok_or_else(|| {
                    ChronoPlaytestError::InvalidInput(
                        "Missing scenario: the project has no scenarios".into(),
                    )
                })?;
                (scenario.clone(), "firstScenario")
            }
        };

        let resolved = resolve_scenario_start(
            &scenario,
            &content.qualities,
            &content.locations,
            &content.characters,
        );
        if !resolved.issues.is_empty() {
            return Err(ChronoPlaytestError::InvalidInput(
                resolved.issues.join("; "),
            ));
        }

        // Start location: explicit request, then the current interaction context,
        // then the scenario's start policy.
        let known = |id: &str| content.locations.iter().any(|location| location.id == id);
        let (start_location, start_source) = if let Some(id) = &request.start_location_id {
            if !known(id) {
                return Err(ChronoPlaytestError::InvalidInput(format!(
                    "Missing location '{id}'"
                )));
            }
            (id.trim().to_string(), "request")
        } else {
            let context_location = interaction["context"]["playtest"]["locationId"]
                .as_str()
                .filter(|id| known(id));
            if let Some(id) = context_location {
                (id.to_string(), "interactionContext")
            } else {
                (
                    resolved.player_start_location.clone(),
                    "scenarioStartPolicy",
                )
            }
        };

        let seed_value = request.seed.unwrap_or(resolved.seed);
        let seed = initialize_chrono_world(
            &compiled,
            &content.qualities,
            &content.locations,
            &content.characters,
            Some(&scenario),
            &start_location,
        )
        .map_err(compile_error)?;
        let current_id_space = CompiledIdSpace::fingerprint(&compiled, &seed.entity_indices);
        let graphs = compile_narrative_graphs(
            &meta.id,
            &content.storylets,
            &compiled,
            &seed.entity_indices,
        )
        .map_err(compile_error)?;
        bind_storylet_start_nodes(&mut compiled, &graphs).map_err(compile_error)?;

        // Deterministic starting storylet: explicit request, otherwise the
        // lexicographically smallest storylet whose location tags name the start
        // location, otherwise the smallest storylet id.
        let (start_storylet, storylet_source) = match &request.storylet_id {
            Some(id) => {
                let id = id.trim();
                if graphs.storylet_start_nodes.contains_key(id) {
                    (id.to_string(), "request")
                } else {
                    return Err(ChronoPlaytestError::InvalidInput(format!(
                        "storyletId '{id}' has no compiled narrative graph"
                    )));
                }
            }
            None => {
                let mut candidates = content
                    .storylets
                    .iter()
                    .filter(|storylet| graphs.storylet_start_nodes.contains_key(&storylet.id))
                    .filter(|storylet| {
                        storylet
                            .location_tags
                            .iter()
                            .any(|tag| tag == &start_location)
                    })
                    .map(|storylet| storylet.id.clone())
                    .collect::<Vec<_>>();
                let source = if candidates.is_empty() {
                    candidates = graphs.storylet_start_nodes.keys().cloned().collect();
                    "firstAvailable"
                } else {
                    "locationTags"
                };
                candidates.sort();
                let storylet = candidates.into_iter().next().ok_or_else(|| {
                    ChronoPlaytestError::InvalidInput(
                        "No storylet has a compiled narrative graph".into(),
                    )
                })?;
                (storylet, source)
            }
        };
        let start_node_guid = graphs
            .storylet_start_nodes
            .get(&start_storylet)
            .copied()
            .ok_or_else(|| {
                ChronoPlaytestError::InvalidInput(format!(
                    "storylet '{start_storylet}' has no compiled narrative graph"
                ))
            })?;

        let now = chrono_now_ms();
        let mut instance = FlowGraphInstance::new(
            deterministic_instance_id(&session_id),
            graphs.asset.clone(),
            seed_value,
        )
        .map_err(runtime_error)?;
        bind_catalog_keys(&mut instance, &compiled);
        instance
            .attach_mission_assets(&graphs.mission_assets, seed_value)
            .map_err(runtime_error)?;
        let mut session = ChronoPlaytestSession {
            session_id: session_id.clone(),
            project_id: meta.id.clone(),
            pinned_project_revision: meta.revision,
            scenario_id: scenario.id.clone(),
            start_location_id: start_location.clone(),
            start_storylet_id: start_storylet.clone(),
            start_node_guid,
            seed: seed_value,
            id_space: current_id_space.clone(),
            instance,
            world: seed.state,
            reconstructed: false,
            last_idempotency_key: String::new(),
            last_result_json: String::new(),
            mutation_revision: 0,
            created_at: now,
            updated_at: now,
        };

        let prepared = Prepared {
            current_revision: meta.revision,
            content,
            compiled,
            current_id_space,
            rebased: false,
        };

        // `start_at_node` only enqueues the entry token; execution happens on the
        // first tick.
        let pending = session
            .instance
            .start_at_node(start_node_guid)
            .map_err(|error| {
                ChronoPlaytestError::Runtime(format!("start node {start_node_guid}: {error}"))
            })?;

        let result = json!({
            "scenarioSource": scenario_source,
            "startSource": start_source,
            "storyletSource": storylet_source,
            "pendingTokens": tokens_value(&session.instance, &pending)
        });
        finish(&mut inner, &mut session, &prepared, "create", result, None)
    }

    pub async fn list_chrono_playtests(&self) -> Result<Value, ChronoPlaytestError> {
        let inner = self.inner.lock().await;
        let meta = inner
            .store
            .get_project_metadata()
            .map_err(persistence_error)?;
        let rows = inner
            .store
            .list_chrono_playtest_rows()
            .map_err(persistence_error)?;
        let sessions = rows
            .iter()
            .map(|row| {
                let global: Vec<CallHistoryEntry> =
                    from_json(&row.global_call_history_json).unwrap_or_default();
                let scoped: BTreeMap<u32, Vec<CallHistoryEntry>> =
                    from_json(&row.scoped_call_histories_json).unwrap_or_default();
                json!({
                    "sessionId": row.session_id,
                    "projectId": row.project_id,
                    "pinnedProjectRevision": row.pinned_project_revision,
                    "currentProjectRevision": meta.revision,
                    "scenarioId": row.scenario_id,
                    "startLocationId": row.start_location_id,
                    "startStoryletId": row.start_storylet_id,
                    "startNodeGuid": row.start_node_guid,
                    "seed": row.seed,
                    "createdAt": row.created_at,
                    "updatedAt": row.updated_at,
                    "live": inner.chrono_sessions.contains_key(&row.session_id),
                    "globalHistoryLength": global.len(),
                    "scopedHistoryLengths": scoped
                        .iter()
                        .map(|(guid, entries)| (guid.to_string(), entries.len()))
                        .collect::<BTreeMap<_, _>>()
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "operation": "list", "sessions": sessions }))
    }

    pub async fn chrono_playtest_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Value, ChronoPlaytestError> {
        let inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        let trigger_events = trigger_events_value(
            session.instance.drain_passion_threshold_events(),
            &session.id_space,
        );
        Ok(payload(
            &session,
            &prepared,
            "snapshot",
            Value::Null,
            trigger_events,
        ))
    }

    pub async fn chrono_playtest_tick(
        &self,
        session_id: &str,
        request: &ChronoPlaytestTickRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let budget = request.frame_budget_ms.unwrap();
        let before = session.instance.global_call_history.len();
        let tokens = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .tick(&mut ctx, budget)
                .map_err(runtime_error)?
        };
        let recorded = session.instance.global_call_history[before..].to_vec();
        let result = json!({
            "frameBudgetMs": budget,
            "recorded": call_history_value(&recorded),
            "tokens": tokens_value(&session.instance, &tokens)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "tick",
            result,
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_signal(
        &self,
        session_id: &str,
        request: &ChronoPlaytestSignalRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let event_name = request.event_name.as_deref().unwrap();
        let before = session.instance.global_call_history.len();
        let tokens = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .signal_event(&mut ctx, event_name)
                .map_err(runtime_error)?
        };
        let recorded = session.instance.global_call_history[before..].to_vec();
        let result = json!({
            "eventName": event_name,
            "recorded": call_history_value(&recorded),
            "tokens": tokens_value(&session.instance, &tokens)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "signal",
            result,
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_choose(
        &self,
        session_id: &str,
        request: &ChronoPlaytestChooseRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let guid = request.node_guid.unwrap();
        let before = session.instance.global_call_history.len();
        let tokens = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            if let Some(pin) = &request.output_pin {
                session
                    .instance
                    .choose(&mut ctx, guid, pin)
                    .map_err(runtime_error)?
            } else {
                let storylet_id = request.storylet_id.as_ref().unwrap().trim();
                let index = prepared
                    .current_id_space
                    .storylets
                    .get(storylet_id)
                    .ok_or_else(|| {
                        ChronoPlaytestError::InvalidInput(format!(
                            "storyletId '{storylet_id}' is not in the compiled dispatch"
                        ))
                    })?;
                session
                    .instance
                    .select_storylet(&mut ctx, guid, *index)
                    .map_err(runtime_error)?
            }
        };
        let recorded = session.instance.global_call_history[before..].to_vec();
        let result = json!({
            "nodeGuid": guid,
            "recorded": call_history_value(&recorded),
            "tokens": tokens_value(&session.instance, &tokens)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "choose",
            result,
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_divert(
        &self,
        session_id: &str,
        request: &ChronoPlaytestDivertRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let node_guid = request.node_guid.unwrap();
        let edge_id = request.edge_id.as_deref().unwrap().trim();
        let before = session.instance.global_call_history.len();
        let tokens = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .divert_topic(&mut ctx, node_guid, edge_id)
                .map_err(runtime_error)?
        };
        let recorded = session.instance.global_call_history[before..].to_vec();
        let token_values = tokens_value(&session.instance, &tokens);
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "divert",
            json!({
                "nodeGuid": node_guid,
                "edgeId": edge_id,
                "recorded": call_history_value(&recorded),
                "tokens": token_values
            }),
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_scrub(
        &self,
        session_id: &str,
        request: &ChronoPlaytestScrubRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request.validate().map_err(ChronoPlaytestError::InvalidInput)?;
        let inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        check_expected_revision(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
        )?;
        let node_guid = request.node_guid.unwrap();
        let delta_ms = request.delta_ms.unwrap();
        // Scrubbing is a preview: composer poses and the scene clock are not
        // part of the persisted history, so applying it live would be lost on
        // reload. Run it on a clone and leave the session untouched.
        let mut preview = session.instance.clone();
        let conflicts_before = preview.composer.conflicts.len();
        let parts = preview
            .preview_time_window(node_guid, delta_ms)
            .map_err(runtime_error)?;
        let conflicts = preview.composer.conflicts[conflicts_before..].to_vec();
        let result = json!({
            "nodeGuid": node_guid,
            "deltaMs": delta_ms,
            "persisted": false,
            "parts": parts,
            "conflicts": conflicts
        });
        Ok(payload(&session, &prepared, "scrub", result, Vec::new()))
    }

    pub async fn chrono_playtest_rollback_mission(
        &self,
        session_id: &str,
        request: &ChronoPlaytestRollbackRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request.validate().map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let node_guid = request.node_guid.unwrap();
        {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .rollback_mission_to_start(&mut ctx, node_guid)
                .map_err(runtime_error)?;
        }
        let result = json!({
            "nodeGuid": node_guid,
            "recorded": call_history_value(&session.instance.global_call_history)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "rollback",
            result,
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_invoke(
        &self,
        session_id: &str,
        request: &ChronoPlaytestInvokeRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let guid = request.node_guid.unwrap();
        let routes = request
            .declared_routes
            .iter()
            .map(|(key, pin)| (key.parse::<u32>().unwrap(), pin.clone()))
            .collect::<BTreeMap<u32, String>>();
        let executed = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .invoke_story_to_poi(&mut ctx, guid, &routes)
                .map_err(runtime_error)?
        };
        let result = json!({
            "nodeGuid": guid,
            "declaredRoutes": request.declared_routes,
            "executed": call_history_value(&executed),
            "tokens": tokens_value(&session.instance, &session.instance.tokens)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "invoke",
            result,
            request.idempotency_key.as_deref(),
        )
    }

    pub async fn chrono_playtest_reconcile(
        &self,
        session_id: &str,
        request: &ChronoPlaytestReconcileRequest,
    ) -> Result<Value, ChronoPlaytestError> {
        request
            .validate()
            .map_err(ChronoPlaytestError::InvalidInput)?;
        let mut inner = self.inner.lock().await;
        let mut session = load_session(&inner, session_id)?;
        let prepared = prepare(&inner, &mut session)?;
        if let Some(response) = begin_mutation(
            &session,
            request.expected_revision,
            request.expected_mutation_revision,
            request.idempotency_key.as_deref(),
        )? {
            return Ok(response);
        }
        let head = {
            let mut ctx = RuntimeContext::with_registries(
                &mut session.world,
                &prepared.compiled.dispatcher,
                &prepared.compiled.registry,
                &prepared.compiled.passion_registry,
            );
            session
                .instance
                .reconcile_history(&mut ctx, &request.history)
                .map_err(runtime_error)?
        };
        let result = json!({
            "head": head,
            "entries": request.history.len(),
            "tokens": tokens_value(&session.instance, &session.instance.tokens)
        });
        finish(
            &mut inner,
            &mut session,
            &prepared,
            "reconcile",
            result,
            request.idempotency_key.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storylet_engine::chrono_runtime::{
        FlowNodeDef, FlowNodeInstance, NodeCallData, NodeCategory, PinDefinition, TOKEN_ACTIVE,
    };
    use storylet_engine::chrono_state::{BijectiveDelta, PassionDelta, PassionThresholdEvent};

    #[test]
    fn threshold_transition_projection_emits_activate_and_cancel_asset_actions() {
        let transition = |activated| NodeCallData::Delta {
            delta: BijectiveDelta {
                passion_delta: Some(PassionDelta {
                    threshold_events: vec![PassionThresholdEvent {
                        npc_id: 0,
                        threshold_tag_bit: 2,
                        linked_graph_asset_id: "storylet:lego".into(),
                        activated,
                    }],
                    ..PassionDelta::default()
                }),
                ..BijectiveDelta::default()
            },
        };
        let instance = FlowGraphInstance {
            nodes: vec![FlowNodeInstance {
                node_guid: 91,
                call_data: vec![transition(true), transition(false)],
                ..FlowNodeInstance::default()
            }],
            ..FlowGraphInstance::default()
        };
        let ids = CompiledIdSpace {
            entities: BTreeMap::from([("character:sera".into(), 0)]),
            tags: BTreeMap::from([("passion:trust".into(), 2)]),
            ..CompiledIdSpace::default()
        };
        let events = passion_transition_events(&instance, &ReverseIndex::build(&ids));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["action"], "activate");
        assert_eq!(events[0]["linkedGraphAssetId"], "storylet:lego");
        assert_eq!(events[0]["characterId"], "character:sera");
        assert_eq!(events[1]["action"], "cancel");
    }
    #[test]
    fn runtime_passion_trigger_events_map_to_host_actions() {
        let ids = CompiledIdSpace {
            entities: BTreeMap::from([("character:sera".into(), 4)]),
            tags: BTreeMap::from([("passion:trust".into(), 2)]),
            ..CompiledIdSpace::default()
        };
        let events = trigger_events_value(
            vec![PassionThresholdTriggerEvent {
                node_guid: 91,
                npc_id: 4,
                threshold_tag_bit: 2,
                linked_graph_asset_id: "storylet:lego".into(),
                activated: false,
            }],
            &ids,
        );

        assert_eq!(events[0]["eventType"], "narrative_graph_trigger");
        assert_eq!(events[0]["action"], "cancel");
        assert_eq!(events[0]["graphAssetId"], "storylet:lego");
        assert_eq!(events[0]["characterId"], "character:sera");
        assert_eq!(events[0]["thresholdTag"], "passion:trust");
        assert_eq!(events[0]["nodeGuid"], 91);
    }

    fn tiny_asset(asset_id: &str) -> FlowGraphAsset {
        FlowGraphAsset {
            asset_id: asset_id.to_owned(),
            start_node_guid: 1,
            nodes: vec![FlowNodeDef {
                node_guid: 1,
                node_type_id: "Mutation".into(),
                category: NodeCategory::Mutator,
                kind: NodeKind::Mutation {
                    op: Default::default(),
                },
                input_pins: vec![PinDefinition {
                    pin_name: "In".into(),
                    is_input: true,
                }],
                output_pins: vec![PinDefinition {
                    pin_name: "Out".into(),
                    is_input: false,
                }],
                ..FlowNodeDef::default()
            }],
            edges: Vec::new(),
        }
    }

    #[test]
    fn mission_assets_are_mapped_by_mission_node() {
        let child = tiny_asset("child");
        let mut root = tiny_asset("root");
        root.nodes.push(FlowNodeDef {
            node_guid: 7,
            node_type_id: "MissionSubGraph".into(),
            category: NodeCategory::ScopedContainer,
            kind: NodeKind::MissionSubGraph {
                sub_graph_asset_id: "child".into(),
            },
            input_pins: vec![PinDefinition {
                pin_name: "In".into(),
                is_input: true,
            }],
            output_pins: vec![PinDefinition {
                pin_name: "Out".into(),
                is_input: false,
            }],
            ..FlowNodeDef::default()
        });
        let map = mission_asset_map(
            &root,
            &BTreeMap::from([("child".to_owned(), child.clone())]),
        )
        .unwrap();
        assert_eq!(map.get(&7).map(|asset| asset.asset_id.as_str()), Some("child"));
    }

    #[test]
    fn ideal_state_weights_reach_the_dispatcher() {
        let quality: QualityRecord = serde_json::from_value(json!({
            "name": "pressure",
            "type": "metric",
            "value": 0.0
        }))
        .unwrap();
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "weighted",
            "title": "Weighted",
            "content": { "text_template": "" },
            "created_at": 0,
            "chrono": {
                "ideal_state_vector": { "pressure": 10.0 },
                "ideal_state_weights": { "pressure": 4.0 }
            }
        }))
        .unwrap();

        let compiled = crate::chrono_adapter::compile_chrono_dispatch(
            &[storylet],
            &[quality],
            &[],
            &[],
        )
        .unwrap();

        assert_eq!(compiled.dispatcher.storylets[0].ideal_vector.len(), 1);
        assert_eq!(compiled.dispatcher.storylets[0].ideal_vector[0].weight, 4.0);
    }

    #[test]
    fn chrono_requests_reject_unknown_fields() {
        let valid = parse_request::<ChronoPlaytestTickRequest>(&json!({
            "operation": "chrono.playtest.tick",
            "id": "session-a",
            "frameBudgetMs": 16,
            "expectedMutationRevision": 2
        }))
        .unwrap();
        assert_eq!(valid.frame_budget_ms, Some(16));
        assert_eq!(valid.expected_mutation_revision, Some(2));

        let misspelled = parse_request::<ChronoPlaytestTickRequest>(&json!({
            "operation": "chrono.playtest.tick",
            "id": "session-a",
            "frameBudgetMs": 16,
            "expectedMutationRevsion": 2
        }));
        assert!(misspelled.is_err());
    }

    #[test]
    fn idempotent_retries_return_the_original_payload() {
        let instance = FlowGraphInstance::new(1, tiny_asset("s"), 1).unwrap();
        let session = ChronoPlaytestSession {
            session_id: "s".into(),
            project_id: "p".into(),
            pinned_project_revision: 3,
            scenario_id: "sc".into(),
            start_location_id: "l".into(),
            start_storylet_id: "st".into(),
            start_node_guid: 1,
            seed: 1,
            id_space: CompiledIdSpace::default(),
            instance,
            world: WorldStateTable::new(),
            reconstructed: false,
            last_idempotency_key: "k".into(),
            last_result_json: "{\"ok\":true}".into(),
            mutation_revision: 0,
            created_at: 0,
            updated_at: 0,
        };

        let replay = begin_mutation(&session, Some(3), Some(0), Some("k")).unwrap();
        assert_eq!(replay.unwrap()["ok"], true);
        // The revision advanced on the first attempt; an exact retry still
        // returns the stored payload.
        assert!(begin_mutation(&session, Some(3), Some(1), Some("k"))
            .unwrap()
            .is_some());
        assert!(begin_mutation(&session, None, None, Some("other"))
            .unwrap()
            .is_none());
        assert!(begin_mutation(&session, Some(2), None, None).is_err());
        // A stale session mutation revision is rejected even when the project
        // revision matches.
        assert!(begin_mutation(&session, Some(3), Some(1), None).is_err());
    }

    #[test]
    fn pending_latent_interaction_round_trips_and_restores() {
        let graph = FlowGraphAsset {
            asset_id: "pending-state".into(),
            start_node_guid: 1,
            nodes: vec![FlowNodeDef {
                node_guid: 1,
                node_type_id: "ChoiceNode".into(),
                category: NodeCategory::DynamicDispatcher,
                kind: NodeKind::Choice {
                    default_output_pin: "choice_a".into(),
                },
                input_pins: vec![
                    PinDefinition {
                        pin_name: "In_0".into(),
                        is_input: true,
                    },
                    PinDefinition {
                        pin_name: "In_1".into(),
                        is_input: true,
                    },
                ],
                output_pins: vec![PinDefinition {
                    pin_name: "choice_a".into(),
                    is_input: false,
                }],
                ..FlowNodeDef::default()
            }],
            edges: vec![],
        };
        let mut instance = FlowGraphInstance::new(42, graph, 7).unwrap();
        let token = PersistedToken {
            token_id: 3,
            graph_instance_id: 42,
            target_node_guid: 1,
            target_pin_name: "In_1".into(),
            time_budget_ms: 8,
            is_active: true,
            deferred_until_ms: 0,
            is_deferred: false,
        };
        let pending = PendingState {
            nodes: vec![PendingNode {
                node_guid: 1,
                executed_input_pin: Some("In_1".into()),
                chosen_output_pin: None,
                chosen_storylet_id: None,
                section_clock_ms: 0,
                waiter_wake_ms: 0,
                current_topic_id: String::new(),
                arrived_input_pins: BTreeSet::new(),
                call_data: vec![NodeCallData::Waiter { wake_ms: 12 }],
            }],
            tokens: vec![token.clone()],
            clock_ms: 12,
            turn: 2,
            actor_pose: BTreeMap::from([("actor".to_owned(), "pose.a".to_owned())]),
            ..PendingState::default()
        };
        let encoded = serde_json::to_string(&pending).unwrap();
        let decoded: PendingState = serde_json::from_str(&encoded).unwrap();

        restore_pending(&mut instance, &decoded, PinMismatch::Error).unwrap();

        assert_eq!(
            instance.node(1).unwrap().state,
            NodeExecutionState::ActiveLatent
        );
        assert_eq!(
            instance.tokens,
            vec![SignalToken {
                token_id: 3,
                graph_instance_id: 42,
                target_node_guid: 1,
                target_pin_index: 1,
                flags: TOKEN_ACTIVE,
                time_budget_ms: 8,
                deferred_until_ms: 0,
            }]
        );
        assert_eq!(instance.next_token_id, 4);
        assert_eq!(instance.clock_ms, 12);
        assert_eq!(instance.turn, 2);
        assert_eq!(
            instance.composer.actor_pose.get("actor").map(String::as_str),
            Some("pose.a")
        );
        assert!(matches!(
            instance.node(1).unwrap().call_data.as_slice(),
            [NodeCallData::Waiter { .. }]
        ));
    }

    #[test]
    fn stale_pending_token_is_dropped_during_rebase() {
        let graph = FlowGraphAsset {
            asset_id: "renamed-pins".into(),
            start_node_guid: 1,
            nodes: vec![FlowNodeDef {
                node_guid: 1,
                node_type_id: "Mutation".into(),
                category: NodeCategory::Mutator,
                kind: NodeKind::Mutation {
                    op: Default::default(),
                },
                input_pins: vec![
                    PinDefinition {
                        pin_name: "In_0".into(),
                        is_input: true,
                    },
                    PinDefinition {
                        pin_name: "In_1".into(),
                        is_input: true,
                    },
                ],
                output_pins: vec![PinDefinition {
                    pin_name: "Out".into(),
                    is_input: false,
                }],
                ..FlowNodeDef::default()
            }],
            edges: vec![],
        };
        let mut instance = FlowGraphInstance::new(1, graph, 1).unwrap();
        let pending = PendingState {
            tokens: vec![PersistedToken {
                token_id: 1,
                graph_instance_id: 1,
                target_node_guid: 1,
                target_pin_name: "In".into(),
                time_budget_ms: 16,
                is_active: true,
                deferred_until_ms: 0,
                is_deferred: false,
            }],
            ..PendingState::default()
        };

        restore_pending(&mut instance, &pending, PinMismatch::Drop).unwrap();

        assert!(instance.tokens.is_empty());
    }

    #[test]
    fn token_u64_identifiers_are_strings_in_the_api_payload() {
        let instance = FlowGraphInstance::new(1, tiny_asset("tokens"), 1).unwrap();
        let tokens = [SignalToken {
            token_id: 7,
            graph_instance_id: 8_070_450_532_247_929_000,
            target_node_guid: 1,
            target_pin_index: 0,
            flags: TOKEN_ACTIVE,
            time_budget_ms: 16,
            deferred_until_ms: 0,
        }];

        let value = tokens_value(&instance, &tokens);
        assert_eq!(value[0]["target_pin_name"], "In");

        assert_eq!(value[0]["token_id"], "7");
        assert_eq!(value[0]["graph_instance_id"], "8070450532247929000");
    }
}
