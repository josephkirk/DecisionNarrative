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
async fn director_decision_endpoint_accepts_a_clean_no_action_result() {
    let app = test_app();
    let (status, created) = request(
        &app,
        "POST",
        "/api/v1/playtests",
        Some(json!({
            "sessionId": "ranked-run", "scenarioId": "opening", "project": empty_project()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, result) = request(
        &app,
        "POST",
        "/api/v1/playtests/ranked-run/director/decision",
        Some(json!({
            "decision": {
                "formatVersion": 1,
                "contextRevision": 0,
                "source": "fixture-ranker",
                "modelVersion": "fixture-1",
                "confidence": 0.9,
                "noActionProbability": 1.0,
                "ranking": []
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["selected"], Value::Null);
    assert_eq!(result["decision"]["source"], "fixture-ranker");
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
    assert!(RuntimeService::from_connection(connection).is_err());
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
async fn studio_graph_requires_one_complete_start_to_end_flow() {
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
    for (id, edges, expected_status) in [
        ("valid", graph["edges"].clone(), StatusCode::OK),
        ("broken", json!([]), StatusCode::CONFLICT),
    ] {
        let mut candidate = graph.clone();
        candidate["edges"] = edges;
        let (status, saved) = request(&app, "POST", "/api/v2/content/storylets", Some(json!({
            "id": id, "title": id, "created_at": 1, "content": { "text_template": "" }, "graph": candidate
        }))).await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        let (status, result) = request(
            &app,
            "POST",
            &format!("/api/v2/storylets/{id}/approve"),
            Some(json!({ "expectedRevision": 1 })),
        )
        .await;
        assert_eq!(status, expected_status, "{result}");
        if id == "broken" {
            assert!(result["error"].as_str().unwrap().contains("path to End"));
        }
    }
}
