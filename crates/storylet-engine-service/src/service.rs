//! Application service coordinator for Storylet Studio.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex};

use storylet_engine::{
    ProjectBundle, RuntimeSessionState, StoryletRuntime, WorldCommand, WorldEvent,
};

use crate::contracts::{
    AuthoringOptions, GenerationJobRecord, LibraryPageResult,
    LibraryQuery, ProjectBundleContent, ProjectBundleV2, ProjectMetadata, ScenarioRecord,
    StoryletRecord,
};
use crate::generation::resources::StoryCraftResourceReference;
use crate::generation::resources::CraftRegistry;
use crate::generation::workflow::{get_stage_descriptors, GenerationService, GENERATION_STAGES};
use crate::projection::project_workspace;
use crate::scenario::resolve_scenario_start;
use crate::store::Store;

use crate::ai_generators::AiGeneratorService;

#[derive(Clone)]
pub struct StudioService {
    inner: Arc<Mutex<StudioInner>>,
    pub generation: Arc<GenerationService>,
    pub ai: Arc<AiGeneratorService>,
    pub updates: broadcast::Sender<SessionChange>,
    pub embedding_url: String,
    pub resource_root: PathBuf,
}

struct StudioInner {
    store: Store,
    runtime: StoryletRuntime,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionChange {
    pub session_id: String,
    pub revision: u64,
    pub snapshot: Value,
}

impl StudioService {
    pub fn open(
        db_path: impl AsRef<Path>,
        resource_root: impl Into<PathBuf>,
        strata_url: impl Into<String>,
        strata_model: impl Into<String>,
        embedding_url: impl Into<String>,
    ) -> Result<Self, String> {
        let store = Store::open(db_path).map_err(|e| e.to_string())?;
        Self::build(store, resource_root, strata_url, strata_model, embedding_url)
    }

    pub fn open_in_memory(
        resource_root: impl Into<PathBuf>,
        strata_url: impl Into<String>,
        strata_model: impl Into<String>,
        embedding_url: impl Into<String>,
    ) -> Result<Self, String> {
        let store = Store::open_in_memory().map_err(|e| e.to_string())?;
        Self::build(store, resource_root, strata_url, strata_model, embedding_url)
    }

    pub fn from_connection(
        conn: rusqlite::Connection,
    ) -> Result<Self, String> {
        let store = Store::from_connection(conn).map_err(|e| e.to_string())?;
        Self::build(store, ".", "http://127.0.0.1:8080/v1", "strata-uncensored", "http://127.0.0.1:4318")
    }

    fn build(
        store: Store,
        resource_root: impl Into<PathBuf>,
        strata_url: impl Into<String>,
        strata_model: impl Into<String>,
        embedding_url: impl Into<String>,
    ) -> Result<Self, String> {
        let mut runtime = StoryletRuntime::default();

        let mut stmt = store.conn
            .prepare("SELECT state_json FROM runtime_sessions ORDER BY session_id")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;

        for enc_res in rows {
            let enc = enc_res.map_err(|e| e.to_string())?;
            let state: RuntimeSessionState = serde_json::from_str(&enc)
                .map_err(|e| format!("saved session JSON is invalid: {e}"))?;

            validate_event_history(&store.conn, &state)?;
            runtime.restore_session(state).map_err(|e| format!("saved session cannot be restored: {e}"))?;
        }
        drop(stmt);

        // Recover any interrupted running generation jobs to ready
        let _ = store.conn.execute(
            "UPDATE generation_jobs SET status = 'ready', revision = revision + 1, updated_at = ?1 WHERE status = 'running'",
            rusqlite::params![chrono_now_ms()],
        );

        let (updates, _) = broadcast::channel(256);
        let res_root = resource_root.into();
        let gen_service = GenerationService::new(res_root.clone(), strata_url, strata_model);

        let litellm_url = std::env::var("LITELLM_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:4000/v1".into());
        let comfyui_url = std::env::var("COMFYUI_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8188".into());
        let ai = Arc::new(AiGeneratorService::new(litellm_url, comfyui_url));

        Ok(Self {
            inner: Arc::new(Mutex::new(StudioInner { store, runtime })),
            generation: Arc::new(gen_service),
            ai,
            updates,
            embedding_url: embedding_url.into(),
            resource_root: res_root,
        })
    }

    // --- Project Settings & Bundles ---

    pub async fn get_project_metadata(&self) -> Result<ProjectMetadata, String> {
        let inner = self.inner.lock().await;
        inner.store.get_project_metadata().map_err(|e| e.to_string())
    }

    pub async fn update_project_name(&self, name: &str) -> Result<ProjectMetadata, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Project name cannot be empty".into());
        }
        let inner = self.inner.lock().await;
        let mut meta = inner.store.get_project_metadata().map_err(|e| e.to_string())?;
        meta.name = trimmed.to_string();
        meta.revision += 1;
        inner.store.save_project_metadata(&meta).map_err(|e| e.to_string())?;
        Ok(meta)
    }

    pub async fn export_playtest_session(&self, session_id: &str) -> Result<RuntimeSessionState, String> {
        let inner = self.inner.lock().await;
        inner.runtime.export_session(session_id).map_err(|e| e.to_string())
    }

    pub async fn restore_playtest_session(&self, state: RuntimeSessionState) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;
        let session_id = state.session_id.clone();
        let snapshot = inner.runtime.restore_session(state).map_err(|e| e.to_string())?;
        persist_session_to_store(&mut inner, &session_id)?;

        let snap_val = serde_json::to_value(&snapshot).map_err(|e| e.to_string())?;
        let _ = self.updates.send(SessionChange {
            session_id: session_id.clone(),
            revision: snapshot.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snapshot }),
        });

        Ok(json!({ "snapshot": snap_val }))
    }

    pub async fn apply_semantic_rank(
        &self,
        session_id: &str,
        slope: f64,
        req: storylet_engine::SemanticRankingRequest,
    ) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let res = inner.runtime.rank_semantic(session_id, slope, req).map_err(|e| e.to_string())?;
        serde_json::to_value(res).map_err(|e| e.to_string())
    }

    pub async fn export_project_bundle(&self) -> Result<ProjectBundleV2, String> {
        let inner = self.inner.lock().await;
        let project = inner.store.get_project_metadata().map_err(|e| e.to_string())?;

        let qualities = inner.store.list_qualities().map_err(|e| e.to_string())?;
        let locations = inner.store.list_locations().map_err(|e| e.to_string())?;
        let characters = inner.store.list_characters().map_err(|e| e.to_string())?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let npc_behaviors = inner.store.list_npc_behaviors().map_err(|e| e.to_string())?;
        let events = inner.store.list_events().map_err(|e| e.to_string())?;
        let scenarios = inner.store.list_scenarios().map_err(|e| e.to_string())?;
        let generation_jobs = inner.store.list_generation_jobs().map_err(|e| e.to_string())?;

        Ok(ProjectBundleV2 {
            format: "decision-narrative-project".into(),
            schema_version: 2,
            project,
            content: ProjectBundleContent {
                qualities,
                locations,
                characters,
                storylets,
                npc_behaviors,
                events,
                scenarios,
                generation_jobs,
            },
        })
    }

    pub async fn validate_bundle(&self, bundle_val: &Value) -> Result<Vec<String>, String> {
        let mut issues = Vec::new();

        let bundle: ProjectBundleV2 = match serde_json::from_value(bundle_val.clone()) {
            Ok(b) => b,
            Err(e) => {
                issues.push(format!("Bundle does not match schema version 2: {e}"));
                return Ok(issues);
            }
        };

        if bundle.format != "decision-narrative-project" {
            issues.push(format!("Unsupported format '{}'", bundle.format));
        }
        if bundle.schema_version != 2 {
            issues.push(format!("Unsupported schema_version {}; expected 2", bundle.schema_version));
        }

        // Validate content through projection
        let res = project_workspace(
            &bundle.project.id,
            &bundle.project.name,
            bundle.project.revision,
            &bundle.content.qualities,
            &bundle.content.locations,
            &bundle.content.characters,
            &bundle.content.storylets,
            &bundle.content.npc_behaviors,
            &bundle.content.events,
            bundle.content.scenarios.first(),
        );

        issues.extend(res.issues);
        Ok(issues)
    }

    pub async fn import_bundle(&self, bundle_val: &Value) -> Result<(), String> {
        let bundle: ProjectBundleV2 = serde_json::from_value(bundle_val.clone())
            .map_err(|e| format!("Invalid bundle JSON: {e}"))?;

        let mut inner = self.inner.lock().await;
        inner.store
            .replace_project_content(&bundle.project, &bundle.content)
            .map_err(|e| e.to_string())
    }

    // --- Content CRUD ---

    pub async fn list_content(&self, entity: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let list = inner.store.list_content_json(entity).map_err(|e| e.to_string())?;
        let items: Vec<Value> = list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect();
        Ok(Value::Array(items))
    }

    pub async fn get_content_item(&self, entity: &str, id: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let opt = inner.store.get_content_json(entity, id).map_err(|e| e.to_string())?;
        opt.and_then(|s| serde_json::from_str(&s).ok())
            .ok_or_else(|| format!("{entity} item '{id}' not found"))
    }

    pub async fn save_content_item(&self, entity: &str, payload: &Value) -> Result<Value, String> {
        let id = match entity {
            "qualities" => payload.get("name").and_then(|v| v.as_str()).ok_or("Quality must have a 'name' field")?,
            _ => payload.get("id").and_then(|v| v.as_str()).ok_or("Item must have an 'id' field")?,
        };

        let inner = self.inner.lock().await;
        let mut final_payload = payload.clone();
        let (revision, approved_rev) = if entity == "storylets" {
            let existing_opt = inner.store.get_storylet(id).map_err(|e| e.to_string())?;
            let next_rev = match existing_opt {
                Some(prev) => prev.revision + 1,
                None => payload.get("revision").and_then(|v| v.as_u64()).unwrap_or(1),
            };
            if let Some(obj) = final_payload.as_object_mut() {
                obj.insert("revision".into(), Value::from(next_rev));
                obj.insert("approved_revision".into(), Value::Null);
            }
            (next_rev, None)
        } else {
            let rev = payload.get("revision").and_then(|v| v.as_u64()).unwrap_or(1);
            let app_rev = payload.get("approved_revision").and_then(|v| v.as_u64());
            (rev, app_rev)
        };

        let json_str = serde_json::to_string(&final_payload).map_err(|e| e.to_string())?;
        inner.store.save_content(entity, id, &json_str, revision, approved_rev).map_err(|e| e.to_string())?;
        Ok(final_payload)
    }

    pub async fn delete_content_item(&self, entity: &str, id: &str) -> Result<bool, String> {
        let inner = self.inner.lock().await;
        inner.store.delete_content(entity, id).map_err(|e| e.to_string())
    }

    pub async fn approve_storylet(&self, id: &str, expected_revision: u64) -> Result<StoryletRecord, String> {
        let inner = self.inner.lock().await;
        inner.store.approve_storylet(id, expected_revision)?
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
        let locations = inner.store.list_locations().map_err(|e| e.to_string())?;
        let characters = inner.store.list_characters().map_err(|e| e.to_string())?;

        let resolved = resolve_scenario_start(&scenario, &qualities, &locations, &characters);
        serde_json::to_value(&resolved).map_err(|e| e.to_string())
    }

    pub async fn get_authoring_options(&self) -> AuthoringOptions {
        AuthoringOptions {
            quality_types: vec!["currency".into(), "metric".into(), "tool".into(), "territory".into(), "menace".into()],
            repeatability_modes: vec!["always".into(), "once_ever".into(), "cooldown".into(), "persistent".into()],
            cluster_types: vec!["player_controlled_passage".into(), "linear".into(), "wheel".into(), "aftermath".into()],
            policy_modes: vec!["fixed".into(), "random".into(), "by_attribute".into()],
            event_types: vec!["canon".into(), "world".into()],
            stages: get_stage_descriptors(),
        }
    }

    // --- Craft Resources & Generation ---

    pub async fn list_craft_resources(&self) -> Result<Vec<StoryCraftResourceReference>, String> {
        let registry = CraftRegistry::new(&self.resource_root);
        registry.catalog()
    }

    pub async fn list_generation_jobs(&self) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let list = inner.store.list_generation_jobs().map_err(|e| e.to_string())?;
        serde_json::to_value(list).map_err(|e| e.to_string())
    }

    pub async fn get_generation_job(&self, id: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let job = inner.store.get_generation_job(id).map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Generation job '{id}' not found"))?;
        serde_json::to_value(job).map_err(|e| e.to_string())
    }

    pub async fn create_generation_job(&self, request: Value) -> Result<GenerationJobRecord, String> {
        let now = chrono_now_ms();
        let job = GenerationJobRecord {
            id: uuid::Uuid::new_v4().to_string(),
            revision: 0,
            status: "ready".into(),
            next_stage_index: 0,
            request,
            stages: Vec::new(),
            attempts: BTreeMap::new(),
            total_retries: 0,
            draft_storylet_id: None,
            draft_revision: None,
            failure: None,
            created_at: now,
            updated_at: now,
        };
        {
            let inner = self.inner.lock().await;
            inner.store.save_generation_job(&job).map_err(|e| e.to_string())?;
        }
        self.generation.notify_job_update(&job);
        Ok(job)
    }

    pub async fn advance_generation_job(&self, id: &str, expected_revision: u64) -> Result<GenerationJobRecord, String> {
        let (mut job, stage_id) = {
            let inner = self.inner.lock().await;
            let mut j = inner.store.get_generation_job(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Generation job '{id}' not found"))?;

            if j.revision != expected_revision {
                return Err(format!("Revision conflict: job revision is {}, expected {}", j.revision, expected_revision));
            }
            if j.status == "complete" {
                return Err("This generation job is already complete.".into());
            }
            if j.status == "running" {
                return Err("A generation stage is already running.".into());
            }
            if j.status == "failed" {
                return Err("This job reached its retry limit. Edit an earlier stage to resume it.".into());
            }
            if j.next_stage_index >= GENERATION_STAGES.len() {
                return Err("No generation stages remain.".into());
            }

            let (s_id, _) = GENERATION_STAGES[j.next_stage_index];
            let attempts = j.attempts.get(s_id).copied().unwrap_or(0) + 1;
            if attempts > 3 || j.total_retries >= 10 {
                j.status = "failed".into();
                j.failure = Some(json!({ "stage": s_id, "message": "Attempt limit reached" }));
                j.revision += 1;
                j.updated_at = chrono_now_ms();
                inner.store.save_generation_job(&j).map_err(|e| e.to_string())?;
                return Err(format!("Stage '{s_id}' reached attempt limit"));
            }

            j.attempts.insert(s_id.to_string(), attempts);
            if attempts > 1 { j.total_retries += 1; }
            j.status = "running".into();
            j.failure = None;
            j.revision += 1;
            j.updated_at = chrono_now_ms();
            inner.store.save_generation_job(&j).map_err(|e| e.to_string())?;
            (j, s_id)
        };

        self.generation.notify_job_update(&job);

        let stage_res = self.generation.execute_stage(stage_id, &job.request, &job.stages).await;
        let inner = self.inner.lock().await;
        match stage_res {
            Ok(rec) => {
                job.stages.push(rec);
                job.next_stage_index += 1;
                job.status = if job.next_stage_index >= GENERATION_STAGES.len() { "complete".into() } else { "ready".into() };
                job.revision += 1;
                job.updated_at = chrono_now_ms();
                inner.store.save_generation_job(&job).map_err(|e| e.to_string())?;
                self.generation.notify_job_update(&job);
                Ok(job)
            }
            Err(err) => {
                job.status = "failed".into();
                job.failure = Some(json!({ "stage": stage_id, "message": err.clone() }));
                job.revision += 1;
                job.updated_at = chrono_now_ms();
                inner.store.save_generation_job(&job).map_err(|e| e.to_string())?;
                self.generation.notify_job_update(&job);
                Err(err)
            }
        }
    }

    pub async fn edit_generation_stage(&self, id: &str, stage: &str, expected_revision: u64, artifact: Value) -> Result<GenerationJobRecord, String> {
        let inner = self.inner.lock().await;
        let job = self.generation.edit_stage(&inner.store, id, stage, expected_revision, artifact)?;
        Ok(job)
    }

    pub async fn save_generation_draft(&self, id: &str) -> Result<StoryletRecord, String> {
        let inner = self.inner.lock().await;
        self.generation.save_job_draft(&inner.store, id)
    }

    pub async fn delete_generation_job(&self, id: &str) -> Result<bool, String> {
        let inner = self.inner.lock().await;
        inner.store.delete_generation_job(id).map_err(|e| e.to_string())
    }

    pub async fn generate_storylet_batch(
        &self,
        family: &str,
        count: usize,
        batch_index: u64,
        brief: &str,
    ) -> Result<Value, String> {
        let count = count.clamp(1, 24);
        let endpoint = format!("{}/chat/completions", self.generation.strata_url.trim_end_matches('/'));
        let prompt = format!(
            "Create exactly {} diverse storylet candidates for scenario family {}.\n\
             Each candidate must match the StoryletRecord shape:\n\
             {{\"storylets\":[{{\"title\":string,\"location\"?:string,\"group_tag\"?:string,\"cluster_type\":\"linear\"|\"player_controlled_passage\"|\"wheel\"|\"aftermath\",\"urgency_weight\":number,\"repeatability\":\"once_ever\"|\"always\"|\"cooldown\"|\"persistent\",\"cooldown_turns\":number,\"parameters\":[{{\"param_name\":string,\"query\":string}}],\"preconditions\":{{\"all_of\"?:[{{\"variable\":string,\"op\":string,\"value\":any}}]}},\"content\":{{\"text_template\":string,\"sceneOpening\"?:string}},\"choices\":[{{\"choice_id\":string,\"label\":string,\"systemVerb\"?:string,\"effects\":object,\"divert_target\"?:string}}],\"effects\":object}}]}}\n\
             Project brief: {}\n\
             Include ending as a required boolean; set true only for the designated terminal storylet. Do not include id, revision, approved_revision, or created_at; the server assigns these deterministically.\n\
             Return only JSON with the top-level 'storylets' key.",
            count, family, brief
        );

        let body = json!({
            "model": self.generation.strata_model,
            "temperature": 0.7,
            "max_tokens": (750 * count).min(12000),
            "messages": [
                { "role": "system", "content": "You are a specialized narrative designer generating storylet content candidates. Return only valid JSON." },
                { "role": "user", "content": prompt }
            ]
        });

        let resp = self.generation.http_client.post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Strata request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Strata returned HTTP status {}", resp.status()));
        }

        let resp_data: Value = resp.json().await.map_err(|e| format!("Failed to parse Strata JSON: {e}"))?;
        let text = resp_data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|m| m.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let parsed: Value = extract_json_from_strata(text)?;
        let rows = parsed.get("storylets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Strata output must contain a 'storylets' array".to_string())?;

        if rows.len() != count {
            return Err(format!("Expected exactly {count} storylets from Strata, but received {}", rows.len()));
        }

        let now = chrono_now_ms();
        let mut results = Vec::new();

        for (idx, row) in rows.iter().enumerate() {
            let title = row.get("title").and_then(|t| t.as_str()).unwrap_or("Generated Scene");
            let ending = row.get("ending").and_then(|e| e.as_bool()).unwrap_or(false);
            let location = row.get("location").and_then(|l| l.as_str()).map(|s| s.to_string());
            let group_tag = row.get("group_tag").and_then(|g| g.as_str()).map(|s| s.to_string());
            let cluster_type = row.get("cluster_type").and_then(|c| c.as_str()).unwrap_or("player_controlled_passage");
            let urgency = row.get("urgency_weight").and_then(|u| u.as_f64()).unwrap_or(50.0);
            let repeatability = row.get("repeatability").and_then(|r| r.as_str()).unwrap_or("always");
            let cooldown = row.get("cooldown_turns").and_then(|c| c.as_u64()).unwrap_or(0);
            let content_obj = row.get("content").cloned().unwrap_or(json!({ "text_template": "" }));
            let preconditions = row.get("preconditions").cloned().unwrap_or(json!({}));
            let choices = row.get("choices").cloned().unwrap_or(json!([]));
            let effects = row.get("effects").cloned().unwrap_or(json!({}));
            let parameters = row.get("parameters").cloned().unwrap_or(json!([]));

            let id = format!("stl_{family}_{batch_index:03}_{:04}", idx + 1);

            results.push(json!({
                "id": id,
                "revision": 1,
                "approved_revision": Value::Null,
                "title": title,
                "ending": ending,
                "location": location,
                "group_tag": group_tag,
                "cluster_type": cluster_type,
                "urgency_weight": urgency,
                "repeatability": repeatability,
                "cooldown_turns": cooldown,
                "parameters": parameters,
                "preconditions": preconditions,
                "content": content_obj,
                "choices": choices,
                "effects": effects,
                "created_at": now,
            }));
        }

        Ok(Value::Array(results))
    }

    pub async fn generate_ranking_batch(
        &self,
        family: &str,
        count: usize,
        batch_index: u64,
        brief: &str,
    ) -> Result<Value, String> {
        let count = count.clamp(1, 24);
        let endpoint = format!("{}/chat/completions", self.generation.strata_url.trim_end_matches('/'));
        let prompt = format!(
            "Generate exactly {} synthetic storylet ranking training pairs for scenario family {}.\n\
             Each row must match model/ranking-example.schema.json with 9 bounded features [0.0 - 1.0]:\n\
             semantic_relevance, urgency, pacing_arc_alignment, novelty, waiting_time, goal_progress, role_binding_quality, predicted_consequence_alignment, no_action.\n\
             Brief: {}\n\
             Return only JSON with top-level 'examples' array.",
            count, family, brief
        );

        let body = json!({
            "model": self.generation.strata_model,
            "temperature": 0.7,
            "max_tokens": (600 * count).min(10000),
            "messages": [
                { "role": "system", "content": "You are a training-data generator for storylet ranking heads. Return only valid JSON with 'examples' array." },
                { "role": "user", "content": prompt }
            ]
        });

        let resp = self.generation.http_client.post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Strata request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Strata returned HTTP status {}", resp.status()));
        }

        let resp_data: Value = resp.json().await.map_err(|e| format!("Failed to parse Strata JSON: {e}"))?;
        let text = resp_data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|m| m.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let parsed: Value = extract_json_from_strata(text)?;
        let rows = parsed.get("examples")
            .or_else(|| parsed.as_array().map(|_| &parsed))
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Strata output must contain an 'examples' array".to_string())?;

        if rows.len() != count {
            return Err(format!("Expected exactly {count} ranking examples from Strata, but received {}", rows.len()));
        }

        let mut results = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            let id = format!("rnk_{family}_{batch_index:03}_{:04}", idx + 1);
            let mut item = row.clone();
            if let Some(obj) = item.as_object_mut() {
                obj.insert("example_id".into(), Value::String(id));
                obj.insert("split".into(), Value::String("train".into()));
                obj.insert("label_source".into(), Value::String("strata_synthetic".into()));
            }
            results.push(item);
        }

        Ok(Value::Array(results))
    }

    // --- Playtests & Runtime Engine Integration ---

    pub async fn create_playtest_session(
        &self,
        session_id: &str,
        project: ProjectBundle,
        scenario_id: &str,
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;
        ensure_session_id_has_no_archive(&inner.store.conn, session_id)?;

        let snapshot = inner.runtime
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

        let meta = inner.store.get_project_metadata().map_err(|e| e.to_string())?;
        let qualities = inner.store.list_qualities().map_err(|e| e.to_string())?;
        let locations = inner.store.list_locations().map_err(|e| e.to_string())?;
        let characters = inner.store.list_characters().map_err(|e| e.to_string())?;
        let storylets = inner.store.list_storylets().map_err(|e| e.to_string())?;
        let npc_behaviors = inner.store.list_npc_behaviors().map_err(|e| e.to_string())?;
        let events = inner.store.list_events().map_err(|e| e.to_string())?;

        let mut scenario_rec: Option<ScenarioRecord> = None;
        if let Some(s_id) = scenario_id {
            if let Some(json_s) = inner.store.get_content_json("scenarios", s_id).map_err(|e| e.to_string())? {
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
            &locations,
            &characters,
            &storylets,
            &npc_behaviors,
            &events,
            scenario_rec.as_ref(),
        );

        if !proj.issues.is_empty() {
            return Err(format!("Workspace has issues before starting playtest: {}", proj.issues.join("; ")));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let snapshot = inner.runtime
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

        let rows = stmt.query_map([], |row| {
            let enc: String = row.get(4)?;
            let state: Option<RuntimeSessionState> = serde_json::from_str(&enc).ok();
            let curr_loc = state.as_ref().and_then(|s| s.world.current_location.clone());
            let ev_count = state.as_ref().map(|s| s.world.events.len()).unwrap_or(0);

            let scen_id = state.as_ref().map(|s| s.scenario_id.clone()).unwrap_or_default();
            Ok(json!({
                "session_id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "scenario_id": scen_id,
                "revision": row.get::<_, String>(2)?.parse::<u64>().unwrap_or(0),
                "turn": row.get::<_, String>(3)?.parse::<u64>().unwrap_or(0),
                "current_location": curr_loc,
                "event_count": ev_count,
            }))
        }).map_err(|e| e.to_string())?;

        let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
        Ok(Value::Array(items))
    }

    pub async fn get_playtest_snapshot(&self, session_id: &str) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let snap = inner.runtime.snapshot(session_id).map_err(|e| e.to_string())?;
        serde_json::to_value(snap).map_err(|e| e.to_string())
    }

    pub async fn apply_director_decision(
        &self,
        session_id: &str,
        slope: f64,
        decision: storylet_engine::DecisionResult,
    ) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let res = inner.runtime.apply_decision(session_id, slope, decision).map_err(|e| e.to_string())?;
        serde_json::to_value(res).map_err(|e| e.to_string())
    }

    pub async fn evaluate_playtest_director(&self, session_id: &str, slope: f64) -> Result<Value, String> {
        let inner = self.inner.lock().await;
        let eval = inner.runtime.evaluate(session_id, slope).map_err(|e| e.to_string())?;
        serde_json::to_value(eval).map_err(|e| e.to_string())
    }

    pub async fn rank_playtest_with_local_encoder(&self, session_id: &str, slope: f64) -> Result<Value, String> {
        let (session, evaluation) = {
            let inner = self.inner.lock().await;
            let sess = inner.runtime.export_session(session_id).map_err(|e| e.to_string())?;
            let eval = inner.runtime.evaluate(session_id, slope).map_err(|e| e.to_string())?;
            (sess, eval)
        };

        let req = crate::ranking::build_semantic_ranking_request(&session, &evaluation, &self.embedding_url).await?;

        let eval = {
            let inner = self.inner.lock().await;
            inner.runtime.rank_semantic(session_id, slope, req).map_err(|e| e.to_string())?
        };

        serde_json::to_value(eval).map_err(|e| e.to_string())
    }

    pub async fn apply_playtest_command(&self, session_id: &str, cmd_val: &Value) -> Result<Value, String> {
        let cmd: WorldCommand = serde_json::from_value(cmd_val.clone())
            .map_err(|e| format!("Invalid command JSON: {e}"))?;

        let mut inner = self.inner.lock().await;
        let prior_export = inner.runtime.export_session(session_id).map_err(|e| e.to_string())?;

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
            let snap = inner.runtime.snapshot(session_id).map_err(|e| e.to_string())?;
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
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;
        let prior_export = inner.runtime.export_session(session_id).map_err(|e| e.to_string())?;

        let res = match inner.runtime.execute_choice(session_id, storylet_id, choice_id, idempotency_key, expected_revision) {
            Ok(r) => r,
            Err(e) => return Err(e.to_string()),
        };

        if let Err(err) = persist_session_to_store(&mut inner, session_id) {
            let _ = inner.runtime.delete_session(session_id);
            let _ = inner.runtime.restore_session(prior_export);
            return Err(format!("Failed to persist choice: {err}"));
        }

        let snap = inner.runtime.snapshot(session_id).map_err(|e| e.to_string())?;
        let _ = self.updates.send(SessionChange {
            session_id: session_id.to_string(),
            revision: snap.world.revision,
            snapshot: json!({ "sessionId": session_id, "snapshot": snap }),
        });

        serde_json::to_value(res).map_err(|e| e.to_string())
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
        let snap = inner.runtime.fork_session(source_session_id, branch_id.clone(), event_id).map_err(|e| e.to_string())?;
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
        let mut stmt = inner.store.conn.prepare(
            "SELECT event_json FROM runtime_events WHERE session_id = ?1 ORDER BY rowid",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        let events: Vec<Value> = rows.filter_map(|r| r.ok()).filter_map(|s| serde_json::from_str(&s).ok()).collect();
        Ok(Value::Array(events))
    }

    pub async fn delete_playtest(&self, session_id: &str) -> Result<bool, String> {
        let mut inner = self.inner.lock().await;
        inner.runtime.delete_session(session_id);
        inner.store.conn.execute("DELETE FROM runtime_sessions WHERE session_id = ?1", [session_id]).map_err(|e| e.to_string())?;
        Ok(true)
    }
}

fn persist_session_to_store(inner: &mut StudioInner, session_id: &str) -> Result<(), String> {
    let saved = inner.runtime.export_session(session_id).map_err(|e| e.to_string())?;
    let state_json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;

    let tx = inner.store.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;

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
            let existing: Option<String> = tx.query_row(
                "SELECT event_json FROM runtime_events WHERE session_id = ?1 AND event_id = ?2",
                rusqlite::params![saved.session_id, ev.id],
                |r| r.get(0),
            ).optional().map_err(|e| e.to_string())?;

            match existing {
                Some(prev_json) if prev_json != ev_json => {
                    return Err(format!("event '{}' conflicts with archived session history", ev.id));
                }
                Some(_) => continue,
                None => {
                    insert_ev.execute(rusqlite::params![saved.session_id, ev.id, ev.revision.to_string(), ev_json])
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn chrono_now_ms() -> i64 {
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

fn validate_event_history(conn: &rusqlite::Connection, state: &RuntimeSessionState) -> Result<(), String> {
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
        return Err(format!("saved session '{}' event journal does not match its snapshot", state.session_id));
    }
    Ok(())
}

fn ensure_session_id_has_no_archive(conn: &rusqlite::Connection, session_id: &str) -> Result<(), String> {
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM runtime_events WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    
    if count > 0 {
        return Err(format!("session '{session_id}' has archived event history and cannot be reused"));
    }
    Ok(())
}

fn extract_json_from_strata(text: &str) -> Result<Value, String> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Ok(v);
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<Value>(&text[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    Err("Strata response did not contain valid JSON object".into())
}
