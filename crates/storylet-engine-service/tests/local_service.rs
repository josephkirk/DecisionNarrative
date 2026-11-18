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
async fn health_reports_ok() {
    let app = test_app();
    let (status, body) = request(&app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn legacy_and_unversioned_api_routes_are_not_registered() {
    let app = test_app();
    let v3 = "/api/v3";
    let mut paths: Vec<String> = [
        "/api/health",
        "/api/storylets",
        "/api/events",
        "/api/npc-behaviors",
        "/api/v1/playtests",
        "/api/v2/playtests",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    for suffix in [
        "",
        "/from-workspace",
        "/restore",
        "/session-1",
        "/session-1/commands",
        "/session-1/choices",
        "/session-1/director",
        "/session-1/events",
        "/session-1/stream",
        "/session-1/fork",
        "/session-1/export",
    ] {
        paths.push(format!("{v3}/playtests{suffix}"));
    }
    for suffix in [
        "",
        "/session-1",
        "/session-1/tick",
        "/session-1/signal",
        "/session-1/choose",
        "/session-1/divert",
        "/session-1/invoke",
        "/session-1/rollback",
        "/session-1/scrub",
        "/session-1/reconcile",
    ] {
        paths.push(format!("{v3}/chrono/playtests{suffix}"));
    }
    for path in &paths {
        let (status, _) = request(&app, "GET", path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "legacy route remained: {path}");
    }
}

#[tokio::test]
async fn v3_project_and_content_routes_remain_available() {
    let app = test_app();
    let (status, _) = request(&app, "GET", "/api/v3/project", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(&app, "GET", "/api/v3/content/entities", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(&app, "GET", "/api/v3/content/gameplay_tags", None).await;
    assert_ne!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn trait_uniqueness_is_enforced_on_save_and_definition_changes() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    let unique = service.save_content_item("trait_templates", &json!({
        "id": "unique-a", "key": "unique_a", "name": "Unique A", "unique": true, "properties": []
    })).await.unwrap();
    service.save_content_item("trait_templates", &json!({
        "id": "unique-b", "key": "unique_b", "name": "Unique B", "unique": true, "properties": []
    })).await.unwrap();
    let normal = service.save_content_item("trait_templates", &json!({
        "id": "normal", "key": "normal", "name": "Normal", "properties": []
    })).await.unwrap();
    assert_eq!(normal["unique"], false);
    let entity = json!({
        "id": "owner", "name": "Owner", "traits": [
            {"id":"a", "template_id":unique["id"], "values":{}},
            {"id":"n", "template_id":normal["id"], "values":{}}
        ]
    });
    service.save_content_item("entities", &entity).await.unwrap();
    let mut invalid = entity.clone();
    invalid["traits"].as_array_mut().unwrap().push(json!({"id":"b","template_id":"unique-b","values":{}}));
    assert!(service.save_content_item("entities", &invalid).await.unwrap_err().contains("only one unique trait"));
    let mut toggle = normal.clone();
    toggle["unique"] = json!(true);
    assert!(service.save_content_item("trait_templates", &toggle).await.unwrap_err().contains("only one unique trait"));
    assert_eq!(service.get_content_item("trait_templates", "normal").await.unwrap()["unique"], false);
}

#[tokio::test]
async fn core_entity_type_traits_are_mutually_exclusive() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    service.save_content_item("trait_templates", &json!({
        "id":"item", "key":"item", "name":"Item", "unique":true, "allow_multiple":false, "properties":[]
    })).await.unwrap();
    let ids = ["system.fact", "item", "system.character", "system.location"];
    for (index, id) in ids.iter().enumerate() {
        let entity = json!({"id":format!("capabilities-{index}"), "name":"Capabilities", "traits":[
            {"id":"first", "template_id":id, "values":{}}
        ]});
        service.save_content_item("entities", &entity).await.unwrap();
        for other in ids {
            let mut invalid = entity.clone();
            invalid["traits"].as_array_mut().unwrap().push(json!({"id":"second", "template_id":other, "values":{}}));
            assert!(service.save_content_item("entities", &invalid).await.unwrap_err().contains("only one unique trait"));
        }
    }
}

#[tokio::test]
async fn existing_system_trait_uniqueness_is_updated_on_open() {
    let path = std::env::temp_dir().join(format!("trait-unique-{}.db", uuid::Uuid::new_v4()));
    drop(StudioService::open(&path, ".").unwrap());
    let connection = Connection::open(&path).unwrap();
    connection.execute("UPDATE content_records SET data_json = json_set(data_json, '$.unique', json('false')) WHERE entity = 'trait_templates' AND id = 'system.fact'", []).unwrap();
    drop(connection);
    let service = StudioService::open(&path, ".").unwrap();
    let fact = service.get_content_item("trait_templates", "system.fact").await.unwrap();
    assert_eq!(fact["unique"], true);
    drop(service);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn disabling_multiple_rejects_existing_duplicate_traits() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    let mut trait_definition = service.save_content_item("trait_templates", &json!({
        "id": "repeatable", "key": "repeatable", "name": "Repeatable", "allow_multiple": true, "properties": []
    })).await.unwrap();
    service.save_content_item("entities", &json!({
        "id": "owner", "name": "Owner", "traits": [
            {"id": "first", "template_id": "repeatable", "values": {}},
            {"id": "second", "template_id": "repeatable", "values": {}}
        ]
    })).await.unwrap();
    trait_definition["allow_multiple"] = json!(false);
    assert!(service.save_content_item("trait_templates", &trait_definition).await.unwrap_err().contains("does not allow multiple instances"));
    assert_eq!(service.get_content_item("trait_templates", "repeatable").await.unwrap()["allow_multiple"], true);
}

#[tokio::test]
async fn tree_graph_references_survive_saves_and_bundle_imports() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    service.save_content_item("trait_templates", &json!({
        "id": "tree_graph", "key": "tree_graph", "name": "TreeGraph",
        "properties": [
            {"id":"nodes","key":"nodes","name":"Nodes","type":"entity","is_array":true,"default":[]},
            {"id":"edges","key":"edges","name":"Edges","type":"string","is_array":true,"default":[]},
            {"id":"direction","key":"direction","name":"Direction","type":"string","default":"right"},
            {"id":"positions","key":"positions","name":"Saved layouts","type":"string","default":""}
        ]
    })).await.unwrap();
    service.save_content_item("trait_templates", &json!({
        "id":"tree_item","key":"tree_item","name":"TreeItem","properties":[]
    })).await.unwrap();
    let item = service.save_content_item("entities", &json!({
        "id":"skill","name":"Skill","traits":[{"id":"marker","template_id":"tree_item","values":{}}]
    })).await.unwrap();
    service.save_content_item("entities", &json!({"id":"plain","name":"Plain"})).await.unwrap();
    let graph = json!({"id":"owner","name":"Owner","traits":[{
        "id":"graph","template_id":"tree_graph",
        "values":{"nodes":["skill"],"edges":["[\"skill\",\"skill\"]"],"direction":"radial","positions":"{\"radial\":{\"skill\":{\"x\":-320,\"y\":-120}}}"}
    }]});
    service.save_content_item("entities", &graph).await.unwrap();
    let mut definition = service.get_content_item("trait_templates", "tree_graph").await.unwrap();
    definition["properties"].as_array_mut().unwrap().push(json!({"id":"shapes","key":"shapes","name":"Node shapes","type":"string","default":""}));
    service.save_content_item("trait_templates", &definition).await.unwrap();
    let mut graph = graph;
    graph["traits"][0]["values"]["shapes"] = json!("{\"skill\":\"diamond\"}");
    service.save_content_item("entities", &graph).await.unwrap();
    let mut invalid_shape = graph.clone();
    invalid_shape["traits"][0]["values"]["shapes"] = json!("{\"skill\":\"triangle\"}");
    assert!(service.save_content_item("entities", &invalid_shape).await.is_err());
    let mut invalid = graph.clone();
    invalid["traits"][0]["values"]["nodes"] = json!(["plain"]);
    assert!(service.save_content_item("entities", &invalid).await.is_err());
    let mut missing_marker = item.clone();
    missing_marker["traits"] = json!([]);
    assert!(service.save_content_item("entities", &missing_marker).await.is_err());
    let mut marker = service.get_content_item("trait_templates", "tree_item").await.unwrap();
    marker["key"] = json!("renamed_item");
    assert!(service.save_content_item("trait_templates", &marker).await.is_err());
    let mut definition = service.get_content_item("trait_templates", "tree_graph").await.unwrap();
    definition["properties"].as_array_mut().unwrap().clear();
    assert!(service.save_content_item("trait_templates", &definition).await.is_err());
    assert!(service.delete_content_item("entities", "skill").await.is_err());

    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    assert!(service.validate_bundle(&bundle).await.unwrap().is_empty());
    service.import_bundle(&bundle).await.unwrap();
    let loaded = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(loaded["traits"][0]["values"], graph["traits"][0]["values"]);
}

#[tokio::test]
async fn tree_graph_appearance_survives_saves_and_rejects_invalids() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    service.save_content_item("trait_templates", &json!({
        "id": "tree_graph", "key": "tree_graph", "name": "TreeGraph",
        "properties": [
            {"id":"nodes","key":"nodes","name":"Nodes","type":"entity","is_array":true,"default":[]},
            {"id":"edges","key":"edges","name":"Edges","type":"string","is_array":true,"default":[]},
            {"id":"direction","key":"direction","name":"Direction","type":"string","default":"right"},
            {"id":"positions","key":"positions","name":"Saved layouts","type":"string","default":""},
            {"id":"appearance","key":"appearance","name":"Appearance","type":"string","default":""}
        ]
    })).await.unwrap();
    service.save_content_item("trait_templates", &json!({
        "id": "tree_item", "key": "tree_item", "name": "TreeItem", "properties": []
    })).await.unwrap();
    let item = json!({"id":"node1","name":"Node 1","traits":[{"id":"item","template_id":"tree_item","values":{}}]});
    service.save_content_item("entities", &item).await.unwrap();
    let valid_appearance = json!({
        "canvasBackground": "#1a1c20",
        "nodes": {
            "node1": {
                "background": "#112233",
                "outline": "#445566",
                "outlineWidth": 4,
                "width": 250,
                "height": 120
            }
        },
        "edges": {
            "[\"node1\",\"node1\"]": {
                "color": "#aabbcc",
                "width": 3.5,
                "pattern": "dash"
            }
        }
    }).to_string();
    let graph = json!({"id":"owner","name":"Owner","traits":[{
        "id":"graph","template_id":"tree_graph",
        "values":{
            "nodes":["node1"],
            "edges":["[\"node1\",\"node1\"]"],
            "direction":"right",
            "appearance": valid_appearance
        }
    }]});
    service.save_content_item("entities", &graph).await.unwrap();
    let loaded = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(loaded["traits"][0]["values"]["appearance"], valid_appearance);

    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    assert!(service.validate_bundle(&bundle).await.unwrap().is_empty());
    service.import_bundle(&bundle).await.unwrap();
    let imported = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(imported["traits"][0]["values"]["appearance"], valid_appearance);
    let omitted_bg_appearance = json!({
        "nodes": {
            "node1": {
                "background": "#112233",
                "outline": "#445566",
                "outlineWidth": 4,
                "width": 250,
                "height": 120
            }
        },
        "edges": {
            "[\"node1\",\"node1\"]": {
                "color": "#aabbcc",
                "width": 3.5
            }
        }
    }).to_string();
    let mut omitted_graph = graph.clone();
    omitted_graph["traits"][0]["values"]["appearance"] = json!(omitted_bg_appearance);
    service.save_content_item("entities", &omitted_graph).await.unwrap();
    let loaded_omitted = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(loaded_omitted["traits"][0]["values"]["appearance"], omitted_bg_appearance);
    let omitted_bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    assert!(service.validate_bundle(&omitted_bundle).await.unwrap().is_empty());
    service.import_bundle(&omitted_bundle).await.unwrap();
    let imported_omitted = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(imported_omitted["traits"][0]["values"]["appearance"], omitted_bg_appearance);

    for pat in ["solid", "dash", "dot", "brush"] {
        let pat_appearance = json!({
            "canvasBackground": "#1a1c20",
            "nodes": {
                "node1": {
                    "background": "#112233",
                    "outline": "#445566",
                    "outlineWidth": 4,
                    "width": 250,
                    "height": 120
                }
            },
            "edges": {
                "[\"node1\",\"node1\"]": {
                    "color": "#aabbcc",
                    "width": 3.5,
                    "pattern": pat
                }
            }
        }).to_string();
        let mut pat_graph = graph.clone();
        pat_graph["traits"][0]["values"]["appearance"] = json!(pat_appearance);
        service.save_content_item("entities", &pat_graph).await.unwrap();
        let loaded_pat = service.get_content_item("entities", "owner").await.unwrap();
        assert_eq!(loaded_pat["traits"][0]["values"]["appearance"], pat_appearance);
        let pat_bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
        assert!(service.validate_bundle(&pat_bundle).await.unwrap().is_empty());
        service.import_bundle(&pat_bundle).await.unwrap();
        let imported_pat = service.get_content_item("entities", "owner").await.unwrap();
        assert_eq!(imported_pat["traits"][0]["values"]["appearance"], pat_appearance);
    }

    service.save_content_item("entities", &graph).await.unwrap();

    let mut bad_bg = graph.clone();
    bad_bg["traits"][0]["values"]["appearance"] = json!("{\"canvasBackground\":\"#123\",\"nodes\":{\"node1\":{\"background\":\"#112233\"}}}");
    assert!(service.save_content_item("entities", &bad_bg).await.is_err());

    bad_bg["traits"][0]["values"]["appearance"] = json!("{\"canvasBackground\":\"invalid\",\"nodes\":{\"node1\":{\"background\":\"#112233\"}}}");
    assert!(service.save_content_item("entities", &bad_bg).await.is_err());

    bad_bg["traits"][0]["values"]["appearance"] = json!("{\"canvasBackground\":12345,\"nodes\":{\"node1\":{\"background\":\"#112233\"}}}");
    assert!(service.save_content_item("entities", &bad_bg).await.is_err());

    let preserved = service.get_content_item("entities", "owner").await.unwrap();
    assert_eq!(preserved["traits"][0]["values"]["appearance"], valid_appearance);

    let mut bad = graph.clone();
    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"background\":\"#123\"}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"outlineWidth\":0.5}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"outlineWidth\":15}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"width\":50}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"width\":700}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"height\":40}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"height\":500}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"missing_node\":{\"width\":200}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"missing\"]":{"width":2}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"edges\":{\"node1->node1\":{\"width\":2}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\", \"node1\"]":{"width":2}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"color":"blue"}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"width":0}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"width":13}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"pattern":"zigzag"}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"pattern":123}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"pattern":true}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"pattern":null}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"nodes\":{\"node1\":{\"unknown\":123}}}");
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!(json!({"edges":{"[\"node1\",\"node1\"]":{"unknown":123}}}).to_string());
    assert!(service.save_content_item("entities", &bad).await.is_err());

    bad["traits"][0]["values"]["appearance"] = json!("{\"extraRoot\":123}");
    assert!(service.save_content_item("entities", &bad).await.is_err());
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
    let (status, body) = request(&app, "POST", "/api/v3/content/trait_templates", Some(json!({
        "id":"inventory.item", "key":"item", "name":"Item", "properties":[]
    }))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let story = json!({"id":"crypt_tale","title":"Crypt Tale","content":{"text_template":"x"},"created_at":1});
    let (status, body) = request(&app, "POST", "/api/v3/content/storylets", Some(story)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let gem = json!({"id":"gem","name":"Gem","traits":[{"id":"item","template_id":"inventory.item","values":{}}]});
    let crypt = json!({"id":"crypt","name":"Crypt","traits":[{"id":"location","template_id":"system.location","values":{}}],"properties":[
        {"id":"map_a_ref","key":"map_a_ref","name":"Canvas: Gem reference","type":"entity","value":"gem"},
        {"id":"map_b_ref","key":"map_b_ref","name":"Canvas: Tale reference","type":"storylet","value":"crypt_tale"}]});
    for entity in [gem, crypt] {
        let (status, body) = request(&app, "POST", "/api/v3/content/entities", Some(entity)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let (status, body) = request(&app, "GET", "/api/v3/content/entities/gem/references", None).await;
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

#[tokio::test]
async fn trait_template_thumbnail_persists_clears_rejects_invalids_and_roundtrips_bundle() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();

    let trait_payload = json!({
        "id": "skill_tree",
        "key": "skill_tree",
        "name": "Skill Tree",
        "thumbnail": "project-files/thumbnails/skill-tree.png",
        "properties": []
    });
    service.save_content_item("trait_templates", &trait_payload).await.unwrap();
    let loaded = service.get_content_item("trait_templates", "skill_tree").await.unwrap();
    assert_eq!(loaded["thumbnail"], "project-files/thumbnails/skill-tree.png");

    let mut update_url = loaded.clone();
    update_url["thumbnail"] = json!("https://example.com/tree.jpg");
    service.save_content_item("trait_templates", &update_url).await.unwrap();
    let loaded_url = service.get_content_item("trait_templates", "skill_tree").await.unwrap();
    assert_eq!(loaded_url["thumbnail"], "https://example.com/tree.jpg");

    let mut update_null = loaded_url.clone();
    update_null["thumbnail"] = json!(null);
    service.save_content_item("trait_templates", &update_null).await.unwrap();
    let loaded_null = service.get_content_item("trait_templates", "skill_tree").await.unwrap();
    assert!(loaded_null.get("thumbnail").is_none() || loaded_null["thumbnail"].is_null());

    let mut update_empty = loaded_null.clone();
    update_empty["thumbnail"] = json!("   ");
    service.save_content_item("trait_templates", &update_empty).await.unwrap();
    let loaded_empty = service.get_content_item("trait_templates", "skill_tree").await.unwrap();
    assert!(loaded_empty.get("thumbnail").is_none() || loaded_empty["thumbnail"].is_null());

    let legacy_json = json!({
        "id": "legacy_trait",
        "key": "legacy_trait",
        "name": "Legacy Trait",
        "description": "Created before thumbnail existed",
        "folder_id": null,
        "properties": [],
        "allow_multiple": false,
        "unique": false,
        "system": false,
        "created_at": 1000
    });
    service.save_content_item("trait_templates", &legacy_json).await.unwrap();
    let loaded_legacy = service.get_content_item("trait_templates", "legacy_trait").await.unwrap();
    assert_eq!(loaded_legacy["name"], "Legacy Trait");
    assert!(loaded_legacy.get("thumbnail").is_none() || loaded_legacy["thumbnail"].is_null());

    let mut add_thumb = loaded_legacy.clone();
    add_thumb["thumbnail"] = json!("project-files/thumbnails/legacy.png");
    service.save_content_item("trait_templates", &add_thumb).await.unwrap();
    let reloaded_legacy = service.get_content_item("trait_templates", "legacy_trait").await.unwrap();
    assert_eq!(reloaded_legacy["thumbnail"], "project-files/thumbnails/legacy.png");

    for invalid_thumb in [
        "/absolute/path.png",
        "project-files\\thumbnails\\escape.png",
        "project-files/../escape.png",
        "project-files/thumbnails/unsupported.gif",
        "project-files/thumbnails/unsupported.svg",
        "project-files//thumbnails/double-slash.png",
    ] {
        let mut invalid = loaded.clone();
        invalid["thumbnail"] = json!(invalid_thumb);
        assert!(service.save_content_item("trait_templates", &invalid).await.is_err(), "should reject {invalid_thumb}");
    }

    let system_char = service.get_content_item("trait_templates", "system.character").await.unwrap();
    let mut modify_system = system_char.clone();
    modify_system["thumbnail"] = json!("project-files/thumbnails/char.png");
    let err = service.save_content_item("trait_templates", &modify_system).await.unwrap_err();
    assert!(err.contains("System traits cannot be modified") || err.contains("system traits are immutable"), "expected immutability error, got {err}");

    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    assert!(service.validate_bundle(&bundle).await.unwrap().is_empty());

    let imported_service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    imported_service.import_bundle(&bundle).await.unwrap();
    let imported_trait = imported_service.get_content_item("trait_templates", "legacy_trait").await.unwrap();
    assert_eq!(imported_trait["thumbnail"], "project-files/thumbnails/legacy.png");
}

#[tokio::test]
async fn trait_property_categories_survive_save_and_bundle_round_trip() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    let payload = json!({"id":"category_test", "key":"category_test", "name":"Category test", "properties":[
        {"id":"a", "key":"a", "name":"A", "type":"string", "default":"", "category":"Combat"},
        {"id":"b", "key":"b", "name":"B", "type":"int", "default":0}
    ]});
    service.save_content_item("trait_templates", &payload).await.unwrap();
    let saved = service.get_content_item("trait_templates", "category_test").await.unwrap();
    assert_eq!(saved["properties"][0]["category"], "Combat");
    assert!(saved["properties"][1].get("category").is_none());
    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    let imported = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    imported.import_bundle(&bundle).await.unwrap();
    let restored = imported.get_content_item("trait_templates", "category_test").await.unwrap();
    assert_eq!(restored["properties"][0]["category"], "Combat");
    assert_eq!(restored["properties"][1]["key"], "b");
}

#[tokio::test]
async fn trait_property_editor_metadata_round_trips_and_is_seeded() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    let settings = service.get_content_item("trait_templates", "system.character_settings").await.unwrap();
    let properties = settings["properties"].as_array().unwrap();
    assert_eq!(properties[0]["editor"]["option_source"], "gameplay_tags");
    assert_eq!(properties[0]["editor"]["selection_only"], true);
    assert_eq!(properties[1]["editor"]["entity_trait_key"], "location");
    service.save_content_item("trait_templates", &json!({
        "id":"editor_rules", "key":"editor_rules", "name":"Editor rules", "properties":[
            {"key":"custom_role", "name":"Custom role", "type":"string", "default":"", "editor":{"option_source":"gameplay_tags", "selection_only":true}},
            {"key":"custom_place", "name":"Custom place", "type":"entity", "default":null, "editor":{"entity_trait_key":"location"}},
            {"key":"score", "name":"Score", "type":"int", "default":0, "min":0, "max":10}
        ]
    })).await.unwrap();
    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    let imported = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    imported.import_bundle(&bundle).await.unwrap();
    let restored = imported.get_content_item("trait_templates", "editor_rules").await.unwrap();
    assert_eq!(restored["properties"][0]["editor"]["option_source"], "gameplay_tags");
    assert_eq!(restored["properties"][1]["editor"]["entity_trait_key"], "location");
    assert_eq!(restored["properties"][2]["min"], 0.0);
    assert_eq!(restored["properties"][2]["max"], 10.0);
    let mut invalid = restored.clone();
    invalid["properties"][0]["editor"]["entity_trait_key"] = json!("location");
    assert!(service.save_content_item("trait_templates", &invalid).await.is_err());
    let mut invalid_bounds = restored.clone();
    invalid_bounds["properties"][2]["min"] = json!(11);
    assert!(service.save_content_item("trait_templates", &invalid_bounds).await.is_err());
}

#[tokio::test]
async fn trait_numeric_bounds_reject_out_of_range_instance_values() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    service.save_content_item("trait_templates", &json!({
        "id":"bounded", "key":"bounded", "name":"Bounded", "properties":[
            {"key":"score", "name":"Score", "type":"int", "default":0, "min":0, "max":10}
        ]
    })).await.unwrap();
    let entity = |score: i64| json!({"id":"hero","name":"Hero","tags":[],"traits":[{"template_id":"bounded","values":{"score":score}}]});
    service.save_content_item("entities", &entity(10)).await.unwrap();
    assert!(service.save_content_item("entities", &entity(11)).await.is_err());
    assert!(service.save_content_item("entities", &entity(-1)).await.is_err());
}

#[test]
fn previous_schema_database_is_rejected() {
    let path = std::env::temp_dir().join(format!("decisionnarrative-v6-{}.db", uuid::Uuid::new_v4()));
    drop(StudioService::open(&path, ".").unwrap());
    let connection = Connection::open(&path).unwrap();
    connection.execute("UPDATE store_metadata SET value = '6' WHERE key = 'schema_version'", []).unwrap();
    drop(connection);
    assert!(StudioService::open(&path, ".").is_err());
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn any_entity_can_be_a_template_with_validated_template_links() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    let tpl = service.save_content_item("entities", &json!({
        "id":"tpl","name":"Guard Template","description":"base","is_template":true,
        "properties":[{"id":"hp","key":"hp","name":"HP","type":"int","value":10}]
    })).await.unwrap();
    assert_eq!(tpl["is_template"], json!(true));
    assert_eq!(tpl["template_id"], Value::Null);
    let child = service.save_content_item("entities", &json!({"id":"child","name":"Child","template_id":"tpl"})).await.unwrap();
    assert_eq!(child["template_id"], json!("tpl"));
    assert_eq!(child["is_template"], json!(false));

    service.save_content_item("entities", &json!({"id":"plain","name":"Plain"})).await.unwrap();
    let error = service.save_content_item("entities", &json!({"id":"bad","name":"Bad","template_id":"plain"})).await.unwrap_err();
    assert!(error.contains("marked as a template"), "{error}");
    let error = service.save_content_item("entities", &json!({"id":"bad","name":"Bad","template_id":"missing"})).await.unwrap_err();
    assert!(error.contains("marked as a template"), "{error}");

    let error = service.save_content_item("entities", &json!({"id":"tpl2","name":"Tpl2","is_template":true,"template_id":"tpl"})).await.unwrap_err();
    assert!(error.contains("cannot itself have a template_id"), "{error}");
    let mut chained = child.clone();
    chained["is_template"] = json!(true);
    assert!(service.save_content_item("entities", &chained).await.unwrap_err().contains("cannot itself have a template_id"));

    let mut unmarked = tpl.clone();
    unmarked["is_template"] = json!(false);
    let error = service.save_content_item("entities", &unmarked).await.unwrap_err();
    assert!(error.contains("1 entities use it"), "{error}");
    assert!(service.delete_content_item("entities", "tpl").await.is_err());

    let mut world = service.get_content_item("entities", "world").await.unwrap();
    world["is_template"] = json!(true);
    assert!(service.save_content_item("entities", &world).await.unwrap_err().contains("world entity cannot be a template"));
    world["is_template"] = json!(false);
    world["template_id"] = json!("tpl");
    assert!(service.save_content_item("entities", &world).await.unwrap_err().contains("world entity cannot have a template_id"));

    let references = service.content_references("entities", "tpl").await.unwrap();
    assert!(references.iter().any(|reference| reference.source_id == "child" && reference.field == "template_id"));

    let bundle = serde_json::to_value(service.export_project_bundle().await.unwrap()).unwrap();
    assert!(bundle["content"].get("entities").is_some_and(|entities| entities.as_array().is_some_and(|list| list.iter().any(|entity| entity["is_template"] == json!(true)))));
    assert!(service.validate_bundle(&bundle).await.unwrap().is_empty());
    service.import_bundle(&bundle).await.unwrap();
    assert_eq!(service.get_content_item("entities", "tpl").await.unwrap()["is_template"], json!(true));
    assert_eq!(service.get_content_item("entities", "child").await.unwrap()["template_id"], json!("tpl"));

    assert!(service.delete_content_item("entities", "child").await.unwrap());
    service.save_content_item("entities", &unmarked).await.unwrap();
    assert!(service.delete_content_item("entities", "tpl").await.unwrap());
}

#[tokio::test]
async fn reserved_world_id_is_rejected_in_any_case() {
    let service = StudioService::from_connection(Connection::open_in_memory().unwrap()).unwrap();
    for id in ["World", "WORLD"] {
        let error = service.save_content_item("entities", &json!({"id": id, "name": "Impostor", "traits": []})).await.unwrap_err();
        assert!(error.contains("reserved"), "{id}: {error}");
    }
}
