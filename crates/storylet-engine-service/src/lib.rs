//! HTTP and persistence host for the host-independent storylet engine.

pub mod chrono_adapter;
pub mod chrono_graph_adapter;
pub mod chrono_playtest;
pub mod contracts;
pub mod http;
pub mod legacy_import;
pub mod mcp;
pub mod projection;
pub mod scenario;
pub mod service;
pub mod store;

pub use http::build_router;
pub use service::StudioService;
pub mod narrative_graph;
pub mod twee;
