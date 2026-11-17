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
