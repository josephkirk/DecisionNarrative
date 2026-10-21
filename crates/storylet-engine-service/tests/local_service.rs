use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::time::Duration;
use tokio_stream::StreamExt;
use tower::ServiceExt;

use storylet_engine_service::service::{RuntimeService, router};

fn test_app() -> Router {
    router(RuntimeService::from_connection(Connection::open_in_memory().unwrap()).unwrap())
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    let body = match body {
        Some(body) => {
            builder = builder.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                panic!(
                    "Failed to parse JSON for {} {}: status={}, bytes='{}', err={}",
                    method,
                    path,
                    status,
                    String::from_utf8_lossy(&bytes),
                    e
                );
            }
        }
    };
    (status, body)
}

fn empty_project() -> Value {
    json!({
        "schema_version": 1,
        "id": "station-mystery",
        "name": "Station Mystery",
        "revision": 1,
        "qualities": {},
        "locations": { "platform": { "id": "platform", "name": "Platform", "description": "", "connected_locations": [], "map_position": [0.0, 0.0] } },
        "characters": {},
        "events": {},
        "storylets": {},
        "npc_behaviors": {},
        "scenarios": { "opening": { "id": "opening", "name": "Opening", "player_start_location": "platform", "initial_qualities": {}, "actor_locations": {}, "storylet_ids": [], "seed": 19 } }
    })
}

fn bundle_with_storylet(graph: Value, ending: bool) -> Value {
    json!({
        "format": "decision-narrative-project",
        "schema_version": 2,
        "project": { "id": "bundle-project", "name": "Bundle Project", "revision": 1 },
        "content": {
            "qualities": [],
            "locations": [],
            "characters": [],
            "storylets": [{
                "id": "bundle-story",
                "title": "Bundle Story",
                "ending": ending,
                "content": { "text_template": "" },
                "effects": {},
                "created_at": 1,
                "graph": graph
            }],
            "npcBehaviors": [],
            "events": [],
            "scenarios": []
        }
    })
}

#[tokio::test]
async fn rust_http_service_creates_and_advances_a_generic_playtest() {
    let app = test_app();
    let (status, health) = request(&app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");

    let (status, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "run-1", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["snapshot"]["world"]["current_location"], "platform");

    let command = json!({
        "idempotency_key": "wait-1", "expected_revision": 0, "actor_id": "player",
        "action": "wait", "effects": [], "advance_turn": true
    });
    let (status, result) = request(
        &app,
        "POST",
        "/api/v1/playtests/run-1/commands",
        Some(command.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["world"]["turn"], 1);
    let (status, duplicate) = request(
        &app,
        "POST",
        "/api/v1/playtests/run-1/commands",
        Some(command),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["world"]["turn"], 1);
    assert_eq!(duplicate["player"]["duplicate"], true);

    let (status, events) = request(&app, "GET", "/api/v1/playtests/run-1/events", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events["events"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn playtest_stream_sends_current_snapshot_then_committed_session_changes() {
    let app = test_app();
    let (status, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "stream-run", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/playtests/stream-run/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut stream = response.into_body().into_data_stream();
    let initial = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let initial = String::from_utf8(initial.to_vec()).unwrap();
    assert!(initial.contains("event: playtest.snapshot"), "{initial}");
    assert!(initial.contains("\"revision\":0"), "{initial}");

    let (status, other_created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "other-stream-run", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{other_created}");
    let (status, other_update) = request(
        &app,
        "POST",
        "/api/v1/playtests/other-stream-run/commands",
        Some(json!({
            "idempotency_key": "other-stream-wait", "expected_revision": 0,
            "actor_id": "player", "action": "wait", "effects": [], "advance_turn": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other_update}");

    let (status, updated) = request(
        &app,
        "POST",
        "/api/v1/playtests/stream-run/commands",
        Some(json!({
            "idempotency_key": "stream-wait", "expected_revision": 0,
            "actor_id": "player", "action": "wait", "effects": [], "advance_turn": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let change = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let change = String::from_utf8(change.to_vec()).unwrap();
    assert!(change.contains("event: playtest.updated"), "{change}");
    assert!(change.contains("\"revision\":1"), "{change}");
    assert!(change.contains("\"turn\":1"), "{change}");

    let (status, duplicate) = request(
        &app,
        "POST",
        "/api/v1/playtests/stream-run/commands",
        Some(json!({
            "idempotency_key": "stream-wait", "expected_revision": 0,
            "actor_id": "player", "action": "wait", "effects": [], "advance_turn": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["player"]["duplicate"], true);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), stream.next())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rust_http_service_rejects_invalid_or_stale_commands_without_mutation() {
    let app = test_app();
    let (_, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "run-2", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(created["snapshot"]["world"]["turn"], 0);

    let (status, error) = request(&app, "POST", "/api/v1/playtests/run-2/commands", Some(json!({
        "idempotency_key": "stale", "expected_revision": 3, "actor_id": "player", "action": "wait", "effects": []
    }))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    let (status, snapshot) = request(&app, "GET", "/api/v1/playtests/run-2", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["world"]["revision"], 0);
    assert_eq!(snapshot["world"]["turn"], 0);
}

#[tokio::test]
async fn studio_interaction_context_can_be_published_and_queried() {
    let app = test_app();
    let context_payload = json!({
        "capturedAt": 1700000000000_i64,
        "route": {
            "name": "storylets",
            "path": "/storylets",
            "fullPath": "/storylets"
        },
        "project": {
            "id": "workspace-project",
            "name": "Workspace",
            "revision": 0
        },
        "entity": {
            "type": "storylet",
            "id": "stl_opening",
            "label": "Opening Scene"
        }
    });

    let (status, post_resp) = request(&app, "POST", "/api/v2/context", Some(context_payload)).await;
    assert_eq!(status, StatusCode::OK, "{post_resp}");
    assert_eq!(post_resp["ok"], true);

    let (status, get_resp) = request(&app, "GET", "/api/v2/context", None).await;
    assert_eq!(status, StatusCode::OK, "{get_resp}");
    assert_eq!(get_resp["hasContext"], true);
    assert_eq!(get_resp["context"]["entity"]["id"], "stl_opening");
    assert_eq!(get_resp["liveProjectRevision"], 0);
    assert_eq!(get_resp["isStale"], false);
}

#[tokio::test]
async fn director_evaluates_playtest_candidates_deterministically() {
    let app = test_app();
    let (status, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "director-run", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, result) = request(
        &app,
        "GET",
        "/api/v1/playtests/director-run/director?targetTensionSlope=0.15",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result.get("candidates").is_some());
}

#[tokio::test]
async fn failed_sqlite_commit_rolls_back_the_live_runtime_state() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE runtime_sessions (session_id TEXT PRIMARY KEY, project_id TEXT NOT NULL, revision TEXT NOT NULL, turn TEXT NOT NULL, state_json TEXT NOT NULL, updated_at TEXT NOT NULL); CREATE TRIGGER reject_snapshot_update BEFORE UPDATE ON runtime_sessions BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;").unwrap();
    let service = RuntimeService::from_connection(connection).unwrap();
    let app = router(service);
    let (_, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "run-3", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(created["snapshot"]["world"]["turn"], 0);

    let (status, error) = request(&app, "POST", "/api/v1/playtests/run-3/commands", Some(json!({
        "idempotency_key": "wait", "expected_revision": 0, "actor_id": "player", "action": "wait", "effects": []
    }))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{error}");
    let (status, snapshot) = request(&app, "GET", "/api/v1/playtests/run-3", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["world"]["revision"], 0);
    assert_eq!(snapshot["world"]["turn"], 0);
}

#[tokio::test]
async fn runtime_sessions_recover_from_shared_in_memory_sqlite_after_service_restart() {
    let uri = "file:storylet_service_restart?mode=memory&cache=shared";
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    let _database_anchor = Connection::open_with_flags(uri, flags).unwrap();
    let service =
        RuntimeService::from_connection(Connection::open_with_flags(uri, flags).unwrap()).unwrap();
    let app = router(service.clone());
    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "persisted-run", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/playtests/persisted-run/commands",
        Some(json!({
            "idempotency_key": "first-wait", "expected_revision": 0, "actor_id": "player",
            "action": "wait", "effects": [], "advance_turn": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    drop(app);
    drop(service);
    let restarted = router(
        RuntimeService::from_connection(Connection::open_with_flags(uri, flags).unwrap()).unwrap(),
    );
    let (status, snapshot) =
        request(&restarted, "GET", "/api/v1/playtests/persisted-run", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["world"]["turn"], 1);
    let (status, duplicate) = request(
        &restarted,
        "POST",
        "/api/v1/playtests/persisted-run/commands",
        Some(json!({
            "idempotency_key": "first-wait", "expected_revision": 0, "actor_id": "player",
            "action": "wait", "effects": [], "advance_turn": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["player"]["duplicate"], true);
    assert_eq!(duplicate["world"]["turn"], 1);
}

#[tokio::test]
async fn concurrent_commands_cannot_commit_from_the_same_revision() {
    let app = test_app();
    let _ = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({ "sessionId": "race", "scenarioId": "opening", "project": empty_project() })),
    )
    .await;
    let command = |key: &str| {
        json!({
            "command": {
                "idempotency_key": key, "expected_revision": 0, "actor_id": "player",
                "action": key, "effects": [], "advance_turn": true
            }
        })
    };
    let (first, second) = tokio::join!(
        request(
            &app,
            "POST",
            "/api/v1/playtests/race/commands",
            Some(command("first"))
        ),
        request(
            &app,
            "POST",
            "/api/v1/playtests/race/commands",
            Some(command("second"))
        ),
    );
    let statuses = [first.0, second.0];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));
    let (_, snapshot) = request(&app, "GET", "/api/v1/playtests/race", None).await;
    assert_eq!(snapshot["world"]["revision"], 1);
    assert_eq!(snapshot["world"]["events"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn reusing_a_deleted_session_id_is_rejected_when_archive_events_exist() {
    let app = test_app();
    let create =
        || json!({ "sessionId": "archived", "scenarioId": "opening", "project": empty_project() });
    let (status, _) = request(&app, "POST", "/api/v1/playtests", Some(create())).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = request(&app, "POST", "/api/v1/playtests/archived/commands", Some(json!({
        "command": { "idempotency_key": "same-event-id", "expected_revision": 0, "actor_id": "player", "action": "wait", "effects": [], "advance_turn": true }
    }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        request(&app, "DELETE", "/api/v1/playtests/archived", None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, _) = request(&app, "POST", "/api/v1/playtests", Some(create())).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        request(&app, "GET", "/api/v1/playtests/archived", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn service_startup_rejects_corrupt_saved_session_state() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE runtime_sessions (session_id TEXT PRIMARY KEY, project_id TEXT NOT NULL, revision TEXT NOT NULL, turn TEXT NOT NULL, state_json TEXT NOT NULL, updated_at TEXT NOT NULL); INSERT INTO runtime_sessions VALUES ('broken', 'project', '0', '0', 'not-json', '0');").unwrap();
    let error = RuntimeService::from_connection(connection)
        .err()
        .expect("corrupt persisted session must fail startup");
    assert!(error.contains("broken"));
}

#[tokio::test]
async fn service_startup_migrates_legacy_runtime_session_indexes() {
    let uri = "file:legacy_runtime_migration?mode=memory&cache=shared";
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    let anchor = Connection::open_with_flags(uri, flags).unwrap();
    anchor
        .execute_batch(
            "CREATE TABLE runtime_sessions (
            session_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            revision TEXT NOT NULL,
            turn TEXT NOT NULL,
            state_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
        )
        .unwrap();

    let project =
        serde_json::from_value::<storylet_engine::ProjectBundle>(empty_project()).unwrap();
    let mut runtime = storylet_engine::StoryletRuntime::default();
    runtime
        .create_session("legacy", project, "opening")
        .unwrap();
    let saved = runtime.export_session("legacy").unwrap();
    let mut legacy = serde_json::to_value(saved).unwrap();
    legacy["format_version"] = json!(1);
    let world = legacy["world"].as_object_mut().unwrap();
    world.remove("location_tags");
    world.remove("visited_locations");
    world.remove("met_characters");
    let state_json = serde_json::to_string(&legacy).unwrap();
    anchor
        .execute(
            "INSERT INTO runtime_sessions VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["legacy", "station-mystery", "0", "0", state_json, "0"],
        )
        .unwrap();

    let service =
        RuntimeService::from_connection(Connection::open_with_flags(uri, flags).unwrap()).unwrap();
    let app = router(service.clone());
    let (status, snapshot) = request(&app, "GET", "/api/v1/playtests/legacy", None).await;
    assert_eq!(status, StatusCode::OK, "{snapshot}");
    assert_eq!(snapshot["world"]["visited_locations"], json!(["platform"]));
    assert!(snapshot["world"]["location_tags"].get("platform").is_some());

    let migrated_json: String = anchor
        .query_row(
            "SELECT state_json FROM runtime_sessions WHERE session_id = 'legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let migrated: Value = serde_json::from_str(&migrated_json).unwrap();
    assert_eq!(migrated["format_version"], json!(2));
    assert_eq!(migrated["world"]["visited_locations"], json!(["platform"]));
    assert!(migrated["world"]["location_tags"].get("platform").is_some());
}

#[tokio::test]
async fn service_startup_rejects_event_journal_that_disagrees_with_snapshot() {
    let uri = "file:storylet_event_mismatch?mode=memory&cache=shared";
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    let anchor = Connection::open_with_flags(uri, flags).unwrap();
    let service =
        RuntimeService::from_connection(Connection::open_with_flags(uri, flags).unwrap()).unwrap();
    let app = router(service.clone());
    let _ = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "tampered", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    let _ = request(&app, "POST", "/api/v1/playtests/tampered/commands", Some(json!({
        "command": { "idempotency_key": "event-one", "expected_revision": 0, "actor_id": "player", "action": "wait", "effects": [], "advance_turn": true }
    }))).await;
    drop(app);
    drop(service);
    anchor
        .execute(
            "UPDATE runtime_events SET event_json = '{}' WHERE session_id = 'tampered'",
            [],
        )
        .unwrap();

    assert!(
        RuntimeService::from_connection(Connection::open_with_flags(uri, flags).unwrap()).is_err()
    );
}

#[tokio::test]
async fn studio_graph_is_validated_when_saved() {
    let app = test_app();
    let graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 10, "y": 20, "width": 160, "height": 80, "text": "Begin" },
            { "id": "dialog", "title": "Dialog", "type": "dialog", "x": 200, "y": 20, "width": 280, "height": 160, "text": "The scene", "sections": [] },
            { "id": "end", "title": "End", "type": "end", "x": 520, "y": 20, "width": 160, "height": 90, "is_terminal": false }
        ],
        "edges": [
            { "id": "start-dialog", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" },
            { "id": "dialog-end", "from_node": "dialog", "from_port": "output", "to_node": "end", "to_port": "input" }
        ],
        "local_variables": []
    });
    let (status, saved) = request(&app, "POST", "/api/v2/content/storylets", Some(json!({
        "id": "valid", "title": "valid", "created_at": 1, "content": { "text_template": "" }, "graph": graph
    }))).await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    let (status, result) = request(
        &app,
        "POST",
        "/api/v2/storylets/valid/approve",
        Some(json!({ "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");

    let mut invalid_graph = graph;
    invalid_graph["edges"] = json!([]);
    let (status, result) = request(&app, "POST", "/api/v2/content/storylets", Some(json!({
        "id": "broken", "title": "broken", "created_at": 1, "content": { "text_template": "" }, "graph": invalid_graph
    }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("path to an End node")
    );
}

#[tokio::test]
async fn storylet_with_multiple_end_nodes_saves_and_approves() {
    let app = test_app();
    let graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 160, "height": 80, "text": "Begin" },
            { "id": "choice", "title": "Choice", "type": "choice", "x": 180, "y": 0, "width": 220, "height": 140,
              "choices": [{ "id": "stay", "text": "Stay" }, { "id": "go", "text": "Go" }] },
            { "id": "end-stay", "title": "Stay End", "type": "end", "x": 440, "y": 0, "width": 160, "height": 80, "is_terminal": true },
            { "id": "end-go", "title": "Go End", "type": "end", "x": 440, "y": 120, "width": 160, "height": 80, "jump_to_storylet": "next" }
        ],
        "edges": [
            { "id": "start-choice", "from_node": "start", "from_port": "output", "to_node": "choice", "to_port": "input" },
            { "id": "choice-stay", "from_node": "choice", "from_port": "choice_0", "to_node": "end-stay", "to_port": "input" },
            { "id": "choice-go", "from_node": "choice", "from_port": "choice_1", "to_node": "end-go", "to_port": "input" }
        ],
        "local_variables": []
    });
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "multi-end", "title": "multi-end", "created_at": 1, "ending": true,
            "content": { "text_template": "" }, "graph": graph
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["ending"], false,
        "mixed exits are not a dedicated ending"
    );
    let (status, result) = request(
        &app,
        "POST",
        "/api/v2/storylets/multi-end/approve",
        Some(json!({ "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");

    let terminal_graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 160, "height": 80, "text": "Begin" },
            { "id": "end", "title": "End", "type": "end", "x": 440, "y": 0, "width": 160, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-end", "from_node": "start", "from_port": "output", "to_node": "end", "to_port": "input" }
        ],
        "local_variables": []
    });
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "terminal-end", "title": "terminal-end", "created_at": 1, "ending": false,
            "content": { "text_template": "" }, "graph": terminal_graph
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["ending"], true,
        "all-terminal exits are a dedicated ending"
    );
}

#[tokio::test]
async fn image_nodes_save_and_approve_with_manual_urls() {
    let app = test_app();
    let graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 160, "height": 80, "text": "Begin" },
            { "id": "image", "title": "Image", "type": "image", "x": 180, "y": 0, "width": 220, "height": 140, "image_url": "assets/platform.png" },
            { "id": "end", "title": "End", "type": "end", "x": 420, "y": 0, "width": 160, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-image", "from_node": "start", "from_port": "output", "to_node": "image", "to_port": "input" },
            { "id": "image-end", "from_node": "image", "from_port": "output", "to_node": "end", "to_port": "input" }
        ],
        "local_variables": []
    });
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "image-storylet",
            "title": "Image Scene",
            "created_at": 1,
            "content": { "text_template": "" },
            "graph": graph
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["graph"]["nodes"][1]["image_url"],
        "assets/platform.png"
    );
    assert!(saved["graph"]["nodes"][1].get("prompt").is_none());
    assert!(saved["graph"]["nodes"][1].get("status").is_none());

    let (status, approved) = request(
        &app,
        "POST",
        "/api/v2/storylets/image-storylet/approve",
        Some(json!({ "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(
        approved["graph"]["nodes"][1]["image_url"],
        "assets/platform.png"
    );
}

#[tokio::test]
async fn mcp_narrative_platform_advertises_and_dispatches_every_operation() {
    let service = RuntimeService::from_connection(Connection::open_in_memory().unwrap()).unwrap();

    // Get tool definitions via tools/list
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });
    let list_resp = storylet_engine_service::mcp::handle_mcp_message(&service, &list_req)
        .await
        .expect("tools/list response");

    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    let platform_tool = tools
        .iter()
        .find(|t| t["name"] == "narrative_platform")
        .expect("narrative_platform tool");
    let input_schema = &platform_tool["inputSchema"];
    for name in [
        "entity",
        "id",
        "payload",
        "bundle",
        "confirmReplace",
        "expectedRevision",
        "startLocationId",
        "scenarioId",
        "command",
        "choiceId",
        "storyletId",
        "idempotencyKey",
        "eventId",
        "branchSessionId",
        "targetTensionSlope",
    ] {
        assert!(
            input_schema["properties"].get(name).is_some(),
            "missing schema property {name}"
        );
    }
    let rules = input_schema["allOf"].as_array().expect("operation rules");
    for (operation, expected) in [
        ("workspace.validate", &["bundle"][..]),
        ("workspace.replace", &["bundle", "confirmReplace"][..]),
        ("content.list", &["entity"][..]),
        ("content.get", &["entity", "id"][..]),
        ("content.delete", &["entity", "id"][..]),
        ("content.save", &["entity", "payload"][..]),
        ("content.move", &["entity", "id", "payload"][..]),
        ("content.references", &["entity", "id"][..]),
        ("scenarios.get", &["id"][..]),
        ("scenarios.delete", &["id"][..]),
        ("scenarios.create", &["payload"][..]),
        ("scenarios.save", &["payload"][..]),
        ("scenarios.preview", &["payload"][..]),
        ("storylet.approve", &["id", "expectedRevision"][..]),
        ("playtest.snapshot", &["id"][..]),
        ("playtest.director", &["id"][..]),
        ("playtest.events", &["id"][..]),
        ("playtest.delete", &["id"][..]),
        ("playtest.command", &["id", "command"][..]),
        (
            "playtest.choose",
            &[
                "id",
                "storyletId",
                "choiceId",
                "expectedRevision",
                "idempotencyKey",
            ][..],
        ),
        ("playtest.fork", &["id", "eventId"][..]),
    ] {
        let rule = rules
            .iter()
            .find(|rule| {
                let operation_schema = &rule["if"]["properties"]["operation"];
                operation_schema["const"] == operation
                    || operation_schema["enum"]
                        .as_array()
                        .is_some_and(|ops| ops.iter().any(|op| op == operation))
            })
            .unwrap_or_else(|| panic!("missing schema rule for {operation}"));
        let required = rule["then"]["required"]
            .as_array()
            .expect("required arguments");
        for name in expected {
            assert!(
                required.iter().any(|value| value == name),
                "{operation} schema missing required {name}"
            );
        }
    }
    assert_eq!(
        rules
            .iter()
            .find(|rule| rule["if"]["properties"]["operation"]["const"] == "workspace.replace")
            .unwrap()["then"]["properties"]["confirmReplace"]["const"],
        true
    );

    let operations = platform_tool["inputSchema"]["properties"]["operation"]["enum"]
        .as_array()
        .expect("operation enum");

    for op_val in operations {
        let op = op_val.as_str().unwrap();
        let call_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "narrative_platform",
                "arguments": {
                    "operation": op,
                    "entity": "storylets",
                    "id": "nonexistent",
                    "payload": { "id": "test", "title": "Test" },
                    "bundle": empty_project(),
                    "confirmReplace": false,
                    "expectedRevision": 0
                }
            }
        });
        let call_resp = storylet_engine_service::mcp::handle_mcp_message(&service, &call_req)
            .await
            .expect("tools/call response");

        // The operation must NOT fail with "Unknown narrative_platform operation"
        if let Some(err) = call_resp["error"]["message"].as_str() {
            assert!(
                !err.starts_with("Unknown narrative_platform operation"),
                "Operation '{op}' advertised in enum but unhandled in dispatch: {err}"
            );
        }
    }
    for (id, arguments, expected_error) in [
        (
            3,
            json!({
                "operation": "playtest.choose",
                "id": "missing-revision",
                "storyletId": "story",
                "choiceId": "choice",
                "idempotencyKey": "once"
            }),
            "Missing expectedRevision",
        ),
        (
            4,
            json!({
                "operation": "playtest.choose",
                "id": "missing-key",
                "storyletId": "story",
                "choiceId": "choice",
                "expectedRevision": 0
            }),
            "Missing idempotencyKey",
        ),
    ] {
        let response = storylet_engine_service::mcp::handle_mcp_message(
            &service,
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": "narrative_platform", "arguments": arguments }
            }),
        )
        .await
        .unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected_error)
        );
    }
}

#[tokio::test]
async fn playtest_choice_http_maps_blank_missing_and_conflicting_requests() {
    let app = test_app();
    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "choice-status",
            "scenarioId": "opening",
            "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    for (field, value) in [
        ("storyletId", ""),
        ("choiceId", ""),
        ("idempotencyKey", " "),
    ] {
        let mut body = json!({
            "storyletId": "story",
            "choiceId": "choice",
            "expectedRevision": 0,
            "idempotencyKey": "once"
        });
        body[field] = json!(value);
        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/playtests/choice-status/choices",
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}");
    }

    let valid = json!({
        "storyletId": "story",
        "choiceId": "choice",
        "expectedRevision": 0,
        "idempotencyKey": "once"
    });
    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/playtests/missing/choices",
        Some(valid.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/playtests/choice-status/choices",
        Some(valid),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn quality_save_persists_chronoflow_bounds_visibility_and_category() {
    let app = test_app();
    let payload = json!({
        "name": "act.progress",
        "type": "progress",
        "value": 6,
        "min_value": 0,
        "max_value": 12,
        "is_hidden": true,
        "menace_crisis_storylet_id": null
    });
    let (status, saved) = request(&app, "POST", "/api/v2/content/qualities", Some(payload)).await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(saved["type"], "progress");
    assert_eq!(saved["min_value"].as_f64(), Some(0.0));
    assert_eq!(saved["max_value"].as_f64(), Some(12.0));
    assert_eq!(saved["is_hidden"], true);

    let (status, persisted) =
        request(&app, "GET", "/api/v2/content/qualities/act.progress", None).await;
    assert_eq!(status, StatusCode::OK, "{persisted}");
    assert_eq!(persisted["max_value"].as_f64(), Some(12.0));

    let invalid = json!({
        "name": "broken",
        "type": "metric",
        "value": 0,
        "min_value": 10,
        "max_value": 5
    });
    let (status, error) = request(&app, "POST", "/api/v2/content/qualities", Some(invalid)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("minimum must not exceed maximum")
    );
}

#[tokio::test]
async fn crisis_storylet_reference_does_not_block_unrelated_storylet_save_or_allow_target_delete() {
    let app = test_app();
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "crisis-target",
            "title": "Crisis Target",
            "content": { "text_template": "A crisis." },
            "effects": {},
            "created_at": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");

    let (status, error) = request(
        &app,
        "POST",
        "/api/v2/content/qualities",
        Some(json!({
            "name": "BadDoom",
            "type": "metric",
            "value": 0,
            "min_value": 0,
            "max_value": 5,
            "menace_crisis_storylet_id": "crisis-target"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("only menace qualities")
    );
    let (status, error) = request(
        &app,
        "POST",
        "/api/v2/content/qualities",
        Some(json!({
            "name": "OrphanDoom",
            "type": "menace",
            "value": 0,
            "min_value": 0,
            "max_value": 5,
            "menace_crisis_storylet_id": "missing-storylet"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("does not exist"));
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/qualities",
        Some(json!({
            "name": "Doom",
            "type": "menace",
            "value": 0,
            "min_value": 0,
            "max_value": 5,
            "menace_crisis_storylet_id": "crisis-target"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");

    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "unrelated-scene",
            "title": "Unrelated Scene",
            "content": { "text_template": "An unrelated scene." },
            "effects": {},
            "created_at": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");

    let (status, error) = request(
        &app,
        "DELETE",
        "/api/v2/content/storylets/crisis-target",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("references it as its crisis storylet")
    );
}

#[tokio::test]
async fn character_passions_and_zero_sum_links_validate_and_persist() {
    let app = test_app();
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "crisis-scene",
            "title": "Crisis",
            "content": { "text_template": "A crisis." },
            "effects": {},
            "created_at": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({
            "id": "rival",
            "name": "Rival",
            "type": "character",
            "character": {
                "role": "npc",
                "home_location": "home",
                "current_location": "home",
                "passion_pool": [{
                    "passion_id": "Fear",
                    "current_value": 20,
                    "weight": 1,
                    "is_hidden": false
                }],
                "active_passions": ["Fear"],
                "trust_inertia": 3
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");

    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({
            "id": "star",
            "name": "Star NPC",
            "type": "character",
            "character": {
                "role": "npc",
                "home_location": "home",
                "current_location": "home",
                "passion_pool": [{
                    "passion_id": "Resolve",
                    "current_value": -15,
                    "weight": 2,
                    "is_hidden": true
                }],
                "active_passions": ["Resolve"],
                "thresholds": [{
                    "threshold_id": "resolve-high",
                    "trigger_value": 50,
                    "direction": "positive",
                    "linked_graph_asset_id": "crisis-scene"
                }],
                "zero_sum_passion_links": [{
                    "target_character_id": "rival",
                    "source_passion_id": "Resolve",
                    "target_passion_id": "Fear",
                    "weight": -1
                }]
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["character"]["passion_pool"][0]["current_value"].as_f64(),
        Some(-15.0)
    );
    assert_eq!(
        saved["character"]["zero_sum_passion_links"][0]["weight"].as_f64(),
        Some(-1.0)
    );
    assert_eq!(
        saved["character"]["thresholds"][0]["threshold_id"],
        "resolve-high"
    );

    let (status, error) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({
            "id": "duplicate",
            "name": "Duplicate",
            "type": "character",
            "character": {
                "role": "npc",
                "home_location": "home",
                "current_location": "home",
                "passion_pool": [
                    { "passion_id": "same", "current_value": 1, "weight": 1 },
                    { "passion_id": "same", "current_value": 2, "weight": 1 }
                ]
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("invalid or duplicate passion")
    );
}


#[tokio::test]
async fn bundle_validation_rejects_a_graph_without_an_end_before_import() {
    let app = test_app();
    let valid_graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 100, "height": 80, "text": "Begin" },
            { "id": "end", "title": "End", "type": "end", "x": 200, "y": 0, "width": 100, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-end", "from_node": "start", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]
    });
    let (status, _) = request(&app, "POST", "/api/v2/content/storylets", Some(json!({
        "id": "seeded", "title": "Seeded", "created_at": 1, "content": { "text_template": "" }, "graph": valid_graph
    }))).await;
    assert_eq!(status, StatusCode::CREATED);

    let graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 100, "height": 80, "text": "Begin" }
        ],
        "edges": []
    });
    let bundle = bundle_with_storylet(graph, false);
    let (status, result) = request(&app, "POST", "/api/v2/project-bundle/validate", Some(json!({ "bundle": bundle }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["valid"], false);
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| issue.as_str().unwrap().contains("at least one End")));

    let (status, result) = request(&app, "POST", "/api/v2/project-bundle/import", Some(json!({
        "bundle": bundle, "confirmReplace": true
    }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result["error"].as_str().unwrap().contains("at least one End"));
    let (status, storylets) = request(&app, "GET", "/api/v2/content/storylets", None).await;
    assert_eq!(status, StatusCode::OK);
    let storylets = storylets.as_array().unwrap();
    assert_eq!(storylets.len(), 1);
    assert_eq!(storylets[0]["id"], "seeded");
}

#[tokio::test]
async fn bundle_import_derives_ending_from_the_graph() {
    let path = std::env::temp_dir().join(format!(
        "dn-bundle-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let app = router(
        RuntimeService::from_connection(Connection::open(&path).unwrap()).unwrap(),
    );
    let graph = json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 100, "height": 80, "text": "Begin" },
            { "id": "end", "title": "End", "type": "end", "x": 200, "y": 0, "width": 100, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-end", "from_node": "start", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]
    });
    let bundle = bundle_with_storylet(graph, false);
    let (status, result) = request(&app, "POST", "/api/v2/project-bundle/import", Some(json!({
        "bundle": bundle, "confirmReplace": true
    }))).await;
    assert_eq!(status, StatusCode::OK, "{result}");

    // Read the stored row directly; the HTTP GET intentionally renormalizes it.
    let conn = Connection::open(&path).unwrap();
    let raw: String = conn
        .query_row(
            "SELECT data_json FROM content_records WHERE entity = 'storylets' AND id = 'bundle-story'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let stored: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(stored["ending"], true);
    drop(conn);
    drop(app);
    let _ = std::fs::remove_file(path);
}

fn is_object_id(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 16
        && hex
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
}

#[tokio::test]
async fn entity_create_assigns_ids_and_derives_game_id() {
    let app = test_app();
    let (status, guard) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "name": "Guard", "type": "character" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{guard}");
    let object_id = guard["object_id"].as_str().unwrap();
    assert!(is_object_id(object_id), "{object_id}");
    assert_eq!(guard["id"].as_str(), Some(object_id));
    assert_eq!(guard["game_id"].as_str(), Some("Chr_Guard"));
    assert!(guard["character"].is_object(), "{guard}");

    let (status, second) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "name": "Guard", "type": "character" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_eq!(second["game_id"].as_str(), Some("Chr_Guard_2"));

    let entity_id = guard["id"].as_str().unwrap().to_string();
    let mut forged = guard.clone();
    forged["object_id"] = json!("0x0000000000000001");
    let (status, error) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{entity_id}"),
        Some(forged),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("object_id is immutable"));

    let mut as_item = guard.clone();
    as_item["type"] = json!("item");
    let (status, saved) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{entity_id}"),
        Some(as_item),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["type"].as_str(), Some("item"));
    assert!(saved.get("character").is_none(), "{saved}");
}

#[tokio::test]
async fn legacy_character_and_location_rows_migrate_to_entities() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE project_metadata (
             singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
             project_id TEXT NOT NULL,
             name TEXT NOT NULL,
             revision INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE content_records (
             entity TEXT NOT NULL,
             id TEXT NOT NULL,
             data_json TEXT NOT NULL,
             revision INTEGER NOT NULL DEFAULT 1,
             approved_revision INTEGER,
             updated_at INTEGER NOT NULL,
             PRIMARY KEY(entity, id)
         );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO project_metadata (singleton, project_id, name, revision) VALUES (1, 'legacy', 'Legacy', 4)",
        [],
    )
    .unwrap();
    for (kind, id, data) in [
        (
            "characters",
            "old-guard",
            json!({
                "id": "old-guard",
                "name": "Old Guard",
                "role": "Role.Guard",
                "home_location": "old-dock",
                "current_location": "old-dock"
            }),
        ),
        (
            "locations",
            "old-dock",
            json!({ "id": "old-dock", "name": "Old Dock", "description": "Wet planks." }),
        ),
        (
            "storylets",
            "old-story",
            json!({
                "id": "old-story",
                "title": "Old Story",
                "content": { "text_template": "" },
                "created_at": 1,
                "position": [10, 20],
                "size": [100, 80]
            }),
        ),
    ] {
        conn.execute(
            "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES (?1, ?2, ?3, 1, NULL, 0)",
            rusqlite::params![kind, id, data.to_string()],
        )
        .unwrap();
    }

    let app = router(RuntimeService::from_connection(conn).unwrap());
    let (status, entities) = request(&app, "GET", "/api/v2/content/entities", None).await;
    assert_eq!(status, StatusCode::OK, "{entities}");
    let entities = entities.as_array().unwrap();
    assert_eq!(entities.len(), 2, "{entities:?}");
    let guard = entities
        .iter()
        .find(|entity| entity["id"] == "old-guard")
        .expect("migrated character keeps its id");
    assert_eq!(guard["type"].as_str(), Some("character"));
    assert_eq!(guard["character"]["role"].as_str(), Some("Role.Guard"));
    let dock = entities
        .iter()
        .find(|entity| entity["id"] == "old-dock")
        .expect("migrated location keeps its id");
    assert_eq!(dock["type"].as_str(), Some("location"));
    assert_eq!(dock["description"].as_str(), Some("Wet planks."));

    let (status, storylet) = request(&app, "GET", "/api/v2/content/storylets/old-story", None).await;
    assert_eq!(status, StatusCode::OK, "{storylet}");
    assert!(storylet.get("position").is_none(), "{storylet}");
    assert!(storylet.get("size").is_none(), "{storylet}");

    let (status, _) = request(&app, "GET", "/api/v2/content/characters", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn folders_validate_roots_cycles_and_block_non_empty_delete() {
    let app = test_app();
    let (status, parent) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Characters", "root": "entities" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{parent}");
    let parent_id = parent["id"].as_str().unwrap().to_string();
    let (status, child) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Nested", "root": "entities", "parent_id": parent_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{child}");
    let child_id = child["id"].as_str().unwrap().to_string();

    let (status, error) = request(
        &app,
        "POST",
        &format!("/api/v2/content/folders/{parent_id}/move"),
        Some(json!({ "folder_id": child_id })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("into itself"), "{error}");

    let (status, error) = request(
        &app,
        "POST",
        "/api/v2/content/assets",
        Some(json!({ "name": "Asset", "folder_id": parent_id })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("does not belong to assets"),
        "{error}"
    );

    let (status, error) = request(
        &app,
        "DELETE",
        &format!("/api/v2/content/folders/{parent_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert!(error["error"].as_str().unwrap().contains("is not empty"), "{error}");
}

fn storylet_with_end_graph() -> Value {
    json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 100, "height": 80, "text": "Begin" },
            { "id": "end", "title": "End", "type": "end", "x": 200, "y": 0, "width": 100, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-end", "from_node": "start", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]
    })
}

#[tokio::test]
async fn move_keeps_storylet_revision_and_approval() {
    let app = test_app();
    let (status, folder) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Act One", "root": "storylets" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{folder}");
    let folder_id = folder["id"].as_str().unwrap().to_string();

    let (status, storylet) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "movable",
            "title": "Movable",
            "created_at": 1,
            "content": { "text_template": "" },
            "graph": storylet_with_end_graph()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{storylet}");
    let (status, approved) = request(
        &app,
        "POST",
        "/api/v2/storylets/movable/approve",
        Some(json!({ "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["approved_revision"].as_u64(), Some(1));

    let (_, before) = request(&app, "GET", "/api/v2/project", None).await;
    let (status, moved) = request(
        &app,
        "POST",
        "/api/v2/content/storylets/movable/move",
        Some(json!({ "folder_id": folder_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["folder_id"].as_str(), Some(folder_id.as_str()));

    let (status, reloaded) = request(&app, "GET", "/api/v2/content/storylets/movable", None).await;
    assert_eq!(status, StatusCode::OK, "{reloaded}");
    assert_eq!(reloaded["revision"].as_u64(), Some(1), "{reloaded}");
    assert_eq!(reloaded["approved_revision"].as_u64(), Some(1), "{reloaded}");

    let (_, after) = request(&app, "GET", "/api/v2/project", None).await;
    assert_eq!(
        after["revision"].as_u64(),
        before["revision"].as_u64().map(|rev| rev + 1)
    );
}

fn storylet_with_goto_graph(location_id: &str) -> Value {
    json!({
        "nodes": [
            { "id": "start", "title": "Start", "type": "start", "x": 0, "y": 0, "width": 100, "height": 80, "text": "Begin" },
            { "id": "choice", "title": "Choice", "type": "choice", "x": 150, "y": 0, "width": 120, "height": 80,
              "choices": [{ "id": "c1", "text": "Go there" }] },
            { "id": "n1", "title": "Go To Station", "type": "go_to_location", "x": 300, "y": 0, "width": 120, "height": 80,
              "location_id": location_id },
            { "id": "end", "title": "End", "type": "end", "x": 450, "y": 0, "width": 100, "height": 80, "is_terminal": true }
        ],
        "edges": [
            { "id": "start-choice", "from_node": "start", "from_port": "output", "to_node": "choice", "to_port": "input" },
            { "id": "choice-goto", "from_node": "choice", "from_port": "choice_0", "to_node": "n1", "to_port": "input" },
            { "id": "goto-end", "from_node": "n1", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]
    })
}

#[tokio::test]
async fn entity_references_list_graph_nodes_and_block_delete() {
    let app = test_app();
    let (status, station) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "id": "station", "name": "Station", "type": "location" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{station}");
    let (status, storylet) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "goto-story",
            "title": "Go To Story",
            "created_at": 1,
            "content": { "text_template": "" },
            "graph": storylet_with_goto_graph("station")
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{storylet}");

    let (status, references) =
        request(&app, "GET", "/api/v2/content/entities/station/references", None).await;
    assert_eq!(status, StatusCode::OK, "{references}");
    let rows = references["references"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{references}");
    assert_eq!(rows[0]["source_kind"].as_str(), Some("storylet"));
    assert_eq!(rows[0]["source_id"].as_str(), Some("goto-story"));
    assert_eq!(rows[0]["node_id"].as_str(), Some("n1"));
    assert_eq!(rows[0]["via"].as_str(), Some("id"));

    let (status, error) = request(&app, "DELETE", "/api/v2/content/entities/station", None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("referenced by 1 item(s)"),
        "{error}"
    );
}

#[tokio::test]
async fn trait_and_custom_properties_reach_scenario_attribute_matching() {
    let app = test_app();
    let (status, template) = request(
        &app,
        "POST",
        "/api/v2/content/trait_templates",
        Some(json!({
            "name": "Personality",
            "key": "Personality",
            "properties": [
                { "key": "Bravery", "name": "Bravery", "type": "int", "default": 1 }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{template}");
    let template_id = template["id"].as_str().unwrap().to_string();
    let property_id = template["properties"][0]["id"].as_str().unwrap().to_string();

    let (status, _) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "id": "home", "name": "Home", "type": "location" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    for (id, name) in [("bold", "Bold"), ("timid", "Timid")] {
        let traits = if id == "bold" {
            json!([{ "template_id": template_id, "values": { property_id.clone(): 5 } }])
        } else {
            json!([])
        };
        let (status, saved) = request(
            &app,
            "POST",
            "/api/v2/content/entities",
            Some(json!({
                "id": id,
                "name": name,
                "type": "character",
                "traits": traits,
                "character": {
                    "role": "npc",
                    "home_location": "home",
                    "current_location": "home"
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
    }

    let (status, preview) = request(
        &app,
        "POST",
        "/api/v2/scenarios/preview",
        Some(json!({
            "id": "brave-only",
            "name": "Brave only",
            "seed": 3,
            "start_location_policy": { "mode": "fixed", "location_id": "home" },
            "actor_locations": {},
            "initial_qualities": {},
            "actor_population_policy": {
                "mode": "by_attribute",
                "attribute_key": "trait.Personality.Bravery",
                "attribute_value": 5
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["active_actor_ids"], json!(["bold"]), "{preview}");
}

#[tokio::test]
async fn asset_versions_are_append_only() {
    let app = test_app();
    let version = json!({
        "id": "v1",
        "file_path": "art/model.fbx",
        "creator": "designer",
        "tool_version": "1.0",
        "created_at": 1_700_000_000_000i64
    });
    let (status, asset) = request(
        &app,
        "POST",
        "/api/v2/content/assets",
        Some(json!({ "name": "Model", "versions": [version] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{asset}");
    let asset_id = asset["id"].as_str().unwrap().to_string();

    let mut edited = asset.clone();
    edited["versions"][0]["file_path"] = json!("art/model-v2.fbx");
    let (status, error) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/assets/{asset_id}"),
        Some(edited),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("append-only"), "{error}");

    let mut appended = asset.clone();
    appended["versions"] = json!([
        version,
        {
            "id": "v2",
            "file_path": "art/model-v2.fbx",
            "creator": "designer",
            "tool_version": "1.1",
            "created_at": 1_700_000_100_000i64
        }
    ]);
    let (status, saved) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/assets/{asset_id}"),
        Some(appended),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["versions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn thumbnail_upload_round_trips() {
    let root = std::env::temp_dir().join(format!(
        "dn-files-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let service = RuntimeService::open_in_memory(&root).unwrap();
    let app = router(service);

    let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];
    let upload = Request::builder()
        .method("POST")
        .uri("/api/v2/files?name=portrait.png")
        .body(Body::from(png.clone()))
        .unwrap();
    let response = app.clone().oneshot(upload).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let uploaded: Value = serde_json::from_slice(&body).unwrap();
    let path = uploaded["path"].as_str().unwrap().to_string();
    assert!(path.starts_with("project-files/thumbnails/"), "{path}");

    let fetch = Request::builder()
        .method("GET")
        .uri(format!("/api/v2/files/{path}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(fetch).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes.to_vec(), png);

    let escape = Request::builder()
        .method("GET")
        .uri("/api/v2/files/../secret.png")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(escape).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let bogus = Request::builder()
        .method("POST")
        .uri("/api/v2/files?name=notes.png")
        .body(Body::from("plain text".to_string()))
        .unwrap();
    let response = app.oneshot(bogus).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn bundle_v2_import_converts_legacy_records() {
    let app = test_app();
    let bundle = json!({
        "format": "decision-narrative-project",
        "schema_version": 2,
        "project": { "id": "legacy-bundle", "name": "Legacy Bundle", "revision": 1 },
        "content": {
            "qualities": [],
            "locations": [{ "id": "yard", "name": "Yard", "description": "Open ground." }],
            "characters": [{
                "id": "keeper",
                "name": "Keeper",
                "role": "Role.Keeper",
                "home_location": "yard",
                "current_location": "yard"
            }],
            "storylets": [],
            "npcBehaviors": [],
            "events": [],
            "scenarios": []
        }
    });
    let (status, result) = request(
        &app,
        "POST",
        "/api/v2/project-bundle/import",
        Some(json!({ "bundle": bundle, "confirmReplace": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");

    let (status, entities) = request(&app, "GET", "/api/v2/content/entities", None).await;
    assert_eq!(status, StatusCode::OK, "{entities}");
    let entities = entities.as_array().unwrap();
    assert_eq!(entities.len(), 2, "{entities:?}");
    assert!(entities.iter().any(|entity| entity["id"] == "keeper" && entity["type"] == "character"));
    assert!(entities.iter().any(|entity| entity["id"] == "yard" && entity["type"] == "location"));

    let (status, exported) = request(&app, "GET", "/api/v2/project-bundle", None).await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["schema_version"].as_u64(), Some(3));
    assert_eq!(
        exported["content"]["entities"].as_array().unwrap().len(),
        2
    );
}

#[tokio::test]
async fn trait_template_defaults_count_as_entity_references() {
    let app = test_app();
    let (status, station) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "id": "station", "name": "Station", "type": "location" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{station}");

    let (status, template) = request(
        &app,
        "POST",
        "/api/v2/content/trait_templates",
        Some(json!({
            "name": "Home Base",
            "key": "HomeBase",
            "properties": [
                { "key": "Base", "name": "Base", "type": "entity", "default": "station" }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{template}");

    let (status, references) =
        request(&app, "GET", "/api/v2/content/entities/station/references", None).await;
    assert_eq!(status, StatusCode::OK, "{references}");
    let rows = references["references"].as_array().unwrap();
    assert!(
        rows.iter().any(|row| row["source_kind"] == "trait_template"
            && row["source_id"] == template["id"]
            && row["field"] == "Default Base"),
        "{references}"
    );

    let (status, error) = request(&app, "DELETE", "/api/v2/content/entities/station", None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
}

#[tokio::test]
async fn imported_asset_versions_survive_a_later_resave() {
    let app = test_app();
    let bundle = json!({
        "format": "decision-narrative-project",
        "schema_version": 3,
        "project": { "id": "asset-bundle", "name": "Asset Bundle", "revision": 1 },
        "content": {
            "entities": [],
            "folders": [],
            "trait_templates": [],
            "qualities": [],
            "storylets": [],
            "npcBehaviors": [],
            "events": [],
            "scenarios": [],
            "assets": [{
                "id": "0x00000000000000A1",
                "name": "Imported Asset",
                "tags": [],
                "versions": [{
                    "id": "v1",
                    "file_path": "art/imported.psd",
                    "creator": "artist",
                    "tool_version": "1.0",
                    "created_at": 1_700_000_000_000i64
                }]
            }]
        }
    });
    let (status, imported) = request(
        &app,
        "POST",
        "/api/v2/project-bundle/import",
        Some(json!({ "bundle": bundle, "confirmReplace": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");

    let (status, asset) = request(&app, "GET", "/api/v2/content/assets/0x00000000000000A1", None).await;
    assert_eq!(status, StatusCode::OK, "{asset}");
    assert_eq!(asset["versions"][0]["custom_data"], json!({}));

    let mut renamed = asset.clone();
    renamed["name"] = json!("Renamed Asset");
    let (status, saved) = request(
        &app,
        "PUT",
        "/api/v2/content/assets/0x00000000000000A1",
        Some(renamed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["name"].as_str(), Some("Renamed Asset"));
    assert_eq!(saved["versions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn blank_folder_ids_are_normalized_to_the_root() {
    let app = test_app();
    let (status, entity) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "id": "loose", "name": "Loose", "type": "item", "folder_id": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{entity}");
    assert!(entity["folder_id"].is_null(), "{entity}");

    let (status, storylet) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "loose-story",
            "title": "Loose Story",
            "created_at": 1,
            "folder_id": "",
            "content": { "text_template": "" },
            "graph": storylet_with_end_graph()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{storylet}");
    assert!(storylet["folder_id"].is_null(), "{storylet}");
}

#[tokio::test]
async fn storylets_save_when_a_character_has_no_role() {
    let app = test_app();
    let (status, character) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({
            "name": "Unroled",
            "type": "character",
            "character": { "role": "", "home_location": "", "current_location": "" }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{character}");

    let (status, storylet) = request(
        &app,
        "POST",
        "/api/v2/content/storylets",
        Some(json!({
            "id": "unroled-story",
            "title": "Unroled Story",
            "created_at": 1,
            "content": { "text_template": "" },
            "graph": storylet_with_end_graph()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{storylet}");

    let (status, approved) = request(
        &app,
        "POST",
        "/api/v2/storylets/unroled-story/approve",
        Some(json!({ "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
}

#[tokio::test]
async fn auto_game_ids_follow_renames_until_the_author_edits_them() {
    let app = test_app();
    let (status, guard) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "name": "Guard", "type": "character" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{guard}");
    assert_eq!(guard["game_id"].as_str(), Some("Chr_Guard"));
    let id = guard["id"].as_str().unwrap().to_string();

    let mut renamed = guard.clone();
    renamed["name"] = json!("Night Guard");
    let (status, saved) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{id}"),
        Some(renamed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["game_id"].as_str(), Some("Chr_NightGuard"));

    let mut manual = saved.clone();
    manual["game_id"] = json!("Chr_Custom");
    let (status, manual) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{id}"),
        Some(manual),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{manual}");
    let mut renamed_again = manual.clone();
    renamed_again["name"] = json!("Dawn Guard");
    let (status, kept) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{id}"),
        Some(renamed_again),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["game_id"].as_str(), Some("Chr_Custom"));

    // A type change regenerates an auto-derived id as well.
    let (status, lantern) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "name": "Lantern", "type": "item" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{lantern}");
    assert_eq!(lantern["game_id"].as_str(), Some("Itm_Lantern"));
    let mut retyped = lantern.clone();
    retyped["type"] = json!("fact");
    let (status, fact) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{}", lantern["id"].as_str().unwrap()),
        Some(retyped),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fact}");
    assert_eq!(fact["game_id"].as_str(), Some("Fct_Lantern"));

    // An id the author supplied is kept even when it matches the generated
    // shape, because auto status comes from the server, not the value.
    let (status, explicit) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "name": "Explicit Guard", "type": "character", "game_id": "Chr_ExplicitGuard" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{explicit}");
    assert_eq!(explicit["game_id_auto"], json!(false));
    let mut explicit_renamed = explicit.clone();
    explicit_renamed["name"] = json!("Renamed Guard");
    let (status, kept_explicit) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/entities/{}", explicit["id"].as_str().unwrap()),
        Some(explicit_renamed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept_explicit}");
    assert_eq!(kept_explicit["game_id"].as_str(), Some("Chr_ExplicitGuard"));

    // Generated ids advertise their provenance so the UI can explain the rule.
    assert_eq!(guard["game_id_auto"], json!(true));
}

#[tokio::test]
async fn bundles_reject_property_definitions_that_cannot_be_saved_back() {
    let app = test_app();
    let bundle = json!({
        "format": "decision-narrative-project",
        "schema_version": 3,
        "project": { "id": "bad-template", "name": "Bad Template", "revision": 1 },
        "content": {
            "entities": [],
            "folders": [],
            "qualities": [],
            "storylets": [],
            "npcBehaviors": [],
            "events": [],
            "scenarios": [],
            "assets": [],
            "trait_templates": [{
                "id": "0x00000000000000C1",
                "key": "Mood",
                "name": "Mood",
                "properties": [
                    { "id": "p1", "key": "Brave", "name": "Brave", "type": "bool", "default": "true" }
                ]
            }]
        }
    });

    let (status, validation) = request(
        &app,
        "POST",
        "/api/v2/project-bundle/validate",
        Some(json!({ "bundle": bundle })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{validation}");
    assert_eq!(validation["valid"], false);
    assert!(
        validation["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue.as_str().unwrap().contains("default does not match its type")),
        "{validation}"
    );

    let (status, result) = request(
        &app,
        "POST",
        "/api/v2/project-bundle/import",
        Some(json!({ "bundle": bundle, "confirmReplace": true })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{result}");
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("default does not match its type"),
        "{result}"
    );

    // A definition without an id cannot be matched to entity overrides later.
    let mut missing_id = bundle.clone();
    missing_id["content"]["trait_templates"][0]["properties"][0]["default"] = json!(true);
    missing_id["content"]["trait_templates"][0]["properties"][0]["id"] = json!("");
    let (status, validation) = request(
        &app,
        "POST",
        "/api/v2/project-bundle/validate",
        Some(json!({ "bundle": missing_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{validation}");
    assert_eq!(validation["valid"], false);
    assert!(
        validation["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue.as_str().unwrap().contains("is missing an id")),
        "{validation}"
    );
}

#[tokio::test]
async fn auto_trait_keys_follow_renames_until_edited() {
    let app = test_app();
    let (status, template) = request(
        &app,
        "POST",
        "/api/v2/content/trait_templates",
        Some(json!({ "name": "New Trait Template", "properties": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{template}");
    assert_eq!(template["key"].as_str(), Some("NewTraitTemplate"));
    assert_eq!(template["key_auto"], json!(true));
    let id = template["id"].as_str().unwrap().to_string();

    let mut renamed = template.clone();
    renamed["name"] = json!("Personality");
    let (status, saved) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/trait_templates/{id}"),
        Some(renamed),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["key"].as_str(), Some("Personality"));

    let mut custom = saved.clone();
    custom["key"] = json!("Mood");
    let (status, custom) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/trait_templates/{id}"),
        Some(custom),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{custom}");
    let mut renamed_again = custom.clone();
    renamed_again["name"] = json!("Temperament");
    let (status, kept) = request(
        &app,
        "PUT",
        &format!("/api/v2/content/trait_templates/{id}"),
        Some(renamed_again),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["key"].as_str(), Some("Mood"));
}

#[tokio::test]
async fn folders_move_within_their_root_only() {
    let app = test_app();
    let (status, parent) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Parent", "root": "entities" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{parent}");
    let (status, child) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Child", "root": "entities", "parent_id": parent["id"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{child}");
    let (status, storylet_folder) = request(
        &app,
        "POST",
        "/api/v2/content/folders",
        Some(json!({ "name": "Act One", "root": "storylets" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{storylet_folder}");

    let (status, moved) = request(
        &app,
        "POST",
        &format!("/api/v2/content/folders/{}/move", child["id"].as_str().unwrap()),
        Some(json!({ "folder_id": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert!(moved["parent_id"].is_null(), "{moved}");

    let (status, error) = request(
        &app,
        "POST",
        &format!("/api/v2/content/folders/{}/move", parent["id"].as_str().unwrap()),
        Some(json!({ "folder_id": storylet_folder["id"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"].as_str().unwrap().contains("belongs to storylets"),
        "{error}"
    );
}

#[tokio::test]
async fn empty_fixed_start_location_falls_back_to_first_available_location() {
    let app = test_app();
    let (status, location) = request(
        &app,
        "POST",
        "/api/v2/content/entities",
        Some(json!({ "id": "station", "name": "Station", "type": "location" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{location}");

    let scenario = json!({
        "id": "default-start",
        "name": "Default Start Scenario",
        "seed": 1,
        "start_location_policy": { "mode": "fixed", "location_id": "" },
        "initial_qualities": {},
        "actor_locations": {},
        "actor_population_policy": { "mode": "all" }
    });

    let (status, preview) = request(
        &app,
        "POST",
        "/api/v2/scenarios/preview",
        Some(scenario),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["player_start_location"], "station");
    assert!(preview["issues"].as_array().unwrap().is_empty());
}
