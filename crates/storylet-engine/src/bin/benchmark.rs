use std::{
    collections::{BTreeMap, BTreeSet},
    hint::black_box,
    time::{Duration, Instant},
};

use serde_json::json;
use storylet_engine::{
    ActorState, ClusterType, DirectorState, Effect, NpcBehaviorRule, PredicateSet, Repeatability,
    RoleRequirement, SceneContent, StoryletDefinition, WorldCommand, WorldState, advance_npcs,
    apply_command, evaluate_storylets,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let definitions_count = arg(&args, "--definitions", 10_000);
    let actors_count = arg(&args, "--actors", 100);
    let locations_count = arg(&args, "--locations", 20);
    let iterations = arg(&args, "--iterations", 50);
    if definitions_count == 0 || actors_count == 0 || locations_count == 0 || iterations < 2 {
        eprintln!("definitions, actors, and locations must be positive; iterations must be >= 2");
        std::process::exit(2);
    }

    let definitions = make_definitions(definitions_count, locations_count);
    let rules = make_npc_rules(actors_count);
    let director = DirectorState::default();
    let base_world = make_world(actors_count, locations_count);

    let cold_started = Instant::now();
    let cold = evaluate_storylets(&definitions, &base_world, &director, 0.15);
    let cold_time = cold_started.elapsed();
    black_box(cold);

    let mut selection_samples = Vec::with_capacity(iterations);
    let mut full_turn_samples = Vec::with_capacity(iterations);
    for iteration in 0..iterations {
        let selection_world = make_world(actors_count, locations_count);
        let started = Instant::now();
        black_box(evaluate_storylets(
            &definitions,
            &selection_world,
            &director,
            0.15,
        ));
        selection_samples.push(started.elapsed());

        let mut world = make_world(actors_count, locations_count);
        let started = Instant::now();
        black_box(evaluate_storylets(&definitions, &world, &director, 0.15));
        let player = apply_command(
            &mut world,
            WorldCommand {
                idempotency_key: format!("bench-player-{iteration}"),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "wait".into(),
                effects: vec![],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .expect("benchmark player command must be valid");
        let report = advance_npcs(&mut world, &rules, actors_count, &[player.event.id])
            .expect("benchmark NPC rules must be valid");
        black_box((world, report));
        full_turn_samples.push(started.elapsed());
    }

    let output = json!({
        "schemaVersion": 1,
        "engine": "storylet-engine",
        "mode": if cfg!(debug_assertions) { "debug" } else { "release" },
        "workload": {
            "storyletDefinitions": definitions_count,
            "npcActors": actors_count,
            "locations": locations_count,
            "npcRulesPerTurn": rules.len(),
            "iterations": iterations,
            "storage": "in-memory",
        },
        "selection": {
            "coldMs": millis(cold_time),
            "warmP50Ms": millis(percentile(&mut selection_samples.clone(), 0.50)),
            "warmP95Ms": millis(percentile(&mut selection_samples.clone(), 0.95)),
            "warmMaxMs": millis(*selection_samples.iter().max().unwrap()),
        },
        "completeTurnWithNpcSimulation": {
            "warmP50Ms": millis(percentile(&mut full_turn_samples.clone(), 0.50)),
            "warmP95Ms": millis(percentile(&mut full_turn_samples.clone(), 0.95)),
            "warmMaxMs": millis(*full_turn_samples.iter().max().unwrap()),
        },
        "host": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "processorIdentifier": std::env::var("PROCESSOR_IDENTIFIER").ok(),
            "availableParallelism": std::thread::available_parallelism().map(|v| v.get()).ok(),
        },
        "notes": [
            "Measures deterministic eligibility, role binding, scoring, and sorting.",
            "Complete-turn measurement includes full candidate evaluation, one player wait, and all configured NPC rule evaluations/actions.",
            "Run on target hardware for runtime acceptance; this harness reports CPU-side Rust runtime costs.",
        ],
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn arg(args: &[String], name: &str, default: usize) -> usize {
    args.iter()
        .position(|value| value == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn percentile(samples: &mut [Duration], fraction: f64) -> Duration {
    samples.sort_unstable();
    let index = ((samples.len() as f64 * fraction).ceil() as usize)
        .saturating_sub(1)
        .min(samples.len() - 1);
    samples[index]
}

fn make_world(actors_count: usize, locations_count: usize) -> WorldState {
    let mut actors = BTreeMap::new();
    for index in 0..actors_count {
        actors.insert(
            format!("npc-{index:03}"),
            ActorState {
                role_tags: BTreeSet::from(["witness".into()]),
                current_location: format!("location-{:02}", index % locations_count),
                busy: false,
            },
        );
    }
    let mut world = WorldState {
        current_location: Some("location-00".into()),
        actors,
        ..WorldState::default()
    };
    world.qualities.insert("quality.pressure".into(), json!(2));
    world.qualities.insert("quality.clock".into(), json!(0));
    world
}

fn make_definitions(count: usize, _locations_count: usize) -> Vec<StoryletDefinition> {
    (0..count)
        .map(|index| StoryletDefinition {
            id: format!("storylet-{index:05}"),
            revision: 1,
            title: format!("Generic scene {index}"),
            summary: "A generic benchmark scene".into(),
            approved: true,
            ending: false,
            location_tags: Default::default(),
            urgency_weight: 25.0 + (index % 75) as f64,
            cluster_type: ClusterType::PlayerControlledPassage,
            repeatability: Repeatability::Always,
            required_roles: vec![RoleRequirement {
                role: "witness".into(),
            }],
            trigger: PredicateSet::default(),
            preconditions: Default::default(),
            content: SceneContent {
                opening: "A person notices something worth discussing.".into(),
            },
            choices: vec![],
        })
        .collect()
}

fn make_npc_rules(count: usize) -> Vec<NpcBehaviorRule> {
    (0..count)
        .map(|index| NpcBehaviorRule {
            id: format!("npc-rule-{index:03}"),
            actor_id: format!("npc-{index:03}"),
            priority: 10,
            conditions: PredicateSet::default(),
            action: "advance a generic personal agenda".into(),
            effects: vec![Effect::AddQuality {
                key: "quality.clock".into(),
                amount: 1.0,
            }],
        })
        .collect()
}
