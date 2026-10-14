use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[test]
fn line_protocol_keeps_sessions_alive_across_requests_and_starts_from_empty_state() {
    let project = json!({
        "schema_version": 1,
        "id": "empty-project",
        "name": "Empty Project",
        "revision": 0,
        "locations": {
            "room": {
                "id": "room",
                "name": "Room",
                "description": "",
                "connected_locations": [],
                "map_position": [0.0, 0.0]
            }
        },
        "scenarios": {
            "intro": {
                "id": "intro",
                "name": "Introduction",
                "player_start_location": "room",
                "initial_qualities": {},
                "actor_locations": {},
                "storylet_ids": [],
                "seed": 7
            }
        }
    });
    let requests = [
        json!({"op": "ping", "request_id": "ready"}),
        json!({
            "op": "create_session",
            "request_id": "1",
            "session_id": "test-run",
            "scenario_id": "intro",
            "project": project
        }),
        json!({
            "op": "snapshot",
            "request_id": "2",
            "session_id": "test-run"
        }),
    ];

    let mut child = Command::new(env!("CARGO_BIN_EXE_storylet-engine-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for request in requests {
            writeln!(stdin, "{}", request).unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["request_id"], "ready");
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[0]["result"]["status"], "ready");
    assert_eq!(responses[1]["request_id"], "1");
    assert_eq!(responses[1]["ok"], true);
    assert_eq!(responses[2]["request_id"], "2");
    assert_eq!(responses[2]["result"]["world"]["turn"], 0);
    assert_eq!(responses[2]["result"]["world"]["current_location"], "room");
}
