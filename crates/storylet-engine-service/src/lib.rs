//! HTTP and persistence host for the host-independent storylet engine.

pub mod contracts;
pub mod store;
pub mod scenario;
pub mod projection;
pub mod legacy_import;
pub mod generation;
pub mod ranking;
pub mod service;
pub mod http;
pub mod mcp;

pub use service::StudioService;
pub use http::build_router;
