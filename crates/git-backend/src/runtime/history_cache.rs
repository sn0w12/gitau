use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::CommitSummary;

const MAX_CACHED_SUMMARIES: usize = 8192;
const MAX_CACHED_STATS: usize = 131_072;
const MAX_CACHED_WALKS: usize = 32;
pub const MAX_WALK_ENTRIES: usize = 50_000;

/// Commit OIDs are immutable, so this cache intentionally survives
/// repository generation bumps; only `clear` drops it.
#[derive(Default)]
pub struct HistoryCache {
    walks: HashMap<WalkKey, Arc<Vec<Oid>>>,
    summaries: HashMap<[u8; 20], Arc<CommitSummary>>,
    /// Compact per-commit aggregate inputs for the history chart. Kept
    /// separate from `summaries`: those evict-all at 8192 entries, which
    /// would thrash while aggregating whole-history statistics.
    stats: HashMap<[u8; 20], CommitStatPoint>,
    tags: Option<Arc<HashMap<[u8; 20], Vec<String>>>>,
}

pub type Oid = git2::Oid;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WalkKey {
    pub tip: Option<[u8; 20]>,
    pub exclusions: Vec<[u8; 20]>,
}

/// Everything the chart aggregation needs from one commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitStatPoint {
    pub time_seconds: i64,
    pub additions: u64,
    pub deletions: u64,
    /// Merge commits are stored with zero line deltas (never diffed) so
    /// `include_merges` toggles only affect the commit count.
    pub is_merge: bool,
}

impl HistoryCache {
    pub fn walk(&self, key: &WalkKey) -> Option<Arc<Vec<Oid>>> {
        self.walks.get(key).cloned()
    }

    pub fn store_walk(&mut self, key: WalkKey, list: Arc<Vec<Oid>>) {
        if self.walks.len() >= MAX_CACHED_WALKS {
            // Arbitrary eviction; walks are cheap relative to stats.
            self.walks.clear();
        }
        self.walks.insert(key, list);
    }

    pub fn summary(&self, oid: &[u8; 20]) -> Option<Arc<CommitSummary>> {
        self.summaries.get(oid).cloned()
    }

    pub fn store_summary(&mut self, oid: [u8; 20], summary: Arc<CommitSummary>) {
        if self.summaries.len() >= MAX_CACHED_SUMMARIES && !self.summaries.contains_key(&oid) {
            self.summaries.clear();
        }
        self.summaries.insert(oid, summary);
    }

    /// `Copy`ed out so callers never hold a borrow into the map.
    pub fn stat(&self, oid: &[u8; 20]) -> Option<CommitStatPoint> {
        self.stats.get(oid).copied()
    }

    pub fn store_stat(&mut self, oid: [u8; 20], point: CommitStatPoint) {
        if self.stats.len() >= MAX_CACHED_STATS && !self.stats.contains_key(&oid) {
            self.stats.clear();
        }
        self.stats.insert(oid, point);
    }

    pub fn tags(&self) -> Option<Arc<HashMap<[u8; 20], Vec<String>>>> {
        self.tags.clone()
    }

    pub fn store_tags(&mut self, tags: Arc<HashMap<[u8; 20], Vec<String>>>) {
        self.tags = Some(tags);
    }

    pub fn clear(&mut self) {
        self.walks.clear();
        self.summaries.clear();
        self.stats.clear();
        self.tags = None;
    }
}
