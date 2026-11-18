use crate::{RunAction, chrono_participants::PartIx};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionProvenance {
    pub policy_id: String,
    pub policy_version: String,
    pub model_id: String,
    pub context_hash: String,
    pub decision_seq: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProposal {
    pub schema_version: u32,
    pub actor: PartIx,
    pub action: RunAction,
    pub expected_revision: u64,
    pub idempotency_key: String,
    #[serde(default)]
    pub provenance: Option<DecisionProvenance>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Authority {
    Human { seat: u8 },
    Policy,
    Architect,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionReceipt {
    pub event_id: u32,
    pub revision: u64,
    pub step: u32,
    pub duplicate: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRejection {
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewindRequest {
    pub to_step: u32,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunOperation {
    #[serde(rename = "act")]
    Action {
        authority: Authority,
        proposal: ActionProposal,
    },
    Rewind {
        authority: Authority,
        request: RewindRequest,
    },
}
