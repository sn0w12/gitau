use git2::Oid;

use crate::domain::graph::{GraphEdge, GraphRowKind};

/// Sequential lane state for one graph walk. Children always precede parents
/// because the walk sorts topologically, so a commit's slot (if any) is known
/// when the commit is reached.
///
/// `columns[lane]` holds the parent oid expected to appear on that lane next;
/// `free` collects vacated lanes for reuse so long-lived branches keep stable
/// colors while dead lanes do not accumulate.
pub struct LaneTracker<T> {
    columns: Vec<Option<T>>,
    free: std::collections::BTreeSet<usize>,
}

pub struct LaneStep {
    pub lane: u32,
    pub edges: Vec<GraphEdge>,
    pub kind: GraphRowKind,
}

impl<T: Copy + PartialEq> LaneTracker<T> {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            free: std::collections::BTreeSet::new(),
        }
    }

    fn allocate(&mut self) -> usize {
        match self.free.pop_first() {
            Some(lane) => lane,
            None => {
                self.columns.push(None);
                self.columns.len() - 1
            }
        }
    }

    fn occupy(&mut self, lane: usize) {
        self.free.remove(&lane);
    }

    fn position_of(&self, id: T) -> Option<usize> {
        self.columns.iter().position(|slot| *slot == Some(id))
    }

    /// Advances the tracker past one commit and returns its lane, the edge
    /// segments spanning this row's band, and the row kind. Segments cover
    /// every occupied lane: the commit's own edges plus straight
    /// continuations for pending lanes passing through, so other branches'
    /// lines never break at rows whose commit sits elsewhere.
    pub fn step(&mut self, id: T, parents: &[T]) -> LaneStep {
        // A commit that was already pending dies here (its child just
        // consumed it); a fresh tip carves out a slot that survives only if
        // its first parent continues on it.
        let (lane, was_pending) = match self.position_of(id) {
            Some(found) => {
                self.columns[found] = None;
                self.free.insert(found);
                (found, true)
            }
            None => (self.allocate(), false),
        };

        let kind;
        let mut edges;
        // Lanes allocated for this row's own merge parents; the fan-out edge
        // already supplies their line in this band, so no continuation.
        let mut fresh_lanes: Vec<u32> = Vec::new();
        if parents.is_empty() {
            kind = GraphRowKind::Root;
            edges = Vec::new();
        } else {
            kind = if parents.len() > 1 {
                GraphRowKind::Merge
            } else {
                GraphRowKind::Commit
            };
            edges = Vec::with_capacity(parents.len());

            // First parent: continues straight down this lane unless that
            // parent is already pending on another lane (criss-cross and
            // fork-point histories), in which case the edge bends over and
            // this lane dies.
            let first = parents[0];
            match self.position_of(first) {
                Some(target) => {
                    if !was_pending {
                        self.free.insert(lane);
                    }
                    edges.push(GraphEdge {
                        from_lane: lane as u32,
                        to_lane: target as u32,
                    });
                }
                None => {
                    self.columns[lane] = Some(first);
                    self.occupy(lane);
                    edges.push(GraphEdge {
                        from_lane: lane as u32,
                        to_lane: lane as u32,
                    });
                }
            }

            // Merge parents: join an existing pending lane when one matches,
            // else allocate a fresh lane for the new branch line.
            for parent in &parents[1..] {
                match self.position_of(*parent) {
                    Some(target) => edges.push(GraphEdge {
                        from_lane: lane as u32,
                        to_lane: target as u32,
                    }),
                    None => {
                        let allocated = self.allocate();
                        self.columns[allocated] = Some(*parent);
                        self.occupy(allocated);
                        fresh_lanes.push(allocated as u32);
                        edges.push(GraphEdge {
                            from_lane: lane as u32,
                            to_lane: allocated as u32,
                        });
                    }
                }
            }
        }

        for (index, slot) in self.columns.iter().enumerate() {
            if slot.is_none() {
                continue;
            }
            let through = index as u32;
            if fresh_lanes.contains(&through) {
                continue;
            }
            // A straight first-parent edge already spans this band.
            if index == lane
                && edges
                    .iter()
                    .any(|edge| edge.from_lane == edge.to_lane && edge.to_lane == through)
            {
                continue;
            }
            edges.push(GraphEdge {
                from_lane: through,
                to_lane: through,
            });
        }

        LaneStep {
            lane: lane as u32,
            edges,
            kind,
        }
    }

    /// Highest lane currently or previously occupied; renderers size the
    /// gutter from this.
    pub fn lane_count(&self) -> usize {
        self.columns.len()
    }
}

impl<T: Copy + PartialEq> Default for LaneTracker<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves the walk's OID list through the shared history cache, using the
/// same tip/exclusion semantics and cache identity as history pages.
pub fn walk_oids(
    session: &crate::engines::git2::session::Git2Session,
    query: &crate::api::graph::GraphQuery,
    cache: &mut crate::runtime::history_cache::HistoryCache,
) -> crate::error::Result<std::sync::Arc<Vec<Oid>>> {
    use crate::error::GitError;

    session.with_repository(|repo| {
        // Unborn HEAD walks nothing; the pipeline completes the stream with
        // zero rows instead of failing it with `invalid revision HEAD`.
        let tip = if crate::engines::git2::history::is_default_revision(query.revision.as_ref()) {
            match crate::engines::git2::history::default_tip(repo)? {
                Some(tip) => tip,
                None => return Ok(std::sync::Arc::new(Vec::new())),
            }
        } else {
            crate::engines::git2::history::resolve_tip(repo, query.revision.as_ref())?
        };
        let mut exclusions = Vec::with_capacity(query.exclude_reachable_from.len());
        for spec in &query.exclude_reachable_from {
            let hidden = crate::engines::git2::history::resolve_tip(repo, Some(spec))?;
            exclusions.push(crate::engines::git2::history::oid_bytes(hidden));
        }
        let walk_key = crate::runtime::history_cache::WalkKey {
            tip: Some(crate::engines::git2::history::oid_bytes(tip)),
            exclusions,
        };
        if let Some(list) = cache.walk(&walk_key) {
            return Ok(list);
        }
        let mut walk = repo.revwalk()?;
        walk.push(tip)?;
        // Same ordering contract as history pages: children before parents.
        walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;
        for spec in &query.exclude_reachable_from {
            let hidden = crate::engines::git2::history::resolve_tip(repo, Some(spec))?;
            walk.hide(hidden)?;
        }
        let mut oids = Vec::new();
        for info in walk {
            if oids.len() >= crate::runtime::history_cache::MAX_WALK_ENTRIES {
                break;
            }
            oids.push(info.map_err(|e| GitError::Internal {
                message: e.to_string(),
            })?);
        }
        let list = std::sync::Arc::new(oids);
        cache.store_walk(walk_key, list.clone());
        Ok(list)
    })
}

/// Branch decorations for the whole repo: peeled commit oid -> sorted names
/// from refs/heads and refs/remotes.
pub fn branch_decorations(
    session: &crate::engines::git2::session::Git2Session,
) -> crate::error::Result<std::collections::HashMap<[u8; 20], Vec<String>>> {
    session.with_repository(|repo| {
        let mut map: std::collections::HashMap<[u8; 20], Vec<String>> =
            std::collections::HashMap::new();
        for pattern in ["refs/heads/*", "refs/remotes/*"] {
            let references = repo.references_glob(pattern)?;
            for reference in references {
                let reference = reference?;
                let Ok(name) = reference.shorthand() else {
                    continue;
                };
                let name = name.to_owned();
                // Skip tags-in-refs or symbolic entries that do not peel to a
                // commit.
                let Ok(commit) = reference.peel_to_commit() else {
                    continue;
                };
                map.entry(crate::engines::git2::history::oid_bytes(commit.id()))
                    .or_default()
                    .push(name);
            }
        }
        for names in map.values_mut() {
            names.sort();
        }
        Ok(map)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[0] = n;
        Oid::from_bytes(&bytes).unwrap()
    }

    fn lanes(step: &LaneStep) -> Vec<(u32, u32)> {
        step.edges
            .iter()
            .map(|edge| (edge.from_lane, edge.to_lane))
            .collect()
    }

    #[test]
    fn linear_history_stays_on_lane_zero() {
        let mut tracker = LaneTracker::new();
        // a(root) <- b <- c, walked newest first: c, b, a.
        let step = tracker.step(oid(3), &[oid(2)]);
        assert_eq!(step.lane, 0);
        assert_eq!(lanes(&step), vec![(0, 0)]);
        let step = tracker.step(oid(2), &[oid(1)]);
        assert_eq!(step.lane, 0);
        let step = tracker.step(oid(1), &[]);
        assert_eq!(step.lane, 0);
        assert_eq!(step.kind, GraphRowKind::Root);
    }

    #[test]
    fn branch_allocates_and_frees_a_lane() {
        let mut tracker = LaneTracker::new();
        // main: c1 <- c4; side: c1 <- c2 <- c3 (tip walked first).
        // Walk: c4(main), c3(side tip), c2, c1.
        let step = tracker.step(oid(4), &[oid(1)]);
        assert_eq!(step.lane, 0);
        assert_eq!(lanes(&step), vec![(0, 0)]);

        let step = tracker.step(oid(3), &[oid(2)]);
        assert_eq!(step.lane, 1, "branch tip gets a fresh lane");
        // Lane 0 (main's pending parent) passes straight through.
        assert_eq!(lanes(&step), vec![(1, 1), (0, 0)]);

        let step = tracker.step(oid(2), &[oid(1)]);
        assert_eq!(step.lane, 1);
        // Parent c1 is pending on lane 0, so the edge bends over and lane 1
        // dies; lane 0 itself continues through the band.
        assert_eq!(lanes(&step), vec![(1, 0), (0, 0)]);

        let step = tracker.step(oid(1), &[]);
        assert_eq!(step.lane, 0);
        assert_eq!(tracker.lane_count(), 2);
    }

    #[test]
    fn merge_keeps_first_parent_lane_and_allocates_for_others() {
        let mut tracker = LaneTracker::new();
        // Merge m walked before both parents (children precede parents):
        // first parent c2 continues m's lane, second parent c5 opens lane 1.
        let step = tracker.step(oid(9), &[oid(2), oid(5)]);
        assert_eq!(step.kind, GraphRowKind::Merge);
        assert_eq!(step.lane, 0);
        assert_eq!(lanes(&step), vec![(0, 0), (0, 1)]);

        let step = tracker.step(oid(5), &[]);
        assert_eq!(step.lane, 1);
        assert_eq!(step.kind, GraphRowKind::Root);

        let step = tracker.step(oid(2), &[oid(1)]);
        assert_eq!(step.lane, 0);
        assert_eq!(lanes(&step), vec![(0, 0)]);
    }

    #[test]
    fn octopus_merge_fans_out_to_new_lanes() {
        let mut tracker = LaneTracker::new();
        let step = tracker.step(oid(9), &[oid(1), oid(2), oid(3), oid(4)]);
        assert_eq!(step.kind, GraphRowKind::Merge);
        assert_eq!(lanes(&step), vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
        let mut seen = Vec::new();
        for parent in [1u8, 2, 3, 4] {
            let step = tracker.step(oid(parent), &[]);
            assert!(!seen.contains(&step.lane));
            seen.push(step.lane);
        }
    }

    #[test]
    fn vacated_lanes_are_reused() {
        let mut tracker = LaneTracker::new();
        // c3 branches from c1; after bending over, lane 1 is free. A second
        // independent branch tip must reuse lane 1, keeping lane 0 for main.
        tracker.step(oid(3), &[oid(1)]);
        tracker.step(oid(2), &[oid(1)]);
        let step = tracker.step(oid(8), &[oid(1)]);
        assert_eq!(step.lane, 1, "freed lane is reused");
    }
}
