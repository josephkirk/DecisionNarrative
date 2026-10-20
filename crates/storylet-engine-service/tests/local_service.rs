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
        "/api/v2/content/characters",
        Some(json!({
            "id": "rival",
            "name": "Rival",
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
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");

    let (status, saved) = request(
        &app,
        "POST",
        "/api/v2/content/characters",
        Some(json!({
            "id": "star",
            "name": "Star NPC",
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
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["passion_pool"][0]["current_value"].as_f64(),
        Some(-15.0)
    );
    assert_eq!(
        saved["zero_sum_passion_links"][0]["weight"].as_f64(),
        Some(-1.0)
    );
    assert_eq!(saved["thresholds"][0]["threshold_id"], "resolve-high");

    let (status, error) = request(
        &app,
        "POST",
        "/api/v2/content/characters",
        Some(json!({
            "id": "duplicate",
            "name": "Duplicate",
            "role": "npc",
            "home_location": "home",
            "current_location": "home",
            "passion_pool": [
                { "passion_id": "same", "current_value": 1, "weight": 1 },
                { "passion_id": "same", "current_value": 2, "weight": 1 }
            ]
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
