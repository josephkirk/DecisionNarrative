use std::collections::BTreeMap;

use serde_json::json;
use storylet_engine::{
    CharacterDefinition, LocationDefinition, ProjectBundle, QualityDefinition, QualityType,
    ScenarioDefinition, StoryletDefinition, build_world, validate_project,
};

#[test]
fn a_new_project_starts_empty_and_builds_a_generic_scenario_world() {
    let mut project = ProjectBundle::empty("glass-town", "Glass Town");
    project.qualities.insert(
        "stock.meals".into(),
        QualityDefinition {
            key: "stock.meals".into(),
            kind: QualityType::Currency,
            default_value: json!(8),
            description: "Meals in the communal pantry".into(),
        },
    );
    project.locations.insert(
        "market".into(),
        LocationDefinition {
            id: "market".into(),
            name: "Covered Market".into(),
            description: "A narrow hall of canvas stalls.".into(),
            connected_locations: vec![],
            map_position: (2.0, 3.0),
        },
    );
    project.locations.insert(
        "archive".into(),
        LocationDefinition {
            id: "archive".into(),
            name: "Night Archive".into(),
            description: "A municipal archive.".into(),
            connected_locations: vec!["market".into()],
            map_position: (4.0, 3.0),
        },
    );
    project.characters.insert(
        "nora".into(),
        CharacterDefinition {
            id: "nora".into(),
            name: "Nora Venn".into(),
            description: "A clerk balancing two ledgers.".into(),
            role_tags: vec!["clerk".into()],
            home_location: "archive".into(),
            goals: vec!["protect the audit copy".into()],
            red_lines: vec!["never alter a signed receipt".into()],
            voice_profile: "Short clauses; names objects directly.".into(),
            schedule: BTreeMap::new(),
        },
    );
    project.scenarios.insert(
        "opening".into(),
        ScenarioDefinition {
            id: "opening".into(),
            name: "A Missing Receipt".into(),
            player_start_location: "market".into(),
            initial_qualities: BTreeMap::new(),
            actor_locations: BTreeMap::from([("nora".into(), "archive".into())]),
            storylet_ids: vec![],
            seed: 42,
        event_ids: vec![],
            active_actor_ids: None,
        },
    );

    let report = validate_project(&project);
    assert!(report.is_empty(), "{report:?}");
    let state = build_world(&project, "opening").unwrap();
    assert_eq!(state.current_location.as_deref(), Some("market"));
    assert_eq!(state.qualities.get("stock.meals"), Some(&json!(8)));
    assert!(state.actors["nora"].role_tags.contains("clerk"));
}

#[test]
fn project_validation_catches_dangling_references_and_unapproved_runtime_content() {
    let mut project = ProjectBundle::empty("town", "Town");
    project.locations.insert(
        "market".into(),
        LocationDefinition {
            id: "market".into(),
            name: "Market".into(),
            description: "".into(),
            connected_locations: vec!["missing".into()],
            map_position: (0.0, 0.0),
        },
    );
    project.scenarios.insert(
        "first".into(),
        ScenarioDefinition {
            id: "first".into(),
            name: "First".into(),
            player_start_location: "nowhere".into(),
            initial_qualities: BTreeMap::new(),
            actor_locations: BTreeMap::new(),
            storylet_ids: vec!["draft".into()],
            seed: 9,
        event_ids: vec![],
            active_actor_ids: None,
        },
    );
    let mut draft = StoryletDefinition {
        id: "draft".into(),
        revision: 1,
        title: "Draft".into(),
        summary: "".into(),
        approved: false,
        ending: false,
        location_id: Some("missing".into()),
        urgency_weight: 50.0,
        cluster_type: storylet_engine::ClusterType::Linear,
        repeatability: storylet_engine::Repeatability::Always,
        required_roles: vec![],
        trigger: Default::default(),
        content: storylet_engine::SceneContent { opening: "".into() },
        choices: vec![],
    };
    draft.approved = false;
    project.storylets.insert("draft".into(), draft);

    let issues = validate_project(&project);
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == "locations.market.connected_locations[0]")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == "scenarios.first.player_start_location")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == "storylets.draft.location_id")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == "scenarios.first.storylet_ids[0]"
                && issue.code == "storylet_not_approved")
    );
}

#[test]
fn bundle_json_round_trips_with_unknown_version_reported() {
    let mut project = ProjectBundle::empty("plain", "Plain");
    project.schema_version = 700;
    let serialized = serde_json::to_string(&project).unwrap();
    let decoded: ProjectBundle = serde_json::from_str(&serialized).unwrap();
    assert_eq!(decoded, project);
    assert!(
        validate_project(&decoded)
            .iter()
            .any(|issue| issue.code == "unsupported_schema_version")
    );
}
