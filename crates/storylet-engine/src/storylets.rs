use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{WorldState, number_value};

/// Runtime presence and eligibility information for one generic character.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActorState {
    #[serde(default)]
    pub role_tags: BTreeSet<String>,
    pub current_location: String,
    #[serde(default)]
    pub busy: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactRef {
    Quality {
        key: String,
    },
    CurrentLocation,
    ActorAt {
        actor_id: String,
        location_id: String,
    },
    ActorHasRole {
        actor_id: String,
        role: String,
    },
    Knows {
        actor_id: String,
        claim_id: String,
    },
    Relationship {
        source_id: String,
        target_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Predicate {
    pub fact: FactRef,
    pub op: PredicateOp,
    pub value: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PredicateSet {
    #[serde(default)]
    pub all_of: Vec<Predicate>,
    #[serde(default)]
    pub any_of: Vec<Predicate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRequirement {
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneContent {
    pub opening: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletChoice {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub requirements: PredicateSet,
    #[serde(default)]
    pub effects: Vec<crate::Effect>,
    #[serde(default)]
    pub continues_scene: bool,
    #[serde(default)]
    pub divert_target: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterType {
    Linear,
    PlayerControlledPassage,
    Wheel,
    Aftermath,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Repeatability {
    OnceEver,
    Always,
    Cooldown { turns: u64 },
    Persistent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletDefinition {
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub summary: String,
    pub approved: bool,
    #[serde(default)]
    pub ending: bool,
    pub location_id: Option<String>,
    pub urgency_weight: f64,
    pub cluster_type: ClusterType,
    pub repeatability: Repeatability,
    #[serde(default)]
    pub required_roles: Vec<RoleRequirement>,
    pub trigger: PredicateSet,
    pub content: SceneContent,
    #[serde(default)]
    pub choices: Vec<StoryletChoice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LifecycleState {
    Active,
    Suspended,
    CoolingDown { until_turn: u64 },
    Completed,
    Retired,
    Expired,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorState {
    #[serde(default)]
    pub lifecycle: BTreeMap<String, LifecycleState>,
    #[serde(default)]
    pub role_reservations: BTreeMap<String, String>,
    #[serde(default)]
    pub instances: BTreeMap<String, StoryletInstance>,
    #[serde(default)]
    pub aftermath_queue: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum CandidateRejection {
    NotApproved,
    InvalidDefinition,
    WrongLocation,
    TriggerFailed { predicate_index: usize },
    MissingRole { role: String },
    RoleUnavailable { role: String },
    Retired,
    Expired,
    Suspended,
    Active,
    CoolingDown { until_turn: u64 },
    RoleReserved { role: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateEvaluation {
    pub storylet_id: String,
    pub eligible: bool,
    pub rejection: Option<CandidateRejection>,
    pub bound_roles: BTreeMap<String, String>,
    pub salience_score: f64,
    pub arc_alignment_score: f64,
    pub final_weight: f64,
}

/// Evaluate the complete definition list, including rejected items and reasons.
pub fn evaluate_storylets(
    definitions: &[StoryletDefinition],
    world: &WorldState,
    director: &DirectorState,
    target_tension_slope: f64,
) -> Vec<CandidateEvaluation> {
    let mut evaluations: Vec<_> = definitions
        .iter()
        .map(|definition| evaluate_one(definition, world, director, target_tension_slope))
        .collect();
    evaluations.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| right.final_weight.total_cmp(&left.final_weight))
            .then_with(|| left.storylet_id.cmp(&right.storylet_id))
    });
    evaluations
}

/// Produce a deterministic weighted ordering of eligible storylets without replacement.
/// The same seed and cursor reproduce the same deck; ineligible content never enters it.
pub fn seeded_weighted_shuffle(
    world: &mut WorldState,
    candidates: &[CandidateEvaluation],
) -> Vec<String> {
    let mut pool: Vec<_> = candidates.iter().filter(|item| item.eligible).collect();
    let mut result = Vec::with_capacity(pool.len());
    while !pool.is_empty() {
        let total: f64 = pool.iter().map(|c| c.final_weight.max(0.0)).sum();
        let draw = next_random(world)
            * if total > 0.0 {
                total
            } else {
                pool.len() as f64
            };
        let mut acc = 0.0;
        let index = pool
            .iter()
            .position(|candidate| {
                acc += if total > 0.0 {
                    candidate.final_weight.max(0.0)
                } else {
                    1.0
                };
                draw < acc
            })
            .unwrap_or(pool.len() - 1);
        result.push(pool.remove(index).storylet_id.clone());
    }
    result
}

fn next_random(world: &mut WorldState) -> f64 {
    let mut z = world
        .random_seed
        .wrapping_add(world.random_cursor)
        .wrapping_add(0x9E3779B97F4A7C15);
    world.random_cursor = world.random_cursor.wrapping_add(1);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^= z >> 31;
    (z >> 11) as f64 / ((1u64 << 53) as f64)
}

fn evaluate_one(
    definition: &StoryletDefinition,
    world: &WorldState,
    director: &DirectorState,
    target_tension_slope: f64,
) -> CandidateEvaluation {
    let mut result = CandidateEvaluation {
        storylet_id: definition.id.clone(),
        eligible: false,
        rejection: None,
        bound_roles: BTreeMap::new(),
        salience_score: 0.0,
        arc_alignment_score: 0.0,
        final_weight: 0.0,
    };
    let reject = |result: &mut CandidateEvaluation, reason| {
        result.rejection = Some(reason);
    };

    if !definition.approved {
        reject(&mut result, CandidateRejection::NotApproved);
        return result;
    }
    if definition.id.trim().is_empty() || !definition.urgency_weight.is_finite() {
        reject(&mut result, CandidateRejection::InvalidDefinition);
        return result;
    }
    if let Some(location) = &definition.location_id {
        if world.current_location.as_ref() != Some(location) {
            reject(&mut result, CandidateRejection::WrongLocation);
            return result;
        }
    }
    if let Some(lifecycle) = director.lifecycle.get(&definition.id) {
        match lifecycle {
            LifecycleState::Retired => {
                reject(&mut result, CandidateRejection::Retired);
                return result;
            }
            LifecycleState::Expired => {
                reject(&mut result, CandidateRejection::Expired);
                return result;
            }
            LifecycleState::Suspended => {
                reject(&mut result, CandidateRejection::Suspended);
                return result;
            }
            LifecycleState::Active => {
                reject(&mut result, CandidateRejection::Active);
                return result;
            }
            LifecycleState::CoolingDown { until_turn } if world.turn < *until_turn => {
                reject(
                    &mut result,
                    CandidateRejection::CoolingDown {
                        until_turn: *until_turn,
                    },
                );
                return result;
            }
            LifecycleState::CoolingDown { .. } | LifecycleState::Completed => {}
        }
    }

    let mut used_actors = BTreeSet::new();
    for requirement in &definition.required_roles {
        if requirement.role.trim().is_empty() {
            reject(&mut result, CandidateRejection::InvalidDefinition);
            return result;
        }
        let matching_role_exists = world
            .actors
            .values()
            .any(|actor| actor.role_tags.contains(&requirement.role));
        let found = world.actors.iter().find(|(actor_id, actor)| {
            actor.role_tags.contains(&requirement.role)
                && !actor.busy
                && !used_actors.contains(*actor_id)
                && definition
                    .location_id
                    .as_ref()
                    .is_none_or(|location| &actor.current_location == location)
                && !director.role_reservations.contains_key(*actor_id)
        });
        let Some((actor_id, _)) = found else {
            if !matching_role_exists {
                reject(
                    &mut result,
                    CandidateRejection::MissingRole {
                        role: requirement.role.clone(),
                    },
                );
            } else if world.actors.iter().any(|(id, actor)| {
                actor.role_tags.contains(&requirement.role)
                    && director.role_reservations.contains_key(id)
            }) {
                reject(
                    &mut result,
                    CandidateRejection::RoleReserved {
                        role: requirement.role.clone(),
                    },
                );
            } else {
                reject(
                    &mut result,
                    CandidateRejection::RoleUnavailable {
                        role: requirement.role.clone(),
                    },
                );
            }
            return result;
        };
        used_actors.insert(actor_id.clone());
        result
            .bound_roles
            .insert(requirement.role.clone(), actor_id.clone());
    }

    let mut match_count = 0;
    for (index, predicate) in definition.trigger.all_of.iter().enumerate() {
        if predicate_matches(predicate, world) {
            match_count += 1;
        } else {
            reject(
                &mut result,
                CandidateRejection::TriggerFailed {
                    predicate_index: index,
                },
            );
            return result;
        }
    }
    if !definition.trigger.any_of.is_empty() {
        if definition
            .trigger
            .any_of
            .iter()
            .any(|predicate| predicate_matches(predicate, world))
        {
            match_count += 1;
        } else {
            reject(
                &mut result,
                CandidateRejection::TriggerFailed {
                    predicate_index: definition.trigger.all_of.len(),
                },
            );
            return result;
        }
    }

    let location_bonus = if definition.location_id.is_some() {
        15.0
    } else {
        0.0
    };
    let tension_delta: f64 = match definition.cluster_type {
        ClusterType::Aftermath => 0.25,
        ClusterType::Linear => 0.15,
        ClusterType::PlayerControlledPassage => -0.05,
        ClusterType::Wheel => 0.05,
    };
    let slope = if target_tension_slope.is_finite() {
        target_tension_slope
    } else {
        0.0
    };
    result.salience_score = match_count as f64 * 10.0 + location_bonus;
    result.arc_alignment_score = (-(slope - tension_delta).abs()).exp();
    result.final_weight = definition.urgency_weight.clamp(0.0, 100.0) * 0.4
        + result.salience_score * 0.3
        + result.arc_alignment_score * 30.0;
    result.eligible = true;
    result
}

pub(crate) fn predicate_matches(predicate: &Predicate, world: &WorldState) -> bool {
    let actual = match &predicate.fact {
        FactRef::Quality { key } => world.qualities.get(key).cloned(),
        FactRef::CurrentLocation => world
            .current_location
            .as_ref()
            .map(|location| Value::String(location.clone())),
        FactRef::ActorAt {
            actor_id,
            location_id,
        } => world
            .actors
            .get(actor_id)
            .map(|actor| Value::Bool(actor.current_location == *location_id)),
        FactRef::ActorHasRole { actor_id, role } => world
            .actors
            .get(actor_id)
            .map(|actor| Value::Bool(actor.role_tags.contains(role))),
        FactRef::Knows { actor_id, claim_id } => world
            .knowledge
            .get(actor_id)
            .map(|claims| Value::Bool(claims.contains(claim_id))),
        FactRef::Relationship {
            source_id,
            target_id,
        } => world
            .relationships
            .get(source_id)
            .and_then(|targets| targets.get(target_id))
            .and_then(|score| number_value(*score, "relationship").ok()),
    };
    let Some(actual) = actual else {
        return false;
    };
    match predicate.op {
        PredicateOp::Eq => match (actual.as_f64(), predicate.value.as_f64()) {
            (Some(actual), Some(expected)) if actual.is_finite() && expected.is_finite() => {
                actual == expected
            }
            _ => actual == predicate.value,
        },
        PredicateOp::Neq => match (actual.as_f64(), predicate.value.as_f64()) {
            (Some(actual), Some(expected)) if actual.is_finite() && expected.is_finite() => {
                actual != expected
            }
            _ => actual != predicate.value,
        },
        PredicateOp::Gt => compare_numbers(&actual, &predicate.value, |a, b| a > b),
        PredicateOp::Gte => compare_numbers(&actual, &predicate.value, |a, b| a >= b),
        PredicateOp::Lt => compare_numbers(&actual, &predicate.value, |a, b| a < b),
        PredicateOp::Lte => compare_numbers(&actual, &predicate.value, |a, b| a <= b),
        PredicateOp::Contains => match (&actual, &predicate.value) {
            (Value::String(actual), Value::String(expected)) => actual.contains(expected),
            (Value::Array(actual), expected) => actual.contains(expected),
            _ => false,
        },
    }
}

fn compare_numbers(
    actual: &Value,
    expected: &Value,
    compare: impl FnOnce(f64, f64) -> bool,
) -> bool {
    match (actual.as_f64(), expected.as_f64()) {
        (Some(actual), Some(expected)) if actual.is_finite() && expected.is_finite() => {
            compare(actual, expected)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Active,
    Suspended,
    Completed,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryletInstance {
    pub id: String,
    pub storylet_id: String,
    pub definition_revision: u64,
    pub bound_roles: BTreeMap<String, String>,
    pub status: InstanceStatus,
    pub cursor: u32,
    pub start_turn: u64,
    pub event_ids: Vec<String>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletExecutionResult {
    pub event: crate::WorldEvent,
    pub duplicate: bool,
    pub instance_completed: bool,
    pub divert_target: Option<String>,
}

/// Start an eligible storylet and reserve all bound actors atomically.
pub fn start_storylet(
    definition: &StoryletDefinition,
    _candidate: &CandidateEvaluation,
    world: &mut WorldState,
    director: &mut DirectorState,
    instance_id: &str,
) -> Result<StoryletInstance, crate::EngineError> {
    if instance_id.trim().is_empty() || director.instances.contains_key(instance_id) {
        return Err(crate::EngineError::InstanceIdInUse);
    }
    let current = evaluate_one(definition, world, director, 0.15);
    if !current.eligible {
        return Err(crate::EngineError::StoryletNotEligible {
            reason: format!(
                "{:?}",
                current
                    .rejection
                    .unwrap_or(CandidateRejection::InvalidDefinition)
            ),
        });
    }

    let instance = StoryletInstance {
        id: instance_id.to_owned(),
        storylet_id: definition.id.clone(),
        definition_revision: definition.revision,
        bound_roles: current.bound_roles,
        status: InstanceStatus::Active,
        cursor: 0,
        start_turn: world.turn,
        event_ids: Vec::new(),
        completed: false,
    };
    for actor_id in instance.bound_roles.values() {
        director
            .role_reservations
            .insert(actor_id.clone(), instance.id.clone());
    }
    director
        .lifecycle
        .insert(definition.id.clone(), LifecycleState::Active);
    director
        .instances
        .insert(instance.id.clone(), instance.clone());
    Ok(instance)
}

/// Commit one approved choice through the shared world transaction path.
pub fn execute_storylet_choice(
    definition: &StoryletDefinition,
    instance_id: &str,
    choice_id: &str,
    idempotency_key: &str,
    world: &mut WorldState,
    director: &mut DirectorState,
) -> Result<StoryletExecutionResult, crate::EngineError> {
    if !definition.approved {
        return Err(crate::EngineError::StoryletNotEligible {
            reason: "content is not approved".into(),
        });
    }
    let choice = definition
        .choices
        .iter()
        .find(|choice| choice.id == choice_id)
        .ok_or(crate::EngineError::ChoiceNotFound)?;
    let caused_by = world
        .applied_commands
        .get(idempotency_key)
        .and_then(|event_id| world.events.iter().find(|event| &event.id == event_id))
        .map(|event| event.caused_by.clone())
        .or_else(|| {
            director
                .instances
                .get(instance_id)
                .map(|instance| instance.event_ids.clone())
        })
        .unwrap_or_default();
    let command = crate::WorldCommand {
        idempotency_key: idempotency_key.to_owned(),
        expected_revision: world.revision,
        actor_id: "player".into(),
        action: format!("storylet:{}:{}", definition.id, choice.id),
        effects: choice.effects.clone(),
        caused_by,
        advance_turn: true,
        terminal: definition.ending,
    };

    // A lost response can be safely retried after the scene has already closed.
    if world.applied_commands.contains_key(idempotency_key) {
        let command_result = crate::apply_command(world, command)?;
        let completed = director
            .instances
            .get(instance_id)
            .is_some_and(|instance| instance.completed);
        return Ok(StoryletExecutionResult {
            event: command_result.event,
            duplicate: true,
            instance_completed: completed,
            divert_target: choice.divert_target.clone(),
        });
    }

    let instance = director
        .instances
        .get(instance_id)
        .ok_or(crate::EngineError::InstanceNotFound)?;
    if instance.status != InstanceStatus::Active
        || instance.completed
        || instance.storylet_id != definition.id
        || instance.definition_revision != definition.revision
    {
        return Err(crate::EngineError::InstanceNotActive);
    }
    for (index, predicate) in choice.requirements.all_of.iter().enumerate() {
        if !predicate_matches(predicate, world) {
            return Err(crate::EngineError::ChoiceRequirementFailed {
                predicate_index: index,
            });
        }
    }
    if !choice.requirements.any_of.is_empty()
        && !choice
            .requirements
            .any_of
            .iter()
            .any(|predicate| predicate_matches(predicate, world))
    {
        return Err(crate::EngineError::ChoiceRequirementFailed {
            predicate_index: choice.requirements.all_of.len(),
        });
    }
    if choice.continues_scene && !matches!(definition.repeatability, Repeatability::Persistent) {
        return Err(crate::EngineError::PersistentSceneCannotContinue);
    }

    let cooldown_until = match definition.repeatability {
        Repeatability::Cooldown { turns } => Some(
            world
                .turn
                .checked_add(1)
                .and_then(|turn| turn.checked_add(turns))
                .ok_or(crate::EngineError::CooldownOverflow)?,
        ),
        _ => None,
    };
    let command_result = crate::apply_command(world, command)?;

    let instance = director
        .instances
        .get_mut(instance_id)
        .ok_or(crate::EngineError::InstanceNotFound)?;
    instance.event_ids.push(command_result.event.id.clone());
    if choice.continues_scene {
        instance.cursor = instance.cursor.saturating_add(1);
        return Ok(StoryletExecutionResult {
            event: command_result.event,
            duplicate: false,
            instance_completed: false,
            divert_target: choice.divert_target.clone(),
        });
    }

    instance.status = InstanceStatus::Completed;
    instance.completed = true;
    for actor_id in instance.bound_roles.values() {
        if director
            .role_reservations
            .get(actor_id)
            .is_some_and(|reserved_by| reserved_by == instance_id)
        {
            director.role_reservations.remove(actor_id);
        }
    }
    let lifecycle = match definition.repeatability {
        Repeatability::OnceEver => LifecycleState::Retired,
        Repeatability::Always | Repeatability::Persistent => LifecycleState::Completed,
        Repeatability::Cooldown { .. } => LifecycleState::CoolingDown {
            until_turn: cooldown_until.expect("cooldown was calculated before commit"),
        },
    };
    director.lifecycle.insert(definition.id.clone(), lifecycle);
    if let Some(target) = &choice.divert_target {
        director.aftermath_queue.insert(0, target.clone());
    }

    Ok(StoryletExecutionResult {
        event: command_result.event,
        duplicate: false,
        instance_completed: true,
        divert_target: choice.divert_target.clone(),
    })
}

/// Suspend a scene without losing its cursor. Actor reservations are released.
pub fn suspend_storylet(
    instance_id: &str,
    director: &mut DirectorState,
) -> Result<(), crate::EngineError> {
    let instance = director
        .instances
        .get_mut(instance_id)
        .ok_or(crate::EngineError::InstanceNotFound)?;
    if instance.status != InstanceStatus::Active || instance.completed {
        return Err(crate::EngineError::InstanceNotActive);
    }
    for actor_id in instance.bound_roles.values() {
        if director
            .role_reservations
            .get(actor_id)
            .is_some_and(|reserved_by| reserved_by == instance_id)
        {
            director.role_reservations.remove(actor_id);
        }
    }
    instance.status = InstanceStatus::Suspended;
    director
        .lifecycle
        .insert(instance.storylet_id.clone(), LifecycleState::Suspended);
    Ok(())
}

/// Resume only if the same definition revision, triggers, and role bindings remain valid.
pub fn resume_storylet(
    definition: &StoryletDefinition,
    instance_id: &str,
    world: &WorldState,
    director: &mut DirectorState,
) -> Result<(), crate::EngineError> {
    let instance = director
        .instances
        .get(instance_id)
        .ok_or(crate::EngineError::InstanceNotFound)?;
    if instance.status != InstanceStatus::Suspended
        || instance.completed
        || instance.storylet_id != definition.id
        || instance.definition_revision != definition.revision
    {
        return Err(crate::EngineError::InstanceNotActive);
    }
    let candidate = evaluate_one(
        definition,
        world,
        &DirectorState {
            lifecycle: BTreeMap::new(),
            role_reservations: director.role_reservations.clone(),
            instances: director.instances.clone(),
            aftermath_queue: director.aftermath_queue.clone(),
        },
        0.15,
    );
    if !candidate.eligible {
        return Err(crate::EngineError::StoryletNotEligible {
            reason: format!("{:?}", candidate.rejection),
        });
    }
    if candidate.bound_roles != instance.bound_roles {
        return Err(crate::EngineError::StoryletNotEligible {
            reason: "the original role bindings are no longer available".into(),
        });
    }
    let roles = instance.bound_roles.clone();
    for actor_id in roles.values() {
        director
            .role_reservations
            .insert(actor_id.clone(), instance_id.to_owned());
    }
    director
        .instances
        .get_mut(instance_id)
        .expect("instance was checked above")
        .status = InstanceStatus::Active;
    director
        .lifecycle
        .insert(definition.id.clone(), LifecycleState::Active);
    Ok(())
}
