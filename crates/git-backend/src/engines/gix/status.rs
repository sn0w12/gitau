use std::sync::atomic::AtomicBool;

use gix::bstr::ByteSlice;
use gix::diff::Rewrites as DiffRewrites;
use gix::diff::index::ChangeRef;
use gix::status::index_worktree::BuiltinSubmoduleStatus;
use gix::status::plumbing::index_as_worktree::traits::{CompareBlobs, FastEq, SubmoduleStatus};
use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};
use gix::status::plumbing::index_as_worktree_with_renames::{Sorting, VisitEntry};

use crate::api::changes::StatusOptions as ApiStatusOptions;
use crate::domain::{
    ChangeKind, ChangeSide, ConflictEntry, ConflictSide, Generation, ObjectId, RelativePath,
    SnapshotId, StatusEntry, StatusReport,
};
use crate::engines::gix::session::GixSession;
use crate::error::{GitError, Result};

/// libgit2-compatible rename settings: 50% similarity, no copy detection,
/// default fuzzy-match limit.
fn rewrites() -> DiffRewrites {
    DiffRewrites {
        copies: None,
        percentage: Some(0.5),
        limit: 1000,
        track_empty: false,
    }
}

fn internal(err: impl std::fmt::Display) -> GitError {
    GitError::Internal {
        message: err.to_string(),
    }
}

pub fn status(session: &GixSession, opts: &ApiStatusOptions) -> Result<StatusReport> {
    // The staged and worktree sides are independent computations sharing only
    // the session, so overlap them on two threads. Each builds its own
    // thread-local handle; gix::Repository is not Sync.
    let (staged, worktree) = std::thread::scope(|scope| {
        let staged = scope.spawn(|| staged_side(&session.thread_local()));
        let worktree = scope.spawn(|| worktree_side(session, &session.thread_local(), opts));
        let staged = staged
            .join()
            .unwrap_or_else(|e| std::panic::resume_unwind(e));
        let worktree = worktree
            .join()
            .unwrap_or_else(|e| std::panic::resume_unwind(e));
        (staged, worktree)
    });

    let (mut entries, mut conflicts) = staged?;
    let (worktree_entries, worktree_conflicts) = worktree?;
    entries.extend(worktree_entries);
    conflicts.extend(worktree_conflicts);

    entries.sort_by(|a, b| {
        a.path
            .as_str()
            .cmp(b.path.as_str())
            .then_with(|| a.side.id_prefix().cmp(b.side.id_prefix()))
    });

    Ok(StatusReport {
        snapshot_id: SnapshotId(0),
        generation: Generation(0),
        entries,
        conflicts,
    })
}

fn staged_side(repo: &gix::Repository) -> Result<(Vec<StatusEntry>, Vec<ConflictEntry>)> {
    let mut entries: Vec<StatusEntry> = Vec::new();
    let conflicts: Vec<ConflictEntry> = Vec::new();

    let index = crate::engines::gix::session::open_index(repo)?;

    // An unborn HEAD compares the index against the empty tree, matching
    // libgit2 so staged files surface as additions before the first commit.
    let tree_id = match repo.head_id() {
        Ok(head_id) => {
            let head_commit = head_id
                .object()
                .map_err(internal)?
                .try_into_commit()
                .map_err(internal)?;
            head_commit.tree_id().map_err(internal)?.detach()
        }
        Err(_) => gix::hash::ObjectId::empty_tree(repo.object_hash()),
    };
    {
        let mut pathspec = crate::engines::gix::session::unrestricted_pathspec(repo)?;
        repo.tree_index_status(
            &tree_id,
            &index,
            Some(&mut pathspec),
            gix::status::tree_index::TrackRenames::Given(rewrites()),
            |change, _lhs, _rhs| -> std::result::Result<std::ops::ControlFlow<()>, GitError> {
                if let Some(entry) = staged_entry(change)? {
                    entries.push(entry);
                }
                Ok(std::ops::ControlFlow::Continue(()))
            },
        )
        .map_err(internal)?;
    }
    Ok((entries, conflicts))
}

fn worktree_side(
    session: &GixSession,
    repo: &gix::Repository,
    opts: &ApiStatusOptions,
) -> Result<(Vec<StatusEntry>, Vec<ConflictEntry>)> {
    let mut entries: Vec<StatusEntry> = Vec::new();
    let mut conflicts: Vec<ConflictEntry> = Vec::new();
    if session.is_bare() {
        return Ok((entries, conflicts));
    }

    let mut index = crate::engines::gix::session::open_index(repo)?;
    let dirwalk_options = build_dirwalk_options(repo, opts)?;
    let options = gix::status::index_worktree::Options {
        sorting: Some(Sorting::ByPathCaseSensitive),
        dirwalk_options,
        rewrites: Some(rewrites()),
        thread_limit: None,
    };
    let submodule = BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default())
        .map_err(internal)?;
    let should_interrupt = AtomicBool::new(false);
    let mut collector = Collector::default();
    repo.index_worktree_status(
        &index,
        [] as [&gix::bstr::BStr; 0],
        &mut collector,
        FastEq,
        submodule,
        &mut gix::progress::Discard,
        &should_interrupt,
        options,
    )
    .map_err(internal)?;
    entries.extend(collector.entries);
    conflicts.extend(collector.conflicts);
    persist_stat_refresh(&mut index, &collector.stat_updates);
    Ok((entries, conflicts))
}

/// Persist refreshed stat data back to the index, the same thing `git status`
/// and libgit2's `update_index(true)` do. Best effort: if the index is locked
/// by a concurrent mutation the refresh is simply skipped.
fn persist_stat_refresh(
    index: &mut gix::index::File,
    updates: &[(usize, gix::index::entry::Stat)],
) {
    if updates.is_empty() {
        return;
    }
    {
        let entries = index.entries_mut();
        for (entry_index, stat) in updates {
            if let Some(entry) = entries.get_mut(*entry_index) {
                entry.stat = *stat;
            }
        }
    }
    // A stale-but-valid tree-cache could make later commits capture outdated
    // subtree content, so drop it whenever entries were touched.
    index.remove_tree();
    let _ = index.write(gix::index::write::Options::default());
}

fn build_dirwalk_options(
    repo: &gix::Repository,
    opts: &ApiStatusOptions,
) -> Result<Option<gix::dirwalk::Options>> {
    if !opts.include_untracked && !opts.include_ignored {
        return Ok(None);
    }
    let mut options = repo.dirwalk_options().map_err(internal)?;
    options.set_emit_untracked(if opts.recurse_untracked_dirs {
        gix::dir::walk::EmissionMode::Matching
    } else {
        gix::dir::walk::EmissionMode::CollapseDirectory
    });
    if opts.include_ignored {
        options.set_emit_ignored(Some(gix::dir::walk::EmissionMode::Matching));
    } else {
        options.set_emit_ignored(None);
    }
    Ok(Some(options))
}

fn rel_path(bytes: impl AsRef<[u8]>) -> Result<RelativePath> {
    RelativePath::parse(&String::from_utf8_lossy(bytes.as_ref()).replace('\\', "/"))
}

fn staged_entry(change: ChangeRef<'_, '_>) -> Result<Option<StatusEntry>> {
    let side = ChangeSide::Index;
    let entry = match change {
        ChangeRef::Addition { location, .. } => {
            StatusEntry::new(side, rel_path(location.as_ref())?, None, ChangeKind::Added)
        }
        ChangeRef::Deletion { location, .. } => StatusEntry::new(
            side,
            rel_path(location.as_ref())?,
            None,
            ChangeKind::Deleted,
        ),
        ChangeRef::Modification {
            location,
            previous_entry_mode,
            entry_mode,
            ..
        } => {
            let kind = if type_changed(previous_entry_mode, entry_mode) {
                ChangeKind::TypeChanged
            } else {
                ChangeKind::Modified
            };
            StatusEntry::new(side, rel_path(location.as_ref())?, None, kind)
        }
        ChangeRef::Rewrite {
            source_location,
            copy,
            ..
        } => {
            let kind = if copy {
                ChangeKind::Copied
            } else {
                ChangeKind::Renamed
            };
            // Same libgit2 quirk as the worktree side: the source path fills
            // both `path` and `old_path` for staged renames.
            let path = rel_path(source_location.as_ref())?;
            StatusEntry::new(side, path.clone(), Some(path), kind)
        }
    };
    Ok(Some(entry))
}

/// libgit2 reports a typechange only when the file *type* flips
/// (file/symlink/submodule); executable-bit-only changes stay modifications.
fn type_changed(a: gix::index::entry::Mode, b: gix::index::entry::Mode) -> bool {
    fn kind(mode: gix::index::entry::Mode) -> Option<u8> {
        let mode = mode.to_tree_entry_mode()?;
        Some(if mode.is_link() {
            1
        } else if mode.is_commit() {
            2
        } else {
            0
        })
    }
    kind(a).zip(kind(b)).is_some_and(|(a, b)| a != b)
}

#[derive(Default)]
struct Collector {
    entries: Vec<StatusEntry>,
    conflicts: Vec<ConflictEntry>,
    // (entry_index, stat) pairs gix offers after each call; writing them back
    // is what git and libgit2 do, so racy entries stop being content-hashed
    // on every subsequent call.
    stat_updates: Vec<(usize, gix::index::entry::Stat)>,
}

impl<'index> VisitEntry<'index> for Collector {
    type ContentChange = <FastEq as CompareBlobs>::Output;
    type SubmoduleStatus = <BuiltinSubmoduleStatus as SubmoduleStatus>::Output;

    fn visit_entry(
        &mut self,
        entry: gix::status::plumbing::index_as_worktree_with_renames::Entry<
            'index,
            Self::ContentChange,
            Self::SubmoduleStatus,
        >,
    ) {
        use gix::dir::entry::{Kind, Status as DirStatus};
        use gix::status::plumbing::index_as_worktree_with_renames::{Entry, RewriteSource};
        match entry {
            Entry::Modification {
                entry_index,
                rela_path,
                status,
                ..
            } => match status {
                EntryStatus::Conflict {
                    entries: stages, ..
                } => {
                    let Ok(path) = rel_path(rela_path) else {
                        return;
                    };
                    push_conflict(&mut self.conflicts, &path, &stages);
                    self.entries.push(StatusEntry::new(
                        ChangeSide::Worktree,
                        path,
                        None,
                        ChangeKind::Conflicted,
                    ));
                }
                EntryStatus::Change(change) => {
                    let Some(kind) = worktree_kind(change) else {
                        return;
                    };
                    if let Ok(path) = rel_path(rela_path) {
                        self.entries
                            .push(StatusEntry::new(ChangeSide::Worktree, path, None, kind));
                    }
                }
                // Stat refresh only; libgit2 performed the equivalent update
                // silently through `update_index(true)`.
                EntryStatus::IntentToAdd => {}
                EntryStatus::NeedsUpdate(stat) => self.stat_updates.push((entry_index, stat)),
            },
            Entry::DirectoryContents { entry, .. } => match (&entry.status, entry.disk_kind) {
                (DirStatus::Untracked, Some(Kind::Directory)) => {
                    // libgit2 lists collapsed untracked directories with a
                    // trailing slash.
                    if let Ok(text) = rel_path(entry.rela_path.as_bytes()) {
                        let path = RelativePath::parse(&format!("{}/", text.as_str()))
                            .expect("slash-terminated path stays valid");
                        self.entries.push(StatusEntry::new(
                            ChangeSide::Worktree,
                            path,
                            None,
                            ChangeKind::Untracked,
                        ));
                    }
                }
                (DirStatus::Untracked, _) => {
                    if let Ok(path) = rel_path(entry.rela_path.as_bytes()) {
                        self.entries.push(StatusEntry::new(
                            ChangeSide::Worktree,
                            path,
                            None,
                            ChangeKind::Untracked,
                        ));
                    }
                }
                // Ignored entries carried an IGNORED-only flag in the libgit2
                // engine and were dropped by its mapper; keep them absent.
                _ => {}
            },
            Entry::Rewrite {
                source,
                dirwalk_entry: _,
                copy,
                ..
            } => {
                let kind = if copy {
                    ChangeKind::Copied
                } else {
                    ChangeKind::Renamed
                };
                // libgit2's status entries report renames with the SOURCE path
                // in both `path` and `old_path`; the destination only appears
                // in the diff layer. Replicate that shape verbatim.
                let source_path = match &source {
                    RewriteSource::RewriteFromIndex {
                        source_rela_path, ..
                    } => rel_path(source_rela_path.as_bytes()).ok(),
                    RewriteSource::CopyFromDirectoryEntry {
                        source_dirwalk_entry,
                        ..
                    } => rel_path(source_dirwalk_entry.rela_path.as_bytes()).ok(),
                };
                if let Some(path) = source_path.clone() {
                    self.entries.push(StatusEntry::new(
                        ChangeSide::Worktree,
                        path,
                        source_path,
                        kind,
                    ));
                }
            }
        }
    }
}

fn worktree_kind(
    change: Change<(), <BuiltinSubmoduleStatus as SubmoduleStatus>::Output>,
) -> Option<ChangeKind> {
    match change {
        Change::Removed => Some(ChangeKind::Deleted),
        Change::Type { .. } => Some(ChangeKind::TypeChanged),
        Change::Modification { .. } => Some(ChangeKind::Modified),
        Change::SubmoduleModification(_) => Some(ChangeKind::Modified),
    }
}

fn push_conflict(
    out: &mut Vec<ConflictEntry>,
    path: &RelativePath,
    stages: &[Option<gix::status::plumbing::index_as_worktree::ConflictIndexEntry>; 3],
) {
    let sides: Vec<ConflictSide> = stages
        .iter()
        .enumerate()
        .filter_map(|(idx, stage)| {
            let stage_entry = stage.as_ref()?;
            Some(ConflictSide {
                stage: (idx + 1) as u8,
                id: ObjectId::from(stage_entry.id),
                mode: stage_entry.mode.bits(),
            })
        })
        .collect();
    if sides.is_empty() {
        return;
    }
    out.push(ConflictEntry {
        path: path.clone(),
        sides,
    });
}
