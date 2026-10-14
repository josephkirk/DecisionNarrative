//! Deterministic, rule-authored autonomous actions for non-player characters.

use serde::{Deserialize, Serialize};

use crate::{Effect, EngineError, PredicateSet, WorldEvent, WorldState, apply_command};

/// An authored NPC behavior. Rules are project data and contain no executable code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NpcBehaviorRule {
    pub id: String,
    pub actor_id: String,
    pub priority: i32,
    #[serde(default)]
    pub conditions: PredicateSet,
    pub action: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// Results and an explicit diagnostic when the per-turn action bound is reached.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NpcSimulationReport {
    pub events: Vec<WorldEvent>,
    pub limit_reached: bool,
}

/// Resolve eligible NPC rules by priority, then stable rule and actor IDs.
/// Each actor acts at most once per call. NPC events share the current turn and
/// point to the player or world events that caused this simulation pass.
/// Mutations are committed as one batch: a failed effect leaves the world intact.
pub fn advance_npcs(
    world: &mut WorldState,
    rules: &[NpcBehaviorRule],
    max_actions: usize,
    caused_by: &[String],
) -> Result<NpcSimulationReport, EngineError> {
    let mut ordered: Vec<&NpcBehaviorRule> = rules.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.actor_id.cmp(&right.actor_id))
    });

    let mut working = world.clone();
    let mut acted = std::collections::BTreeSet::new();
    let mut report = NpcSimulationReport::default();
    for rule in ordered {
        if report.events.len() >= max_actions {
            report.limit_reached = has_eligible_rule(rule, &working, &acted);
            if report.limit_reached {
                break;
            }
        }
        if !has_eligible_rule(rule, &working, &acted) {
            continue;
        }

        let idempotency_key = format!("npc:{}:{}:{}", working.turn, rule.actor_id, rule.id);
        let expected_revision = working.revision;
        let result = apply_command(
            &mut working,
            crate::WorldCommand {
                idempotency_key,
                expected_revision,
                actor_id: rule.actor_id.clone(),
                action: rule.action.clone(),
                effects: rule.effects.clone(),
                caused_by: caused_by.to_vec(),
                advance_turn: false,
            },
        )?;
        acted.insert(rule.actor_id.clone());
        report.events.push(result.event);
    }

    *world = working;
    Ok(report)
}

fn has_eligible_rule(
    rule: &NpcBehaviorRule,
    world: &WorldState,
    acted: &std::collections::BTreeSet<String>,
) -> bool {
    if rule.id.trim().is_empty() || rule.action.trim().is_empty() || acted.contains(&rule.actor_id)
    {
        return false;
    }
    let Some(actor) = world.actors.get(&rule.actor_id) else {
        return false;
    };
    if actor.busy {
        return false;
    }
    rule.conditions
        .all_of
        .iter()
        .all(|predicate| crate::storylets::predicate_matches(predicate, world))
        && (rule.conditions.any_of.is_empty()
            || rule
                .conditions
                .any_of
                .iter()
                .any(|predicate| crate::storylets::predicate_matches(predicate, world)))
}
