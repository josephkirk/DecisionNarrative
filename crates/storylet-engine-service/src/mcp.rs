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
                    "name": "decisionnarrative",
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
    vec![json!({
        "name": "narrative_platform",
        "description": "Operate the local DecisionNarrative platform: inspect workspace context and content, manage authored project entities, and exercise deterministic Rust playtests.",
        "inputSchema": {
            "type": "object",
            "required": ["operation"],
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "platform.health", "workspace.context", "workspace.inspect", "workspace.validate", "workspace.replace",
                        "content.list", "content.get", "content.save", "content.delete", "storylet.approve",
                        "scenarios.list", "scenarios.get", "scenarios.create", "scenarios.save", "scenarios.delete", "scenarios.preview",
                        "playtest.list", "playtest.create_from_workspace", "playtest.snapshot",
                        "playtest.director", "playtest.command", "playtest.choose", "playtest.fork",
                        "playtest.events", "playtest.delete"
                    ]
                },
                "entity": {
                    "type": "string",
                    "enum": ["qualities", "characters", "locations", "storylets", "npc_behaviors", "events", "scenarios"]
                },
                "id": { "type": "string" },
                "payload": { "type": "object" },
                "bundle": { "type": "object" },
                "confirmReplace": { "type": "boolean" },
                "expectedRevision": { "type": "integer", "minimum": 0 },
                "startLocationId": { "type": "string" },
                "scenarioId": { "type": "string" },
                "command": { "type": "object" },
                "choiceId": { "type": "string" },
                "storyletId": { "type": "string" },
                "idempotencyKey": { "type": "string" },
                "eventId": { "type": "string" },
                "branchSessionId": { "type": "string" },
                "targetTensionSlope": { "type": "number" }
            },
            "allOf": [
                {
                    "if": {
                        "properties": { "operation": { "const": "workspace.validate" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["bundle"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "workspace.replace" } },
                        "required": ["operation"]
                    },
                    "then": {
                        "required": ["bundle", "confirmReplace"],
                        "properties": { "confirmReplace": { "const": true } }
                    }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "content.list" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["entity"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "enum": ["content.get", "content.delete"] } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["entity", "id"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "content.save" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["entity", "payload"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "enum": ["scenarios.get", "scenarios.delete"] } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["id"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "enum": ["scenarios.create", "scenarios.save", "scenarios.preview"] } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["payload"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "storylet.approve" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["id", "expectedRevision"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "enum": ["playtest.snapshot", "playtest.director", "playtest.events", "playtest.delete"] } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["id"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "playtest.command" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["id", "command"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "playtest.choose" } },
                        "required": ["operation"]
                    },
                    "then": {
                        "required": ["id", "storyletId", "choiceId", "expectedRevision", "idempotencyKey"]
                    }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "playtest.fork" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["id", "eventId"] }
                }
            ]
        }
    })]
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
        "workspace.context" => service.get_interaction_context().await,
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
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after workspace replace: {e}"
                );
            }
            Ok(json!({ "success": true, "message": "Project replaced successfully" }))
        }
        "content.list" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            service.list_content(entity).await
        }
        "content.get" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_content_item(entity, id).await
        }
        "content.save" => {
            let entity = args
                .get("entity")
                .and_then(|e| e.as_str())
                .ok_or_else(|| "Missing entity".to_string())?;
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            let res = service.save_content_item(entity, payload).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after content save: {e}"
                );
            }
            Ok(res)
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
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after content delete: {e}"
                );
            }
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
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after storylet approve: {e}"
                );
            }
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
            let res = service.save_content_item("scenarios", payload).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after scenario save: {e}"
                );
            }
            Ok(res)
        }
        "scenarios.delete" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service.delete_content_item("scenarios", id).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after scenario delete: {e}"
                );
            }
            Ok(json!({ "deleted": deleted }))
        }
        "scenarios.preview" => {
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            service.preview_scenario(payload).await
        }
        "playtest.list" => service.list_playtest_sessions().await,
        "playtest.create_from_workspace" => {
            let start_loc = args.get("startLocationId").and_then(|s| s.as_str());
            let scen_id = args.get("scenarioId").and_then(|s| s.as_str());
            let res = service
                .create_playtest_from_workspace(start_loc, scen_id)
                .await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after playtest create: {e}"
                );
            }
            Ok(res)
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
        "playtest.command" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let cmd = args
                .get("command")
                .ok_or_else(|| "Missing command".to_string())?;
            let res = service.apply_playtest_command(id, cmd).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after playtest command: {e}"
                );
            }
            Ok(res)
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
                .ok_or_else(|| "Missing expectedRevision".to_string())?;
            let key = args
                .get("idempotencyKey")
                .and_then(|k| k.as_str())
                .filter(|key| !key.trim().is_empty())
                .ok_or_else(|| "Missing idempotencyKey (non-empty)".to_string())?;
            let res = service
                .apply_playtest_choice(id, st_id, choice_id, rev, key)
                .await
                .map_err(|error| error.to_string())?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after playtest choice: {e}"
                );
            }
            Ok(res)
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
            let res = service.fork_playtest(src, branch, ev_id).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after playtest fork: {e}"
                );
            }
            Ok(res)
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
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after playtest delete: {e}"
                );
            }
            Ok(json!({ "deleted": deleted }))
        }
        _ => Err(format!("Unknown narrative_platform operation: {op}")),
    }
}
