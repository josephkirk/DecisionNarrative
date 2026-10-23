//! Stress test: 60 actors, an open gas-station hub, linear prologue, QBN,
//! salience beats, waypoint dialogue, butterfly apocalypse tracks, and endings.
//! Every storylet has 2-4 dialogue choices. Inventory items can be picked up,
//! appear/expire by turn, or be given by NPCs. NPCs can die and later return.
//!
//! Run:
//!   cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example gas_station_stress

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use storylet_engine::chrono_dispatch::{
    MetricGate, RoleRequirement, StoryletClusterType, StoryletDef, UnifiedDispatcher,
    WaypointTopicEdge, WaypointTopicGraph, WaypointTopicNode,
};
use storylet_engine::chrono_runtime::{
    ActionPart, EventOverlapActivity, SceneComposer, TimelineTrackEvent,
};
use storylet_engine::chrono_state::{
    apply_mutation_with_registry, revert_delta, EntityTagMutation, GameplayTagRegistry,
    MetricRegistry, MutationOp, TagBitset, WorldStateTable,
};

const ACTORS: usize = 60;
const TRIALS: u64 = 500;
const HUB_TURNS: u64 = 32;

const LOC_STATION: &str = "loc.station";
const ACT_PROLOGUE: &str = "act.prologue";
const ACT_HUB: &str = "act.hub";
const ARC1: &str = "arc.act1";
const ARC2: &str = "arc.act2";
const ARC3: &str = "arc.act3";

const ROLE_NAMES: [&str; 16] = [
    "role.attendant",
    "role.mechanic",
    "role.sheriff",
    "role.cook",
    "role.trucker",
    "role.cultist",
    "role.soldier",
    "role.medic",
    "role.drifter",
    "role.child",
    "role.scientist",
    "role.radio",
    "role.president",
    "role.general",
    "role.politician",
    "role.aide",
];

const PLAYER_METRICS: [&str; 8] = [
    "p.guilt",
    "p.denial",
    "p.empathy",
    "p.paranoia",
    "p.courage",
    "p.growth",
    "p.truth",
    "p.deaths",
];

const TAG_NAMES: [&str; 75] = [
    LOC_STATION,
    "route.left",
    "route.right",
    "obstruction.deer",
    "obstruction.ghost",
    "obstruction.zombie",
    "obstruction.human",
    "ghost.seen",
    "human.hit",
    "phone.fixed",
    "ready.leave",
    "actor.dead",
    "player.dead",
    "wife.zombie",
    "divorce.accepted",
    "president.orders",
    "politician.deal",
    "aide.leak",
    "loc.pumps",
    "loc.diner",
    "loc.backroom",
    "loc.road",
    "role.attendant",
    "role.mechanic",
    "role.sheriff",
    "role.cook",
    "role.trucker",
    "role.cultist",
    "role.soldier",
    "role.medic",
    "role.drifter",
    "role.child",
    "role.scientist",
    "role.radio",
    "role.president",
    "role.general",
    "role.politician",
    "role.aide",
    ACT_PROLOGUE,
    ACT_HUB,
    ARC1,
    ARC2,
    ARC3,
    "phone.divorce",
    "crash",
    "phone.broken",
    "trunk.locked",
    "trunk.opened",
    "trunk.body",
    "truth.remembered",
    "truth.denied",
    "station.power",
    "station.online",
    "radio.fixed",
    "safe.opened",
    "fuel",
    "sheriff.alert",
    "cult.recruited",
    "zombie.vector",
    "zombie.outbreak",
    "alien.signal",
    "alien.contact",
    "nuclear.codes",
    "nuclear.launch",
    "silent.trigger",
    "wife.divorce",
    "child.trust",
    "backroom.open",
    "ride.offered",
    "cult.rumor",
    "apocalypse.zombie",
    "apocalypse.alien",
    "apocalypse.nuclear",
    "apocalypse.silent",
    "ending.flags",
];

fn tags() -> Vec<String> {
    let mut names: Vec<String> = TAG_NAMES.iter().map(|name| (*name).to_owned()).collect();
    for ending in [
        "ending.home",
        "ending.divorce",
        "ending.zombie",
        "ending.alien",
        "ending.nuclear",
        "ending.silent",
        "ending.death",
        "ending.home_zombie_wife",
        "ending.home_world_zombie",
        "ending.divorce_nuke",
    ] {
        names.push(ending.to_owned());
    }
    for id in 1..=100 {
        names.push(format!("done.{id}"));
    }
    names
}

fn mask(registry: &GameplayTagRegistry, names: &[&str]) -> TagBitset {
    let mut bits = TagBitset::new();
    for name in names {
        bits.set_bit(
            registry
                .id(name)
                .unwrap_or_else(|| panic!("missing tag {name}")),
        )
        .unwrap();
    }
    bits
}

fn gate(metrics: &MetricRegistry, name: &str, min: f32, max: f32) -> MetricGate {
    MetricGate {
        metric_id: metrics.id(name).unwrap(),
        min,
        max,
    }
}

fn role(role_tag: &str) -> RoleRequirement {
    RoleRequirement {
        role_tag: role_tag.to_owned(),
        location_tag: Some(LOC_STATION.to_owned()),
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn storylet(
    tag_registry: &GameplayTagRegistry,
    metric_registry: &MetricRegistry,
    id: u16,
    required: &[&str],
    forbidden: &[&str],
    gates: Vec<MetricGate>,
    roles: Vec<RoleRequirement>,
    add: &[&str],
    remove: &[&str],
    metrics: &[(&str, f32)],
    arc: Option<&str>,
    priority: i32,
    cluster: StoryletClusterType,
    repeatable: bool,
) -> StoryletDef {
    let metric_deltas = metrics
        .iter()
        .map(|(name, delta)| (metric_registry.id(name).unwrap(), *delta))
        .collect();
    let mut added = mask(tag_registry, add);
    added
        .set_bit(tag_registry.id(&format!("done.{id}")).unwrap())
        .unwrap();
    StoryletDef {
        storylet_id: id,
        display_priority: priority,
        cluster_type: cluster,
        required_tags: mask(tag_registry, required),
        forbidden_tags: mask(tag_registry, forbidden),
        required_arc_phases: arc
            .map(|name| mask(tag_registry, &[name]))
            .unwrap_or_default(),
        metric_gates: gates,
        required_roles: roles,
        on_complete_mutation: MutationOp {
            add_global_tags: added,
            remove_global_tags: mask(tag_registry, remove),
            metric_deltas,
            ..Default::default()
        },
        is_repeatable: repeatable,
        ..Default::default()
    }
}

fn actor_roles(index: usize) -> Vec<&'static str> {
    match index {
        0 => vec![ROLE_NAMES[0]],
        1 => vec![ROLE_NAMES[1]],
        2 => vec![ROLE_NAMES[2]],
        3 => vec![ROLE_NAMES[3]],
        4 => vec![ROLE_NAMES[4]],
        5 => vec![ROLE_NAMES[12]],
        6 => vec![ROLE_NAMES[13]],
        7 => vec![ROLE_NAMES[14]],
        8 => vec![ROLE_NAMES[15]],
        _ => {
            let primary = ROLE_NAMES[index % ROLE_NAMES.len()];
            if index % 3 == 0 {
                vec![primary, ROLE_NAMES[(index * 5 + 3) % ROLE_NAMES.len()]]
            } else {
                vec![primary]
            }
        }
    }
}

fn actor_present(index: usize, turn: u64) -> bool {
    if index < 9 {
        return true;
    }
    let start = 1 + (index as u64 * 7 % 18);
    let on_length = 4 + (index as u64 * 3 % 5);
    let off_length = 3 + (index as u64 * 5 % 6);
    let period = on_length + off_length;
    turn >= start && (turn - start) % period < on_length
}

#[derive(Clone)]
struct ChoiceSpec {
    label: &'static str,
    metrics: Vec<(&'static str, f32)>,
    grants: Vec<&'static str>,
    consumes: Vec<&'static str>,
    requires: Option<&'static str>,
    requires_ground: Option<&'static str>,
    weight: f64,
}

struct Fixture {
    tags: GameplayTagRegistry,
    metrics: MetricRegistry,
    dispatcher: UnifiedDispatcher,
    deck: Vec<u16>,
    salience_pool: Vec<u16>,
    names: BTreeMap<u16, &'static str>,
    actor_roles: Vec<Vec<&'static str>>,
    actor_skill: Vec<f32>,
    choice_specs: BTreeMap<u16, Vec<ChoiceSpec>>,
}

fn build_fixture() -> Fixture {
    let tag_registry = GameplayTagRegistry::compile(tags()).unwrap();
    let mut metric_names: Vec<String> = PLAYER_METRICS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for actor in 0..ACTORS {
        for stat in ["trust", "fear", "skill", "stability"] {
            metric_names.push(format!("a{actor}.{stat}"));
        }
    }
    let metric_registry = MetricRegistry::compile(metric_names).unwrap();

    let mut defs = Vec::new();
    let mut names = BTreeMap::new();
    for (id, name) in [
        (1, "prologue.call"),
        (2, "prologue.drive.left"),
        (3, "prologue.drive.right"),
        (4, "prologue.crash.deer"),
        (5, "prologue.crash.ghost"),
        (6, "prologue.crash.zombie"),
        (7, "prologue.crash.human"),
        (8, "prologue.walk"),
    ] {
        names.insert(id, name);
    }

    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        1,
        &[ACT_PROLOGUE],
        &[],
        vec![],
        vec![],
        &["phone.divorce"],
        &[],
        &[("p.denial", 5.0)],
        None,
        10,
        StoryletClusterType::LinearProgression,
        false,
    ));
    for (id, direction) in [(2, "route.left"), (3, "route.right")] {
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            id,
            &[ACT_PROLOGUE, "phone.divorce"],
            &[],
            vec![],
            vec![],
            &[direction],
            &[],
            &[("p.courage", 1.0)],
            None,
            10,
            StoryletClusterType::LinearProgression,
            false,
        ));
    }
    for (id, obstruction, add, metrics) in [
        (
            4,
            "obstruction.deer",
            &["crash", "phone.broken"][..],
            &[("p.guilt", 2.0), ("p.paranoia", 1.0)][..],
        ),
        (
            5,
            "obstruction.ghost",
            &["crash", "phone.broken", "ghost.seen"][..],
            &[("p.paranoia", 10.0), ("p.denial", 2.0)][..],
        ),
        (
            6,
            "obstruction.zombie",
            &["crash", "phone.broken", "zombie.vector"][..],
            &[("p.paranoia", 6.0), ("p.guilt", 3.0)][..],
        ),
        (
            7,
            "obstruction.human",
            &["crash", "phone.broken", "human.hit", "sheriff.alert"][..],
            &[("p.guilt", 8.0), ("p.paranoia", 3.0)][..],
        ),
    ] {
        let mut add_tags = vec![obstruction];
        add_tags.extend_from_slice(add);
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            id,
            &[ACT_PROLOGUE, "phone.divorce"],
            &[],
            vec![],
            vec![],
            &add_tags,
            &[],
            metrics,
            None,
            10,
            StoryletClusterType::LinearProgression,
            false,
        ));
    }
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        8,
        &[ACT_PROLOGUE, "crash"],
        &[],
        vec![],
        vec![],
        &[ACT_HUB],
        &[ACT_PROLOGUE],
        &[("p.courage", 2.0)],
        None,
        10,
        StoryletClusterType::LinearProgression,
        false,
    ));

    // (id, name, required, roles, add, metrics, priority, cluster, repeatable)
    let hub: &[(
        u16,
        &str,
        &[&str],
        &[&str],
        &[&str],
        &[(&str, f32)],
        i32,
        bool,
    )] = &[
        (
            10,
            "attendant.talk",
            &[],
            &[ROLE_NAMES[0]],
            &[],
            &[("p.empathy", 2.0), ("p.growth", 3.0)],
            20,
            false,
        ),
        (
            11,
            "mechanic.fix",
            &[],
            &[ROLE_NAMES[1]],
            &["station.power"],
            &[("p.growth", 2.0)],
            20,
            false,
        ),
        (
            12,
            "sheriff.talk",
            &[],
            &[ROLE_NAMES[2]],
            &["sheriff.alert"],
            &[("p.denial", 5.0)],
            15,
            false,
        ),
        (
            13,
            "cook.rumor",
            &[],
            &[ROLE_NAMES[3]],
            &["cult.rumor"],
            &[("p.paranoia", 3.0)],
            12,
            false,
        ),
        (
            14,
            "trucker.lift",
            &[],
            &[ROLE_NAMES[4]],
            &["ride.offered"],
            &[("p.growth", 2.0)],
            14,
            false,
        ),
        (
            15,
            "cultist.recruit",
            &[],
            &[ROLE_NAMES[5]],
            &["cult.recruited"],
            &[("p.paranoia", 8.0), ("p.denial", 2.0), ("p.growth", -3.0)],
            16,
            false,
        ),
        (
            16,
            "soldier.convoy",
            &[],
            &[ROLE_NAMES[6]],
            &["nuclear.codes"],
            &[("p.denial", 5.0)],
            18,
            false,
        ),
        (
            17,
            "medic.help",
            &[],
            &[ROLE_NAMES[7]],
            &["zombie.vector"],
            &[("p.empathy", 4.0), ("p.growth", 5.0), ("p.guilt", 2.0)],
            18,
            false,
        ),
        (
            18,
            "drifter.steal",
            &[],
            &[ROLE_NAMES[8]],
            &["fuel"],
            &[("p.denial", 4.0), ("p.guilt", 3.0)],
            12,
            false,
        ),
        (
            19,
            "child.talk",
            &[],
            &[ROLE_NAMES[9]],
            &["child.trust"],
            &[("p.empathy", 6.0), ("p.growth", 8.0), ("p.guilt", 2.0)],
            22,
            false,
        ),
        (
            20,
            "scientist.scan",
            &[],
            &[ROLE_NAMES[10]],
            &["alien.signal"],
            &[("p.paranoia", 6.0)],
            16,
            false,
        ),
        (
            21,
            "radio.fix",
            &[],
            &[ROLE_NAMES[11]],
            &["radio.fixed"],
            &[("p.growth", 2.0)],
            20,
            false,
        ),
        (
            22,
            "backroom.enter",
            &[],
            &[],
            &["backroom.open"],
            &[("p.paranoia", 3.0)],
            15,
            false,
        ),
        (
            23,
            "trunk.open",
            &["backroom.open"],
            &[],
            &["trunk.opened"],
            &[
                ("p.truth", 1.0),
                ("p.guilt", 10.0),
                ("p.paranoia", 8.0),
                ("p.growth", 4.0),
            ],
            25,
            false,
        ),
        (
            24,
            "trunk.look",
            &["trunk.opened"],
            &[],
            &["trunk.body"],
            &[
                ("p.truth", 1.0),
                ("p.guilt", 15.0),
                ("p.paranoia", 10.0),
                ("p.growth", 5.0),
            ],
            26,
            false,
        ),
        (
            25,
            "confess",
            &["trunk.body"],
            &[],
            &["truth.remembered"],
            &[("p.guilt", 10.0), ("p.growth", 10.0)],
            28,
            false,
        ),
        (
            26,
            "deny",
            &["trunk.body"],
            &[],
            &["truth.denied"],
            &[("p.denial", 10.0), ("p.growth", -5.0)],
            28,
            false,
        ),
        (
            27,
            "phone.call",
            &[],
            &[],
            &["phone.fixed"],
            &[("p.growth", 1.0)],
            10,
            false,
        ),
        (
            28,
            "voicemail",
            &["phone.fixed"],
            &[],
            &["wife.divorce"],
            &[("p.guilt", 2.0)],
            12,
            false,
        ),
        (
            29,
            "radio.listen",
            &["radio.fixed"],
            &[],
            &["alien.signal"],
            &[("p.paranoia", 5.0)],
            18,
            false,
        ),
        (
            30,
            "safe.open",
            &[],
            &[],
            &["safe.opened"],
            &[("p.denial", 5.0)],
            16,
            false,
        ),
        (
            31,
            "fuel.pumps",
            &[],
            &[],
            &["fuel"],
            &[("p.growth", 1.0)],
            10,
            false,
        ),
        (
            32,
            "power.generator",
            &["station.power"],
            &[],
            &["station.online"],
            &[("p.growth", 2.0)],
            14,
            false,
        ),
        (
            33,
            "child.hide",
            &[],
            &[ROLE_NAMES[9]],
            &["child.trust"],
            &[("p.empathy", 4.0), ("p.growth", 5.0)],
            20,
            false,
        ),
        (
            34,
            "medic.triage",
            &[],
            &[ROLE_NAMES[7]],
            &["zombie.vector"],
            &[("p.growth", 3.0)],
            16,
            false,
        ),
        (
            35,
            "trucker.ride",
            &[],
            &[ROLE_NAMES[4]],
            &["ride.offered"],
            &[("p.growth", 2.0)],
            14,
            false,
        ),
        (
            36,
            "cultist.sermon",
            &[],
            &[ROLE_NAMES[5]],
            &["cult.recruited"],
            &[("p.paranoia", 5.0)],
            15,
            false,
        ),
        (
            37,
            "sheriff.bribe",
            &[],
            &[ROLE_NAMES[2]],
            &["sheriff.alert"],
            &[("p.denial", 5.0)],
            13,
            false,
        ),
        (
            38,
            "soldier.orders",
            &[],
            &[ROLE_NAMES[6]],
            &["nuclear.codes"],
            &[("p.denial", 3.0)],
            17,
            false,
        ),
        (
            39,
            "scientist.analyze",
            &[],
            &[ROLE_NAMES[10]],
            &["alien.signal"],
            &[("p.paranoia", 4.0)],
            15,
            false,
        ),
        (
            40,
            "mechanic.fuel",
            &[],
            &[ROLE_NAMES[1]],
            &["fuel"],
            &[("p.growth", 1.0)],
            12,
            false,
        ),
        (
            41,
            "attendant.key",
            &[],
            &[ROLE_NAMES[0]],
            &["backroom.open"],
            &[("p.growth", 1.0)],
            18,
            false,
        ),
        (42, "wait", &[], &[], &[], &[], 0, true),
        (
            43,
            "drink",
            &[],
            &[],
            &[],
            &[("p.guilt", 5.0), ("p.paranoia", 3.0), ("p.growth", -1.0)],
            1,
            true,
        ),
        (
            44,
            "confront.wife",
            &["truth.remembered"],
            &[],
            &[],
            &[("p.guilt", 5.0), ("p.growth", 5.0)],
            24,
            false,
        ),
        (
            45,
            "leave.station",
            &[],
            &[],
            &["ready.leave"],
            &[("p.growth", 1.0)],
            8,
            false,
        ),
    ];

    for (id, name, required, roles, add, metrics, priority, repeatable) in hub {
        names.insert(*id, *name);
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            *id,
            &[ACT_HUB, LOC_STATION]
                .into_iter()
                .chain(required.iter().copied())
                .collect::<Vec<_>>(),
            &[],
            vec![],
            roles.iter().map(|role_name| role(role_name)).collect(),
            add,
            &[],
            metrics,
            None,
            *priority,
            StoryletClusterType::PlayerPassage,
            *repeatable,
        ));
    }

    names.insert(46, "hub.search");
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        46,
        &[ACT_HUB, LOC_STATION],
        &[],
        vec![],
        vec![],
        &[],
        &[],
        &[],
        None,
        5,
        StoryletClusterType::PlayerPassage,
        true,
    ));

    for (id, name) in [
        (47, "president.order"),
        (48, "general.launch"),
        (49, "politician.deal"),
        (50, "aide.leak"),
        (55, "wife.zombie"),
        (56, "accept.divorce"),
        (51, "kill.nuclear"),
        (52, "kill.zombie"),
        (53, "kill.coup"),
        (54, "kill.general"),
        (78, "ending.death"),
        (79, "ending.home_zombie_wife"),
        (80, "ending.home_world_zombie"),
        (81, "ending.divorce_nuke"),
    ] {
        names.insert(id, name);
    }

    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        47,
        &[ACT_HUB, LOC_STATION],
        &[],
        vec![],
        vec![role(ROLE_NAMES[12])],
        &["president.orders", "nuclear.codes"],
        &[],
        &[("p.denial", 5.0)],
        None,
        75,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        48,
        &[ACT_HUB, LOC_STATION, "done.47", "nuclear.codes"],
        &[],
        vec![],
        vec![role(ROLE_NAMES[13])],
        &["nuclear.launch", "apocalypse.nuclear"],
        &[],
        &[("p.paranoia", 5.0)],
        None,
        80,
        StoryletClusterType::AftermathAccelerator,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        49,
        &[ACT_HUB, LOC_STATION, "done.12"],
        &[],
        vec![],
        vec![role(ROLE_NAMES[14])],
        &["politician.deal", "divorce.accepted"],
        &[],
        &[("p.denial", 4.0)],
        None,
        65,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        50,
        &[ACT_HUB, LOC_STATION, "done.21"],
        &[],
        vec![],
        vec![role(ROLE_NAMES[15])],
        &["aide.leak", "alien.signal"],
        &[],
        &[("p.paranoia", 4.0)],
        None,
        65,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        55,
        &[ACT_HUB, LOC_STATION, "done.24", "done.17"],
        &[],
        vec![],
        vec![],
        &["wife.zombie"],
        &[],
        &[("p.paranoia", 5.0)],
        None,
        70,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        56,
        &[ACT_HUB, LOC_STATION, "done.28"],
        &[],
        vec![],
        vec![],
        &["divorce.accepted"],
        &[],
        &[("p.denial", 3.0)],
        None,
        45,
        StoryletClusterType::LinearProgression,
        false,
    ));
    for (id, required, name, tag) in [
        (
            51,
            &["done.48", "done.30"][..],
            "kill.nuclear",
            "kill.nuclear",
        ),
        (
            52,
            &["done.60", "done.17"][..],
            "kill.zombie",
            "kill.zombie",
        ),
        (53, &["done.49", "done.12"][..], "kill.coup", "kill.coup"),
        (
            54,
            &["done.48", "done.16"][..],
            "kill.general",
            "kill.general",
        ),
    ] {
        let _ = name;
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            id,
            &[ACT_HUB, LOC_STATION, ARC3]
                .into_iter()
                .chain(required.iter().copied())
                .collect::<Vec<_>>(),
            &[],
            vec![],
            vec![],
            &["player.dead", "ending.death"],
            &[],
            &[],
            Some(ARC3),
            95,
            StoryletClusterType::AftermathAccelerator,
            false,
        ));
        let _ = tag;
    }
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        78,
        &[ACT_HUB, ARC3, "player.dead"],
        &[],
        vec![],
        vec![],
        &["ending.death"],
        &[],
        &[],
        Some(ARC3),
        100,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        79,
        &[ACT_HUB, ARC3, "truth.remembered", "wife.zombie"],
        &[],
        vec![],
        vec![],
        &["ending.home_zombie_wife"],
        &[],
        &[],
        Some(ARC3),
        100,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        80,
        &[ACT_HUB, ARC3, "truth.remembered", "apocalypse.zombie"],
        &["wife.zombie"],
        vec![],
        vec![],
        &["ending.home_world_zombie"],
        &[],
        &[],
        Some(ARC3),
        100,
        StoryletClusterType::LinearProgression,
        false,
    ));
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        81,
        &[ACT_HUB, ARC3, "divorce.accepted", "nuclear.launch"],
        &[],
        vec![],
        vec![],
        &["ending.divorce_nuke"],
        &[],
        &[],
        Some(ARC3),
        100,
        StoryletClusterType::LinearProgression,
        false,
    ));

    let butterfly: &[(u16, &str, &[&str], &[&str], i32)] = &[
        (
            60,
            "butterfly.zombie",
            &["zombie.vector", "cult.recruited"],
            &["zombie.outbreak", "apocalypse.zombie"],
            80,
        ),
        (
            61,
            "butterfly.alien",
            &["alien.signal", "radio.fixed"],
            &["alien.contact", "apocalypse.alien"],
            80,
        ),
        (
            62,
            "butterfly.nuclear",
            &["nuclear.codes", "safe.opened"],
            &["nuclear.launch", "apocalypse.nuclear"],
            80,
        ),
        (
            63,
            "butterfly.silent",
            &["trunk.body"],
            &["silent.trigger", "apocalypse.silent"],
            80,
        ),
    ];
    for (id, name, required, add, priority) in butterfly {
        names.insert(*id, *name);
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            *id,
            &[ACT_HUB, LOC_STATION]
                .into_iter()
                .chain(required.iter().copied())
                .collect::<Vec<_>>(),
            &[],
            vec![],
            vec![],
            add,
            &[],
            &[],
            None,
            *priority,
            StoryletClusterType::AftermathAccelerator,
            false,
        ));
    }

    names.insert(64, "butterfly.ghost");
    defs.push(storylet(
        &tag_registry,
        &metric_registry,
        64,
        &[ACT_HUB, LOC_STATION, "ghost.seen"],
        &[],
        vec![gate(&metric_registry, "p.paranoia", 50.0, f32::MAX)],
        vec![],
        &["apocalypse.silent"],
        &[],
        &[],
        None,
        70,
        StoryletClusterType::AftermathAccelerator,
        false,
    ));

    let endings: &[(u16, &str, &[&str], &[&str], &[(&str, f32, f32)])] = &[
        (
            70,
            "ending.home",
            &["truth.remembered"],
            &[
                "human.hit",
                "apocalypse.zombie",
                "apocalypse.alien",
                "apocalypse.nuclear",
                "apocalypse.silent",
            ],
            &[("p.growth", 30.0, f32::MAX)],
        ),
        (
            71,
            "ending.divorce",
            &[],
            &[
                "truth.remembered",
                "apocalypse.zombie",
                "apocalypse.alien",
                "apocalypse.nuclear",
                "apocalypse.silent",
            ],
            &[],
        ),
        (
            76,
            "ending.divorce",
            &["truth.remembered"],
            &[
                "apocalypse.zombie",
                "apocalypse.alien",
                "apocalypse.nuclear",
                "apocalypse.silent",
            ],
            &[("p.growth", 0.0, 29.0)],
        ),
        (
            77,
            "ending.divorce",
            &["human.hit"],
            &[
                "apocalypse.zombie",
                "apocalypse.alien",
                "apocalypse.nuclear",
                "apocalypse.silent",
            ],
            &[],
        ),
        (72, "ending.zombie", &["apocalypse.zombie"], &[], &[]),
        (73, "ending.alien", &["apocalypse.alien"], &[], &[]),
        (74, "ending.nuclear", &["apocalypse.nuclear"], &[], &[]),
        (75, "ending.silent", &["apocalypse.silent"], &[], &[]),
    ];
    for (id, name, required, forbidden, gates) in endings {
        names.insert(*id, *name);
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            *id,
            &[ACT_HUB, ARC3]
                .into_iter()
                .chain(required.iter().copied())
                .collect::<Vec<_>>(),
            forbidden,
            gates
                .iter()
                .map(|(metric_name, min, max)| gate(&metric_registry, metric_name, *min, *max))
                .collect(),
            vec![],
            &[],
            &[],
            &[],
            None,
            100,
            StoryletClusterType::LinearProgression,
            false,
        ));
    }

    let salience: &[(u16, &str, &str, i32)] = &[
        (90, "beat.act1.denial", ARC1, 1),
        (91, "beat.act1.fear", ARC1, 2),
        (92, "beat.act2.crack", ARC2, 3),
        (93, "beat.act3.truth", ARC3, 4),
        (94, "beat.act3.choice", ARC3, 5),
    ];
    for (id, name, arc, priority) in salience {
        names.insert(*id, *name);
        defs.push(storylet(
            &tag_registry,
            &metric_registry,
            *id,
            &[ACT_HUB],
            &[],
            vec![],
            vec![],
            &[],
            &[],
            &[("p.paranoia", 1.0), ("p.guilt", 1.0)],
            Some(arc),
            *priority,
            StoryletClusterType::LinearProgression,
            false,
        ));
    }

    let deck: Vec<u16> = (10..=56).chain(60..=64).chain(70..=81).collect();
    let salience_pool = vec![90, 91, 92, 93, 94];
    let mut dispatcher = UnifiedDispatcher::new(defs);
    dispatcher.resolve(&tag_registry, &metric_registry).unwrap();

    let actor_roles: Vec<Vec<&'static str>> = (0..ACTORS).map(actor_roles).collect();
    let actor_skill: Vec<f32> = (0..ACTORS)
        .map(|index| 35.0 + (index * 17 % 61) as f32)
        .collect();
    let mut choice_specs = BTreeMap::new();
    for def in &dispatcher.storylets {
        let choices = choices_for(def.storylet_id);
        assert!(
            (2..=4).contains(&choices.len()),
            "storylet {} has {} choices",
            def.storylet_id,
            choices.len()
        );
        choice_specs.insert(def.storylet_id, choices);
    }

    Fixture {
        tags: tag_registry,
        metrics: metric_registry,
        dispatcher,
        deck,
        salience_pool,
        names,
        actor_roles,
        actor_skill,
        choice_specs,
    }
}

fn choices_for(id: u16) -> Vec<ChoiceSpec> {
    if id == 46 {
        return vec![
            ChoiceSpec {
                label: "Pick up the flashlight",
                metrics: vec![("p.growth", 1.0)],
                grants: vec!["flashlight"],
                consumes: vec![],
                requires: None,
                requires_ground: Some("flashlight"),
                weight: 1.5,
            },
            ChoiceSpec {
                label: "Pick up the radio parts",
                metrics: vec![("p.growth", 1.0)],
                grants: vec!["radio_parts"],
                consumes: vec![],
                requires: None,
                requires_ground: Some("radio_parts"),
                weight: 1.5,
            },
            ChoiceSpec {
                label: "Pick up the spare key",
                metrics: vec![("p.growth", 1.0)],
                grants: vec!["key"],
                consumes: vec![],
                requires: None,
                requires_ground: Some("key"),
                weight: 1.5,
            },
            ChoiceSpec {
                label: "Leave it",
                metrics: vec![("p.denial", 1.0)],
                grants: vec![],
                consumes: vec![],
                requires: None,
                requires_ground: None,
                weight: 1.0,
            },
        ];
    }
    let mut choices = vec![
        ChoiceSpec {
            label: "Push through",
            metrics: vec![("p.courage", 2.0), ("p.paranoia", 1.0)],
            grants: vec![],
            consumes: vec![],
            requires: None,
            requires_ground: None,
            weight: 1.0,
        },
        ChoiceSpec {
            label: "Hold back",
            metrics: vec![("p.guilt", 1.0), ("p.denial", 1.0)],
            grants: vec![],
            consumes: vec![],
            requires: None,
            requires_ground: None,
            weight: 1.0,
        },
    ];
    let item_choice = match id {
        10 => Some(("Accept the key", "key", None)),
        11 => Some(("Accept the wrench", "wrench", None)),
        16 => Some(("Accept the launch code", "military_code", None)),
        17 => Some(("Accept the medkit", "medkit", None)),
        18 => Some(("Siphon the fuel", "fuel", None)),
        19 => Some(("Accept the locket", "locket", None)),
        21 => Some(("Accept the radio parts", "radio_parts", None)),
        23 => Some(("Take the flashlight", "flashlight", Some("key"))),
        24 => Some(("Use the flashlight", "evidence", Some("flashlight"))),
        25 => Some(("Show the locket", "confession", Some("locket"))),
        30 => Some(("Take the spare key", "key", None)),
        41 => Some(("Take the backroom key", "key", None)),
        _ => None,
    };
    if let Some((label, item, requires)) = item_choice {
        choices.push(ChoiceSpec {
            label,
            metrics: vec![("p.growth", 1.0)],
            grants: if item == "evidence" || item == "confession" {
                vec![]
            } else {
                vec![item]
            },
            consumes: vec![],
            requires,
            requires_ground: None,
            weight: 1.5,
        });
    }
    if id % 5 == 0 && choices.len() < 4 {
        choices.push(ChoiceSpec {
            label: "Ask for help",
            metrics: vec![("p.empathy", 3.0), ("p.growth", 2.0)],
            grants: vec![],
            consumes: vec![],
            requires: None,
            requires_ground: None,
            weight: 1.25,
        });
    }
    if id % 7 == 0 && choices.len() < 4 {
        choices.push(ChoiceSpec {
            label: "Use what you have",
            metrics: vec![("p.courage", 1.0)],
            grants: vec![],
            consumes: vec!["lighter"],
            requires: Some("lighter"),
            requires_ground: None,
            weight: 1.25,
        });
    }
    choices
}

fn storylet_weight(id: u16, inventory: &BTreeMap<&'static str, u32>) -> f64 {
    let affinity: &[(&str, f64)] = match id {
        11 => &[("wrench", 3.0), ("fuel", 1.0)],
        16 => &[("military_code", 2.0)],
        17 => &[("medkit", 2.0)],
        18 | 31 | 40 => &[("fuel", 1.0)],
        21 => &[("radio_parts", 3.0)],
        23 => &[("key", 2.0)],
        24 => &[("flashlight", 2.0)],
        25 => &[("locket", 2.0)],
        30 => &[("key", 1.0)],
        _ => &[],
    };
    affinity
        .iter()
        .map(|(item, weight)| inventory.get(item).copied().unwrap_or(0) as f64 * weight)
        .sum::<f64>()
        + 1.0
}

fn choose_choice(
    fixture: &Fixture,
    action: u16,
    inventory: &BTreeMap<&'static str, u32>,
    ground_items: &BTreeSet<&'static str>,
    taken_items: &BTreeSet<&'static str>,
    policy: &str,
    rng: &mut u64,
) -> Option<usize> {
    let choices = fixture.choice_specs.get(&action)?;
    let available: Vec<(usize, &ChoiceSpec)> = choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| {
            let inventory_ok = choice
                .requires
                .map(|item| inventory.get(item).copied().unwrap_or(0) > 0)
                .unwrap_or(true);
            let ground_ok = choice
                .requires_ground
                .map(|item| ground_items.contains(item) && !taken_items.contains(item))
                .unwrap_or(true);
            inventory_ok && ground_ok
        })
        .collect();
    if available.is_empty() {
        return None;
    }
    if !policy.starts_with("weighted") && matches!(action, 10 | 11 | 16 | 17 | 19 | 21) {
        if let Some((index, _)) = available
            .iter()
            .rev()
            .find(|(_, choice)| !choice.grants.is_empty())
        {
            return Some(*index);
        }
    }
    if policy.starts_with("weighted") {
        let total: f64 = available.iter().map(|(_, choice)| choice.weight).sum();
        let mut cursor = (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64) * total;
        for (index, choice) in &available {
            cursor -= choice.weight;
            if cursor <= 0.0 {
                return Some(*index);
            }
        }
        return Some(available.last().unwrap().0);
    }
    Some(available[0].0)
}

fn apply_choice(
    fixture: &Fixture,
    state: &mut WorldStateTable,
    action: u16,
    inventory: &mut BTreeMap<&'static str, u32>,
    ground_items: &mut BTreeSet<&'static str>,
    taken_items: &mut BTreeSet<&'static str>,
    policy: &str,
    rng: &mut u64,
) -> Option<usize> {
    let index = choose_choice(
        fixture,
        action,
        inventory,
        ground_items,
        taken_items,
        policy,
        rng,
    )?;
    let choice = fixture.choice_specs.get(&action)?.get(index)?.clone();
    debug_assert!(!choice.label.is_empty());
    let metric_deltas: Vec<(u16, f32)> = choice
        .metrics
        .iter()
        .map(|(name, delta)| (fixture.metrics.id(name).unwrap(), *delta))
        .collect();
    apply_mutation_with_registry(
        state,
        &MutationOp {
            metric_deltas,
            ..Default::default()
        },
        &fixture.tags,
    )
    .unwrap();
    for item in choice.consumes {
        if let Some(count) = inventory.get_mut(item) {
            *count = count.saturating_sub(1);
        }
    }
    if let Some(item) = choice.requires_ground {
        ground_items.remove(item);
        taken_items.insert(item);
    }
    for item in choice.grants {
        *inventory.entry(item).or_insert(0) += 1;
    }
    Some(index)
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

fn set_actor_location(
    state: &mut WorldStateTable,
    tags: &GameplayTagRegistry,
    actor: usize,
    on: bool,
) {
    let bit = tags.id(LOC_STATION).unwrap();
    if on {
        state.entity_tags[actor].set_bit(bit).unwrap();
    } else {
        state.entity_tags[actor].clear_bit(bit).unwrap();
    }
}

fn initial(fixture: &Fixture) -> WorldStateTable {
    let mut state = WorldStateTable::new();
    for name in PLAYER_METRICS {
        state
            .set_metric_bound(fixture.metrics.id(name).unwrap(), 0.0, 100.0)
            .unwrap();
    }
    for actor in 0..ACTORS {
        for stat in ["trust", "fear", "skill", "stability"] {
            let name = format!("a{actor}.{stat}");
            state
                .set_metric_bound(fixture.metrics.id(&name).unwrap(), 0.0, 100.0)
                .unwrap();
        }
    }
    for (name, value) in [
        ("p.guilt", 20.0),
        ("p.denial", 60.0),
        ("p.empathy", 40.0),
        ("p.paranoia", 20.0),
        ("p.courage", 40.0),
        ("p.growth", 0.0),
        ("p.truth", 0.0),
        ("p.deaths", 0.0),
    ] {
        let index = fixture.metrics.id(name).unwrap();
        state.metrics[index as usize] = value;
    }
    for actor in 0..ACTORS {
        let trust = fixture.metrics.id(&format!("a{actor}.trust")).unwrap();
        let fear = fixture.metrics.id(&format!("a{actor}.fear")).unwrap();
        let skill = fixture.metrics.id(&format!("a{actor}.skill")).unwrap();
        let stability = fixture.metrics.id(&format!("a{actor}.stability")).unwrap();
        state.metrics[trust as usize] = 50.0;
        state.metrics[fear as usize] = 30.0;
        state.metrics[skill as usize] = fixture.actor_skill[actor];
        state.metrics[stability as usize] = 50.0;
        for role_name in &fixture.actor_roles[actor] {
            state.entity_tags[actor]
                .set_bit(fixture.tags.id(role_name).unwrap())
                .unwrap();
        }
    }
    for name in [LOC_STATION, ACT_PROLOGUE, ARC1, "trunk.locked"] {
        set_tag(&mut state, &fixture.tags, name, true);
    }
    state
}

fn set_arc(state: &mut WorldStateTable, tags: &GameplayTagRegistry, turn: u64) {
    for arc in [ARC1, ARC2, ARC3] {
        set_tag(state, tags, arc, false);
    }
    let arc = if turn < 6 {
        ARC1
    } else if turn < 16 {
        ARC2
    } else {
        ARC3
    };
    set_tag(state, tags, arc, true);
}

fn chance(rng: &mut u64, probability: f64) -> bool {
    (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64) < probability
}

fn kill_actor(fixture: &Fixture, state: &mut WorldStateTable, actor: usize) {
    let dead = fixture.tags.id("actor.dead").unwrap();
    state.entity_tags[actor] = TagBitset::new();
    state.entity_tags[actor].set_bit(dead).unwrap();
    let deaths = fixture.metrics.id("p.deaths").unwrap() as usize;
    state.metrics[deaths] += 1.0;
}

fn maybe_kill(
    fixture: &Fixture,
    state: &mut WorldStateTable,
    dead_actors: &mut BTreeSet<usize>,
    rng: &mut u64,
    probability: f64,
) -> Option<usize> {
    if !chance(rng, probability) {
        return None;
    }
    let present: Vec<usize> = (0..ACTORS)
        .filter(|actor| {
            !dead_actors.contains(actor)
                && state.entity_tags[*actor].bit(fixture.tags.id(LOC_STATION).unwrap())
        })
        .collect();
    if present.is_empty() {
        return None;
    }
    let victim = present[(splitmix64(rng) as usize) % present.len()];
    kill_actor(fixture, state, victim);
    dead_actors.insert(victim);
    Some(victim)
}

fn update_presence(
    state: &mut WorldStateTable,
    tags: &GameplayTagRegistry,
    dead_actors: &BTreeSet<usize>,
    seen_present: &mut BTreeSet<usize>,
    turn: u64,
) -> (usize, usize, usize, usize) {
    let mut present = 0usize;
    let mut arrivals = 0usize;
    let mut departures = 0usize;
    let mut returns = 0usize;
    for actor in 0..ACTORS {
        let was = state.entity_tags[actor].bit(tags.id(LOC_STATION).unwrap());
        let now = actor_present(actor, turn) && !dead_actors.contains(&actor);
        if now && !was {
            arrivals += 1;
            if seen_present.contains(&actor) {
                returns += 1;
            }
        }
        if !now && was {
            departures += 1;
        }
        if now {
            present += 1;
            seen_present.insert(actor);
        }
        set_actor_location(state, tags, actor, now);
    }
    (present, arrivals, departures, returns)
}

fn busy_set(busy: &[u8]) -> BTreeSet<usize> {
    busy.iter()
        .enumerate()
        .filter_map(|(actor, turns)| (*turns > 0).then_some(actor))
        .collect()
}

fn playable_with_busy(
    fixture: &Fixture,
    state: &WorldStateTable,
    completed: &BTreeSet<u16>,
    busy: &BTreeSet<usize>,
    id: u16,
) -> bool {
    let Some(def) = fixture.dispatcher.storylet(id) else {
        return false;
    };
    if !def.is_repeatable && completed.contains(&id) {
        return false;
    }
    if !fixture
        .dispatcher
        .try_is_eligible(def, state)
        .unwrap_or(false)
    {
        return false;
    }
    if def.required_roles.is_empty() {
        return true;
    }
    fixture
        .dispatcher
        .bind_roles_excluding(def, state, busy)
        .is_ok()
}

fn bind(fixture: &Fixture, state: &WorldStateTable, id: u16, busy: &BTreeSet<usize>) -> Vec<usize> {
    let Some(def) = fixture.dispatcher.storylet(id) else {
        return Vec::new();
    };
    if def.required_roles.is_empty() {
        return Vec::new();
    }
    fixture
        .dispatcher
        .bind_roles_excluding(def, state, busy)
        .map(|bindings| {
            bindings
                .into_iter()
                .map(|binding| binding.entity_index)
                .collect()
        })
        .unwrap_or_default()
}

fn apply_action(
    fixture: &Fixture,
    state: &mut WorldStateTable,
    completed: &mut BTreeSet<u16>,
    busy: &mut [u8],
    action: u16,
) -> Option<Vec<usize>> {
    let def = fixture.dispatcher.storylet(action)?.clone();
    let busy_now = busy_set(busy);
    let bound = bind(fixture, state, action, &busy_now);
    if !def.required_roles.is_empty() && bound.is_empty() {
        return None;
    }
    apply_mutation_with_registry(state, &def.on_complete_mutation, &fixture.tags).unwrap();
    for actor in &bound {
        let trust = fixture.metrics.id(&format!("a{actor}.trust")).unwrap() as usize;
        let fear = fixture.metrics.id(&format!("a{actor}.fear")).unwrap() as usize;
        state.metrics[trust] = (state.metrics[trust] + 2.0).min(100.0);
        state.metrics[fear] = (state.metrics[fear] + 1.0).min(100.0);
        busy[*actor] = 2;
    }
    if !def.is_repeatable {
        completed.insert(action);
    }
    Some(bound)
}

fn canonical_ending(id: u16) -> u16 {
    match id {
        76 | 77 => 71,
        51..=54 => 78,
        _ => id,
    }
}

fn choose_policy_action(
    fixture: &Fixture,
    state: &WorldStateTable,
    completed: &BTreeSet<u16>,
    busy: &BTreeSet<usize>,
    inventory: &BTreeMap<&'static str, u32>,
    policy: &str,
    rng: &mut u64,
) -> Option<u16> {
    let mut eligible: Vec<u16> = fixture
        .dispatcher
        .query_qbn_deck(&fixture.deck, state, completed)
        .into_iter()
        .filter(|id| playable_with_busy(fixture, state, completed, busy, *id))
        .collect();
    if eligible.is_empty() {
        return None;
    }
    eligible.sort_by_key(|id| {
        let def = fixture.dispatcher.storylet(*id).unwrap();
        (-def.display_priority, *id)
    });
    if policy == "random" {
        let index = (splitmix64(rng) as usize) % eligible.len();
        return Some(eligible[index]);
    }
    if policy.starts_with("weighted") {
        let weights: Vec<f64> = eligible
            .iter()
            .map(|id| storylet_weight(*id, inventory))
            .collect();
        let total: f64 = weights.iter().sum();
        let mut cursor = (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64) * total;
        for (index, weight) in weights.iter().enumerate() {
            cursor -= *weight;
            if cursor <= 0.0 {
                return Some(eligible[index]);
            }
        }
        return eligible.last().copied();
    }
    let avoid: &[u16] = match policy {
        "combo_wife" => &[51, 52, 53, 54, 70, 71, 72, 73, 74, 75, 76, 77, 78, 80, 81],
        "combo_world" => &[
            51, 52, 53, 54, 55, 79, 70, 71, 72, 73, 74, 75, 76, 77, 78, 81,
        ],
        _ => &[],
    };
    let preference: &[u16] = match policy {
        "denial" => &[71, 76, 77, 10, 12, 13, 14, 11, 27, 28, 42, 43],
        "confess" => &[70, 71, 76, 77, 22, 41, 19, 10, 17, 23, 24, 25, 44, 42],
        "alien" => &[73, 21, 20, 29, 61, 42],
        "nuclear" => &[74, 16, 30, 62, 42],
        "zombie" => &[72, 17, 15, 60, 42],
        "silent" => &[75, 64, 22, 41, 23, 24, 63, 43, 15, 42],
        "truth_low" => &[76, 22, 41, 23, 24, 25, 42],
        "lethal" => &[78, 51, 52, 53, 54, 48, 47, 49, 50, 30, 16, 12, 17, 60, 42],
        "combo" => &[
            79, 80, 81, 55, 56, 49, 48, 47, 50, 22, 41, 23, 24, 17, 60, 42,
        ],
        "political" => &[81, 48, 49, 50, 47, 55, 56, 30, 16, 12, 28, 42],
        _ => &[],
    };
    for id in preference {
        if !avoid.contains(id) && eligible.contains(id) {
            return Some(*id);
        }
    }
    eligible.iter().find(|id| !avoid.contains(id)).copied()
}

fn world_items_at(turn: u64) -> Vec<&'static str> {
    if (3..=6).contains(&turn) {
        vec!["flashlight"]
    } else if (8..=11).contains(&turn) {
        vec!["radio_parts"]
    } else if (13..=17).contains(&turn) {
        vec!["key"]
    } else {
        Vec::new()
    }
}

fn state_hash(fixture: &Fixture, state: &WorldStateTable) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for word in state.global_tags.words() {
        hash ^= *word;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for (index, value) in state.metrics.iter().enumerate() {
        hash ^= value.to_bits() as u64 ^ index as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for actor in 0..ACTORS {
        for word in state.entity_tags[actor].words() {
            hash ^= *word;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
    }
    for value in &state.passions.micro {
        hash ^= value.to_bits() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for value in &state.passions.macro_values {
        hash ^= value.to_bits() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    let _ = fixture;
    hash
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    z ^ (z >> 31)
}

struct TrialResult {
    ending: Option<u16>,
    ending_storylet: Option<u16>,
    fallback_ending: bool,
    left: bool,
    obstruction: u8,
    turns: u64,
    hub_used: BTreeSet<u16>,
    actors_used: BTreeSet<usize>,
    role_counts: BTreeMap<u16, u64>,
    gap_sum: f64,
    gap_count: u64,
    gap_max: f64,
    salience_phases: BTreeMap<u8, u64>,
    premature: u64,
    max_present: usize,
    arrivals: usize,
    departures: usize,
    deck_sum: u64,
    deck_max: usize,
    hash: u64,
    choices_taken: u64,
    pickups: u64,
    gifts: u64,
    first_hub: Option<u16>,
    choice_labels: BTreeMap<&'static str, u64>,
    deaths: u64,
    fixed_deaths: u64,
    dead_actors: BTreeSet<usize>,
    returns: u64,
}

fn run_trial(fixture: &Fixture, seed: u64, policy: &str) -> TrialResult {
    run_trial_forced(fixture, seed, policy, None)
}

fn run_trial_forced(
    fixture: &Fixture,
    seed: u64,
    policy: &str,
    forced_left: Option<bool>,
) -> TrialResult {
    let mut state = initial(fixture);
    let mut completed = BTreeSet::new();
    let mut busy = vec![0u8; ACTORS];
    let mut rng = seed;
    let mut hub_used = BTreeSet::new();
    let mut actors_used = BTreeSet::new();
    let mut role_counts: BTreeMap<u16, u64> = BTreeMap::new();
    let mut gap_sum = 0.0;
    let mut gap_count = 0u64;
    let mut gap_max: f64 = 0.0;
    let mut salience_phases: BTreeMap<u8, u64> = BTreeMap::new();
    let mut premature = 0u64;
    let mut max_present = 0usize;
    let mut arrivals = 0usize;
    let mut departures = 0usize;
    let mut deck_sum = 0u64;
    let mut deck_max = 0usize;
    let mut hub_turn = 0u64;
    let mut ending = None;
    let mut ending_storylet = None;
    let mut inventory: BTreeMap<&'static str, u32> = BTreeMap::new();
    inventory.insert("lighter", 1);
    inventory.insert("wallet", 1);
    inventory.insert("phone", 1);
    if policy == "weighted_empty" {
        inventory.clear();
    }
    if policy == "weighted_geared" {
        for item in [
            "wrench",
            "fuel",
            "radio_parts",
            "key",
            "medkit",
            "flashlight",
            "military_code",
            "locket",
        ] {
            inventory.insert(item, 1);
        }
    }
    let mut ground_items: BTreeSet<&'static str> = BTreeSet::new();
    let mut taken_items: BTreeSet<&'static str> = BTreeSet::new();
    let mut choices_taken = 0u64;
    let mut choice_labels: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut pickups = 0u64;
    let mut gifts = 0u64;
    let mut first_hub = None;
    let mut dead_actors: BTreeSet<usize> = BTreeSet::new();
    let mut seen_present: BTreeSet<usize> = BTreeSet::new();
    let mut returns = 0u64;
    let mut deaths = 0u64;
    let mut fixed_deaths = 0u64;

    let left = forced_left.unwrap_or_else(|| splitmix64(&mut rng) & 1 == 0);
    let direction = if left { 2 } else { 3 };
    let object_slot = (splitmix64(&mut rng) % 2) as u8;
    let obstruction = if left { object_slot } else { 2 + object_slot };
    for action in [1, direction, 4 + obstruction as u16, 8] {
        let busy_now = busy_set(&busy);
        assert!(
            playable_with_busy(fixture, &state, &completed, &busy_now, action),
            "prologue action {action} was not eligible"
        );
        apply_action(fixture, &mut state, &mut completed, &mut busy, action);
        if let Some(index) = apply_choice(
            fixture,
            &mut state,
            action,
            &mut inventory,
            &mut ground_items,
            &mut taken_items,
            policy,
            &mut rng,
        ) {
            let spec = &fixture.choice_specs[&action][index];
            choices_taken += 1;
            *choice_labels.entry(spec.label).or_insert(0) += 1;
            if spec.requires_ground.is_some() {
                pickups += 1;
            }
        }
    }
    set_tag(&mut state, &fixture.tags, ACT_HUB, true);

    loop {
        hub_turn += 1;
        if hub_turn > HUB_TURNS {
            break;
        }
        for count in busy.iter_mut() {
            if *count > 0 {
                *count -= 1;
            }
        }
        let (present, arrived, left, returned) = update_presence(
            &mut state,
            &fixture.tags,
            &dead_actors,
            &mut seen_present,
            hub_turn,
        );
        max_present = max_present.max(present);
        arrivals += arrived;
        departures += left;
        returns += returned as u64;
        set_arc(&mut state, &fixture.tags, hub_turn);
        ground_items = world_items_at(hub_turn)
            .into_iter()
            .filter(|item| !taken_items.contains(item))
            .collect();

        if [4u64, 8, 12, 16].contains(&hub_turn) {
            let seed = splitmix64(&mut rng);
            if let Some(selected) = fixture.dispatcher.match_most_salient(
                &fixture.salience_pool,
                &state,
                &completed,
                seed,
            ) {
                let def = fixture.dispatcher.storylet(selected).unwrap();
                let current_phase = if has(&state, &fixture.tags, ARC1) {
                    1
                } else if has(&state, &fixture.tags, ARC2) {
                    2
                } else {
                    3
                };
                let expected = if def.required_arc_phases.bit(fixture.tags.id(ARC1).unwrap()) {
                    1
                } else if def.required_arc_phases.bit(fixture.tags.id(ARC2).unwrap()) {
                    2
                } else {
                    3
                };
                if expected != current_phase {
                    premature += 1;
                }
                *salience_phases.entry(current_phase).or_insert(0) += 1;
                let op = def.on_complete_mutation.clone();
                apply_mutation_with_registry(&mut state, &op, &fixture.tags).unwrap();
                if !def.is_repeatable {
                    completed.insert(selected);
                }
                if let Some(index) = apply_choice(
                    fixture,
                    &mut state,
                    selected,
                    &mut inventory,
                    &mut ground_items,
                    &mut taken_items,
                    policy,
                    &mut rng,
                ) {
                    let spec = &fixture.choice_specs[&selected][index];
                    choices_taken += 1;
                    *choice_labels.entry(spec.label).or_insert(0) += 1;
                    if spec.requires_ground.is_some() {
                        pickups += 1;
                    }
                }
            }
        }

        let busy_now = busy_set(&busy);
        let deck = fixture
            .dispatcher
            .query_qbn_deck(&fixture.deck, &state, &completed);
        deck_sum += deck.len() as u64;
        deck_max = deck_max.max(deck.len());
        let Some(action) = choose_policy_action(
            fixture, &state, &completed, &busy_now, &inventory, policy, &mut rng,
        ) else {
            break;
        };
        let def = fixture.dispatcher.storylet(action).unwrap();
        let required_roles = def.required_roles.clone();
        let Some(bound) = apply_action(fixture, &mut state, &mut completed, &mut busy, action)
        else {
            break;
        };
        if first_hub.is_none() && (10..=56).contains(&action) {
            first_hub = Some(action);
        }
        if let Some(index) = apply_choice(
            fixture,
            &mut state,
            action,
            &mut inventory,
            &mut ground_items,
            &mut taken_items,
            policy,
            &mut rng,
        ) {
            let spec = &fixture.choice_specs[&action][index];
            choices_taken += 1;
            *choice_labels.entry(spec.label).or_insert(0) += 1;
            if spec.requires_ground.is_some() {
                pickups += 1;
            }
            if !required_roles.is_empty() && !spec.grants.is_empty() {
                gifts += 1;
            }
        }
        if required_roles.len() == 1 {
            let role_tag = required_roles[0].role_tag.as_str();
            let role_bit = fixture.tags.id(role_tag).unwrap();
            *role_counts.entry(role_bit).or_insert(0) += 1;
            if let Some(actor) = bound.first().copied() {
                let candidates: Vec<f32> = (0..ACTORS)
                    .filter(|other| {
                        fixture.actor_roles[*other]
                            .iter()
                            .any(|name| *name == role_tag)
                            && state.entity_tags[*other].bit(fixture.tags.id(LOC_STATION).unwrap())
                            && !busy_now.contains(other)
                    })
                    .map(|other| fixture.actor_skill[other])
                    .collect();
                if !candidates.is_empty() {
                    let best = candidates.into_iter().fold(f32::MIN, f32::max);
                    let gap = (best - fixture.actor_skill[actor]).max(0.0) as f64;
                    gap_sum += gap;
                    gap_max = gap_max.max(gap);
                    gap_count += 1;
                }
            }
        }
        for actor in bound {
            actors_used.insert(actor);
        }
        if (10..=56).contains(&action) {
            hub_used.insert(action);
        }
        if !(70..=81).contains(&action) {
            if let Some(victim) = maybe_kill(fixture, &mut state, &mut dead_actors, &mut rng, 0.08)
            {
                deaths += 1;
                if victim < 5 {
                    fixed_deaths += 1;
                }
            }
            if has(&state, &fixture.tags, "apocalypse.zombie") {
                if let Some(victim) =
                    maybe_kill(fixture, &mut state, &mut dead_actors, &mut rng, 0.2)
                {
                    deaths += 1;
                    if victim < 5 {
                        fixed_deaths += 1;
                    }
                }
            }
        }
        if (51..=54).contains(&action) {
            ending = Some(78);
            ending_storylet = Some(78);
            break;
        }
        if (70..=81).contains(&action) {
            ending = Some(canonical_ending(action));
            ending_storylet = Some(action);
            break;
        }
    }

    let mut fallback_ending = false;
    if ending.is_none() {
        let busy_now = busy_set(&busy);
        if let Some(fallback) =
            (70..=81).find(|id| playable_with_busy(fixture, &state, &completed, &busy_now, *id))
        {
            apply_action(fixture, &mut state, &mut completed, &mut busy, fallback);
            let _ = apply_choice(
                fixture,
                &mut state,
                fallback,
                &mut inventory,
                &mut ground_items,
                &mut taken_items,
                policy,
                &mut rng,
            );
            ending = Some(canonical_ending(fallback));
            ending_storylet = Some(fallback);
            fallback_ending = true;
        }
    }

    TrialResult {
        ending,
        ending_storylet,
        fallback_ending,
        left,
        obstruction,
        turns: hub_turn,
        hub_used,
        actors_used,
        role_counts,
        gap_sum,
        gap_count,
        gap_max,
        salience_phases,
        premature,
        max_present,
        arrivals,
        departures,
        deck_sum,
        deck_max,
        hash: state_hash(fixture, &state),
        choices_taken,
        pickups,
        gifts,
        first_hub,
        choice_labels,
        deaths,
        fixed_deaths,
        dead_actors,
        returns,
    }
}

#[derive(Default)]
struct PolicyStats {
    trials: u64,
    no_ending: u64,
    ending_variants: BTreeMap<u16, u64>,
    fallback_endings: u64,
    left: u64,
    right: u64,
    obstructions: [u64; 4],
    endings: BTreeMap<u16, u64>,
    turns: u64,
    hub: BTreeMap<u16, u64>,
    actors: BTreeSet<usize>,
    actor_trials: BTreeMap<usize, u64>,
    roles: BTreeMap<u16, u64>,
    gap_sum: f64,
    gap_count: u64,
    gap_max: f64,
    salience: BTreeMap<u8, u64>,
    premature: u64,
    max_present: usize,
    arrivals: usize,
    departures: usize,
    deck_sum: u64,
    deck_max: usize,
    deaths: u64,
    fixed_deaths: u64,
    dead_actors: BTreeSet<usize>,
    choices_taken: u64,
    pickups: u64,
    gifts: u64,
    first_hub: BTreeMap<u16, u64>,
    choice_labels: BTreeMap<&'static str, u64>,
    returns: u64,
}

fn run_policy(fixture: &Fixture, policy: &str) -> PolicyStats {
    let mut stats = PolicyStats::default();
    for seed in 0..TRIALS {
        let result = run_trial(fixture, seed, policy);
        stats.trials += 1;
        if result.left {
            stats.left += 1;
        } else {
            stats.right += 1;
        }
        stats.obstructions[result.obstruction as usize] += 1;
        if let Some(ending) = result.ending {
            *stats.endings.entry(ending).or_insert(0) += 1;
        } else {
            stats.no_ending += 1;
        }
        if result.fallback_ending {
            stats.fallback_endings += 1;
        }
        if let Some(storylet) = result.ending_storylet {
            *stats.ending_variants.entry(storylet).or_insert(0) += 1;
        }
        stats.turns += result.turns;
        for id in result.hub_used {
            *stats.hub.entry(id).or_insert(0) += 1;
        }
        for actor in result.actors_used {
            stats.actors.insert(actor);
            *stats.actor_trials.entry(actor).or_insert(0) += 1;
        }
        for (role, count) in result.role_counts {
            *stats.roles.entry(role).or_insert(0) += count;
        }
        stats.gap_sum += result.gap_sum;
        stats.gap_count += result.gap_count;
        stats.gap_max = stats.gap_max.max(result.gap_max);
        for (phase, count) in result.salience_phases {
            *stats.salience.entry(phase).or_insert(0) += count;
        }
        stats.premature += result.premature;
        stats.max_present = stats.max_present.max(result.max_present);
        stats.arrivals += result.arrivals;
        stats.departures += result.departures;
        stats.deck_sum += result.deck_sum;
        stats.deck_max = stats.deck_max.max(result.deck_max);
        stats.deaths += result.deaths;
        stats.returns += result.returns;
        stats.fixed_deaths += result.fixed_deaths;
        stats.dead_actors.extend(result.dead_actors);
        stats.choices_taken += result.choices_taken;
        for (label, count) in result.choice_labels {
            *stats.choice_labels.entry(label).or_insert(0) += count;
        }
        stats.pickups += result.pickups;
        stats.gifts += result.gifts;
        if let Some(first) = result.first_hub {
            *stats.first_hub.entry(first).or_insert(0) += 1;
        }
    }
    stats
}

fn full_cast_check(fixture: &Fixture) -> (usize, usize) {
    let mut state = initial(fixture);
    for actor in 0..ACTORS {
        set_actor_location(&mut state, &fixture.tags, actor, true);
    }
    let mut busy = vec![0u8; ACTORS];
    let mut used = BTreeSet::new();
    for _round in 0..6 {
        for id in [
            10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 47, 48, 49, 50,
        ]
        .iter()
        .copied()
        {
            let busy_now = busy_set(&busy);
            let bound = bind(fixture, &state, id, &busy_now);
            for actor in bound {
                used.insert(actor);
                busy[actor] = 6;
            }
        }
        for count in busy.iter_mut() {
            if *count > 0 {
                *count -= 1;
            }
        }
    }
    (used.len(), ACTORS)
}

fn rollback_check(fixture: &Fixture) -> (u64, u64) {
    let mut state = initial(fixture);
    let mut exact = 0u64;
    for iteration in 0..100u16 {
        let before = state_hash(fixture, &state);
        let actor = (iteration as usize) % ACTORS;
        let mut op = MutationOp {
            metric_deltas: vec![
                (fixture.metrics.id("p.guilt").unwrap(), 1.0),
                (
                    fixture.metrics.id(&format!("a{actor}.trust")).unwrap(),
                    -1.0,
                ),
            ],
            ..Default::default()
        };
        let station = fixture.tags.id(LOC_STATION).unwrap();
        op.entity_tag_ops.push(EntityTagMutation {
            entity_id: actor as u16,
            add_tags: match iteration % 2 {
                0 => {
                    let mut bits = TagBitset::new();
                    bits.set_bit(station).unwrap();
                    bits
                }
                _ => TagBitset::new(),
            },
            remove_tags: match iteration % 2 {
                0 => TagBitset::new(),
                _ => {
                    let mut bits = TagBitset::new();
                    bits.set_bit(station).unwrap();
                    bits
                }
            },
        });
        let delta = apply_mutation_with_registry(&mut state, &op, &fixture.tags).unwrap();
        revert_delta(&mut state, &delta).unwrap();
        if state_hash(fixture, &state) == before {
            exact += 1;
        }
    }
    (exact, 100)
}

fn waypoint_check() -> bool {
    let graph = WaypointTopicGraph {
        nodes: ["people", "god", "government", "royalty", "revolution"]
            .iter()
            .map(|id| WaypointTopicNode {
                topic_id: (*id).to_owned(),
                is_player_only_trigger: *id == "revolution",
                ..Default::default()
            })
            .collect(),
        edges: [
            ("people", "god", 1.0, 5.0, false),
            ("people", "government", 2.0, 5.0, false),
            ("god", "royalty", 1.0, 5.0, false),
            ("government", "royalty", 1.0, 5.0, false),
            ("people", "revolution", 1.0, 5.0, false),
        ]
        .iter()
        .enumerate()
        .map(
            |(index, (from, to, base, penalty, fallback))| WaypointTopicEdge {
                edge_id: format!("e{index}"),
                from_topic_id: (*from).to_owned(),
                to_topic_id: (*to).to_owned(),
                base_cost: *base,
                used_penalty_cost: *penalty,
                is_fallback_default: *fallback,
                ..Default::default()
            },
        )
        .collect(),
    };
    let dispatcher = UnifiedDispatcher::new(Vec::new());
    let mut counts = BTreeMap::new();
    let first = dispatcher
        .compute_next_npc_topic_step(&graph, "people", "royalty", &counts)
        .unwrap();
    if first.to_topic_id != "god" || first.via_fallback {
        return false;
    }
    counts.insert(first.edge_id.clone(), 1);
    let second = dispatcher
        .compute_next_npc_topic_step(&graph, "people", "royalty", &counts)
        .unwrap();
    // The used people->god edge is penalized, so the NPC routes via government.
    second.to_topic_id == "government" && !second.via_fallback
}

fn scene_check() -> (usize, usize, bool) {
    let part = |entity: &str, event: u64, priority: i32, pose: &str| ActionPart {
        action_key_priority: priority,
        combiner_entity_path: entity.to_owned(),
        activity: EventOverlapActivity::RegularActivity,
        local_cursor_time_ms: 0,
        event_ref: TimelineTrackEvent {
            event_id: event,
            target_entity_path: entity.to_owned(),
            start_time_ms: 0,
            duration_ms: 50,
            payload_asset_id: pose.to_owned(),
            cached_previous_pose_id: format!("prev.{entity}"),
            priority_offset: 0,
            ..Default::default()
        },
    };
    let mut left: Vec<ActionPart> = Vec::new();
    let mut right: Vec<ActionPart> = Vec::new();
    let mut left_actor0 = part("actor0", 1, 2, "pose.left.actor0");
    left_actor0.event_ref.cached_previous_pose_id = "prev.left.actor0".to_owned();
    let mut right_actor0 = part("actor0", 2, 4, "pose.right.actor0");
    right_actor0.event_ref.cached_previous_pose_id = "prev.right.actor0".to_owned();
    let distinct_poses = left_actor0.event_ref.cached_previous_pose_id
        != right_actor0.event_ref.cached_previous_pose_id;
    left.push(left_actor0);
    right.push(right_actor0);
    for actor in 1..ACTORS {
        let target = if actor % 2 == 0 {
            &mut left
        } else {
            &mut right
        };
        target.push(part(
            &format!("actor{actor}"),
            100 + actor as u64,
            1,
            &format!("pose.actor{actor}"),
        ));
    }
    let mut composer = SceneComposer::default();
    let applied = composer
        .execute_sorted_action_parts_multi(&[(10, &left), (11, &right)], false)
        .unwrap();
    let winner_is_right = composer
        .conflicts
        .first()
        .map(|conflict| conflict.winner.node_guid == 11)
        .unwrap_or(false);
    let reverse_ok = {
        let mut reverse = SceneComposer::default();
        reverse
            .execute_sorted_action_parts_multi(&[(10, &left), (11, &right)], true)
            .unwrap();
        reverse.actor_pose.get("actor0").map(String::as_str) == Some("prev.right.actor0")
    };
    (
        composer.conflicts.len(),
        applied.len(),
        winner_is_right && reverse_ok && distinct_poses,
    )
}

fn inventory_weight_check() -> bool {
    let empty = BTreeMap::new();
    let mut geared = BTreeMap::new();
    for item in [
        "wrench",
        "fuel",
        "radio_parts",
        "key",
        "medkit",
        "flashlight",
        "military_code",
        "locket",
    ] {
        geared.insert(item, 1);
    }
    storylet_weight(11, &geared) > storylet_weight(11, &empty)
        && storylet_weight(21, &geared) > storylet_weight(21, &empty)
        && storylet_weight(23, &geared) > storylet_weight(23, &empty)
        && storylet_weight(24, &geared) > storylet_weight(24, &empty)
        && storylet_weight(25, &geared) > storylet_weight(25, &empty)
}

fn linked_rollback_check(fixture: &Fixture) -> bool {
    let mut state = initial(fixture);
    for name in [ACT_PROLOGUE, ARC1, ARC2] {
        set_tag(&mut state, &fixture.tags, name, false);
    }
    for name in [ACT_HUB, ARC3] {
        set_tag(&mut state, &fixture.tags, name, true);
    }
    let before = state_hash(fixture, &state);
    let mut deltas = Vec::new();
    for id in [47u16, 48, 49] {
        let def = fixture.dispatcher.storylet(id).unwrap();
        deltas.push(
            apply_mutation_with_registry(&mut state, &def.on_complete_mutation, &fixture.tags)
                .unwrap(),
        );
    }
    let linked = ["done.47", "done.48", "done.49"]
        .iter()
        .all(|name| has(&state, &fixture.tags, name));
    for delta in deltas.iter().rev() {
        revert_delta(&mut state, delta).unwrap();
    }
    linked && state_hash(fixture, &state) == before
}

fn growth_boundary_check(fixture: &Fixture) -> bool {
    let mut state = initial(fixture);
    for name in [ACT_PROLOGUE, ARC1, ARC2, ARC3] {
        set_tag(&mut state, &fixture.tags, name, false);
    }
    for name in [ACT_HUB, ARC3, "truth.remembered"] {
        set_tag(&mut state, &fixture.tags, name, true);
    }
    for name in [
        "human.hit",
        "apocalypse.zombie",
        "apocalypse.alien",
        "apocalypse.nuclear",
        "apocalypse.silent",
    ] {
        set_tag(&mut state, &fixture.tags, name, false);
    }
    let growth = fixture.metrics.id("p.growth").unwrap() as usize;
    let completed = BTreeSet::new();
    let busy = BTreeSet::new();
    state.metrics[growth] = 29.0;
    let low = playable_with_busy(fixture, &state, &completed, &busy, 76)
        && !playable_with_busy(fixture, &state, &completed, &busy, 70)
        && !playable_with_busy(fixture, &state, &completed, &busy, 71)
        && !playable_with_busy(fixture, &state, &completed, &busy, 77);
    state.metrics[growth] = 30.0;
    let high = playable_with_busy(fixture, &state, &completed, &busy, 70)
        && !playable_with_busy(fixture, &state, &completed, &busy, 76)
        && !playable_with_busy(fixture, &state, &completed, &busy, 71)
        && !playable_with_busy(fixture, &state, &completed, &busy, 77);
    low && high
}

fn main() {
    let fixture = build_fixture();
    let started = Instant::now();
    let policies = [
        "denial",
        "confess",
        "alien",
        "nuclear",
        "zombie",
        "silent",
        "truth_low",
        "random",
        "weighted_empty",
        "weighted_geared",
        "lethal",
        "combo_wife",
        "combo_world",
        "political",
    ];
    let mut all_endings = BTreeSet::new();
    let mut all_hub = BTreeSet::new();
    let mut all_actors = BTreeSet::new();
    let mut all_roles = BTreeSet::new();
    let mut all_choice_labels: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut total_trials = 0u64;

    let choice_min = fixture
        .choice_specs
        .values()
        .map(Vec::len)
        .min()
        .unwrap_or(0);
    let choice_max = fixture
        .choice_specs
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    println!(
        "Gas-station stress: {} actors, {} roles, {} storylets, {} hub turns, {} trials/policy",
        ACTORS,
        ROLE_NAMES.len(),
        fixture.dispatcher.storylets.len(),
        HUB_TURNS,
        TRIALS
    );
    let done_names: Vec<String> = (1..=100).map(|id| format!("done.{id}")).collect();
    let done_refs: Vec<&str> = done_names.iter().map(String::as_str).collect();
    let done_mask = mask(&fixture.tags, &done_refs);
    let linked = fixture
        .dispatcher
        .storylets
        .iter()
        .filter(|def| def.required_tags.popcount_intersection(&done_mask) > 0)
        .count();
    let multi_linked = fixture
        .dispatcher
        .storylets
        .iter()
        .filter(|def| def.required_tags.popcount_intersection(&done_mask) >= 2)
        .count();
    println!(
        "choice validation: {} storylets, choices per storylet min {} max {} | linked storylets {} (multi {})",
        fixture.choice_specs.len(),
        choice_min,
        choice_max,
        linked,
        multi_linked
    );

    for policy in policies {
        let stats = run_policy(&fixture, policy);
        total_trials += stats.trials;
        all_endings.extend(stats.endings.keys().copied());
        all_hub.extend(stats.hub.keys().copied());
        all_actors.extend(stats.actors.iter().copied());
        all_roles.extend(stats.roles.keys().copied());
        for (label, count) in &stats.choice_labels {
            *all_choice_labels.entry(*label).or_insert(0) += count;
        }
        println!("\nPOLICY {policy}");
        println!(
            "  endings {:?} | mean turns {:.2} | hub coverage {}/{} | actors used {}/{} | roles {}/{}",
            stats
                .endings
                .iter()
                .map(|(id, count)| (fixture.names.get(id).copied().unwrap_or("?"), *count))
                .collect::<BTreeMap<_, _>>(),
            stats.turns as f64 / stats.trials as f64,
            stats.hub.len(),
            (10..=56).count(),
            stats.actors.len(),
            ACTORS,
            stats.roles.len(),
            ROLE_NAMES.len()
        );
        println!(
            "  no ending {} | fallback endings {} | ending storylets {:?}",
            stats.no_ending, stats.fallback_endings, stats.ending_variants
        );
        println!(
            "  choices {} | pickups {} | npc gifts {} | returns {} | deaths {} (fixed {}) | unique dead {}",
            stats.choices_taken,
            stats.pickups,
            stats.gifts,
            stats.returns,
            stats.deaths,
            stats.fixed_deaths,
            stats.dead_actors.len()
        );
        println!(
            "  openings left {} / right {} | obstructions deer {} ghost {} zombie {} human {}",
            stats.left,
            stats.right,
            stats.obstructions[0],
            stats.obstructions[1],
            stats.obstructions[2],
            stats.obstructions[3]
        );
        println!(
            "  mean QBN eligible/turn {:.1}, max {} | max present {} | arrivals {} departures {} | salience {:?} premature {}",
            stats.deck_sum as f64 / stats.turns as f64,
            stats.deck_max,
            stats.max_present,
            stats.arrivals,
            stats.departures,
            stats.salience,
            stats.premature
        );
        println!(
            "  suitability gap mean {:.2} max {:.2} over {} bindings | actor appearances {:?}",
            if stats.gap_count == 0 {
                0.0
            } else {
                stats.gap_sum / stats.gap_count as f64
            },
            stats.gap_max,
            stats.gap_count,
            {
                let mut counts: Vec<usize> = (0..ACTORS)
                    .map(|actor| *stats.actor_trials.get(&actor).unwrap_or(&0) as usize)
                    .collect();
                counts.sort_unstable();
                (
                    counts.first().copied().unwrap_or(0),
                    counts.last().copied().unwrap_or(0),
                )
            }
        );
    }

    let orphans: Vec<u16> = (10..=45).filter(|id| !all_hub.contains(id)).collect();
    let unused_actors: Vec<usize> = (0..ACTORS)
        .filter(|actor| !all_actors.contains(actor))
        .collect();
    let growth_boundary = growth_boundary_check(&fixture);
    let linked_rollback = linked_rollback_check(&fixture);
    let inventory_weight = inventory_weight_check();
    let (cast_bound, cast_total) = full_cast_check(&fixture);
    let (rollback_exact, rollback_total) = rollback_check(&fixture);
    let waypoint_ok = waypoint_check();
    let (conflicts, applied, scene_ok) = scene_check();
    let same_seed_a = run_trial(&fixture, 42, "confess");
    let same_seed_b = run_trial(&fixture, 42, "confess");
    let deterministic =
        same_seed_a.ending == same_seed_b.ending && same_seed_a.hash == same_seed_b.hash;

    let mut paired_total = 0u64;
    let mut paired_diverged = 0u64;
    let mut paired_by_policy: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for policy in policies {
        let mut diverged = 0u64;
        for seed in 0..200u64 {
            let left = run_trial_forced(&fixture, seed, policy, Some(true));
            let right = run_trial_forced(&fixture, seed, policy, Some(false));
            paired_total += 1;
            if left.ending != right.ending {
                diverged += 1;
            }
        }
        paired_diverged += diverged;
        paired_by_policy.insert(policy, (diverged, 200));
    }
    println!("\nPAIRED OPENING (same seed, forced left vs right)");
    println!(
        "  diverged {}/{} | by policy {:?}",
        paired_diverged, paired_total, paired_by_policy
    );

    let mut weighted_total = 0u64;
    let mut weighted_first_diverged = 0u64;
    for seed in 0..200u64 {
        let empty = run_trial_forced(&fixture, seed, "weighted_empty", None);
        let geared = run_trial_forced(&fixture, seed, "weighted_geared", None);
        weighted_total += 1;
        if empty.first_hub != geared.first_hub {
            weighted_first_diverged += 1;
        }
    }
    println!("\nINVENTORY WEIGHTING (same seed, empty vs geared inventory, weighted policy)");
    println!(
        "  first hub choice diverged {}/{}",
        weighted_first_diverged, weighted_total
    );

    let all_labels: BTreeSet<&'static str> = fixture
        .choice_specs
        .values()
        .flat_map(|choices| choices.iter().map(|choice| choice.label))
        .collect();
    let unused_labels: Vec<&'static str> = all_labels
        .iter()
        .filter(|label| !all_choice_labels.contains_key(*label))
        .copied()
        .collect();

    println!("\nGLOBAL");
    println!(
        "  endings covered {}/10 {:?} | hub coverage {}/{} | actors bound {}/{} | roles used {}/{}",
        all_endings.len(),
        all_endings
            .iter()
            .map(|id| fixture.names.get(id).copied().unwrap_or("?"))
            .collect::<BTreeSet<_>>(),
        all_hub.len(),
        (10..=56).count(),
        all_actors.len(),
        ACTORS,
        all_roles.len(),
        ROLE_NAMES.len()
    );
    println!(
        "  unused choice labels {:?} | choices used {}",
        unused_labels,
        all_choice_labels.len()
    );
    println!(
        "  hub orphans {:?} | unused actors {:?} | trials {} | elapsed {:?}",
        orphans,
        unused_actors,
        total_trials,
        started.elapsed()
    );
    println!(
        "  growth boundary 29/30 {} | linked rollback {} | inventory weight {} | full cast rotation {}/{} | rollback exact {}/{} | waypoint diversion {} | scene conflicts {} applied {} valid {} | same-seed deterministic {}",
        growth_boundary,
        linked_rollback,
        inventory_weight,
        cast_bound,
        cast_total,
        rollback_exact,
        rollback_total,
        waypoint_ok,
        conflicts,
        applied,
        scene_ok,
        deterministic
    );
}
