//! Engine-type probe for the dense-runtime backlog.
//!
//! Validates two seams that a synthetic benchmark cannot:
//! - `FlowGraphInstance` call history can rebuild completed-storylet bits.
//! - zero-sum passion propagation writes targets not named by the explicit op.
//!
//! This is not production code and does not modify engine tables.
//!
//! Run:
//!   cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_engine_probe

use storylet_engine::chrono_dispatch::{StoryletDef, UnifiedDispatcher};
use storylet_engine::chrono_runtime::{
    EdgeConnection, FlowGraphAsset, FlowGraphInstance, NodeCategory, NodeKind, PinDefinition,
    RuntimeContext,
};
use storylet_engine::chrono_state::{
    apply_passion_mutation, GameplayTagRegistry, MetricBound, MetricRegistry, MicroPassionDef,
    PassionMutation, PassionRegistry, RivalPassionLink, StarNpcDef, TagBitset, WorldStateTable,
};

fn node(guid: u32, kind: NodeKind) -> storylet_engine::chrono_runtime::FlowNodeDef {
    storylet_engine::chrono_runtime::FlowNodeDef {
        node_guid: guid,
        node_type_id: "real-type-proto".into(),
        category: NodeCategory::DynamicDispatcher,
        kind,
        input_pins: vec![PinDefinition {
            pin_name: "In".into(),
            is_input: true,
        }],
        output_pins: vec![PinDefinition {
            pin_name: "Out".into(),
            is_input: false,
        }],
        ..Default::default()
    }
}

fn main() {
    let tags = GameplayTagRegistry::empty();
    let metrics = MetricRegistry::empty();
    let mut dispatcher = UnifiedDispatcher::new(vec![
        StoryletDef {
            storylet_id: 1,
            ..Default::default()
        },
        StoryletDef {
            storylet_id: 2,
            ..Default::default()
        },
        StoryletDef {
            storylet_id: 3,
            ..Default::default()
        },
    ]);
    dispatcher.resolve(&tags, &metrics).unwrap();
    let graph = FlowGraphAsset {
        asset_id: "qbn-real-type".into(),
        start_node_guid: 1,
        nodes: vec![node(
            1,
            NodeKind::QbnHub {
                deck: vec![1, 2, 3],
                exit_storylet_id: Some(3),
            },
        )],
        edges: Vec::<EdgeConnection>::new(),
    };
    let mut state = WorldStateTable::new();
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &tags);
    for id in [1u16, 2, 3] {
        instance
            .force_complete(&mut ctx, 1, "Out", Some(id))
            .unwrap();
    }
    let live: Vec<u16> = instance.completed_storylets.iter_ones().collect();
    let mut rebuilt = 0u64;
    for entry in &instance.global_call_history {
        if let Some(id) = entry.chosen_storylet_id {
            rebuilt |= 1u64 << id;
        }
    }
    let live_bits = live.iter().fold(0u64, |bits, id| bits | (1u64 << *id));
    println!(
        "live_completed {:?} | history_entries {} | rebuilt_bits {} | matches {}",
        live,
        instance.global_call_history.len(),
        rebuilt,
        live_bits == rebuilt
    );

    let npcs = vec![
        StarNpcDef {
            npc_id: 0,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 0,
                passion_id: 1,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: MetricBound {
                min: -100.0,
                max: 100.0,
            },
            zero_sum_links: vec![RivalPassionLink {
                target_character_id: 1,
                source_passion_id: 1,
                target_passion_id: 2,
                weight: -1.0,
            }],
        },
        StarNpcDef {
            npc_id: 1,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 1,
                passion_id: 2,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: MetricBound {
                min: -100.0,
                max: 100.0,
            },
            zero_sum_links: vec![],
        },
    ];
    let passions = PassionRegistry::compile(npcs, std::iter::empty()).unwrap();
    let mut passion_state = WorldStateTable::new();
    passion_state.passions.active[1] = true;
    passion_state.passions.active[2] = true;
    let mut flags = TagBitset::new();
    let delta = apply_passion_mutation(
        &mut passion_state.passions,
        &PassionMutation {
            micro_deltas: vec![(1, 10.0)],
            active_ops: vec![(1, true)],
        },
        &passions,
        &mut flags,
    )
    .unwrap();
    let touched: Vec<u16> = delta.macro_undos.iter().map(|(id, _)| *id).collect();
    println!(
        "explicit_passion_write 1 | actual_macro_writes {:?} | zero_sum_target_seen {}",
        touched,
        touched.contains(&1)
    );
}
