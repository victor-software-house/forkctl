use crate::manifest::{BaseTarget, Patch, RecoveryEvidence};
use crate::protocol::CaptureSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivePatchState {
    Draft { metadata: Patch },
    Existing { patch: String },
}

impl ActivePatchState {
    pub fn name(&self) -> &str {
        match self {
            Self::Draft { metadata } => &metadata.name,
            Self::Existing { patch } => patch,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    PatchRefresh,
    PatchEdit,
    PatchRemove,
    PatchDisable,
    PatchEnable,
    Rebase,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchCommitEvidence {
    pub name: String,
    pub commit: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportEvidence {
    pub path: String,
    pub object_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationIntent {
    Edit {
        patch: Patch,
    },
    Refresh {
        patch: Patch,
        capture: CaptureSource,
        captured_paths: Vec<String>,
    },
    Transition {
        patch: Patch,
        commit: String,
        position: usize,
        reason: String,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationState {
    pub schema: u32,
    pub id: String,
    pub kind: OperationKind,
    pub phase: String,
    pub started_at_unix_ms: u128,
    pub expected_remote_sha: String,
    pub old_base: String,
    pub old_tip: String,
    pub old_patches: Vec<PatchCommitEvidence>,
    pub recovery: RecoveryEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_recovery: Option<RecoveryEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<OperationIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<BaseTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_tip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportEvidence>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}
