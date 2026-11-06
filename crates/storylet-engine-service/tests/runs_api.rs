use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use storylet_engine_service::{StudioService, build_router};
use tower::ServiceExt;

#[tokio::test]
async fn mcp_run_ops_listed() {
    let service = StudioService::open_in_memory(".").unwrap();
    let response = storylet_engine_service::mcp::handle_mcp_message(
        &service,
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    )
    .await
    .unwrap();
    let operations =
        response["result"]["tools"][0]["inputSchema"]["properties"]["operation"]["enum"]
            .as_array()
            .unwrap();
    for op in [
        "run.list",
        "run.start",
        "run.view",
        "run.act",
        "run.rewind",
        "run.why_not",
        "run.inspect",
        "run.reload",
        "run.delete",
    ] {
        assert!(operations.contains(&json!(op)), "missing {op}");
    }
}
#[tokio::test]
async fn empty_runs_are_listed() {
    let app = build_router(StudioService::open_in_memory(".").unwrap(), None);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v4/runs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body, json!([]));
}

fn fixture() -> (std::path::PathBuf, Value) {
    let path = std::env::temp_dir().join(format!("dn-runs-{}.db", uuid::Uuid::new_v4()));
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    let location = json!({"id":"harbor","name":"Harbor","root_tag":"location"});
    store
        .save_content("entity_templates", "harbor", &location.to_string(), 1, None)
        .unwrap();
    let scenario = json!({"id":"scenario","name":"Scenario","seed":7,"start_location_policy":{"mode":"fixed","location_id":"harbor"}});
    store
        .save_content("scenarios", "scenario", &scenario.to_string(), 1, None)
        .unwrap();
    let story = json!({"id":"story","title":"Story","content":{"text_template":"Hi"},"created_at":1,
  "graph":{"nodes":[
   {"id":"start","title":"Start","type":"start","text":"Start","x":0,"y":0,"width":100,"height":80},
   {"id":"choice","title":"Choice","type":"choice","choices":[{"id":"a","text":"A"},{"id":"b","text":"B"}],"x":100,"y":0,"width":100,"height":80},
   {"id":"end","title":"End","type":"end","is_terminal":true,"x":200,"y":0,"width":100,"height":80}],
  "edges":[{"id":"e1","from_node":"start","from_port":"output","to_node":"choice","to_port":"input"},
   {"id":"e2","from_node":"choice","from_port":"choice_0","to_node":"end","to_port":"input"},
   {"id":"e3","from_node":"choice","from_port":"choice_1","to_node":"end","to_port":"input"}]}});
    store
        .save_content("storylets", "story", &story.to_string(), 1, None)
        .unwrap();
    drop(store);
    (path, story)
}
async fn req(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(path);
    let data = if let Some(v) = body {
        b = b.header("content-type", "application/json");
        Body::from(v.to_string())
    } else {
        Body::empty()
    };
    let r = app.clone().oneshot(b.body(data).unwrap()).await.unwrap();
    let status = r.status();
    let bytes = to_bytes(r.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(json!({})))
}
#[tokio::test]
async fn start_and_act_round_trip_and_restart() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, start) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"start"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{start}");
    let id = start["id"].as_str().unwrap();
    let action = start["view"]["scene"]["choices"][0]["action"].clone();
    assert!(!action.is_null(), "{start}");
    let endpoint = format!("/api/v4/runs/{id}/act");
    let body = json!({"participant":0,"action":action,"expectedRevision":0,"idempotencyKey":"one"});
    let (status, acted) = req(&app, "POST", &endpoint, Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{acted}");
    assert_eq!(acted["view"]["currentStep"], 1);
    let (_, duplicate) = req(&app, "POST", &endpoint, Some(body)).await;
    assert_eq!(acted, duplicate);
    let (status,conflict)=req(&app,"POST",&endpoint,Some(json!({"participant":0,"action":{"kind":"wait"},"expectedRevision":0,"idempotencyKey":"two"}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    let uri = format!("/api/v4/runs/{id}");
    let (_, before) = req(&app, "GET", &uri, None).await;
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, after) = req(&app, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(before, after);
}

#[tokio::test]
async fn reload_reports_divergence_after_choice_removed() {
    let (path, mut story) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, start) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"start"})),
    )
    .await;
    let id = start["id"].as_str().unwrap().to_string();
    let action = start["view"]["scene"]["choices"][1]["action"].clone();
    let (_, acted) = req(
        &app,
        "POST",
        &format!("/api/v4/runs/{id}/act"),
        Some(json!({"participant":0,"action":action,"expectedRevision":0,"idempotencyKey":"act"})),
    )
    .await;
    assert_eq!(acted["view"]["currentStep"], 1, "{acted}");
    story["graph"]["nodes"][1]["choices"]
        .as_array_mut()
        .unwrap()
        .pop();
    story["graph"]["edges"].as_array_mut().unwrap().pop();
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    store
        .save_content("storylets", "story", &story.to_string(), 2, None)
        .unwrap();
    drop(store);
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let uri = format!("/api/v4/runs/{id}");
    let (status, stale) = req(&app, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{stale}");
    assert_eq!(stale["stale"], true);
    assert_eq!(stale["view"], acted["view"]);
    let (status, reload) = req(
        &app,
        "POST",
        &format!("{uri}/reload"),
        Some(json!({"expectedRevision":1,"idempotencyKey":"reload"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reload}");
    assert_eq!(reload["divergedAt"], 1);
    assert_eq!(reload["replayed"], 0);
    assert_eq!(reload["stale"], false);
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, restarted) = req(&app, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{restarted}");
    assert_eq!(restarted["view"], reload["view"]);
    assert_eq!(restarted["contentRevision"], reload["contentRevision"]);
}

#[tokio::test]
async fn rewound_branch_and_old_keys_survive_restart() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let start_body =
        json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"s"});
    let (_, start) = req(&app, "POST", "/api/v4/runs", Some(start_body.clone())).await;
    let id = start["id"].as_str().unwrap();
    let body = json!({"participant":0,"action":start["view"]["scene"]["choices"][0]["action"],"expectedRevision":0,"idempotencyKey":"a"});
    let (_, first) = req(
        &app,
        "POST",
        &format!("/api/v4/runs/{id}/act"),
        Some(body.clone()),
    )
    .await;
    assert_eq!(first["view"]["currentStep"], 1);
    let (_, rewound) = req(
        &app,
        "POST",
        &format!("/api/v4/runs/{id}/rewind"),
        Some(json!({"toStep":0,"expectedRevision":1,"idempotencyKey":"r"})),
    )
    .await;
    assert_eq!(rewound["view"]["currentStep"], 0, "{rewound}");
    let (_,branched)=req(&app,"POST",&format!("/api/v4/runs/{id}/act"),Some(json!({"participant":0,"action":start["view"]["scene"]["choices"][1]["action"],"expectedRevision":2,"idempotencyKey":"b"}))).await;
    assert_eq!(
        branched["view"]["branches"].as_array().unwrap().len(),
        2,
        "{branched}"
    );
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, replayed) = req(&app, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(replayed, branched);
    let (_, duplicate) = req(&app, "POST", &format!("/api/v4/runs/{id}/act"), Some(body)).await;
    assert_eq!(duplicate, first);
    let (_, same_start) = req(&app, "POST", "/api/v4/runs", Some(start_body)).await;
    assert_eq!(same_start, start);
}
#[tokio::test]
async fn independent_service_instances_detect_stale_revisions() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let other = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, start) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"s"})),
    )
    .await;
    let id = start["id"].as_str().unwrap();
    let uri = format!("/api/v4/runs/{id}");
    let _ = req(&other, "GET", &uri, None).await;
    let body = json!({"participant":0,"action":start["view"]["scene"]["choices"][0]["action"],"expectedRevision":0,"idempotencyKey":"a"});
    let (status, acted) = req(&app, "POST", &format!("{uri}/act"), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{acted}");
    let (status,conflict)=req(&other,"POST",&format!("{uri}/act"),Some(json!({"participant":0,"action":{"kind":"wait"},"expectedRevision":0,"idempotencyKey":"b"}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    let (_, view) = req(&other, "GET", &uri, None).await;
    assert_eq!(view, acted);
}
#[tokio::test]
async fn stalled_start_returns_view_and_survives_restart() {
    let (path, mut story) = fixture();
    story["graph"]["nodes"][1] = json!({"id":"choice","title":"Wait","type":"waiter","wait_event":"event.never","x":100,"y":0,"width":100,"height":80});
    story["graph"]["edges"].as_array_mut().unwrap().pop();
    story["graph"]["edges"][1]["from_port"] = json!("output");
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    store
        .save_content("storylets", "story", &story.to_string(), 2, None)
        .unwrap();
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, start) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":4,"idempotencyKey":"s"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{start}");
    assert!(start["view"].is_object());
    assert!(start["stalled"]["nodeGuid"].as_u64().is_some(), "{start}");
    let id = start["id"].as_str().unwrap().to_owned();
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, after) = req(&app, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(after, start);
    let (_, rewound) = req(
        &app,
        "POST",
        &format!("/api/v4/runs/{id}/rewind"),
        Some(json!({"toStep":0,"expectedRevision":0,"idempotencyKey":"r"})),
    )
    .await;
    assert_eq!(rewound["stalled"], start["stalled"]);
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, after) = req(&app, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(after, rewound);
}
#[tokio::test]
async fn mcp_operations_share_http_state_and_idempotency() {
    let (path, _) = fixture();
    let service = StudioService::open(&path, ".").unwrap();
    let app = build_router(service.clone(), None);
    let call = |id, arguments| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"narrative_platform","arguments":arguments}});
    let msg = call(
        1,
        json!({"operation":"run.start","start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"s"}),
    );
    let result = storylet_engine_service::mcp::handle_mcp_message(&service, &msg)
        .await
        .unwrap();
    assert!(result["result"]["isError"].is_null(), "{result}");
    let started: Value =
        serde_json::from_str(result["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let id = started["id"].as_str().unwrap();
    let (_, http) = req(&app, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(started, http);
    let action = started["view"]["scene"]["choices"][0]["action"].clone();
    let msg = call(
        2,
        json!({"operation":"run.act","id":id,"participant":0,"action":action,"expectedRevision":0,"idempotencyKey":"a"}),
    );
    let result = storylet_engine_service::mcp::handle_mcp_message(&service, &msg)
        .await
        .unwrap();
    assert!(result["result"]["isError"].is_null(), "{result}");
    let acted: Value =
        serde_json::from_str(result["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(acted["view"]["currentStep"], 1);
}
#[tokio::test]
async fn inspect_reports_actual_knowledge_and_advanced_context() {
    let (path, mut story) = fixture();
    story["graph"]["nodes"].as_array_mut().unwrap().push(json!({"id":"learn","title":"Learn","type":"learn","fact_tag":"fact.secret","kind":"rumor","visibility":"private","x":50,"y":50,"width":100,"height":80}));
    story["graph"]["edges"][0]["to_node"] = json!("learn");
    story["graph"]["edges"].as_array_mut().unwrap().push(json!({"id":"learn-choice","from_node":"learn","from_port":"output","to_node":"choice","to_port":"input"}));
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    store
        .save_content("storylets", "story", &story.to_string(), 2, None)
        .unwrap();
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status,start)=req(&app,"POST","/api/v4/runs",Some(json!({"start":{"kind":"beginning"},"seed":17,"expectedRevision":4,"idempotencyKey":"s"}))).await;
    assert_eq!(status, StatusCode::OK, "{start}");
    let id = start["id"].as_str().unwrap();
    let (status, inspect) = req(
        &app,
        "GET",
        &format!("/api/v4/runs/{id}/inspect?participant=0"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inspect}");
    assert_eq!(inspect["knows"][0]["fact"], "fact.secret");
    assert_eq!(inspect["knows"][0]["confirmed"], false);
    assert_eq!(inspect["advanced"]["runSeed"], 17);
    assert_eq!(inspect["advanced"]["contentRevision"], 4);
}
#[tokio::test]
async fn action_that_stalls_is_recorded_and_replayed() {
    let (path, mut story) = fixture();
    story["graph"]["nodes"].as_array_mut().unwrap().push(json!({"id":"wait","title":"Wait","type":"waiter","wait_event":"event.never","x":100,"y":100,"width":100,"height":80}));
    story["graph"]["edges"][1]["to_node"] = json!("wait");
    story["graph"]["edges"].as_array_mut().unwrap().push(json!({"id":"wait-end","from_node":"wait","from_port":"output","to_node":"end","to_port":"input"}));
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    store
        .save_content("storylets", "story", &story.to_string(), 2, None)
        .unwrap();
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, start) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":4,"idempotencyKey":"s"})),
    )
    .await;
    let id = start["id"].as_str().unwrap();
    let (status,acted)=req(&app,"POST",&format!("/api/v4/runs/{id}/act"),Some(json!({"participant":0,"action":start["view"]["scene"]["choices"][0]["action"],"expectedRevision":0,"idempotencyKey":"a"}))).await;
    assert_eq!(status, StatusCode::OK, "{acted}");
    assert!(acted["stalled"]["nodeGuid"].as_u64().is_some(), "{acted}");
    assert_eq!(acted["view"]["currentStep"], 1);
    drop(app);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, replayed) = req(&app, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(replayed, acted);
}
#[tokio::test]
async fn beginning_prefers_eligible_storylet_at_tagged_start_location() {
    let (path, mut story) = fixture();
    story["location_tags"] = json!(["location.harbor"]);
    let mut other = story.clone();
    other["id"] = json!("a_other");
    other["title"] = json!("Elsewhere");
    other["location_tags"] = json!(["location.elsewhere"]);
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    store
        .save_content("storylets", "story", &story.to_string(), 2, None)
        .unwrap();
    store
        .save_content("storylets", "a_other", &other.to_string(), 1, None)
        .unwrap();
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, result) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"expectedRevision":5,"idempotencyKey":"s"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["view"]["scene"]["storylet"]["title"], "Story");
}
#[test]
fn concurrent_identical_start_and_act_return_identical_results() {
    use std::sync::{Arc, Barrier};
    let (path, _) = fixture();
    let app1 = build_router(StudioService::open(&path, ".").unwrap(), None);
    let app2 = build_router(StudioService::open(&path, ".").unwrap(), None);
    fn race(
        a: axum::Router,
        b: axum::Router,
        uri: String,
        body: Value,
    ) -> [(StatusCode, Value); 2] {
        let gate = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for app in [a, b] {
            let gate = gate.clone();
            let uri = uri.clone();
            let body = body.clone();
            handles.push(std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                gate.wait();
                runtime.block_on(req(&app, "POST", &uri, Some(body)))
            }));
        }
        gate.wait();
        let mut h = handles.into_iter();
        [
            h.next().unwrap().join().unwrap(),
            h.next().unwrap().join().unwrap(),
        ]
    }
    let body =
        json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"same-start"});
    let [one, two] = race(app1.clone(), app2.clone(), "/api/v4/runs".into(), body);
    assert_eq!(one.0, StatusCode::OK, "{:?}", one.1);
    assert_eq!(two.0, StatusCode::OK, "{:?}", two.1);
    assert_eq!(one.1, two.1);
    let id = one.1["id"].as_str().unwrap();
    let body = json!({"participant":0,"action":one.1["view"]["scene"]["choices"][0]["action"],"expectedRevision":0,"idempotencyKey":"same-act"});
    let [one, two] = race(app1, app2, format!("/api/v4/runs/{id}/act"), body);
    assert_eq!(one.0, StatusCode::OK, "{:?}", one.1);
    assert_eq!(two.0, StatusCode::OK, "{:?}", two.1);
    assert_eq!(one.1, two.1);
}
#[test]
fn concurrent_content_edit_never_mislabels_compiled_revision() {
    use std::sync::{Arc, Barrier};
    for _ in 0..8 {
        let (path, mut story) = fixture();
        let app = build_router(StudioService::open(&path, ".").unwrap(), None);
        let gate = Arc::new(Barrier::new(3));
        let wg = gate.clone();
        let wp = path.clone();
        let writer = std::thread::spawn(move || {
            let store = storylet_engine_service::store::Store::open(&wp).unwrap();
            story["title"] = json!("Edited");
            wg.wait();
            store
                .save_content("storylets", "story", &story.to_string(), 2, None)
                .unwrap();
        });
        let rg = gate.clone();
        let reader = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rg.wait();
            runtime.block_on(req(
                &app,
                "POST",
                "/api/v4/runs",
                Some(
                    json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"s"}),
                ),
            ))
        });
        gate.wait();
        writer.join().unwrap();
        let (status, body) = reader.join().unwrap();
        if status == StatusCode::OK {
            assert_eq!(body["contentRevision"], 3, "{body}");
            assert_eq!(
                body["view"]["scene"]["storylet"]["title"], "Story",
                "{body}"
            );
        } else {
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
        }
    }
}
#[test]
fn failed_revision_update_rolls_back_content_save() {
    let store = storylet_engine_service::store::Store::open_in_memory().unwrap();
    let before = store.get_project_metadata().unwrap().revision;
    store.conn.execute_batch("CREATE TRIGGER reject_revision BEFORE UPDATE OF revision ON project_metadata BEGIN SELECT RAISE(ABORT, 'reject'); END;").unwrap();
    assert!(
        store
            .save_content("storylets", "partial", "{}", 1, None)
            .is_err()
    );
    assert!(
        store
            .get_content_json("storylets", "partial")
            .unwrap()
            .is_none()
    );
    assert_eq!(store.get_project_metadata().unwrap().revision, before);
}

#[tokio::test]
async fn start_from_storylet_id_resolves_the_index() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, started) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"storyletId":"story","expectedRevision":3,"idempotencyKey":"by-id"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["view"]["scene"]["storylet"]["title"], "Story");
    assert_eq!(started["view"]["scene"]["storylet"]["key"], "story");
    assert_eq!(started["view"]["place"]["name"], "Harbor");
    let (status, error) = req(
        &app,
        "POST",
        "/api/v4/runs",
        Some(json!({"start":{"kind":"beginning"},"storyletId":"nope","expectedRevision":3,"idempotencyKey":"bad-id"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
}

#[test]
fn coverage_system_traits_are_available_to_old_and_new_projects() {
    use storylet_engine_service::contracts::{PropertyType, TraitTemplateRecord};
    let store = storylet_engine_service::store::Store::open_in_memory().unwrap();
    for (id, expected) in [
        ("system.milestone", vec![("routes", PropertyType::Storylet, true), ("deadline_slot", PropertyType::Int, false)]),
        ("system.intent_probe", vec![("holder_knows", PropertyType::String, true), ("expected_routes", PropertyType::Storylet, true)]),
    ] {
        let raw = store.get_content_json("trait_templates", id).unwrap().expect("missing system trait");
        let record: TraitTemplateRecord = serde_json::from_str(&raw).unwrap();
        assert!(record.allow_multiple);
        for (key, kind, array) in expected {
            let property = record.properties.iter().find(|p| p.property.key == key).unwrap();
            assert_eq!(property.property.property_type, kind);
            assert_eq!(property.property.is_array, array);
        }
    }
}

#[tokio::test]
async fn coverage_rejects_runs_over_50000() {
    let app = build_router(StudioService::open_in_memory(".").unwrap(), None);
    let (status, _) = req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":50001}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn coverage_runs_compiled_project() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, report) = req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":1,"seed":7}))).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["cells"].as_array().unwrap().len(), 4);
    assert_eq!(report["error_count"], 0);
    assert_eq!(report["content_revision"], 3);
    assert_eq!(report["storylets"]["0"]["key"], "story");
}

#[tokio::test]
async fn coverage_reports_milestones_from_world_traits() {
    let (path, _) = fixture();
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    let mut world = store.list_entities().unwrap().into_iter().find(|entity| entity.id == "world").unwrap();
    world.traits.push(serde_json::from_value(json!({"template_id":"system.milestone","values":{"key":"rescue","name":"Rescue","routes":["story"],"reachable_for":["solo","coop"],"deadline_slot":-1}})).unwrap());
    store.save_content("entities", "world", &serde_json::to_string(&world).unwrap(), 2, None).unwrap();
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, report) = req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":1,"seed":7}))).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["cells"][0]["milestone_reached"][0][0], "rescue");
}

#[tokio::test]
async fn mcp_workspace_coverage_listed() {
    let service = StudioService::open_in_memory(".").unwrap();
    let response = storylet_engine_service::mcp::handle_mcp_message(&service, &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).await.unwrap();
    let operations = response["result"]["tools"][0]["inputSchema"]["properties"]["operation"]["enum"].as_array().unwrap();
    assert!(operations.contains(&json!("workspace.coverage")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_coverage_requests_do_not_overlap() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (first, second) = tokio::join!(req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":5000}))), req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":5000}))));
    let statuses = [first.0, second.0];
    assert!(statuses.contains(&StatusCode::OK), "{first:?} {second:?}");
    assert!(statuses.contains(&StatusCode::CONFLICT), "{first:?} {second:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_is_exclusive_across_services_for_the_same_project() {
    let (path, _) = fixture();
    let a = build_router(StudioService::open(&path, ".").unwrap(), None);
    let b = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (first, second) = tokio::join!(req(&a, "POST", "/api/v4/coverage", Some(json!({"runs":5000}))), req(&b, "POST", "/api/v4/coverage", Some(json!({"runs":5000}))));
    let statuses = [first.0, second.0];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT), "{first:?} {second:?}");
}

#[tokio::test]
async fn mcp_and_http_coverage_return_the_same_report() {
    let (path, _) = fixture();
    let service = StudioService::open(&path, ".").unwrap();
    let app = build_router(service.clone(), None);
    let (_, http) = req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":2,"seed":7}))).await;
    let message = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"narrative_platform","arguments":{"operation":"workspace.coverage","runs":2,"seed":7}}});
    let result = storylet_engine_service::mcp::handle_mcp_message(&service, &message).await.unwrap();
    assert!(result["result"]["isError"].is_null(), "{result}");
    let mcp: Value = serde_json::from_str(result["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(mcp, http);
}

#[tokio::test]
async fn start_with_recorded_actions_opens_at_last_step_and_survives_restart() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (_, original) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"expectedRevision":3,"idempotencyKey":"original"}))).await;
    let action = original["view"]["scene"]["choices"][0]["action"].clone();
    let (status, replay) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"seed":7,"actions":[[0,action]],"expectedRevision":3,"idempotencyKey":"replay"}))).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["view"]["currentStep"], 1);
    let id = replay["id"].as_str().unwrap().to_owned();
    drop(app);
    let restarted = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, restored) = req(&restarted, "GET", &format!("/api/v4/runs/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["view"]["currentStep"], 1);
}

#[tokio::test]
async fn replay_participant_configuration_survives_reload() {
    let (path, _) = fixture();
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    for (id, seat) in [("mara", 0), ("teodor", 1)] {
        let value = json!({"id":id,"name":id,"root_tag":"character","traits":[{"template_id":"system.participant","values":{"controller":"human","seat":seat,"start_location":"harbor"}},{"template_id":"system.character_settings","values":{"home_location":"harbor","current_location":"harbor"}}]});
        store.save_content("entity_templates", id, &value.to_string(), 1, None).unwrap();
    }
    let revision = store.get_project_metadata().unwrap().revision;
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, start) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"participantConfig":"solo","expectedRevision":revision,"idempotencyKey":"solo"}))).await;
    assert_eq!(status, StatusCode::OK, "{start}");
    assert_eq!(start["view"]["participants"][1]["human"], false);
    let id = start["id"].as_str().unwrap();
    let (status, reloaded) = req(&app, "POST", &format!("/api/v4/runs/{id}/reload"), Some(json!({"expectedRevision":0,"idempotencyKey":"reload"}))).await;
    assert_eq!(status, StatusCode::OK, "{reloaded}");
    assert_eq!(reloaded["view"]["participants"][1]["human"], false);
}

#[test]
fn fallback_storylet_metadata_reaches_the_explorer_dispatcher() {
    let story: storylet_engine_service::contracts::StoryletRecord = serde_json::from_value(json!({"id":"filler","title":"Filler","content":{"text_template":"Wait"},"created_at":1,"is_fallback":true})).unwrap();
    let content = storylet_engine_service::chrono_adapter::compile_chrono_dispatch(&[story], &[], &[]).unwrap();
    assert!(content.dispatcher.storylet(0).unwrap().is_fallback);
}

#[tokio::test]
async fn coverage_and_replay_preserve_large_seeds_exactly() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let seed = "18446744073709551615";
    let (status, report) = req(&app, "POST", "/api/v4/coverage", Some(json!({"runs":1,"seed":seed}))).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["seed"], seed);
    let (status, replay) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"seed":seed,"expectedRevision":3,"idempotencyKey":"wide-seed"}))).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
}

#[tokio::test]
async fn mcp_created_storylet_has_agent_origin() {
    let service = StudioService::open_in_memory(".").unwrap();
    let message = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"narrative_platform","arguments":{"operation":"content.save","entity":"storylets","payload":{"id":"agent-story","title":"Agent story","content":{"text_template":"Hello"},"created_at":1}}}});
    let result = storylet_engine_service::mcp::handle_mcp_message(&service, &message).await.unwrap();
    assert!(result["result"]["isError"].is_null(), "{result}");
    let story = service.get_content_item("storylets", "agent-story").await.unwrap();
    assert_eq!(story["origin"], "agent");
    assert!(story["approved_revision"].is_null());
    let library = service.query_library(&Default::default()).await.unwrap();
    assert_eq!(library.items.iter().find(|item| item.id == "agent-story").unwrap().origin, "agent");
    let mut edited = story.clone();
    edited["origin"] = json!("human");
    let saved = service.save_content_item("storylets", &edited).await.unwrap();
    assert_eq!(saved["origin"], "agent");
    let revision = saved["revision"].as_u64().unwrap();
    let approved = service.approve_storylet("agent-story", revision).await.unwrap();
    assert_eq!(approved.origin, "agent");
    assert_eq!(approved.approved_revision, Some(revision));
}

#[tokio::test]
async fn http_created_storylet_has_human_origin() {
    let app = build_router(StudioService::open_in_memory(".").unwrap(), None);
    let (status, story) = req(&app, "POST", "/api/v3/content/storylets", Some(json!({"id":"human-story","title":"Human story","content":{"text_template":"Hello"},"created_at":1}))).await;
    assert_eq!(status, StatusCode::CREATED, "{story}");
    assert_eq!(story["origin"], "human");
}

#[tokio::test]
async fn storylet_window_is_compiled_and_used_at_start() {
    let (path, mut story) = fixture();
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    let mut world = store.list_entities().unwrap().into_iter().find(|entity| entity.id == "world").unwrap();
    world.traits.push(serde_json::from_value(json!({"template_id":"system.clock","values":{"slot_labels":["Dawn","Dusk"],"actions_per_slot":2}})).unwrap());
    store.save_content("entities", "world", &serde_json::to_string(&world).unwrap(), 2, None).unwrap();
    story["window"] = json!([0]);
    store.save_content("storylets", "story", &story.to_string(), 2, None).unwrap();
    let revision = store.get_project_metadata().unwrap().revision;
    drop(store);
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let (status, result) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"expectedRevision":revision,"idempotencyKey":"window-start"}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["view"]["place"]["clockLabel"], "Dawn");
    let store = storylet_engine_service::store::Store::open(&path).unwrap();
    story["window"] = json!([1]);
    store.save_content("storylets", "story", &story.to_string(), 3, None).unwrap();
    let next_revision = store.get_project_metadata().unwrap().revision;
    drop(store);
    let (status, result) = req(&app, "POST", "/api/v4/runs", Some(json!({"start":{"kind":"beginning"},"expectedRevision":next_revision,"idempotencyKey":"closed-window"}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{result}");
    let (status, library) = req(&app, "GET", "/api/v3/library?q=Story", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(library["items"][0]["window"], json!([1]));
}

#[tokio::test]
async fn invalid_default_learn_visibility_is_rejected() {
    let app = build_router(StudioService::open_in_memory(".").unwrap(), None);
    let (status, _) = req(&app, "POST", "/api/v3/content/storylets", Some(json!({"id":"bad-visibility","title":"Bad","content":{"text_template":"x"},"created_at":1,"default_learn_visibility":"everyone"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_replay_does_not_create_a_partial_run() {
    let (path, _) = fixture();
    let app = build_router(StudioService::open(&path, ".").unwrap(), None);
    let body = json!({"start":{"kind":"beginning"},"actions":[[0,{"kind":"choose","nodeGuid":999999,"outputPin":0}]],"expectedRevision":3,"idempotencyKey":"bad-replay"});
    let (status, _) = req(&app, "POST", "/api/v4/runs", Some(body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, runs) = req(&app, "GET", "/api/v4/runs", None).await;
    assert!(runs.as_array().unwrap().is_empty());
}
