use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use storylet_engine_service::http::{build_router, build_router_with_origins};
use storylet_engine_service::service::StudioService;

const EVIL_ORIGIN: &str = "https://evil.example";
const ALLOWED_ORIGIN: &str = "http://localhost:5173";

fn test_app() -> Router {
    build_router(StudioService::open_in_memory(".").unwrap(), None)
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    origin: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let data = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(data).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}))
    };
    (status, headers, value)
}

async fn project_name(app: &Router) -> String {
    let (status, _, body) = send(app, "GET", "/api/v3/project", None, &[], None).await;
    assert_eq!(status, StatusCode::OK);
    body["name"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn preflight_from_disallowed_origin_has_no_allow_origin_header() {
    let app = test_app();
    let (_, headers, _) = send(
        &app,
        "OPTIONS",
        "/api/v3/project",
        Some(EVIL_ORIGIN),
        &[
            ("access-control-request-method", "POST"),
            ("access-control-request-headers", "content-type"),
        ],
        None,
    )
    .await;
    assert!(
        headers.get("access-control-allow-origin").is_none(),
        "evil preflight must not be granted an allow-origin header: {headers:?}"
    );
}

#[tokio::test]
async fn preflight_from_allowed_origin_is_answered() {
    let app = test_app();
    let (_, headers, _) = send(
        &app,
        "OPTIONS",
        "/api/v3/project",
        Some(ALLOWED_ORIGIN),
        &[
            ("access-control-request-method", "POST"),
            ("access-control-request-headers", "content-type"),
        ],
        None,
    )
    .await;
    assert_eq!(
        headers.get("access-control-allow-origin").unwrap(),
        ALLOWED_ORIGIN
    );
}

#[tokio::test]
async fn state_changing_request_with_disallowed_origin_is_refused_and_changes_nothing() {
    let app = test_app();
    assert_eq!(project_name(&app).await, "Workspace");
    let (status, _, body) = send(
        &app,
        "PATCH",
        "/api/v3/project",
        Some(EVIL_ORIGIN),
        &[],
        Some(json!({"name": "Hacked"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(project_name(&app).await, "Workspace");
}

#[tokio::test]
async fn same_state_changing_request_from_allowed_origin_succeeds() {
    let app = test_app();
    let (status, _, body) = send(
        &app,
        "PATCH",
        "/api/v3/project",
        Some(ALLOWED_ORIGIN),
        &[],
        Some(json!({"name": "Renamed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(project_name(&app).await, "Renamed");
}

#[tokio::test]
async fn requests_without_origin_keep_working() {
    let app = test_app();
    let (status, _, _) = send(
        &app,
        "PATCH",
        "/api/v3/project",
        None,
        &[],
        Some(json!({"name": "Native"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(project_name(&app).await, "Native");
    let (status, _, body) = send(
        &app,
        "POST",
        "/mcp",
        None,
        &[],
        Some(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn mcp_request_with_disallowed_origin_is_refused() {
    let app = test_app();
    let (status, _, _) = send(
        &app,
        "POST",
        "/mcp",
        Some(EVIL_ORIGIN),
        &[],
        Some(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn safe_reads_with_disallowed_origin_still_work() {
    let app = test_app();
    let (status, _, body) =
        send(&app, "GET", "/api/v3/project", Some(EVIL_ORIGIN), &[], None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn extra_launch_origin_is_accepted() {
    let app = build_router_with_origins(
        StudioService::open_in_memory(".").unwrap(),
        None,
        &["https://app.example".to_string()],
    );
    let (status, _, body) = send(
        &app,
        "PATCH",
        "/api/v3/project",
        Some("https://app.example"),
        &[],
        Some(json!({"name": "Custom"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(project_name(&app).await, "Custom");
}

#[tokio::test]
async fn mcp_tool_schema_advertises_no_playtest_operations() {
    let service = StudioService::open_in_memory(".").unwrap();
    let response = storylet_engine_service::mcp::handle_mcp_message(
        &service,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    )
    .await
    .unwrap();
    let operations =
        response["result"]["tools"][0]["inputSchema"]["properties"]["operation"]["enum"]
            .as_array()
            .unwrap();
    let advertised: std::collections::BTreeSet<&str> = operations
        .iter()
        .map(|operation| operation.as_str().unwrap())
        .collect();
    let expected: std::collections::BTreeSet<&str> = [
        "run.list",
        "run.start",
        "run.view",
        "run.observe",
        "run.explain",
        "run.act",
        "run.rewind",
        "run.why_not",
        "run.inspect",
        "run.reload",
        "run.delete",
        "platform.health",
        "workspace.context",
        "workspace.inspect",
        "workspace.validate",
        "workspace.replace",
        "workspace.coverage",
        "content.list",
        "content.get",
        "content.save",
        "content.delete",
        "content.move",
        "content.references",
        "storylet.approve",
        "scenarios.list",
        "scenarios.get",
        "scenarios.create",
        "scenarios.save",
        "scenarios.delete",
        "scenarios.preview",
    ]
    .into_iter()
    .collect();
    assert_eq!(advertised, expected);
    let response = storylet_engine_service::mcp::handle_mcp_message(
        &service,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "narrative_platform", "arguments": {"operation": "playtest.list"}}}),
    )
    .await
    .unwrap();
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn origin_header_that_is_not_text_is_refused_and_changes_nothing() {
    let app = test_app();
    let request = Request::builder()
        .method("PATCH")
        .uri("/api/v3/project")
        .header("content-type", "application/json")
        .header(
            "origin",
            axum::http::HeaderValue::from_bytes(b"https://evil.example/\xff").unwrap(),
        )
        .body(Body::from(json!({"name": "Hacked"}).to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(project_name(&app).await, "Workspace");
}
