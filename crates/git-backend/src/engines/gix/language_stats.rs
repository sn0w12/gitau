//! Worktree language shares, mirroring GitHub's Linguist model: tracked
//! and untracked (not ignored) text files, weighted by byte size, grouped
//! by detected language, each entry carrying its github-colors display
//! color. Shares sum to 100; files that map to no language (lockfiles,
//! bundles, binaries, dotfiles) leave the denominator.

use std::sync::atomic::AtomicBool;

use gix::bstr::ByteSlice;
use gix::dir::EntryRef;
use gix::dir::entry::Kind as DirKind;
use gix::dir::entry::Status as DirStatus;
use gix::dir::walk::{Action, Delegate, EmissionMode, ForDeletionMode};

use crate::domain::lang_colors::{Resolution, classify_language, color_for_language, resolve};
use crate::error::{GitError, Result};

/// One language's share of the worktree's code.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEntry {
    pub language: String,
    pub color: String,
    /// Percent of the classified byte total, 1..=100.
    pub percent: u64,
}

/// Per-language byte totals, ordered by share.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LanguageStats {
    pub entries: Vec<LanguageEntry>,
    /// Sum of the displayed percents; 0 when nothing was classified.
    pub total_percent: u64,
    /// Number of files excluded from stats by classifier rules.
    pub ignored_files: u64,
}

/// Files above this size count by size alone, skipping the read.
const READ_CAP_BYTES: usize = 64 * 1024;
/// Hard cap on visited entries so pathological trees cannot pin the
/// scheduler.
const MAX_ENTRIES: usize = 200_000;

/// Walks the worktree, respecting `.gitignore` rules, tracking tracked and
/// untracked files. Bare repositories yield empty stats.
pub fn language_stats(workdir: &std::path::Path) -> Result<LanguageStats> {
    let session = crate::engines::gix::GixSession::discover(workdir)?;
    let repo = session.handle();
    if repo.workdir().is_none() {
        return Ok(LanguageStats::default());
    }
    let worktree = repo.workdir().expect("checked above").to_owned();
    let index = crate::engines::gix::session::open_index(&repo)?;
    // Sizes for tracked files come free with the index; only untracked
    // files pay a stat at classification time.
    let tracked_sizes: std::collections::HashMap<&gix::bstr::BStr, u32> = index
        .entries()
        .iter()
        .filter(|entry| entry.stage() == gix::index::entry::Stage::Unconflicted)
        .map(|entry| (entry.path(&index), entry.stat.size))
        .collect();
    let mut options = repo.dirwalk_options().map_err(internal)?;
    options.set_emit_untracked(EmissionMode::Matching);
    options.set_emit_ignored(None);
    options.set_emit_tracked(true);
    options.set_emit_empty_directories(false);

    let interrupt = AtomicBool::new(false);
    let mut delegate = LanguageDelegate {
        worktree,
        tracked_sizes: Some(&tracked_sizes),
        ..LanguageDelegate::default()
    };
    let _outcome = repo
        .dirwalk(
            &index,
            [] as [&gix::bstr::BStr; 0],
            &interrupt,
            options,
            &mut delegate,
        )
        .map_err(internal)?;
    Ok(delegate.finish())
}

fn internal(err: impl std::fmt::Display) -> GitError {
    GitError::Internal {
        message: err.to_string(),
    }
}

#[derive(Default)]
struct LanguageDelegate<'a> {
    counts: std::collections::HashMap<&'static str, u64>,
    ignored_files: u64,
    visited: usize,
    worktree: std::path::PathBuf,
    tracked_sizes: Option<&'a std::collections::HashMap<&'a gix::bstr::BStr, u32>>,
}

impl<'a> Delegate for LanguageDelegate<'a> {
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
        let rela = entry.rela_path.to_str_lossy();
        let file_name = rela.rsplit('/').next().unwrap_or_default();
        // One name pass decides most files with no syscalls at all: the
        // stat and read below run only when content must break a tie.
        match resolve(Some(&rela), file_name) {
            Resolution::Decided(Some(language)) => {
                self.count_named(&rela, language, entry.status);
                return std::ops::ControlFlow::Continue(());
            }
            Resolution::Decided(None) => {
                self.ignored_files += 1;
                return std::ops::ControlFlow::Continue(());
            }
            Resolution::Disambiguate(_) | Resolution::Shebang => {}
        }
        let path = self.worktree.join(&*rela);
        let Ok(meta) = std::fs::metadata(&path) else {
            // The file may vanish between walk and read; count nothing.
            return std::ops::ControlFlow::Continue(());
        };
        let size = meta.len();
        // Nothing to weigh or the file is too big to sniff cheaply.
        if size == 0 || size > READ_CAP_BYTES as u64 {
            return std::ops::ControlFlow::Continue(());
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return std::ops::ControlFlow::Continue(()),
        };
        if let Some(language) = classify_language(Some(&rela), file_name, &bytes) {
            self.count_named(&rela, language, entry.status);
        } else {
            self.ignored_files += 1;
        }
        std::ops::ControlFlow::Continue(())
    }

    fn can_recurse(
        &mut self,
        entry: EntryRef<'_>,
        for_deletion: Option<ForDeletionMode>,
        worktree_root_is_repository: bool,
    ) -> bool {
        entry.status.can_recurse(
            entry.disk_kind,
            entry.pathspec_match,
            for_deletion,
            worktree_root_is_repository,
        )
    }
}

impl LanguageDelegate<'_> {
    /// Weights `language` by the file's byte size. Tracked files read the
    /// size from the index, everyone else pays one stat; the size may be
    /// stale by one edit for tracked files, which is noise at share
    /// granularity.
    fn count_named(&mut self, rela: &str, language: &'static str, status: DirStatus) {
        let size = match (status, self.tracked_sizes) {
            (DirStatus::Tracked, Some(sizes)) => sizes
                .get(gix::bstr::BStr::new(rela.as_bytes()))
                .copied()
                .unwrap_or(0) as u64,
            _ => std::fs::metadata(self.worktree.join(rela))
                .map(|meta| meta.len())
                .unwrap_or(0),
        };
        if size > 0 {
            *self.counts.entry(language).or_default() += size;
        }
    }

    fn finish(self) -> LanguageStats {
        let total: u64 = self.counts.values().sum();
        let mut raw: Vec<(&'static str, u64)> = self
            .counts
            .into_iter()
            .map(|(language, bytes)| {
                let percent = (bytes * 100).checked_div(total).unwrap_or(0);
                (language, percent)
            })
            .collect();
        // Rounding can drop a point or two; hand the residue to the
        // largest language so shares always sum to 100.
        let mut shown: u64 = raw.iter().map(|(_, percent)| *percent).sum();
        if !raw.is_empty() {
            let largest = raw
                .iter_mut()
                .max_by_key(|(language, percent)| (*percent, std::cmp::Reverse(*language)))
                .expect("non-empty");
            largest.1 += 100 - shown;
            shown = 100;
        }
        raw.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let entries = raw
            .into_iter()
            .filter(|(_, percent)| *percent > 0)
            .map(|(language, percent)| LanguageEntry {
                language: language.to_owned(),
                color: color_for_language(language).unwrap_or_default().to_owned(),
                percent,
            })
            .collect();
        LanguageStats {
            total_percent: shown,
            entries,
            ignored_files: self.ignored_files,
        }
    }
}
