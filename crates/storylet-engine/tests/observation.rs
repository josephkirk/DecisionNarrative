#[path = "support/mod.rs"]
mod support;

use storylet_engine::{Authority, RunAction, RunKernel, RunStart};

#[test]
fn participants_in_different_places_observe_different_events() {
    let mut content = support::weirs_crossing::content_all_humans();
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
    let (mut content, run) = support::actions::start();
    let fact = content.registry.id("fact.light_doused").unwrap();
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
}

#[test]
fn colocated_participant_witnesses_committed_step() {
    let (content, mut run) = support::actions::start();
    let p = support::actions::proposal(&run, 0, RunAction::Wait, "witnessed");
    run.propose(&content, Authority::Human { seat: 0 }, p)
        .unwrap();
    assert!(run.observe(&content, 1).witnessed_events.contains(&1));
}
