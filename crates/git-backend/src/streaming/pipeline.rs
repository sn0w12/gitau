use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::{Delta, DiffOptions, Repository};
use tokio::sync::mpsc::Sender;

use crate::api::queries::DiffRequest;
use crate::domain::{Generation, OperationId, RelativePath, SnapshotId};
use crate::engines::gix::session::GixSession;
use crate::error::{GitError, Result};
use crate::streaming::model::{DiffEvent, DiffRow, DiffRowKind, SectionKind, SectionMeta};
use crate::streaming::store::DiffOperation;
const CHUNK_ROWS: usize = 256;
/// Highlighting is inline per chunk, so the first chunk stays small enough
/// that its parse cost does not delay first paint.
const FIRST_HIGHLIGHTED_CHUNK_ROWS: usize = 160;
/// First chunk of a section is oversized so one IPC message carries the whole
/// initial viewport; later chunks stay small to keep the stream flowing.
const FIRST_CHUNK_ROWS: usize = 1024;
const NO_NEWLINE_MARKER: &str = "\\ No newline at end of file";

#[derive(Clone)]
pub struct DiffJob {
    #[allow(dead_code)]
    repo_path: PathBuf,
    request: DiffRequest,
    operation_id: OperationId,
    snapshot_id: SnapshotId,
    generation: Generation,
    /// Persistent handle reused across operations; discovery and object reads
    /// stay warm instead of rediscovering per job.
    session: GixSession,
    /// Per-job blob store; enumeration warms it concurrently with status.
    blobs: crate::engines::gix::diff::BlobCache,
}

impl DiffJob {
    pub fn new(
        repo_path: PathBuf,
        request: DiffRequest,
        operation_id: OperationId,
        snapshot_id: SnapshotId,
        generation: Generation,
        session: GixSession,
    ) -> Self {
        Self {
            repo_path,
            request,
            operation_id,
            snapshot_id,
            generation,
            session,
            blobs: crate::engines::gix::diff::new_blob_cache(),
        }
    }
}

/// Blocking diff computation intended to run inside `spawn_blocking`.
///
/// Protocol invariants:
/// - `SectionLayout` for a section is emitted before any of its chunks.
/// - Once emitted, a section's `start_row` and `row_count` never change.
/// - Absolute row positions are therefore stable for the lifetime of the operation.
pub fn run(job: DiffJob, op: Arc<DiffOperation>, tx: Sender<DiffEvent>) {
    match compute(&job, &op, &tx) {
        Ok(()) => {}
        Err(GitError::Cancelled) => {
            op.mark_complete();
            let _ = tx.blocking_send(DiffEvent::Cancelled {
                operation_id: job.operation_id,
            });
        }
        Err(err) => {
            op.mark_complete();
            let _ = tx.blocking_send(DiffEvent::Failed {
                operation_id: job.operation_id,
                code: err.code().to_owned(),
                message: err.to_string(),
            });
        }
    }
}

struct Emitter<'a> {
    tx: &'a Sender<DiffEvent>,
    receiver_gone: bool,
}

impl<'a> Emitter<'a> {
    fn send(&mut self, event: DiffEvent) {
        if self.receiver_gone {
            return;
        }
        // A closed channel means the consumer stopped listening; computation
        // and storage continue so range reads still serve rows.
        if self.tx.blocking_send(event).is_err() {
            self.receiver_gone = true;
        }
    }
}

/// Dedicated section-materialization pool. The work mixes file I/O with
/// diffing; oversubscribing beyond a handful of threads degrades NTFS
/// throughput and contends on gix's object cache, so the count stays bounded
/// instead of following the machine's core count.
fn section_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 6);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("diff-section-{i}"))
            .build()
            .expect("failed to build diff section thread pool")
    })
}

fn compute(job: &DiffJob, op: &Arc<DiffOperation>, tx: &Sender<DiffEvent>) -> Result<()> {
    if op.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    let started_at = std::time::Instant::now();

    let plans = crate::engines::gix::diff::enumerate(&job.session, &job.request, &job.blobs)?;
    if op.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    let metas: Vec<SectionMeta> = plans.iter().map(|p| p.meta.clone()).collect();
    let estimate: u64 = metas.iter().map(|m| if m.binary { 2 } else { 10 }).sum();
    op.set_started(metas.clone(), estimate);
    let mut emitter = Emitter {
        tx,
        receiver_gone: false,
    };
    emitter.send(DiffEvent::Started {
        operation_id: job.operation_id,
        snapshot_id: job.snapshot_id,
        generation: job.generation.0,
        sections: metas,
        estimated_total_rows: estimate,
    });

    // Sections are independent; the persistent rayon pool materializes and
    // progressively highlights them concurrently while this thread stores
    // and streams results strictly in section order. Producers ship one
    // message per chunk so a section's first chunk can be delivered before
    // its remaining rows are highlighted.
    let plans = Arc::new(plans);
    let (done_tx, done_rx) = std::sync::mpsc::channel::<(usize, ProducerMsg)>();
    let done_rx = std::sync::Mutex::new(done_rx);

    let mut pending: BTreeMap<usize, PendingSection> = BTreeMap::new();
    let mut total_rows: u64 = 0u64;

    section_pool().scope(|scope| {
        for (idx, plan) in plans.iter().enumerate() {
            if op.is_cancelled() {
                break;
            }
            let done_tx = done_tx.clone();
            scope.spawn(move |_| {
                if op.is_cancelled() {
                    return;
                }
                if produce_section(job, op, idx, plan, &done_tx).is_err() {
                    // Receiver gone or cancelled: stop producing.
                }
            });
        }
        drop(done_tx);

        let mut expected = 0usize;
        while expected < plans.len() {
            if op.is_cancelled() {
                return Err(GitError::Cancelled);
            }
            let (idx, msg) = {
                let receiver = done_rx.lock().unwrap();
                match receiver.recv() {
                    Ok(value) => value,
                    Err(_) => break,
                }
            };
            {
                let entry = pending.entry(idx).or_default();
                match msg {
                    ProducerMsg::Meta { row_count } => entry.row_count = Some(row_count),
                    ProducerMsg::Chunk(chunk) => {
                        entry.total_chunks = Some(chunk.total);
                        entry.chunks.insert(chunk.index, chunk);
                    }
                }
            }
            // Emit everything now deliverable for the expected section.
            loop {
                if op.is_cancelled() {
                    return Err(GitError::Cancelled);
                }
                let Some(entry) = pending.get_mut(&expected) else {
                    break;
                };
                let Some(row_count) = entry.row_count else {
                    break;
                };
                if row_count == 0 {
                    let meta = &plans[expected].meta;
                    op.mark_section_complete(meta.section_id)?;
                    pending.remove(&expected);
                    expected += 1;
                    continue;
                }
                let next_chunk = entry.next_chunk;
                let Some(chunk) = entry.chunks.remove(&next_chunk) else {
                    break;
                };
                if next_chunk == 0 {
                    let meta = &plans[expected].meta;
                    let section_start = op.begin_section(meta.section_id, row_count)?;
                    emitter.send(DiffEvent::SectionLayout {
                        operation_id: job.operation_id,
                        section_id: meta.section_id,
                        start_row: section_start,
                        row_count,
                    });
                    entry.start_row = section_start;
                }
                op.append_chunk(
                    plans[expected].meta.section_id,
                    chunk.rows.clone(),
                    chunk.additions,
                    chunk.deletions,
                )?;
                emitter.send(DiffEvent::Chunk {
                    operation_id: job.operation_id,
                    section_id: plans[expected].meta.section_id,
                    row_start: entry.start_row + entry.emitted as u64,
                    rows: crate::streaming::model::SharedRows::slice(
                        chunk.rows.clone(),
                        0,
                        chunk.rows.len(),
                    ),
                    spans_by_row: chunk.spans_by_row.clone(),
                    styles: chunk.styles.clone(),
                });
                entry.next_chunk += 1;
                entry.emitted += chunk.rows.len();
                total_rows += chunk.rows.len() as u64;
                if entry.emitted as u64 >= row_count {
                    let meta = &plans[expected].meta;
                    op.mark_section_complete(meta.section_id)?;
                    pending.remove(&expected);
                    expected += 1;
                }
            }
        }
        Ok(())
    })?;

    if op.is_cancelled() {
        return Err(GitError::Cancelled);
    }

    finish(op, &mut emitter, total_rows, started_at)
}

enum ProducerMsg {
    Meta { row_count: u64 },
    Chunk(PreparedChunk),
}

struct PreparedChunk {
    index: u32,
    total: u32,
    rows: Arc<[DiffRow]>,
    spans_by_row: Option<Vec<Option<Vec<u32>>>>,
    styles: Vec<crate::streaming::model::WireStyle>,
    additions: u64,
    deletions: u64,
}

#[derive(Default)]
struct PendingSection {
    row_count: Option<u64>,
    total_chunks: Option<u32>,
    chunks: BTreeMap<u32, PreparedChunk>,
    /// Index of the next chunk to emit (chunk numbering, not rows).
    next_chunk: u32,
    /// Rows emitted so far within the section.
    emitted: usize,
    start_row: u64,
}

/// Materializes one section and ships its chunks with highlighting applied
/// incrementally, so the first chunk's delivery cost covers only its own
/// rows.
fn produce_section(
    job: &DiffJob,
    op: &Arc<DiffOperation>,
    idx: usize,
    plan: &crate::engines::gix::diff::SectionPlan,
    tx: &std::sync::mpsc::Sender<(usize, ProducerMsg)>,
) -> Result<()> {
    let mut section =
        crate::engines::gix::diff::materialize(&job.session, &job.request, plan, &job.blobs)?;
    if let Some(image) = section.image.clone() {
        op.set_section_image(plan.meta.section_id, image)?;
    }
    let _ = tx.send((
        idx,
        ProducerMsg::Meta {
            row_count: section.rows.len() as u64,
        },
    ));
    if section.rows.is_empty() {
        return Ok(());
    }

    let first_len = if section.highlighter.is_some() {
        FIRST_HIGHLIGHTED_CHUNK_ROWS
    } else {
        FIRST_CHUNK_ROWS
    };
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0usize;
    while offset < section.rows.len() {
        let len = if offset == 0 { first_len } else { CHUNK_ROWS }.min(section.rows.len() - offset);
        bounds.push((offset, offset + len));
        offset += len;
    }
    let total = bounds.len() as u32;

    let mut highlighter = section.highlighter;
    for (index, (start, end)) in bounds.into_iter().enumerate() {
        if op.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        if let Some(hl) = highlighter.as_mut() {
            hl.highlight_rows(&mut section.rows[start..end]);
        }
        let spans_by_row = highlighter.is_some().then(|| {
            section.rows[start..end]
                .iter()
                .map(|r| r.spans.clone())
                .collect()
        });
        let styles = match highlighter.as_mut() {
            Some(hl) if spans_by_row.is_some() => hl.take_new_styles(),
            _ => Vec::new(),
        };
        let additions = count_kind(&section.rows[start..end], DiffRowKind::Addition);
        let deletions = count_kind(&section.rows[start..end], DiffRowKind::Deletion);
        let _ = tx.send((
            idx,
            ProducerMsg::Chunk(PreparedChunk {
                index: index as u32,
                total,
                rows: section.rows[start..end].into(),
                spans_by_row,
                styles,
                additions,
                deletions,
            }),
        ));
    }
    Ok(())
}

fn finish(
    op: &Arc<DiffOperation>,
    emitter: &mut Emitter<'_>,
    computed_total: u64,
    started_at: std::time::Instant,
) -> Result<()> {
    let (stored_total, additions, deletions) = op.totals();
    debug_assert_eq!(stored_total, computed_total);
    emitter.send(DiffEvent::LayoutReady {
        operation_id: op.operation_id,
        total_rows: stored_total,
    });
    op.mark_complete();
    emitter.send(DiffEvent::Completed {
        operation_id: op.operation_id,
        total_rows: stored_total,
        additions,
        deletions,
        duration_ms: started_at.elapsed().as_secs_f64() * 1000.0,
    });
    Ok(())
}

fn count_kind(rows: &[DiffRow], kind: DiffRowKind) -> u64 {
    rows.iter()
        .filter(|r| r.kind == kind && r.content != NO_NEWLINE_MARKER)
        .count() as u64
}

pub(crate) fn build_options(request: &DiffRequest) -> Result<DiffOptions> {
    let mut opts = DiffOptions::new();
    opts.context_lines(request.context_lines.clamp(0, 64))
        .interhunk_lines(request.interhunk_lines)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true)
        .skip_binary_check(false);
    if request.ignore_whitespace {
        opts.ignore_whitespace(true);
    }
    let pathspecs: Vec<RelativePath> = request
        .paths
        .iter()
        .map(|p| RelativePath::parse(p))
        .collect::<Result<_>>()?;
    if !pathspecs.is_empty() {
        for spec in pathspecs {
            opts.pathspec(spec.as_str().to_owned());
        }
    }
    Ok(opts)
}

pub(crate) fn enumerate_sections(diff: &git2::Diff<'_>) -> Result<Vec<SectionMeta>> {
    let mut sections = Vec::new();
    for delta in diff.deltas() {
        let status = map_delta_status(delta.status());
        if status.is_none() {
            continue;
        }
        let status = status.unwrap();
        let binary = delta.flags().is_binary();
        let new_path = delta.new_file().path();
        let old_path = delta.old_file().path();
        let path = new_path.or(old_path);
        let Some(path_str) = path else { continue };
        let rel = RelativePath::parse(&normalize_separators(path_str))?;
        sections.push(SectionMeta {
            section_id: sections.len() as u32,
            path: rel,
            old_path: old_path
                .map(normalize_separators)
                .map(|p| RelativePath::parse(&p))
                .transpose()?,
            kind: status,
            binary,
            image: is_image_path(path_str.to_string_lossy().as_bytes()),
            complete: false,
        });
    }
    Ok(sections)
}

fn is_image_path(path: &[u8]) -> bool {
    let path = String::from_utf8_lossy(path);
    let Some(extension) = path.rsplit('.').next() else {
        return false;
    };
    [
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "svg",
    ]
    .iter()
    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

fn normalize_separators(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

fn map_delta_status(status: Delta) -> Option<SectionKind> {
    match status {
        Delta::Added | Delta::Untracked | Delta::Unreadable => Some(SectionKind::Added),
        Delta::Deleted => Some(SectionKind::Deleted),
        Delta::Modified | Delta::Ignored => Some(SectionKind::Modified),
        Delta::Renamed => Some(SectionKind::Renamed),
        Delta::Copied => Some(SectionKind::Copied),
        Delta::Typechange => Some(SectionKind::TypeChanged),
        Delta::Conflicted => Some(SectionKind::Conflicted),
        Delta::Unmodified => None,
    }
}

pub(crate) fn resolve_commit<'repo>(
    repo: &'repo Repository,
    spec: &crate::domain::RevisionSpec,
) -> Result<git2::Commit<'repo>> {
    let obj = repo
        .revparse_single(spec.as_str())
        .map_err(|_| GitError::InvalidRevision {
            spec: spec.as_str().to_owned(),
        })?;
    obj.peel_to_commit().map_err(|_| GitError::InvalidRevision {
        spec: spec.as_str().to_owned(),
    })
}

pub(crate) fn map_open_error(path: &Path, err: git2::Error) -> GitError {
    if err.code() == git2::ErrorCode::NotFound {
        GitError::NotARepository {
            path: path.to_owned(),
        }
    } else {
        GitError::Internal {
            message: err.message().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::model::DiffEvent;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn fixture_repo() -> tempfile::TempDir {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("multi-chunk");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "T").unwrap();
        config.set_str("user.email", "t@example.com").unwrap();
        drop(config);

        let base: String = (0..600).map(|i| format!("line {i} original\n")).collect();
        std::fs::write(root.join("big.rs"), &base).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        // Every line changes: the diff far exceeds one chunk.
        let edited: String = (0..600).map(|i| format!("line {i} rewritten\n")).collect();
        std::fs::write(root.join("big.rs"), &edited).unwrap();
        outer
    }

    #[test]
    fn multi_chunk_sections_stream_completely_and_in_order() {
        let outer = fixture_repo();
        let root = outer.path().join("multi-chunk");
        let session = crate::engines::gix::GixSession::discover(&root).unwrap();
        let op = Arc::new(DiffOperation::new(
            crate::domain::RepoId(1),
            crate::domain::OperationId(1),
            crate::domain::Generation(1),
        ));
        let (tx, mut rx) = tokio::sync::mpsc::channel::<DiffEvent>(4096);
        let job = DiffJob::new(
            root.clone(),
            crate::api::queries::DiffRequest::default(),
            crate::domain::OperationId(1),
            crate::domain::SnapshotId(0),
            crate::domain::Generation(1),
            session.clone(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let producer = {
            let op = op.clone();
            let handle = rt.handle().clone();
            std::thread::spawn(move || {
                // run() hands chunks to a tokio mpsc Sender; give the thread
                // the same runtime context spawn_blocking would provide.
                let _guard = handle.enter();
                run(job, op, tx)
            })
        };

        let deadline = Instant::now() + Duration::from_secs(30);
        let mut chunk_rows_by_section: std::collections::HashMap<u32, usize> =
            std::collections::HashMap::new();
        let mut layout_rows: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
        let mut completed = false;

        while !completed {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "pipeline stalled before Completed: multi-chunk regression"
            );
            let event =
                rt.block_on(async { tokio::time::timeout(remaining, rx.recv()).await.unwrap() });
            match event.expect("channel closed before Completed") {
                DiffEvent::SectionLayout {
                    section_id,
                    row_count,
                    ..
                } => {
                    layout_rows.insert(section_id, row_count);
                }
                DiffEvent::Chunk {
                    section_id,
                    rows,
                    spans_by_row,
                    ..
                } => {
                    *chunk_rows_by_section.entry(section_id).or_default() += rows.len();
                    // Highlighting is always on for text sections; every
                    // delivered chunk must carry aligned span entries.
                    assert_eq!(spans_by_row.as_ref().map(Vec::len), Some(rows.len()));
                }
                DiffEvent::Completed {
                    total_rows,
                    additions,
                    deletions,
                    ..
                } => {
                    completed = true;
                    let (stored_total, stored_add, stored_del) = op.totals();
                    assert_eq!(total_rows, stored_total);
                    assert_eq!((additions, deletions), (stored_add, stored_del));
                    // The 600 rewritten lines must exceed the first chunk so
                    // this test exercises multi-chunk ordering.
                    assert!(total_rows > FIRST_HIGHLIGHTED_CHUNK_ROWS as u64);
                }
                _ => {}
            }
        }

        for (section_id, row_count) in &layout_rows {
            assert_eq!(
                chunk_rows_by_section.get(section_id),
                Some(&(*row_count as usize)),
                "section {section_id} delivered {row_count} declared rows incompletely"
            );
        }

        // Range reads serve exactly what was streamed, highlighted included.
        let range = op.read_range(0, 50_000).unwrap();
        assert_eq!(range.rows.len() as u64, layout_rows.values().sum::<u64>());
        // All non-header rows should have spans (file headers are the exception).
        let highlighted_content = range
            .rows
            .iter()
            .filter(|r| r.row.kind != DiffRowKind::FileHeader)
            .filter(|r| r.row.spans.is_some())
            .count();
        let total_content = range
            .rows
            .iter()
            .filter(|r| r.row.kind != DiffRowKind::FileHeader)
            .count();
        assert!(
            highlighted_content >= total_content.saturating_sub(1),
            "all but at most one non-header row should have spans (got {highlighted_content}/{total_content})"
        );

        producer.join().unwrap();
    }
}
