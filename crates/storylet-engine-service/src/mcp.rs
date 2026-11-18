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
                    "version": env!("CARGO_PKG_VERSION")
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
            "description": "Operate the local DecisionNarrative platform. run.view and run.inspect return the Architect's global run view; use run.observe for an actor-specific observation and run.explain for an event's causal chain.",
        "inputSchema": {
            "type": "object",
            "required": ["operation"],
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "run.list", "run.start", "run.view", "run.observe", "run.explain", "run.act", "run.rewind", "run.why_not", "run.inspect", "run.reload", "run.delete",
                        "platform.health", "workspace.context", "workspace.inspect", "workspace.validate", "workspace.replace", "workspace.coverage",
                        "content.list", "content.get", "content.save", "content.delete", "content.move", "content.references", "storylet.approve",
                        "scenarios.list", "scenarios.get", "scenarios.create", "scenarios.save", "scenarios.delete", "scenarios.preview"
                    ]
                },
                "entity": {
                    "type": "string",
                    "enum": crate::contracts::CONTENT_KINDS
                },
                "id": { "type": "string" },
                "start": { "type": "object" },
                "action": { "type": "object" },
                "provenance": {
                    "type": "object",
                    "properties": {
                        "policyId": { "type": "string" },
                        "policyVersion": { "type": "string" },
                        "modelId": { "type": "string" },
                        "contextHash": { "type": "string" },
                        "decisionSeq": { "type": "integer", "minimum": 0 }
                    }
                },
                "participant": { "type": "integer", "minimum": 0 },
                "toStep": { "type": "integer", "minimum": 0 },
                "event": { "type": "integer", "minimum": 0 },
                "payload": { "type": "object" },
                "bundle": { "type": "object" },
                "confirmReplace": { "type": "boolean" },
                "expectedRevision": { "type": "integer", "minimum": 0 },
                "scenarioId": { "type": "string" },
                "storyletId": { "type": "string" },
                "idempotencyKey": { "type": "string" },
                "seed": { "anyOf": [{ "type": "integer", "minimum": 0 }, { "type": "string", "pattern": "^[0-9]+$" }] },
                "runs": { "type": "integer", "minimum": 1, "maximum": 50000 }
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
                        "properties": { "operation": { "const": "content.move" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["entity", "id", "payload"] }
                },
                {
                    "if": {
                        "properties": { "operation": { "const": "content.references" } },
                        "required": ["operation"]
                    },
                    "then": { "required": ["entity", "id"] }
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

fn content_kind(args: &Value) -> Result<&'static str, String> {
    let requested = args
        .get("entity")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing entity".to_string())?;
    crate::contracts::CONTENT_KINDS
        .iter()
        .copied()
        .find(|kind| *kind == requested)
        .ok_or_else(|| format!("Unsupported content kind '{requested}'"))
}

async fn dispatch_narrative_platform_op(
    service: &StudioService,
    op: &str,
    args: &Value,
) -> Result<Value, String> {
    match op {
        "run.list" => service.run_list().await.map_err(|e| e.to_string()),
        "run.start" => service
            .run_start(serde_json::from_value(args.clone()).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string()),
        "run.observe" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("Missing id")?;
            let participant = u16::try_from(
                args.get("participant")
                    .and_then(Value::as_u64)
                    .ok_or("Missing participant")?,
            )
            .map_err(|e| e.to_string())?;
            service
                .run_observe(id, participant)
                .await
                .map_err(|e| e.to_string())
        }
        "run.explain" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("Missing id")?;
            let event = u32::try_from(
                args.get("event")
                    .and_then(Value::as_u64)
                    .ok_or("Missing event")?,
            )
            .map_err(|e| e.to_string())?;
            service
                .run_explain(id, event)
                .await
                .map_err(|e| e.to_string())
        }
        "run.view" | "run.why_not" | "run.inspect" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("Missing id")?;
            let participant = args.get("participant").and_then(Value::as_u64).unwrap_or(0);
            let participant = u16::try_from(participant).map_err(|e| e.to_string())?;
            match op {
                "run.view" => service.run_view(id, participant).await,
                "run.why_not" => service.run_why_not(id, participant).await,
                _ => service.run_inspect(id, participant).await,
            }
            .map_err(|e| e.to_string())
        }
        "run.act" | "run.rewind" | "run.reload" | "run.delete" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("Missing id")?;
            match op {
                "run.act" => {
                    service
                        .run_act(
                            id,
                            serde_json::from_value(args.clone()).map_err(|e| e.to_string())?,
                        )
                        .await
                }
                "run.rewind" => {
                    service
                        .run_rewind(
                            id,
                            serde_json::from_value(args.clone()).map_err(|e| e.to_string())?,
                        )
                        .await
                }
                "run.reload" => {
                    service
                        .run_reload(
                            id,
                            serde_json::from_value(args.clone()).map_err(|e| e.to_string())?,
                        )
                        .await
                }
                _ => {
                    service
                        .run_delete(
                            id,
                            serde_json::from_value(args.clone()).map_err(|e| e.to_string())?,
                        )
                        .await
                }
            }
            .map_err(|e| e.to_string())
        }
        "platform.health" => {
            Ok(json!({ "status": "ok", "service": "decisionnarrative_studio_rust" }))
        }
        "workspace.coverage" => service
            .run_coverage(serde_json::from_value(args.clone()).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string()),
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
            let revision = service.import_bundle(bundle_val).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after workspace replace: {e}"
                );
            }
            Ok(json!({
                "success": true,
                "message": "Project replaced successfully",
                "revision": revision
            }))
        }
        "content.list" => {
            let entity = content_kind(args)?;
            service.list_content(entity).await
        }
        "content.get" => {
            let entity = content_kind(args)?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            service.get_content_item(entity, id).await
        }
        "content.save" => {
            let entity = content_kind(args)?;
            let payload = args
                .get("payload")
                .ok_or_else(|| "Missing payload".to_string())?;
            let res = service
                .save_content_item_with_origin(entity, payload, "agent")
                .await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after content save: {e}"
                );
            }
            Ok(res)
        }
        "content.delete" => {
            let entity = content_kind(args)?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let deleted = service
                .delete_content_item(entity, id)
                .await
                .map_err(|error| error.to_string())?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after content delete: {e}"
                );
            }
            Ok(json!({ "deleted": deleted }))
        }
        "content.move" => {
            let entity = content_kind(args)?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            let folder_id = args
                .get("payload")
                .and_then(|payload| payload.get("folder_id"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());
            let res = service.move_content_item(entity, id, folder_id).await?;
            if let Err(e) = service.mark_interaction_context_stale().await {
                eprintln!(
                    "Warning: failed to mark interaction context stale after content move: {e}"
                );
            }
            Ok(res)
        }
        "content.references" => {
            let entity = content_kind(args)?;
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| "Missing id".to_string())?;
            serde_json::to_value(service.content_references(entity, id).await?)
                .map_err(|error| error.to_string())
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
            let deleted = service
                .delete_content_item("scenarios", id)
                .await
                .map_err(|error| error.to_string())?;
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
        _ => Err(format!("Unknown narrative_platform operation: {op}")),
    }
}
