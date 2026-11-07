use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use tower::ServiceExt;

use storylet_engine_service::http::build_router;
use storylet_engine_service::service::StudioService;

fn test_app() -> Router {
    build_router(
        StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap(),
        None,
    )
}

#[test]
fn legacy_database_is_rejected_without_schema_or_data_changes() {
    let path = std::env::temp_dir().join(format!(
        "decisionnarrative-legacy-{}.db",
        uuid::Uuid::new_v4()
    ));
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(
        "CREATE TABLE store_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO store_metadata VALUES ('schema_version', '3');
         CREATE TABLE legacy_sentinel (value TEXT NOT NULL);
         INSERT INTO legacy_sentinel VALUES ('preserved');",
    ).unwrap();
    drop(connection);

    assert!(StudioService::open(&path, ".").is_err());

    let connection = Connection::open(&path).unwrap();
    let value: String = connection.query_row(
        "SELECT value FROM store_metadata WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    ).unwrap();
    let sentinel: String = connection.query_row(
        "SELECT value FROM legacy_sentinel",
        [],
        |row| row.get(0),
    ).unwrap();
    let created_project_schema: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'project_metadata')",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(value, "3");
    assert_eq!(sentinel, "preserved");
    assert!(!created_project_schema);
    drop(connection);
    std::fs::remove_file(path).unwrap();
}

async fn request(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}))
    };
    (status, value)
}

#[tokio::test]
async fn health_and_chrono_playtests_are_v3_surfaces() {
    let app = test_app();
    let (status, body) = request(&app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let (status, _) = request(&app, "GET", "/api/v3/chrono/playtests", None).await;
    assert!(status.is_success() || status == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn legacy_and_unversioned_api_routes_are_not_registered() {
    let app = test_app();
    for path in [
        "/api/health",
        "/api/storylets",
        "/api/events",
        "/api/npc-behaviors",
        "/api/v1/playtests",
        "/api/v2/playtests",
    ] {
        let (status, _) = request(&app, "GET", path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "legacy route remained: {path}");
    }
}

#[tokio::test]
async fn v3_storylet_playtest_routes_remain_available() {
    let app = test_app();
    let (status, list) = request(&app, "GET", "/api/v3/playtests", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.is_array());

    let (status, missing_snapshot) = request(&app, "GET", "/api/v3/playtests/missing", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(missing_snapshot["error"].as_str().is_some_and(|message| message.contains("not found")));

    let (status, events) = request(&app, "GET", "/api/v3/playtests/missing/events", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(events["events"].is_array());
}

#[tokio::test]
async fn v3_project_and_content_routes_remain_available() {
    let app = test_app();
    let (status, _) = request(&app, "GET", "/api/v3/project", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(&app, "GET", "/api/v3/content/entity_templates", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(&app, "GET", "/api/v3/content/gameplay_tags", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn old_v5_virtual_participant_trait_is_directly_readable_and_protected() {
    use storylet_engine_service::store::Store;
    let path = std::env::temp_dir().join(format!("dn-participant-{}.db", uuid::Uuid::new_v4()));
    drop(Store::open(&path).unwrap());
    let conn = Connection::open(&path).unwrap();
    conn.execute("DELETE FROM content_records WHERE entity='trait_templates' AND id='system.participant'", []).unwrap();
    drop(conn);
    let service = StudioService::open(&path, ".").unwrap();
    let item = service.get_content_item("trait_templates", "system.participant").await.unwrap();
    assert_eq!(item["id"], "system.participant");
    assert!(service.save_content_item("trait_templates", &item).await.is_err());
    assert!(service.delete_content_item("trait_templates", "system.participant").await.is_err());
    drop(service);
    let conn = Connection::open(&path).unwrap();
    let count: i64 = conn.query_row("SELECT count(*) FROM content_records WHERE entity='trait_templates' AND id='system.participant'", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0);
    drop(conn);
    std::fs::remove_file(path).unwrap();
}

fn reveal_storylet(event_id: &str) -> Value {
    json!({"id":"tell","title":"Tell","content":{"text_template":"x"},"created_at":1,
      "graph":{"nodes":[
        {"id":"start","title":"Start","type":"start","text":"Start","x":0,"y":0,"width":100,"height":80},
        {"id":"reveal","title":"Tell the flood","type":"reveal","event_id":event_id,"kind":"recollection","visibility":"private","x":100,"y":0,"width":100,"height":80},
        {"id":"end","title":"End","type":"end","is_terminal":true,"x":200,"y":0,"width":100,"height":80}],
       "edges":[
        {"id":"e1","from_node":"start","from_port":"output","to_node":"reveal","to_port":"input"},
        {"id":"e2","from_node":"reveal","from_port":"output","to_node":"end","to_port":"input"}]}})
}

fn event(id: &str, kind: &str, story_time: Value) -> Value {
    json!({"id":id,"title":id,"description":"","event_type":kind,"trigger":null,"effects":null,"story_time":story_time})
}

#[tokio::test]
async fn reveal_requires_canon_event_with_story_time() {
    let app = test_app();
    for e in [event("flood", "canon", json!(20)), event("untimed", "canon", Value::Null), event("storm", "world", json!(5))] {
        let (status, body) = request(&app, "POST", "/api/v3/content/events", Some(e)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let (status, body) = request(&app, "POST", "/api/v3/content/storylets", Some(reveal_storylet("flood"))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for bad in ["untimed", "storm", "missing"] {
        let (status, body) = request(&app, "POST", "/api/v3/content/storylets", Some(reveal_storylet(bad))).await;
        assert!(status.is_client_error(), "{bad}: {status} {body}");
        let message = body["error"].as_str().unwrap_or_default().to_string();
        assert!(
            message.contains(&format!("Reveal node \"Tell the flood\": event \"{bad}\" is not a canon event with a story time")),
            "{bad}: {message}"
        );
    }
}

#[tokio::test]
async fn canvas_reference_properties_are_tracked_as_references() {
    let app = test_app();
    let story = json!({"id":"crypt_tale","title":"Crypt Tale","content":{"text_template":"x"},"created_at":1});
    let (status, body) = request(&app, "POST", "/api/v3/content/storylets", Some(story)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gem = json!({"id":"gem","name":"Gem","root_tag":"item"});
    let crypt = json!({"id":"crypt","name":"Crypt","root_tag":"location","properties":[
        {"id":"map_a_ref","key":"map_a_ref","name":"Canvas: Gem reference","type":"entity","value":"gem"},
        {"id":"map_b_ref","key":"map_b_ref","name":"Canvas: Tale reference","type":"storylet","value":"crypt_tale"}]});
    for entity in [gem, crypt] {
        let (status, body) = request(&app, "POST", "/api/v3/content/entity_templates", Some(entity)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let (status, body) = request(&app, "GET", "/api/v3/content/entity_templates/gem/references", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs = body["references"].as_array().unwrap();
    assert!(refs.iter().any(|r| r["source_id"] == "crypt" && r["field"] == "Canvas: Gem reference"), "{body}");
    let (status, body) = request(&app, "GET", "/api/v3/content/storylets/crypt_tale/references", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refs = body["references"].as_array().unwrap();
    assert!(refs.iter().any(|r| r["source_id"] == "crypt" && r["field"] == "Canvas: Tale reference"), "{body}");
    let (status, _) = request(&app, "GET", "/api/v3/content/storylets/missing/references", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
