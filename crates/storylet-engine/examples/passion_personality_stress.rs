//! Reproducible stress test for personality gates, Ken Levine-style passion
//! approvals, item gating, chance, role replacement, and an authored break.
//!
//! Fixture: 6 actors (player + 5 NPCs) x 40 traits = 240 global metrics, plus
//! 5 macro passions with approve/hostile threshold flags.
//!
//! The harness is standalone: it never opens the studio database or mutates the
//! Tell-Tale Heart project. It exists to reproduce the limits measured in the
//! project analysis.
//!
//! Run:
//!   cargo run --manifest-path crates/storylet-engine/Cargo.toml --example passion_personality_stress

use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::chrono_dispatch::{
    MetricGate, RoleRequirement, StoryletDef, UnifiedDispatcher,
};
use storylet_engine::chrono_state::{
    apply_mutation_with_registry, apply_passion_mutation, EntityTagMutation, GameplayTagRegistry,
    MetricBound, MetricRegistry, MicroPassionDef, MutationOp, PassionDirection, PassionMutation,
    PassionRegistry, PassionThresholdRule, RivalPassionLink, StarNpcDef, TagBitset,
    WorldStateTable,
};

const ENTITIES: usize = 6;
const TRAITS: usize = 40;
const NPCS: usize = 5;
const TRIALS: u64 = 5_000;

const HALL: &str = "loc.hall";
const ROOM: &str = "loc.room";
const ROLE_KEYHOLDER: &str = "role.keyholder";
const ROLE_GIRL: &str = "role.girl";
const ROLE_MONEY: &str = "role.money";
const OLD_ALIVE: &str = "oldman.alive";
const OLD_DEAD: &str = "oldman.dead";
const GIRL_PRESENT: &str = "girl.present";
const KICKER_PRESENT: &str = "kicker.present";
const JANITOR_PRESENT: &str = "janitor.present";
const CAREGIVER_PRESENT: &str = "caregiver.present";
const VENDING: &str = "vending";
const COIN: &str = "item.coin";
const HAIRPIN: &str = "item.hairpin";
const LOCKPICK: &str = "item.lockpick";
const MANUAL: &str = "item.manual";
const KEY_OFFERED: &str = "item.key.offered";
const KEY: &str = "item.key";
const CENTER_LOCKED: &str = "door.center.locked";
const CENTER_OPEN: &str = "door.center.open";
const STORAGE_LOCKED: &str = "door.storage.locked";
const STORAGE_OPEN: &str = "door.storage.open";
const KICKED: &str = "act.kicked";
const REFUSED: &str = "oldman.refused";
const TRUST_OLDMAN: &str = "oldman.trust";
const CLUE_KICK: &str = "clue.kick";
const BREAK_READY: &str = "break.ready";

const TRAIT_NAMES: [&str; 8] = [
    "charisma",
    "assertive",
    "empathy",
    "honesty",
    "courage",
    "patience",
    "curiosity",
    "loyalty",
];

fn trait_name(index: usize) -> String {
    if index < TRAIT_NAMES.len() {
        TRAIT_NAMES[index].to_owned()
    } else {
        format!("passion{index:02}")
    }
}

fn metric_name(character: usize, trait_index: usize) -> String {
    format!("c{character}.{}", trait_name(trait_index))
}

fn tags() -> Vec<String> {
    [
        HALL,
        ROOM,
        ROLE_KEYHOLDER,
        ROLE_GIRL,
        ROLE_MONEY,
        OLD_ALIVE,
        OLD_DEAD,
        GIRL_PRESENT,
        KICKER_PRESENT,
        JANITOR_PRESENT,
        CAREGIVER_PRESENT,
        VENDING,
        COIN,
        HAIRPIN,
        LOCKPICK,
        MANUAL,
        KEY_OFFERED,
        KEY,
        CENTER_LOCKED,
        CENTER_OPEN,
        STORAGE_LOCKED,
        STORAGE_OPEN,
        KICKED,
        REFUSED,
        TRUST_OLDMAN,
        CLUE_KICK,
        BREAK_READY,
        "approve.oldman",
        "hostile.oldman",
        "approve.girl",
        "hostile.girl",
        "approve.kicker",
        "hostile.kicker",
        "approve.janitor",
        "hostile.janitor",
        "approve.caregiver",
        "hostile.caregiver",
    ]
    .iter()
    .map(|name| (*name).to_owned())
    .collect()
}

fn mask(registry: &GameplayTagRegistry, names: &[&str]) -> TagBitset {
    let mut bits = TagBitset::new();
    for name in names {
        bits.set_bit(registry.id(name).unwrap()).unwrap();
    }
    bits
}

fn mutation(registry: &GameplayTagRegistry, add: &[&str], remove: &[&str]) -> MutationOp {
    MutationOp {
        add_global_tags: mask(registry, add),
        remove_global_tags: mask(registry, remove),
        ..Default::default()
    }
}

fn role(role_tag: &str, location_tag: &str) -> RoleRequirement {
    RoleRequirement {
        role_tag: role_tag.to_owned(),
        location_tag: Some(location_tag.to_owned()),
        ..Default::default()
    }
}

fn gate(metrics: &MetricRegistry, name: &str, min: f32, max: f32) -> MetricGate {
    MetricGate {
        metric_id: metrics.id(name).unwrap(),
        min,
        max,
    }
}

#[allow(clippy::too_many_arguments)]
fn storylet(
    registry: &GameplayTagRegistry,
    id: u16,
    repeatable: bool,
    required: &[&str],
    required_any: &[&str],
    forbidden: &[&str],
    gates: Vec<MetricGate>,
    roles: Vec<RoleRequirement>,
    on_complete: MutationOp,
) -> StoryletDef {
    StoryletDef {
        storylet_id: id,
        is_repeatable: repeatable,
        required_tags: mask(registry, required),
        required_any_tags: mask(registry, required_any),
        forbidden_tags: mask(registry, forbidden),
        metric_gates: gates,
        required_roles: roles,
        on_complete_mutation: on_complete,
        ..Default::default()
    }
}

struct Fixture {
    tags: GameplayTagRegistry,
    metrics: MetricRegistry,
    dispatcher: UnifiedDispatcher,
    passions: PassionRegistry,
    metric_indices: Vec<u16>,
    trait_names: Vec<String>,
    rival_link: bool,
}

fn build_fixture(rival_link: bool) -> Fixture {
    let tag_registry = GameplayTagRegistry::compile(tags()).unwrap();

    let mut metric_names = Vec::with_capacity(ENTITIES * TRAITS + 2);
    let mut metric_indices = Vec::with_capacity(ENTITIES * TRAITS);
    let mut trait_names = Vec::with_capacity(TRAITS);
    for trait_index in 0..TRAITS {
        trait_names.push(trait_name(trait_index));
    }
    for character in 0..ENTITIES {
        for trait_index in 0..TRAITS {
            metric_names.push(metric_name(character, trait_index));
        }
    }
    metric_names.push("turn".to_owned());
    metric_names.push("luck".to_owned());
    let metric_registry = MetricRegistry::compile(metric_names).unwrap();
    for character in 0..ENTITIES {
        for trait_index in 0..TRAITS {
            metric_indices.push(
                metric_registry
                    .id(&metric_name(character, trait_index))
                    .unwrap(),
            );
        }
    }

    let effect = |add: &[&str], remove: &[&str]| mutation(&tag_registry, add, remove);
    let mut defs = Vec::new();

    defs.push(storylet(
        &tag_registry,
        100,
        false,
        &[HALL, OLD_ALIVE],
        &[],
        &["hostile.oldman", REFUSED],
        vec![gate(&metric_registry, "c1.empathy", 30.0, f32::MAX)],
        vec![role(ROLE_KEYHOLDER, HALL)],
        effect(&[KEY_OFFERED, TRUST_OLDMAN], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        101,
        false,
        &[HALL, OLD_DEAD, CAREGIVER_PRESENT],
        &[],
        &[],
        vec![gate(&metric_registry, "c5.empathy", 35.0, f32::MAX)],
        vec![role(ROLE_KEYHOLDER, HALL)],
        effect(&[KEY_OFFERED], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        102,
        false,
        &[KEY_OFFERED],
        &[],
        &[],
        vec![],
        vec![],
        effect(&[KEY], &[KEY_OFFERED]),
    ));
    defs.push(storylet(
        &tag_registry,
        103,
        false,
        &[HALL, KEY, CENTER_LOCKED],
        &[],
        &[],
        vec![],
        vec![],
        effect(&[CENTER_OPEN], &[CENTER_LOCKED]),
    ));
    defs.push(storylet(
        &tag_registry,
        104,
        false,
        &[HALL, OLD_ALIVE],
        &[],
        &[],
        vec![],
        vec![role(ROLE_KEYHOLDER, HALL)],
        effect(&[REFUSED], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        105,
        false,
        &[HALL, REFUSED],
        &[],
        &[OLD_DEAD],
        vec![],
        vec![],
        effect(&[TRUST_OLDMAN], &[REFUSED]),
    ));
    defs.push(storylet(
        &tag_registry,
        106,
        false,
        &[HALL, OLD_ALIVE],
        &["approve.oldman", TRUST_OLDMAN],
        &["hostile.oldman", REFUSED],
        vec![gate(&metric_registry, "c0.charisma", 40.0, f32::MAX)],
        vec![role(ROLE_KEYHOLDER, HALL)],
        effect(&[KEY_OFFERED], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        107,
        false,
        &[HALL, GIRL_PRESENT],
        &[],
        &[],
        vec![gate(&metric_registry, "c0.empathy", 25.0, f32::MAX)],
        vec![],
        MutationOp::default(),
    ));
    defs.push(storylet(
        &tag_registry,
        108,
        false,
        &[HALL, GIRL_PRESENT, "approve.girl"],
        &[],
        &["hostile.girl"],
        vec![gate(&metric_registry, "c0.empathy", 30.0, f32::MAX)],
        vec![role(ROLE_GIRL, HALL)],
        effect(&[HAIRPIN], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        109,
        false,
        &[HALL, VENDING],
        &[],
        &[],
        vec![],
        vec![],
        effect(&[KICKED], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        110,
        false,
        &[HALL],
        &[],
        &[],
        vec![],
        vec![role(ROLE_MONEY, HALL)],
        effect(&[CLUE_KICK], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        111,
        false,
        &[HALL, COIN],
        &[],
        &[],
        vec![gate(&metric_registry, "c0.honesty", 30.0, f32::MAX)],
        vec![role(ROLE_MONEY, HALL)],
        effect(&[], &[COIN]),
    ));
    defs.push(storylet(
        &tag_registry,
        112,
        false,
        &[HALL],
        &["approve.kicker", "approve.janitor"],
        &[],
        vec![gate(&metric_registry, "c0.honesty", 30.0, f32::MAX)],
        vec![role(ROLE_MONEY, HALL)],
        effect(&[LOCKPICK], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        113,
        true,
        &[ROOM, STORAGE_LOCKED],
        &[HAIRPIN, LOCKPICK],
        &[],
        vec![],
        vec![],
        MutationOp::default(),
    ));
    defs.push(storylet(
        &tag_registry,
        114,
        false,
        &[ROOM, STORAGE_OPEN],
        &[],
        &[],
        vec![],
        vec![],
        effect(&[MANUAL], &[]),
    ));
    defs.push(storylet(
        &tag_registry,
        115,
        true,
        &[HALL, HAIRPIN, MANUAL, CENTER_LOCKED],
        &[],
        &[],
        vec![],
        vec![],
        MutationOp::default(),
    ));
    defs.push(storylet(
        &tag_registry,
        116,
        true,
        &[HALL, LOCKPICK, MANUAL, CENTER_LOCKED],
        &[],
        &[],
        vec![],
        vec![],
        MutationOp::default(),
    ));
    defs.push(storylet(
        &tag_registry,
        117,
        false,
        &[HALL],
        &[HAIRPIN, LOCKPICK],
        &[],
        vec![],
        vec![],
        effect(&[ROOM], &[HALL]),
    ));
    defs.push(storylet(
        &tag_registry,
        118,
        false,
        &[ROOM],
        &[],
        &[],
        vec![],
        vec![],
        effect(&[HALL], &[ROOM]),
    ));
    defs.push(storylet(
        &tag_registry,
        119,
        false,
        &[HALL, OLD_DEAD],
        &[],
        &[KEY],
        vec![],
        vec![],
        MutationOp::default(),
    ));
    defs.push(storylet(
        &tag_registry,
        120,
        false,
        &[HALL, JANITOR_PRESENT, "approve.janitor"],
        &[],
        &[],
        vec![gate(&metric_registry, "c0.assertive", 45.0, f32::MAX)],
        vec![role(ROLE_MONEY, HALL)],
        effect(&[CENTER_OPEN], &[CENTER_LOCKED]),
    ));
    defs.push(storylet(
        &tag_registry,
        121,
        false,
        &[HALL, CENTER_LOCKED, BREAK_READY],
        &[],
        &[],
        vec![gate(&metric_registry, "c0.courage", 40.0, f32::MAX)],
        vec![],
        effect(&[CENTER_OPEN], &[CENTER_LOCKED]),
    ));

    let mut dispatcher = UnifiedDispatcher::new(defs);
    dispatcher.resolve(&tag_registry, &metric_registry).unwrap();

    let mut npcs = Vec::new();
    for npc in 0..NPCS {
        npcs.push(StarNpcDef {
            npc_id: npc as u16,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: npc as u16,
                passion_id: npc as u16,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: MetricBound {
                min: -1.0,
                max: 1.0,
            },
            zero_sum_links: if rival_link && npc == 2 {
                vec![RivalPassionLink {
                    target_character_id: 3,
                    source_passion_id: 2,
                    target_passion_id: 3,
                    weight: -0.5,
                }]
            } else {
                Vec::new()
            },
        });
    }
    let mut thresholds = Vec::new();
    for npc in 0..NPCS {
        let approve = format!("approve.{}", npc_name(npc));
        let hostile = format!("hostile.{}", npc_name(npc));
        thresholds.push(PassionThresholdRule {
            npc_id: npc as u16,
            threshold_tag_bit: tag_registry.id(&approve).unwrap(),
            trigger_value: 0.2,
            direction: PassionDirection::Up,
            linked_graph_asset_id: format!("graph.{approve}"),
        });
        thresholds.push(PassionThresholdRule {
            npc_id: npc as u16,
            threshold_tag_bit: tag_registry.id(&hostile).unwrap(),
            trigger_value: -0.2,
            direction: PassionDirection::Down,
            linked_graph_asset_id: format!("graph.{hostile}"),
        });
    }
    let passions = PassionRegistry::compile(npcs, thresholds).unwrap();

    Fixture {
        tags: tag_registry,
        metrics: metric_registry,
        dispatcher,
        passions,
        metric_indices,
        trait_names,
        rival_link,
    }
}

fn npc_name(npc: usize) -> &'static str {
    match npc {
        0 => "oldman",
        1 => "girl",
        2 => "kicker",
        3 => "janitor",
        _ => "caregiver",
    }
}

fn has(state: &WorldStateTable, tags: &GameplayTagRegistry, name: &str) -> bool {
    state.global_tags.bit(tags.id(name).unwrap())
}

fn set_tag(state: &mut WorldStateTable, tags: &GameplayTagRegistry, name: &str, on: bool) {
    let bit = tags.id(name).unwrap();
    if on {
        state.global_tags.set_bit(bit).unwrap();
    } else {
        state.global_tags.clear_bit(bit).unwrap();
    }
}

fn set_entity_tag(
    state: &mut WorldStateTable,
    tags: &GameplayTagRegistry,
    entity: usize,
    name: &str,
    on: bool,
) {
    let bit = tags.id(name).unwrap();
    if on {
        state.entity_tags[entity].set_bit(bit).unwrap();
    } else {
        state.entity_tags[entity].clear_bit(bit).unwrap();
    }
}

fn initial(fixture: &Fixture) -> WorldStateTable {
    let mut state = WorldStateTable::new();
    for name in ["turn", "luck"] {
        state
            .set_metric_bound(fixture.metrics.id(name).unwrap(), 0.0, 1000.0)
            .unwrap();
    }
    for character in 0..ENTITIES {
        for trait_index in 0..TRAITS {
            let index = fixture
                .metrics
                .id(&metric_name(character, trait_index))
                .unwrap();
            state.set_metric_bound(index, 0.0, 100.0).unwrap();
            let base = 40.0 + ((character * 7 + trait_index * 3) % 21) as f32 - 10.0;
            state.metrics[index as usize] = base;
        }
    }
    for (character, name, value) in [
        (0, "charisma", 40.0),
        (0, "assertive", 40.0),
        (0, "empathy", 40.0),
        (0, "honesty", 40.0),
        (0, "courage", 45.0),
        (1, "empathy", 50.0),
        (1, "patience", 50.0),
        (2, "empathy", 50.0),
        (3, "empathy", 35.0),
        (3, "assertive", 55.0),
        (4, "empathy", 50.0),
        (4, "honesty", 55.0),
        (5, "empathy", 60.0),
        (5, "honesty", 60.0),
    ] {
        let index = fixture.metrics.id(&format!("c{character}.{name}")).unwrap();
        state.metrics[index as usize] = value;
    }
    for name in [
        HALL,
        VENDING,
        CENTER_LOCKED,
        STORAGE_LOCKED,
        OLD_ALIVE,
        GIRL_PRESENT,
        KICKER_PRESENT,
    ] {
        set_tag(&mut state, &fixture.tags, name, true);
    }
    for (entity, role_tag) in [(1, ROLE_KEYHOLDER), (2, ROLE_GIRL), (3, ROLE_MONEY)] {
        set_entity_tag(&mut state, &fixture.tags, entity, HALL, true);
        set_entity_tag(&mut state, &fixture.tags, entity, role_tag, true);
    }
    set_entity_tag(&mut state, &fixture.tags, 4, ROLE_MONEY, true);
    set_entity_tag(&mut state, &fixture.tags, 5, ROLE_KEYHOLDER, true);
    for npc in 0..NPCS {
        state.passions.active[npc] = true;
    }
    state
}

fn metric(fixture: &Fixture, state: &WorldStateTable, name: &str) -> f32 {
    state.metrics[fixture.metrics.id(name).unwrap() as usize]
}

fn is_playable(
    fixture: &Fixture,
    state: &WorldStateTable,
    completed: &BTreeSet<u16>,
    action: u16,
) -> bool {
    fixture
        .dispatcher
        .storylet(action)
        .map(|def| fixture.dispatcher.is_playable(def, state, completed))
        .unwrap_or(false)
}

fn bound_entity(fixture: &Fixture, state: &WorldStateTable, action: u16) -> Option<usize> {
    let def = fixture.dispatcher.storylet(action)?;
    fixture
        .dispatcher
        .bind_roles(def, state)
        .ok()
        .and_then(|bindings| bindings.first().map(|binding| binding.entity_index))
}

fn explicit_trait_delta(action: u16, character: usize, kind: &str) -> f32 {
    if character == 0 {
        match (action, kind) {
            (100, "charisma") => 2.0,
            (100, "empathy") => 1.0,
            (100, "honesty") => 1.0,
            (104, "assertive") => 8.0,
            (104, "empathy") => -10.0,
            (104, "charisma") => -4.0,
            (105, "honesty") => 4.0,
            (105, "empathy") => 4.0,
            (105, "charisma") => 2.0,
            (106, "charisma") => 3.0,
            (107, "empathy") => 4.0,
            (107, "charisma") => 2.0,
            (108, "empathy") => 2.0,
            (109, "assertive") => 8.0,
            (109, "empathy") => -10.0,
            (109, "charisma") => -3.0,
            (110, "empathy") => 2.0,
            (111, "honesty") => 4.0,
            (111, "empathy") => 3.0,
            (112, "honesty") => 1.0,
            (113 | 115 | 116, "assertive") => 2.0,
            (114, "honesty") => 1.0,
            (117 | 118, "empathy") => 1.0,
            (119, "empathy") => -2.0,
            (120, "assertive") => 1.0,
            (121, "assertive") => 5.0,
            (121, "courage") => 5.0,
            _ => 0.0,
        }
    } else {
        match (action, character, kind) {
            (100, 1, "empathy") => 2.0,
            (104, 1, "empathy") => -5.0,
            (104, 1, "patience") => -5.0,
            (105, 1, "empathy") => 3.0,
            (105, 1, "patience") => 2.0,
            (104, 2, "empathy") => -2.0,
            (107, 2, "empathy") => 2.0,
            (108, 2, "empathy") => 1.0,
            (109, 3, "empathy") => -4.0,
            (109, 3, "patience") => -4.0,
            (110, 3, "empathy") => 1.0,
            (111, 3, "honesty") => 2.0,
            (110, 4, "empathy") => 1.0,
            (111, 4, "honesty") => 2.0,
            (120, 4, "empathy") => 1.0,
            (101, 5, "empathy") => 2.0,
            (101, 5, "honesty") => 2.0,
            _ => 0.0,
        }
    }
}

fn apply_personality(fixture: &Fixture, state: &mut WorldStateTable, action: u16) {
    let mut deltas = Vec::with_capacity(ENTITIES * TRAITS);
    for character in 0..ENTITIES {
        for trait_index in 0..TRAITS {
            let jitter = match (action as usize * 31 + character * 13 + trait_index * 7) % 4 {
                0 => -0.2,
                1 => -0.1,
                2 => 0.1,
                _ => 0.2,
            };
            let delta =
                jitter + explicit_trait_delta(action, character, &fixture.trait_names[trait_index]);
            deltas.push((
                fixture.metric_indices[character * TRAITS + trait_index],
                delta,
            ));
        }
    }
    let op = MutationOp {
        metric_deltas: deltas,
        ..Default::default()
    };
    apply_mutation_with_registry(state, &op, &fixture.tags).unwrap();
}

fn passion_delta(fixture: &Fixture, state: &mut WorldStateTable, npc: u16, delta: f32) {
    let op = PassionMutation {
        micro_deltas: vec![(npc, delta)],
        active_ops: vec![(npc, true)],
    };
    apply_passion_mutation(
        &mut state.passions,
        &op,
        &fixture.passions,
        &mut state.global_tags,
    )
    .unwrap();
}

fn money_npc(state: &WorldStateTable, tags: &GameplayTagRegistry) -> u16 {
    if has(state, tags, KICKER_PRESENT) {
        2
    } else {
        3
    }
}

fn apply_action_passions(
    fixture: &Fixture,
    state: &mut WorldStateTable,
    action: u16,
    bound: Option<usize>,
) {
    let bound_npc = |state: &WorldStateTable| {
        bound
            .map(|entity| entity.saturating_sub(1) as u16)
            .unwrap_or_else(|| money_npc(state, &fixture.tags))
    };
    match action {
        100 => {
            passion_delta(fixture, state, 0, 0.25);
            passion_delta(fixture, state, 1, 0.05);
        }
        101 => passion_delta(fixture, state, 4, 0.2),
        104 => {
            passion_delta(fixture, state, 0, -0.45);
            passion_delta(fixture, state, 1, -0.1);
        }
        105 => {
            passion_delta(fixture, state, 0, 0.30);
            passion_delta(fixture, state, 1, 0.05);
        }
        106 => passion_delta(fixture, state, 0, 0.1),
        107 => passion_delta(fixture, state, 1, 0.3),
        108 => passion_delta(fixture, state, 1, 0.1),
        109 => {
            let npc = money_npc(state, &fixture.tags);
            passion_delta(fixture, state, npc, -0.4);
        }
        110 => {
            let npc = bound_npc(state);
            passion_delta(fixture, state, npc, 0.15);
        }
        111 => {
            let npc = bound_npc(state);
            passion_delta(fixture, state, npc, 0.5);
        }
        112 => {
            let npc = bound_npc(state);
            passion_delta(fixture, state, npc, 0.1);
        }
        120 => passion_delta(fixture, state, 3, 0.2),
        _ => {}
    }
}

fn replace_kicker(fixture: &Fixture, state: &mut WorldStateTable) {
    let mut op = MutationOp::default();
    op.entity_tag_ops.push(EntityTagMutation {
        entity_id: 3,
        add_tags: TagBitset::new(),
        remove_tags: mask(&fixture.tags, &[ROLE_MONEY, HALL]),
    });
    op.entity_tag_ops.push(EntityTagMutation {
        entity_id: 4,
        add_tags: mask(&fixture.tags, &[ROLE_MONEY, HALL]),
        remove_tags: TagBitset::new(),
    });
    op.add_global_tags = mask(&fixture.tags, &[JANITOR_PRESENT]);
    op.remove_global_tags = mask(&fixture.tags, &[KICKER_PRESENT]);
    apply_mutation_with_registry(state, &op, &fixture.tags).unwrap();
}

fn kill_oldman(fixture: &Fixture, state: &mut WorldStateTable, caregiver: bool) {
    let mut op = MutationOp::default();
    op.entity_tag_ops.push(EntityTagMutation {
        entity_id: 1,
        add_tags: TagBitset::new(),
        remove_tags: mask(&fixture.tags, &[ROLE_KEYHOLDER, HALL]),
    });
    if caregiver {
        op.entity_tag_ops.push(EntityTagMutation {
            entity_id: 5,
            add_tags: mask(&fixture.tags, &[ROLE_KEYHOLDER, HALL]),
            remove_tags: TagBitset::new(),
        });
        op.add_global_tags = mask(&fixture.tags, &[OLD_DEAD, CAREGIVER_PRESENT]);
    } else {
        op.add_global_tags = mask(&fixture.tags, &[OLD_DEAD]);
    }
    op.remove_global_tags = mask(&fixture.tags, &[OLD_ALIVE]);
    apply_mutation_with_registry(state, &op, &fixture.tags).unwrap();
}

fn replace_keyholder(fixture: &Fixture, state: &mut WorldStateTable) {
    kill_oldman(fixture, state, true);
}

#[derive(Default)]
struct Stats {
    success: u64,
    fail: u64,
    break_used: u64,
    oldman_death: u64,
    kicker_left: u64,
    turns: u64,
    routes: BTreeMap<&'static str, u64>,
    money_kicker: u64,
    money_janitor: u64,
    money_actions: u64,
    ever_hostile_oldman: u64,
    ever_hostile_money: u64,
    ever_approve_girl: u64,
    caregiver_key: u64,
    body_attempts: u64,
    body_key: u64,
    hairpin_given: u64,
    lockpick_given: u64,
    master_actions: u64,
    janitor_master: u64,
    kicker_master: u64,
    forced_death: u64,
    forced_janitor: u64,
    fail_metric_blocked: u64,
    fail_metric_actions: BTreeMap<u16, u64>,
    failure_example: Option<String>,
    failure_actions: Vec<u16>,
}

fn rng_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    z ^ (z >> 31)
}

fn chance(rng: &mut u64, p: f64) -> bool {
    (rng_next(rng) >> 11) as f64 / ((1u64 << 53) as f64) < p
}

fn choose(
    fixture: &Fixture,
    state: &WorldStateTable,
    completed: &BTreeSet<u16>,
    policy: &str,
) -> Option<u16> {
    let playable = |action| is_playable(fixture, state, completed, action);
    match policy {
        "polite" => {
            if playable(100) {
                Some(100)
            } else if playable(102) {
                Some(102)
            } else if playable(103) {
                Some(103)
            } else if playable(105) {
                Some(105)
            } else if playable(106) {
                Some(106)
            } else if playable(101) {
                Some(101)
            } else if playable(119) {
                Some(119)
            } else if playable(107) {
                Some(107)
            } else if playable(108) {
                Some(108)
            } else {
                None
            }
        }
        "caregiver" => {
            if playable(101) {
                Some(101)
            } else if playable(102) {
                Some(102)
            } else if playable(103) {
                Some(103)
            } else {
                None
            }
        }
        "body" => {
            if playable(119) {
                Some(119)
            } else if playable(102) {
                Some(102)
            } else if playable(103) {
                Some(103)
            } else {
                None
            }
        }
        "aggressive" => {
            if playable(104) {
                Some(104)
            } else if playable(109) && !has(state, &fixture.tags, KICKED) {
                Some(109)
            } else if playable(110) && !has(state, &fixture.tags, CLUE_KICK) {
                Some(110)
            } else if playable(111) {
                Some(111)
            } else if playable(112) {
                Some(112)
            } else if playable(105) {
                Some(105)
            } else if playable(106) {
                Some(106)
            } else if playable(102) {
                Some(102)
            } else if playable(103) {
                Some(103)
            } else if playable(120) {
                Some(120)
            } else if playable(108) {
                Some(108)
            } else if playable(107) {
                Some(107)
            } else if playable(117) {
                Some(117)
            } else if playable(113) {
                Some(113)
            } else if playable(114) {
                Some(114)
            } else if playable(118) {
                Some(118)
            } else if playable(116) {
                Some(116)
            } else if playable(115) {
                Some(115)
            } else if playable(101) {
                Some(101)
            } else if playable(119) {
                Some(119)
            } else {
                None
            }
        }
        "empathetic" => {
            if playable(107) && !has(state, &fixture.tags, HAIRPIN) {
                Some(107)
            } else if playable(108) {
                Some(108)
            } else if playable(117) {
                Some(117)
            } else if playable(113) {
                Some(113)
            } else if playable(114) {
                Some(114)
            } else if playable(118) {
                Some(118)
            } else if playable(115) {
                Some(115)
            } else if playable(103) {
                Some(103)
            } else if playable(102) {
                Some(102)
            } else if playable(100) {
                Some(100)
            } else if playable(101) {
                Some(101)
            } else if playable(119) {
                Some(119)
            } else {
                None
            }
        }
        _ => None,
    }
}

struct Scenario {
    policy: &'static str,
    allow_break: bool,
    forced_death: bool,
    caregiver: bool,
    forced_janitor: bool,
}

fn run_one(fixture: &Fixture, seed: u64, scenario: &Scenario, stats: &mut Stats) {
    let mut state = initial(fixture);
    let mut completed = BTreeSet::new();
    let mut rng = seed;
    let mut turns = 0u64;
    let mut route = None;
    let mut actions = Vec::new();
    let mut ever_hostile_oldman = false;
    let mut ever_hostile_money = false;
    let mut ever_approve_girl = false;

    if scenario.forced_death {
        kill_oldman(fixture, &mut state, scenario.caregiver);
        stats.forced_death += 1;
    }
    if scenario.forced_janitor {
        replace_kicker(fixture, &mut state);
        stats.forced_janitor += 1;
    }

    loop {
        turns += 1;
        let mut action = choose(fixture, &state, &completed, scenario.policy);
        if action.is_none() && scenario.allow_break {
            set_tag(&mut state, &fixture.tags, BREAK_READY, true);
            if is_playable(fixture, &state, &completed, 121) {
                action = Some(121);
                stats.break_used += 1;
            }
        }
        let Some(action) = action else {
            stats.fail += 1;
            let charisma = metric(fixture, &state, "c0.charisma");
            let empathy = metric(fixture, &state, "c0.empathy");
            let honesty = metric(fixture, &state, "c0.honesty");
            let assertive = metric(fixture, &state, "c0.assertive");
            let mut metric_blocked = BTreeMap::new();
            for candidate in 100..=121 {
                let Some(def) = fixture.dispatcher.storylet(candidate) else {
                    continue;
                };
                if fixture.dispatcher.is_playable(def, &state, &completed) {
                    continue;
                }
                let mut unlocked = def.clone();
                unlocked.metric_gates.clear();
                if fixture
                    .dispatcher
                    .is_playable(&unlocked, &state, &completed)
                {
                    *metric_blocked.entry(candidate).or_insert(0u64) += 1;
                }
            }
            if !metric_blocked.is_empty() {
                stats.fail_metric_blocked += 1;
            }
            for (candidate, count) in metric_blocked {
                *stats.fail_metric_actions.entry(candidate).or_insert(0) += count;
            }
            if stats.failure_example.is_none() {
                stats.failure_actions = actions.clone();
                stats.failure_example = Some(format!(
                    "turn {turns} policy {} charisma {charisma:.1} empathy {empathy:.1} honesty {honesty:.1} assertive {assertive:.1} coin={} approve_money={} girl_approve={} key={} hairpin={} lockpick={} caregiver={} body={}",
                    scenario.policy,
                    has(&state, &fixture.tags, COIN),
                    has(&state, &fixture.tags, "approve.kicker") || has(&state, &fixture.tags, "approve.janitor"),
                    has(&state, &fixture.tags, "approve.girl"),
                    has(&state, &fixture.tags, KEY),
                    has(&state, &fixture.tags, HAIRPIN),
                    has(&state, &fixture.tags, LOCKPICK),
                    is_playable(fixture, &state, &completed, 101),
                    is_playable(fixture, &state, &completed, 119),
                ));
            }
            break;
        };

        actions.push(action);
        let def = fixture.dispatcher.storylet(action).unwrap().clone();
        let bound = bound_entity(fixture, &state, action);
        if let Some(entity) = bound {
            if action == 110 || action == 111 || action == 112 {
                stats.money_actions += 1;
                if entity == 3 {
                    stats.money_kicker += 1;
                } else if entity == 4 {
                    stats.money_janitor += 1;
                }
            }
        }
        match action {
            101 => stats.caregiver_key += 1,
            108 => stats.hairpin_given += 1,
            112 => stats.lockpick_given += 1,
            120 => {
                stats.master_actions += 1;
                if bound == Some(4) {
                    stats.janitor_master += 1;
                }
                if bound == Some(3) {
                    stats.kicker_master += 1;
                }
            }
            _ => {}
        }

        apply_personality(fixture, &mut state, action);
        apply_action_passions(fixture, &mut state, action, bound);
        apply_mutation_with_registry(&mut state, &def.on_complete_mutation, &fixture.tags).unwrap();

        match action {
            109 => {
                if chance(&mut rng, 0.7) {
                    set_tag(&mut state, &fixture.tags, COIN, true);
                }
                if has(&state, &fixture.tags, KICKER_PRESENT) && chance(&mut rng, 0.55) {
                    replace_kicker(fixture, &mut state);
                    stats.kicker_left += 1;
                }
            }
            113 => {
                if chance(&mut rng, 0.65) {
                    set_tag(&mut state, &fixture.tags, STORAGE_OPEN, true);
                    set_tag(&mut state, &fixture.tags, STORAGE_LOCKED, false);
                }
            }
            115 => {
                if chance(&mut rng, 0.5) {
                    set_tag(&mut state, &fixture.tags, CENTER_OPEN, true);
                    set_tag(&mut state, &fixture.tags, CENTER_LOCKED, false);
                    route = Some("hairpin");
                }
            }
            116 => {
                if chance(&mut rng, 0.65) {
                    set_tag(&mut state, &fixture.tags, CENTER_OPEN, true);
                    set_tag(&mut state, &fixture.tags, CENTER_LOCKED, false);
                    route = Some("lockpick");
                }
            }
            119 => {
                stats.body_attempts += 1;
                if chance(&mut rng, 0.5) {
                    set_tag(&mut state, &fixture.tags, KEY, true);
                    stats.body_key += 1;
                }
            }
            103 => route = Some("key"),
            120 => route = Some("master"),
            121 => route = Some("break"),
            _ => {}
        }

        if !def.is_repeatable {
            completed.insert(action);
        }
        if has(&state, &fixture.tags, "hostile.oldman") {
            ever_hostile_oldman = true;
        }
        if has(&state, &fixture.tags, "hostile.kicker")
            || has(&state, &fixture.tags, "hostile.janitor")
        {
            ever_hostile_money = true;
        }
        if has(&state, &fixture.tags, "approve.girl") {
            ever_approve_girl = true;
        }

        if action != 103 && action != 115 && action != 116 && action != 120 && action != 121 {
            if has(&state, &fixture.tags, OLD_ALIVE)
                && !has(&state, &fixture.tags, KEY)
                && chance(&mut rng, 0.04)
            {
                replace_keyholder(fixture, &mut state);
                stats.oldman_death += 1;
            }
        }

        if route.is_some() {
            break;
        }
        if turns >= 40 {
            stats.fail += 1;
            break;
        }
    }

    stats.turns += turns;
    if ever_hostile_oldman {
        stats.ever_hostile_oldman += 1;
    }
    if ever_hostile_money {
        stats.ever_hostile_money += 1;
    }
    if ever_approve_girl {
        stats.ever_approve_girl += 1;
    }
    if let Some(route) = route {
        stats.success += 1;
        *stats.routes.entry(route).or_insert(0) += 1;
    }
}

fn pct(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        value as f64 * 100.0 / total as f64
    }
}

fn run_scenario(fixture: &Fixture, scenario: &Scenario) {
    let mut stats = Stats::default();
    for seed in 0..TRIALS {
        run_one(fixture, seed, scenario, &mut stats);
    }
    let total = stats.success + stats.fail;
    println!(
        "\nPOLICY {} break={} forced_death={} caregiver={} forced_janitor={} rival_link={}",
        scenario.policy,
        scenario.allow_break,
        scenario.forced_death,
        scenario.caregiver,
        scenario.forced_janitor,
        fixture.rival_link
    );
    println!(
        "  success {:.2}% | fail {:.2}% | break {:.2}% | mean turns {:.1}",
        pct(stats.success, total),
        pct(stats.fail, total),
        pct(stats.break_used, total),
        stats.turns as f64 / total as f64
    );
    println!(
        "  routes: {:?}",
        stats
            .routes
            .iter()
            .map(|(route, count)| (*route, pct(*count, total)))
            .collect::<BTreeMap<_, _>>()
    );
    println!(
        "  ever hostile oldman {:.2}% | ever hostile money {:.2}% | ever approve girl {:.2}%",
        pct(stats.ever_hostile_oldman, total),
        pct(stats.ever_hostile_money, total),
        pct(stats.ever_approve_girl, total)
    );
    println!(
        "  oldman death {:.2}% | kicker replaced {:.2}% | money binds kicker {:.2}% / janitor {:.2}%",
        pct(stats.oldman_death, total), pct(stats.kicker_left, total),
        pct(stats.money_kicker, stats.money_actions.max(1)),
        pct(stats.money_janitor, stats.money_actions.max(1))
    );
    println!(
        "  caregiver key {:.2}% | body attempts {:.2}% | body key {:.2}% | hairpin {:.2}% | lockpick {:.2}% | master {:.2}% (janitor {:.2}%, kicker {:.2}%)",
        pct(stats.caregiver_key, total), pct(stats.body_attempts, total),
        pct(stats.body_key, total), pct(stats.hairpin_given, total),
        pct(stats.lockpick_given, total), pct(stats.master_actions, total),
        pct(stats.janitor_master, total), pct(stats.kicker_master, total)
    );
    if stats.fail > 0 {
        println!(
            "  failures where clearing only the metric gates would open an action: {:.2}%",
            pct(stats.fail_metric_blocked, stats.fail)
        );
        if !stats.fail_metric_actions.is_empty() {
            println!(
                "  metric-blocked candidates: {:?}",
                stats
                    .fail_metric_actions
                    .iter()
                    .map(|(id, count)| (*id, pct(*count, stats.fail)))
                    .collect::<BTreeMap<_, _>>()
            );
        }
        if let Some(example) = &stats.failure_example {
            println!("  failure sample: {example}");
            println!("  failure route: {:?}", stats.failure_actions);
        }
    }
    println!(
        "  personality metrics changed per action: {}",
        ENTITIES * TRAITS
    );
}

fn main() {
    let linked = build_fixture(true);
    let unlinked = build_fixture(false);

    run_scenario(
        &linked,
        &Scenario {
            policy: "polite",
            allow_break: false,
            forced_death: false,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "empathetic",
            allow_break: false,
            forced_death: false,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "aggressive",
            allow_break: false,
            forced_death: false,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "aggressive",
            allow_break: true,
            forced_death: false,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &unlinked,
        &Scenario {
            policy: "aggressive",
            allow_break: false,
            forced_death: false,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "caregiver",
            allow_break: false,
            forced_death: true,
            caregiver: true,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "body",
            allow_break: false,
            forced_death: true,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "body",
            allow_break: true,
            forced_death: true,
            caregiver: false,
            forced_janitor: false,
        },
    );
    run_scenario(
        &linked,
        &Scenario {
            policy: "aggressive",
            allow_break: false,
            forced_death: false,
            caregiver: false,
            forced_janitor: true,
        },
    );
}
