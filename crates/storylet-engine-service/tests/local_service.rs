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
