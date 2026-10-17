//! Resumable 9-stage storylet generation workflow and Strata pipeline client.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::contracts::{
    GenerationJobRecord, GenerationStageDescriptor, StoryletChoiceRecord, StoryletContentRecord,
    StoryletParameterRecord, StoryletRecord,
};
use crate::generation::resources::CraftRegistry;
use crate::store::Store;

pub const GENERATION_STAGES: &[(&str, &str)] = &[
    ("canon_and_voice", "Gather canon and voice constraints"),
    ("physical_seed", "Establish a physical scene seed"),
    ("premise_options", "Compare three distinct premises"),
    ("scene_plan", "Plan wants, conflict, subtext, and beats"),
    (
        "storylet_contract",
        "Shape the executable storylet contract",
    ),
    ("scene_variants", "Draft three contract-bound variants"),
    ("craft_audit", "Audit continuity, voice, and choices"),
    ("revision", "Revise the selected candidate"),
    ("review_package", "Prepare the approval package"),
];

pub fn get_stage_descriptors() -> Vec<GenerationStageDescriptor> {
    GENERATION_STAGES
        .iter()
        .map(|(id, title)| GenerationStageDescriptor {
            id: id.to_string(),
            title: title.to_string(),
        })
        .collect()
}

pub struct GenerationService {
    pub resource_root: PathBuf,
    pub strata_url: String,
    pub strata_model: String,
    pub http_client: reqwest::Client,
    pub job_events: broadcast::Sender<(String, Value)>,
}

impl GenerationService {
    pub fn new(
        resource_root: impl Into<PathBuf>,
        strata_url: impl Into<String>,
        strata_model: impl Into<String>,
    ) -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            resource_root: resource_root.into(),
            strata_url: strata_url.into(),
            strata_model: strata_model.into(),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            job_events: tx,
        }
    }

    pub fn notify_job_update(&self, job: &GenerationJobRecord) {
        let _ = self.job_events.send((
            job.id.clone(),
            serde_json::to_value(job).unwrap_or(Value::Null),
        ));
    }

    pub async fn create_job(
        &self,
        store: &Store,
        request: Value,
    ) -> Result<GenerationJobRecord, String> {
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
        store.save_generation_job(&job).map_err(|e| e.to_string())?;
        self.notify_job_update(&job);
        Ok(job)
    }

    pub async fn advance_stage(
        &self,
        store: &Store,
        id: &str,
        expected_revision: u64,
    ) -> Result<GenerationJobRecord, String> {
        let mut job = store
            .get_generation_job(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Generation job '{id}' not found"))?;

        if job.revision != expected_revision {
            return Err(format!(
                "Revision conflict: job revision is {}, expected {}",
                job.revision, expected_revision
            ));
        }

        if job.status == "complete" {
            return Err("This generation job is already complete.".into());
        }
        if job.status == "running" {
            return Err("A generation stage is already running.".into());
        }
        if job.status == "failed" {
            return Err(
                "This job reached its retry limit. Edit an earlier stage to resume it.".into(),
            );
        }

        let stage_idx = job.next_stage_index;
        if stage_idx >= GENERATION_STAGES.len() {
            return Err("No generation stages remain.".into());
        }

        let (stage_id, _) = GENERATION_STAGES[stage_idx];
        let current_attempts = job.attempts.get(stage_id).copied().unwrap_or(0);
        let next_attempt = current_attempts + 1;

        if next_attempt > 3 || job.total_retries >= 10 {
            job.status = "failed".into();
            job.failure = Some(json!({
                "stage": stage_id,
                "message": format!("Retry limit reached for stage '{stage_id}'"),
            }));
            job.revision += 1;
            job.updated_at = chrono_now_ms();
            store.save_generation_job(&job).map_err(|e| e.to_string())?;
            self.notify_job_update(&job);
            return Err(format!("Stage '{stage_id}' reached attempt limit"));
        }

        job.attempts.insert(stage_id.to_string(), next_attempt);
        if next_attempt > 1 {
            job.total_retries += 1;
        }

        job.status = "running".into();
        job.failure = None;
        job.revision += 1;
        job.updated_at = chrono_now_ms();
        store.save_generation_job(&job).map_err(|e| e.to_string())?;
        self.notify_job_update(&job);

        // Run the stage
        match self
            .execute_stage(stage_id, &job.request, &job.stages)
            .await
        {
            Ok(stage_record) => {
                job.stages.push(stage_record);
                job.next_stage_index += 1;
                job.status = if job.next_stage_index >= GENERATION_STAGES.len() {
                    "complete".into()
                } else {
                    "ready".into()
                };
                job.revision += 1;
                job.updated_at = chrono_now_ms();
                store.save_generation_job(&job).map_err(|e| e.to_string())?;
                self.notify_job_update(&job);
                Ok(job)
            }
            Err(err) => {
                job.status = "failed".into();
                job.failure = Some(json!({ "stage": stage_id, "message": err.clone() }));
                job.revision += 1;
                job.updated_at = chrono_now_ms();
                store.save_generation_job(&job).map_err(|e| e.to_string())?;
                self.notify_job_update(&job);
                Err(err)
            }
        }
    }

    pub fn edit_stage(
        &self,
        store: &Store,
        id: &str,
        stage: &str,
        expected_revision: u64,
        artifact: Value,
    ) -> Result<GenerationJobRecord, String> {
        let mut job = store
            .get_generation_job(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Generation job '{id}' not found"))?;

        if job.revision != expected_revision {
            return Err(format!(
                "Revision conflict: job revision is {}, expected {}",
                job.revision, expected_revision
            ));
        }

        if job.status == "running" {
            return Err("Wait for active stage before editing inputs.".into());
        }

        let stage_pos = GENERATION_STAGES
            .iter()
            .position(|(s, _)| *s == stage)
            .ok_or_else(|| format!("Unknown stage '{stage}'"))?;

        if stage_pos >= job.next_stage_index {
            return Err("Only completed stage outputs can be edited.".into());
        }

        // Truncate stages to this point
        job.stages.truncate(stage_pos);
        job.stages.push(json!({
            "stage": stage,
            "artifact": artifact,
            "source": "designer",
            "model": "human edit",
            "promptVersion": "manual-v1",
            "completedAt": chrono_now_ms(),
        }));

        job.next_stage_index = stage_pos + 1;
        for (s, _) in &GENERATION_STAGES[stage_pos..] {
            job.attempts.remove(*s);
        }

        job.status = if job.next_stage_index >= GENERATION_STAGES.len() {
            "complete".into()
        } else {
            "ready".into()
        };
        job.failure = None;
        job.revision += 1;
        job.updated_at = chrono_now_ms();
        store.save_generation_job(&job).map_err(|e| e.to_string())?;
        self.notify_job_update(&job);
        Ok(job)
    }

    pub fn save_job_draft(&self, store: &Store, id: &str) -> Result<StoryletRecord, String> {
        let mut job = store
            .get_generation_job(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Generation job '{id}' not found"))?;

        if job.status != "complete" {
            return Err("Only a complete generation job can produce a draft.".into());
        }

        // Extract storylet from final stage review_package or revision stage
        let final_stage = job
            .stages
            .last()
            .ok_or_else(|| "No stages completed".to_string())?;
        let artifact = final_stage.get("artifact").unwrap_or(&Value::Null);

        let storylet_val = artifact.get("storylet").unwrap_or(artifact);

        let req_title = job
            .request
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Generated Scene");
        let title = storylet_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(req_title)
            .to_string();
        let scene_opening = storylet_val
            .get("sceneOpening")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let text_template = storylet_val
            .get("text_template")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let req_location_tags: Vec<String> = job
            .request
            .get("locationTags")
            .and_then(|value| value.as_array())
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let cluster_type = job
            .request
            .get("clusterType")
            .and_then(|v| v.as_str())
            .unwrap_or("player_controlled_passage")
            .to_string();
        let urgency = job
            .request
            .get("urgencyWeight")
            .and_then(|v| v.as_f64())
            .unwrap_or(50.0);

        let mut choices = Vec::new();
        if let Some(choices_arr) = storylet_val.get("choices").and_then(|v| v.as_array()) {
            for (idx, ch) in choices_arr.iter().enumerate() {
                let choice_id = ch
                    .get("choice_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("choice_{}", idx + 1))
                    .to_string();
                let label = ch
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Proceed")
                    .to_string();
                let system_verb = ch
                    .get("systemVerb")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let effects = ch.get("effects").cloned().unwrap_or(json!({}));
                let divert_target = ch
                    .get("divert_target")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                choices.push(StoryletChoiceRecord {
                    choice_id,
                    label,
                    system_verb,
                    conditions: None,
                    effects,
                    divert_target,
                    advisor_preview: None,
                });
            }
        }

        let mut parameters = Vec::new();
        if let Some(roles_arr) = job.request.get("characterRoles").and_then(|v| v.as_array()) {
            for role_v in roles_arr {
                if let Some(role) = role_v.as_str() {
                    parameters.push(StoryletParameterRecord {
                        param_name: role.to_string(),
                        query: format!("role == \"{role}\""),
                    });
                }
            }
        }

        let st_id = format!("gen_{}", &job.id[..8]);
        let now = chrono_now_ms();

        let record = StoryletRecord {
            id: st_id.clone(),
            revision: 1,
            approved_revision: None,
            title,
            ending: false,
            position: None,
            size: None,
            location_tags: req_location_tags,
            group_tag: Some("generated".into()),
            cluster_type,
            urgency_weight: urgency,
            repeatability: "once_ever".into(),
            cooldown_turns: 0,
            parameters,
            preconditions: job
                .request
                .get("triggerPreconditions")
                .cloned()
                .unwrap_or(json!({})),
            content: StoryletContentRecord {
                text_template,
                speaker_id: None,
                scene_opening,
                dialogue_quips: None,
            },
            choices,
            effects: job
                .request
                .get("desiredOutcomes")
                .cloned()
                .unwrap_or(json!({})),
            local_variables: None,
            graph: None,
            created_at: now,
        };

        store.save_storylet(&record).map_err(|e| e.to_string())?;

        job.draft_storylet_id = Some(st_id);
        job.draft_revision = Some(1);
        job.revision += 1;
        job.updated_at = now;
        store.save_generation_job(&job).map_err(|e| e.to_string())?;
        self.notify_job_update(&job);

        Ok(record)
    }

    pub async fn execute_stage(
        &self,
        stage: &str,
        request: &Value,
        previous: &[Value],
    ) -> Result<Value, String> {
        let registry = CraftRegistry::new(&self.resource_root);
        let craft = registry.for_stage(stage)?;

        let endpoint = format!("{}/chat/completions", self.strata_url.trim_end_matches('/'));
        let stage_title = GENERATION_STAGES
            .iter()
            .find(|(s, _)| *s == stage)
            .map(|(_, t)| *t)
            .unwrap_or(stage);

        let prompt = format!(
            "{}\n\n{}\n\nDESIGNER BRIEF:\n{}\n\nPRIOR STAGE OUTPUTS:\n{}\n\nReturn only one JSON object with the stage result under the 'artifact' key.",
            stage_title,
            craft.prompt,
            serde_json::to_string_pretty(request).unwrap_or_default(),
            serde_json::to_string_pretty(previous).unwrap_or_default()
        );

        let body = json!({
            "model": self.strata_model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a careful narrative design partner. Follow project-owned facts and the designer's executable contract. Return only valid JSON with an 'artifact' object."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.7,
            "max_tokens": if stage == "scene_variants" || stage == "review_package" { 2800 } else { 1600 }
        });

        let resp_res = self.http_client.post(&endpoint).json(&body).send().await;
        match resp_res {
            Ok(resp) if resp.status().is_success() => {
                let data: Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse Strata JSON response: {e}"))?;
                let content_text = data
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|m| m.get("message"))
                    .and_then(|msg| msg.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let parsed_artifact = extract_artifact_json(content_text)?;
                let now = chrono_now_ms();

                Ok(json!({
                    "stage": stage,
                    "artifact": parsed_artifact,
                    "source": "strata",
                    "model": self.strata_model,
                    "promptVersion": "storylet-craft-v2",
                    "craftResources": craft.references,
                    "completedAt": now
                }))
            }
            Ok(resp) => Err(format!("Strata returned HTTP status {}", resp.status())),
            Err(e) => Err(format!("Strata request failed: {e}")),
        }
    }
}

fn extract_artifact_json(text: &str) -> Result<Value, String> {
    // Try full parse first
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        if let Some(art) = v.get("artifact") {
            return Ok(art.clone());
        }
        return Ok(v);
    }

    // Try finding JSON object in markdown code block
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let slice = &text[start..=end];
                if let Ok(v) = serde_json::from_str::<Value>(slice) {
                    if let Some(art) = v.get("artifact") {
                        return Ok(art.clone());
                    }
                    return Ok(v);
                }
            }
        }
    }

    Err("Strata response did not contain valid JSON object".into())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
