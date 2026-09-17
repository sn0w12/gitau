use serde::{Deserialize, Serialize};

use crate::domain::changes::ConflictEntry;
use crate::domain::commits::{CommitDetail, CommitSummary};
use crate::domain::ids::{Generation, ObjectId, SnapshotId};
use crate::domain::paths::RelativePath;
use crate::domain::refs::{BranchInfo, TagInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub snapshot_id: SnapshotId,
    pub generation: Generation,
    pub commits: Vec<CommitSummary>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: RelativePath,
    pub revision: ObjectId,
    pub data: Vec<u8>,
    pub binary: bool,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line_no: u32,
    pub commit: ObjectId,
    pub boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameResult {
    pub path: RelativePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<ObjectId>,
    pub lines: Vec<BlameLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoListing {
    pub branches: Vec<BranchInfo>,
    pub tags: Vec<TagInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictReport {
    pub conflicts: Vec<ConflictEntry>,
    /// Present when a merge/rebase/cherry-pick/revert is mid-flight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitWithDetail {
    pub detail: CommitDetail,
}

/// Time window covered by one chart bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BucketSize {
    Hour,
    QuarterDay,
    HalfDay,
    #[default]
    Day,
    TwoDays,
    /// Twice per week (3.5-day buckets).
    HalfWeek,
    Week,
    Month,
    Quarter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartBucket {
    /// UTC epoch seconds of the bucket's inclusive start.
    pub start_seconds: i64,
    /// UTC epoch seconds of the bucket's exclusive end.
    pub end_seconds: i64,
    pub commits: u32,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryChart {
    pub snapshot_id: SnapshotId,
    pub generation: Generation,
    /// The resolved unit after applying auto-selection.
    pub bucket_size: BucketSize,
    /// Chronological; gaps between first and last activity are zero-filled
    /// so the chart shows quiet periods instead of compressing them away.
    pub buckets: Vec<ChartBucket>,
    /// Commits included in the aggregation (merges excluded by default).
    pub total_commits: u32,
    /// True when the walk hit the commit cap and older history is not shown.
    pub truncated: bool,
}
