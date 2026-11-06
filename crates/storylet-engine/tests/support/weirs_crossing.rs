//! Shared Rust fixture for the Weir's Crossing exploration prototype.
use std::collections::BTreeMap;

use storylet_engine::chrono_dispatch::{StoryletDef, UnifiedDispatcher};
use storylet_engine::chrono_participants::{Controller, KnowKind};
use storylet_engine::chrono_runtime::{
    DataEdgeConnection, DataPinDefinition, EdgeConnection, FlowGraphAsset, FlowNodeDef, HolderRef,
    NodeCategory, NodeKind, PinDefinition, Visibility,
};
use storylet_engine::chrono_space::{Clock, ClockState, Connection, OnClose};
use storylet_engine::chrono_state::{
    GameplayTagRegistry, PassionRegistry, TagBitset, WorldStateTable,
};
use storylet_engine::run::RunContent;

pub const WATCH_THE_WATER: u16 = 0;
pub const LAMP_SOOT: u16 = 1;
pub const ILSE_LOGBOOK: u16 = 2;
pub const HARBOR_MASTER_TIDES: u16 = 3;
pub const PRESS_TEODOR: u16 = 4;
pub const TEODOR_LANTERN_MEMORY: u16 = 5;
pub const CUSTOMS_LEDGER: u16 = 6;
pub const ASK_ROSA_OPENLY: u16 = 7;
pub const LISTEN_QUIETLY: u16 = 8;
pub const PIECE_TOGETHER: u16 = 9;
pub const CONFRONT_BENEDEK: u16 = 10;
pub const FIND_ILSE: u16 = 11;
pub const BLUFF_BENEDEK: u16 = 12;
pub const ENDING_LIGHT_RESTORED: u16 = 13;
pub const ENDING_KEEPER_COMES_HOME: u16 = 14;
pub const ENDING_LEDGER_WITH_A_NAME: u16 = 15;
pub const ENDING_LOW_WATER: u16 = 16;

pub const LIGHTHOUSE: u16 = 0;
pub const HARBOR: u16 = 1;
pub const INN: u16 = 2;
pub const CUSTOMS: u16 = 3;
pub const ROCK: u16 = 4;
pub const MARA: u16 = 0;
pub const TEODOR: u16 = 1;
pub const BENEDEK: u16 = 2;
pub const ROSA: u16 = 3;

const STORYLETS: [(&str, &str); 17] = [
    ("watch_the_water", "Watch the water"),
    ("lamp_soot", "Lamp soot"),
    ("ilse_logbook", "Ilse's logbook"),
    ("harbor_master_tides", "Harbor master tides"),
    ("press_teodor", "Press Teodor"),
    ("teodor_lantern_memory", "Teodor's lantern memory"),
    ("customs_ledger", "Customs ledger"),
    ("ask_rosa_openly", "Ask Rosa openly"),
    ("listen_quietly", "Listen quietly"),
    ("piece_together", "Piece together the clues"),
    ("confront_benedek", "Confront Benedek"),
    ("find_ilse", "Find Ilse"),
    ("bluff_benedek", "Bluff Benedek"),
    ("ending_light_restored", "The light restored"),
    ("ending_keeper_comes_home", "The keeper comes home"),
    ("ending_ledger_with_a_name", "A ledger with a name"),
    ("ending_low_water", "Low water"),
];
const FACTS: [&str; 12] = [
    "fact.light_doused",
    "fact.logbook",
    "fact.tides",
    "fact.crates_landed",
    "fact.benedek_suspected",
    "fact.benedek_identified",
    "fact.benedek_confessed",
    "fact.ilse_alive",
    "fact.where_rock",
    "fact.benedek_delayed",
    "fact.ilse_rescued",
    "fact.belief_ilse_neglected",
];

fn pin(name: &str, is_input: bool) -> PinDefinition {
    PinDefinition {
        pin_name: name.into(),
        is_input,
    }
}
fn node(guid: u32, kind: NodeKind, category: NodeCategory, outputs: &[&str]) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: format!("node_{guid}"),
        category,
        kind,
        input_pins: vec![pin("In", true)],
        output_pins: outputs.iter().map(|s| pin(s, false)).collect(),
        ..Default::default()
    }
}
fn edge(from: u32, output: &str, to: u32) -> EdgeConnection {
    EdgeConnection {
        source_node_guid: from,
        source_pin_name: output.into(),
        target_node_guid: to,
        target_pin_name: "In".into(),
    }
}
fn knows_graph(fact: u16) -> FlowGraphAsset {
    let mut condition = node(
        100,
        NodeKind::Condition {
            operator: Default::default(),
            literal: None,
        },
        NodeCategory::LogicGate,
        &["true", "false"],
    );
    condition.data_input_pins = vec![DataPinDefinition {
        pin_name: "input".into(),
        value_type: "bool".into(),
        is_input: true,
    }];
    let mut knows = node(
        101,
        NodeKind::Knows {
            holder: HolderRef::Acting,
            fact,
            confirmed_only: false,
        },
        NodeCategory::LogicGate,
        &[],
    );
    knows.data_output_pins = vec![DataPinDefinition {
        pin_name: "value".into(),
        value_type: "bool".into(),
        is_input: false,
    }];
    FlowGraphAsset {
        asset_id: "press_gate".into(),
        start_node_guid: 100,
        nodes: vec![
            condition,
            knows,
            node(
                102,
                NodeKind::End {
                    jump_to_storylet: None,
                    precondition_result: true,
                },
                NodeCategory::Mutator,
                &["Out"],
            ),
            node(
                103,
                NodeKind::End {
                    jump_to_storylet: None,
                    precondition_result: false,
                },
                NodeCategory::Mutator,
                &["Out"],
            ),
        ],
        edges: vec![edge(100, "true", 102), edge(100, "false", 103)],
        data_edges: vec![DataEdgeConnection {
            source_node_guid: 101,
            source_pin_name: "value".into(),
            target_node_guid: 100,
            target_pin_name: "input".into(),
        }],
    }
}

fn knows_all_graph(facts: &[u16]) -> FlowGraphAsset {
    let mut graph = knows_graph(facts[0]);
    let mut and = node(200, NodeKind::And, NodeCategory::LogicGate, &[]);
    and.data_input_pins = (0..facts.len())
        .map(|i| DataPinDefinition {
            pin_name: format!("input_{i}"),
            value_type: "bool".into(),
            is_input: true,
        })
        .collect();
    and.data_output_pins = vec![DataPinDefinition {
        pin_name: "value".into(),
        value_type: "bool".into(),
        is_input: false,
    }];
    graph.data_edges.clear();
    for (i, fact) in facts.iter().enumerate() {
        let guid = if i == 0 { 101 } else { 300 + i as u32 };
        if i != 0 {
            let mut knows = node(
                guid,
                NodeKind::Knows {
                    holder: HolderRef::Acting,
                    fact: *fact,
                    confirmed_only: false,
                },
                NodeCategory::LogicGate,
                &[],
            );
            knows.data_output_pins = vec![DataPinDefinition {
                pin_name: "value".into(),
                value_type: "bool".into(),
                is_input: false,
            }];
            graph.nodes.push(knows);
        }
        graph.data_edges.push(DataEdgeConnection {
            source_node_guid: guid,
            source_pin_name: "value".into(),
            target_node_guid: 200,
            target_pin_name: format!("input_{i}"),
        });
    }
    graph.nodes.push(and);
    graph.data_edges.push(DataEdgeConnection {
        source_node_guid: 200,
        source_pin_name: "value".into(),
        target_node_guid: 100,
        target_pin_name: "input".into(),
    });
    graph
}
fn require_unknown(mut graph: FlowGraphAsset, fact: u16) -> FlowGraphAsset {
    let edge_to_success = graph
        .edges
        .iter_mut()
        .find(|edge| edge.target_node_guid == 102)
        .unwrap();
    edge_to_success.target_node_guid = 400;
    let mut condition = node(
        400,
        NodeKind::Condition {
            operator: Default::default(),
            literal: None,
        },
        NodeCategory::LogicGate,
        &["true", "false"],
    );
    condition.data_input_pins = vec![DataPinDefinition {
        pin_name: "input".into(),
        value_type: "bool".into(),
        is_input: true,
    }];
    let mut knows = node(
        401,
        NodeKind::Knows {
            holder: HolderRef::Acting,
            fact,
            confirmed_only: false,
        },
        NodeCategory::LogicGate,
        &[],
    );
    knows.data_output_pins = vec![DataPinDefinition {
        pin_name: "value".into(),
        value_type: "bool".into(),
        is_input: false,
    }];
    graph.nodes.extend([condition, knows]);
    graph
        .edges
        .extend([edge(400, "true", 103), edge(400, "false", 102)]);
    graph.data_edges.push(DataEdgeConnection {
        source_node_guid: 401,
        source_pin_name: "value".into(),
        target_node_guid: 400,
        target_pin_name: "input".into(),
    });
    graph
}
fn require_any(mut graph: FlowGraphAsset, facts: &[u16]) -> FlowGraphAsset {
    let mut any = node(500, NodeKind::Or, NodeCategory::LogicGate, &[]);
    any.data_input_pins = (0..facts.len())
        .map(|i| DataPinDefinition {
            pin_name: format!("input_{i}"),
            value_type: "bool".into(),
            is_input: true,
        })
        .collect();
    any.data_output_pins = vec![DataPinDefinition {
        pin_name: "value".into(),
        value_type: "bool".into(),
        is_input: false,
    }];
    for (i, fact) in facts.iter().enumerate() {
        let guid = 510 + i as u32;
        let mut knows = node(
            guid,
            NodeKind::Knows {
                holder: HolderRef::Acting,
                fact: *fact,
                confirmed_only: false,
            },
            NodeCategory::LogicGate,
            &[],
        );
        knows.data_output_pins = vec![DataPinDefinition {
            pin_name: "value".into(),
            value_type: "bool".into(),
            is_input: false,
        }];
        graph.nodes.push(knows);
        graph.data_edges.push(DataEdgeConnection {
            source_node_guid: guid,
            source_pin_name: "value".into(),
            target_node_guid: 500,
            target_pin_name: format!("input_{i}"),
        });
    }
    graph
        .nodes
        .iter_mut()
        .find(|node| node.node_guid == 200)
        .unwrap()
        .data_input_pins
        .push(DataPinDefinition {
            pin_name: "any".into(),
            value_type: "bool".into(),
            is_input: true,
        });
    graph.nodes.push(any);
    graph.data_edges.push(DataEdgeConnection {
        source_node_guid: 500,
        source_pin_name: "value".into(),
        target_node_guid: 200,
        target_pin_name: "any".into(),
    });
    graph
}

fn require_colocated(
    mut graph: FlowGraphAsset,
    participant: u16,
    expected: bool,
) -> FlowGraphAsset {
    graph
        .edges
        .iter_mut()
        .find(|edge| edge.target_node_guid == 102)
        .unwrap()
        .target_node_guid = 600;
    let mut condition = node(
        600,
        NodeKind::Condition {
            operator: Default::default(),
            literal: None,
        },
        NodeCategory::LogicGate,
        &["true", "false"],
    );
    condition.data_input_pins = vec![DataPinDefinition {
        pin_name: "input".into(),
        value_type: "bool".into(),
        is_input: true,
    }];
    let mut present = node(
        601,
        NodeKind::Colocated { participant },
        NodeCategory::LogicGate,
        &[],
    );
    present.data_output_pins = vec![DataPinDefinition {
        pin_name: "value".into(),
        value_type: "bool".into(),
        is_input: false,
    }];
    graph.nodes.extend([condition, present]);
    graph.edges.extend([
        edge(600, "true", if expected { 102 } else { 103 }),
        edge(600, "false", if expected { 103 } else { 102 }),
    ]);
    graph.data_edges.push(DataEdgeConnection {
        source_node_guid: 601,
        source_pin_name: "value".into(),
        target_node_guid: 600,
        target_pin_name: "input".into(),
    });
    graph
}
/// Full explorer scenario: Mara starts at the inn and Teodor is an NPC.
pub fn explorer_content() -> RunContent {
    build(false, true)
}

/// Variant for the co-op explorer cell, where Teodor takes a human seat.
pub fn content_all_humans() -> RunContent {
    build(true, true)
}

/// Deliberately broken variant used to prove the explorer catches stranding.
pub fn content_without_causeway_close_rule() -> RunContent {
    build(false, false)
}

/// Legacy five-storylet fixture retained for run-kernel regression tests.
///
/// Those tests exercise the original harbor-only scene and its unlimited
/// action sequence. Explorer coverage uses `explorer_content()` instead.
pub fn content() -> RunContent {
    let registry = GameplayTagRegistry::compile(
        [
            "fact.light_doused",
            "fact.logbook",
            "fact.tides",
            "fact.teodor_statement",
            "location.harbor",
        ]
        .into_iter()
        .map(String::from),
    )
    .unwrap();
    let harbor = registry.id("location.harbor").unwrap();
    let fact = registry.id("fact.light_doused").unwrap();
    let mut world = WorldStateTable::with_capacity(2);
    registry
        .add_hierarchical(&mut world.entity_tags[0], harbor)
        .unwrap();
    world
        .participants
        .push("player", u16::MAX, Controller::Human { seat: 0 }, Some(0));
    world
        .participants
        .push("teodor", 1, Controller::Rule, Some(0));
    let keys = [
        "watch_the_water",
        "lamp_soot",
        "ilse_logbook",
        "harbor_master_tides",
        "press_teodor",
    ];
    let titles = [
        "Watch the water",
        "Lamp soot",
        "Ilse's logbook",
        "Harbor master tides",
        "Press Teodor",
    ];
    let facts = [
        "fact.light_doused",
        "fact.logbook",
        "fact.tides",
        "fact.teodor_statement",
    ];
    let mut nodes = vec![
        node(
            1,
            NodeKind::Choice {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
            &["watch", "wait"],
        ),
        node(
            10,
            NodeKind::QbnHub {
                deck: vec![1, 2, 3, 4],
                exit_storylet_id: None,
            },
            NodeCategory::DynamicDispatcher,
            &["Out"],
        ),
    ];
    let mut edges = vec![edge(1, "watch", 10), edge(1, "wait", 10)];
    let mut defs = vec![StoryletDef {
        storylet_id: 0,
        payload_node_guid: 1,
        is_repeatable: true,
        ..Default::default()
    }];
    for i in 1..5u16 {
        let base = 20 + (i as u32 - 1) * 3;
        let mut scene = node(
            base,
            NodeKind::Choice {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
            &["examine", "leave"],
        );
        scene.payload = serde_json::json!({"nodeData":{"type":"choice","choices":[{"text":format!("Examine {}", titles[i as usize])},{"text":"Leave"}]}});
        nodes.push(scene);
        nodes.push(node(
            base + 1,
            NodeKind::Learn {
                fact: registry.id(facts[i as usize - 1]).unwrap(),
                kind: KnowKind::Evidence,
                visibility: Visibility::CoLocated,
            },
            NodeCategory::Mutator,
            &["Out"],
        ));
        nodes.push(node(
            base + 2,
            NodeKind::End {
                jump_to_storylet: None,
                precondition_result: true,
            },
            NodeCategory::Mutator,
            &["Out"],
        ));
        edges.extend([
            edge(base, "examine", base + 1),
            edge(base, "leave", base + 2),
            edge(base + 1, "Out", base + 2),
        ]);
        defs.push(StoryletDef {
            storylet_id: i,
            payload_node_guid: base,
            precondition_graph: if i == PRESS_TEODOR {
                Some(knows_graph(fact))
            } else {
                None
            },
            is_repeatable: true,
            ..Default::default()
        });
    }
    let mut catalog = storylet_engine::chrono_catalog::StoryletCatalog::new();
    for key in keys {
        catalog.insert(key, false, 0, 0.0).unwrap();
    }
    let mut harbor_mask = TagBitset::new();
    harbor_mask.set_bit(harbor).unwrap();
    RunContent {
        registry,
        dispatcher: UnifiedDispatcher::new(defs),
        passion_registry: PassionRegistry::empty(),
        storylet_titles: titles
            .iter()
            .enumerate()
            .map(|(i, title)| (i as u16, (*title).to_string()))
            .collect(),
        entity_names: BTreeMap::from([(0, "Harbor".into()), (1, "Teodor".into())]),
        storylet_location_tags: (0..5).map(|i| (i, harbor_mask.clone())).collect(),
        start_storylet: WATCH_THE_WATER,
        initial_world: world,
        root_asset: FlowGraphAsset {
            asset_id: "weirs-crossing-legacy".into(),
            start_node_guid: 1,
            nodes,
            edges,
            data_edges: vec![],
        },
        catalog,
        ..Default::default()
    }
}

fn build(teodor_is_human: bool, causeway_has_close_rule: bool) -> RunContent {
    let mut tags: Vec<String> = FACTS.iter().map(|s| (*s).into()).collect();
    tags.extend(
        [
            "fact.past.P1",
            "fact.past.P2",
            "fact.past.P3",
            "fact.past.P4",
        ]
        .map(String::from),
    );
    tags.extend(
        [
            "location.lighthouse",
            "location.harbor",
            "location.inn",
            "location.customs",
            "location.rock",
        ]
        .map(String::from),
    );
    let registry = GameplayTagRegistry::compile(tags).unwrap();
    let loc = |name: &str| registry.id(name).unwrap();
    let mut world = WorldStateTable::with_capacity(5);
    for (entity, tag) in [
        (LIGHTHOUSE, "location.lighthouse"),
        (HARBOR, "location.harbor"),
        (INN, "location.inn"),
        (CUSTOMS, "location.customs"),
        (ROCK, "location.rock"),
    ] {
        registry
            .add_hierarchical(&mut world.entity_tags[entity as usize], loc(tag))
            .unwrap();
    }
    world
        .participants
        .push("mara", 10, Controller::Human { seat: 0 }, Some(INN));
    world.participants.push(
        "teodor",
        11,
        if teodor_is_human {
            Controller::Human { seat: 1 }
        } else {
            Controller::Rule
        },
        Some(HARBOR),
    );
    world
        .participants
        .push("benedek", 12, Controller::Rule, Some(CUSTOMS));
    world
        .participants
        .push("rosa", 13, Controller::Rule, Some(INN));
    let clock = Clock {
        slot_labels: [
            "D1 morning",
            "D1 evening",
            "D2 morning",
            "D2 evening",
            "D3 morning",
            "D3 evening",
        ]
        .map(String::from)
        .to_vec(),
        actions_per_slot: 2,
    };
    world.clock = Some(ClockState::new(&clock, 4));

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut defs = Vec::new();
    let mut titles = BTreeMap::new();
    let mut catalog = storylet_engine::chrono_catalog::StoryletCatalog::new();
    let mut locations = BTreeMap::new();
    let loc_mask = |tag: &str| {
        let mut mask = TagBitset::new();
        mask.set_bit(loc(tag)).unwrap();
        mask
    };
    let mut root_nodes = vec![node(
        1,
        NodeKind::Choice {
            default_output_pin: String::new(),
        },
        NodeCategory::LogicGate,
        &["start", "wait"],
    )];
    root_nodes[0].payload = serde_json::json!({"nodeData":{"type":"choice","choices":[{"text":"Begin investigating"},{"text":"Wait"}]}});
    let hub = node(
        10,
        NodeKind::QbnHub {
            deck: (1..=12).collect(),
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
        &["Out"],
    );
    root_nodes.push(hub);
    edges.extend([edge(1, "start", 10), edge(1, "wait", 10)]);
    defs.push(StoryletDef {
        storylet_id: WATCH_THE_WATER,
        payload_node_guid: 1,
        is_repeatable: true,
        ..Default::default()
    });
    catalog.insert(STORYLETS[0].0, false, 0, 0.0).unwrap();
    titles.insert(WATCH_THE_WATER, STORYLETS[0].1.into());
    let fact_for_story = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(3),
        Some(5),
        Some(4),
        Some(4),
        Some(5),
        Some(6),
        Some(7),
        Some(5),
    ];
    let places = [
        "location.harbor",
        "location.lighthouse",
        "location.lighthouse",
        "location.harbor",
        "location.harbor",
        "location.harbor",
        "location.customs",
        "location.inn",
        "location.inn",
        "location.inn",
        "location.customs",
        "location.rock",
        "location.customs",
    ];
    for id in 1..=12u16 {
        let base = 20 + id as u32 * 3;
        let mut scene = node(
            base,
            NodeKind::Choice {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
            &["examine", "leave"],
        );
        scene.payload = serde_json::json!({"nodeData":{"type":"choice","choices":[{"text":format!("Investigate {}", STORYLETS[id as usize].1)},{"text":"Leave"}]}});
        let fact_ix = fact_for_story[id as usize].unwrap_or(0);
        let tag = registry.id(FACTS[fact_ix]).unwrap();
        if id == TEODOR_LANTERN_MEMORY {
            // Rule participants auto-resolve their own storylets and cannot
            // enter a player choice node.
            nodes.push(node(
                base,
                NodeKind::Learn {
                    fact: registry.id("fact.crates_landed").unwrap(),
                    kind: KnowKind::Recollection,
                    visibility: Visibility::CoLocated,
                },
                NodeCategory::Mutator,
                &["Out"],
            ));
            nodes.push(node(
                base + 2,
                NodeKind::End {
                    jump_to_storylet: None,
                    precondition_result: true,
                },
                NodeCategory::Mutator,
                &["Out"],
            ));
            edges.push(edge(base, "Out", base + 2));
        } else {
            nodes.push(scene);
        }
        if id != TEODOR_LANTERN_MEMORY {
            nodes.push(node(
                base + 1,
                NodeKind::Learn {
                    fact: tag,
                    kind: if id == ASK_ROSA_OPENLY || id == LISTEN_QUIETLY {
                        KnowKind::Rumor
                    } else {
                        KnowKind::Evidence
                    },
                    visibility: if id == CONFRONT_BENEDEK || id == FIND_ILSE || id == BLUFF_BENEDEK
                    {
                        Visibility::CoLocated
                    } else {
                        Visibility::Private
                    },
                },
                NodeCategory::Mutator,
                &["Out"],
            ));
            nodes.push(node(
                base + 2,
                NodeKind::End {
                    jump_to_storylet: None,
                    precondition_result: true,
                },
                NodeCategory::Mutator,
                &["Out"],
            ));
            edges.extend([
                edge(base, "examine", base + 1),
                edge(base, "leave", base + 2),
                edge(base + 1, "Out", base + 2),
            ]);
        }
        let precondition_graph = match id {
            PRESS_TEODOR => Some(knows_graph(registry.id("fact.light_doused").unwrap())),
            CUSTOMS_LEDGER => Some(knows_graph(registry.id("fact.crates_landed").unwrap())),
            PIECE_TOGETHER => Some(require_unknown(
                knows_all_graph(
                    &[
                        "fact.benedek_suspected",
                        "fact.light_doused",
                        "fact.crates_landed",
                    ]
                    .map(|name| registry.id(name).unwrap()),
                ),
                registry.id("fact.benedek_identified").unwrap(),
            )),
            CONFRONT_BENEDEK => Some(require_colocated(
                knows_graph(registry.id("fact.benedek_identified").unwrap()),
                BENEDEK,
                true,
            )),
            BLUFF_BENEDEK => Some(require_colocated(
                require_unknown(
                    require_any(
                        knows_all_graph(&[registry.id("fact.benedek_suspected").unwrap()]),
                        &[
                            registry.id("fact.light_doused").unwrap(),
                            registry.id("fact.crates_landed").unwrap(),
                        ],
                    ),
                    registry.id("fact.benedek_identified").unwrap(),
                ),
                BENEDEK,
                true,
            )),
            _ => None,
        };
        let mut def = StoryletDef {
            storylet_id: id,
            payload_node_guid: base,
            precondition_graph,
            is_repeatable: id == WATCH_THE_WATER,
            ..Default::default()
        };
        if id == TEODOR_LANTERN_MEMORY {
            def.played_by = vec![TEODOR];
        }
        defs.push(def);
        let (key, title) = STORYLETS[id as usize];
        catalog.insert(key, false, 0, 0.0).unwrap();
        titles.insert(id, title.into());
        locations.insert(id, loc_mask(places[id as usize]));
    }
    // Four explicit endings. Their authoring identity is present even though
    // this fixture's payloads are intentionally minimal executable scenes.
    for id in ENDING_LIGHT_RESTORED..=ENDING_LOW_WATER {
        let base = 200 + id as u32 * 3;
        nodes.extend([
            node(
                base,
                NodeKind::Choice {
                    default_output_pin: String::new(),
                },
                NodeCategory::LogicGate,
                &["choose"],
            ),
            node(
                base + 1,
                NodeKind::End {
                    jump_to_storylet: None,
                    precondition_result: true,
                },
                NodeCategory::Mutator,
                &["Out"],
            ),
        ]);
        edges.push(edge(base, "choose", base + 1));
        defs.push(StoryletDef {
            storylet_id: id,
            payload_node_guid: base,
            is_ending: true,
            ..Default::default()
        });
        let (key, title) = STORYLETS[id as usize];
        catalog.insert(key, false, 0, 0.0).unwrap();
        titles.insert(id, title.into());
        locations.insert(id, loc_mask("location.harbor"));
    }
    // The immutable past is revealed only by the corresponding investigation scenes.
    use storylet_engine::chrono_chronicle::{ChronicleEntry, ChronicleTable};
    let mut chronicle = ChronicleTable::new(vec![
        ChronicleEntry {
            key: "P1".into(),
            story_time: 2200,
            label: "The lamp is lit".into(),
            location: Some(LIGHTHOUSE),
            facts: vec![registry.id("fact.where_rock").unwrap()],
            text: "Ilse lights the lamp and logs the storm.".into(),
        },
        ChronicleEntry {
            key: "P2".into(),
            story_time: 2300,
            label: "The unlit boat".into(),
            location: Some(HARBOR),
            facts: vec![registry.id("fact.crates_landed").unwrap()],
            text: "An unlit boat lands crates beside a customs lantern.".into(),
        },
        ChronicleEntry {
            key: "P3".into(),
            story_time: 2330,
            label: "The lamp is doused".into(),
            location: Some(LIGHTHOUSE),
            facts: vec![registry.id("fact.benedek_identified").unwrap()],
            text: "The lamp is doused; lamp oil was signed out to B.".into(),
        },
        ChronicleEntry {
            key: "P4".into(),
            story_time: 2430,
            label: "Ilse rows out".into(),
            location: Some(ROCK),
            facts: vec![registry.id("fact.ilse_alive").unwrap()],
            text: "Ilse rows to the outer rock to warn ships away.".into(),
        },
    ]);
    for entry in &mut chronicle.entries {
        entry
            .facts
            .push(registry.id(&format!("fact.past.{}", entry.key)).unwrap());
    }
    for (storylet, entry) in [
        (ILSE_LOGBOOK, 0u16),
        (PRESS_TEODOR, 1),
        (TEODOR_LANTERN_MEMORY, 1),
        (CUSTOMS_LEDGER, 2),
        (FIND_ILSE, 3),
    ] {
        let base = 20 + storylet as u32 * 3;
        let from = if storylet == TEODOR_LANTERN_MEMORY {
            base
        } else {
            base + 1
        };
        let reveal = 500 + u32::from(storylet);
        let outgoing = edges
            .iter_mut()
            .find(|e| e.source_node_guid == from && e.target_node_guid == base + 2)
            .unwrap();
        outgoing.target_node_guid = reveal;
        let facts = chronicle.entries[entry as usize].facts.clone();
        nodes.push(node(
            reveal,
            NodeKind::Reveal {
                entry,
                facts,
                kind: KnowKind::Recollection,
                visibility: Visibility::Private,
            },
            NodeCategory::Mutator,
            &["Out"],
        ));
        edges.push(edge(reveal, "Out", base + 2));
    } // The global catalog starts after the setup storylet, preserving the
    // historic IDs 0–4 used by run_kernel tests.
    let lighthouse = LIGHTHOUSE;
    let harbor = HARBOR;
    let inn = INN;
    let customs = CUSTOMS;
    let rock = ROCK;
    let rock_mask = (1u64 << 1) | (1u64 << 4);
    let mut content = RunContent {
        registry,
        dispatcher: UnifiedDispatcher::new(defs),
        passion_registry: PassionRegistry::empty(),
        storylet_titles: titles,
        entity_names: BTreeMap::from([
            (lighthouse, "Lighthouse".into()),
            (harbor, "Harbor".into()),
            (inn, "Inn".into()),
            (customs, "Customs house".into()),
            (rock, "Outer rock".into()),
            (10, "Mara".into()),
            (11, "Teodor".into()),
            (12, "Benedek".into()),
            (13, "Rosa".into()),
        ]),
        storylet_location_tags: locations,
        start_storylet: WATCH_THE_WATER,
        initial_world: world,
        root_asset: FlowGraphAsset {
            asset_id: "weirs-crossing".into(),
            start_node_guid: 1,
            nodes: root_nodes.into_iter().chain(nodes).collect(),
            edges,
            data_edges: vec![],
        },
        catalog,
        chronicle,
        clock: Some(clock),
        connections: vec![
            Connection {
                from: lighthouse,
                to: harbor,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
            Connection {
                from: harbor,
                to: inn,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
            Connection {
                from: inn,
                to: customs,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
            Connection {
                from: customs,
                to: harbor,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
            Connection {
                from: harbor,
                to: customs,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
            Connection {
                from: lighthouse,
                to: rock,
                open_slots: Some(rock_mask),
                requires_fact: Some(FACTS[2].parse::<u16>().unwrap_or(0)),
                on_close: if causeway_has_close_rule {
                    Some(OnClose::Return { to: lighthouse })
                } else {
                    None
                },
            },
            Connection {
                from: rock,
                to: lighthouse,
                open_slots: None,
                requires_fact: None,
                on_close: None,
            },
        ],
        ..Default::default()
    };
    for (from, to) in [(harbor, lighthouse), (inn, harbor), (customs, inn)] {
        content.connections.push(Connection {
            from,
            to,
            open_slots: None,
            requires_fact: None,
            on_close: None,
        });
    } // The crossing is tide-gated; require the compiled tide fact, resolved
    // after the registry has been moved into RunContent.
    content.connections[5].requires_fact = content.registry.id("fact.tides");
    content
}
