//! Chrono RunKernel C ABI. JSON arguments and results are UTF-8 C strings.
use serde::{Deserialize,Serialize};
use serde_json::json;
use std::ptr;
pub struct DnRunHandle { content:crate::RunContent, kernel:crate::RunKernel }
fn into_json(value:&impl Serialize)->*mut std::ffi::c_char { serde_json::to_string(value).ok().and_then(|s|std::ffi::CString::new(s).ok()).map_or(ptr::null_mut(),|s|s.into_raw()) }
unsafe fn parse<T:for<'de>Deserialize<'de>>(p:*const std::ffi::c_char)->Result<T,String>{if p.is_null(){return Err("null JSON input".into())}let s=unsafe{std::ffi::CStr::from_ptr(p)}.to_str().map_err(|e|e.to_string())?;serde_json::from_str(s).map_err(|e|e.to_string())}
#[derive(Deserialize)] struct ProposalInput { authority:crate::Authority, proposal:crate::ActionProposal }
#[unsafe(no_mangle)] pub extern "C" fn dn_engine_abi_version()->u32{2}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_start(c:*const std::ffi::c_char,s:*const std::ffi::c_char,seed:u64,error:*mut *mut std::ffi::c_char)->*mut DnRunHandle{if !error.is_null(){unsafe{*error=ptr::null_mut()}}let r=(||{let mut content:crate::RunContent=unsafe{parse(c)}?;content.dispatcher.rebuild_index();let start:crate::RunStart=unsafe{parse(s)}?;let kernel=crate::RunKernel::start(&content,start,seed).map_err(|e|format!("{e:?}"))?;Ok::<_,String>(DnRunHandle{content,kernel})})();match r{Ok(h)=>Box::into_raw(Box::new(h)),Err(e)=>{if !error.is_null(){unsafe{*error=into_json(&json!({"error":e}))}}ptr::null_mut()}}}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_propose(h:*mut DnRunHandle,input:*const std::ffi::c_char)->*mut std::ffi::c_char{let Some(h)=(unsafe{h.as_mut()})else{return into_json(&json!({"error":"null run"}))};let i:ProposalInput=match unsafe{parse(input)}{Ok(v)=>v,Err(e)=>return into_json(&json!({"error":e}))};match h.kernel.propose(&h.content,i.authority,i.proposal){Ok(r)=>into_json(&json!({"ok":true,"result":r})),Err(e)=>into_json(&json!({"ok":false,"error":e}))}}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_observe(h:*const DnRunHandle,actor:u16)->*mut std::ffi::c_char{let Some(h)=(unsafe{h.as_ref()})else{return into_json(&json!({"error":"null run"}))};into_json(&h.kernel.observe(&h.content,actor))}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_explain(h:*const DnRunHandle,id:u64)->*mut std::ffi::c_char{let Some(h)=(unsafe{h.as_ref()})else{return into_json(&json!({"error":"null run"}))};into_json(&json!({"event":h.kernel.explain(id)}))}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_architect_view(h:*const DnRunHandle,actor:u16)->*mut std::ffi::c_char{let Some(h)=(unsafe{h.as_ref()})else{return into_json(&json!({"error":"null run"}))};into_json(&h.kernel.architect_view(&h.content,actor))}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_run_free(h:*mut DnRunHandle){if !h.is_null(){drop(unsafe{Box::from_raw(h)})}}
#[unsafe(no_mangle)] pub unsafe extern "C" fn dn_string_free(s:*mut std::ffi::c_char){if !s.is_null(){drop(unsafe{std::ffi::CString::from_raw(s)})}}
