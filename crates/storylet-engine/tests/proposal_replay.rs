#[path = "support/mod.rs"]
mod support;

use storylet_engine::{Authority, RunAction, RunKernel, RunStart};

#[test]
fn transfer_and_tell_replay_to_identical_kernel_and_event_log() {
    let (content, mut run) = support::actions::start();
    let fact = content.registry.id("fact.light_doused").unwrap();
    let proposals = vec![
        (
            Authority::Human { seat: 0 },
            support::actions::proposal(
                &run,
                0,
                RunAction::Transfer {
                    to: 1,
                    resource: 20,
                    amount: 1,
                },
                "transfer",
            ),
        ),
        (
            Authority::Human { seat: 0 },
            storylet_engine::ActionProposal {
                schema_version: 1,
                actor: 0,
                action: RunAction::Tell { to: 1, fact },
                expected_revision: 1,
                idempotency_key: "tell".into(),
                provenance: None,
            },
        ),
    ];
    for (authority, proposal) in &proposals {
        run.propose(&content, *authority, proposal.clone()).unwrap();
    }
    let replay = RunKernel::replay(&content, RunStart::Beginning, 17, &proposals).unwrap();
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
    let told = replay.world().knowledge.knows(1, fact).unwrap();
    assert_eq!(
        told.kind,
        storylet_engine::chrono_participants::KnowKind::Told
    );
    assert_eq!(told.told_by, Some(0));
    assert_eq!(
        replay
            .explain(told.source_event)
            .unwrap()
            .last()
            .unwrap()
            .id,
        told.source_event
    );
}
