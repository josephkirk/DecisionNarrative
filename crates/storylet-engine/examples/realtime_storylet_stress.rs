//! Realtime storylet stress test: nested branching graphs, concurrent/reentrant
//! storylets, item show/hide/give/take, and reusable combination endings.
//!
//! Run:
//!   cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example realtime_storylet_stress

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use storylet_engine::chrono_state::{GameplayTagRegistry, WorldStateTable};

const TRIALS: u64 = 500;
const MAX_TICKS: u64 = 24;
const MAX_ACTIVE: usize = 8;

const TAGS: [&str; 20] = [
    "radio.fixed",
    "radio.broken",
    "backroom.open",
    "alien.signal",
    "alien.contact",
    "alien.hostile",
    "zombie.vector",
    "zombie.outbreak",
    "trunk.opened",
    "trunk.body",
    "wife.zombie",
    "president.orders",
    "nuclear.launch",
    "truth.remembered",
    "divorce.accepted",
    "apocalypse.zombie",
    "apocalypse.alien",
    "apocalypse.nuclear",
    "apocalypse.silent",
    "panic",
];

#[derive(Clone, Debug)]
enum Effect {
    Tag(&'static str),
    Show(&'static str),
    Hide(&'static str),
    Give(&'static str),
    Take(&'static str),
}

#[derive(Clone, Debug)]
struct Choice {
    label: &'static str,
    requires_item: Option<&'static str>,
    requires_tag: Option<&'static str>,
    effects: Vec<Effect>,
    next: u8,
}

#[derive(Clone, Debug)]
enum Node {
    Dialogue { choices: Vec<Choice> },
    Condition { tag: &'static str, yes: u8, no: u8 },
    Item { effect: Effect, next: u8 },
    End,
}

#[derive(Clone, Debug)]
struct Graph {
    entry: u8,
    nodes: BTreeMap<u8, Node>,
}

#[derive(Clone, Debug)]
enum Trigger {
    Always,
    Tag(&'static str),
    Item(&'static str),
}

#[derive(Clone, Debug)]
struct Storylet {
    id: u16,
    name: &'static str,
    graph: Graph,
    trigger: Trigger,
}

fn choice(label: &'static str, then: u8) -> Choice {
    Choice {
        label,
        requires_item: None,
        requires_tag: None,
        effects: Vec::new(),
        next: then,
    }
}

fn with_effects(mut choice: Choice, effects: Vec<Effect>) -> Choice {
    choice.effects = effects;
    choice
}

fn graphs() -> Vec<Storylet> {
    let mut radio = BTreeMap::new();
    radio.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Fix the radio", 1),
                    vec![
                        Effect::Tag("radio.fixed"),
                        Effect::Tag("backroom.open"),
                        Effect::Give("radio_parts"),
                    ],
                ),
                with_effects(
                    choice("Help the sick", 2),
                    vec![Effect::Tag("zombie.vector"), Effect::Give("medkit")],
                ),
                with_effects(
                    choice("Smash the radio", 3),
                    vec![Effect::Tag("radio.broken"), Effect::Hide("flashlight")],
                ),
            ],
        },
    );
    radio.insert(
        1,
        Node::Condition {
            tag: "radio.fixed",
            yes: 4,
            no: 3,
        },
    );
    radio.insert(
        2,
        Node::Condition {
            tag: "zombie.vector",
            yes: 6,
            no: 3,
        },
    );
    radio.insert(3, Node::End);
    radio.insert(
        4,
        Node::Item {
            effect: Effect::Show("flashlight"),
            next: 5,
        },
    );
    radio.insert(
        5,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Take the flashlight", 3),
                    vec![Effect::Give("flashlight")],
                ),
                with_effects(choice("Leave it", 3), vec![Effect::Hide("flashlight")]),
            ],
        },
    );
    radio.insert(
        6,
        Node::Dialogue {
            choices: vec![
                with_effects(choice("Take the medkit", 3), vec![Effect::Give("medkit")]),
                with_effects(choice("Leave it", 3), vec![Effect::Hide("medkit")]),
            ],
        },
    );

    let mut reactive = BTreeMap::new();
    reactive.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Answer the voice", 1),
                    vec![Effect::Tag("alien.signal"), Effect::Give("headset")],
                ),
                with_effects(choice("Ignore it", 2), vec![Effect::Tag("alien.signal")]),
            ],
        },
    );
    reactive.insert(
        1,
        Node::Condition {
            tag: "alien.signal",
            yes: 3,
            no: 2,
        },
    );
    reactive.insert(2, Node::End);
    reactive.insert(
        3,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Open the channel", 2),
                    vec![Effect::Tag("alien.contact")],
                ),
                with_effects(
                    choice("Cut the power", 2),
                    vec![Effect::Tag("radio.broken"), Effect::Hide("flashlight")],
                ),
            ],
        },
    );

    let mut alien = BTreeMap::new();
    alien.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Negotiate", 1),
                    vec![
                        Effect::Tag("alien.contact"),
                        Effect::Tag("apocalypse.alien"),
                        Effect::Give("beacon"),
                    ],
                ),
                with_effects(
                    choice("Attack", 1),
                    vec![Effect::Tag("alien.hostile"), Effect::Take("headset")],
                ),
            ],
        },
    );
    alien.insert(1, Node::End);

    let mut trunk = BTreeMap::new();
    trunk.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Open the trunk", 1),
                    vec![Effect::Tag("trunk.opened"), Effect::Show("body")],
                ),
                choice("Leave it", 3),
            ],
        },
    );
    trunk.insert(
        1,
        Node::Condition {
            tag: "trunk.opened",
            yes: 2,
            no: 3,
        },
    );
    trunk.insert(
        2,
        Node::Item {
            effect: Effect::Show("body"),
            next: 4,
        },
    );
    trunk.insert(
        4,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Look inside", 3),
                    vec![
                        Effect::Tag("trunk.body"),
                        Effect::Tag("wife.zombie"),
                        Effect::Tag("zombie.vector"),
                        Effect::Take("flashlight"),
                    ],
                ),
                with_effects(
                    choice("Smell the decay", 3),
                    vec![
                        Effect::Tag("trunk.body"),
                        Effect::Tag("wife.zombie"),
                        Effect::Tag("zombie.vector"),
                        Effect::Tag("apocalypse.silent"),
                        Effect::Take("flashlight"),
                    ],
                ),
                with_effects(choice("Close it", 3), vec![Effect::Hide("body")]),
            ],
        },
    );
    trunk.insert(3, Node::End);

    let mut president = BTreeMap::new();
    president.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Order the launch", 1),
                    vec![Effect::Tag("president.orders")],
                ),
                with_effects(choice("Stand down", 1), vec![Effect::Tag("radio.broken")]),
            ],
        },
    );
    president.insert(1, Node::End);

    let mut general = BTreeMap::new();
    general.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Launch", 1),
                    vec![
                        Effect::Tag("nuclear.launch"),
                        Effect::Tag("apocalypse.nuclear"),
                    ],
                ),
                with_effects(choice("Refuse", 1), vec![Effect::Tag("radio.broken")]),
            ],
        },
    );
    general.insert(1, Node::End);

    let mut zombie = BTreeMap::new();
    zombie.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Triage the infected", 1),
                    vec![
                        Effect::Tag("zombie.outbreak"),
                        Effect::Tag("apocalypse.zombie"),
                        Effect::Take("medkit"),
                    ],
                ),
                with_effects(choice("Flee", 1), vec![Effect::Tag("panic")]),
            ],
        },
    );
    zombie.insert(1, Node::End);

    let mut truth = BTreeMap::new();
    truth.insert(
        0,
        Node::Dialogue {
            choices: vec![
                with_effects(
                    choice("Tell the truth", 1),
                    vec![Effect::Tag("truth.remembered")],
                ),
                with_effects(
                    choice("Accept the divorce", 1),
                    vec![Effect::Tag("divorce.accepted")],
                ),
                choice("Say nothing", 1),
            ],
        },
    );
    truth.insert(1, Node::End);

    vec![
        Storylet {
            id: 1,
            name: "radio",
            graph: Graph {
                entry: 0,
                nodes: radio,
            },
            trigger: Trigger::Always,
        },
        Storylet {
            id: 2,
            name: "reactive",
            graph: Graph {
                entry: 0,
                nodes: reactive,
            },
            trigger: Trigger::Tag("radio.fixed"),
        },
        Storylet {
            id: 3,
            name: "alien",
            graph: Graph {
                entry: 0,
                nodes: alien,
            },
            trigger: Trigger::Item("headset"),
        },
        Storylet {
            id: 4,
            name: "trunk",
            graph: Graph {
                entry: 0,
                nodes: trunk,
            },
            trigger: Trigger::Tag("backroom.open"),
        },
        Storylet {
            id: 5,
            name: "president",
            graph: Graph {
                entry: 0,
                nodes: president,
            },
            trigger: Trigger::Always,
        },
        Storylet {
            id: 6,
            name: "general",
            graph: Graph {
                entry: 0,
                nodes: general,
            },
            trigger: Trigger::Tag("president.orders"),
        },
        Storylet {
            id: 7,
            name: "zombie",
            graph: Graph {
                entry: 0,
                nodes: zombie,
            },
            trigger: Trigger::Tag("zombie.vector"),
        },
        Storylet {
            id: 8,
            name: "truth",
            graph: Graph {
                entry: 0,
                nodes: truth,
            },
            trigger: Trigger::Always,
        },
    ]
}

fn validate_graphs(storylets: &[Storylet]) -> (usize, usize, usize, usize) {
    let mut dialogue = 0;
    let mut item_nodes = 0;
    let mut conditions = 0;
    let mut reachable_failures = 0;
    for storylet in storylets {
        let mut seen = BTreeSet::new();
        let mut stack = vec![storylet.graph.entry];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            match storylet.graph.nodes.get(&id) {
                Some(Node::Dialogue { choices }) => {
                    dialogue += 1;
                    assert!(
                        (2..=4).contains(&choices.len()),
                        "{} dialogue node {id} has {} choices",
                        storylet.name,
                        choices.len()
                    );
                    for choice in choices {
                        if storylet.graph.nodes.contains_key(&choice.next) {
                            stack.push(choice.next);
                        }
                    }
                }
                Some(Node::Condition { yes, no, .. }) => {
                    conditions += 1;
                    stack.push(*yes);
                    stack.push(*no);
                }
                Some(Node::Item { next, .. }) => {
                    item_nodes += 1;
                    stack.push(*next);
                }
                Some(Node::End) => {}
                None => reachable_failures += 1,
            }
        }
        if seen.len() != storylet.graph.nodes.len() {
            reachable_failures += storylet.graph.nodes.len() - seen.len();
        }
    }
    (dialogue, item_nodes, conditions, reachable_failures)
}

struct World {
    tags: GameplayTagRegistry,
    state: WorldStateTable,
    inventory: BTreeMap<&'static str, u32>,
}

impl World {
    fn new() -> Self {
        let tags = GameplayTagRegistry::compile(TAGS.iter().map(|tag| (*tag).to_owned())).unwrap();
        World {
            tags,
            state: WorldStateTable::new(),
            inventory: BTreeMap::new(),
        }
    }

    fn has(&self, tag: &str) -> bool {
        self.state.global_tags.bit(self.tags.id(tag).unwrap())
    }

    fn add(&mut self, tag: &str) {
        let bit = self.tags.id(tag).unwrap();
        self.state.global_tags.set_bit(bit).unwrap();
    }

    fn hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for word in self.state.global_tags.words() {
            hash ^= *word;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        for value in &self.state.metrics {
            hash ^= value.to_bits() as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        for (item, count) in &self.inventory {
            for byte in item.bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x1000_0000_01b3);
            }
            hash ^= *count as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash
    }
}

struct Instance {
    id: u64,
    storylet: usize,
    node: u8,
    scene_items: BTreeSet<&'static str>,
}

#[derive(Default, Clone)]
struct Trial {
    endings: BTreeSet<String>,
    starts: u64,
    item_starts: u64,
    interleaved: u64,
    max_active: usize,
    choices: u64,
    items_shown: u64,
    items_hidden: u64,
    items_given: u64,
    items_taken: u64,
    completed: BTreeSet<&'static str>,
    unfinished: u64,
    ticks: u64,
    trace: Vec<String>,
}

fn choice_available(world: &World, choice: &Choice) -> bool {
    choice
        .requires_item
        .map(|item| world.inventory.get(item).copied().unwrap_or(0) > 0)
        .unwrap_or(true)
        && choice
            .requires_tag
            .map(|tag| world.has(tag))
            .unwrap_or(true)
}

#[derive(Default, Clone, Copy)]
struct EffectCounts {
    shown: u64,
    hidden: u64,
    given: u64,
    taken: u64,
}

fn apply_effects(world: &mut World, instance: &mut Instance, effects: &[Effect]) -> EffectCounts {
    let mut counts = EffectCounts::default();
    for effect in effects {
        match effect {
            Effect::Tag(tag) => world.add(tag),
            Effect::Show(item) => {
                if instance.scene_items.insert(item) {
                    counts.shown += 1;
                }
            }
            Effect::Hide(item) => {
                if instance.scene_items.remove(item) {
                    counts.hidden += 1;
                }
            }
            Effect::Give(item) => {
                instance.scene_items.remove(item);
                *world.inventory.entry(item).or_insert(0) += 1;
                counts.given += 1;
            }
            Effect::Take(item) => {
                if let Some(count) = world.inventory.get_mut(item) {
                    if *count > 0 {
                        *count -= 1;
                        counts.taken += 1;
                    }
                }
            }
        }
    }
    counts
}

fn choose_choice<'a>(
    policy: &str,
    choices: &'a [Choice],
    world: &World,
    rng: &mut u64,
) -> Option<&'a Choice> {
    let available: Vec<&Choice> = choices
        .iter()
        .filter(|choice| choice_available(world, choice))
        .collect();
    if available.is_empty() {
        return None;
    }
    if policy == "random" {
        return Some(available[(splitmix64(rng) as usize) % available.len()]);
    }
    let preferred: &[&str] = match policy {
        "alien" => &[
            "Fix the radio",
            "Answer the voice",
            "Open the channel",
            "Negotiate",
            "Stand down",
            "Refuse",
            "Leave it",
        ],
        "nuclear" => &["Order the launch", "Launch", "Leave it"],
        "zombie" => &[
            "Help the sick",
            "Triage the infected",
            "Stand down",
            "Refuse",
            "Leave it",
        ],
        "silent" => &[
            "Open the trunk",
            "Smell the decay",
            "Look inside",
            "Fix the radio",
            "Answer the voice",
            "Stand down",
            "Refuse",
        ],
        "combo_zombie" => &[
            "Fix the radio",
            "Open the trunk",
            "Look inside",
            "Tell the truth",
            "Triage the infected",
            "Stand down",
            "Refuse",
        ],
        "truth" => &["Smash the radio", "Tell the truth", "Stand down", "Refuse"],
        "divorce" => &[
            "Smash the radio",
            "Accept the divorce",
            "Stand down",
            "Refuse",
        ],
        _ => &[],
    };
    for label in preferred {
        if let Some(choice) = available.iter().find(|choice| choice.label == *label) {
            return Some(choice);
        }
    }
    Some(available[0])
}

fn maybe_start(
    storylets: &[Storylet],
    world: &World,
    active: &mut BTreeMap<u64, Instance>,
    started: &mut BTreeSet<u16>,
    next_instance: &mut u64,
    trial: &mut Trial,
    tick: u64,
) {
    for (index, storylet) in storylets.iter().enumerate() {
        if started.contains(&storylet.id) {
            continue;
        }
        let trigger = match storylet.trigger {
            Trigger::Always => true,
            Trigger::Tag(tag) => world.has(tag),
            Trigger::Item(item) => world.inventory.get(item).copied().unwrap_or(0) > 0,
        };
        if !trigger || active.len() >= MAX_ACTIVE {
            continue;
        }
        started.insert(storylet.id);
        trial.trace.push(format!("t{tick} start {}", storylet.name));
        if matches!(storylet.trigger, Trigger::Item(_)) {
            trial.item_starts += 1;
        }
        if !active.is_empty() {
            trial.interleaved += 1;
        }
        trial.starts += 1;
        trial.max_active = trial.max_active.max(active.len() + 1);
        let id = *next_instance;
        *next_instance += 1;
        active.insert(
            id,
            Instance {
                id,
                storylet: index,
                node: storylet.graph.entry,
                scene_items: BTreeSet::new(),
            },
        );
    }
}

fn resolve_ending(world: &World) -> String {
    let base = if world.has("truth.remembered") {
        "home"
    } else if world.has("divorce.accepted") {
        "divorce"
    } else {
        "stay"
    };
    let world_state = if world.has("apocalypse.silent") {
        "silent"
    } else if world.has("apocalypse.nuclear") {
        "nuclear"
    } else if world.has("apocalypse.zombie") {
        "zombie"
    } else if world.has("apocalypse.alien") {
        "alien"
    } else {
        "normal"
    };
    let wife = if world.has("wife.zombie") {
        "zombie_wife"
    } else {
        "normal_wife"
    };
    format!("{base}:{world_state}:{wife}")
}

fn run_trial(storylets: &[Storylet], seed: u64, policy: &str) -> (Trial, u64) {
    let mut world = World::new();
    let mut trial = Trial::default();
    let mut active: BTreeMap<u64, Instance> = BTreeMap::new();
    let mut started = BTreeSet::new();
    let mut next_instance = 0u64;
    let mut rng = seed;
    let mut tick = 0u64;

    maybe_start(
        storylets,
        &world,
        &mut active,
        &mut started,
        &mut next_instance,
        &mut trial,
        0,
    );

    while tick < MAX_TICKS && !active.is_empty() {
        tick += 1;
        trial.ticks += 1;
        let ids: Vec<u64> = active.keys().copied().collect();
        for id in ids {
            let (storylet_index, node, mut instance) = {
                let instance = active.get(&id).unwrap();
                (
                    instance.storylet,
                    instance.node,
                    Instance {
                        id: instance.id,
                        storylet: instance.storylet,
                        node: instance.node,
                        scene_items: instance.scene_items.clone(),
                    },
                )
            };
            let storylet = &storylets[storylet_index];
            let next = match storylet.graph.nodes.get(&node) {
                Some(Node::Dialogue { choices }) => {
                    if let Some(choice) = choose_choice(policy, choices, &world, &mut rng) {
                        trial.choices += 1;
                        trial.trace.push(format!(
                            "t{} {} node {}: {}",
                            tick, storylet.name, node, choice.label
                        ));
                        let counts = apply_effects(&mut world, &mut instance, &choice.effects);
                        trial.items_shown += counts.shown;
                        trial.items_hidden += counts.hidden;
                        trial.items_given += counts.given;
                        trial.items_taken += counts.taken;
                        Some(choice.next)
                    } else {
                        Some(255)
                    }
                }
                Some(Node::Condition { tag, yes, no }) => {
                    let branch = if world.has(tag) { *yes } else { *no };
                    trial.trace.push(format!(
                        "t{} {} node {}: condition {} -> {}",
                        tick, storylet.name, node, tag, branch
                    ));
                    Some(branch)
                }
                Some(Node::Item { effect, next }) => {
                    trial.trace.push(format!(
                        "t{} {} node {}: item {:?}",
                        tick, storylet.name, node, effect
                    ));
                    let counts =
                        apply_effects(&mut world, &mut instance, std::slice::from_ref(effect));
                    trial.items_shown += counts.shown;
                    trial.items_hidden += counts.hidden;
                    trial.items_given += counts.given;
                    trial.items_taken += counts.taken;
                    Some(*next)
                }
                Some(Node::End) => Some(255),
                None => Some(255),
            };
            if let Some(next) = next {
                if next == 255 || !storylet.graph.nodes.contains_key(&next) {
                    trial.completed.insert(storylet.name);
                    trial.trace.push(format!("t{tick} {} end", storylet.name));
                    active.remove(&id);
                } else {
                    instance.node = next;
                    active.insert(id, instance);
                }
            }
            maybe_start(
                storylets,
                &world,
                &mut active,
                &mut started,
                &mut next_instance,
                &mut trial,
                tick,
            );
        }
    }

    trial.unfinished = active.len() as u64;
    trial.endings.insert(resolve_ending(&world));
    (trial, world.hash())
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    z ^ (z >> 31)
}

#[derive(Default)]
struct Stats {
    trials: u64,
    endings: BTreeMap<String, u64>,
    starts: u64,
    item_starts: u64,
    interleaved: u64,
    max_active: usize,
    choices: u64,
    items_shown: u64,
    items_hidden: u64,
    items_given: u64,
    items_taken: u64,
    completed: BTreeSet<&'static str>,
    unfinished: u64,
}

fn main() {
    let storylets = graphs();
    let (dialogues, item_nodes, conditions, reachability_failures) = validate_graphs(&storylets);
    let started = Instant::now();
    let policies = [
        "random",
        "alien",
        "nuclear",
        "zombie",
        "silent",
        "truth",
        "divorce",
        "combo_zombie",
    ];
    let mut all_endings = BTreeSet::new();
    let mut all_bases = BTreeSet::new();
    let mut all_worlds = BTreeSet::new();
    let mut all_completed = BTreeSet::new();
    let mut total_trials = 0u64;
    let mut longest: Option<(String, u64, Trial)> = None;

    println!(
        "Realtime storylet stress: {} storylets, {} dialogue nodes, {} condition nodes, {} item nodes, {} trials/policy",
        storylets.len(),
        dialogues,
        conditions,
        item_nodes,
        TRIALS
    );
    println!(
        "graph validation: unreachable nodes {} | max active {}",
        reachability_failures, MAX_ACTIVE
    );

    for policy in policies {
        let mut stats = Stats::default();
        for seed in 0..TRIALS {
            let (trial, _) = run_trial(&storylets, seed, policy);
            if longest
                .as_ref()
                .map(|(_, _, current)| trial.trace.len() > current.trace.len())
                .unwrap_or(true)
            {
                longest = Some((policy.to_owned(), seed, trial.clone()));
            }
            stats.trials += 1;
            for ending in &trial.endings {
                *stats.endings.entry(ending.clone()).or_insert(0) += 1;
                let mut parts = ending.split(':');
                if let (Some(base), Some(world)) = (parts.next(), parts.next()) {
                    all_bases.insert(base.to_owned());
                    all_worlds.insert(world.to_owned());
                }
            }
            stats.starts += trial.starts;
            stats.item_starts += trial.item_starts;
            stats.interleaved += trial.interleaved;
            stats.max_active = stats.max_active.max(trial.max_active);
            stats.choices += trial.choices;
            stats.items_shown += trial.items_shown;
            stats.items_hidden += trial.items_hidden;
            stats.items_given += trial.items_given;
            stats.items_taken += trial.items_taken;
            stats.completed.extend(trial.completed);
            stats.unfinished += trial.unfinished;
            for ending in trial.endings {
                all_endings.insert(ending);
            }
        }
        all_completed.extend(stats.completed.iter().copied());
        total_trials += stats.trials;
        println!("\nPOLICY {policy}");
        println!(
            "  endings {:?} | starts {} (item {}) | interleaved {} | max active {} | unfinished {}",
            stats
                .endings
                .iter()
                .map(|(ending, count)| (ending.as_str(), *count))
                .collect::<BTreeMap<_, _>>(),
            stats.starts,
            stats.item_starts,
            stats.interleaved,
            stats.max_active,
            stats.unfinished
        );
        println!(
            "  choices {} | shown {} | hidden {} | given {} | taken {} | completed {:?}",
            stats.choices,
            stats.items_shown,
            stats.items_hidden,
            stats.items_given,
            stats.items_taken,
            stats.completed
        );
    }

    let same_a = run_trial(&storylets, 42, "alien");
    let same_b = run_trial(&storylets, 42, "alien");
    let deterministic = same_a.1 == same_b.1 && same_a.0.endings == same_b.0.endings;

    if let Some((policy, seed, trial)) = &longest {
        println!("\nLONGEST PLAYTHROUGH: policy {policy} seed {seed}");
        println!(
            "  ticks {} | choices {} | starts {} | trace steps {}",
            trial.ticks,
            trial.choices,
            trial.starts,
            trial.trace.len()
        );
        for (index, step) in trial.trace.iter().enumerate() {
            println!("  {:02}. {}", index + 1, step);
        }
    }

    println!("\nGLOBAL");
    println!("  endings {} {:?}", all_endings.len(), all_endings);
    let mut base_worlds: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut world_bases: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ending in &all_endings {
        let parts: Vec<&str> = ending.split(':').collect();
        if parts.len() == 3 {
            base_worlds
                .entry(parts[0].to_owned())
                .or_default()
                .insert(parts[1].to_owned());
            world_bases
                .entry(parts[1].to_owned())
                .or_default()
                .insert(parts[0].to_owned());
        }
    }
    let reused_bases = base_worlds
        .values()
        .filter(|worlds| worlds.len() >= 2)
        .count();
    let reused_worlds = world_bases
        .values()
        .filter(|bases| bases.len() >= 2)
        .count();
    println!(
        "  bases {:?} | worlds {:?} | combinations {} | reused bases {} | reused worlds {} | completed {:?}",
        all_bases, all_worlds, all_endings.len(), reused_bases, reused_worlds, all_completed
    );
    println!(
        "  trials {} | elapsed {:?} | deterministic {}",
        total_trials,
        started.elapsed(),
        deterministic
    );
}
