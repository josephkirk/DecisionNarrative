#[path = "support/weirs_crossing.rs"]
mod weirs_crossing;

use serde_json::Value;
use std::{ffi::{CStr, CString, c_char}, ptr};
use storylet_engine::{ActionProposal, Authority, RunAction, RunStart, ffi};

unsafe fn take_string(value: *mut c_char) -> Value {
    let text = unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned();
    unsafe { ffi::dn_string_free(value) };
    serde_json::from_str(&text).unwrap()
}

#[test]
fn abi_v2_starts_proposes_observes_and_explains() {
    assert_eq!(ffi::dn_engine_abi_version(), 2);
    let mut content = weirs_crossing::explorer_content();
    content.dispatcher.rebuild_index();
    let content = CString::new(serde_json::to_string(&content).unwrap()).unwrap();
    let start = CString::new(serde_json::to_string(&RunStart::Beginning).unwrap()).unwrap();
    let mut error = ptr::null_mut();
    let run = unsafe { ffi::dn_run_start(content.as_ptr(), start.as_ptr(), 7, &mut error) };
    assert!(!run.is_null());
    let observation = unsafe { take_string(ffi::dn_run_observe(run, 0)) };
    assert_eq!(observation["actor"], 0);
    let view = unsafe { take_string(ffi::dn_run_architect_view(run, 0)) };
    let action: RunAction = serde_json::from_value(view["scene"]["choices"][0]["action"].clone()).unwrap();
    let proposal = ActionProposal { schema_version: 1, actor: 0, action, expected_revision: 0, idempotency_key: "ffi-v2".into(), provenance: None };
    let input = CString::new(serde_json::to_string(&serde_json::json!({"authority":Authority::Human { seat:0 },"proposal":proposal})).unwrap()).unwrap();
    let receipt = unsafe { take_string(ffi::dn_run_propose(run, input.as_ptr())) };
    assert_eq!(receipt["ok"], true);
    let chain = unsafe { take_string(ffi::dn_run_explain(run, receipt["result"]["event_id"].as_u64().unwrap() as u32)) };
    assert!(chain["event"].as_array().is_some_and(|events| !events.is_empty()));
    unsafe { ffi::dn_run_free(run) };
}
