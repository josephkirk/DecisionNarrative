use storylet_engine::chrono_participants::{Controller, KnowKind, KnowledgeRow};
use storylet_engine::chrono_state::{AttributeValue, WorldVertex};
use storylet_engine::{ActionProposal, Authority, RunContent, RunKernel, RunStart};

pub fn content_with_inventory() -> RunContent {
    let mut content = super::weirs_crossing::content_all_humans();
    let world = &mut content.initial_world;
    world.participants.controller[1] = Controller::Human { seat: 1 };
    world.participants.location[0] = Some(super::weirs_crossing::INN);
    world.participants.location[1] = Some(super::weirs_crossing::INN);
    world.vertices = vec![
        WorldVertex {
            vertex_id: "actor-a".into(),
            entity_index: Some(world.participants.entity[0]),
            attributes: [
                (
                    "prop.__inventory_items".into(),
                    AttributeValue::Array(vec![AttributeValue::EntityRef("apple".into())]),
                ),
                ("prop.__inventory_columns".into(), AttributeValue::Int(2)),
                ("prop.__inventory_rows".into(), AttributeValue::Int(2)),
            ]
            .into(),
            ..Default::default()
        },
        WorldVertex {
            vertex_id: "actor-b".into(),
            entity_index: Some(world.participants.entity[1]),
            attributes: [
                (
                    "prop.__inventory_items".into(),
                    AttributeValue::Array(Vec::new()),
                ),
                ("prop.__inventory_columns".into(), AttributeValue::Int(2)),
                ("prop.__inventory_rows".into(), AttributeValue::Int(2)),
            ]
            .into(),
            ..Default::default()
        },
        WorldVertex {
            vertex_id: "apple".into(),
            entity_index: Some(20),
            attributes: [("prop.__inventory_item".into(), AttributeValue::Bool(true))].into(),
            ..Default::default()
        },
    ];
    world.vertices.sort_by(|a, b| a.vertex_id.cmp(&b.vertex_id));
    let fact = content.registry.id("fact.light_doused").unwrap();
    world.knowledge.learn(KnowledgeRow {
        holder: 0,
        fact,
        kind: KnowKind::Evidence,
        source_event: 0,
        told_by: None,
    });
    content
}

pub fn start() -> (RunContent, RunKernel) {
    let content = content_with_inventory();
    let run = RunKernel::start(&content, RunStart::Beginning, 17).unwrap();
    (content, run)
}

pub fn proposal(
    run: &RunKernel,
    actor: u16,
    action: storylet_engine::RunAction,
    key: &str,
) -> ActionProposal {
    ActionProposal {
        schema_version: 1,
        actor,
        action,
        expected_revision: run.revision(),
        idempotency_key: key.into(),
        provenance: None,
    }
}

pub fn submit(
    run: &mut RunKernel,
    content: &RunContent,
    actor: u16,
    action: storylet_engine::RunAction,
    key: &str,
) -> Result<storylet_engine::ActionReceipt, storylet_engine::ActionRejection> {
    let seat = match run.world().participants.controller[actor as usize] {
        Controller::Human { seat } => seat,
        Controller::Rule => {
            return Err(storylet_engine::ActionRejection {
                reason: "test actor is rule-controlled".into(),
            });
        }
    };
    run.propose(
        content,
        Authority::Human { seat },
        proposal(run, actor, action, key),
    )
}

