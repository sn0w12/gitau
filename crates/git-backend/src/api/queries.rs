use serde::{Deserialize, Serialize};

use crate::streaming::model::DiffComparison;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiffRequest {
    pub comparison: DiffComparison,
    /// Context lines around hunks.
    pub context_lines: u32,
    pub interhunk_lines: u32,
    pub detect_renames: bool,
    pub detect_copies: bool,
    /// Restrict to these repository-relative paths.
    pub paths: Vec<String>,
    pub ignore_whitespace: bool,
}

impl Default for DiffRequest {
    fn default() -> Self {
        Self {
            comparison: DiffComparison::WorkingTree,
            context_lines: 3,
            interhunk_lines: 0,
            detect_renames: true,
            detect_copies: false,
            paths: vec![],
            ignore_whitespace: false,
        }
    }
}

/// Handle returned immediately after opening a diff stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHandle {
    pub operation_id: u64,
}
