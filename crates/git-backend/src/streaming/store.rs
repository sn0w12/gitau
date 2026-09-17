use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

use serde::Serialize;

use crate::domain::{Generation, OperationId, RelativePath, RepoId};
use crate::error::{GitError, Result};
use crate::streaming::model::{DiffRow, SectionMeta};

/// Fully materialized state of one diff operation, safe to read from any thread.
#[derive(Debug)]
pub struct DiffOperation {
    pub repo_id: RepoId,
    pub operation_id: OperationId,
    pub generation: Generation,
    pub cancelled: AtomicBool,
    pub complete: AtomicBool,
    complete_notify: Notify,
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    sections: Vec<SectionData>,
    total_rows: u64,
    additions: u64,
    deletions: u64,
    started_sections: Vec<SectionMeta>,
    estimated_total_rows: u64,
}

#[derive(Debug)]
struct SectionData {
    meta: SectionMeta,
    image: Option<DiffImage>,
    start_row: u64,
    declared_rows: Option<u64>,
    /// Maintained incrementally on append; equals the sum of chunk lengths.
    row_count: u64,
    /// One allocation per streamed chunk; highlighted progressively by the
    /// emitter so storage never runs ahead of what has been delivered.
    chunks: Vec<Arc<[DiffRow]>>,
}

/// Chunk size used for both storage ranges and streamed events.
pub const CHUNK_ROWS: usize = 256;

impl DiffOperation {
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

    pub fn set_started(&self, sections: Vec<SectionMeta>, estimated_total_rows: u64) {
        let mut guard = self.inner.lock().unwrap();
        guard.sections = sections
            .iter()
            .map(|meta| SectionData {
                meta: meta.clone(),
                image: None,
                start_row: 0,
                declared_rows: None,
                row_count: 0,
                chunks: Vec::new(),
            })
            .collect();
        guard.started_sections = sections;
        guard.estimated_total_rows = estimated_total_rows;
    }

    pub fn set_section_image(&self, section_id: u32, image: DiffImage) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        let section = section_mut(&mut guard, section_id)?;
        section.image = Some(image);
        Ok(())
    }

    pub fn read_image(&self, section_id: u32) -> Result<Option<DiffImage>> {
        let guard = self.inner.lock().unwrap();
        let section = guard
            .sections
            .get(section_id as usize)
            .filter(|section| section.meta.section_id == section_id)
            .ok_or_else(|| GitError::invalid_input(format!("unknown section id {section_id}")))?;
        Ok(section.image.clone())
    }

    pub fn started_sections(&self) -> Vec<SectionMeta> {
        self.inner.lock().unwrap().started_sections.clone()
    }

    pub fn estimated_total_rows(&self) -> u64 {
        self.inner.lock().unwrap().estimated_total_rows
    }

    /// Registers exact layout for a section before its rows stream in.
    pub fn begin_section(&self, section_id: u32, row_count: u64) -> Result<u64> {
        let mut guard = self.inner.lock().unwrap();
        let index = section_id as usize;
        if guard
            .sections
            .get(index)
            .map(|s| s.meta.section_id != section_id)
            .unwrap_or(true)
        {
            return Err(GitError::invalid_input(format!(
                "unknown section id {section_id}"
            )));
        }
        if let Some(existing) = guard.sections[index].declared_rows {
            return Err(GitError::internal(format!(
                "section {section_id} already laid out with {existing} rows"
            )));
        }
        let prior: u64 = guard.sections[..index].iter().map(|s| s.row_count).sum();
        guard.sections[index].declared_rows = Some(row_count);
        guard.sections[index].start_row = prior;
        Ok(prior)
    }

    /// Appends one streamed chunk's rows to a laid-out section. Addition/
    /// deletion counts are per chunk and accumulate into the operation
    /// totals; the sum over all chunks equals the section totals.
    pub fn append_chunk(
        &self,
        section_id: u32,
        chunk: Arc<[DiffRow]>,
        additions: u64,
        deletions: u64,
    ) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        let section = section_mut(&mut guard, section_id)?;
        let added = chunk.len() as u64;
        if let Some(declared) = section.declared_rows {
            if section.row_count + added > declared {
                return Err(GitError::internal(format!(
                    "section {section_id} chunk exceeds declared {} rows",
                    declared
                )));
            }
        }
        section.chunks.push(chunk);
        section.row_count += added;
        guard.additions += additions;
        guard.deletions += deletions;
        guard.total_rows += added;
        Ok(())
    }

    pub fn mark_section_complete(&self, section_id: u32) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        let section = section_mut(&mut guard, section_id)?;
        section.meta.complete = true;
        Ok(())
    }

    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Release);
        self.complete_notify.notify_waiters();
    }

    pub fn totals(&self) -> (u64, u64, u64) {
        let guard = self.inner.lock().unwrap();
        (guard.total_rows, guard.additions, guard.deletions)
    }

    /// Returns a contiguous slice of materialized rows across section boundaries.
    pub fn read_range(&self, start: u64, max_rows: u32) -> Result<RangeResult> {
        if max_rows == 0 || max_rows > 50_000 {
            return Err(GitError::invalid_input("maxRows must be in 1..=50000"));
        }
        let guard = self.inner.lock().unwrap();

        let mut out: Vec<RangeRow> = Vec::with_capacity(max_rows as usize);
        let mut remaining = max_rows as u64;
        let mut cursor = start;
        for section in &guard.sections {
            if remaining == 0 {
                break;
            }
            let section_end = section.start_row + section.row_count;
            if section_end <= cursor {
                continue;
            }
            if section.chunks.is_empty() {
                continue;
            }
            let mut offset_in_section = cursor.saturating_sub(section.start_row) as usize;
            for chunk in &section.chunks {
                if remaining == 0 {
                    break;
                }
                let len = chunk.len();
                if offset_in_section >= len {
                    offset_in_section -= len;
                    continue;
                }
                let take = (len - offset_in_section).min(remaining as usize);
                for row in &chunk[offset_in_section..offset_in_section + take] {
                    out.push(RangeRow {
                        section_id: section.meta.section_id,
                        path: None,
                        kind: section.meta.kind,
                        row: row.clone(),
                    });
                }
                remaining -= take as u64;
                cursor += take as u64;
                offset_in_section = 0;
            }
        }

        let is_complete = self.complete.load(Ordering::Acquire);
        let next_cursor = start + out.len() as u64;
        Ok(RangeResult {
            rows: out,
            next_cursor,
            has_more: !(is_complete && next_cursor >= guard.total_rows),
            known_total_rows: guard.total_rows,
            complete: is_complete,
        })
    }

    pub fn snapshot_meta(&self) -> Vec<(SectionMeta, u64, u64)> {
        let guard = self.inner.lock().unwrap();
        guard
            .sections
            .iter()
            .map(|s| (s.meta.clone(), s.start_row, s.row_count))
            .collect()
    }

    pub fn section_count(&self) -> usize {
        self.inner.lock().unwrap().sections.len()
    }
}

fn section_mut(inner: &mut Inner, section_id: u32) -> Result<&mut SectionData> {
    inner
        .sections
        .get_mut(section_id as usize)
        .filter(|s| s.meta.section_id == section_id)
        .ok_or_else(|| GitError::invalid_input(format!("unknown section id {section_id}")))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffImageSide {
    pub data: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffImage {
    pub old: Option<DiffImageSide>,
    pub new: Option<DiffImageSide>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeRow {
    pub section_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<RelativePath>,
    #[serde(rename = "sectionKind")]
    pub kind: crate::streaming::model::SectionKind,
    #[serde(flatten)]
    pub row: DiffRow,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeResult {
    pub rows: Vec<RangeRow>,
    pub next_cursor: u64,
    pub has_more: bool,
    pub known_total_rows: u64,
    pub complete: bool,
}

/// Registry of live diff operations.
#[derive(Default)]
pub struct DiffOperationRegistry {
    ops: Mutex<HashMap<OperationId, Arc<DiffOperation>>>,
}

impl DiffOperationRegistry {
    pub fn insert(&self, op: Arc<DiffOperation>) {
        self.ops.lock().unwrap().insert(op.operation_id, op);
    }

    pub fn get(&self, id: OperationId) -> Result<Arc<DiffOperation>> {
        self.ops
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(GitError::ObjectNotFound {
                oid: format!("operation {id}"),
            })
    }

    pub fn remove(&self, id: OperationId) {
        self.ops.lock().unwrap().remove(&id);
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

    pub fn len(&self) -> usize {
        self.ops.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops completed operations beyond the newest `keep` finished ones.
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
    use crate::streaming::model::{DiffRowKind, SectionKind};

    fn row(content: &str, kind: DiffRowKind) -> DiffRow {
        DiffRow {
            kind,
            old_lineno: None,
            new_lineno: None,
            content: content.into(),
            raw_hex: None,
            spans: None,
        }
    }

    fn meta(id: u32) -> SectionMeta {
        SectionMeta {
            section_id: id,
            path: RelativePath::parse("a.txt").unwrap(),
            old_path: None,
            kind: SectionKind::Modified,
            binary: false,
            image: false,
            complete: false,
        }
    }

    fn op() -> Arc<DiffOperation> {
        Arc::new(DiffOperation::new(RepoId(1), OperationId(1), Generation(1)))
    }

    #[test]
    fn absolute_positions_are_stable_across_sections() {
        let operation = op();
        operation.set_started(vec![meta(0), meta(1)], 100);

        assert_eq!(operation.begin_section(0, 4).unwrap(), 0);
        let rows0: Arc<[DiffRow]> = (0..4)
            .map(|i| row(&format!("s0-{i}"), DiffRowKind::Context))
            .collect::<Vec<_>>()
            .into();
        operation.append_chunk(0, rows0, 2, 2).unwrap();

        assert_eq!(operation.begin_section(1, 3).unwrap(), 4);
        let rows1: Arc<[DiffRow]> = (0..3)
            .map(|i| row(&format!("s1-{i}"), DiffRowKind::Addition))
            .collect::<Vec<_>>()
            .into();
        operation.append_chunk(1, rows1, 3, 0).unwrap();
        operation.mark_complete();

        let range = operation.read_range(2, 10).unwrap();
        assert_eq!(range.rows.len(), 5);
        assert_eq!(range.rows[0].row.content, "s0-2");
        assert_eq!(range.rows[2].row.content, "s1-0");
        assert_eq!(range.next_cursor, 7);
        assert!(!range.has_more);
        assert_eq!(operation.totals(), (7, 5, 2));
    }

    #[test]
    fn range_reads_span_progressive_chunks() {
        let operation = op();
        operation.set_started(vec![meta(0)], 500);
        operation.begin_section(0, 6).unwrap();
        for part in 0..3u64 {
            let rows: Arc<[DiffRow]> = (part * 2..part * 2 + 2)
                .map(|i| row(&format!("{i}"), DiffRowKind::Context))
                .collect::<Vec<_>>()
                .into();
            operation.append_chunk(0, rows, 0, 0).unwrap();
        }
        operation.mark_section_complete(0).unwrap();
        operation.mark_complete();

        // A read straddling all three chunk allocations.
        let range = operation.read_range(1, 4).unwrap();
        assert_eq!(range.rows.len(), 4);
        assert_eq!(range.rows[0].row.content, "1");
        assert_eq!(range.rows[3].row.content, "4");
        assert_eq!(operation.totals(), (6, 0, 0));
    }

    #[test]
    fn range_reads_span_chunks_and_report_pending_state() {
        let operation = op();
        operation.set_started(vec![meta(0), meta(1)], 500);
        operation.begin_section(0, 300).unwrap();
        let backing: Arc<[DiffRow]> = (0..300)
            .map(|i| row(&format!("{i}"), DiffRowKind::Context))
            .collect::<Vec<_>>()
            .into();
        operation.append_chunk(0, backing, 0, 0).unwrap();

        let page = operation.read_range(0, 8).unwrap();
        assert_eq!(page.rows.len(), 8);
        assert!(page.has_more);
        assert_eq!(page.next_cursor, 8);

        // Past the stored prefix of an unfinished diff: nothing yet, but the
        // known totals and completeness state are reported.
        let pending = operation.read_range(400, 8).unwrap();
        assert!(pending.rows.is_empty());
        assert_eq!(pending.known_total_rows, 300);
        assert!(!pending.complete);

        let tail = operation.read_range(296, 8).unwrap();
        assert_eq!(tail.rows.len(), 4);
        assert_eq!(tail.rows[3].row.content, "299");
        operation.mark_complete();
        let tail = operation.read_range(296, 8).unwrap();
        assert_eq!(tail.rows.len(), 4);
        assert!(!tail.has_more);
    }

    #[test]
    fn double_layout_is_rejected() {
        let operation = op();
        operation.set_started(vec![meta(0)], 10);
        operation.begin_section(0, 5).unwrap();
        assert!(operation.begin_section(0, 5).is_err());
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
        let registry = DiffOperationRegistry::default();
        let done_old = Arc::new(DiffOperation::new(RepoId(1), OperationId(1), Generation(1)));
        done_old.mark_complete();
        let done_new = Arc::new(DiffOperation::new(RepoId(1), OperationId(3), Generation(1)));
        done_new.mark_complete();
        let running = Arc::new(DiffOperation::new(RepoId(1), OperationId(2), Generation(1)));
        registry.insert(done_old);
        registry.insert(running);
        registry.insert(done_new);

        registry.prune_completed(1);

        assert!(registry.get(OperationId(2)).is_ok());
        assert!(registry.get(OperationId(3)).is_ok());
        assert!(registry.get(OperationId(1)).is_err());
    }
}
