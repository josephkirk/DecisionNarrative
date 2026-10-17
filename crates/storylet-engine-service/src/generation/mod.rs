pub mod audit;
pub mod resources;
pub mod workflow;

pub use audit::{audit_unslop_compliance, summarize_unslop_violations, UnslopAuditResult, UnslopSummary};
pub use resources::{CraftRegistry, StoryCraftResourceReference};
pub use workflow::{get_stage_descriptors, GenerationService, GENERATION_STAGES};
