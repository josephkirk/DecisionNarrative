#[path = "support/mod.rs"]
mod support;

use storylet_engine::{Authority, RunAction, RunKernel, RunStart};

fn set_learn_visibility(
    content: &mut storylet_engine::RunContent,
    fact_name: &str,
    visibility: storylet_engine::chrono_runtime::Visibility,
) {
    let fact = content.registry.id(fact_name).unwrap();
    for asset in std::iter::once(&mut content.root_asset).chain(content.mission_assets.values_mut())
    {
        for node in &mut asset.nodes {
            if let storylet_engine::chrono_runtime::NodeKind::Learn {
                fact: learned,
                visibility: current,
                ..
            } = &mut node.kind
            {
                if *learned == fact {
                    *current = visibility;
                }
            }
        }
    }
}

#[test]
fn participants_in_different_places_observe_different_events() {
    let content = support::weirs_crossing::content_all_humans();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 4).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Wait, "private-step");
    run.propose(&content, Authority::Human { seat: 0 }, p)
        .unwrap();
    let a = run.observe(&content, 0);
    let b = run.observe(&content, 1);
    assert_ne!(
        a.place.as_ref().map(|p| p.id),
        b.place.as_ref().map(|p| p.id)
    );
    assert!(a.witnessed_events.contains(&1));
    assert!(!b.witnessed_events.contains(&1));
    assert!(a.scene.is_some());
    assert!(b.scene.is_none());
}

#[test]
fn private_knowledge_is_absent_from_another_observation() {
    let mut content = support::weirs_crossing::content_all_humans();
    content.initial_world.participants.location[0] = Some(support::weirs_crossing::HARBOR);
    content.initial_world.participants.location[1] = Some(support::weirs_crossing::HARBOR);
    content.initial_world.participants.location[2] = Some(support::weirs_crossing::INN);
    content.clock = None;
    content.initial_world.clock = None;
    set_learn_visibility(
        &mut content,
        "fact.tides",
        storylet_engine::chrono_runtime::Visibility::Private,
    );
    let mut run = RunKernel::start(&content, RunStart::Beginning, 17).unwrap();
    run.propose(
        &content,
        Authority::Human { seat: 0 },
        support::actions::proposal(
            &run,
            0,
            RunAction::Choose {
                node_guid: 1,
                output_pin: 0,
            },
            "private-start",
        ),
    )
    .unwrap();
    run.propose(
        &content,
        Authority::Human { seat: 0 },
        support::actions::proposal(
            &run,
            0,
            RunAction::PlayStorylet {
                node_guid: 10,
                storylet: 3,
            },
            "private-storylet",
        ),
    )
    .unwrap();
    let node_guid = run.architect_view(&content, 0).scene.unwrap().node_guid;
    run.propose(
        &content,
        Authority::Human { seat: 0 },
        support::actions::proposal(
            &run,
            0,
            RunAction::Choose {
                node_guid,
                output_pin: 0,
            },
            "private-learn",
        ),
    )
    .unwrap();
    let fact = content.registry.id("fact.tides").unwrap();
    assert!(
        run.observe(&content, 0)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
    assert!(
        !run.observe(&content, 1)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
    assert!(
        !run.observe(&content, 2)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
}

#[test]
fn colocated_learn_appears_only_for_witnesses() {
    let mut content = support::weirs_crossing::content_all_humans();
    content.initial_world.participants.location[0] = Some(support::weirs_crossing::HARBOR);
    content.initial_world.participants.location[1] = Some(support::weirs_crossing::HARBOR);
    content.initial_world.participants.location[2] = Some(support::weirs_crossing::INN);
    content.clock = None;
    content.initial_world.clock = None;
    set_learn_visibility(
        &mut content,
        "fact.tides",
        storylet_engine::chrono_runtime::Visibility::CoLocated,
    );
    let mut run = RunKernel::start(&content, RunStart::Beginning, 17).unwrap();
    for (key, action) in [
        (
            "witness-start",
            RunAction::Choose {
                node_guid: 1,
                output_pin: 0,
            },
        ),
        (
            "witness-storylet",
            RunAction::PlayStorylet {
                node_guid: 10,
                storylet: 3,
            },
        ),
    ] {
        let proposal = support::actions::proposal(&run, 0, action, key);
        run.propose(&content, Authority::Human { seat: 0 }, proposal)
            .unwrap();
    }
    let node_guid = run.architect_view(&content, 0).scene.unwrap().node_guid;
    let proposal = support::actions::proposal(
        &run,
        0,
        RunAction::Choose {
            node_guid,
            output_pin: 0,
        },
        "witness-learn",
    );
    run.propose(&content, Authority::Human { seat: 0 }, proposal)
        .unwrap();
    let fact = content.registry.id("fact.tides").unwrap();
    assert!(
        run.observe(&content, 0)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
    assert!(
        run.observe(&content, 1)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
    assert!(
        !run.observe(&content, 2)
            .knowledge
            .iter()
            .any(|row| row.fact == fact)
    );
}
