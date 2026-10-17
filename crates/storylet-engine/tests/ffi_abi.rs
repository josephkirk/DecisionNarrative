use serde_json::Value;
use storylet_engine::ffi::{
    dn_buffer_free, dn_engine_abi_version, dn_engine_create, dn_engine_destroy,
    dn_engine_export_snapshot, dn_engine_import_snapshot, dn_playtest_apply_decision,
    dn_playtest_create, dn_playtest_destroy, dn_playtest_evaluate, dn_playtest_execute_choice,
    dn_playtest_export_session, dn_playtest_rank_semantic, dn_playtest_restore_session,
    dn_playtest_snapshot, dn_playtest_start,
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
        "qualities":{"clue.receipt":{"key":"clue.receipt","kind":"custom","default_value":false,"description":"Receipt clue"}},
        "locations":{"archive":{"id":"archive","name":"Receipt Archive","description":"A quiet records room","tags":["archive"],"connected_locations":[],"map_position":[0.0,0.0]}},
        "characters":{},"events":{},
        "storylets":{"read-receipt":{"id":"read-receipt","revision":1,"title":"The Missing Receipt","summary":"Find a receipt in the archive","approved":true,"location_tags":["archive"],"urgency_weight":50.0,"cluster_type":"player_controlled_passage","repeatability":{"kind":"always"},"required_roles":[],"trigger":{"all_of":[],"any_of":[]},"content":{"opening":"A receipt lies between the ledger pages."},"choices":[{"id":"read","label":"Read the receipt","requirements":{"all_of":[],"any_of":[]},"effects":[{"kind":"set_quality","key":"clue.receipt","value":true}],"continues_scene":false,"divert_target":null}]}},
        "npc_behaviors":{},"scenarios":{"opening":{"id":"opening","name":"Opening","player_start_location":"archive","initial_qualities":{},"actor_locations":{},"storylet_ids":["read-receipt"],"seed":7}}
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

    let semantic = br#"{"formatVersion":1,"contextRevision":0,"modelVersion":"fixture@rev-1","queryEmbedding":[1.0,0.0],"candidates":[{"storyletId":"read-receipt","embedding":[1.0,0.0]}]}"#;
    let ranked =
        unsafe { dn_playtest_rank_semantic(handle, semantic.as_ptr(), semantic.len(), 0.15) };
    let ranked_value = unsafe { take_json(&ranked) };
    unsafe { dn_buffer_free(ranked) };
    assert_eq!(ranked_value["ok"], true, "{ranked_value}");
    assert_eq!(
        ranked_value["result"]["decision"]["source"],
        "embedding_baseline_uncalibrated"
    );

    let decision = br#"{"formatVersion":1,"contextRevision":0,"source":"native-host","modelVersion":"fixture-1","confidence":0.8,"noActionProbability":0.1,"ranking":[{"storyletId":"read-receipt","score":0.9,"probability":0.9}]}"#;
    let resolved =
        unsafe { dn_playtest_apply_decision(handle, decision.as_ptr(), decision.len(), 0.15) };
    let resolved_value = unsafe { take_json(&resolved) };
    unsafe { dn_buffer_free(resolved) };
    assert_eq!(resolved_value["result"]["selected"], "read-receipt");

    let choice = br#"{"storyletId":"read-receipt","choiceId":"read","idempotencyKey":"native:read-receipt","expectedRevision":0}"#;
    let applied = unsafe { dn_playtest_execute_choice(handle, choice.as_ptr(), choice.len()) };
    let applied_value = unsafe { take_json(&applied) };
    unsafe { dn_buffer_free(applied) };
    assert_eq!(applied_value["ok"], true, "{applied_value}");
    assert_eq!(
        applied_value["result"]["world"]["qualities"]["clue.receipt"],
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
        restored_value["result"]["world"]["qualities"]["clue.receipt"],
        true
    );
    unsafe { dn_playtest_destroy(restored) };
    unsafe { dn_playtest_destroy(handle) };
}
