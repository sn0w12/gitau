//! Blocking commit-graph producer, mirroring the diff pipeline: walks the
//! cached OID list, assigns lanes sequentially, stores chunks in the
//! operation, and streams `GraphEvent`s to the IPC channel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::Sender;

use crate::api::graph::GraphQuery;
use crate::domain::{Generation, ObjectId, OperationId, SnapshotId};
use crate::engines::git2::graph::{LaneTracker, branch_decorations, walk_oids};
use crate::engines::git2::session::Git2Session;
use crate::error::{GitError, Result};
use crate::runtime::history_cache::HistoryCache;
use crate::streaming::graph::{GRAPH_CHUNK_ROWS, GraphOperation};
use crate::streaming::model::GraphEvent;

#[derive(Clone)]
pub struct GraphJob {
    repo_path: PathBuf,
    request: GraphQuery,
    operation_id: OperationId,
    snapshot_id: SnapshotId,
    generation: Generation,
    history_cache: Arc<Mutex<HistoryCache>>,
}

impl GraphJob {
    pub fn new(
        repo_path: PathBuf,
        request: GraphQuery,
        operation_id: OperationId,
        snapshot_id: SnapshotId,
        generation: Generation,
        history_cache: Arc<Mutex<HistoryCache>>,
    ) -> Self {
        Self {
            repo_path,
            request,
            operation_id,
            snapshot_id,
            generation,
            history_cache,
        }
    }
}

pub fn run(job: GraphJob, op: Arc<GraphOperation>, tx: Sender<GraphEvent>) {
    match compute(&job, &op, &tx) {
        Ok(()) => {}
        Err(GitError::Cancelled) => {
            op.mark_complete();
            let _ = tx.blocking_send(GraphEvent::Cancelled {
                operation_id: job.operation_id,
            });
        }
        Err(err) => {
            op.mark_complete();
            let _ = tx.blocking_send(GraphEvent::Failed {
                operation_id: job.operation_id,
                code: err.code().to_owned(),
                message: err.to_string(),
            });
        }
    }
}

struct Emitter<'a> {
    tx: &'a Sender<GraphEvent>,
    receiver_gone: bool,
}

impl<'a> Emitter<'a> {
    fn send(&mut self, event: GraphEvent) {
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

fn compute(job: &GraphJob, op: &Arc<GraphOperation>, tx: &Sender<GraphEvent>) -> Result<()> {
    let session = Git2Session::new(job.repo_path.clone());
    let decorations = branch_decorations(&session)?;
    let tags = tag_map(&session)?;
    let oids = {
        let mut guard = job.history_cache.lock().unwrap();
        walk_oids(&session, &job.request, &mut guard)?
    };

    let mut emitter = Emitter {
        tx,
        receiver_gone: false,
    };
    emitter.send(GraphEvent::Started {
        operation_id: job.operation_id,
        snapshot_id: job.snapshot_id,
        generation: job.generation,
    });

    let mut tracker = LaneTracker::new();
    let mut emitted = 0u64;
    for chunk in oids.chunks(GRAPH_CHUNK_ROWS) {
        if op.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        let rows = rows_for_chunk(&session, chunk, emitted, &mut tracker, &decorations, &tags)?;
        emitted += rows.len() as u64;
        op.append_chunk(rows.as_slice().into())?;
        emitter.send(GraphEvent::Chunk {
            operation_id: job.operation_id,
            row_start: emitted - rows.len() as u64,
            rows,
        });
    }

    op.mark_complete();
    emitter.send(GraphEvent::Completed {
        operation_id: job.operation_id,
        total_rows: emitted,
    });
    Ok(())
}

fn rows_for_chunk(
    session: &Git2Session,
    chunk: &[git2::Oid],
    start_index: u64,
    tracker: &mut LaneTracker<git2::Oid>,
    decorations: &HashMap<[u8; 20], Vec<String>>,
    tags: &HashMap<[u8; 20], Vec<String>>,
) -> Result<Vec<crate::domain::GraphRow>> {
    session.with_repository(|repo| {
        let mut rows = Vec::with_capacity(chunk.len());
        for (offset, oid) in chunk.iter().enumerate() {
            let commit = repo
                .find_commit(*oid)
                .map_err(|_| GitError::ObjectNotFound {
                    oid: oid.to_string(),
                })?;
            let parents: Vec<git2::Oid> = commit.parent_ids().collect();
            let step = tracker.step(*oid, &parents);
            let key = crate::engines::git2::history::oid_bytes(*oid);
            let author = commit.author();
            let committer = commit.committer();
            rows.push(crate::domain::GraphRow {
                index: start_index + offset as u64,
                id: ObjectId::from_bytes(oid.as_bytes())?,
                lane: step.lane,
                edges: step.edges,
                kind: step.kind,
                summary_line: String::from_utf8_lossy(commit.summary_bytes().unwrap_or(b""))
                    .into_owned(),
                author_name: author.name().unwrap_or_default().to_owned(),
                author_email: author.email().unwrap_or_default().to_owned(),
                time_seconds: committer.when().seconds(),
                tags: tags.get(&key).cloned().unwrap_or_default(),
                refs: decorations.get(&key).cloned().unwrap_or_default(),
            });
        }
        Ok(rows)
    })
}

/// Peeled tag names per commit; the history cache's tag map is only populated
/// on the page path, so the graph keeps its own cheap one-shot map.
fn tag_map(session: &Git2Session) -> Result<HashMap<[u8; 20], Vec<String>>> {
    session.with_repository(|repo| {
        let mut map: HashMap<[u8; 20], Vec<String>> = HashMap::new();
        let references = repo.references_glob("refs/tags/*")?;
        for reference in references {
            let reference = reference?;
            let Ok(name) = reference.shorthand() else {
                continue;
            };
            let name = name.to_owned();
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            map.entry(crate::engines::git2::history::oid_bytes(commit.id()))
                .or_default()
                .push(name);
        }
        for names in map.values_mut() {
            names.sort();
        }
        Ok(map)
    })
}
