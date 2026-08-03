use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingOperation {
    New,
    Rebase,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingState {
    pub schema: u32,
    pub operation: PendingOperation,
    pub expected_remote_sha: String,
    pub old_base: String,
    pub old_tip: String,
    pub backup_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_tip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
}

impl PendingState {
    pub fn new(
        operation: PendingOperation,
        expected_remote_sha: String,
        old_base: String,
        old_tip: String,
        backup_tag: String,
    ) -> Self {
        Self {
            schema: 1,
            operation,
            expected_remote_sha,
            old_base,
            old_tip,
            backup_tag,
            target_label: None,
            target_sha: None,
            new_base: None,
            new_tip: None,
            report: None,
        }
    }
}
