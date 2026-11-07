//! Application service coordinator for Storylet Studio.

use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

use crate::chrono_playtest::ChronoPlaytestSession;
use storylet_engine::{
    EngineError, ProjectBundle as EngineProjectBundle, RuntimeError, RuntimeSessionState,
    StoryletRuntime, WorldCommand, WorldEvent,
};

#[derive(Debug)]
pub enum ChoiceOperationError {
    InvalidInput(String),
    SessionNotFound(String),
    NotFound(String),
    Conflict(String),
    Runtime(String),
    Persistence(String),
}

impl std::fmt::Display for ChoiceOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::Conflict(message)
            | Self::Runtime(message)
            | Self::Persistence(message) => formatter.write_str(message),
            Self::SessionNotFound(session_id) => {
                write!(formatter, "session '{session_id}' was not found")
            }
            Self::NotFound(message) => formatter.write_str(message),
        }
    }
}

#[derive(Debug)]
pub enum ContentDeleteError {
    Conflict(String),
    Persistence(String),
}

impl std::fmt::Display for ContentDeleteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict(message) | Self::Persistence(message) => formatter.write_str(message),
        }
    }
}

use crate::chrono_adapter::compile_chrono_dispatch_for_storylet_validation;
use crate::contracts::{
    AssetRecord, AuthoringOptions, CharacterRecord, ContentReference, EntityRecord,
    EntityTemplateRecord, FolderRecord, GameplayTagRecord, LibraryPageResult, LibraryQuery,
    ProjectBundle, ProjectBundleContent, ProjectMetadata, ScenarioRecord, StoryletRecord,
    TraitTemplateRecord, CONTENT_KINDS, FOLDER_ROOTS, PROJECT_BUNDLE_SCHEMA_VERSION,
};
use crate::entities::{self, WorkspaceIndex};
use crate::narrative_graph::NarrativeGraph;
use crate::projection::project_workspace;
use crate::references::{self, ReferenceContext};
use crate::scenario::resolve_scenario_start;
use crate::store::Store;

fn persisted_runtime(store: &Store, templates: &[TraitTemplateRecord], entity_templates: &[EntityTemplateRecord]) -> Result<entities::RuntimeRecords, String> {
    let world = store.list_entities().map_err(|e| e.to_string())?
        .into_iter().find(|entity| entity.id == entities::WORLD_ENTITY_ID)
        .ok_or_else(|| "Intrinsic World entity is missing from persisted entities".to_string())?;
    Ok(entities::runtime_records_with_world(&world, entity_templates, templates))
}

#[derive(Clone)]
pub struct StudioService {
    pub(crate) inner: Arc<Mutex<StudioInner>>,
    pub updates: broadcast::Sender<SessionChange>,
    pub resource_root: PathBuf,
    pub(crate) coverage_running: Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) struct StudioInner {
    pub(crate) store: Store,
    runtime: StoryletRuntime,
    /// Live ChronoFlow v5 playtest sessions (REQ-CH-02). The persisted rows are
    /// the source of truth; these are the runtime instances rebuilt from them.
    pub(crate) chrono_sessions: BTreeMap<String, ChronoPlaytestSession>,
    pub(crate) runs: BTreeMap<String, crate::runs::CachedRun>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionChange {
    pub session_id: String,
    pub revision: u64,
    pub snapshot: Value,
}

fn validate_and_normalize_bundle(bundle: &mut ProjectBundle) -> Vec<String> {
    let mut issues = Vec::new();
    for storylet in &mut bundle.content.storylets {
        let graph = match storylet.graph.clone() {
            Some(graph) => graph,
            None => NarrativeGraph::from_storylet(storylet),
        };
        match graph.validate() {
            Ok(()) => storylet.ending = graph.ending_from_graph(),
            Err(errors) => issues.push(format!(
                "storylet {} graph is invalid: {}",
                storylet.id,
                errors.join("; ")
            )),
        }
    }
    issues
}

fn normalize_storylet_ending(value: &mut Value) {
    let Some(graph) = value.get("graph").cloned() else {
        return;
    };
    let Ok(graph) = serde_json::from_value::<NarrativeGraph>(graph) else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("ending".into(), Value::Bool(graph.ending_from_graph()));
    }
}

fn shared_coverage_guard(store: &Store) -> Arc<std::sync::atomic::AtomicBool> {
    use std::sync::{Mutex as StdMutex, OnceLock, Weak};
    static GUARDS: OnceLock<StdMutex<BTreeMap<PathBuf, Weak<std::sync::atomic::AtomicBool>>>> = OnceLock::new();
    let Some(path) = store.conn.path().filter(|path| !path.is_empty() && *path != ":memory:") else {
        return Arc::new(std::sync::atomic::AtomicBool::new(false));
    };
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let mut guards = GUARDS.get_or_init(|| StdMutex::new(BTreeMap::new())).lock().unwrap_or_else(|error| error.into_inner());
    guards.retain(|_, guard| guard.strong_count() > 0);
    if let Some(guard) = guards.get(&path).and_then(Weak::upgrade) { return guard; }
    let guard = Arc::new(std::sync::atomic::AtomicBool::new(false));
    guards.insert(path, Arc::downgrade(&guard));
    guard
}
impl StudioService {
    pub fn open(
        db_path: impl AsRef<Path>,
        resource_root: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let store = Store::open(db_path).map_err(|e| e.to_string())?;
        Self::build(store, resource_root)
    }

    pub fn open_in_memory(resource_root: impl Into<PathBuf>) -> Result<Self, String> {
        let store = Store::open_in_memory().map_err(|e| e.to_string())?;
        Self::build(store, resource_root)
    }

    pub fn from_connection(conn: rusqlite::Connection) -> Result<Self, String> {
        let store = Store::from_connection(conn).map_err(|e| e.to_string())?;
        Self::build(store, ".")
    }

    fn build(store: Store, resource_root: impl Into<PathBuf>) -> Result<Self, String> {
        let coverage_running = shared_coverage_guard(&store);
        let mut runtime = StoryletRuntime::default();
        let saved_sessions = store.list_runtime_sessions().map_err(|e| e.to_string())?;
        for state in saved_sessions {
            let session_id = state.session_id.clone();
            validate_event_history(&store.conn, &state)
                .map_err(|error| format!("saved session '{session_id}' event history is invalid: {error}"))?;
            runtime
                .restore_session(state)
                .map_err(|error| format!("saved session '{session_id}' cannot be restored: {error}"))?;
        }

        let (updates, _) = broadcast::channel(256);
        let res_root = resource_root.into();

        Ok(Self {
            inner: Arc::new(Mutex::new(StudioInner {
                store,
                runtime,
                chrono_sessions: BTreeMap::new(),
                runs: BTreeMap::new(),
            })),
            updates,
            resource_root: res_root,
            coverage_running,
        })
    }

    // --- Project Settings & Bundles ---

    pub async fn get_project_metadata(&self) -> Result<ProjectMetadata, String> {
        let inner = self.inner.lock().await;
        inner
            .store
            .get_project_metadata()
            .map_err(|e| e.to_string())
    }

    pub async fn update_project_name(&self, name: &str) -> Result<ProjectMetadata, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Project name cannot be empty".into());
        }
        let inner = self.inner.lock().await;
        let mut meta = inner
            .store
            .get_project_metadata()
            .map_err(|e| e.to_string())?;
        meta.name = trimmed.to_string();
        meta.revision += 1;
        inner
            .store
            .save_project_metadata(&meta)
            .map_err(|e| e.to_string())?;
        Ok(meta)
    }

    pub async fn export_playtest_session(
        &self,
        session_id: &str,
    ) -> Result<RuntimeSessionState, String> {
        let inner = self.inner.lock().await;
        inner
            .runtime
            .export_session(session_id)
            .map_err(|e| e.to_string())
    }

    pub async fn restore_playtest_session(
        &self,
        state: RuntimeSessionState,
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;
        let session_id = state.session_id.clone();
        let snapshot = inner
            .runtime
            .restore_session(state)
            .map_err(|e| e.to_string())?;
        persist_session_to_store(&mut inner, &session_id)?;

        let snap_val = serde_json::to_value(&snapshot).map_err(|e| e.to_string())?;
        let _ = self.updates.send(SessionChange {
            session_id: session_id.clone(),
            revision: snapshot.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
        });

        Ok(json!({ "snapshot": snap_val }))
    }

    pub async fn export_project_bundle(&self) -> Result<ProjectBundle, String> {
        let inner = self.inner.lock().await;
        let project = inner
            .store
            .get_project_metadata()
            .map_err(|e| e.to_string())?;

        let entities = inner.store.list_content_json("entities").map_err(|e| e.to_string())?
            .into_iter().filter_map(|s| serde_json::from_str::<EntityRecord>(&s).ok()).collect();
        let entity_templates = inner.store.list_entity_templates().map_err(|e| e.to_string())?;
        let gameplay_tags = inner.store.list_gameplay_tags().map_err(|e| e.to_string())?;
        let folders = inner.store.list_folders().map_err(|e| e.to_string())?;
        let trait_templates = inner
            .store
            .list_trait_templates()
            .map_err(|e| e.to_string())?;
        let assets = inner.store.list_assets().map_err(|e| e.to_string())?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let events = inner.store.list_events().map_err(|e| e.to_string())?;
        let scenarios = inner.store.list_scenarios().map_err(|e| e.to_string())?;
        Ok(ProjectBundle {
            format: "decision-narrative-project".into(),
            schema_version: PROJECT_BUNDLE_SCHEMA_VERSION,
            project,
            content: ProjectBundleContent {
                entities,
                entity_templates,
                gameplay_tags,
                folders,
                trait_templates,
                assets,
                storylets,
                events,
                scenarios,
            },
        })
    }

    pub async fn validate_bundle(&self, bundle_val: &Value) -> Result<Vec<String>, String> {
        let mut issues = Vec::new();

        let mut bundle: ProjectBundle = match entities::parse_project_bundle(bundle_val) {
            Ok(bundle) => bundle,
            Err(error) => {
                issues.push(error);
                return Ok(issues);
            }
        };
        issues.extend(validate_and_normalize_bundle(&mut bundle));

        if bundle.format != "decision-narrative-project" {
            issues.push(format!("Unsupported format '{}'", bundle.format));
        }
        issues.extend(entities::validate_bundle_content(&bundle.content));

        let Some(world) = bundle.content.entities.iter().find(|entity| entity.id == "world") else {
            issues.sort();
            issues.dedup();
            return Ok(issues);
        };
        let runtime = entities::runtime_records_with_world(
            world,
            &bundle.content.entity_templates,
            &bundle.content.trait_templates,
        );
        let res = project_workspace(
            &bundle.project.id,
            &bundle.project.name,
            bundle.project.revision,
            &runtime.locations,
            &runtime.characters,
            &bundle.content.storylets,
            &bundle.content.events,
            bundle.content.scenarios.first(),
        );

        issues.extend(res.issues);
        issues.sort();
        issues.dedup();
        Ok(issues)
    }

    pub async fn import_bundle(&self, bundle_val: &Value) -> Result<u64, String> {
        let mut bundle: ProjectBundle = entities::parse_project_bundle(bundle_val)
            .map_err(|error| format!("Invalid bundle JSON: {error}"))?;
        let mut issues = validate_and_normalize_bundle(&mut bundle);
        issues.extend(entities::validate_bundle_content(&bundle.content));
        if !issues.is_empty() {
            return Err(issues.join("; "));
        }

        let mut inner = self.inner.lock().await;
        inner
            .store
            .replace_project_content(&bundle.project, &bundle.content)
            .map_err(|e| e.to_string())
    }

    // --- Content CRUD ---

    pub async fn list_content(&self, entity: &str) -> Result<Value, String> {
        ensure_content_kind(entity)?;
        let inner = self.inner.lock().await;
        if entity == "entities" {
            return serde_json::to_value(inner.store.list_entities().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string());
        }
        let list = inner
            .store
            .list_content_json(entity)
            .map_err(|e| e.to_string())?;
        let mut items: Vec<Value> = list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        if entity == "storylets" {
            for item in &mut items {
                normalize_storylet_ending(item);
            }
        }
        Ok(Value::Array(items))
    }

    pub async fn get_content_item(&self, entity: &str, id: &str) -> Result<Value, String> {
        ensure_content_kind(entity)?;
        let inner = self.inner.lock().await;
        if entity == "entities" {
            let record = inner.store.list_entities().map_err(|error| error.to_string())?
                .into_iter().find(|record| record.id == id)
                .ok_or_else(|| format!("entities item '{id}' not found"))?;
            return serde_json::to_value(record).map_err(|error| error.to_string());
        }
        let opt = inner
            .store
            .get_content_json(entity, id)
            .map_err(|e| e.to_string())?;
        let mut value: Option<Value> = opt.and_then(|s| serde_json::from_str(&s).ok());
        if entity == "storylets"
            && let Some(item) = value.as_mut()
        {
            normalize_storylet_ending(item);
        }
        value.ok_or_else(|| format!("{entity} item '{id}' not found"))
    }

    pub async fn save_content_item(&self, entity: &str, payload: &Value) -> Result<Value, String> {
        self.save_content_item_with_origin(entity, payload, "human").await
    }

    pub(crate) async fn save_content_item_with_origin(&self, entity: &str, payload: &Value, created_origin: &str) -> Result<Value, String> {
        ensure_content_kind(entity)?;
        let inner = self.inner.lock().await;
        let index = build_workspace_index(&inner.store)?;
        let stored_id = payload
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());

        let mut final_payload = match entity {
            "entities" => {
                let stored: Option<EntityRecord> =
                    load_stored_record(&inner.store, "entities", stored_id)?;
                let prepared = entities::prepare_entity_record(payload, stored.as_ref(), &index)?;
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            "entity_templates" => {
                let stored: Option<EntityTemplateRecord> =
                    load_stored_record(&inner.store, "entity_templates", stored_id)?;
                let prepared = entities::prepare_entity(payload, stored.as_ref(), &index)?;
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            "trait_templates" => {
                let stored: Option<TraitTemplateRecord> =
                    load_stored_record(&inner.store, "trait_templates", stored_id)?;
                if stored.as_ref().is_some_and(|template| template.system) {
                    return Err("System traits cannot be modified".into());
                }
                let prepared =
                    entities::prepare_trait_template(payload, stored.as_ref(), &index)?;
                if stored.as_ref().is_some_and(|previous| previous.unique != prepared.unique) {
                    let mut templates = inner.store.list_trait_templates().map_err(|error| error.to_string())?;
                    templates.retain(|template| template.id != prepared.id);
                    templates.push(prepared.clone());
                    for record in inner.store.list_entities().map_err(|error| error.to_string())? {
                        entities::validate_unique_traits(&record.traits, &templates, &format!("entity '{}'", record.name))?;
                    }
                    for record in inner.store.list_entity_templates().map_err(|error| error.to_string())? {
                        entities::validate_unique_traits(&record.traits, &templates, &format!("entity template '{}'", record.name))?;
                    }
                }
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            "gameplay_tags" => {
                let tag: GameplayTagRecord = serde_json::from_value(payload.clone())
                    .map_err(|error| format!("Gameplay tag validation failed: {error}"))?;
                let stored: Option<GameplayTagRecord> =
                    load_stored_record(&inner.store, "gameplay_tags", stored_id)?;
                let prepared = prepare_gameplay_tag(&inner.store, tag, stored.as_ref())?;
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            "assets" => {
                let stored: Option<AssetRecord> =
                    load_stored_record(&inner.store, "assets", stored_id)?;
                let prepared = entities::prepare_asset(payload, stored.as_ref(), &index)?;
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            "folders" => {
                let stored: Option<FolderRecord> =
                    load_stored_record(&inner.store, "folders", stored_id)?;
                let prepared = entities::prepare_folder(payload, stored.as_ref(), &index)?;
                serde_json::to_value(prepared).map_err(|e| e.to_string())?
            }
            _ => payload.clone(),
        };

        let id = final_payload
            .get("id")
            .and_then(Value::as_str)
            .ok_or("Item must have an 'id' field")?
            .to_string();

        if entity == "storylets" {
            if let Some(value) = final_payload.get("default_learn_visibility") {
                if !matches!(value.as_str(), Some("private" | "co_located")) {
                    return Err("Default Learn visibility must be private or co_located".into());
                }
            }
            let settings = match final_payload.get("chrono") {
                None | Some(Value::Null) => crate::contracts::ChronoStoryletSettings::default(),
                Some(value) => serde_json::from_value(value.clone()).map_err(|error| {
                    format!("Storylet Chrono settings validation failed: {error}")
                })?,
            };
            final_payload
                .as_object_mut()
                .ok_or("Storylet payload must be a JSON object")?
                .insert(
                    "chrono".into(),
                    serde_json::to_value(settings).map_err(|error| error.to_string())?,
                );
        }

        let (revision, approved_rev) = if entity == "storylets" {
            let next_rev = match inner.store.get_storylet(&id).map_err(|e| e.to_string())? {
                Some(previous) => previous.revision + 1,
                None => payload
                    .get("revision")
                    .and_then(Value::as_u64)
                    .unwrap_or(1),
            };
            let origin = inner.store.get_storylet(&id).map_err(|e| e.to_string())?.map(|story| story.origin).unwrap_or_else(|| created_origin.to_string());            if let Some(object) = final_payload.as_object_mut() {
                object.insert("origin".into(), Value::String(origin));
                object.insert("revision".into(), Value::from(next_rev));
                object.insert("approved_revision".into(), Value::Null);
            }
            (next_rev, None)
        } else {
            (
                payload
                    .get("revision")
                    .and_then(Value::as_u64)
                    .unwrap_or(1),
                payload.get("approved_revision").and_then(Value::as_u64),
            )
        };

        if entity == "entity_templates" {
            let mut templates = inner
                .store
                .list_entity_templates()
                .map_err(|error| error.to_string())?;
            let candidate: EntityTemplateRecord = serde_json::from_value(final_payload.clone())
                .map_err(|error| format!("Entity template validation failed: {error}"))?;
            templates.retain(|item| item.id != candidate.id);
            templates.push(candidate);
            let trait_templates = inner
                .store
                .list_trait_templates()
                .map_err(|error| error.to_string())?;
                let runtime = persisted_runtime(&inner.store, &trait_templates, &templates)?;
            for character in &runtime.characters {
                validate_character(character, &runtime.characters, |storylet_id| {
                    Ok(inner
                        .store
                        .get_content_json("storylets", storylet_id)
                        .map_err(|error| error.to_string())?
                        .is_some())
                })?;
            }
        }

        if entity == "storylets" {
            let mut storylet: StoryletRecord = serde_json::from_value(final_payload.clone())
                .map_err(|error| format!("Storylet validation failed: {error}"))?;
            if !crate::contracts::STORYLET_CLUSTER_TYPES.contains(&storylet.cluster_type.as_str()) {
                return Err(format!("Storylet validation failed: unsupported cluster_type '{}'", storylet.cluster_type));
            }
            let graph = match storylet.graph.clone() {
                Some(graph) => graph,
                None => NarrativeGraph::from_storylet(&storylet),
            };
            graph.validate().map_err(|issues| {
                format!("Storylet graph validation failed: {}", issues.join("; "))
            })?;
            if graph.nodes.iter().any(|node| matches!(&node.data,
                crate::narrative_graph::NarrativeNodeData::InventoryAdd {}
                | crate::narrative_graph::NarrativeNodeData::InventoryRemove {}
                | crate::narrative_graph::NarrativeNodeData::InventoryHas {}))
            {
                let mut runtime_entities = index.entity_templates.clone();
                crate::inventory::project_inventory_entities(&mut runtime_entities, &index.entities, &index.templates)
                    .map_err(|error| format!("Inventory validation failed: {error}"))?;
                crate::inventory::validate_inventory_graph(&graph, &runtime_entities)
                    .map_err(|error| format!("Inventory validation failed: {error}"))?;
            }
            storylet.ending = graph.ending_from_graph();
            entities::normalize_optional_id(&mut storylet.folder_id);
            entities::validate_folder_ref("storylets", storylet.folder_id.as_deref(), &index)
                .map_err(|error| format!("Storylet validation failed: {error}"))?;
            validate_storylet_gameplay_tags(&inner.store, &storylet, &graph)?;
            let payload_object = final_payload
                .as_object_mut()
                .ok_or("Storylet payload must be a JSON object")?;
            payload_object.insert("ending".into(), Value::Bool(storylet.ending));
            payload_object.insert(
                "folder_id".into(),
                storylet
                    .folder_id
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            let templates = inner
                .store
                .list_entity_templates()
                .map_err(|error| error.to_string())?;
            let trait_templates = inner
                .store
                .list_trait_templates()
                .map_err(|error| error.to_string())?;
            let runtime = persisted_runtime(&inner.store, &trait_templates, &templates)?;
            compile_chrono_dispatch_for_storylet_validation(
                &[storylet],
                &runtime.locations,
                &runtime.characters,
            )
            .map_err(|error| format!("ChronoFlow storylet validation failed: {error}"))?;
        }

        let json_str = serde_json::to_string(&final_payload).map_err(|e| e.to_string())?;
        inner
            .store
            .save_content(entity, &id, &json_str, revision, approved_rev)
            .map_err(|e| e.to_string())?;
        Ok(final_payload)
    }

    pub async fn delete_content_item(
        &self,
        entity: &str,
        id: &str,
    ) -> Result<bool, ContentDeleteError> {
        ensure_content_kind(entity).map_err(ContentDeleteError::Conflict)?;
        if entity == "entities" && id == "world" {
            return Err(ContentDeleteError::Conflict(
                "Intrinsic World entity is system-owned and cannot be deleted".into(),
            ));
        }
        let inner = self.inner.lock().await;
        if entity == "storylets" {
            let templates = inner
                .store
                .list_entity_templates()
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?;
            let trait_templates = inner
                .store
                .list_trait_templates()
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?;
            let runtime = persisted_runtime(&inner.store, &trait_templates, &templates)
                .map_err(ContentDeleteError::Persistence)?;
            if let Some(character) = runtime.characters.iter().find(|character| {
                character
                    .thresholds
                    .iter()
                    .any(|threshold| threshold.linked_graph_asset_id == id)
            }) {
                return Err(ContentDeleteError::Conflict(format!(
                    "Cannot delete storylet '{id}': character '{}' has a passion threshold linked to it",
                    character.name
                )));
            }
        }
        if entity == "folders" {
            let folders = inner
                .store
                .list_folders()
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?;
            let Some(folder) = folders.iter().find(|folder| folder.id == id) else {
                return Ok(false);
            };
            let subfolders = folders
                .iter()
                .filter(|candidate| candidate.parent_id.as_deref() == Some(id))
                .count();
            let items = count_items_in_folder(&inner.store, id)
                .map_err(ContentDeleteError::Persistence)?;
            if subfolders > 0 || items > 0 {
                return Err(ContentDeleteError::Conflict(format!(
                    "Folder '{}' is not empty ({subfolders} subfolders, {items} items)",
                    folder.name
                )));
            }
            return inner
                .store
                .delete_content(entity, id)
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()));
        }
        if matches!(
            entity,
            "entity_templates" | "gameplay_tags" | "assets" | "trait_templates"
        ) {
            if inner
                .store
                .get_content_json(entity, id)
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?
                .is_none()
            {
                return Ok(false);
            }
            if entity == "trait_templates" {
                let template: Option<TraitTemplateRecord> =
                    load_stored_record(&inner.store, entity, Some(id))
                        .map_err(ContentDeleteError::Persistence)?;
                if template.is_some_and(|template| template.system) {
                    return Err(ContentDeleteError::Conflict(
                        "System traits cannot be deleted".into(),
                    ));
                }
            }
            if entity == "gameplay_tags" {
                let tags = inner
                    .store
                    .list_gameplay_tags()
                    .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?;
                let tag = tags
                    .iter()
                    .find(|tag| tag.id == id)
                    .ok_or_else(|| ContentDeleteError::Conflict(format!("Gameplay tag '{id}' not found")))?;
                if tag.protected {
                    return Err(ContentDeleteError::Conflict(format!("Cannot delete protected gameplay tag '{}'", tag.name)));
                }
                if tags.iter().any(|child| {
                    child
                        .name
                        .get(..tag.name.len() + 1)
                        .is_some_and(|prefix| {
                            prefix.eq_ignore_ascii_case(&format!("{}.", tag.name))
                        })
                }) {
                    return Err(ContentDeleteError::Conflict(format!(
                        "Cannot delete gameplay tag '{}': child tags still exist",
                        tag.name
                    )));
                }
            }
            let references = collect_references(&inner, entity, id)
                .map_err(ContentDeleteError::Persistence)?;
            let direct: Vec<&ContentReference> = references
                .iter()
                .filter(|reference| {
                    entity == "gameplay_tags"
                        || reference.via == crate::contracts::ReferenceVia::Id
                })
                .collect();
            if !direct.is_empty() {
                let label = record_label(&inner.store, entity, id)
                    .map_err(ContentDeleteError::Persistence)?
                    .unwrap_or_else(|| id.to_string());
                let mut grouped: Vec<(String, String, Vec<String>)> = Vec::new();
                for reference in &direct {
                    match grouped
                        .iter_mut()
                        .find(|(source, _, _)| source == &reference.source_id)
                    {
                        Some((_, _, fields)) => {
                            if !fields.contains(&reference.field) {
                                fields.push(reference.field.clone());
                            }
                        }
                        None => grouped.push((
                            reference.source_id.clone(),
                            reference.source_label.clone(),
                            vec![reference.field.clone()],
                        )),
                    }
                }
                let summary = grouped
                    .iter()
                    .take(5)
                    .map(|(_, source_label, fields)| {
                        format!("{source_label} ({})", fields.join(", "))
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(ContentDeleteError::Conflict(format!(
                    "Cannot delete {} '{label}': referenced by {} item(s): {summary}",
                    kind_label(entity),
                    grouped.len()
                )));
            }
        }
        inner
            .store
            .delete_content(entity, id)
            .map_err(|error| ContentDeleteError::Persistence(error.to_string()))
    }

    /// Move an item (or a folder) into a different folder of the same root.
    pub async fn move_content_item(
        &self,
        kind: &str,
        id: &str,
        folder_id: Option<String>,
    ) -> Result<Value, String> {
        if kind != "folders" && !FOLDER_ROOTS.contains(&kind) {
            return Err(format!("Unknown content kind '{kind}'"));
        }
        let inner = self.inner.lock().await;
        let mut value: Value = inner
            .store
            .get_content_json(kind, id)
            .map_err(|error| error.to_string())?
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .ok_or_else(|| format!("{kind} item '{id}' not found"))?;
        let index = build_workspace_index(&inner.store)?;
        let target = folder_id.filter(|folder| !folder.trim().is_empty());

        if kind == "folders" {
            let stored: FolderRecord =
                serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
            let mut candidate = value.clone();
            if let Some(object) = candidate.as_object_mut() {
                object.insert(
                    "parent_id".into(),
                    target
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
            }
            let prepared = entities::prepare_folder(&candidate, Some(&stored), &index)?;
            value = serde_json::to_value(&prepared).map_err(|error| error.to_string())?;
        } else {
            entities::validate_folder_ref(kind, target.as_deref(), &index)?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "folder_id".into(),
                    target
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
            }
        }

        let json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        if !inner
            .store
            .update_content_json(kind, id, &json)
            .map_err(|error| error.to_string())?
        {
            return Err(format!("{kind} item '{id}' not found"));
        }
        Ok(value)
    }

    /// Where an entity template, gameplay tag, asset, or trait is used.
    pub async fn content_references(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Vec<ContentReference>, String> {
        if !matches!(
            kind,
            "entity_templates" | "gameplay_tags" | "assets" | "trait_templates" | "storylets"
        ) {
            return Err("Unsupported reference kind".into());
        }
        let inner = self.inner.lock().await;
        if kind != "storylets" && inner
            .store
            .get_content_json(kind, id)
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err(format!("{kind} item '{id}' not found"));
        }
        collect_references(&inner, kind, id)
    }

    pub async fn approve_storylet(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<StoryletRecord, String> {
        let inner = self.inner.lock().await;
        inner
            .store
            .approve_storylet(id, expected_revision)?
            .ok_or_else(|| format!("Storylet '{id}' not found"))
    }

    pub async fn query_library(&self, query: &LibraryQuery) -> Result<LibraryPageResult, String> {
        let inner = self.inner.lock().await;
        inner.store.query_library(query).map_err(|e| e.to_string())
    }

    pub async fn preview_scenario(&self, scenario_val: &Value) -> Result<Value, String> {
        let scenario: ScenarioRecord = serde_json::from_value(scenario_val.clone())
            .map_err(|e| format!("Invalid scenario format: {e}"))?;

        let inner = self.inner.lock().await;
        let templates = inner
            .store
            .list_entity_templates()
            .map_err(|e| e.to_string())?;
        let trait_templates = inner
            .store
            .list_trait_templates()
            .map_err(|e| e.to_string())?;
        let runtime = persisted_runtime(&inner.store, &trait_templates, &templates)?;
        let resolved =
            resolve_scenario_start(&scenario, &runtime.locations, &runtime.characters);
        serde_json::to_value(&resolved).map_err(|e| e.to_string())
    }

    pub async fn get_authoring_options(&self) -> AuthoringOptions {
        AuthoringOptions {
            repeatability_modes: vec![
                "always".into(),
                "once_ever".into(),
                "cooldown".into(),
                "persistent".into(),
            ],
            cluster_types: vec![
                "player_passage".into(),
                "linear_progression".into(),
                "wheel_deck".into(),
                "aftermath_accelerator".into(),
            ],
            policy_modes: vec!["fixed".into(), "random".into(), "by_attribute".into()],
            event_types: vec!["canon".into(), "world".into()],
        }
    }

    // --- Studio Interaction Context ---

    pub async fn save_interaction_context(&self, context: &Value) -> Result<(), String> {
        let inner = self.inner.lock().await;
        inner
            .store
            .save_interaction_context(context)
            .map_err(|e| e.to_string())
    }

    pub async fn mark_interaction_context_stale(&self) -> Result<(), String> {
        let inner = self.inner.lock().await;
        inner
            .store
            .mark_interaction_context_stale()
            .map_err(|e| e.to_string())
    }

    pub async fn get_interaction_context(&self) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let meta = inner
            .store
            .get_project_metadata()
            .map_err(|e| e.to_string())?;
        let row = inner
            .store
            .get_interaction_context()
            .map_err(|e| e.to_string())?;

        let now = chrono_now_ms();
        match row {
            Some((context, captured_at, updated_at, is_stale)) => {
                let age_ms = (now - captured_at).max(0);
                let context_project_rev = context
                    .get("project")
                    .and_then(|p| p.get("revision"))
                    .and_then(|r| r.as_u64());
                let live_project_rev = meta.revision;
                let revision_matches = context_project_rev == Some(live_project_rev);
                let fresh = age_ms < 60_000 && !is_stale;

                Ok(json!({
                    "hasContext": true,
                    "context": context,
                    "capturedAt": captured_at,
                    "updatedAt": updated_at,
                    "ageMs": age_ms,
                    "fresh": fresh,
                    "isStale": is_stale,
                    "revisionMatches": revision_matches,
                    "liveProjectRevision": live_project_rev,
                    "contextProjectRevision": context_project_rev,
                }))
            }
            None => Ok(json!({
                "hasContext": false,
                "context": Value::Null,
                "capturedAt": Value::Null,
                "updatedAt": Value::Null,
                "ageMs": Value::Null,
                "fresh": false,
                "isStale": true,
                "revisionMatches": false,
                "liveProjectRevision": meta.revision,
                "contextProjectRevision": Value::Null,
            })),
        }
    }

    // --- Playtests & Runtime Engine Integration ---

    pub async fn create_playtest_session(
        &self,
        session_id: &str,
        project: EngineProjectBundle,
        scenario_id: &str,
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;
        ensure_session_id_has_no_archive(&inner.store.conn, session_id)?;

        let snapshot = inner
            .runtime
            .create_session(session_id.to_string(), project, scenario_id)
            .map_err(|e| e.to_string())?;

        persist_session_to_store(&mut inner, session_id)?;

        let snap_val = serde_json::to_value(&snapshot).map_err(|e| e.to_string())?;
        let change = SessionChange {
            session_id: session_id.to_string(),
            revision: snapshot.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
        };
        let _ = self.updates.send(change);

        Ok(json!({ "sessionId": session_id, "snapshot": snap_val }))
    }

    pub async fn create_playtest_from_workspace(
        &self,
        start_location_override: Option<&str>,
        scenario_id: Option<&str>,
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;

        let meta = inner
            .store
            .get_project_metadata()
            .map_err(|e| e.to_string())?;
        let templates = inner
            .store
            .list_entity_templates()
            .map_err(|e| e.to_string())?;
        let trait_templates = inner
            .store
            .list_trait_templates()
            .map_err(|e| e.to_string())?;
        let projection_issues = entities::generic_playtest_projection_issues(&templates);
        if !projection_issues.is_empty() {
            return Err(format!(
                "Workspace cannot be represented by generic playtests: {}",
                projection_issues.join("; ")
            ));
        }
        let runtime = persisted_runtime(&inner.store, &trait_templates, &templates)?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let events = inner.store.list_events().map_err(|e| e.to_string())?;

        let mut scenario_rec: Option<ScenarioRecord> = None;
        if let Some(s_id) = scenario_id {
            if let Some(json_s) = inner
                .store
                .get_content_json("scenarios", s_id)
                .map_err(|e| e.to_string())?
            {
                scenario_rec = serde_json::from_str(&json_s).ok();
            }
        }

        if let Some(loc_id) = start_location_override {
            if let Some(scen) = &mut scenario_rec {
                scen.start_location_policy.mode = "fixed".into();
                scen.start_location_policy.location_id = Some(loc_id.to_string());
            }
        }

        let proj = project_workspace(
            &meta.id,
            &meta.name,
            meta.revision,
            &runtime.locations,
            &runtime.characters,
            &storylets,
            &events,
            scenario_rec.as_ref(),
        );

        if !proj.issues.is_empty() {
            return Err(format!(
                "Workspace has issues before starting playtest: {}",
                proj.issues.join("; ")
            ));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let snapshot = inner
            .runtime
            .create_session(session_id.clone(), proj.bundle, &proj.scenario_id)
            .map_err(|e| e.to_string())?;

        // Persist session to Store
        persist_session_to_store(&mut inner, &session_id)?;

        let snap_val = serde_json::to_value(&snapshot).map_err(|e| e.to_string())?;
        let change = SessionChange {
            session_id: session_id.clone(),
            revision: snapshot.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
        };
        let _ = self.updates.send(change);

        Ok(json!({ "sessionId": session_id, "snapshot": snap_val }))
    }

    pub async fn list_playtest_sessions(&self) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let mut stmt = inner.store.conn.prepare(
            "SELECT session_id, project_id, revision, turn, state_json FROM runtime_sessions ORDER BY updated_at DESC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let enc: String = row.get(4)?;
                let state: Option<RuntimeSessionState> = serde_json::from_str(&enc).ok();
                let curr_loc = state
                    .as_ref()
                    .and_then(|s| s.world.current_location.clone());
                let ev_count = state.as_ref().map(|s| s.world.events.len()).unwrap_or(0);

                let scen_id = state
                    .as_ref()
                    .map(|s| s.scenario_id.clone())
                    .unwrap_or_default();
                Ok(json!({
                    "session_id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "scenario_id": scen_id,
                    "revision": row.get::<_, String>(2)?.parse::<u64>().unwrap_or(0),
                    "turn": row.get::<_, String>(3)?.parse::<u64>().unwrap_or(0),
                    "current_location": curr_loc,
                    "event_count": ev_count,
                }))
            })
            .map_err(|e| e.to_string())?;

        let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
        Ok(Value::Array(items))
    }

    pub async fn get_playtest_snapshot(&self, session_id: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let snap = inner
            .runtime
            .snapshot(session_id)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(snap).map_err(|e| e.to_string())
    }

    pub async fn evaluate_playtest_director(
        &self,
        session_id: &str,
        slope: f64,
    ) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let eval = inner
            .runtime
            .evaluate(session_id, slope)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(eval).map_err(|e| e.to_string())
    }

    pub async fn apply_playtest_command(
        &self,
        session_id: &str,
        cmd_val: &Value,
    ) -> Result<Value, String> {
        let cmd: WorldCommand = serde_json::from_value(cmd_val.clone())
            .map_err(|e| format!("Invalid command JSON: {e}"))?;

        let mut inner = self.inner.lock().await;
        let prior_export = inner
            .runtime
            .export_session(session_id)
            .map_err(|e| e.to_string())?;

        let res = match inner.runtime.submit_command(session_id, cmd) {
            Ok(r) => r,
            Err(e) => return Err(e.to_string()),
        };

        if let Err(err) = persist_session_to_store(&mut inner, session_id) {
            let _ = inner.runtime.delete_session(session_id);
            let _ = inner.runtime.restore_session(prior_export);
            return Err(format!("Failed to persist command: {err}"));
        }

        if !res.player.duplicate {
            let snap = inner
                .runtime
                .snapshot(session_id)
                .map_err(|e| e.to_string())?;
            let _ = self.updates.send(SessionChange {
                session_id: session_id.to_string(),
                revision: snap.world.revision,
                snapshot: json!({ "sessionId": session_id, "snapshot": snap }),
            });
        }

        serde_json::to_value(res).map_err(|e| e.to_string())
    }

    pub async fn apply_playtest_choice(
        &self,
        session_id: &str,
        storylet_id: &str,
        choice_id: &str,
        expected_revision: u64,
        idempotency_key: &str,
    ) -> Result<Value, ChoiceOperationError> {
        if storylet_id.trim().is_empty() {
            return Err(ChoiceOperationError::InvalidInput(
                "storyletId must not be empty".into(),
            ));
        }
        if choice_id.trim().is_empty() {
            return Err(ChoiceOperationError::InvalidInput(
                "choiceId must not be empty".into(),
            ));
        }
        if idempotency_key.trim().is_empty() {
            return Err(ChoiceOperationError::InvalidInput(
                "idempotencyKey must not be empty".into(),
            ));
        }
        let mut inner = self.inner.lock().await;
        let prior_export =
            inner
                .runtime
                .export_session(session_id)
                .map_err(|error| match error {
                    RuntimeError::SessionNotFound { session_id } => {
                        ChoiceOperationError::SessionNotFound(session_id)
                    }
                    other => ChoiceOperationError::Runtime(other.to_string()),
                })?;

        let res = inner
            .runtime
            .execute_choice(
                session_id,
                storylet_id,
                choice_id,
                idempotency_key,
                expected_revision,
            )
            .map_err(|error| match error {
                RuntimeError::SessionNotFound { session_id } => {
                    ChoiceOperationError::SessionNotFound(session_id)
                }
                RuntimeError::Engine(
                    EngineError::RevisionConflict { .. } | EngineError::IdempotencyKeyReuse,
                ) => ChoiceOperationError::Conflict(error.to_string()),
                RuntimeError::Engine(EngineError::ChoiceNotFound) => {
                    ChoiceOperationError::NotFound(error.to_string())
                }
                RuntimeError::Engine(
                    EngineError::StoryletNotEligible { .. }
                    | EngineError::ChoiceRequirementFailed { .. }
                    | EngineError::InstanceNotActive
                    | EngineError::SessionEnded
                    | EngineError::PersistentSceneCannotContinue,
                ) => ChoiceOperationError::Conflict(error.to_string()),
                other => ChoiceOperationError::Runtime(other.to_string()),
            })?;

        if let Err(err) = persist_session_to_store(&mut inner, session_id) {
            let _ = inner.runtime.delete_session(session_id);
            let _ = inner.runtime.restore_session(prior_export);
            return Err(ChoiceOperationError::Persistence(format!(
                "Failed to persist choice: {err}"
            )));
        }

        let snap = inner
            .runtime
            .snapshot(session_id)
            .map_err(|error| ChoiceOperationError::Runtime(error.to_string()))?;
        let _ = self.updates.send(SessionChange {
            session_id: session_id.to_string(),
            revision: snap.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snap }),
        });

        serde_json::to_value(res).map_err(|error| ChoiceOperationError::Runtime(error.to_string()))
    }

    pub async fn fork_playtest(
        &self,
        source_session_id: &str,
        branch_session_id: Option<&str>,
        event_id: &str,
    ) -> Result<Value, String> {
        let branch_id = branch_session_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let mut inner = self.inner.lock().await;
        let snap = inner
            .runtime
            .fork_session(source_session_id, branch_id.clone(), event_id)
            .map_err(|e| e.to_string())?;
        persist_session_to_store(&mut inner, &branch_id)?;

        let _ = self.updates.send(SessionChange {
            session_id: branch_id.clone(),
            revision: snap.world.revision,
            snapshot: json!({ "sessionId": branch_id, "snapshot": snap }),
        });

        Ok(json!({ "sessionId": branch_id, "snapshot": snap }))
    }

    pub async fn get_playtest_events(&self, session_id: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let mut stmt = inner
            .store
            .conn
            .prepare("SELECT event_json FROM runtime_events WHERE session_id = ?1 ORDER BY rowid")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([session_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let events: Vec<Value> = rows
            .filter_map(|r| r.ok())
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        Ok(Value::Array(events))
    }

    pub async fn delete_playtest(&self, session_id: &str) -> Result<bool, String> {
        let mut inner = self.inner.lock().await;
        inner.runtime.delete_session(session_id);
        inner
            .store
            .conn
            .execute(
                "DELETE FROM runtime_sessions WHERE session_id = ?1",
                [session_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
}

fn ensure_content_kind(entity: &str) -> Result<(), String> {
    if CONTENT_KINDS.contains(&entity) {
        Ok(())
    } else {
        Err(format!("Unknown content kind '{entity}'"))
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "entity_templates" => "entity template",
        "gameplay_tags" => "gameplay tag",
        "assets" => "asset",
        "trait_templates" => "trait",
        _ => "item",
    }
}

fn build_workspace_index(store: &Store) -> Result<WorkspaceIndex, String> {
    Ok(WorkspaceIndex {
        entities: store.list_entities().map_err(|error| error.to_string())?,
        entity_templates: store
            .list_entity_templates()
            .map_err(|error| error.to_string())?,
        gameplay_tags: store
            .list_gameplay_tags()
            .map_err(|error| error.to_string())?,
        templates: store
            .list_trait_templates()
            .map_err(|error| error.to_string())?,
        assets: store.list_assets().map_err(|error| error.to_string())?,
        folders: store.list_folders().map_err(|error| error.to_string())?,
        storylet_ids: store
            .list_storylets()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|storylet| storylet.id)
            .collect(),
    })
}

fn prepare_gameplay_tag(
    store: &Store,
    mut tag: GameplayTagRecord,
    stored: Option<&GameplayTagRecord>,
) -> Result<GameplayTagRecord, String> {
    let name = tag.name.trim().to_owned();
    let segments: Vec<&str> = name.split('.').collect();
    if segments.iter().any(|segment| !entities::is_identifier(segment)) {
        return Err("Gameplay tag name must be a dot-separated identifier path".into());
    }
    let tags = store.list_gameplay_tags().map_err(|error| error.to_string())?;
    if tags.iter().any(|existing| {
        Some(existing.id.as_str()) != stored.map(|record| record.id.as_str())
            && existing.name.eq_ignore_ascii_case(&name)
    }) {
        return Err(format!("Gameplay tag '{name}' already exists"));
    }
    if let Some((parent, _)) = name.rsplit_once('.')
        && !tags.iter().any(|existing| existing.name.eq_ignore_ascii_case(parent))
    {
        return Err(format!(
            "Gameplay tag parent '{parent}' must exist before creating '{name}'"
        ));
    }
    if let Some(stored) = stored {
        if stored.name != name {
            if stored.protected {
                return Err(format!("Protected gameplay tag '{}' cannot be renamed", stored.name));
            }
            if tags.iter().any(|candidate| {
                candidate
                    .name
                    .get(..stored.name.len() + 1)
                    .is_some_and(|prefix| {
                        prefix.eq_ignore_ascii_case(&format!("{}.", stored.name))
                    })
            }) {
                return Err(format!(
                    "Gameplay tag '{}' cannot be renamed while child tags exist",
                    stored.name
                ));
            }
            if !gameplay_tag_references_for_store(store, stored)?.is_empty() {
                return Err(format!(
                    "Gameplay tag '{}' cannot be renamed while it is referenced",
                    stored.name
                ));
            }
        }
        tag.id.clone_from(&stored.id);
        tag.protected = stored.protected;
        tag.created_at = stored.created_at;
    } else {
        if tag.id.trim().is_empty() {
            tag.id = uuid::Uuid::new_v4().to_string();
        } else if store
            .get_content_json("gameplay_tags", &tag.id)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err(format!("Gameplay tag id '{}' already exists", tag.id));
        }
        tag.protected = false;
    }
    tag.name = name.to_owned();
    Ok(tag)
}

fn gameplay_tag_references_for_store(
    store: &Store,
    target: &GameplayTagRecord,
) -> Result<Vec<ContentReference>, String> {
    let entities = store
        .list_entity_templates()
        .map_err(|error| error.to_string())?;
    let templates = store
        .list_trait_templates()
        .map_err(|error| error.to_string())?;
    let assets = store.list_assets().map_err(|error| error.to_string())?;
    let storylets = store.list_storylets().map_err(|error| error.to_string())?;
    let scenarios = store.list_scenarios().map_err(|error| error.to_string())?;
    let gameplay_tags = store.list_gameplay_tags().map_err(|error| error.to_string())?;
    let context = ReferenceContext {
        entities: &entities,
        templates: &templates,
        assets: &assets,
        storylets: &storylets,
        scenarios: &scenarios,
        gameplay_tags: &gameplay_tags,
    };
    Ok(references::gameplay_tag_references(target, &context))
}

fn validate_storylet_gameplay_tags(
    store: &Store,
    storylet: &StoryletRecord,
    graph: &NarrativeGraph,
) -> Result<(), String> {
    let tags = store.list_gameplay_tags().map_err(|error| error.to_string())?;
    let is_registered = |name: &str| tags.iter().any(|tag| tag.name == name);
    for name in &storylet.location_tags {
        if !is_registered(name) {
            return Err(format!(
                "Storylet validation failed: gameplay tag '{name}' is not registered"
            ));
        }
    }
    for parameter in &storylet.parameters {
        let Some(role) = crate::projection::parse_role_query(&parameter.query) else {
            return Err(format!(
                "Storylet validation failed: parameter '{}' must use a canonical role query",
                parameter.param_name
            ));
        };
        if !is_registered(&role) { return Err(format!("Storylet validation failed: gameplay tag '{role}' is not registered")); }
    }

    let events = store.list_events().map_err(|error| error.to_string())?;
    for node in &graph.nodes {
        if let crate::narrative_graph::NarrativeNodeData::Reveal { event_id, .. } = &node.data
            && !events.iter().any(|event| event.id == *event_id && event.is_chronicle_entry())
        {
            return Err(format!(
                "Storylet validation failed: Reveal node \"{}\": event \"{event_id}\" is not a canon event with a story time",
                node.title
            ));
        }
    }

    fn check_graph_tags(value: &Value, tags: &[GameplayTagRecord]) -> Result<(), String> {
        let Some(object) = value.as_object() else {
            if let Some(values) = value.as_array() {
                for item in values {
                    check_graph_tags(item, tags)?;
                }
            }
            return Ok(());
        };
        for (key, value) in object {
            if let Some(name) = value.as_str() {
                let role_key = matches!(
                    key.as_str(),
                    "speaker_role" | "target_role" | "actor_role" | "action_target"
                ) || (key == "target"
                    && object.get("kind").and_then(Value::as_str) == Some("idle_action"));
                let tag_key = matches!(
                    key.as_str(),
                    "tag" | "emotion_tag" | "action_tag" | "emote"
                );
                if role_key && name != "player" {
                    if !tags.iter().any(|tag| tag.name == name) { return Err(format!("Storylet validation failed: gameplay tag '{name}' is not registered")); }
                } else if tag_key {
                    if !tags.iter().any(|tag| tag.name == name) { return Err(format!("Storylet validation failed: gameplay tag '{name}' is not registered")); }
                }
            }
            check_graph_tags(value, tags)?;
        }
        Ok(())
    }
    check_graph_tags(
        &serde_json::to_value(graph).map_err(|error| error.to_string())?,
        &tags,
    )
}

fn load_stored_record<T: serde::de::DeserializeOwned>(
    store: &Store,
    kind: &str,
    id: Option<&str>,
) -> Result<Option<T>, String> {
    let Some(id) = id else {
        return Ok(None);
    };
    let Some(raw) = store
        .get_content_json(kind, id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&raw).ok())
}

fn record_label(store: &Store, kind: &str, id: &str) -> Result<Option<String>, String> {
    let Some(raw) = store
        .get_content_json(kind, id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let name = match kind {
        "storylets" => value.get("title"),
        _ => value.get("name"),
    };
    Ok(name.and_then(Value::as_str).map(str::to_string))
}

fn count_items_in_folder(store: &Store, folder_id: &str) -> Result<usize, String> {
    let mut total = 0;
    for kind in FOLDER_ROOTS {
        let rows = store
            .list_content_json(kind)
            .map_err(|error| error.to_string())?;
        for raw in rows {
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            if value.get("folder_id").and_then(Value::as_str) == Some(folder_id) {
                total += 1;
            }
        }
    }
    Ok(total)
}

fn collect_references(
    inner: &StudioInner,
    kind: &str,
    id: &str,
) -> Result<Vec<ContentReference>, String> {
    if !matches!(
        kind,
        "entity_templates" | "gameplay_tags" | "assets" | "trait_templates" | "storylets"
    ) {
        return Err(
            "References are available for entity templates, gameplay tags, assets, traits, and storylets"
                .to_string(),
        );
    }
    let entities = inner
        .store
        .list_entity_templates()
        .map_err(|error| error.to_string())?;
    let gameplay_tags = inner
        .store
        .list_gameplay_tags()
        .map_err(|error| error.to_string())?;
    let templates = inner
        .store
        .list_trait_templates()
        .map_err(|error| error.to_string())?;
    let assets = inner
        .store
        .list_assets()
        .map_err(|error| error.to_string())?;
    let storylets = inner
        .store
        .list_storylets()
        .map_err(|error| error.to_string())?;
    let scenarios = inner
        .store
        .list_scenarios()
        .map_err(|error| error.to_string())?;
    let context = ReferenceContext {
        entities: &entities,
        templates: &templates,
        assets: &assets,
        storylets: &storylets,
        scenarios: &scenarios,
        gameplay_tags: &gameplay_tags,
    };
    match kind {
        "entity_templates" => {
            let target = entities
                .iter()
                .find(|entity| entity.id == id)
                .ok_or_else(|| format!("entity_templates item '{id}' not found"))?;
            Ok(references::entity_references(target, &context))
        }
        "gameplay_tags" => {
            let target = gameplay_tags
                .iter()
                .find(|tag| tag.id == id)
                .ok_or_else(|| format!("gameplay_tags item '{id}' not found"))?;
            Ok(references::gameplay_tag_references(target, &context))
        }
        "assets" => Ok(references::asset_references(id, &context)),
        "storylets" => {
            if !storylets.iter().any(|storylet| storylet.id == id) {
                return Err(format!("storylets item '{id}' not found"));
            }
            Ok(references::storylet_references(id, &context))
        }
        _ => Ok(references::trait_template_references(id, &context)),
    }
}

fn validate_character(
    character: &CharacterRecord,
    others: &[CharacterRecord],
    storylet_exists: impl Fn(&str) -> Result<bool, String>,
) -> Result<(), String> {
    let mut passion_ids = std::collections::BTreeSet::new();
    for passion in &character.passion_pool {
        if passion.passion_id.trim().is_empty()
            || !passion_ids.insert(passion.passion_id.as_str())
            || !passion.current_value.is_finite()
            || !(-100.0..=100.0).contains(&passion.current_value)
            || !passion.weight.is_finite()
            || passion.weight < 0.0
        {
            return Err(format!(
                "Character validation failed: invalid or duplicate passion '{}'",
                passion.passion_id
            ));
        }
    }
    let mut active_ids = std::collections::BTreeSet::new();
    for passion_id in &character.active_passions {
        if !active_ids.insert(passion_id.as_str()) || !passion_ids.contains(passion_id.as_str()) {
            return Err(format!(
                "Character validation failed: active passion '{passion_id}' is missing or duplicated"
            ));
        }
    }
    if !character.trust_inertia.is_finite() {
        return Err("Character validation failed: trust inertia must be finite".into());
    }
    let mut threshold_ids = std::collections::BTreeSet::new();
    for threshold in &character.thresholds {
        if threshold.threshold_id.trim().is_empty()
            || !threshold_ids.insert(threshold.threshold_id.as_str())
            || !threshold.trigger_value.is_finite()
            || !(-100.0..=100.0).contains(&threshold.trigger_value)
            || !matches!(threshold.direction.as_str(), "positive" | "negative")
            || threshold.linked_graph_asset_id.trim().is_empty()
            || !storylet_exists(&threshold.linked_graph_asset_id)?
        {
            return Err("Character validation failed: macro thresholds need a valid direction, value, and existing graph storylet".into());
        }
    }
    let mut links = std::collections::BTreeSet::new();
    for link in &character.zero_sum_passion_links {
        let target = others
            .iter()
            .find(|target| target.id == link.target_character_id);
        let link_key = (
            link.target_character_id.as_str(),
            link.source_passion_id.as_str(),
            link.target_passion_id.as_str(),
        );
        if link.target_character_id.trim().is_empty()
            || !passion_ids.contains(link.source_passion_id.as_str())
            || !link.weight.is_finite()
            || !links.insert(link_key)
            || target.is_none_or(|target| {
                !target
                    .passion_pool
                    .iter()
                    .any(|passion| passion.passion_id == link.target_passion_id)
            })
        {
            return Err(format!(
                "Character validation failed: invalid zero-sum passion link from '{}' to '{}'/'{}'",
                link.source_passion_id, link.target_character_id, link.target_passion_id
            ));
        }
    }
    Ok(())
}

fn persist_session_to_store(inner: &mut StudioInner, session_id: &str) -> Result<(), String> {
    let saved = inner
        .runtime
        .export_session(session_id)
        .map_err(|e| e.to_string())?;
    let state_json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;

    let tx = inner
        .store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;

    let now = chrono_now_ms().to_string();
    tx.execute(
        "INSERT INTO runtime_sessions (session_id, project_id, revision, turn, state_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(session_id) DO UPDATE SET
             revision = excluded.revision,
             turn = excluded.turn,
             state_json = excluded.state_json,
             updated_at = excluded.updated_at",
        rusqlite::params![saved.session_id, saved.project_id, saved.world.revision.to_string(), saved.world.turn.to_string(), state_json, now],
    ).map_err(|e| e.to_string())?;

    {
        let mut insert_ev = tx.prepare(
            "INSERT INTO runtime_events (session_id, event_id, revision, event_json) VALUES (?1, ?2, ?3, ?4)"
        ).map_err(|e| e.to_string())?;

        for ev in &saved.world.events {
            let ev_json = serde_json::to_string(ev).map_err(|e| e.to_string())?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT event_json FROM runtime_events WHERE session_id = ?1 AND event_id = ?2",
                    rusqlite::params![saved.session_id, ev.id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;

            match existing {
                Some(prev_json) if prev_json != ev_json => {
                    return Err(format!(
                        "event '{}' conflicts with archived session history",
                        ev.id
                    ));
                }
                Some(_) => continue,
                None => {
                    insert_ev
                        .execute(rusqlite::params![
                            saved.session_id,
                            ev.id,
                            ev.revision.to_string(),
                            ev_json
                        ])
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}


fn validate_event_history(
    conn: &rusqlite::Connection,
    state: &RuntimeSessionState,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT event_json FROM runtime_events WHERE session_id = ?1 ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&state.session_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;

    let mut events = Vec::new();
    for row in rows {
        let enc = row.map_err(|e| e.to_string())?;
        events.push(serde_json::from_str::<WorldEvent>(&enc).map_err(|e| e.to_string())?);
    }
    if events != state.world.events {
        return Err(format!(
            "saved session '{}' event journal does not match its snapshot",
            state.session_id
        ));
    }
    Ok(())
}

fn ensure_session_id_has_no_archive(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<(), String> {
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM runtime_events WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count > 0 {
        return Err(format!(
            "session '{session_id}' has archived event history and cannot be reused"
        ));
    }
    Ok(())
}
