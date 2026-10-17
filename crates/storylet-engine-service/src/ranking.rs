//! Adapter for optional local EmbeddingGemma / ranking head service.

use serde::Deserialize;
use serde_json::json;
use storylet_engine::{
    RankingHead, RuntimeEvaluation, RuntimeSessionState, SemanticCandidateEmbedding,
    SemanticRankingRequest,
};

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmbeddingResponse {
    pub model_revision: String,
    pub dimensions: usize,
    pub embeddings: Vec<Vec<f64>>,
}

pub async fn build_semantic_ranking_request(
    session: &RuntimeSessionState,
    evaluation: &RuntimeEvaluation,
    embedding_service_url: &str,
) -> Result<SemanticRankingRequest, String> {
    let eligible_ids: Vec<String> = evaluation
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.storylet_id.clone())
        .collect();

    if eligible_ids.is_empty() {
        return Err("No eligible storylets to rank in this session state.".into());
    }

    let query_text = build_scene_context(session);
    let candidate_docs: Vec<String> = eligible_ids
        .iter()
        .map(|id| build_storylet_doc(id, session))
        .collect();

    let client = reqwest::Client::new();
    let base_url = embedding_service_url.trim_end_matches('/');

    let query_resp = client
        .post(format!("{base_url}/v1/embeddings"))
        .json(&json!({
            "input": query_text,
            "input_type": "query"
        }))
        .send()
        .await
        .map_err(|e| format!("Embedding query request failed: {e}"))?;

    let query_data: EmbeddingResponse = query_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse query embedding: {e}"))?;

    let query_embedding = query_data
        .embeddings
        .first()
        .ok_or_else(|| "Empty query embedding array".to_string())?
        .clone();

    let cand_resp = client
        .post(format!("{base_url}/v1/embeddings"))
        .json(&json!({
            "input": candidate_docs,
            "input_type": "document"
        }))
        .send()
        .await
        .map_err(|e| format!("Embedding candidate request failed: {e}"))?;

    let cand_data: EmbeddingResponse = cand_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse candidate embeddings: {e}"))?;

    let ranking_head: Option<RankingHead> = if let Ok(head_resp) = client
        .get(format!("{base_url}/v1/ranking-head"))
        .send()
        .await
    {
        if head_resp.status().is_success() {
            head_resp.json().await.ok()
        } else {
            None
        }
    } else {
        None
    };

    let candidates: Vec<SemanticCandidateEmbedding> = eligible_ids
        .iter()
        .zip(cand_data.embeddings)
        .map(|(id, emb)| {
            let eval_c = evaluation.candidates.iter().find(|c| c.storylet_id == *id);
            let features = compute_candidate_features(eval_c, id, session);
            SemanticCandidateEmbedding {
                storylet_id: id.clone(),
                embedding: emb,
                features,
            }
        })
        .collect();

    Ok(SemanticRankingRequest {
        format_version: 1,
        context_revision: session.world.revision,
        model_version: query_data.model_revision,
        query_embedding,
        ranking_head,
        candidates,
    })
}

fn build_scene_context(session: &RuntimeSessionState) -> String {
    let loc_id = session.world.current_location.as_deref().unwrap_or("unset");
    let loc_name = session
        .project
        .locations
        .get(loc_id)
        .map(|l| l.name.as_str())
        .unwrap_or(loc_id);

    let actors = session
        .world
        .actors
        .iter()
        .map(|(id, a)| {
            let name = session
                .project
                .characters
                .get(id)
                .map(|c| c.name.as_str())
                .unwrap_or(id);
            format!("{name} at {}", a.current_location)
        })
        .collect::<Vec<_>>()
        .join("; ");

    let qualities = session
        .world
        .qualities
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ");

    format!("Location: {loc_name}. Characters: {actors}. World: {qualities}.")
}

fn build_storylet_doc(id: &str, session: &RuntimeSessionState) -> String {
    if let Some(st) = session.project.storylets.get(id) {
        let choices = st
            .choices
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            "Scene: {}. Summary: {}. Opening: {}. Choices: {}.",
            st.title, st.summary, st.content.opening, choices
        )
    } else {
        id.to_string()
    }
}

fn compute_candidate_features(
    eval_c: Option<&storylet_engine::CandidateEvaluation>,
    id: &str,
    session: &RuntimeSessionState,
) -> std::collections::BTreeMap<String, f64> {
    let mut map = std::collections::BTreeMap::new();
    let st = session.project.storylets.get(id);

    let urgency = st
        .map(|s| (s.urgency_weight / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let arc_score = eval_c.map(|c| c.arc_alignment_score).unwrap_or(0.5);

    let prior_count = session
        .world
        .events
        .iter()
        .filter(|e| e.action.contains(&format!("storylet:{id}:")))
        .count();
    let novelty = 1.0 / (1.0 + prior_count as f64);

    let req_count = st.map(|s| s.required_roles.len()).unwrap_or(0);
    let bound_count = eval_c.map(|c| c.bound_roles.len()).unwrap_or(0);
    let role_quality = if req_count > 0 {
        (bound_count as f64 / req_count as f64).clamp(0.0, 1.0)
    } else {
        1.0
    };

    map.insert("semantic_relevance".into(), 0.0);
    map.insert("urgency".into(), urgency);
    map.insert("pacing_arc_alignment".into(), arc_score);
    map.insert("novelty".into(), novelty);
    map.insert("waiting_time".into(), 0.5);
    map.insert("goal_progress".into(), 0.5);
    map.insert("role_binding_quality".into(), role_quality);
    map.insert("predicted_consequence_alignment".into(), 0.5);
    map.insert("no_action".into(), 0.0);

    map
}
