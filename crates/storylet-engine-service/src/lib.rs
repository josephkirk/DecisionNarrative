#![recursion_limit = "256"]

pub mod chrono_adapter;
pub mod chrono_graph_adapter;
pub mod contracts;
pub mod entities;
pub mod files;
pub mod http;
pub mod inventory;
pub mod mcp;
pub mod references;
pub mod run_content;
pub mod scenario;
pub mod service;
pub mod store;
pub mod tree_graph;
pub mod tree_pointer;

pub use http::build_router;
pub use service::StudioService;
pub mod narrative_graph;
pub mod runs;
