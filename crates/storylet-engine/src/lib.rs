//! Host-independent storylet decision and simulation engine.
//!
//! This crate owns deterministic state transitions. It deliberately contains no
//! networking, persistence, model calls, or engine-specific dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod chrono_catalog;
pub mod chrono_completion;
pub mod chrono_features;
pub mod chrono_tables;
pub mod chrono_transactions;
pub mod chrono_triggers;
pub mod chrono_dispatch;
pub mod chrono_runtime;
pub mod chrono_selection;
pub mod chrono_state;

/// Serializable world state shared by the studio, native hosts, and replay.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub revision: u64,
    pub turn: u64,
    #[serde(default)]
    pub scenario_id: Option<String>,
    #[serde(default)]
    pub random_seed: u64,
    #[serde(default)]
    pub random_cursor: u64,
    pub qualities: BTreeMap<String, Value>,
    #[serde(default)]
    pub current_location: Option<String>,
    #[serde(default)]
    pub location_tags: BTreeMap<String, BTreeSet<String>>,
    #[serde(default)]
    pub visited_locations: BTreeSet<String>,
    #[serde(default)]
    pub met_characters: BTreeSet<String>,
    #[serde(default)]
    pub actors: BTreeMap<String, crate::storylets::ActorState>,
    #[serde(default)]
    pub relationships: BTreeMap<String, BTreeMap<String, f64>>,
    #[serde(default)]
    pub knowledge: BTreeMap<String, BTreeSet<String>>,
    #[serde(default)]
    pub events: Vec<WorldEvent>,
    pub applied_commands: BTreeMap<String, String>,
}

impl WorldState {
    pub fn is_terminal(&self) -> bool {
        self.events.iter().any(|event| event.terminal)
    }
}

/// A single idempotent request to mutate the world and advance one action turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldCommand {
    pub idempotency_key: String,
    pub expected_revision: u64,
    pub actor_id: String,
    pub action: String,
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub caused_by: Vec<String>,
    #[serde(default = "default_true")]
    pub advance_turn: bool,
    #[serde(default, skip_deserializing)]
    pub terminal: bool,
}

fn default_true() -> bool {
    true
}

/// Typed operations on qualities. Resource movement is an explicit transfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    SetQuality {
        key: String,
        value: Value,
    },
    AddQuality {
        key: String,
        amount: f64,
    },
    TransferQuality {
        from: String,
        to: String,
        amount: f64,
    },
    MovePlayer {
        location_id: String,
    },
    MoveActor {
        actor_id: String,
        location_id: String,
    },
}

/// The before/after values for one quality changed by a committed event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityDelta {
    pub key: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// Durable causal record of an accepted command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldEvent {
    pub id: String,
    pub revision: u64,
    pub turn: u64,
    pub actor_id: String,
    pub action: String,
    #[serde(default = "default_true")]
    pub advanced_turn: bool,
    #[serde(default)]
    pub effects: Vec<Effect>,
    pub deltas: Vec<QualityDelta>,
    pub caused_by: Vec<String>,
    #[serde(default)]
    pub location_delta: Option<(Option<String>, Option<String>)>,
    #[serde(default)]
    pub actor_location_deltas: BTreeMap<String, (String, String)>,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub revision: u64,
    pub turn: u64,
    pub event: WorldEvent,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineError {
    EmptyIdempotencyKey,
    IdempotencyKeyReuse,
    RevisionConflict {
        expected: u64,
        actual: u64,
    },
    MissingCausalParent {
        parent: String,
    },
    ReplayMismatch {
        event_id: String,
    },
    InvalidQualityKey,
    NonNumericQuality {
        key: String,
    },
    NonFiniteNumber,
    NumericOverflow {
        key: String,
    },
    InvalidTransferAmount,
    InsufficientBalance {
        key: String,
        available: f64,
        requested: f64,
    },
    RevisionOverflow,
    TurnOverflow,
    ActorNotFound {
        actor_id: String,
    },
    InstanceIdInUse,
    InstanceNotFound,
    InstanceNotActive,
    StoryletNotEligible {
        reason: String,
    },
    ChoiceNotFound,
    ChoiceRequirementFailed {
        predicate_index: usize,
    },
    PersistentSceneCannotContinue,
    CooldownOverflow,
    SessionEnded,
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdempotencyKey => write!(formatter, "idempotency key must not be empty"),
            Self::IdempotencyKeyReuse => write!(
                formatter,
                "idempotency key was already used for a different command"
            ),
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "revision conflict: expected {expected}, found {actual}"
            ),
            Self::ReplayMismatch { event_id } => write!(
                formatter,
                "event '{event_id}' did not reproduce during replay"
            ),
            Self::MissingCausalParent { parent } => {
                write!(formatter, "causal parent '{parent}' does not exist")
            }
            Self::InvalidQualityKey => write!(formatter, "quality key must not be empty"),
            Self::NonNumericQuality { key } => write!(
                formatter,
                "quality '{key}' must be numeric for this operation"
            ),
            Self::NonFiniteNumber => write!(formatter, "numeric effects must be finite"),
            Self::NumericOverflow { key } => {
                write!(formatter, "numeric effect overflowed quality '{key}'")
            }
            Self::InvalidTransferAmount => {
                write!(formatter, "transfer amount must be positive and finite")
            }
            Self::InsufficientBalance {
                key,
                available,
                requested,
            } => write!(
                formatter,
                "quality '{key}' has {available}, cannot transfer {requested}"
            ),
            Self::RevisionOverflow => write!(formatter, "world revision overflow"),
            Self::TurnOverflow => write!(formatter, "world turn overflow"),
            Self::ActorNotFound { actor_id } => {
                write!(formatter, "actor '{actor_id}' does not exist")
            }
            Self::InstanceIdInUse => write!(formatter, "storylet instance id is already in use"),
            Self::InstanceNotFound => write!(formatter, "storylet instance was not found"),
            Self::InstanceNotActive => write!(formatter, "storylet instance is not active"),
            Self::StoryletNotEligible { reason } => {
                write!(formatter, "storylet is no longer eligible: {reason}")
            }
            Self::ChoiceNotFound => write!(formatter, "storylet choice was not found"),
            Self::ChoiceRequirementFailed { predicate_index } => {
                write!(formatter, "choice requirement {predicate_index} failed")
            }
            Self::PersistentSceneCannotContinue => write!(
                formatter,
                "only persistent storylets may continue across a choice"
            ),
            Self::CooldownOverflow => {
                write!(formatter, "storylet cooldown exceeds world turn range")
            }
            Self::SessionEnded => write!(formatter, "playtest session has reached its ending"),
        }
    }
}

impl Error for EngineError {}

/// Validates and atomically applies a command. Replaying an accepted command
/// returns its original event without changing state a second time.
pub fn apply_command(
    state: &mut WorldState,
    command: WorldCommand,
) -> Result<CommandResult, EngineError> {
    if command.idempotency_key.trim().is_empty() {
        return Err(EngineError::EmptyIdempotencyKey);
    }

    // Idempotency wins over revision checking so a retried request remains safe.
    if let Some(event_id) = state.applied_commands.get(&command.idempotency_key) {
        let event = state
            .events
            .iter()
            .find(|event| &event.id == event_id)
            .expect("applied command index must point to a recorded event")
            .clone();
        if event.actor_id != command.actor_id
            || event.action != command.action
            || event.effects != command.effects
            || event.caused_by != command.caused_by
            || event.advanced_turn != command.advance_turn
            || event.terminal != command.terminal
        {
            return Err(EngineError::IdempotencyKeyReuse);
        }
        return Ok(CommandResult {
            revision: event.revision,
            turn: event.turn,
            event,
            duplicate: true,
        });
    }

    if state.is_terminal() {
        return Err(EngineError::SessionEnded);
    }

    if command.expected_revision != state.revision {
        return Err(EngineError::RevisionConflict {
            expected: command.expected_revision,
            actual: state.revision,
        });
    }
    for parent in &command.caused_by {
        if !state.events.iter().any(|event| &event.id == parent) {
            return Err(EngineError::MissingCausalParent {
                parent: parent.clone(),
            });
        }
    }

    let next_revision = state
        .revision
        .checked_add(1)
        .ok_or(EngineError::RevisionOverflow)?;
    let next_turn = if command.advance_turn {
        state.turn.checked_add(1).ok_or(EngineError::TurnOverflow)?
    } else {
        state.turn
    };
    let mut next_qualities = state.qualities.clone();
    let mut touched = BTreeSet::new();
    let mut next_location = state.current_location.clone();
    let mut next_actors = state.actors.clone();
    let mut next_visited_locations = state.visited_locations.clone();
    let mut next_met_characters = state.met_characters.clone();
    let mut location_delta = None;
    let mut actor_location_deltas = BTreeMap::new();

    for effect in &command.effects {
        match effect {
            Effect::MovePlayer { location_id } => {
                validate_key(location_id)?;
                if next_location.as_ref() != Some(location_id) {
                    location_delta = Some((next_location.clone(), Some(location_id.clone())));
                    next_location = Some(location_id.clone());
                }
            }
            Effect::MoveActor {
                actor_id,
                location_id,
            } => {
                validate_key(actor_id)?;
                validate_key(location_id)?;
                let actor =
                    next_actors
                        .get_mut(actor_id)
                        .ok_or_else(|| EngineError::ActorNotFound {
                            actor_id: actor_id.clone(),
                        })?;
                if actor.current_location != *location_id {
                    actor_location_deltas.insert(
                        actor_id.clone(),
                        (actor.current_location.clone(), location_id.clone()),
                    );
                    actor.current_location = location_id.clone();
                }
            }
            _ => apply_effect(&mut next_qualities, effect, &mut touched)?,
        }
    }

    let deltas = touched
        .into_iter()
        .filter_map(|key| {
            let before = state.qualities.get(&key).cloned();
            let after = next_qualities.get(&key).cloned();
            (before != after).then_some(QualityDelta { key, before, after })
        })
        .collect();

    let event = WorldEvent {
        id: command.idempotency_key.clone(),
        revision: next_revision,
        turn: next_turn,
        actor_id: command.actor_id,
        action: command.action,
        advanced_turn: command.advance_turn,
        effects: command.effects,
        deltas,
        caused_by: command.caused_by,
        location_delta,
        actor_location_deltas,
        terminal: command.terminal,
    };

    // Commit only after every effect has validated successfully.
    if let Some(location_id) = &next_location {
        next_visited_locations.insert(location_id.clone());
        for (actor_id, actor) in &next_actors {
            if actor.current_location == *location_id {
                next_met_characters.insert(actor_id.clone());
            }
        }
    }
    state.qualities = next_qualities;
    state.current_location = next_location;
    state.actors = next_actors;
    state.visited_locations = next_visited_locations;
    state.met_characters = next_met_characters;
    state.revision = next_revision;
    state.turn = next_turn;
    state
        .applied_commands
        .insert(command.idempotency_key, event.id.clone());
    state.events.push(event.clone());

    Ok(CommandResult {
        revision: next_revision,
        turn: next_turn,
        event,
        duplicate: false,
    })
}

fn apply_effect(
    qualities: &mut BTreeMap<String, Value>,
    effect: &Effect,
    touched: &mut BTreeSet<String>,
) -> Result<(), EngineError> {
    match effect {
        Effect::SetQuality { key, value } => {
            validate_key(key)?;
            qualities.insert(key.clone(), value.clone());
            touched.insert(key.clone());
        }
        Effect::AddQuality { key, amount } => {
            validate_key(key)?;
            if !amount.is_finite() {
                return Err(EngineError::NonFiniteNumber);
            }
            let before = numeric_value(qualities.get(key), key)?;
            let after = before + amount;
            if !after.is_finite() {
                return Err(EngineError::NumericOverflow { key: key.clone() });
            }
            qualities.insert(key.clone(), number_value(after, key)?);
            touched.insert(key.clone());
        }
        Effect::MovePlayer { .. } | Effect::MoveActor { .. } => {
            unreachable!("movement effects are applied by the world transaction")
        }
        Effect::TransferQuality { from, to, amount } => {
            if from == to {
                return Err(EngineError::InvalidTransferAmount);
            }
            validate_key(from)?;
            validate_key(to)?;
            if !amount.is_finite() || *amount <= 0.0 {
                return Err(EngineError::InvalidTransferAmount);
            }
            let available = numeric_value(qualities.get(from), from)?;
            if available < *amount {
                return Err(EngineError::InsufficientBalance {
                    key: from.clone(),
                    available,
                    requested: *amount,
                });
            }
            let destination = numeric_value(qualities.get(to), to)?;
            let source_after = available - amount;
            let destination_after = destination + amount;
            if !source_after.is_finite() || !destination_after.is_finite() {
                return Err(EngineError::NumericOverflow { key: to.clone() });
            }
            qualities.insert(from.clone(), number_value(source_after, from)?);
            qualities.insert(to.clone(), number_value(destination_after, to)?);
            touched.insert(from.clone());
            touched.insert(to.clone());
        }
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), EngineError> {
    if key.trim().is_empty() {
        Err(EngineError::InvalidQualityKey)
    } else {
        Ok(())
    }
}

fn numeric_value(value: Option<&Value>, key: &str) -> Result<f64, EngineError> {
    match value {
        None => Ok(0.0),
        Some(value) => value
            .as_f64()
            .filter(|number| number.is_finite())
            .ok_or_else(|| EngineError::NonNumericQuality {
                key: key.to_owned(),
            }),
    }
}

fn number_value(number: f64, key: &str) -> Result<Value, EngineError> {
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| EngineError::NumericOverflow {
            key: key.to_owned(),
        })
}

/// Rebuild a branch by replaying its append-only event prefix from a scenario baseline.
pub fn replay_events(
    mut state: WorldState,
    events: &[WorldEvent],
) -> Result<WorldState, EngineError> {
    for recorded in events {
        let expected_revision = state.revision;
        let result = apply_command(
            &mut state,
            WorldCommand {
                idempotency_key: recorded.id.clone(),
                expected_revision,
                actor_id: recorded.actor_id.clone(),
                action: recorded.action.clone(),
                effects: recorded.effects.clone(),
                caused_by: recorded.caused_by.clone(),
                advance_turn: recorded.advanced_turn,
                terminal: recorded.terminal,
            },
        )?;
        if result.event != *recorded {
            return Err(EngineError::ReplayMismatch {
                event_id: recorded.id.clone(),
            });
        }
    }
    Ok(state)
}

/// Create a branch state at the event with the requested ID (inclusive).
pub fn fork_at(
    initial: WorldState,
    events: &[WorldEvent],
    event_id: &str,
) -> Result<WorldState, EngineError> {
    let Some(end) = events.iter().position(|event| event.id == event_id) else {
        return Err(EngineError::MissingCausalParent {
            parent: event_id.to_owned(),
        });
    };
    replay_events(initial, &events[..=end])
}

pub mod storylets;
pub use storylets::*;

pub mod simulation;
pub use simulation::*;

pub mod runtime;
pub use runtime::*;

pub mod project;
pub use project::*;

pub mod ffi;
