//! Chunked storage for commit-graph operations: the same contract as the
//! diff store but over `GraphRow`s and without sections.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

use serde::Serialize;

use crate::domain::{Generation, GraphRow, OperationId, RepoId};
use crate::error::{GitError, Result};

/// Chunk size used for both storage ranges and streamed events. Graph rows
/// are small, so this is double the diff chunk to halve IPC overhead.
pub const GRAPH_CHUNK_ROWS: usize = 512;

/// Fully materialized state of one graph operation, safe to read from any thread.
#[derive(Debug)]
pub struct GraphOperation {
    pub repo_id: RepoId,
    pub operation_id: OperationId,
    pub generation: Generation,
    cancelled: AtomicBool,
    complete: AtomicBool,
    complete_notify: Notify,
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// One allocation per streamed chunk.
    chunks: Vec<Arc<[GraphRow]>>,
    row_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRangeResult {
    pub rows: Vec<GraphRow>,
    pub next_cursor: u64,
    pub has_more: bool,
    /// Rows materialized so far.
    pub known_total_rows: u64,
    pub complete: bool,
}

impl GraphOperation {
    pub fn new(repo_id: RepoId, operation_id: OperationId, generation: Generation) -> Self {
        Self {
            repo_id,
            operation_id,
            generation,
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            complete_notify: Notify::new(),
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) -> bool {
        !self.complete.load(Ordering::Acquire) && !self.cancelled.swap(true, Ordering::AcqRel)
    }

    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Release);
        self.complete_notify.notify_waiters();
    }

    pub fn append_chunk(&self, chunk: Arc<[GraphRow]>) -> Result<()> {
        if chunk.is_empty() {
            return Ok(());
        }
        let mut guard = self.inner.lock().unwrap();
        guard.row_count += chunk.len() as u64;
        guard.chunks.push(chunk);
        Ok(())
    }

    pub fn row_count(&self) -> u64 {
        self.inner.lock().unwrap().row_count
    }

    /// Returns a contiguous slice of materialized rows. Reads past the
    /// streamed prefix return whatever exists plus pending-state flags
    /// instead of erroring, mirroring the diff store.
    pub fn read_range(&self, start: u64, max_rows: u32) -> Result<GraphRangeResult> {
        if max_rows == 0 || max_rows > 50_000 {
            return Err(GitError::invalid_input("maxRows must be in 1..=50000"));
        }
        let guard = self.inner.lock().unwrap();

        let mut out: Vec<GraphRow> = Vec::with_capacity(max_rows as usize);
        let mut remaining = max_rows as u64;
        let mut cursor = start;
        let mut consumed = 0u64;
        for chunk in &guard.chunks {
            if remaining == 0 {
                break;
            }
            let len = chunk.len() as u64;
            let chunk_end = consumed + len;
            if chunk_end <= cursor {
                consumed = chunk_end;
                continue;
            }
            let offset = (cursor - consumed) as usize;
            let take = (len - offset as u64).min(remaining);
            out.extend_from_slice(&chunk[offset..offset + take as usize]);
            remaining -= take;
            cursor += take;
            consumed = chunk_end;
        }

        let complete = self.complete.load(Ordering::Acquire);
        let next_cursor = start + out.len() as u64;
        Ok(GraphRangeResult {
            rows: out,
            next_cursor,
            has_more: !(complete && next_cursor >= guard.row_count),
            known_total_rows: guard.row_count,
            complete,
        })
    }
}

/// Registry of live graph operations; shape mirrors the diff registry.
#[derive(Default)]
pub struct GraphOperationRegistry {
    ops: Mutex<HashMap<OperationId, Arc<GraphOperation>>>,
}

impl GraphOperationRegistry {
    pub fn insert(&self, op: Arc<GraphOperation>) {
        self.ops.lock().unwrap().insert(op.operation_id, op);
    }

    pub fn get(&self, id: OperationId) -> Result<Arc<GraphOperation>> {
        self.ops
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(GitError::ObjectNotFound {
                oid: format!("operation {id}"),
            })
    }

    /// Cancels every live operation of `repo_id`; returns how many were
    /// cancelled. Running jobs observe the flag between chunks and release
    /// their repository handles promptly.
    pub fn cancel_for_repo(&self, repo_id: RepoId) -> usize {
        let guard = self.ops.lock().unwrap();
        guard
            .values()
            .filter(|op| op.repo_id == repo_id && op.cancel())
            .count()
    }

    pub fn is_running_for_repo(&self, repo_id: RepoId) -> bool {
        self.ops
            .lock()
            .unwrap()
            .values()
            .any(|op| op.repo_id == repo_id && !op.complete.load(Ordering::Acquire))
    }

    pub async fn cancel_and_wait_for_repo(&self, repo_id: RepoId) {
        let operations: Vec<_> = {
            let guard = self.ops.lock().unwrap();
            guard
                .values()
                .filter(|op| op.repo_id == repo_id)
                .cloned()
                .collect()
        };
        for operation in &operations {
            operation.cancel();
        }
        for operation in operations {
            while !operation.complete.load(Ordering::Acquire) {
                operation.complete_notify.notified().await;
            }
        }
    }

    /// Drops completed operations beyond the newest `keep` finished ones.
    pub fn prune_completed(&self, keep: usize) {
        let mut guard = self.ops.lock().unwrap();
        let mut completed: Vec<OperationId> = guard
            .values()
            .filter(|op| op.complete.load(Ordering::Acquire))
            .map(|op| op.operation_id)
            .collect();
        completed.sort_unstable();
        while completed.len() > keep {
            let victim = completed.remove(0);
            guard.remove(&victim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op() -> Arc<GraphOperation> {
        Arc::new(GraphOperation::new(
            RepoId(1),
            OperationId(1),
            Generation(1),
        ))
    }

    fn row(index: u64) -> GraphRow {
        GraphRow {
            index,
            id: crate::domain::ObjectId::from_bytes(&[(index % 200) as u8 + 1; 20]).unwrap(),
            lane: 0,
            edges: vec![],
            kind: crate::domain::GraphRowKind::Commit,
            summary_line: "row".into(),
            author_name: "a".into(),
            author_email: "a@x".into(),
            time_seconds: 0,
            tags: vec![],
            refs: vec![],
        }
    }

    fn chunk(range: std::ops::Range<u64>) -> Arc<[GraphRow]> {
        range.map(row).collect::<Vec<_>>().into()
    }

    #[test]
    fn range_reads_span_progressive_chunks() {
        let operation = op();
        operation.append_chunk(chunk(0..2)).unwrap();
        operation.append_chunk(chunk(2..4)).unwrap();
        operation.append_chunk(chunk(4..6)).unwrap();
        operation.mark_complete();

        let range = operation.read_range(1, 4).unwrap();
        assert_eq!(range.rows.len(), 4);
        assert_eq!(range.rows[0].index, 1);
        assert_eq!(range.rows[3].index, 4);
        assert_eq!(range.next_cursor, 5);
        assert!(range.has_more);
        assert_eq!(range.known_total_rows, 6);

        let tail = operation.read_range(5, 4).unwrap();
        assert_eq!(tail.rows.len(), 1);
        assert!(!tail.has_more);
    }

    #[test]
    fn reads_past_streamed_prefix_report_pending_state() {
        let operation = op();
        operation.append_chunk(chunk(0..300)).unwrap();

        let page = operation.read_range(0, 8).unwrap();
        assert_eq!(page.rows.len(), 8);
        assert!(page.has_more);

        let pending = operation.read_range(400, 8).unwrap();
        assert!(pending.rows.is_empty());
        assert_eq!(pending.known_total_rows, 300);
        assert!(!pending.complete);

        operation.mark_complete();
        let tail = operation.read_range(296, 8).unwrap();
        assert_eq!(tail.rows.len(), 4);
        assert!(!tail.has_more);
    }

    #[test]
    fn cancel_is_ignored_after_completion() {
        let operation = op();
        assert!(operation.cancel());
        assert!(operation.is_cancelled());
        operation.mark_complete();
        assert!(!operation.cancel());
    }

    #[test]
    fn registry_prunes_only_finished_operations() {
        let registry = GraphOperationRegistry::default();
        let done_old = Arc::new(GraphOperation::new(
            RepoId(1),
            OperationId(1),
            Generation(1),
        ));
        done_old.mark_complete();
        let done_new = Arc::new(GraphOperation::new(
            RepoId(1),
            OperationId(3),
            Generation(1),
        ));
        done_new.mark_complete();
        let running = Arc::new(GraphOperation::new(
            RepoId(1),
            OperationId(2),
            Generation(1),
        ));
        registry.insert(done_old);
        registry.insert(running);
        registry.insert(done_new);

        registry.prune_completed(1);

        assert!(registry.get(OperationId(2)).is_ok());
        assert!(registry.get(OperationId(3)).is_ok());
        assert!(registry.get(OperationId(1)).is_err());
    }
}
