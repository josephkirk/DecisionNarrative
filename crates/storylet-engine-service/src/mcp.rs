//! Native MCP (Model Context Protocol) stdio and HTTP support for DecisionNarrative.

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

use crate::service::StudioService;

pub async fn run_stdio_mcp(service: StudioService) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        if let Some(resp) = handle_mcp_message(&service, &msg).await {
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

pub async fn handle_mcp_message(service: &StudioService, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg.get("method").and_then(|m| m.as_str())?;

    if method == "notifications/initialized" || method.starts_with("notifications/") {
        return None;
    }

    if method == "initialize" {
        return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "decisionnarrative_strata",
                    "version": "1.0.0"
                }
            }
        }));
    }

    if method == "ping" {
        return Some(json!({ "jsonrpc": "2.0", "id": id, "result": {} }));
    }

    if method == "tools/list" {
        return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": get_tool_definitions()
            }
        }));
    }

    if method == "tools/call" {
        let name = msg
            .get("params")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let args = msg
            .get("params")
            .and_then(|p| p.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        let call_res = dispatch_tool_call(service, name, &args).await;
        match call_res {
            Ok(val) => {
                let text = if val.is_string() {
                    val.as_str().unwrap().to_string()
                } else {
                    serde_json::to_string_pretty(&val).unwrap_or_default()
                };
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": text }]
                    }
                }));
            }
            Err(err_msg) => {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Error: {err_msg}") }],
                        "isError": true
                    }
                }));
            }
        }
    }

    Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32601,
            "message": format!("Method not found: {method}")
        }
    }))
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "narrative_platform",
            "description": "Operate the local DecisionNarrative platform: inspect and validate workspace content, manage project entities, run staged Strata-backed generation, and exercise deterministic Rust playtests.",
            "inputSchema": {
                "type": "object",
                "required": ["operation"],
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": [
                            "platform.health", "workspace.inspect", "workspace.validate", "workspace.replace",
                            "content.list", "content.save", "content.delete", "storylet.approve",
                            "scenarios.list", "scenarios.get", "scenarios.create", "scenarios.save", "scenarios.delete", "scenarios.preview",
                            "craft_resources.list", "generation.list", "generation.create", "generation.get",
                            "generation.advance", "generation.edit_stage", "generation.save_draft", "generation.delete",
                            "playtest.list", "playtest.create_from_workspace", "playtest.create", "playtest.snapshot",
                            "playtest.director", "playtest.rank", "playtest.command", "playtest.choose", "playtest.fork",
                            "playtest.events", "playtest.delete"
                        ]
                    },
                    "entity": {
                        "type": "string",
                        "enum": ["qualities", "characters", "locations", "storylets", "npc_behaviors", "events"]
                    },
                    "id": { "type": "string" },
                    "payload": { "type": "object" },
                    "bundle": { "type": "object" },
                    "confirmReplace": { "type": "boolean" },
                    "expectedRevision": { "type": "integer", "minimum": 0 },
                    "stage": { "type": "string" },
                    "artifact": {},
                    "startLocationId": { "type": "string" },
                    "scenarioId": { "type": "string" },
                    "command": { "type": "object" },
                    "choiceId": { "type": "string" },
                    "idempotencyKey": { "type": "string" },
                    "eventId": { "type": "string" },
                    "branchSessionId": { "type": "string" },
                    "targetTensionSlope": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "generate_storylet_content_batch",
            "description": "Generate 1-24 valid DecisionNarrative StoryletRecord draft candidates using local Strata. Returns candidate records with deterministic IDs and approved_revision=null.",
            "inputSchema": {
                "type": "object",
                "required": ["scenario_family", "count", "brief"],
                "properties": {
                    "scenario_family": { "type": "string", "minLength": 1, "maxLength": 80 },
                    "count": { "type": "integer", "minimum": 1, "maximum": 24 },
                    "batch_index": { "type": "integer", "minimum": 1, "default": 1 },
                    "brief": { "type": "string", "minLength": 1, "maxLength": 12000 },
                    "constraints": { "type": "array", "items": { "type": "string" } }
                }
            }
        }),
        json!({
            "name": "generate_storylet_ranking_batch",
            "description": "Generate 1-24 Strata-synthetic, training-only storylet ranking examples matching model/ranking-example.schema.json.",
            "inputSchema": {
                "type": "object",
                "required": ["scenario_family", "count", "brief"],
                "properties": {
                    "scenario_family": { "type": "string", "minLength": 1, "maxLength": 80 },
                    "count": { "type": "integer", "minimum": 1, "maximum": 24 },
                    "batch_index": { "type": "integer", "minimum": 1, "default": 1 },
                    "brief": { "type": "string", "minLength": 1, "maxLength": 12000 },
                    "constraints": { "type": "array", "items": { "type": "string" } }
                }
            }
        }),
    ]
}

async fn dispatch_tool_call(
    service: &StudioService,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    match name {
        "narrative_platform" => {
            let op = args
                .get("operation")
                .and_then(|o| o.as_str())
                .ok_or_else(|| "Missing required 'operation'".to_string())?;
            dispatch_narrative_platform_op(service, op, args).await
        }
        "generate_storylet_content_batch" => generate_content_batch_mcp(service, args).await,
        "generate_storylet_ranking_batch" => generate_ranking_batch_mcp(service, args).await,
        _ => Err(format!("Unknown tool: {name}")),
    }
}

async fn dispatch_narrative_platform_op(
    service: &StudioService,
    op: &str,
    args: &Value,
) -> Result<Value, String> {
    match op {
        "platform.health" => {
            Ok(json!({ "status": "ok", "service": "decisionnarrative_studio_rust" }))
        }
        "workspace.inspect" => {
            let bundle = service.export_project_bundle().await?;
            serde_json::to_value(&bundle).map_err(|e| e.to_string())
        }
        "workspace.validate" => {
            let bundle_val = args
                .get("bundle")
                .ok_or_else(|| "Missing bundle payload".to_string())?;
            let issues = service.validate_bundle(bundle_val).await?;
            Ok(json!({ "valid": issues.is_empty(), "issues": issues }))
        }
        "workspace.replace" => {
            if args.get("confirmReplace") != Some(&Value::Bool(true)) {
                return Err("workspace.replace requires confirmReplace=true".into());
            }
            let bundle_val = args
                .get("bundle")
                .ok_or_else(|| "Missing bundle payload".to_string())?;
            service.import_bundle(bundle_val).await?;
            Ok(json!({ "success": true, "message": "Project replaced successfully" }))
        }
        "content.list" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            service.list_content(entity).await
        }
        "content.save" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            service.save_content_item(entity, payload).await
        }
        "content.delete" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service.delete_content_item(entity, id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "storylet.approve" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let rev = args
                .get("expectedRevision")
                .and_then(|r| r.as_u64())
                .ok_or_else(|| "Missing expectedRevision".to_string())?;
            let st = service.approve_storylet(id, rev).await?;
            serde_json::to_value(&st).map_err(|e| e.to_string())
        }
        "scenarios.list" => service.list_content("scenarios").await,
        "scenarios.get" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_content_item("scenarios", id).await
        }
        "scenarios.create" | "scenarios.save" => {
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            service.save_content_item("scenarios", payload).await
        }
        "scenarios.delete" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service.delete_content_item("scenarios", id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "scenarios.preview" => {
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            service.preview_scenario(payload).await
        }
        "craft_resources.list" => {
            let catalog = service.list_craft_resources().await?;
            serde_json::to_value(&catalog).map_err(|e| e.to_string())
        }
        "generation.list" => service.list_generation_jobs().await,
        "generation.create" => {
            let payload = args.get("payload").cloned().unwrap_or(json!({}));
            let job = service.create_generation_job(payload).await?;
            serde_json::to_value(&job).map_err(|e| e.to_string())
        }
        "generation.get" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_generation_job(id).await
        }
        "generation.advance" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let rev = args
                .get("expectedRevision")
                .and_then(|r| r.as_u64())
                .ok_or_else(|| "Missing expectedRevision".to_string())?;
            let job = service.advance_generation_job(id, rev).await?;
            serde_json::to_value(&job).map_err(|e| e.to_string())
        }
        "generation.edit_stage" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let stage = args
                .get("stage")
                .and_then(|s| s.as_str())
                .ok_or_else(|| "Missing stage".to_string())?;
            let rev = args
                .get("expectedRevision")
                .and_then(|r| r.as_u64())
                .ok_or_else(|| "Missing expectedRevision".to_string())?;
            let artifact = args.get("artifact").cloned().unwrap_or(Value::Null);
            let job = service
                .edit_generation_stage(id, stage, rev, artifact)
                .await?;
            serde_json::to_value(&job).map_err(|e| e.to_string())
        }
        "generation.save_draft" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let st = service.save_generation_draft(id).await?;
            serde_json::to_value(&st).map_err(|e| e.to_string())
        }
        "generation.delete" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service.delete_generation_job(id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "playtest.list" => service.list_playtest_sessions().await,
        "playtest.create_from_workspace" => {
            let start_loc = args.get("startLocationId").and_then(|s| s.as_str());
            let scen_id = args.get("scenarioId").and_then(|s| s.as_str());
            service
                .create_playtest_from_workspace(start_loc, scen_id)
                .await
        }
        "playtest.snapshot" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_playtest_snapshot(id).await
        }
        "playtest.director" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let slope = args
                .get("targetTensionSlope")
                .and_then(|s| s.as_f64())
                .unwrap_or(0.15);
            service.evaluate_playtest_director(id, slope).await
        }
        "playtest.rank" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let slope = args
                .get("targetTensionSlope")
                .and_then(|s| s.as_f64())
                .unwrap_or(0.15);
            service.rank_playtest_with_local_encoder(id, slope).await
        }
        "playtest.command" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let cmd = args
                .get("command")
                .ok_or_else(|| "Missing command".to_string())?;
            service.apply_playtest_command(id, cmd).await
        }
        "playtest.choose" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let st_id = args
                .get("storyletId")
                .and_then(|s| s.as_str())
                .ok_or_else(|| "Missing storyletId".to_string())?;
            let choice_id = args
                .get("choiceId")
                .and_then(|c| c.as_str())
                .ok_or_else(|| "Missing choiceId".to_string())?;
            let rev = args
                .get("expectedRevision")
                .and_then(|r| r.as_u64())
                .unwrap_or(0);
            let key = args
                .get("idempotencyKey")
                .and_then(|k| k.as_str())
                .unwrap_or("");
            service
                .apply_playtest_choice(id, st_id, choice_id, rev, key)
                .await
        }
        "playtest.fork" => {
            let src = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let branch = args.get("branchSessionId").and_then(|b| b.as_str());
            let ev_id = args
                .get("eventId")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing eventId".to_string())?;
            service.fork_playtest(src, branch, ev_id).await
        }
        "playtest.events" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_playtest_events(id).await
        }
        "playtest.delete" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service.delete_playtest(id).await?;
            Ok(json!({ "deleted": deleted }))
        }
        _ => Err(format!("Unknown narrative_platform operation: {op}")),
    }
}

async fn generate_content_batch_mcp(
    service: &StudioService,
    args: &Value,
) -> Result<Value, String> {
    let family = args
        .get("scenario_family")
        .and_then(|f| f.as_str())
        .unwrap_or("default");
    let count = args.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as usize;
    let brief = args.get("brief").and_then(|b| b.as_str()).unwrap_or("");
    let batch_idx = args
        .get("batch_index")
        .and_then(|b| b.as_u64())
        .unwrap_or(1);
    service
        .generate_storylet_batch(family, count, batch_idx, brief)
        .await
}

async fn generate_ranking_batch_mcp(
    service: &StudioService,
    args: &Value,
) -> Result<Value, String> {
    let family = args
        .get("scenario_family")
        .and_then(|f| f.as_str())
        .unwrap_or("default");
    let count = args.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as usize;
    let brief = args.get("brief").and_then(|b| b.as_str()).unwrap_or("");
    let batch_idx = args
        .get("batch_index")
        .and_then(|b| b.as_u64())
        .unwrap_or(1);
    service
        .generate_ranking_batch(family, count, batch_idx, brief)
        .await
}
