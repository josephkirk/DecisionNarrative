//! Stable C ABI for native hosts. Inputs are UTF-8 JSON; outputs are owned buffers released with dn_buffer_free. ABI version: 1.
use crate::{RuntimeSessionState, StoryletRuntime, WorldCommand, WorldState, apply_command};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ptr;

#[repr(C)]
pub struct DnBuffer {
    pub data: *mut u8,
    pub len: usize,
}
pub struct DnEngineHandle {
    state: WorldState,
}

pub struct DnPlaytestHandle {
    runtime: StoryletRuntime,
    session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChoiceRequest {
    storylet_id: String,
    choice_id: String,
    idempotency_key: String,
    expected_revision: u64,
}

fn into_buffer(bytes: Vec<u8>) -> DnBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let result = DnBuffer {
        data: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}
fn json_buffer<T: Serialize>(value: &T) -> DnBuffer {
    serde_json::to_vec(value)
        .map(into_buffer)
        .unwrap_or_else(|_| DnBuffer {
            data: ptr::null_mut(),
            len: 0,
        })
}
fn result_buffer<T: Serialize>(result: Result<T, String>) -> DnBuffer {
    let value = match result {
        Ok(value) => json!({ "ok": true, "result": value }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    };
    json_buffer(&value)
}
unsafe fn read_json<T: for<'de> Deserialize<'de>>(data: *const u8, len: usize) -> Option<T> {
    if data.is_null() || len == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    serde_json::from_slice(bytes).ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn dn_engine_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn dn_engine_create() -> *mut DnEngineHandle {
    Box::into_raw(Box::new(DnEngineHandle {
        state: WorldState::default(),
    }))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_engine_destroy(handle: *mut DnEngineHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_buffer_free(buffer: DnBuffer) {
    if !buffer.data.is_null() {
        let slice = ptr::slice_from_raw_parts_mut(buffer.data, buffer.len);
        drop(unsafe { Box::from_raw(slice) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_engine_import_snapshot(
    handle: *mut DnEngineHandle,
    data: *const u8,
    len: usize,
) -> bool {
    if handle.is_null() {
        return false;
    }
    let Some(state) = (unsafe { read_json::<WorldState>(data, len) }) else {
        return false;
    };
    unsafe {
        (*handle).state = state;
    }
    true
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_engine_export_snapshot(handle: *const DnEngineHandle) -> DnBuffer {
    if handle.is_null() {
        return DnBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
    }
    json_buffer(unsafe { &(*handle).state })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_engine_submit_command(
    handle: *mut DnEngineHandle,
    data: *const u8,
    len: usize,
) -> DnBuffer {
    if handle.is_null() {
        return DnBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
    }
    let Some(command) = (unsafe { read_json::<WorldCommand>(data, len) }) else {
        return DnBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
    };
    let response = match apply_command(unsafe { &mut (*handle).state }, command) {
        Ok(result) => serde_json::json!({"ok":true,"result":result}),
        Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
    };
    json_buffer(&response)
}

/// Create a host handle with no active project or playtest session.
#[unsafe(no_mangle)]
pub extern "C" fn dn_playtest_create() -> *mut DnPlaytestHandle {
    Box::into_raw(Box::new(DnPlaytestHandle {
        runtime: StoryletRuntime::default(),
        session_id: None,
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_destroy(handle: *mut DnPlaytestHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Import a versioned project bundle and start one fresh scenario session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_start(
    handle: *mut DnPlaytestHandle,
    project_data: *const u8,
    project_len: usize,
    scenario_data: *const u8,
    scenario_len: usize,
) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(project) = (unsafe { read_json(project_data, project_len) }) else {
        return result_buffer::<Value>(Err("project bundle is not valid UTF-8 JSON".into()));
    };
    if scenario_data.is_null() || scenario_len == 0 {
        return result_buffer::<Value>(Err("scenario id is empty".into()));
    }
    let scenario_bytes = unsafe { std::slice::from_raw_parts(scenario_data, scenario_len) };
    let Ok(scenario_id) = std::str::from_utf8(scenario_bytes) else {
        return result_buffer::<Value>(Err("scenario id is not UTF-8".into()));
    };
    let session_id = "native-host-session";
    match handle
        .runtime
        .create_session(session_id, project, scenario_id)
    {
        Ok(snapshot) => {
            handle.session_id = Some(session_id.to_owned());
            result_buffer(Ok(snapshot))
        }
        Err(error) => result_buffer::<Value>(Err(error.to_string())),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_snapshot(handle: *const DnPlaytestHandle) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(session_id) = handle.session_id.as_deref() else {
        return result_buffer::<Value>(Err("playtest has not been started".into()));
    };
    result_buffer(
        handle
            .runtime
            .snapshot(session_id)
            .map_err(|error| error.to_string()),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_export_session(handle: *const DnPlaytestHandle) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(session_id) = handle.session_id.as_deref() else {
        return result_buffer::<Value>(Err("playtest has not been started".into()));
    };
    result_buffer(
        handle
            .runtime
            .export_session(session_id)
            .map_err(|error| error.to_string()),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_restore_session(
    handle: *mut DnPlaytestHandle,
    data: *const u8,
    len: usize,
) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(state) = (unsafe { read_json::<RuntimeSessionState>(data, len) }) else {
        return result_buffer::<Value>(Err("runtime session is not valid JSON".into()));
    };
    let session_id = state.session_id.clone();
    match handle.runtime.restore_session(state) {
        Ok(snapshot) => {
            handle.session_id = Some(session_id);
            result_buffer(Ok(snapshot))
        }
        Err(error) => result_buffer::<Value>(Err(error.to_string())),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_evaluate(
    handle: *const DnPlaytestHandle,
    target_tension_slope: f64,
) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(session_id) = handle.session_id.as_deref() else {
        return result_buffer::<Value>(Err("playtest has not been started".into()));
    };
    result_buffer(
        handle
            .runtime
            .evaluate(session_id, target_tension_slope)
            .map_err(|error| error.to_string()),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_submit_command(
    handle: *mut DnPlaytestHandle,
    data: *const u8,
    len: usize,
) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(command) = (unsafe { read_json::<WorldCommand>(data, len) }) else {
        return result_buffer::<Value>(Err("world command is not valid JSON".into()));
    };
    let Some(session_id) = handle.session_id.as_deref() else {
        return result_buffer::<Value>(Err("playtest has not been started".into()));
    };
    result_buffer(
        handle
            .runtime
            .submit_command(session_id, command)
            .map_err(|error| error.to_string()),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_playtest_execute_choice(
    handle: *mut DnPlaytestHandle,
    data: *const u8,
    len: usize,
) -> DnBuffer {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return result_buffer::<Value>(Err("playtest handle is null".into()));
    };
    let Some(choice) = (unsafe { read_json::<ChoiceRequest>(data, len) }) else {
        return result_buffer::<Value>(Err("storylet choice is not valid JSON".into()));
    };
    let Some(session_id) = handle.session_id.as_deref() else {
        return result_buffer::<Value>(Err("playtest has not been started".into()));
    };
    result_buffer(
        handle
            .runtime
            .execute_choice(
                session_id,
                &choice.storylet_id,
                &choice.choice_id,
                &choice.idempotency_key,
                choice.expected_revision,
            )
            .map_err(|error| error.to_string()),
    )
}

/// A native host owns one deterministic run and its immutable content.
pub struct DnRunHandle {
    content: crate::run::RunContent,
    kernel: crate::run::RunKernel,
}

fn into_c_json(value: &impl Serialize) -> *mut std::ffi::c_char {
    match serde_json::to_string(value).ok().and_then(|s| std::ffi::CString::new(s).ok()) {
        Some(value) => value.into_raw(),
        None => ptr::null_mut(),
    }
}

unsafe fn parse_c_json<T: for<'de> Deserialize<'de>>(input: *const std::ffi::c_char) -> Result<T, String> {
    if input.is_null() { return Err("null JSON input".into()); }
    let input = unsafe { std::ffi::CStr::from_ptr(input) }.to_str().map_err(|e| e.to_string())?;
    serde_json::from_str(input).map_err(|e| e.to_string())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_run_start(content_json: *const std::ffi::c_char, start_json: *const std::ffi::c_char, seed: u64, error_out: *mut *mut std::ffi::c_char) -> *mut DnRunHandle {
    if !error_out.is_null() { unsafe { *error_out = ptr::null_mut(); } }
    let result = (|| {
        let mut content: crate::run::RunContent = unsafe { parse_c_json(content_json) }?;
        content.dispatcher.rebuild_index();
        let start: crate::run::RunStart = unsafe { parse_c_json(start_json) }?;
        let kernel = crate::run::RunKernel::start(&content, start, seed).map_err(|e| format!("{e:?}"))?;
        Ok::<_, String>(DnRunHandle { content, kernel })
    })();
    match result {
        Ok(handle) => Box::into_raw(Box::new(handle)),
        Err(error) => {
            if !error_out.is_null() { unsafe { *error_out = into_c_json(&json!({"error": error})); } }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_run_view(handle: *mut DnRunHandle, participant: u16) -> *mut std::ffi::c_char {
    let Some(run) = (unsafe { handle.as_ref() }) else { return into_c_json(&json!({"error":"null run"})); };
    if participant as usize >= run.kernel.world.participants.len() { return into_c_json(&json!({"error":"unknown participant"})); }
    into_c_json(&run.kernel.view(&run.content, participant))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_run_act(handle: *mut DnRunHandle, participant: u16, action_json: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    let Some(run) = (unsafe { handle.as_mut() }) else { return into_c_json(&json!({"error":"null run"})); };
    let action = match unsafe { parse_c_json(action_json) } { Ok(action) => action, Err(error) => return into_c_json(&json!({"error":error})) };
    match run.kernel.act(&run.content, participant, action) {
        Ok(_) => into_c_json(&run.kernel.view(&run.content, participant)),
        Err(error) => into_c_json(&json!({"error":format!("{error:?}")})),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_run_rewind(handle: *mut DnRunHandle, step: u32) -> i32 {
    let Some(run) = (unsafe { handle.as_mut() }) else { return -1 };
    if run.kernel.rewind(step).is_ok() { 0 } else { -1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_run_free(handle: *mut DnRunHandle) {
    if !handle.is_null() { drop(unsafe { Box::from_raw(handle) }); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dn_string_free(value: *mut std::ffi::c_char) {
    if !value.is_null() { drop(unsafe { std::ffi::CString::from_raw(value) }); }
}
