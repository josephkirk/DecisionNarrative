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
    EngineError, ProjectBundle as EngineProjectBundle, RUNTIME_SESSION_FORMAT_VERSION, RuntimeError,
    RuntimeSessionState, StoryletRuntime, WorldCommand, WorldEvent,
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
    AssetRecord, AuthoringOptions, CharacterRecord, ContentReference, EntityRecord, FolderRecord,
    LibraryPageResult, LibraryQuery, ProjectBundle, ProjectBundleContent, ProjectMetadata,
    ScenarioRecord, StoryletRecord, TraitTemplateRecord, CONTENT_KINDS, FOLDER_ROOTS,
};
use crate::entities::{self, WorkspaceIndex};
use crate::narrative_graph::NarrativeGraph;
use crate::projection::project_workspace;
use crate::references::{self, ReferenceContext};
use crate::scenario::resolve_scenario_start;
use crate::store::Store;

#[derive(Clone)]
pub struct StudioService {
    pub(crate) inner: Arc<Mutex<StudioInner>>,
    pub updates: broadcast::Sender<SessionChange>,
    pub resource_root: PathBuf,
}

pub(crate) struct StudioInner {
    pub(crate) store: Store,
    runtime: StoryletRuntime,
    /// Live ChronoFlow v5 playtest sessions (REQ-CH-02). The persisted rows are
    /// the source of truth; these are the runtime instances rebuilt from them.
    pub(crate) chrono_sessions: BTreeMap<String, ChronoPlaytestSession>,
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
            Some(value) => match serde_json::from_value::<NarrativeGraph>(value) {
                Ok(graph) => graph,
                Err(error) => {
                    issues.push(format!("storylet {} graph is invalid: {error}", storylet.id));
                    continue;
                }
            },
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

    fn build(mut store: Store, resource_root: impl Into<PathBuf>) -> Result<Self, String> {
        let mut runtime = StoryletRuntime::default();
        let saved_sessions = {
            let mut stmt = store
                .conn
                .prepare("SELECT session_id, state_json FROM runtime_sessions ORDER BY session_id")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };

        let mut migrations = Vec::new();
        for (row_id, enc) in saved_sessions {
            let state: RuntimeSessionState = serde_json::from_str(&enc)
                .map_err(|e| format!("saved session '{row_id}' JSON is invalid: {e}"))?;
            if state.session_id != row_id {
                return Err(format!(
                    "saved session row '{row_id}' contains state for '{}'",
                    state.session_id
                ));
            }
            validate_event_history(&store.conn, &state)
                .map_err(|e| format!("saved session '{row_id}' event history is invalid: {e}"))?;
            let needs_migration = state.format_version < RUNTIME_SESSION_FORMAT_VERSION;
            runtime
                .restore_session(state)
                .map_err(|e| format!("saved session '{row_id}' cannot be restored: {e}"))?;
            if needs_migration {
                migrations.push((
                    enc,
                    runtime.export_session(&row_id).map_err(|e| {
                        format!("failed to export migrated session '{row_id}': {e}")
                    })?,
                ));
            }
        }

        if !migrations.is_empty() {
            let tx = store.conn.transaction().map_err(|e| e.to_string())?;
            for (old_state_json, state) in migrations {
                let new_state_json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
                let updated = tx.execute(
                    "UPDATE runtime_sessions SET revision = ?1, turn = ?2, state_json = ?3, updated_at = ?4
                     WHERE session_id = ?5 AND state_json = ?6",
                    rusqlite::params![
                        state.world.revision.to_string(),
                        state.world.turn.to_string(),
                        new_state_json,
                        chrono_now_ms().to_string(),
                        &state.session_id,
                        old_state_json
                    ],
                ).map_err(|e| e.to_string())?;
                if updated != 1 {
                    return Err(format!(
                        "saved session '{}' changed during migration",
                        state.session_id
                    ));
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
        }

        let (updates, _) = broadcast::channel(256);
        let res_root = resource_root.into();

        Ok(Self {
            inner: Arc::new(Mutex::new(StudioInner {
                store,
                runtime,
                chrono_sessions: BTreeMap::new(),
            })),
            updates,
            resource_root: res_root,
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

        let qualities = inner.store.list_qualities().map_err(|e| e.to_string())?;
        let entities = inner.store.list_entities().map_err(|e| e.to_string())?;
        let folders = inner.store.list_folders().map_err(|e| e.to_string())?;
        let trait_templates = inner
            .store
            .list_trait_templates()
            .map_err(|e| e.to_string())?;
        let assets = inner.store.list_assets().map_err(|e| e.to_string())?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let npc_behaviors = inner
            .store
            .list_npc_behaviors()
            .map_err(|e| e.to_string())?;
        let events = inner.store.list_events().map_err(|e| e.to_string())?;
        let scenarios = inner.store.list_scenarios().map_err(|e| e.to_string())?;
        Ok(ProjectBundle {
            format: "decision-narrative-project".into(),
            schema_version: 3,
            project,
            content: ProjectBundleContent {
                qualities,
                entities,
                folders,
                trait_templates,
                assets,
                storylets,
                npc_behaviors,
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

        // Validate content through projection
        let runtime = entities::runtime_records(
            &bundle.content.entities,
            &bundle.content.trait_templates,
        );
        let res = project_workspace(
            &bundle.project.id,
            &bundle.project.name,
            bundle.project.revision,
            &bundle.content.qualities,
            &runtime.locations,
            &runtime.characters,
            &bundle.content.storylets,
            &bundle.content.npc_behaviors,
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
                let prepared = entities::prepare_entity(payload, stored.as_ref(), &index)?;
                serde_json::to_value(&prepared).map_err(|e| e.to_string())?
            }
            "trait_templates" => {
                let stored: Option<TraitTemplateRecord> =
                    load_stored_record(&inner.store, "trait_templates", stored_id)?;
                let prepared =
                    entities::prepare_trait_template(payload, stored.as_ref(), &index)?;
                serde_json::to_value(&prepared).map_err(|e| e.to_string())?
            }
            "assets" => {
                let stored: Option<AssetRecord> =
                    load_stored_record(&inner.store, "assets", stored_id)?;
                let prepared = entities::prepare_asset(payload, stored.as_ref(), &index)?;
                serde_json::to_value(&prepared).map_err(|e| e.to_string())?
            }
            "folders" => {
                let stored: Option<FolderRecord> =
                    load_stored_record(&inner.store, "folders", stored_id)?;
                let prepared = entities::prepare_folder(payload, stored.as_ref(), &index)?;
                serde_json::to_value(&prepared).map_err(|e| e.to_string())?
            }
            _ => payload.clone(),
        };

        let id = match entity {
            "qualities" => final_payload
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Quality must have a 'name' field")?
                .to_string(),
            _ => final_payload
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Item must have an 'id' field")?
                .to_string(),
        };

        if entity == "storylets" {
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
            let existing_opt = inner.store.get_storylet(&id).map_err(|e| e.to_string())?;
            let next_rev = match existing_opt {
                Some(prev) => prev.revision + 1,
                None => payload
                    .get("revision")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1),
            };
            if let Some(obj) = final_payload.as_object_mut() {
                obj.insert("revision".into(), Value::from(next_rev));
                obj.insert("approved_revision".into(), Value::Null);
            }
            (next_rev, None)
        } else {
            let rev = payload
                .get("revision")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            let app_rev = payload.get("approved_revision").and_then(|v| v.as_u64());
            (rev, app_rev)
        };
        if entity == "qualities" {
            let mut quality: crate::contracts::QualityRecord =
                serde_json::from_value(final_payload.clone())
                    .map_err(|error| format!("Quality validation failed: {error}"))?;
            if quality.name.trim().is_empty()
                || !matches!(
                    quality.kind.as_str(),
                    "currency" | "metric" | "tool" | "progress" | "menace" | "territory"
                )
            {
                return Err(
                    "Quality validation failed: name and supported category are required".into(),
                );
            }
            if !quality.value.is_finite()
                || !quality.min_value.is_finite()
                || !quality.max_value.is_finite()
                || quality.min_value > quality.max_value
            {
                return Err("Quality validation failed: value bounds must be finite and minimum must not exceed maximum".into());
            }
            if let Some(storylet_id) = &quality.menace_crisis_storylet_id {
                if quality.kind != "menace" {
                    return Err("Quality validation failed: only menace qualities can reference a crisis storylet".into());
                }
                if inner
                    .store
                    .get_content_json("storylets", storylet_id)
                    .map_err(|error| error.to_string())?
                    .is_none()
                {
                    return Err(format!(
                        "Quality validation failed: crisis storylet '{storylet_id}' does not exist"
                    ));
                }
            }
            storylet_engine::chrono_state::MetricRegistry::compile([quality.name.clone()])
                .map_err(|error| format!("Quality validation failed: {error}"))?;
            entities::normalize_optional_id(&mut quality.folder_id);
            entities::validate_folder_ref("qualities", quality.folder_id.as_deref(), &index)
                .map_err(|error| format!("Quality validation failed: {error}"))?;
            final_payload = serde_json::to_value(quality).map_err(|error| error.to_string())?;
        }
        if entity == "entities" {
            let entity_record: EntityRecord = serde_json::from_value(final_payload.clone())
                .map_err(|error| format!("Entity validation failed: {error}"))?;
            if entity_record.entity_type == crate::contracts::EntityType::Character {
                let character = entities::character_record(&entity_record, Value::Null)
                    .ok_or("Entity validation failed: character settings are missing")?;
                let mut characters = inner
                    .store
                    .runtime_records()
                    .map_err(|error| error.to_string())?
                    .characters;
                characters.retain(|existing| existing.id != character.id);
                characters.push(character.clone());
                validate_character(&character, &characters, |storylet_id| {
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
            let graph = match storylet.graph.clone() {
                Some(value) => serde_json::from_value::<NarrativeGraph>(value)
                    .map_err(|error| format!("Storylet graph validation failed: {error}"))?,
                None => NarrativeGraph::from_storylet(&storylet),
            };
            graph.validate().map_err(|issues| {
                format!("Storylet graph validation failed: {}", issues.join("; "))
            })?;
            storylet.ending = graph.ending_from_graph();
            entities::normalize_optional_id(&mut storylet.folder_id);
            entities::validate_folder_ref("storylets", storylet.folder_id.as_deref(), &index)
                .map_err(|error| format!("Storylet validation failed: {error}"))?;
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
            let qualities = inner
                .store
                .list_qualities()
                .map_err(|error| error.to_string())?;
            let runtime = inner
                .store
                .runtime_records()
                .map_err(|error| error.to_string())?;
            compile_chrono_dispatch_for_storylet_validation(
                &[storylet],
                &qualities,
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
        let inner = self.inner.lock().await;
        if entity == "storylets" {
            let qualities = inner
                .store
                .list_qualities()
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?;
            if let Some(quality) = qualities
                .iter()
                .find(|quality| quality.menace_crisis_storylet_id.as_deref() == Some(id))
            {
                return Err(ContentDeleteError::Conflict(format!(
                    "Cannot delete storylet '{id}': menace quality '{}' references it as its crisis storylet",
                    quality.name
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
        if matches!(entity, "entities" | "assets" | "trait_templates") {
            if inner
                .store
                .get_content_json(entity, id)
                .map_err(|error| ContentDeleteError::Persistence(error.to_string()))?
                .is_none()
            {
                return Ok(false);
            }
            let references = collect_references(&inner, entity, id)
                .map_err(ContentDeleteError::Persistence)?;
            let direct: Vec<&ContentReference> = references
                .iter()
                .filter(|reference| reference.via == crate::contracts::ReferenceVia::Id)
                .collect();
            if !direct.is_empty() {
                let label = record_label(&inner.store, entity, id)
                    .map_err(ContentDeleteError::Persistence)?
                    .unwrap_or_else(|| id.to_string());
                // One storylet can reference the same record from several nodes;
                // count and name it once, listing the fields it references.
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
                    .map(|(_, source_label, fields)| format!("{source_label} ({})", fields.join(", ")))
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

    /// Where an entity, asset, or trait template is used.
    pub async fn content_references(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Vec<ContentReference>, String> {
        if !matches!(kind, "entities" | "assets" | "trait_templates") {
            return Err(
                "References are only available for entities, assets, and trait templates".into(),
            );
        }
        let inner = self.inner.lock().await;
        if inner
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
        let qualities = inner.store.list_qualities().map_err(|e| e.to_string())?;
        let runtime = inner
            .store
            .runtime_records()
            .map_err(|e| e.to_string())?;

        let resolved = resolve_scenario_start(
            &scenario,
            &qualities,
            &runtime.locations,
            &runtime.characters,
        );
        serde_json::to_value(&resolved).map_err(|e| e.to_string())
    }

    pub async fn get_authoring_options(&self) -> AuthoringOptions {
        AuthoringOptions {
            quality_types: vec![
                "currency".into(),
                "metric".into(),
                "tool".into(),
                "progress".into(),
                "menace".into(),
                "territory".into(),
            ],
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
        let qualities = inner.store.list_qualities().map_err(|e| e.to_string())?;
        let runtime = inner
            .store
            .runtime_records()
            .map_err(|e| e.to_string())?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let npc_behaviors = inner
            .store
            .list_npc_behaviors()
            .map_err(|e| e.to_string())?;
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

        // Apply start location override if provided
        if let Some(loc_id) = start_location_override {
            if let Some(ref mut scen) = scenario_rec {
                scen.start_location_policy.mode = "fixed".into();
                scen.start_location_policy.location_id = Some(loc_id.to_string());
            }
        }

        let proj = project_workspace(
            &meta.id,
            &meta.name,
            meta.revision,
            &qualities,
            &runtime.locations,
            &runtime.characters,
            &storylets,
            &npc_behaviors,
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
        "entities" => "entity",
        "assets" => "asset",
        "trait_templates" => "trait template",
        _ => "item",
    }
}

fn build_workspace_index(store: &Store) -> Result<WorkspaceIndex, String> {
    Ok(WorkspaceIndex {
        entities: store.list_entities().map_err(|error| error.to_string())?,
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
    if !matches!(kind, "entities" | "assets" | "trait_templates") {
        return Err(
            "References are only available for entities, assets, and trait templates".to_string(),
        );
    }
    let entities = inner
        .store
        .list_entities()
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
    let npc_behaviors = inner
        .store
        .list_npc_behaviors()
        .map_err(|error| error.to_string())?;
    let context = ReferenceContext {
        entities: &entities,
        templates: &templates,
        assets: &assets,
        storylets: &storylets,
        scenarios: &scenarios,
        npc_behaviors: &npc_behaviors,
    };
    match kind {
        "entities" => {
            let target = entities
                .iter()
                .find(|entity| entity.id == id)
                .ok_or_else(|| format!("entities item '{id}' not found"))?;
            Ok(references::entity_references(target, &context))
        }
        "assets" => Ok(references::asset_references(id, &context)),
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

pub type RuntimeService = StudioService;

pub fn router(service: StudioService) -> axum::Router {
    crate::http::build_router(service, None)
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
