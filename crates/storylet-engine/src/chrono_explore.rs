//! Seeded random generator used by the coverage explorer.

/// Mulberry32, matching the Weir's Crossing prototype's 32-bit sequence.
#[derive(Clone, Debug)]
pub struct SeededRng(u32);

impl SeededRng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }
}

/// First step of a shortest currently enabled route to one of the targets.
/// The search uses the participant's actual knowledge and the current clock.
fn first_move_toward(
    world: &crate::chrono_state::WorldStateTable,
    content: &crate::run::RunContent,
    participant: crate::chrono_participants::PartIx,
    targets: &[u16],
) -> Option<u16> {
    use std::collections::{BTreeSet, VecDeque};
    let start = world.active_location(participant)?;
    if targets.contains(&start) {
        return None;
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([(start, None)]);
    let mut simulated = world.clone();
    while let Some((place, first)) = queue.pop_front() {
        simulated.participants.location[participant as usize] = Some(place);
        for movement in crate::chrono_space::moves_for(&simulated, content, participant) {
            if !movement.enabled || !seen.insert(movement.to) {
                continue;
            }
            let first = first.unwrap_or(movement.to);
            if targets.contains(&movement.to) {
                return Some(first);
            }
            queue.push_back((movement.to, Some(first)));
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrono_participants::Controller;
    use crate::chrono_space::{Clock, ClockState, Connection};
    use crate::chrono_state::WorldStateTable;
    use crate::run::RunContent;

    #[test]
    fn detects_unhandled_closure_but_not_an_authored_strand_rule() {
        use crate::chrono_space::OnClose;
        let mut world = WorldStateTable::new();
        world
            .participants
            .push("player", u16::MAX, Controller::Human { seat: 0 }, Some(1));
        let clock = Clock {
            slot_labels: vec!["open".into(), "closed".into()],
            actions_per_slot: 1,
        };
        world.clock = Some(ClockState {
            slot: 1,
            actions_left: vec![1],
        });
        let mut content = RunContent {
            clock: Some(clock),
            connections: vec![Connection {
                from: 0,
                to: 1,
                open_slots: Some(1),
                requires_fact: None,
                on_close: None,
            }],
            ..Default::default()
        };
        assert!(has_unhandled_closed_occupant(&world, &content));
        content.connections[0].on_close = Some(OnClose::Strand);
        assert!(!has_unhandled_closed_occupant(&world, &content));
    }
    #[test]
    fn new_knowledge_must_reference_an_existing_event() {
        use crate::chrono_participants::{KnowKind, KnowledgeRow};
        let baseline = WorldStateTable::new();
        let mut world = baseline.clone();
        world.knowledge.learn(KnowledgeRow {
            holder: 0,
            fact: 1,
            kind: KnowKind::Evidence,
            source_event: 2,
        });
        assert!(has_unsourced_knowledge(&world, &baseline, |event| event < 2));
        assert!(!has_unsourced_knowledge(&world, &baseline, |event| event < 3));
        assert!(!has_unsourced_knowledge(&world, &world, |_| false));
    }
    #[test]
    fn past_claim_requires_the_corresponding_telling_for_its_holder() {
        use crate::chrono_chronicle::{ChronicleEntry, ChronicleTable, TellingRow};
        use crate::chrono_participants::{KnowKind, KnowledgeRow};
        let registry =
            crate::chrono_state::GameplayTagRegistry::compile(["fact.past.P1".to_string()])
                .unwrap();
        let fact = registry.id("fact.past.P1").unwrap();
        let chronicle = ChronicleTable::new(vec![ChronicleEntry {
            key: "P1".into(),
            ..Default::default()
        }]);
        let content = RunContent {
            registry,
            chronicle,
            ..Default::default()
        };
        let mut world = WorldStateTable::new();
        world.knowledge.learn(KnowledgeRow {
            holder: 0,
            fact,
            kind: KnowKind::Recollection,
            source_event: 0,
        });
        world.telling.reveal(TellingRow {
            holder: 1,
            entry: 0,
            source_event: 0,
        });
        assert!(has_untold_past(&world, &content));
        world.telling.reveal(TellingRow {
            holder: 0,
            entry: 0,
            source_event: 0,
        });
        assert!(!has_untold_past(&world, &content));
    }
    #[test]
    fn contradictory_properties_are_checked_from_declared_pairs() {
        let registry = crate::chrono_state::GameplayTagRegistry::compile([
            "world.left".to_string(),
            "world.right".to_string(),
        ])
        .unwrap();
        let mut world = WorldStateTable::new();
        world
            .global_tags
            .set_bit(registry.id("world.left").unwrap())
            .unwrap();
        let pairs = vec![("world.left".to_string(), "world.right".to_string())];
        assert!(contradictory_pair(&world, &registry, &pairs).is_none());
        world
            .global_tags
            .set_bit(registry.id("world.right").unwrap())
            .unwrap();
        assert_eq!(
            contradictory_pair(&world, &registry, &pairs),
            Some(("world.left".to_string(), "world.right".to_string()))
        );
    }
    #[test]
    fn route_finds_shortest_path_using_only_open_connections() {
        let mut world = WorldStateTable::new();
        world
            .participants
            .push("player", u16::MAX, Controller::Human { seat: 0 }, Some(0));
        let clock = Clock {
            slot_labels: vec!["day".into()],
            actions_per_slot: 2,
        };
        world.clock = Some(ClockState::new(&clock, 1));
        let edge = |from, to, open_slots| Connection {
            from,
            to,
            open_slots,
            requires_fact: None,
            on_close: None,
        };
        let content = RunContent {
            clock: Some(clock),
            connections: vec![
                edge(0, 3, Some(0)),
                edge(0, 1, None),
                edge(1, 2, None),
                edge(2, 3, None),
            ],
            ..Default::default()
        };
        assert_eq!(first_move_toward(&world, &content, 0, &[3]), Some(1));
        assert_eq!(first_move_toward(&world, &content, 0, &[9]), None);
        assert_eq!(world.participants.location[0], Some(0));
    }
}

use crate::chrono_participants::{Controller, FactIx, PartIx};
use crate::run::{RunAction, RunContent, RunKernel, RunStart};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Persona {
    Wanderer,
    Investigator { epsilon_permille: u16 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantConfig {
    Solo,
    AllHumans,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Milestone {
    pub key: String,
    pub name: String,
    pub routes: Vec<u16>,
    pub reachable_for: Vec<ParticipantConfig>,
    pub deadline_slot: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentProbe {
    pub name: String,
    pub holder_knows: Vec<FactIx>,
    pub colocated_with: Option<u16>,
    pub expected_routes: Vec<u16>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreConfig {
    pub runs: u32,
    pub seed: u64,
    pub personas: Vec<Persona>,
    pub configs: Vec<ParticipantConfig>,
    pub milestones: Vec<Milestone>,
    pub probes: Vec<IntentProbe>,
    #[serde(default)]
    pub contradictory_props: Vec<(String, String)>,
    pub max_steps: u32,
}
impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            runs: 2000,
            seed: 0,
            personas: vec![
                Persona::Wanderer,
                Persona::Investigator {
                    epsilon_permille: 150,
                },
            ],
            configs: vec![ParticipantConfig::Solo, ParticipantConfig::AllHumans],
            milestones: vec![],
            probes: vec![],
            contradictory_props: vec![],
            max_steps: 500,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
pub enum Invariant {
    KnowledgeWithoutSource,
    PastWithoutTelling,
    OccupantBeyondClosedEdge,
    ContradictoryProps(String, String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub invariant: Invariant,
    pub seed: u64,
    pub persona: Persona,
    pub config: ParticipantConfig,
    pub step: u32,
    pub actions: Vec<(PartIx, RunAction)>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreError {
    pub seed: u64,
    pub persona: Persona,
    pub config: ParticipantConfig,
    pub step: u32,
    pub message: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreReport {
    pub cells: Vec<ExploreCell>,
    pub violations: Vec<Violation>,
    pub determinism_mismatches: u32,
    pub errors: Vec<ExploreError>,
    pub error_count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExploreCell {
    pub persona: Persona,
    pub config: ParticipantConfig,
    pub endings: Vec<(u16, u32)>,
    pub milestone_reached: Vec<(String, u32)>,
    pub probe_hits: Vec<(String, u32, u32)>,
    pub telling_orders: u32,
    pub dead_air_by_location: Vec<(u16, u32, u32)>,
    #[serde(skip)]
    telling_sequences: BTreeSet<Vec<u16>>,
}

impl PartialEq for ExploreCell {
    fn eq(&self, other: &Self) -> bool {
        self.persona == other.persona
            && self.config == other.config
            && self.endings == other.endings
            && self.milestone_reached == other.milestone_reached
            && self.probe_hits == other.probe_hits
            && self.telling_orders == other.telling_orders
            && self.dead_air_by_location == other.dead_air_by_location
    }
}
impl Eq for ExploreCell {}
pub fn configured_content(content: &RunContent, config: ParticipantConfig) -> RunContent {
    let mut copy = content.clone();
    if config == ParticipantConfig::Solo {
        let mut first_human = false;
        for controller in &mut copy.initial_world.participants.controller {
            if matches!(controller, Controller::Human { .. }) {
                if first_human {
                    *controller = Controller::Rule;
                } else {
                    first_human = true;
                }
            }
        }
    }
    copy
}

fn has_unsourced_knowledge(
    world: &crate::chrono_state::WorldStateTable,
    initial: &crate::chrono_state::WorldStateTable,
    has_source: impl Fn(u32) -> bool,
) -> bool {
    world
        .knowledge
        .rows
        .iter()
        .any(|row| !initial.knowledge.rows.contains(row) && !has_source(row.source_event))
}
fn contradictory_pair(
    world: &crate::chrono_state::WorldStateTable,
    registry: &crate::chrono_state::GameplayTagRegistry,
    pairs: &[(String, String)],
) -> Option<(String, String)> {
    pairs
        .iter()
        .find(|(a, b)| {
            registry.id(a).is_some_and(|id| world.global_tags.bit(id))
                && registry.id(b).is_some_and(|id| world.global_tags.bit(id))
        })
        .cloned()
}
fn has_untold_past(world: &crate::chrono_state::WorldStateTable, content: &RunContent) -> bool {
    world.knowledge.rows.iter().any(|row| {
        let Some(key) = content
            .registry
            .tag(row.fact)
            .and_then(|tag| tag.strip_prefix("fact.past."))
        else {
            return false;
        };
        content
            .chronicle
            .index_of(key)
            .is_none_or(|entry| !world.telling.is_told(row.holder, entry))
    })
}
fn has_unhandled_closed_occupant(
    world: &crate::chrono_state::WorldStateTable,
    content: &RunContent,
) -> bool {
    let (Some(_), Some(clock)) = (&content.clock, &world.clock) else {
        return false;
    };
    world
        .participants
        .location
        .iter()
        .flatten()
        .any(|&location| {
            let inbound: Vec<_> = content
                .connections
                .iter()
                .filter(|c| c.to == location)
                .collect();
            !inbound.is_empty()
                && inbound.iter().all(|c| match c.open_slots {
                    Some(mask) => clock.slot >= 64 || mask & (1u64 << clock.slot) == 0,
                    None => false,
                })
                && inbound.iter().any(|c| c.on_close.is_none())
        })
}
fn equal_serialized_views(a: &RunKernel, b: &RunKernel, content: &RunContent) -> bool {
    a.world.participants.len() == b.world.participants.len()
        && (0..a.world.participants.len() as PartIx).all(|participant| {
            match (
                serde_json::to_vec(&a.view(content, participant)),
                serde_json::to_vec(&b.view(content, participant)),
            ) {
                (Ok(left), Ok(right)) => left == right,
                _ => false,
            }
        })
}
fn random_index(rng: &mut SeededRng, len: usize) -> usize {
    ((u64::from(rng.next_u32()) * len as u64) >> 32) as usize
}

/// Explore the authored content exclusively through the same actions used by Play.
/// Explore independent seeds concurrently while merging results in seed order.
/// Unique telling orders are unioned, rather than summed across workers.
pub fn explore(content: &RunContent, cfg: &ExploreConfig) -> ExploreReport {
    let count = cfg.runs.min(50_000);
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8)
        .min(count.max(1) as usize);
    if workers <= 1 || count < 256 {
        return explore_chunk(content, cfg, 0, count);
    }
    let chunk = count.div_ceil(workers as u32);
    let partials = std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for offset in (0..count).step_by(chunk as usize) {
            let n = chunk.min(count - offset);
            jobs.push(scope.spawn(move || explore_chunk(content, cfg, offset, n)));
        }
        jobs.into_iter()
            .map(|job| job.join().expect("explorer worker panicked"))
            .collect::<Vec<_>>()
    });
    let mut merged = ExploreReport::default();
    for mut part in partials {
        merged.determinism_mismatches += part.determinism_mismatches;
        merged.error_count += part.error_count;
        merged.violations.append(&mut part.violations);
        merged.errors.append(&mut part.errors);
        if merged.cells.is_empty() {
            merged.cells = part.cells;
            continue;
        }
        for (cell, other) in merged.cells.iter_mut().zip(part.cells) {
            let mut endings: BTreeMap<_, _> = cell.endings.drain(..).collect();
            for (id, count) in other.endings {
                *endings.entry(id).or_default() += count;
            }
            cell.endings = endings.into_iter().collect();
            for (left, right) in cell
                .milestone_reached
                .iter_mut()
                .zip(other.milestone_reached)
            {
                left.1 += right.1;
            }
            for (left, right) in cell.probe_hits.iter_mut().zip(other.probe_hits) {
                left.1 += right.1;
                left.2 += right.2;
            }
            cell.telling_sequences.extend(other.telling_sequences);
            cell.telling_orders = cell.telling_sequences.len() as u32;
            let mut dead: BTreeMap<_, _> = cell
                .dead_air_by_location
                .drain(..)
                .map(|(id, empty, total)| (id, (empty, total)))
                .collect();
            for (id, empty, total) in other.dead_air_by_location {
                let entry = dead.entry(id).or_default();
                entry.0 += empty;
                entry.1 += total;
            }
            cell.dead_air_by_location = dead
                .into_iter()
                .map(|(id, (empty, total))| (id, empty, total))
                .collect();
        }
    }
    let ordering = |persona: &Persona, config: ParticipantConfig, seed: u64| {
        (
            cfg.personas
                .iter()
                .position(|p| p == persona)
                .unwrap_or(usize::MAX),
            cfg.configs
                .iter()
                .position(|c| *c == config)
                .unwrap_or(usize::MAX),
            seed.wrapping_sub(cfg.seed),
        )
    };
    merged
        .violations
        .sort_by_key(|v| ordering(&v.persona, v.config, v.seed));
    merged.violations.truncate(20);
    merged
        .errors
        .sort_by_key(|e| ordering(&e.persona, e.config, e.seed));
    merged.errors.truncate(20);
    merged
}
fn explore_chunk(
    content: &RunContent,
    cfg: &ExploreConfig,
    offset: u32,
    count: u32,
) -> ExploreReport {
    let mut report = ExploreReport::default();
    for persona in &cfg.personas {
        for &config in &cfg.configs {
            let configured = configured_content(content, config);
            let mut cell = ExploreCell {
                persona: persona.clone(),
                config,
                endings: vec![],
                milestone_reached: cfg.milestones.iter().map(|m| (m.key.clone(), 0)).collect(),
                probe_hits: cfg.probes.iter().map(|p| (p.name.clone(), 0, 0)).collect(),
                telling_orders: 0,
                dead_air_by_location: vec![],
                telling_sequences: BTreeSet::new(),
            };
            let mut endings = BTreeMap::<u16, u32>::new();
            let mut dead_air = BTreeMap::<u16, (u32, u32)>::new();
            let mut orders = BTreeSet::new();
            for run_no in offset..offset + count {
                let seed = cfg.seed.wrapping_add(u64::from(run_no));
                let mut run = match RunKernel::start(&configured, RunStart::Beginning, seed) {
                    Ok(run) => run,
                    Err(error) => {
                        report.error_count += 1;
                        if report.errors.len() < 20 {
                            report.errors.push(ExploreError {
                                seed,
                                persona: persona.clone(),
                                config,
                                step: 0,
                                message: format!("{error:?}"),
                            });
                        }
                        continue;
                    }
                };
                let mut replay = if report.cells.is_empty() && run_no < 200 {
                    RunKernel::start(&configured, RunStart::Beginning, seed).ok()
                } else {
                    None
                };
                let check_replay = report.cells.is_empty() && run_no < 200;
                let mut mismatch = check_replay
                    && replay
                        .as_ref()
                        .is_none_or(|other| !equal_serialized_views(&run, other, &configured));
                let mut rng = SeededRng::new((seed ^ (seed >> 32)) as u32);
                let mut actions = Vec::new();
                let mut reached = BTreeSet::new();
                let mut seen_probe = vec![0u32; cfg.probes.len()];
                let mut filled_probe = vec![0u32; cfg.probes.len()];
                for step in 0..cfg.max_steps {
                    let humans: Vec<_> = run.world.participants.humans().collect();
                    if humans.is_empty() {
                        break;
                    }
                    let available: Vec<_> = humans
                        .into_iter()
                        .filter(|p| {
                            run.world
                                .clock
                                .as_ref()
                                .and_then(|c| c.actions_left.get(*p as usize))
                                .is_none_or(|v| *v > 0)
                        })
                        .collect();
                    if available.is_empty() {
                        break;
                    }
                    let p = available[(step as usize) % available.len()];
                    let view = run.exploration_view(&configured, p);
                    if let Some(ending) = &view.ending {
                        *endings.entry(ending.id).or_default() += 1;
                        break;
                    }
                    if let Some(place) = &view.place {
                        let counts = dead_air.entry(place.id).or_default();
                        counts.1 += 1;
                        if view.nothing_here
                            || (view.hub.is_some()
                                && view.also_here.iter().all(|storylet| {
                                    configured
                                        .dispatcher
                                        .storylet(storylet.id)
                                        .is_some_and(|def| def.is_fallback)
                                }))
                        {
                            counts.0 += 1;
                        }
                    }
                    for (i, probe) in cfg.probes.iter().enumerate() {
                        if probe
                            .holder_knows
                            .iter()
                            .all(|f| run.world.knowledge.knows(p, *f).is_some())
                            && probe.colocated_with.is_none_or(|other| {
                                run.world.participants.colocated(p).any(|q| {
                                    run.world.participants.entity.get(q as usize) == Some(&other)
                                })
                            })
                        {
                            seen_probe[i] += 1;
                            if view
                                .also_here
                                .iter()
                                .any(|s| probe.expected_routes.contains(&s.id))
                            {
                                filled_probe[i] += 1;
                            }
                        }
                    }
                    let mut candidates = Vec::<RunAction>::new();
                    if let Some(scene) = &view.scene {
                        candidates.extend(
                            scene
                                .choices
                                .iter()
                                .filter(|c| c.enabled)
                                .map(|c| c.action.clone()),
                        );
                    }
                    if let Some(hub) = view.hub {
                        candidates.extend(view.also_here.iter().map(|s| RunAction::PlayStorylet {
                            node_guid: hub,
                            storylet: s.id,
                        }));
                    }
                    if let Some(place) = &view.place {
                        candidates.extend(
                            place
                                .moves
                                .iter()
                                .filter(|m| m.enabled)
                                .map(|m| RunAction::Move { to: m.to }),
                        );
                    }
                    if configured.clock.is_some() {
                        candidates.push(RunAction::Wait);
                    }
                    if candidates.is_empty() {
                        break;
                    }
                    let exploit = match persona {
                        Persona::Wanderer => false,
                        Persona::Investigator { epsilon_permille } => {
                            rng.next_u32() % 1000 >= u32::from((*epsilon_permille).min(1000))
                        }
                    };
                    let action = if exploit {
                        let ranked = candidates.iter().find(|a| matches!(a, RunAction::PlayStorylet { storylet, .. } if cfg.milestones.iter().any(|m| m.routes.contains(storylet))));
                        let choice = candidates.iter().find(|a| {
                            matches!(a, RunAction::Choose { .. } | RunAction::Topic { .. })
                        });
                        // Evaluate potential destinations against a copy of the actual world;
                        // eligibility is still determined by the normal deck view.
                        let mut destinations = Vec::new();
                        if view.hub.is_some() {
                            for &place in configured.entity_names.keys() {
                                if place as usize >= run.world.entity_tags.len()
                                    || Some(place) == run.world.active_location(p)
                                {
                                    continue;
                                }
                                if !run.storylets_at(&configured, p, place).is_empty() {
                                    destinations.push(place);
                                }
                            }
                        }
                        let travel = first_move_toward(&run.world, &configured, p, &destinations)
                            .and_then(|to| candidates.iter().find(|a| matches!(a, RunAction::Move { to: destination } if *destination == to)));
                        ranked
                            .or(choice)
                            .or(travel)
                            .or_else(|| {
                                candidates
                                    .iter()
                                    .find(|a| matches!(a, RunAction::PlayStorylet { .. }))
                            })
                            .cloned()
                            .unwrap_or_else(|| {
                                candidates[random_index(&mut rng, candidates.len())].clone()
                            })
                    } else {
                        candidates[random_index(&mut rng, candidates.len())].clone()
                    };
                    let slot_before = run.world.clock.as_ref().map(|clock| clock.slot);
                    if let Err(error) = run.act(&configured, p, action.clone()) {
                        report.error_count += 1;
                        if report.errors.len() < 20 {
                            report.errors.push(ExploreError {
                                seed,
                                persona: persona.clone(),
                                config,
                                step: actions.len() as u32 + 1,
                                message: format!("{error:?}"),
                            });
                        }
                        break;
                    }
                    for (i, milestone) in cfg.milestones.iter().enumerate() {
                        if milestone
                            .routes
                            .iter()
                            .any(|storylet| run.instance.completed_storylets.contains(*storylet))
                            && milestone.deadline_slot.is_none_or(|deadline| {
                                slot_before.is_none_or(|slot| slot <= deadline)
                            })
                        {
                            reached.insert(i);
                        }
                    }
                    if let Some(other) = &mut replay {
                        if other.act(&configured, p, action.clone()).is_err()
                            || !equal_serialized_views(&run, other, &configured)
                        {
                            mismatch = true;
                            replay = None;
                        }
                    }
                    actions.push((p, action));
                    if report.violations.len() < 20 {
                        if let Some((left, right)) = contradictory_pair(
                            &run.world,
                            &configured.registry,
                            &cfg.contradictory_props,
                        ) {
                            report.violations.push(Violation {
                                invariant: Invariant::ContradictoryProps(left, right),
                                seed,
                                persona: persona.clone(),
                                config,
                                step: actions.len() as u32,
                                actions: actions.clone(),
                            });
                            break;
                        }
                    }
                    if report.violations.len() < 20 && has_untold_past(&run.world, &configured) {
                        report.violations.push(Violation {
                            invariant: Invariant::PastWithoutTelling,
                            seed,
                            persona: persona.clone(),
                            config,
                            step: actions.len() as u32,
                            actions: actions.clone(),
                        });
                        break;
                    }
                    if report.violations.len() < 20
                        && has_unsourced_knowledge(&run.world, &configured.initial_world, |event| {
                            run.step_of_event(event).is_some()
                        })
                    {
                        report.violations.push(Violation {
                            invariant: Invariant::KnowledgeWithoutSource,
                            seed,
                            persona: persona.clone(),
                            config,
                            step: actions.len() as u32,
                            actions: actions.clone(),
                        });
                        break;
                    }
                    if report.violations.len() < 20
                        && has_unhandled_closed_occupant(&run.world, &configured)
                    {
                        report.violations.push(Violation {
                            invariant: Invariant::OccupantBeyondClosedEdge,
                            seed,
                            persona: persona.clone(),
                            config,
                            step: actions.len() as u32,
                            actions: actions.clone(),
                        });
                        break;
                    }
                }
                for i in reached {
                    cell.milestone_reached[i].1 += 1;
                }
                for (i, hits) in seen_probe.into_iter().enumerate() {
                    cell.probe_hits[i].1 += hits;
                }
                for (i, hits) in filled_probe.into_iter().enumerate() {
                    cell.probe_hits[i].2 += hits;
                }
                for p in 0..run.world.participants.len() as PartIx {
                    let order = run.world.telling.order_for(p);
                    if !order.is_empty() {
                        orders.insert(order);
                    }
                }
                if mismatch {
                    report.determinism_mismatches += 1;
                }
            }
            cell.endings = endings.into_iter().collect();
            cell.telling_orders = orders.len() as u32;
            cell.telling_sequences = orders;
            cell.dead_air_by_location = dead_air
                .into_iter()
                .map(|(place, (empty, total))| (place, empty, total))
                .collect();
            report.cells.push(cell);
        }
    }
    report
}
