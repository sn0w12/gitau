use serde::{Deserialize, Serialize};

use crate::domain::RevisionSpec;

/// Tip and exclusions for a commit-graph stream. Mirrors the paging-free
/// subset of HistoryPageQuery: the graph always walks the whole history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphQuery {
    /// Tip of the walk (defaults to HEAD).
    pub revision: Option<RevisionSpec>,
    /// Hide commits reachable from these revisions.
    pub exclude_reachable_from: Vec<RevisionSpec>,
}
