use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::{Value, json};
use storylet_engine::{ProjectBundle, RuntimeSessionState, StoryletRuntime, WorldCommand};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Ping {
        request_id: Value,
    },
    CreateSession {
        request_id: Value,
        session_id: String,
        project: ProjectBundle,
        scenario_id: String,
    },
    Snapshot {
        request_id: Value,
        session_id: String,
    },
    ExportSession {
        request_id: Value,
        session_id: String,
    },
    RestoreSession {
        request_id: Value,
        state: RuntimeSessionState,
    },
    DeleteSession {
        request_id: Value,
        session_id: String,
    },
    ForkSession {
        request_id: Value,
        source_session_id: String,
        branch_session_id: String,
        event_id: String,
    },
    Evaluate {
        request_id: Value,
        session_id: String,
        #[serde(default = "default_tension_slope")]
        target_tension_slope: f64,
    },
    SubmitCommand {
        request_id: Value,
        session_id: String,
        command: WorldCommand,
    },
    ExecuteChoice {
        request_id: Value,
        session_id: String,
        storylet_id: String,
        choice_id: String,
        idempotency_key: String,
        expected_revision: u64,
    },
}

fn default_tension_slope() -> f64 {
    0.15
}

fn handle(runtime: &mut StoryletRuntime, request: Request) -> Value {
    let (request_id, result) = match request {
        Request::Ping { request_id } => (request_id, Ok(json!({"status": "ready"}))),
        Request::CreateSession {
            request_id,
            session_id,
            project,
            scenario_id,
        } => (
            request_id,
            runtime
                .create_session(session_id, project, &scenario_id)
                .map(|result| json!(result)),
        ),
        Request::Snapshot {
            request_id,
            session_id,
        } => (
            request_id,
            runtime.snapshot(&session_id).map(|result| json!(result)),
        ),
        Request::ExportSession {
            request_id,
            session_id,
        } => (
            request_id,
            runtime
                .export_session(&session_id)
                .map(|result| json!(result)),
        ),
        Request::RestoreSession { request_id, state } => (
            request_id,
            runtime.restore_session(state).map(|result| json!(result)),
        ),
        Request::DeleteSession {
            request_id,
            session_id,
        } => (
            request_id,
            Ok(json!({"deleted": runtime.delete_session(&session_id)})),
        ),
        Request::ForkSession {
            request_id,
            source_session_id,
            branch_session_id,
            event_id,
        } => (
            request_id,
            runtime
                .fork_session(&source_session_id, branch_session_id, &event_id)
                .map(|result| json!(result)),
        ),
        Request::Evaluate {
            request_id,
            session_id,
            target_tension_slope,
        } => (
            request_id,
            runtime
                .evaluate(&session_id, target_tension_slope)
                .map(|result| json!(result)),
        ),
        Request::SubmitCommand {
            request_id,
            session_id,
            command,
        } => (
            request_id,
            runtime
                .submit_command(&session_id, command)
                .map(|result| json!(result)),
        ),
        Request::ExecuteChoice {
            request_id,
            session_id,
            storylet_id,
            choice_id,
            idempotency_key,
            expected_revision,
        } => (
            request_id,
            runtime
                .execute_choice(
                    &session_id,
                    &storylet_id,
                    &choice_id,
                    &idempotency_key,
                    expected_revision,
                )
                .map(|result| json!(result)),
        ),
    };

    match result {
        Ok(result) => json!({"request_id": request_id, "ok": true, "result": result}),
        Err(error) => json!({
            "request_id": request_id,
            "ok": false,
            "error": {"message": error.to_string()}
        }),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut runtime = StoryletRuntime::default();

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => match serde_json::from_str::<Request>(&line) {
                Ok(request) => handle(&mut runtime, request),
                Err(error) => {
                    let request_id = serde_json::from_str::<Value>(&line)
                        .ok()
                        .and_then(|value| value.get("request_id").cloned())
                        .unwrap_or(Value::Null);
                    json!({
                        "request_id": request_id,
                        "ok": false,
                        "error": {"message": format!("invalid request: {error}")}
                    })
                }
            },
            Err(error) => json!({
                "request_id": Value::Null,
                "ok": false,
                "error": {"message": format!("failed to read request: {error}")}
            }),
        };
        if serde_json::to_writer(&mut stdout, &response).is_err()
            || writeln!(&mut stdout).is_err()
            || stdout.flush().is_err()
        {
            break;
        }
    }
}
