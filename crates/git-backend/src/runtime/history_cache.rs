use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::CommitSummary;
use crate::domain::history::BucketSize;

const MAX_CACHED_SUMMARIES: usize = 8192;
/// Sized for a very large repository's full history rather than a typical
/// page's working set. Past this the map is dropped wholesale, which only
/// costs recomputation because every entry is immutable.
const MAX_CACHED_STATS: usize = 1 << 21;
const MAX_CACHED_WALKS: usize = 32;
/// Pagination bound for the history list. The chart walks the full history
/// and may leave a longer list behind, which pages then slice.
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
    /// Read once per process; the points file only ever grows by oid, so a
    /// second read would find the same commits.
    points_loaded: bool,
    /// Tip and walk order of the latest aggregation. The points file is
    /// written from this so the file keeps the order the chart consumed.
    chart_walk: Option<(Vec<[u8; 20]>, [u8; 20])>,
    /// Aggregates computed since the last flush.
    pending_charts: Vec<PendingChart>,
}

/// A finished aggregate waiting to be written to the chart cache.
pub struct PendingChart {
    pub tip: [u8; 20],
    pub exclusions: u64,
    /// What the caller asked for, which is what the file is named after. It
    /// differs from `resolved_bucket` whenever auto-selection ran, and writing
    /// to the resolved name would leave the auto request permanently missing.
    pub requested_bucket: Option<BucketSize>,
    pub resolved_bucket: BucketSize,
    pub include_merges: bool,
    pub total_commits: u32,
    pub buckets: Vec<crate::domain::history::ChartBucket>,
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

    /// Inserts a batch recovered from disk. The map is emptied once up front
    /// rather than repeatedly mid-insert, so a history larger than the cap
    /// settles into one coherent map instead of clearing on every commit and
    /// never converging.
    pub fn extend_stats(&mut self, entries: Vec<([u8; 20], CommitStatPoint)>) {
        if entries.is_empty() {
            return;
        }
        if self.stats.len() + entries.len() > MAX_CACHED_STATS {
            self.stats.clear();
        }
        self.stats.extend(entries);
    }

    /// Cached points for `oids` in walk order, skipping commits with no entry.
    /// This is what the on-disk copy is written from, so the file preserves
    /// the walk order the chart aggregated.
    pub fn stats_in_walk_order(&self, oids: &[[u8; 20]]) -> Vec<([u8; 20], CommitStatPoint)> {
        oids.iter()
            .filter_map(|bytes| self.stats.get(bytes).map(|point| (*bytes, *point)))
            .collect()
    }

    /// True exactly once per cache, for the caller that should merge the
    /// on-disk points file in.
    pub fn claim_points_load(&mut self) -> bool {
        if self.points_loaded {
            return false;
        }
        self.points_loaded = true;
        true
    }

    pub fn record_chart(
        &mut self,
        tip: [u8; 20],
        walk_order: Vec<[u8; 20]>,
        pending: PendingChart,
    ) {
        self.chart_walk = Some((walk_order, tip));
        self.pending_charts.push(pending);
    }

    pub fn chart_walk(&self) -> Option<([u8; 20], &[[u8; 20]])> {
        self.chart_walk
            .as_ref()
            .map(|(order, tip)| (*tip, order.as_slice()))
    }

    pub fn pending_charts(&self) -> Vec<((Option<BucketSize>, bool), &PendingChart)> {
        self.pending_charts
            .iter()
            .map(|chart| ((chart.requested_bucket, chart.include_merges), chart))
            .collect()
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
        self.pending_charts.clear();
        // Forgetting everything means the next chart has to read the points
        // file again, otherwise a cleared cache silently recomputes.
        self.points_loaded = false;
    }

    /// Drops everything derived from ref tips: the tag map plus the
    /// summaries that bake tag names in. Walks re-key on the tip OID and
    /// per-commit stats are immutable, so both survive. The watcher calls
    /// this when a burst touches `.git` ref tips, covering tags, branches,
    /// and checkouts created outside the app.
    pub fn clear_ref_dependent(&mut self) {
        self.summaries.clear();
        self.tags = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commits::{CommitSummary, Signature};
    use crate::domain::ids::ObjectId;

    fn summary(id_byte: u8) -> Arc<CommitSummary> {
        let id = ObjectId::from_bytes(&[id_byte; 20]).unwrap();
        Arc::new(CommitSummary {
            id,
            tree_id: id,
            parent_ids: vec![],
            summary_line: "subject".into(),
            message: "subject".into(),
            author: Signature {
                name: "author".into(),
                email: String::new(),
                time_seconds: 0,
                time_offset_minutes: 0,
            },
            committer: Signature {
                name: "author".into(),
                email: String::new(),
                time_seconds: 0,
                time_offset_minutes: 0,
            },
            files_changed: 0,
            additions: 0,
            deletions: 0,
            tags: vec!["v0.1.0".into()],
            match_ranges: None,
        })
    }

    #[test]
    fn ref_clear_drops_tags_and_summaries_but_keeps_walks_and_stats() {
        let mut cache = HistoryCache::default();
        let oid = [7u8; 20];
        cache.store_walk(
            WalkKey {
                tip: Some(oid),
                exclusions: vec![],
            },
            Arc::new(vec![Oid::from_bytes(&oid).unwrap()]),
        );
        cache.store_summary(oid, summary(7));
        cache.store_stat(
            oid,
            CommitStatPoint {
                time_seconds: 0,
                additions: 1,
                deletions: 2,
                is_merge: false,
            },
        );
        let mut tags = HashMap::new();
        tags.insert(oid, vec!["v0.1.0".to_owned()]);
        cache.store_tags(Arc::new(tags));

        cache.clear_ref_dependent();

        assert!(cache.tags().is_none());
        assert!(cache.summary(&oid).is_none());
        assert!(
            cache
                .walk(&WalkKey {
                    tip: Some(oid),
                    exclusions: vec![]
                })
                .is_some()
        );
        assert!(cache.stat(&oid).is_some());
    }
}
