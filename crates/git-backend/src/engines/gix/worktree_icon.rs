use std::sync::atomic::AtomicBool;

use gix::bstr::ByteSlice;
use gix::dir::EntryRef;
use gix::dir::entry::Kind as DirKind;
use gix::dir::entry::Status as DirStatus;
use gix::dir::walk::{Action, Delegate, EmissionMode, ForDeletionMode};

use crate::error::{GitError, Result};

/// How deep (path components) an icon file may sit; directories at this
/// depth are not recursed into.
const MAX_DEPTH: usize = 4;
/// Hard cap on emitted entries so pathological worktrees cannot pin the
/// scheduler; whatever ranked best before the cap wins.
const MAX_ENTRIES: usize = 50_000;
/// Matches `IconConfig::max_bytes`.
const MAX_ICON_BYTES: usize = 5 * 1024 * 1024;

/// Best rank any file can achieve: favicon.ico at the worktree root.
const BEST_RANK: Rank = (1, 0, 0);

/// Stem priority: `favicon` beats `icon` beats `logo` beats
/// `apple-touch-icon`. Extension priority decides within one stem.
const STEMS: [(&str, u8); 4] = [
    ("favicon", 0),
    ("icon", 1),
    ("logo", 2),
    ("apple-touch-icon", 3),
];
const EXTENSIONS: [(&str, u8); 7] = [
    ("ico", 0),
    ("png", 1),
    ("svg", 2),
    ("jpg", 3),
    ("jpeg", 3),
    ("webp", 4),
    ("gif", 5),
];

pub struct WorktreeIcon {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub modified_ms: u64,
}

type Rank = (usize, u8, u8);

fn internal(err: impl std::fmt::Display) -> GitError {
    GitError::Internal {
        message: err.to_string(),
    }
}

/// Walks the worktree for an icon file, respecting `.gitignore` rules and
/// including untracked files. Returns `None` when nothing matches.
pub fn find_worktree_icon(workdir: &std::path::Path) -> Result<Option<WorktreeIcon>> {
    let session = crate::engines::gix::GixSession::discover(workdir)?;
    let repo = session.handle();
    let Some(worktree) = repo.workdir() else {
        return Ok(None);
    };

    let index = crate::engines::gix::session::open_index(&repo)?;
    let mut options = repo.dirwalk_options().map_err(internal)?;
    options.set_emit_untracked(EmissionMode::Matching);
    options.set_emit_ignored(None);
    options.set_emit_tracked(true);
    options.set_emit_empty_directories(false);

    let interrupt = AtomicBool::new(false);
    let mut delegate = IconDelegate::default();
    let _outcome = repo
        .dirwalk(
            &index,
            [] as [&gix::bstr::BStr; 0],
            &interrupt,
            options,
            &mut delegate,
        )
        .map_err(internal)?;

    let matched = match delegate.best {
        Some(matched) => matched,
        None => return Ok(None),
    };

    let path = worktree.join(matched.rela_path);
    let modified_ms = file_modified_ms(&path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        // The file may vanish between walk and read; treat as no icon.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > MAX_ICON_BYTES {
        return Ok(None);
    }
    let Some(content_type) = crate::icons::sniff_image(&bytes) else {
        return Ok(None);
    };

    Ok(Some(WorktreeIcon {
        bytes,
        content_type,
        modified_ms,
    }))
}

#[derive(Default)]
struct IconDelegate {
    best: Option<Matched>,
    visited: usize,
}

struct Matched {
    rela_path: String,
    rank: Rank,
}

impl Delegate for IconDelegate {
    fn emit(
        &mut self,
        entry: EntryRef<'_>,
        _collapsed_directory_status: Option<gix::dir::entry::Status>,
    ) -> Action {
        self.visited += 1;
        if self.visited > MAX_ENTRIES {
            return std::ops::ControlFlow::Break(());
        }
        if !matches!(entry.status, DirStatus::Tracked | DirStatus::Untracked) {
            return std::ops::ControlFlow::Continue(());
        }
        let is_file = |kind: Option<DirKind>| kind == Some(DirKind::File);
        if !is_file(entry.disk_kind) && !is_file(entry.index_kind) {
            return std::ops::ControlFlow::Continue(());
        }
        let Some(rank) = icon_rank(entry.rela_path.as_ref()) else {
            return std::ops::ControlFlow::Continue(());
        };
        if self.best.as_ref().is_some_and(|best| best.rank <= rank) {
            return std::ops::ControlFlow::Continue(());
        }
        self.best = Some(Matched {
            rela_path: entry.rela_path.to_str_lossy().into_owned(),
            rank,
        });
        if rank == BEST_RANK {
            return std::ops::ControlFlow::Break(());
        }
        std::ops::ControlFlow::Continue(())
    }

    fn can_recurse(
        &mut self,
        entry: EntryRef<'_>,
        for_deletion: Option<ForDeletionMode>,
        worktree_root_is_repository: bool,
    ) -> bool {
        if component_count(entry.rela_path.as_ref()) >= MAX_DEPTH {
            return false;
        }
        entry.status.can_recurse(
            entry.disk_kind,
            entry.pathspec_match,
            for_deletion,
            worktree_root_is_repository,
        )
    }
}

/// Ranks a path like `docs/brand/logo.svg` as (depth, stem, extension);
/// `None` when the filename is not a recognized icon name or lies deeper
/// than [`MAX_DEPTH`]. Lower ranks win.
fn icon_rank(rela_path: &[u8]) -> Option<Rank> {
    let depth = component_count(rela_path);
    if depth == 0 || depth > MAX_DEPTH {
        return None;
    }
    let file_name = rela_path.rsplit(|byte| *byte == b'/').next()?;
    let text = std::str::from_utf8(file_name).ok()?;
    let (stem, ext) = text.rsplit_once('.')?;
    let stem_rank = stem_rank(stem)?;
    let ext_rank = extension_rank(ext)?;
    Some((depth, stem_rank, ext_rank))
}

fn stem_rank(stem: &str) -> Option<u8> {
    let lowered = stem.to_ascii_lowercase();
    STEMS
        .iter()
        .find(|(name, _)| *name == lowered)
        .map(|(_, rank)| *rank)
}

fn extension_rank(extension: &str) -> Option<u8> {
    let lowered = extension.to_ascii_lowercase();
    EXTENSIONS
        .iter()
        .find(|(name, _)| *name == lowered)
        .map(|(_, rank)| *rank)
}

fn component_count(rela_path: &[u8]) -> usize {
    if rela_path.is_empty() {
        return 0;
    }
    rela_path.split(|byte| *byte == b'/').count()
}

fn file_modified_ms(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(
            || {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0)
            },
            |d| d.as_millis() as u64,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_recognized_icon_names() {
        assert_eq!(icon_rank(b"favicon.ico"), Some((1, 0, 0)));
        assert_eq!(icon_rank(b"icon.png"), Some((1, 1, 1)));
        assert_eq!(icon_rank(b"logo.svg"), Some((1, 2, 2)));
        assert_eq!(icon_rank(b"apple-touch-icon.png"), Some((1, 3, 1)));
        assert_eq!(icon_rank(b"docs/brand/logo.png"), Some((3, 2, 1)));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(icon_rank(b"Favicon.ICO"), Some((1, 0, 0)));
        assert_eq!(icon_rank(b"LOGO.Svg"), Some((1, 2, 2)));
    }

    #[test]
    fn rejects_non_icon_names_and_depths() {
        assert_eq!(icon_rank(b"main.rs"), None);
        assert_eq!(icon_rank(b"favicon.txt"), None);
        assert_eq!(icon_rank(b"myicon.png"), None);
        assert_eq!(icon_rank(b"icon"), None);
        assert_eq!(icon_rank(b"a/b/c/d/icon.png"), None);
        assert_eq!(icon_rank(b""), None);
    }

    #[test]
    fn deeper_but_prettier_names_lose() {
        let deep_favicon = icon_rank(b"docs/favicon.ico").unwrap();
        let root_logo = icon_rank(b"logo.png").unwrap();
        assert!(root_logo < deep_favicon);
    }
}
