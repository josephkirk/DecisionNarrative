//! In-memory playtest sessions driven exclusively by the Rust engine.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    CandidateEvaluation, CommandResult, DirectorState, EngineError, NpcSimulationReport,
    ProjectBundle, ProjectIssue, WorldCommand, WorldState, advance_npcs, apply_command,
    build_world, evaluate_storylets, execute_storylet_choice, start_storylet,
};

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeError {
    EmptySessionId,
    SessionAlreadyExists { session_id: String },
    SessionNotFound { session_id: String },
    InvalidProject(Vec<ProjectIssue>),
    InvalidSessionState(String),
    UnknownLocation { location_id: String },
    UnknownQuality { key: String },
    Engine(EngineError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySessionId => write!(formatter, "session id must not be empty"),
            Self::SessionAlreadyExists { session_id } => {
                write!(formatter, "session '{session_id}' already exists")
            }
            Self::SessionNotFound { session_id } => {
                write!(formatter, "session '{session_id}' was not found")
            }
            Self::InvalidProject(issues) => {
                write!(
                    formatter,
                    "project validation failed with {} issue(s)",
                    issues.len()
                )
            }
            Self::InvalidSessionState(message) => {
                write!(formatter, "invalid persisted session: {message}")
            }
            Self::UnknownLocation { location_id } => {
                write!(
                    formatter,
                    "location '{location_id}' is not part of this project"
                )
            }
            Self::UnknownQuality { key } => {
                write!(formatter, "quality '{key}' is not part of this project")
            }
            Self::Engine(error) => error.fmt(formatter),
        }
    }
}

impl Error for RuntimeError {}

impl From<EngineError> for RuntimeError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub project_id: String,
    pub world: WorldState,
}

/// Versioned, host-neutral durable session state. Hosts may store this as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSessionState {
    pub format_version: u32,
    pub session_id: String,
    pub project_id: String,
    pub scenario_id: String,
    pub world: WorldState,
    pub storylet_ids: Vec<String>,
    pub project: ProjectBundle,
    pub director: DirectorState,
}

pub const RUNTIME_SESSION_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvaluation {
    pub candidates: Vec<CandidateEvaluation>,
    pub selected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionResult>,
}

/// Host- or model-produced ranking over the complete eligible set. Rust still
/// re-evaluates hard conditions against the current revision before accepting it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionResult {
    pub format_version: u32,
    pub context_revision: u64,
    pub source: String,
    pub model_version: String,
    pub confidence: f64,
    pub no_action_probability: f64,
    pub ranking: Vec<RankedStorylet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedStorylet {
    pub storylet_id: String,
    pub score: f64,
    pub probability: f64,
}

/// Exported pairwise ranking head trained from reviewed storylet preferences.
/// The key and weight names are versioned so the model service and host cannot
/// silently disagree about feature meaning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankingHead {
    pub schema_version: u32,
    pub model_version: String,
    pub feature_names: Vec<String>,
    pub weights: BTreeMap<String, f64>,
    pub bias: f64,
    pub temperature: f64,
    pub no_action_bias: f64,
}

/// Embeddings returned by an optional host-side encoder. The library performs
/// normalization, feature blending, probability calculation, and selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRankingRequest {
    pub format_version: u32,
    pub context_revision: u64,
    pub model_version: String,
    pub query_embedding: Vec<f64>,
    pub candidates: Vec<SemanticCandidateEmbedding>,
    #[serde(default)]
    pub ranking_head: Option<RankingHead>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticCandidateEmbedding {
    pub storylet_id: String,
    pub embedding: Vec<f64>,
    #[serde(default)]
    pub features: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCommandResult {
    pub player: CommandResult,
    pub npc: NpcSimulationReport,
    pub world: WorldState,
}

#[derive(Clone, Debug)]
struct PlaytestSession {
    project_id: String,
    scenario_id: String,
    world: WorldState,
    storylet_ids: Vec<String>,
    project: ProjectBundle,
    director: DirectorState,
}

/// Owns ephemeral sessions. A new runtime starts empty; sessions can be
/// snapshotted and restored by a host without coupling the engine to storage.
#[derive(Clone, Debug, Default)]
pub struct StoryletRuntime {
    sessions: BTreeMap<String, PlaytestSession>,
}

impl StoryletRuntime {
    pub fn create_session(
        &mut self,
        session_id: impl Into<String>,
        project: ProjectBundle,
        scenario_id: &str,
    ) -> Result<SessionSnapshot, RuntimeError> {
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(RuntimeError::EmptySessionId);
        }
        if self.sessions.contains_key(&session_id) {
            return Err(RuntimeError::SessionAlreadyExists { session_id });
        }
        let world = build_world(&project, scenario_id).map_err(RuntimeError::InvalidProject)?;
        let scenario = project
            .scenarios
            .get(scenario_id)
            .expect("build_world validates the scenario before returning");
        let snapshot = SessionSnapshot {
            session_id: session_id.clone(),
            project_id: project.id.clone(),
            world: world.clone(),
        };
        self.sessions.insert(
            session_id,
            PlaytestSession {
                project_id: project.id.clone(),
                scenario_id: scenario_id.to_owned(),
                world,
                storylet_ids: scenario.storylet_ids.clone(),
                project,
                director: DirectorState::default(),
            },
        );
        Ok(snapshot)
    }

    pub fn snapshot(&self, session_id: &str) -> Result<SessionSnapshot, RuntimeError> {
        let session = self.session(session_id)?;
        Ok(SessionSnapshot {
            session_id: session_id.to_owned(),
            project_id: session.project_id.clone(),
            world: session.world.clone(),
        })
    }

    pub fn export_session(&self, session_id: &str) -> Result<RuntimeSessionState, RuntimeError> {
        let session = self.session(session_id)?;
        Ok(RuntimeSessionState {
            format_version: RUNTIME_SESSION_FORMAT_VERSION,
            session_id: session_id.to_owned(),
            project_id: session.project_id.clone(),
            scenario_id: session.scenario_id.clone(),
            world: session.world.clone(),
            storylet_ids: session.storylet_ids.clone(),
            project: session.project.clone(),
            director: session.director.clone(),
        })
    }

    pub fn restore_session(
        &mut self,
        state: RuntimeSessionState,
    ) -> Result<SessionSnapshot, RuntimeError> {
        if state.format_version != RUNTIME_SESSION_FORMAT_VERSION {
            return Err(RuntimeError::InvalidSessionState(format!(
                "format version {} is unsupported",
                state.format_version
            )));
        }
        if state.session_id.trim().is_empty() {
            return Err(RuntimeError::EmptySessionId);
        }
        if self.sessions.contains_key(&state.session_id) {
            return Err(RuntimeError::SessionAlreadyExists {
                session_id: state.session_id,
            });
        }
        if state.project_id != state.project.id {
            return Err(RuntimeError::InvalidSessionState(
                "project id does not match the embedded project".into(),
            ));
        }
        let initial = build_world(&state.project, &state.scenario_id)
            .map_err(RuntimeError::InvalidProject)?;
        let scenario = state
            .project
            .scenarios
            .get(&state.scenario_id)
            .expect("build_world validates the scenario before returning");
        if state.storylet_ids != scenario.storylet_ids {
            return Err(RuntimeError::InvalidSessionState(
                "scenario storylet list has changed".into(),
            ));
        }
        let replayed = crate::replay_events(initial, &state.world.events)?;
        if replayed != state.world {
            return Err(RuntimeError::InvalidSessionState(
                "event log does not reproduce the saved world".into(),
            ));
        }
        let snapshot = SessionSnapshot {
            session_id: state.session_id.clone(),
            project_id: state.project_id.clone(),
            world: state.world.clone(),
        };
        self.sessions.insert(
            state.session_id,
            PlaytestSession {
                project_id: state.project_id,
                scenario_id: state.scenario_id,
                world: state.world,
                storylet_ids: state.storylet_ids,
                project: state.project,
                director: state.director,
            },
        );
        Ok(snapshot)
    }

    /// Remove a playtest session and all of its in-memory runtime state.
    pub fn delete_session(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    /// Fork a session at an existing event. The fork replays the prefix through
    /// the same transaction path and rebuilds storylet lifecycle state.
    pub fn fork_session(
        &mut self,
        source_session_id: &str,
        branch_session_id: impl Into<String>,
        event_id: &str,
    ) -> Result<SessionSnapshot, RuntimeError> {
        let branch_session_id = branch_session_id.into();
        if branch_session_id.trim().is_empty() {
            return Err(RuntimeError::EmptySessionId);
        }
        if self.sessions.contains_key(&branch_session_id) {
            return Err(RuntimeError::SessionAlreadyExists {
                session_id: branch_session_id,
            });
        }
        let source = self.session(source_session_id)?.clone();
        // Rebuild the director alongside the event prefix. Storylet choices are
        // replayed through their lifecycle transaction; all other events use
        // the ordinary world transaction.
        let mut replayed_world = build_world(&source.project, &source.scenario_id)
            .map_err(RuntimeError::InvalidProject)?;
        let mut director = DirectorState::default();
        let end = source
            .world
            .events
            .iter()
            .position(|event| event.id == event_id)
            .ok_or_else(|| {
                RuntimeError::Engine(EngineError::MissingCausalParent {
                    parent: event_id.to_owned(),
                })
            })?;
        for event in &source.world.events[..=end] {
            let storylet_choice = source.project.storylets.values().find_map(|definition| {
                definition
                    .choices
                    .iter()
                    .find(|choice| {
                        event.action == format!("storylet:{}:{}", definition.id, choice.id)
                            && event.effects == choice.effects
                            && event.actor_id == "player"
                    })
                    .map(|choice| (definition, choice.id.as_str()))
            });
            if let Some((definition, choice_id)) = storylet_choice {
                let candidate = evaluate_storylets(
                    std::slice::from_ref(definition),
                    &replayed_world,
                    &director,
                    0.15,
                )
                .into_iter()
                .next()
                .expect("one definition produces one candidate");
                let instance_id = format!(
                    "playtest:{branch_session_id}:{}:{}",
                    definition.id, replayed_world.revision
                );
                start_storylet(
                    definition,
                    &candidate,
                    &mut replayed_world,
                    &mut director,
                    &instance_id,
                )?;
                let execution = execute_storylet_choice(
                    definition,
                    &instance_id,
                    choice_id,
                    &event.id,
                    &mut replayed_world,
                    &mut director,
                )?;
                if execution.event != *event {
                    return Err(RuntimeError::Engine(EngineError::ReplayMismatch {
                        event_id: event.id.clone(),
                    }));
                }
            } else {
                let expected_revision = replayed_world.revision;
                let replayed = apply_command(
                    &mut replayed_world,
                    WorldCommand {
                        idempotency_key: event.id.clone(),
                        expected_revision,
                        actor_id: event.actor_id.clone(),
                        action: event.action.clone(),
                        effects: event.effects.clone(),
                        caused_by: event.caused_by.clone(),
                        advance_turn: event.advanced_turn,
                        terminal: event.terminal,
                    },
                )?;
                if replayed.event != *event {
                    return Err(RuntimeError::Engine(EngineError::ReplayMismatch {
                        event_id: event.id.clone(),
                    }));
                }
            }
        }
        let world = replayed_world;
        let snapshot = SessionSnapshot {
            session_id: branch_session_id.clone(),
            project_id: source.project_id.clone(),
            world: world.clone(),
        };
        self.sessions.insert(
            branch_session_id,
            PlaytestSession {
                project_id: source.project_id,
                scenario_id: source.scenario_id,
                world,
                storylet_ids: source.storylet_ids,
                project: source.project,
                director,
            },
        );
        Ok(snapshot)
    }

    pub fn evaluate(
        &self,
        session_id: &str,
        target_tension_slope: f64,
    ) -> Result<RuntimeEvaluation, RuntimeError> {
        let session = self.session(session_id)?;
        if session.world.is_terminal() {
            return Ok(RuntimeEvaluation {
                candidates: Vec::new(),
                selected: None,
                decision: None,
            });
        }
        let definitions: Vec<_> = session
            .storylet_ids
            .iter()
            .filter_map(|id| session.project.storylets.get(id).cloned())
            .collect();
        let candidates = evaluate_storylets(
            &definitions,
            &session.world,
            &session.director,
            target_tension_slope,
        );
        let selected = candidates
            .iter()
            .find(|candidate| candidate.eligible)
            .map(|candidate| candidate.storylet_id.clone());
        Ok(RuntimeEvaluation {
            candidates,
            selected,
            decision: None,
        })
    }

    /// Accept an external Jev/model ranking only when it describes this exact
    /// world revision and every currently eligible storylet exactly once.
    pub fn apply_decision(
        &self,
        session_id: &str,
        target_tension_slope: f64,
        mut decision: DecisionResult,
    ) -> Result<RuntimeEvaluation, RuntimeError> {
        let mut evaluation = self.evaluate(session_id, target_tension_slope)?;
        let session = self.session(session_id)?;
        let invalid = |message: &str| RuntimeError::InvalidSessionState(message.to_owned());
        if decision.format_version != 1 {
            return Err(invalid("unsupported decision result format"));
        }
        if decision.context_revision != session.world.revision {
            return Err(RuntimeError::Engine(EngineError::RevisionConflict {
                expected: decision.context_revision,
                actual: session.world.revision,
            }));
        }
        if decision.source.trim().is_empty()
            || decision.model_version.trim().is_empty()
            || !decision.confidence.is_finite()
            || !(0.0..=1.0).contains(&decision.confidence)
            || !decision.no_action_probability.is_finite()
            || !(0.0..=1.0).contains(&decision.no_action_probability)
        {
            return Err(invalid("decision metadata and probabilities must be valid"));
        }
        let eligible: BTreeMap<_, _> = evaluation
            .candidates
            .iter()
            .filter(|candidate| candidate.eligible)
            .map(|candidate| (candidate.storylet_id.clone(), candidate.clone()))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let mut probability_total = decision.no_action_probability;
        for item in &decision.ranking {
            if !item.score.is_finite()
                || !item.probability.is_finite()
                || !(0.0..=1.0).contains(&item.probability)
                || !seen.insert(item.storylet_id.clone())
                || !eligible.contains_key(&item.storylet_id)
            {
                return Err(invalid(
                    "ranking must contain unique eligible storylets and finite probabilities",
                ));
            }
            probability_total += item.probability;
        }
        if seen.len() != eligible.len() || (probability_total - 1.0).abs() > 1e-5 {
            return Err(invalid(
                "ranking must cover every eligible storylet and probabilities must sum to one",
            ));
        }

        decision.ranking.sort_by(|left, right| {
            right
                .probability
                .total_cmp(&left.probability)
                .then_with(|| right.score.total_cmp(&left.score))
                .then_with(|| left.storylet_id.cmp(&right.storylet_id))
        });

        let ranked_ids: Vec<_> = decision
            .ranking
            .iter()
            .map(|item| item.storylet_id.as_str())
            .collect();
        evaluation.candidates.sort_by(|left, right| {
            let left_rank = ranked_ids.iter().position(|id| *id == left.storylet_id);
            let right_rank = ranked_ids.iter().position(|id| *id == right.storylet_id);
            match (left_rank, right_rank) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => right.storylet_id.cmp(&left.storylet_id),
            }
        });
        let choice = decision
            .ranking
            .first()
            .filter(|item| item.probability > decision.no_action_probability);
        evaluation.selected = choice.map(|item| item.storylet_id.clone());
        evaluation.decision = Some(decision);
        Ok(evaluation)
    }

    /// Build and apply an uncalibrated semantic-baseline decision entirely in
    /// the Rust engine. The host supplies vectors, never weights or eligibility.
    pub fn rank_semantic(
        &self,
        session_id: &str,
        target_tension_slope: f64,
        request: SemanticRankingRequest,
    ) -> Result<RuntimeEvaluation, RuntimeError> {
        let evaluation = self.evaluate(session_id, target_tension_slope)?;
        let session = self.session(session_id)?;
        let invalid = |message: &str| RuntimeError::InvalidSessionState(message.to_owned());
        if request.format_version != 1 {
            return Err(invalid("unsupported semantic ranking format"));
        }
        if request.context_revision != session.world.revision {
            return Err(RuntimeError::Engine(EngineError::RevisionConflict {
                expected: request.context_revision,
                actual: session.world.revision,
            }));
        }
        if request.model_version.trim().is_empty() {
            return Err(invalid("semantic ranking requires a model version"));
        }
        let dimensions = request.query_embedding.len();
        if dimensions == 0 || dimensions > 4096 {
            return Err(invalid(
                "semantic vector dimension must be between 1 and 4096",
            ));
        }
        let query = normalized_embedding(&request.query_embedding, dimensions)?;
        let eligible: BTreeMap<_, _> = evaluation
            .candidates
            .iter()
            .filter(|candidate| candidate.eligible)
            .map(|candidate| (candidate.storylet_id.clone(), candidate.final_weight))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let expected_features = [
            "semantic_relevance",
            "urgency",
            "pacing_arc_alignment",
            "novelty",
            "waiting_time",
            "goal_progress",
            "role_binding_quality",
            "predicted_consequence_alignment",
            "no_action",
        ];
        let head = request.ranking_head.as_ref();
        if let Some(head) = head {
            if head.schema_version != 1
                || head.model_version.trim().is_empty()
                || head
                    .feature_names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    != expected_features
                || head.weights.len() != expected_features.len()
                || expected_features.iter().any(|name| {
                    head.weights
                        .get(*name)
                        .is_none_or(|weight| !weight.is_finite())
                })
                || !head.bias.is_finite()
                || !head.no_action_bias.is_finite()
                || !head.temperature.is_finite()
                || head.temperature <= 0.0
            {
                return Err(invalid(
                    "ranking head schema, weights, or calibration settings are invalid",
                ));
            }
        }
        let mut scores = Vec::with_capacity(request.candidates.len());
        let mut no_action_score = 0.55;
        let mut temperature = 0.12;
        for candidate in &request.candidates {
            let Some(base_weight) = eligible.get(&candidate.storylet_id) else {
                return Err(invalid(
                    "semantic vectors may only be supplied for eligible storylets",
                ));
            };
            if !seen.insert(candidate.storylet_id.clone()) {
                return Err(invalid("semantic ranking contains a duplicate storylet"));
            }
            let embedding = normalized_embedding(&candidate.embedding, dimensions)?;
            let cosine = query
                .iter()
                .zip(&embedding)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .clamp(-1.0, 1.0);
            let semantic = (cosine + 1.0) / 2.0;
            let heuristic = (base_weight / 100.0).clamp(0.0, 1.0);
            let score = if let Some(head) = head {
                if candidate.features.len() != expected_features.len()
                    || expected_features.iter().any(|name| {
                        candidate
                            .features
                            .get(*name)
                            .is_none_or(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                    })
                {
                    return Err(invalid(
                        "trained ranking requires a finite value from 0 to 1 for every candidate feature",
                    ));
                }
                let mut value = head.bias;
                for name in expected_features {
                    let feature = if name == "semantic_relevance" {
                        semantic
                    } else {
                        candidate.features[name]
                    };
                    value += head.weights[name] * feature;
                }
                value
            } else {
                0.75 * semantic + 0.25 * heuristic
            };
            scores.push((candidate.storylet_id.clone(), score));
        }
        if seen.len() != eligible.len() {
            return Err(invalid(
                "semantic ranking must include every eligible storylet",
            ));
        }

        if let Some(head) = head {
            no_action_score = head.bias + head.weights["no_action"] + head.no_action_bias;
            temperature = head.temperature;
        }
        let mut logits: Vec<_> = scores
            .iter()
            .map(|(_, score)| score / temperature)
            .collect();
        let no_action_logit = no_action_score / temperature;
        let max_logit = logits.iter().copied().fold(no_action_logit, f64::max);
        let no_action_weight = (no_action_logit - max_logit).exp();
        for logit in &mut logits {
            *logit = (*logit - max_logit).exp();
        }
        let total = no_action_weight + logits.iter().sum::<f64>();
        let no_action_probability = no_action_weight / total;
        let ranking = scores
            .into_iter()
            .zip(logits)
            .map(|((storylet_id, score), weight)| RankedStorylet {
                storylet_id,
                score,
                probability: weight / total,
            })
            .collect();
        self.apply_decision(
            session_id,
            target_tension_slope,
            DecisionResult {
                format_version: 1,
                context_revision: request.context_revision,
                source: if head.is_some() {
                    "trained_ranking_head_uncalibrated"
                } else {
                    "embedding_baseline_uncalibrated"
                }
                .into(),
                model_version: head.map_or(request.model_version.clone(), |head| {
                    format!("{}+{}", request.model_version, head.model_version)
                }),
                confidence: 0.0,
                no_action_probability,
                ranking,
            },
        )
    }

    pub fn submit_command(
        &mut self,
        session_id: &str,
        command: WorldCommand,
    ) -> Result<RuntimeCommandResult, RuntimeError> {
        let session =
            self.sessions
                .get_mut(session_id)
                .ok_or_else(|| RuntimeError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;

        if session.world.is_terminal()
            && !session
                .world
                .applied_commands
                .contains_key(&command.idempotency_key)
        {
            return Err(RuntimeError::Engine(EngineError::SessionEnded));
        }
        validate_command_references(&session.project, &command)?;

        // Resolve the player action and all resulting NPC actions against a
        // working snapshot. A failed NPC effect cannot leave half a turn.
        let mut world = session.world.clone();
        let player = apply_command(&mut world, command)?;
        let npc = if player.event.advanced_turn && !player.duplicate {
            let parent = player.event.id.clone();
            advance_npcs(
                &mut world,
                &session
                    .project
                    .npc_behaviors
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
                64,
                &[parent],
            )?
        } else {
            NpcSimulationReport::default()
        };
        if player.event.advanced_turn && !player.duplicate && !player.event.terminal {
            fire_scenario_events(
                &session.project,
                &session.scenario_id,
                &mut world,
                &player.event.id,
            )?;
        }
        session.world = world.clone();
        Ok(RuntimeCommandResult { player, npc, world })
    }

    pub fn execute_choice(
        &mut self,
        session_id: &str,
        storylet_id: &str,
        choice_id: &str,
        idempotency_key: &str,
        expected_revision: u64,
    ) -> Result<RuntimeCommandResult, RuntimeError> {
        let session =
            self.sessions
                .get_mut(session_id)
                .ok_or_else(|| RuntimeError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
        let definition = session
            .project
            .storylets
            .get(storylet_id)
            .filter(|_| session.storylet_ids.iter().any(|id| id == storylet_id))
            .cloned()
            .ok_or_else(|| {
                RuntimeError::Engine(EngineError::StoryletNotEligible {
                    reason: "storylet is not part of this scenario".into(),
                })
            })?;
        let choice = definition
            .choices
            .iter()
            .find(|choice| choice.id == choice_id)
            .cloned()
            .ok_or(RuntimeError::Engine(EngineError::ChoiceNotFound))?;

        // Retry an already committed choice before evaluating lifecycle again.
        // This allows a lost HTTP response to be retried after the scene closes.
        if let Some(event_id) = session.world.applied_commands.get(idempotency_key) {
            let recorded = session
                .world
                .events
                .iter()
                .find(|event| &event.id == event_id)
                .expect("applied command index must point to an event")
                .clone();
            if recorded.actor_id != "player"
                || recorded.action != format!("storylet:{storylet_id}:{choice_id}")
                || recorded.effects != choice.effects
                || !recorded.advanced_turn
                || recorded.terminal != definition.ending
            {
                return Err(RuntimeError::Engine(EngineError::IdempotencyKeyReuse));
            }
            let mut world = session.world.clone();
            let player = apply_command(
                &mut world,
                WorldCommand {
                    idempotency_key: idempotency_key.to_owned(),
                    expected_revision,
                    actor_id: recorded.actor_id.clone(),
                    action: recorded.action.clone(),
                    effects: recorded.effects.clone(),
                    caused_by: recorded.caused_by.clone(),
                    advance_turn: true,
                    terminal: definition.ending,
                },
            )?;
            return Ok(RuntimeCommandResult {
                player,
                npc: NpcSimulationReport::default(),
                world,
            });
        }

        if session.world.revision != expected_revision {
            return Err(RuntimeError::Engine(EngineError::RevisionConflict {
                expected: expected_revision,
                actual: session.world.revision,
            }));
        }
        let candidate = evaluate_storylets(
            std::slice::from_ref(&definition),
            &session.world,
            &session.director,
            0.15,
        )
        .into_iter()
        .next()
        .expect("one definition produces one candidate");
        if !candidate.eligible {
            return Err(RuntimeError::Engine(EngineError::StoryletNotEligible {
                reason: format!("{:?}", candidate.rejection),
            }));
        }

        let mut world = session.world.clone();
        let mut director = session.director.clone();
        let instance_id = format!("playtest:{session_id}:{}:{}", definition.id, world.revision);
        start_storylet(
            &definition,
            &candidate,
            &mut world,
            &mut director,
            &instance_id,
        )?;
        let execution = execute_storylet_choice(
            &definition,
            &instance_id,
            choice_id,
            idempotency_key,
            &mut world,
            &mut director,
        )?;
        let player = CommandResult {
            revision: execution.event.revision,
            turn: execution.event.turn,
            event: execution.event,
            duplicate: execution.duplicate,
        };
        let event_id = player.event.id.clone();
        let npc = if player.event.terminal {
            NpcSimulationReport::default()
        } else {
            advance_npcs(
                &mut world,
                &session
                    .project
                    .npc_behaviors
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
                64,
                &[event_id],
            )?
        };
        if player.event.advanced_turn && !player.duplicate && !player.event.terminal {
            fire_scenario_events(
                &session.project,
                &session.scenario_id,
                &mut world,
                &player.event.id,
            )?;
        }
        session.world = world.clone();
        session.director = director;
        Ok(RuntimeCommandResult { player, npc, world })
    }

    fn session(&self, session_id: &str) -> Result<&PlaytestSession, RuntimeError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| RuntimeError::SessionNotFound {
                session_id: session_id.to_owned(),
            })
    }
}

fn fire_scenario_events(
    project: &ProjectBundle,
    scenario_id: &str,
    world: &mut WorldState,
    parent_event_id: &str,
) -> Result<(), RuntimeError> {
    let Some(scenario) = project.scenarios.get(scenario_id) else {
        return Err(RuntimeError::InvalidSessionState(
            "scenario event schedule is missing".into(),
        ));
    };
    for event_id in &scenario.event_ids {
        let Some(definition) = project.events.get(event_id) else {
            return Err(RuntimeError::InvalidSessionState(format!(
                "scenario event '{}' is missing",
                event_id
            )));
        };
        if world.turn < definition.delay_turns {
            continue;
        }
        if !definition.repeatable
            && world.events.iter().any(|event| {
                event.action
                    == format!(
                        "event:{}:{}",
                        event_kind(definition.event_type),
                        definition.id
                    )
            })
        {
            continue;
        }
        let triggered = definition
            .trigger
            .all_of
            .iter()
            .all(|predicate| crate::storylets::predicate_matches(predicate, world))
            && (definition.trigger.any_of.is_empty()
                || definition
                    .trigger
                    .any_of
                    .iter()
                    .any(|predicate| crate::storylets::predicate_matches(predicate, world)));
        if !triggered {
            continue;
        }
        let action = format!(
            "event:{}:{}",
            event_kind(definition.event_type),
            definition.id
        );
        let idempotency_key = if definition.repeatable {
            format!(
                "scenario-event:{}:{}:turn:{}",
                event_kind(definition.event_type),
                definition.id,
                world.turn
            )
        } else {
            format!(
                "scenario-event:{}:{}",
                event_kind(definition.event_type),
                definition.id
            )
        };
        apply_command(
            world,
            WorldCommand {
                idempotency_key,
                expected_revision: world.revision,
                actor_id: "world".into(),
                action,
                effects: definition.effects.clone(),
                caused_by: vec![parent_event_id.to_owned()],
                advance_turn: false,
                terminal: false,
            },
        )?;
    }
    Ok(())
}

fn event_kind(event_type: crate::EventType) -> &'static str {
    match event_type {
        crate::EventType::Canon => "canon",
        crate::EventType::World => "world",
    }
}

fn normalized_embedding(values: &[f64], dimensions: usize) -> Result<Vec<f64>, RuntimeError> {
    let invalid = || RuntimeError::InvalidSessionState("semantic embedding is invalid".into());
    if values.len() != dimensions || values.iter().any(|value| !value.is_finite()) {
        return Err(invalid());
    }
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !norm.is_finite() || norm <= 1e-12 {
        return Err(invalid());
    }
    Ok(values.iter().map(|value| value / norm).collect())
}

fn validate_command_references(
    project: &ProjectBundle,
    command: &WorldCommand,
) -> Result<(), RuntimeError> {
    for effect in &command.effects {
        match effect {
            crate::Effect::SetQuality { key, .. } | crate::Effect::AddQuality { key, .. } => {
                if !project.qualities.contains_key(key) {
                    return Err(RuntimeError::UnknownQuality { key: key.clone() });
                }
            }
            crate::Effect::TransferQuality { from, to, .. } => {
                for key in [from, to] {
                    if !project.qualities.contains_key(key) {
                        return Err(RuntimeError::UnknownQuality { key: key.clone() });
                    }
                }
            }
            crate::Effect::MovePlayer { location_id }
            | crate::Effect::MoveActor { location_id, .. } => {
                if !project.locations.contains_key(location_id) {
                    return Err(RuntimeError::UnknownLocation {
                        location_id: location_id.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}
