use git2::{Status, StatusOptions};

use crate::api::changes::StatusOptions as ApiStatusOptions;
use crate::domain::{
    ChangeKind, ChangeSide, ConflictEntry, ConflictSide, Generation, ObjectId, RelativePath,
    SnapshotId, StatusEntry, StatusReport,
};
use crate::engines::git2::session::Git2Session;
use crate::error::{GitError, Result};

pub fn status(session: &Git2Session, opts: &ApiStatusOptions) -> Result<StatusReport> {
    session.with_repository(|repo| {
        let mut options = StatusOptions::new();
        options
            .include_untracked(opts.include_untracked)
            .recurse_untracked_dirs(opts.recurse_untracked_dirs)
            .include_ignored(opts.include_ignored)
            .include_unmodified(false)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .sort_case_insensitively(false)
            .update_index(true);

        let statuses = repo.statuses(Some(&mut options))?;

        // Loaded at most once per report and only when a conflict appears.
        let mut index_cache: Option<git2::Index> = None;

        let mut entries: Vec<StatusEntry> = Vec::with_capacity(statuses.len());
        let mut conflicts: Vec<ConflictEntry> = Vec::new();

        for entry in statuses.iter() {
            let Ok(path_str) = entry.path() else {
                continue;
            };
            let path = RelativePath::parse(&path_str.replace('\\', "/"))?;
            let flags = entry.status();

            if flags.contains(Status::CONFLICTED) {
                if index_cache.is_none() {
                    index_cache = Some(repo.index()?);
                }
                conflicts.push(conflict_entry(index_cache.as_ref().expect("index"), &path)?);
            }

            // Index side (staged): HEAD -> index.
            if let Some((kind, old_path)) = index_kind(&entry, flags)? {
                entries.push(StatusEntry::new(
                    ChangeSide::Index,
                    path.clone(),
                    old_path,
                    kind,
                ));
            }

            // Worktree side (unstaged): index -> working tree. Conflicted paths
            // surface here too so they appear in the unstaged list.
            match worktree_kind(&entry, flags)? {
                Some((kind, old_path)) => {
                    entries.push(StatusEntry::new(
                        ChangeSide::Worktree,
                        path.clone(),
                        old_path,
                        kind,
                    ));
                }
                None if flags.contains(Status::CONFLICTED) => {
                    entries.push(StatusEntry::new(
                        ChangeSide::Worktree,
                        path,
                        None,
                        ChangeKind::Conflicted,
                    ));
                }
                None => {}
            }
        }

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
    })
}

fn rename_old_path(delta: Option<git2::DiffDelta>) -> Result<Option<RelativePath>> {
    Ok(match delta {
        Some(delta) => delta
            .old_file()
            .path()
            .map(|old| RelativePath::parse(&old.to_string_lossy().replace('\\', "/")))
            .transpose()?,
        None => None,
    })
}

fn index_kind(
    entry: &git2::StatusEntry<'_>,
    flags: Status,
) -> Result<Option<(ChangeKind, Option<RelativePath>)>> {
    let kind = if flags.contains(Status::INDEX_NEW) {
        ChangeKind::Added
    } else if flags.contains(Status::INDEX_MODIFIED) {
        ChangeKind::Modified
    } else if flags.contains(Status::INDEX_DELETED) {
        ChangeKind::Deleted
    } else if flags.contains(Status::INDEX_TYPECHANGE) {
        ChangeKind::TypeChanged
    } else if flags.contains(Status::INDEX_RENAMED) {
        ChangeKind::Renamed
    } else {
        return Ok(None);
    };

    let old_path = if kind == ChangeKind::Renamed {
        rename_old_path(entry.head_to_index())?
    } else {
        None
    };
    Ok(Some((kind, old_path)))
}

fn worktree_kind(
    entry: &git2::StatusEntry<'_>,
    flags: Status,
) -> Result<Option<(ChangeKind, Option<RelativePath>)>> {
    let kind = if flags.contains(Status::WT_NEW) {
        ChangeKind::Untracked
    } else if flags.contains(Status::WT_MODIFIED) {
        ChangeKind::Modified
    } else if flags.contains(Status::WT_DELETED) {
        ChangeKind::Deleted
    } else if flags.contains(Status::WT_RENAMED) {
        ChangeKind::Renamed
    } else if flags.contains(Status::WT_TYPECHANGE) {
        ChangeKind::TypeChanged
    } else {
        return Ok(None);
    };

    let old_path = if kind == ChangeKind::Renamed {
        rename_old_path(entry.index_to_workdir())?
    } else {
        None
    };
    Ok(Some((kind, old_path)))
}

fn conflict_entry(index: &git2::Index, path: &RelativePath) -> Result<ConflictEntry> {
    let rel = path.to_path_buf();
    let mut sides = Vec::new();
    for stage in [1i32, 2, 3] {
        if let Some(entry) = index.get_path(&rel, stage) {
            sides.push(ConflictSide {
                stage: stage as u8,
                id: ObjectId::from_bytes(entry.id.as_bytes())?,
                mode: entry.mode,
            });
        }
    }
    if sides.is_empty() {
        return Err(GitError::internal(format!(
            "conflict stages missing for {path}"
        )));
    }
    Ok(ConflictEntry {
        path: path.clone(),
        sides,
    })
}

pub(crate) fn conflicted_paths(repo: &git2::Repository) -> Result<Vec<RelativePath>> {
    let statuses = repo.statuses(None)?;
    let mut out = Vec::new();
    for entry in statuses.iter() {
        if entry.status().contains(Status::CONFLICTED) {
            if let Ok(path) = entry.path() {
                out.push(RelativePath::parse(&path.replace('\\', "/"))?);
            }
        }
    }
    Ok(out)
}
