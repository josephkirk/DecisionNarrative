use serde_json::Value;
use storylet_engine::ffi::{
    dn_buffer_free, dn_engine_abi_version, dn_engine_create, dn_engine_destroy,
    dn_engine_export_snapshot, dn_engine_import_snapshot, dn_playtest_create, dn_playtest_destroy,
    dn_playtest_evaluate, dn_playtest_execute_choice, dn_playtest_export_session,
    dn_playtest_restore_session, dn_playtest_snapshot, dn_playtest_start,
};

unsafe fn take_json(buffer: &storylet_engine::ffi::DnBuffer) -> Value {
    let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) };
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn native_abi_exports_imports_and_releases_versioned_world_snapshot() {
    assert_eq!(dn_engine_abi_version(), 1);
    let handle = dn_engine_create();
    let output = unsafe { dn_engine_export_snapshot(handle) };
    assert!(!output.data.is_null());
    let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["revision"], 0);
    assert!(unsafe { dn_engine_import_snapshot(handle, output.data, output.len) });
    unsafe {
        dn_buffer_free(output);
        dn_engine_destroy(handle);
    }
}

#[test]
fn native_abi_hosts_a_generic_storylet_from_scenario_through_choice() {
    let handle = dn_playtest_create();
    assert!(!handle.is_null());
    let project = br#"{
        "schema_version":1,"id":"archive-mystery","name":"Archive Mystery","revision":1,
        "properties":{"clue.receipt":{"key":"clue.receipt","kind":"custom","default_value":false,"description":"Receipt clue"}},
        "locations":{"archive":{"id":"archive","name":"Receipt Archive","description":"A quiet records room","tags":["archive"],"connected_locations":[],"map_position":[0.0,0.0]}},
        "characters":{},"events":{},
        "storylets":{"read-receipt":{"id":"read-receipt","revision":1,"title":"The Missing Receipt","summary":"Find a receipt in the archive","approved":true,"location_tags":["archive"],"urgency_weight":50.0,"cluster_type":"player_passage","repeatability":{"kind":"always"},"required_roles":[],"trigger":{"all_of":[],"any_of":[]},"content":{"opening":"A receipt lies between the ledger pages."},"choices":[{"id":"read","label":"Read the receipt","requirements":{"all_of":[],"any_of":[]},"effects":[{"kind":"set_property","key":"clue.receipt","value":true}],"continues_scene":false,"divert_target":null}]}},
        "npc_behaviors":{},"scenarios":{"opening":{"id":"opening","name":"Opening","player_start_location":"archive","initial_properties":{},"actor_locations":{},"storylet_ids":["read-receipt"],"seed":7}}
    }"#;
    let scenario = b"opening";
    let started = unsafe {
        dn_playtest_start(
            handle,
            project.as_ptr(),
            project.len(),
            scenario.as_ptr(),
            scenario.len(),
        )
    };
    let started_value = unsafe { take_json(&started) };
    unsafe { dn_buffer_free(started) };
    assert_eq!(started_value["ok"], true, "{started_value}");

    let evaluation = unsafe { dn_playtest_evaluate(handle, 0.15) };
    let evaluation_value = unsafe { take_json(&evaluation) };
    unsafe { dn_buffer_free(evaluation) };
    assert_eq!(evaluation_value["result"]["selected"], "read-receipt");
    let choice = br#"{"storyletId":"read-receipt","choiceId":"read","idempotencyKey":"native:read-receipt","expectedRevision":0}"#;
    let applied = unsafe { dn_playtest_execute_choice(handle, choice.as_ptr(), choice.len()) };
    let applied_value = unsafe { take_json(&applied) };
    unsafe { dn_buffer_free(applied) };
    assert_eq!(applied_value["ok"], true, "{applied_value}");
    assert_eq!(
        applied_value["result"]["world"]["properties"]["clue.receipt"],
        true
    );

    let snapshot = unsafe { dn_playtest_snapshot(handle) };
    let snapshot_value = unsafe { take_json(&snapshot) };
    unsafe { dn_buffer_free(snapshot) };
    assert_eq!(snapshot_value["result"]["world"]["revision"], 1);

    let exported = unsafe { dn_playtest_export_session(handle) };
    let exported_value = unsafe { take_json(&exported) };
    unsafe { dn_buffer_free(exported) };
    assert_eq!(exported_value["ok"], true, "{exported_value}");
    let export_json = serde_json::to_vec(&exported_value["result"]).unwrap();
    let restored = dn_playtest_create();
    let restored_buffer =
        unsafe { dn_playtest_restore_session(restored, export_json.as_ptr(), export_json.len()) };
    let restored_value = unsafe { take_json(&restored_buffer) };
    unsafe { dn_buffer_free(restored_buffer) };
    assert_eq!(
        restored_value["result"]["world"]["properties"]["clue.receipt"],
        true
    );
    unsafe { dn_playtest_destroy(restored) };
    unsafe { dn_playtest_destroy(handle) };
}

#[path = "support/weirs_crossing.rs"]
mod weirs_crossing;

#[test]
fn ffi_run_round_trip() {
    use std::ffi::{CStr, CString};
    use storylet_engine::ffi::{dn_run_act, dn_run_free, dn_run_rewind, dn_run_start, dn_run_view, dn_string_free};
    let content = CString::new(serde_json::to_string(&weirs_crossing::content()).unwrap()).unwrap();
    let start = CString::new(r#"{"kind":"beginning"}"#).unwrap();
    let mut error = std::ptr::null_mut();
    let run = unsafe { dn_run_start(content.as_ptr(), start.as_ptr(), 7, &mut error) };
    assert!(error.is_null());
    assert!(!run.is_null());
    let read = |pointer: *mut std::ffi::c_char| { assert!(!pointer.is_null()); let value: Value = serde_json::from_str(unsafe { CStr::from_ptr(pointer) }.to_str().unwrap()).unwrap(); unsafe { dn_string_free(pointer) }; value };
    let first = read(unsafe { dn_run_view(run, 0) });
    let action = CString::new(first["scene"]["choices"][0]["action"].to_string()).unwrap();
    let acted = read(unsafe { dn_run_act(run, 0, action.as_ptr()) });
    assert_eq!(acted["currentStep"], 1);
    assert_eq!(unsafe { dn_run_rewind(run, 0) }, 0);
    assert_eq!(read(unsafe { dn_run_view(run, 0) })["currentStep"], 0);
    unsafe { dn_run_free(run) };
}

#[test]
fn ffi_invalid_action_returns_error_json() {
    use std::ffi::{CStr, CString};
    use storylet_engine::ffi::{dn_run_act, dn_run_free, dn_run_start, dn_string_free};
    let content = CString::new(serde_json::to_string(&weirs_crossing::content()).unwrap()).unwrap();
    let start = CString::new(r#"{"kind":"beginning"}"#).unwrap();
    let run = unsafe { dn_run_start(content.as_ptr(), start.as_ptr(), 7, std::ptr::null_mut()) };
    assert!(!run.is_null());
    let bad = CString::new(r#"{"kind":"choose","nodeGuid":999,"outputPin":0}"#).unwrap();
    let response = unsafe { dn_run_act(run, 0, bad.as_ptr()) };
    let value: Value = serde_json::from_str(unsafe { CStr::from_ptr(response) }.to_str().unwrap()).unwrap();
    assert!(value["error"].is_string());
    unsafe { dn_string_free(response); dn_run_free(run); }
}

#[test]
fn ffi_start_reports_invalid_json_and_allows_null_cleanup() {
    use std::ffi::{CStr, CString};
    use storylet_engine::ffi::{dn_run_free, dn_run_start, dn_string_free};
    let invalid = CString::new("not json").unwrap();
    let start = CString::new(r#"{"kind":"beginning"}"#).unwrap();
    let mut error = std::ptr::null_mut();
    let handle = unsafe { dn_run_start(invalid.as_ptr(), start.as_ptr(), 0, &mut error) };
    assert!(handle.is_null());
    assert!(!error.is_null());
    let value: Value = serde_json::from_str(unsafe { CStr::from_ptr(error) }.to_str().unwrap()).unwrap();
    assert!(value["error"].is_string());
    unsafe { dn_string_free(error); dn_string_free(std::ptr::null_mut()); dn_run_free(handle); }
}
