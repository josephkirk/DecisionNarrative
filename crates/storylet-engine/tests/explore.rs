use storylet_engine::chrono_explore::SeededRng;

#[test]
fn seeded_rng_repeats_the_prototype_sequence() {
    let mut rng = SeededRng::new(1);
    assert_eq!(
        [rng.next_u32(), rng.next_u32(), rng.next_u32()],
        [2693262067, 11749833, 2265367787]
    );
}

#[path = "support/weirs_crossing.rs"]
mod weirs_crossing;

#[test]
fn exploration_is_repeatable_and_counts_milestone_routes() {
    use storylet_engine::chrono_explore::{
        ExploreConfig, Milestone, ParticipantConfig, Persona, explore,
    };
    let content = weirs_crossing::content();
    let config = ExploreConfig {
        runs: 12,
        seed: 31,
        personas: vec![Persona::Investigator {
            epsilon_permille: 0,
        }],
        configs: vec![ParticipantConfig::Solo],
        milestones: vec![Milestone {
            key: "logbook".into(),
            name: "Logbook".into(),
            routes: vec![weirs_crossing::ILSE_LOGBOOK],
            reachable_for: vec![ParticipantConfig::Solo],
            deadline_slot: None,
        }],
        max_steps: 12,
        ..Default::default()
    };
    let report = explore(&content, &config);
    assert_eq!(report, explore(&content, &config));
    assert_eq!(report.determinism_mismatches, 0);
    assert!(report.cells[0].milestone_reached[0].1 > 0);
}

#[test]
fn exploration_reports_start_failures_instead_of_silent_empty_coverage() {
    use storylet_engine::chrono_explore::{ExploreConfig, ParticipantConfig, Persona, explore};
    let config = ExploreConfig {
        runs: 1,
        personas: vec![Persona::Wanderer],
        configs: vec![ParticipantConfig::Solo],
        ..Default::default()
    };
    let report = explore(&Default::default(), &config);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].seed, 0);
}

#[test]
fn expanded_fixture_exploration_has_no_errors_and_replays() {
    use storylet_engine::chrono_explore::{ExploreConfig, explore};
    let config = ExploreConfig {
        runs: 10,
        max_steps: 30,
        ..Default::default()
    };
    let report = explore(&weirs_crossing::content_all_humans(), &config);
    assert_eq!(report.error_count, 0, "{:?}", report.errors);
    assert_eq!(report.determinism_mismatches, 0);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

#[test]
fn milestone_requires_completed_route_not_just_selecting_a_storylet() {
    use storylet_engine::chrono_explore::{
        ExploreConfig, Milestone, ParticipantConfig, Persona, explore,
    };
    let config = ExploreConfig {
        runs: 1,
        seed: 1,
        personas: vec![Persona::Investigator {
            epsilon_permille: 0,
        }],
        configs: vec![ParticipantConfig::Solo],
        milestones: vec![Milestone {
            key: "logbook".into(),
            name: "Logbook".into(),
            routes: vec![weirs_crossing::ILSE_LOGBOOK],
            reachable_for: vec![ParticipantConfig::Solo],
            deadline_slot: None,
        }],
        max_steps: 2,
        ..Default::default()
    };
    let report = explore(&weirs_crossing::content(), &config);
    assert_eq!(report.error_count, 0);
    assert_eq!(report.cells[0].milestone_reached[0].1, 0);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn release_explorer_2000_runs_finishes_within_budget() {
    use storylet_engine::chrono_explore::{ExploreConfig, ParticipantConfig, Persona, explore};
    let content = weirs_crossing::content_all_humans();
    let config = ExploreConfig {
        runs: 2000,
        max_steps: 500,
        personas: vec![Persona::Investigator {
            epsilon_permille: 150,
        }],
        configs: vec![ParticipantConfig::AllHumans],
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let report = explore(&content, &config);
    assert_eq!(report.error_count, 0, "{:?}", report.errors);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "2000 runs took {:?}",
        started.elapsed()
    );
}

#[test]
fn parallel_exploration_matches_disjoint_serial_batches() {
    use storylet_engine::chrono_explore::{ExploreConfig, ParticipantConfig, Persona, explore};
    let content = weirs_crossing::content();
    let mut config = ExploreConfig {
        runs: 256,
        max_steps: 4,
        personas: vec![Persona::Wanderer],
        configs: vec![ParticipantConfig::Solo],
        ..Default::default()
    };
    let all = explore(&content, &config);
    config.runs = 128;
    let first = explore(&content, &config);
    config.seed += 128;
    let second = explore(&content, &config);
    assert_eq!(all.error_count, first.error_count + second.error_count);
    assert_eq!(
        all.determinism_mismatches,
        first.determinism_mismatches + second.determinism_mismatches
    );
    assert_eq!(
        all.cells[0]
            .dead_air_by_location
            .iter()
            .map(|(_, _, total)| total)
            .sum::<u32>(),
        first.cells[0]
            .dead_air_by_location
            .iter()
            .chain(second.cells[0].dead_air_by_location.iter())
            .map(|(_, _, total)| total)
            .sum::<u32>()
    );
    assert_eq!(
        all,
        explore(
            &content,
            &ExploreConfig {
                runs: 256,
                max_steps: 4,
                personas: vec![Persona::Wanderer],
                configs: vec![ParticipantConfig::Solo],
                ..Default::default()
            }
        )
    );
}

#[test]
fn coverage_report_json_round_trips_public_results() {
    use storylet_engine::chrono_explore::{ExploreConfig, ExploreReport, explore};
    let report = explore(
        &weirs_crossing::content_all_humans(),
        &ExploreConfig {
            runs: 20,
            max_steps: 30,
            ..Default::default()
        },
    );
    assert!(report.cells.iter().any(|cell| cell.telling_orders > 0));
    let decoded: ExploreReport =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(report, decoded);
}

#[test]
fn probe_colocation_uses_authored_entity_identity() {
    use storylet_engine::chrono_explore::{
        ExploreConfig, IntentProbe, ParticipantConfig, Persona, explore,
    };
    let config = ExploreConfig {
        runs: 1,
        max_steps: 2,
        personas: vec![Persona::Investigator {
            epsilon_permille: 0,
        }],
        configs: vec![ParticipantConfig::Solo],
        probes: vec![IntentProbe {
            name: "with_rosa".into(),
            holder_knows: vec![],
            colocated_with: Some(13),
            expected_routes: vec![],
        }],
        ..Default::default()
    };
    let report = explore(&weirs_crossing::explorer_content(), &config);
    assert_eq!(report.cells[0].probe_hits[0].1, 2);
}

#[test]
fn dead_air_counts_a_hub_that_offers_only_fallback_storylets() {
    use storylet_engine::chrono_explore::{ExploreConfig, ParticipantConfig, Persona, explore};
    let mut content = weirs_crossing::content();
    for storylet in &mut content.dispatcher.storylets {
        storylet.is_fallback = true;
    }
    let config = ExploreConfig {
        runs: 1,
        seed: 1,
        max_steps: 2,
        personas: vec![Persona::Investigator {
            epsilon_permille: 0,
        }],
        configs: vec![ParticipantConfig::Solo],
        ..Default::default()
    };
    let report = explore(&content, &config);
    assert_eq!(report.cells[0].dead_air_by_location[0].1, 1);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn stranding_detected_without_on_close_replays_the_recorded_actions() {
    use storylet_engine::chrono_explore::{
        ExploreConfig, Invariant, ParticipantConfig, Persona, explore,
    };
    use storylet_engine::run::{RunKernel, RunStart};
    let content = weirs_crossing::content_without_causeway_close_rule();
    let config = ExploreConfig {
        runs: 2000,
        max_steps: 40,
        personas: vec![Persona::Wanderer],
        configs: vec![ParticipantConfig::Solo],
        ..Default::default()
    };
    let report = explore(&content, &config);
    let violation = report
        .violations
        .iter()
        .find(|v| matches!(v.invariant, Invariant::OccupantBeyondClosedEdge))
        .expect("expected a stranding trace");
    assert!(!violation.actions.is_empty());
    let (replayed, divergence) = RunKernel::replay(
        &content,
        RunStart::Beginning,
        violation.seed,
        &violation.actions,
    );
    assert_eq!(divergence, None);
    assert!(
        replayed
            .unwrap()
            .world
            .participants
            .location
            .iter()
            .any(|place| *place == Some(weirs_crossing::ROCK))
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn weirs_crossing_has_no_invariant_violations() {
    use storylet_engine::chrono_explore::{ExploreConfig, ParticipantConfig, Persona, explore};
    let config = ExploreConfig {
        runs: 2000,
        max_steps: 40,
        personas: vec![Persona::Wanderer],
        configs: vec![ParticipantConfig::Solo],
        ..Default::default()
    };
    let report = explore(&weirs_crossing::explorer_content(), &config);
    assert_eq!(report.error_count, 0, "{:?}", report.errors);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}
