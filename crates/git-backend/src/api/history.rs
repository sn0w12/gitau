use crate::domain::RevisionSpec;
use crate::domain::history::BucketSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryPageQuery {
    /// Tip of the walk.
    pub revision: Option<RevisionSpec>,
    pub limit: u32,
    pub skip: u64,
    /// Hide commits reachable from these revisions.
    pub exclude_reachable_from: Vec<RevisionSpec>,
    /// Only commits touching this path.
    pub path: Option<String>,
    /// Case-insensitive filter against commit message and author: each
    /// whitespace-separated term must substring-match a field or
    /// fuzzy-match inside a single word.
    pub search: Option<String>,
}

impl Default for HistoryPageQuery {
    fn default() -> Self {
        Self {
            revision: None,
            limit: 100,
            skip: 0,
            exclude_reachable_from: vec![],
            path: None,
            search: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetailQuery {
    pub revision: RevisionSpec,
    pub detect_renames: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileAtRevisionQuery {
    pub revision: RevisionSpec,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BlameRange {
    #[default]
    Whole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BlameQuery {
    pub revision: Option<RevisionSpec>,
    pub path: String,
    pub ignore_whitespace: bool,
    #[serde(flatten)]
    pub range: BlameRange,
}

impl Default for BlameQuery {
    fn default() -> Self {
        Self {
            revision: None,
            path: String::new(),
            ignore_whitespace: false,
            range: BlameRange::Whole,
        }
    }
}

/// Aggregated history for the chart view. Every field is optional so callers
/// can finetune a single knob without repeating the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryChartQuery {
    /// Tip of the walk (defaults to HEAD).
    pub revision: Option<RevisionSpec>,
    /// Hide commits reachable from these revisions.
    pub exclude_reachable_from: Vec<RevisionSpec>,
    /// Fixed bucket unit; `None` auto-selects purely from the time since the
    /// first commit: 10y+ monthly, 6y+ weekly, 3y+ half-weekly, 1y+ two-day,
    /// 180d+ daily, 90d+ half-daily, 14d+ quarter-daily, younger hourly.
    pub bucket: Option<BucketSize>,
    /// Walk cap; older commits past this are not charted.
    pub max_commits: Option<u32>,
    /// Merge commits contribute nothing when false (default): no commit
    /// count and no diff stats, avoiding double-counted branch lines.
    pub include_merges: bool,
}

pub const DEFAULT_MAX_CHART_COMMITS: u32 = 10_000;

impl HistoryChartQuery {
    pub fn max_commits(&self) -> u32 {
        self.max_commits
            .unwrap_or(DEFAULT_MAX_CHART_COMMITS)
            .clamp(1, crate::runtime::history_cache::MAX_WALK_ENTRIES as u32)
    }
}
