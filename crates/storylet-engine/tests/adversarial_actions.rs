#[path = "support/mod.rs"]
mod support;

use storylet_engine::{Authority, RunAction};

fn unchanged(run: &storylet_engine::RunKernel, before: &[u8]) {
    assert_eq!(serde_json::to_vec(run).unwrap(), before);
}

#[test]
fn unknown_actor_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 999, RunAction::Wait, "unknown");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn wrong_authority_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Wait, "wrong-seat");
    assert!(
        run.propose(&content, Authority::Human { seat: 8 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn policy_cannot_drive_human_seat() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Wait, "policy-human");
    assert!(run.propose(&content, Authority::Policy, p).is_err());
    unchanged(&run, &before);
}

#[test]
fn stale_revision_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let mut p = support::actions::proposal(&run, 0, RunAction::Wait, "stale");
    p.expected_revision += 1;
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn unavailable_action_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(
        &run,
        0,
        RunAction::Choose {
            node_guid: u32::MAX,
            output_pin: u16::MAX,
        },
        "unavailable",
    );
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn disconnected_move_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Move { to: u16::MAX }, "disconnected");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn overlarge_transfer_rejects_without_mutation() {
    let (content, mut run) = support::actions::start();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(
        &run,
        0,
        RunAction::Transfer {
            to: 1,
            resource: 20,
            amount: 2,
        },
        "overlarge",
    );
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn teller_cannot_share_unknown_fact() {
    let (content, mut run) = support::actions::start();
    let fact = content.registry.id("fact.logbook").unwrap();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Tell { to: 1, fact }, "unknown-fact");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn idempotency_key_cannot_be_reused_for_another_body() {
    let (content, mut run) = support::actions::start();
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "same").unwrap();
    let before = serde_json::to_vec(&run).unwrap();
    let p = support::actions::proposal(&run, 0, RunAction::Move { to: u16::MAX }, "same");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, p)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn late_commit_failure_rolls_back_internal_mutation() {
    let (content, run) = support::actions::start();
    let mut serialized = serde_json::to_value(run).unwrap();
    serialized["revision"] = serde_json::Value::from(u64::MAX);
    let mut run: storylet_engine::RunKernel = serde_json::from_value(serialized).unwrap();
    let before = serde_json::to_vec(&run).unwrap();
    let proposal = support::actions::proposal(&run, 0, RunAction::Wait, "overflow");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, proposal)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn branch_failure_after_rewind_preserves_kernel() {
    let (content, mut run) = support::actions::start();
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "first").unwrap();
    run.rewind(
        Authority::Architect,
        storylet_engine::RewindRequest {
            to_step: 0,
            expected_revision: run.revision(),
            idempotency_key: format!("rewind-{}", run.revision()),
        },
    )
    .unwrap();
    let mut serialized = serde_json::to_value(run).unwrap();
    serialized["revision"] = serde_json::Value::from(u64::MAX);
    let mut run: storylet_engine::RunKernel = serde_json::from_value(serialized).unwrap();
    let before = serde_json::to_vec(&run).unwrap();
    let proposal = support::actions::proposal(&run, 0, RunAction::Wait, "overflow-branch");
    assert!(
        run.propose(&content, Authority::Human { seat: 0 }, proposal)
            .is_err()
    );
    unchanged(&run, &before);
}

#[test]
fn repeated_branch_rewind_restores_log_prefix() {
    let (mut content, mut run) = support::actions::start();
    content.clock = None;
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "first").unwrap();
    run.rewind(
        Authority::Architect,
        storylet_engine::RewindRequest {
            to_step: 0,
            expected_revision: run.revision(),
            idempotency_key: format!("rewind-{}", run.revision()),
        },
    )
    .unwrap();
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "branch-first").unwrap();
    let expected = serde_json::to_value(&run).unwrap();
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "branch-second").unwrap();
    run.rewind(
        Authority::Architect,
        storylet_engine::RewindRequest {
            to_step: 1,
            expected_revision: run.revision(),
            idempotency_key: format!("rewind-{}", run.revision()),
        },
    )
    .unwrap();
    let actual = serde_json::to_value(&run).unwrap();
    assert_eq!(actual["proposals"], expected["proposals"]);
    assert_eq!(actual["event_log"], expected["event_log"]);
}

#[test]
fn rewind_is_idempotent_monotonic_and_replayable() {
    let (mut content, mut run) = support::actions::start();
    content.clock = None;
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "before").unwrap();
    let request = storylet_engine::RewindRequest {
        to_step: 0,
        expected_revision: run.revision(),
        idempotency_key: "rewind".into(),
    };
    let receipt = run.rewind(Authority::Architect, request.clone()).unwrap();
    assert_eq!(receipt.revision, 2);
    assert!(
        run.rewind(Authority::Architect, request.clone())
            .unwrap()
            .duplicate
    );
    assert_eq!(run.revision(), 2);
    let mut different = request.clone();
    different.to_step = 1;
    assert!(run.rewind(Authority::Architect, different).is_err());
    let mut stale = request;
    stale.idempotency_key = "stale".into();
    assert!(run.rewind(Authority::Architect, stale).is_err());
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "after").unwrap();
    let operations = run.operations().to_vec();
    let mut replay =
        storylet_engine::RunKernel::start(&content, storylet_engine::RunStart::Beginning, 17)
            .unwrap();
    for operation in operations {
        replay.apply_operation(&content, operation).unwrap();
    }
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
}

#[test]
fn rewind_can_return_forward_before_forking() {
    let (mut content, mut run) = support::actions::start();
    content.clock = None;
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "first").unwrap();
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "second").unwrap();
    let original = serde_json::to_value(&run).unwrap();
    for target in [0, 2] {
        run.rewind(
            Authority::Architect,
            storylet_engine::RewindRequest {
                to_step: target,
                expected_revision: run.revision(),
                idempotency_key: format!("navigate-{target}"),
            },
        )
        .unwrap();
    }
    let restored = serde_json::to_value(&run).unwrap();
    assert_eq!(restored["proposals"], original["proposals"]);
    assert_eq!(restored["event_log"], original["event_log"]);
    assert_eq!(restored["world"], original["world"]);
    assert_eq!(run.revision(), 4);
    support::actions::submit(&mut run, &content, 0, RunAction::Wait, "third").unwrap();
    let event_id = serde_json::to_value(&run).unwrap()["event_log"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    assert!(
        event_id
            > original["event_log"].as_array().unwrap().last().unwrap()["id"]
                .as_u64()
                .unwrap()
    );
}
