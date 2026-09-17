use serde::Serialize;

use crate::domain::ids::ObjectId;

/// One lane-to-lane edge segment drawn across a row's vertical band. The
/// edge starts at `from_lane` on this row and lands on `to_lane` on the next
/// row; straight continuations have equal values and are emitted for every
/// pending lane so through-lines span bands whose commit sits elsewhere.
/// Renderers color bends by `to_lane` so a curve matches the lane it joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from_lane: u32,
    pub to_lane: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphRowKind {
    /// Linear-history commit with one first-parent edge.
    Commit,
    /// Two or more parents.
    Merge,
    /// No parents; nothing is drawn below the node.
    Root,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    /// Absolute position in the walk; stable for the operation's lifetime.
    pub index: u64,
    pub id: ObjectId,
    /// Lane the commit's node occupies.
    pub lane: u32,
    pub edges: Vec<GraphEdge>,
    pub kind: GraphRowKind,
    pub summary_line: String,
    pub author_name: String,
    pub author_email: String,
    /// Committer time, epoch seconds (matches CommitSummary timestamps).
    pub time_seconds: i64,
    /// Tag names whose target resolves to this commit.
    pub tags: Vec<String>,
    /// Branch decorations (local and remote-tracking) pointing at this commit.
    pub refs: Vec<String>,
}
