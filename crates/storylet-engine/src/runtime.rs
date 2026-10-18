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

const LEGACY_RUNTIME_SESSION_FORMAT_VERSION: u32 = 1;
pub const RUNTIME_SESSION_FORMAT_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvaluation {
    pub candidates: Vec<CandidateEvaluation>,
    pub selected: Option<String>,
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
        mut state: RuntimeSessionState,
    ) -> Result<SessionSnapshot, RuntimeError> {
        if !matches!(
            state.format_version,
            LEGACY_RUNTIME_SESSION_FORMAT_VERSION | RUNTIME_SESSION_FORMAT_VERSION
        ) {
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
        if state.format_version == LEGACY_RUNTIME_SESSION_FORMAT_VERSION {
            if replayed != state.world {
                // Older v1 snapshots omitted these indexes; reconstruct only those derived fields.
                let mut normalized = state.world.clone();
                normalized.location_tags.clone_from(&replayed.location_tags);
                normalized
                    .visited_locations
                    .clone_from(&replayed.visited_locations);
                normalized
                    .met_characters
                    .clone_from(&replayed.met_characters);
                if replayed != normalized {
                    return Err(RuntimeError::InvalidSessionState(
                        "legacy event log does not reproduce the saved world".into(),
                    ));
                }
            }
            state.world = replayed;
            state.format_version = RUNTIME_SESSION_FORMAT_VERSION;
        } else if replayed != state.world {
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
        })
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
