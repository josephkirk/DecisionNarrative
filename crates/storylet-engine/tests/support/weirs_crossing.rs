use std::collections::BTreeMap;
use storylet_engine::chrono_dispatch::{StoryletDef, UnifiedDispatcher};
use storylet_engine::chrono_participants::{Controller, KnowKind};
use storylet_engine::chrono_runtime::{
    DataEdgeConnection, DataPinDefinition, EdgeConnection, FlowGraphAsset, FlowNodeDef, HolderRef,
    NodeCategory, NodeKind, PinDefinition, Visibility,
};
use storylet_engine::chrono_state::{
    GameplayTagRegistry, PassionRegistry, TagBitset, WorldStateTable,
};
use storylet_engine::run::RunContent;
pub const WATCH_THE_WATER: u16 = 0;
pub const LAMP_SOOT: u16 = 1;
pub const ILSE_LOGBOOK: u16 = 2;
pub const HARBOR_MASTER_TIDES: u16 = 3;
pub const PRESS_TEODOR: u16 = 4;
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
        scene.payload = serde_json::json!({"nodeData":{"type":"choice","choices":[{"text":format!("Examine {}",titles[i as usize])},{"text":"Leave"}]}});
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
            .map(|(i, s)| (i as u16, (*s).to_string()))
            .collect(),
        entity_names: BTreeMap::from([(0, "Harbor".into()), (1, "Teodor".into())]),
        storylet_location_tags: (0..5).map(|i| (i, harbor_mask)).collect(),
        start_storylet: WATCH_THE_WATER,
        initial_world: world,
        root_asset: FlowGraphAsset {
            asset_id: "weirs-crossing".into(),
            start_node_guid: 1,
            nodes,
            edges,
            data_edges: vec![],
        },
        catalog,
        ..Default::default()
    }
}
