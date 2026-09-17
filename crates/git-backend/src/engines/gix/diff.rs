use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use gix::bstr::{BStr, BString, ByteSlice};
use gix::diff::blob::{Algorithm, Diff as ImaraDiff, Hunk, InternedInput};
use gix::objs::tree::EntryMode;

use gix::status::index_worktree::BuiltinSubmoduleStatus;
use gix::status::plumbing::index_as_worktree::traits::{CompareBlobs, FastEq, SubmoduleStatus};
use gix::status::plumbing::index_as_worktree_with_renames::Sorting;
use gix::status::plumbing::index_as_worktree_with_renames::VisitEntry;

use crate::api::queries::DiffRequest;
use crate::domain::{RelativePath, RevisionSpec};
use crate::engines::gix::session::GixSession;
use crate::error::{GitError, Result};
use crate::streaming::model::{DiffComparison, DiffRow, DiffRowKind, SectionKind, SectionMeta};

const NO_NEWLINE_MARKER: &str = "\\ No newline at end of file";
/// git's binary heuristic: a NUL byte within the leading 8000 bytes marks a
/// side as binary.
const BINARY_SNIFF_LEN: usize = 8000;
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "svg",
];

type Oid = gix::hash::ObjectId;

/// Everything needed to materialize one section's rows on demand.
pub struct SectionPlan {
    pub meta: SectionMeta,
    old_id: Option<Oid>,
    new_id: Option<Oid>,
    /// The new side lives in the working tree under `meta.path`.
    worktree: bool,
    /// Symlinks diff their link target, not the target file's contents.
    symlink: bool,
}

/// Enumerates changed files without reading or diffing any content, so the
/// caller can emit `Started` immediately and stream sections lazily.
pub fn enumerate(
    session: &GixSession,
    request: &DiffRequest,
    blobs: &BlobCache,
) -> Result<Vec<SectionPlan>> {
    let repo = session.handle();
    let mut raw: Vec<Renamed> = match &request.comparison {
        DiffComparison::WorkingTree => head_vs_workdir(session, blobs)?,
        DiffComparison::Unstaged => unstaged_changes(&repo)?,
        DiffComparison::Staged => head_vs_index_changes(&repo)?,
        DiffComparison::TreeToTree { old, new } => {
            tree_vs_tree_changes(&repo, tree_at(&repo, old)?, tree_at(&repo, new)?)?
        }
        DiffComparison::CommitToParent { commit } => {
            let commit_obj = repo
                .find_object(resolve_commit_id(&repo, commit)?)
                .map_err(internal)?
                .try_into_commit()
                .map_err(internal)?;
            let new_tree = commit_obj.tree_id().map_err(internal)?.detach();
            let old_tree = match commit_obj.parent_ids().next() {
                Some(parent_id) => repo
                    .find_object(parent_id)
                    .map_err(internal)?
                    .try_into_commit()
                    .map_err(internal)?
                    .tree_id()
                    .map_err(internal)?
                    .detach(),
                // The root commit has no parent: diffing the empty tree lists
                // everything it introduced. Returning no sections here would
                // report an empty diff for the first commit of every repo.
                None => Oid::empty_tree(repo.object_hash()),
            };
            tree_vs_tree_changes(&repo, old_tree, new_tree)?
        }
    };

    if !request.paths.is_empty() {
        raw.retain(|delta| {
            request
                .paths
                .iter()
                .any(|p| p.as_bytes() == delta.path.as_bytes())
        });
    }
    raw.sort_by(|a, b| a.path.cmp(&b.path));

    let mut plans = Vec::with_capacity(raw.len());
    for (section_id, delta) in raw.into_iter().enumerate() {
        let image = is_image_path(delta.path.as_bytes());
        let binary = sniff_delta_binary(session, blobs, &delta);
        plans.push(SectionPlan {
            meta: SectionMeta {
                section_id: section_id as u32,
                path: RelativePath::parse(delta.path.to_str_lossy().as_ref())?,
                old_path: delta
                    .old_path
                    .as_ref()
                    .map(|p| RelativePath::parse(p.to_str_lossy().as_ref()))
                    .transpose()?,
                kind: delta.kind,
                binary,
                image,
                complete: false,
            },
            old_id: delta.old_id,
            new_id: delta.new_id,
            worktree: delta.worktree,
            symlink: delta.symlink,
        });
    }
    Ok(plans)
}

/// Per-job blob store shared by enumeration prefetch and materialization.
/// Loose-object reads dominate section cost; warming them while the status
/// machinery still runs hides that I/O from the streaming phase.
pub type BlobCache = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Oid, Arc<[u8]>>>>;

pub fn new_blob_cache() -> BlobCache {
    std::sync::Arc::default()
}

/// Loads missing blobs into `cache` on detached threads. Best effort: races
/// are harmless because materialization falls back to the ODB on a miss.
pub(crate) fn warm_blobs(
    cache: &BlobCache,
    session: &GixSession,
    ids: impl IntoIterator<Item = Oid>,
) {
    let mut missing: Vec<Oid> = {
        let guard = cache.lock().unwrap();
        ids.into_iter()
            .filter(|id| !guard.contains_key(id))
            .collect()
    };
    if missing.is_empty() {
        return;
    }
    missing.sort_unstable();
    missing.dedup();
    let readers = missing.len().min(3);
    let chunk_size = missing.len().div_ceil(readers);
    for chunk in missing.chunks(chunk_size) {
        let cache = Arc::clone(cache);
        let session = session.clone();
        let chunk = chunk.to_vec();
        let _ = std::thread::Builder::new()
            .name("gix-diff::warm".into())
            .spawn(move || {
                let repo = session.handle();
                for id in chunk {
                    if let Ok(blob) = repo.find_blob(id) {
                        let data: Arc<[u8]> = Arc::from(blob.data.as_bytes());
                        cache.lock().unwrap().entry(id).or_insert(data);
                    }
                }
            });
    }
}

fn cached_blob(repo: &gix::Repository, cache: &BlobCache, id: Oid) -> Result<Arc<[u8]>> {
    if let Some(data) = cache.lock().unwrap().get(&id) {
        return Ok(Arc::clone(data));
    }
    let blob = repo.find_blob(id).map_err(internal)?;
    let data: Arc<[u8]> = Arc::from(blob.data.as_bytes());
    cache.lock().unwrap().entry(id).or_insert(Arc::clone(&data));
    Ok(data)
}

/// One materialized section plus, when syntax highlighting applies, a
/// resumable highlighter the emitter drives chunk-by-chunk.
pub struct MaterializedSection {
    pub rows: Vec<DiffRow>,
    pub image: Option<crate::streaming::store::DiffImage>,
    pub highlighter: Option<crate::engines::gix::highlight::SectionHighlighter>,
}

/// Materializes one planned section's rows. Binary files produce a bare file
/// header, matching the libgit2 patch shape.
pub fn materialize(
    session: &GixSession,
    request: &DiffRequest,
    plan: &SectionPlan,
    blobs: &BlobCache,
) -> Result<MaterializedSection> {
    let repo = session.handle();
    let old_arc = match plan.old_id {
        Some(id) => Some(cached_blob(&repo, blobs, id)?),
        None => None,
    };
    let old_bytes: &[u8] = match &old_arc {
        Some(data) => data,
        None => &[],
    };
    let path_bytes = plan.meta.path.as_str().as_bytes();
    let new_arc = match plan.new_id {
        Some(id) => Some(cached_blob(&repo, blobs, id)?),
        None => None,
    };
    let new_buf;
    let new_bytes: &[u8] = match (&new_arc, plan.worktree) {
        (Some(data), _) => data,
        (None, true) if plan.symlink => {
            new_buf = link_target(session, path_bytes)?;
            new_buf.as_slice()
        }
        (None, true) => {
            new_buf = workdir_bytes(&repo, path_bytes)?;
            new_buf.as_slice()
        }
        (None, false) => {
            new_buf = Vec::new();
            new_buf.as_slice()
        }
    };

    if is_binary(old_bytes)
        || is_binary(new_bytes)
        || is_image_path(plan.meta.path.as_str().as_bytes())
    {
        let image = if plan.meta.image {
            Some(crate::streaming::store::DiffImage {
                old: Some(crate::streaming::store::DiffImageSide {
                    data: old_bytes.to_vec(),
                    mime_type: mime_type(plan.meta.path.as_str()),
                })
                .filter(|_| plan.old_id.is_some()),
                new: Some(crate::streaming::store::DiffImageSide {
                    data: new_bytes.to_vec(),
                    mime_type: mime_type(plan.meta.path.as_str()),
                })
                .filter(|_| plan.new_id.is_some() || plan.worktree),
            })
        } else {
            None
        };
        let mut rows = vec![file_header_row(&plan.meta)];
        if !plan.meta.image {
            rows.push(binary_notice_row());
        }
        return Ok(MaterializedSection {
            rows,
            image,
            highlighter: None,
        });
    }

    let rows = patch_rows(
        &plan.meta,
        old_bytes,
        new_bytes,
        request.context_lines.clamp(0, 64),
        request.interhunk_lines,
        request.ignore_whitespace,
    );
    let highlighter = crate::engines::gix::highlight::SectionHighlighter::new(
        plan.meta.path.as_str(),
        old_bytes,
        new_bytes,
    );
    Ok(MaterializedSection {
        rows,
        image: None,
        highlighter,
    })
}

fn internal(err: impl std::fmt::Display) -> GitError {
    GitError::Internal {
        message: err.to_string(),
    }
}

/// A change between the comparison endpoints, before section-id assignment.
struct Renamed {
    #[allow(dead_code)]
    path: BString,
    kind: SectionKind,
    old_id: Option<Oid>,
    new_id: Option<Oid>,
    old_path: Option<BString>,
    worktree: bool,
    symlink: bool,
    tombstone: bool,
}

impl Renamed {
    fn blobs(path: BString, kind: SectionKind, old_id: Option<Oid>, new_id: Option<Oid>) -> Self {
        Self {
            path,
            kind,
            old_id,
            new_id,
            old_path: None,
            worktree: false,
            symlink: false,
            tombstone: false,
        }
    }

    fn worktree(path: BString, kind: SectionKind, old_id: Option<Oid>, symlink: bool) -> Self {
        Self {
            path,
            kind,
            old_id,
            new_id: None,
            old_path: None,
            worktree: true,
            symlink,
            tombstone: false,
        }
    }

    fn tombstone() -> Self {
        Self {
            path: BString::default(),
            kind: SectionKind::Deleted,
            old_id: None,
            new_id: None,
            old_path: None,
            worktree: false,
            symlink: false,
            tombstone: true,
        }
    }

    fn is_tombstone(&self) -> bool {
        self.tombstone
    }
    fn new(
        path: BString,
        old_path: BString,
        old_id: Option<Oid>,
        new_id: Option<Oid>,
        worktree: bool,
        copied: bool,
    ) -> Self {
        Self {
            path,
            kind: if copied {
                SectionKind::Copied
            } else {
                SectionKind::Renamed
            },
            old_id,
            new_id,
            old_path: Some(old_path),
            worktree,
            symlink: false,
            tombstone: false,
        }
    }
}

/// Reads a worktree file as git stores it by running the configured clean
/// filter (crlf/eol/.gitattributes, and any clean drivers) over the raw bytes.
/// The index and HEAD blobs already hold the cleaned form, so diffing the raw
/// smudged file against them rewrites every line on repositories that
/// round-trip line endings. Skipping the filter would turn a one-line edit
/// into a full-file diff. Conversion failures fall back to the raw bytes so a
/// broken filter config cannot hide the diff entirely.
fn workdir_bytes(repo: &gix::Repository, path: &[u8]) -> Result<Vec<u8>> {
    let rela_path = gix::path::from_bstr(BStr::new(path));
    let root = repo.workdir().expect("working tree required");
    let raw = match std::fs::read(root.join(rela_path.as_ref())) {
        Ok(content) => content,
        Err(e) => {
            return Err(GitError::Internal {
                message: format!("reading worktree file failed: {e}"),
            });
        }
    };

    let (mut filter, index) = match repo.filter_pipeline(None) {
        Ok(parts) => parts,
        Err(_) => return Ok(raw),
    };
    let index_state = match &index {
        gix::worktree::IndexPersistedOrInMemory::Persisted(index) => &**index,
        gix::worktree::IndexPersistedOrInMemory::InMemory(index) => &**index,
    };
    let mut outcome =
        match filter.convert_to_git(std::io::Cursor::new(&raw), rela_path.as_ref(), index_state) {
            Ok(outcome) => outcome,
            Err(_) => return Ok(raw),
        };
    let mut cleaned = Vec::with_capacity(raw.len());
    match outcome.read_to_end(&mut cleaned) {
        Ok(_) => Ok(cleaned),
        Err(_) => Ok(raw),
    }
}

fn link_target(session: &GixSession, path: &[u8]) -> Result<Vec<u8>> {
    std::fs::read_link(worktree_path(session, path))
        .map(|p| p.as_os_str().as_encoded_bytes().to_vec())
        .map_err(|e| GitError::Internal {
            message: format!("reading symlink failed: {e}"),
        })
}

fn worktree_path(session: &GixSession, path: &[u8]) -> PathBuf {
    let root = session.workdir().expect("working tree required");
    root.join(gix::path::from_bstr(BStr::new(path)))
}

fn tree_at(repo: &gix::Repository, spec: &RevisionSpec) -> Result<Oid> {
    let id = repo
        .rev_parse_single(spec.as_str())
        .map_err(|_| GitError::InvalidRevision {
            spec: spec.as_str().to_owned(),
        })?;
    id.object()
        .map_err(internal)?
        .peel_to_kind(gix::object::Kind::Tree)
        .map(|t| t.id().detach())
        .map_err(internal)
}

fn resolve_commit_id(repo: &gix::Repository, spec: &RevisionSpec) -> Result<Oid> {
    repo.rev_parse_single(spec.as_str())
        .map(|id| id.detach())
        .map_err(|_| GitError::InvalidRevision {
            spec: spec.as_str().to_owned(),
        })
}

fn head_tree_id(repo: &gix::Repository) -> Result<Option<Oid>> {
    match repo.head_id() {
        Ok(id) => {
            let commit = id
                .object()
                .map_err(internal)?
                .try_into_commit()
                .map_err(internal)?;
            Ok(Some(commit.tree_id().map_err(internal)?.detach()))
        }
        // An unborn HEAD diffs against the empty tree, matching libgit2 so
        // staged files surface as additions before the first commit.
        Err(_) => Ok(Some(Oid::empty_tree(repo.object_hash()))),
    }
}

type StateMap = BTreeMap<BString, (EntryMode, Oid)>;

fn flatten_tree(repo: &gix::Repository, tree_id: Oid, out: &mut StateMap) -> Result<()> {
    enum Task {
        Enter(Oid, BString),
    }
    let mut stack = vec![Task::Enter(tree_id, BString::default())];
    while let Some(Task::Enter(id, prefix)) = stack.pop() {
        let tree = repo.find_tree(id).map_err(internal)?;
        for entry in tree.iter() {
            let entry = entry.map_err(internal)?;
            let mut path = prefix.clone();
            path.extend_from_slice(entry.filename());
            if entry.mode().is_tree() {
                let mut child_prefix = path;
                child_prefix.push(b'/');
                stack.push(Task::Enter((*entry.oid()).into(), child_prefix));
            } else {
                out.insert(path, (entry.mode(), (*entry.oid()).into()));
            }
        }
    }
    Ok(())
}

fn index_state(repo: &gix::Repository) -> Result<StateMap> {
    let index = crate::engines::gix::session::open_index(repo)?;
    let mut map = StateMap::new();
    for entry in index.entries() {
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            continue;
        }
        map.insert(
            entry.path(&index).to_owned(),
            (
                entry
                    .mode
                    .to_tree_entry_mode()
                    .unwrap_or_else(regular_file_mode),
                entry.id,
            ),
        );
    }
    Ok(map)
}
fn regular_file_mode() -> EntryMode {
    EntryMode::try_from(0o100644).expect("regular file mode is valid")
}

/// Exact-blob rename/copy pairing over deletion/addition candidates. Covers
/// pure renames; similarity-based detection beyond identical blobs is a
/// documented deviation.
fn pair_exact_renames(deltas: &mut Vec<Renamed>) {
    let mut removed: Vec<usize> = Vec::new();
    let mut added: Vec<usize> = Vec::new();
    for (idx, delta) in deltas.iter().enumerate() {
        match delta.kind {
            SectionKind::Deleted => removed.push(idx),
            SectionKind::Added => added.push(idx),
            _ => {}
        }
    }
    let mut consumed_old = vec![false; removed.len()];
    let mut consumed_new = vec![false; added.len()];
    let mut replacements: Vec<(usize, Renamed)> = Vec::new();

    for (ai, &add_idx) in added.iter().enumerate() {
        if consumed_new[ai] {
            continue;
        }
        let add = &deltas[add_idx];
        let Some(new_id) = add.new_id else {
            continue;
        };
        let mut matched = None;
        for (ri, &rem_idx) in removed.iter().enumerate() {
            if consumed_old[ri] {
                continue;
            }
            if deltas[rem_idx].old_id == Some(new_id) {
                matched = Some(ri);
                break;
            }
        }
        if let Some(ri) = matched {
            consumed_old[ri] = true;
            consumed_new[ai] = true;
            let rem_idx = removed[ri];
            let old_path = deltas[rem_idx].path.clone();
            let path = add.path.clone();
            replacements.push((
                add_idx,
                Renamed::new(path, old_path, add.old_id, Some(new_id), false, false),
            ));
            // Tombstone the consumed deletion; filtered out below.
            replacements.push((rem_idx, Renamed::tombstone()));
        }
    }
    for (idx, replacement) in replacements {
        deltas[idx] = replacement;
    }
    deltas.retain(|delta| !delta.is_tombstone());
}

fn tree_vs_tree_changes(
    repo: &gix::Repository,
    old_tree: Oid,
    new_tree: Oid,
) -> Result<Vec<Renamed>> {
    // gix's native diff skips subtrees whose oids match, so a commit touching
    // a few files never loads the rest of the tree; a flattened state-map
    // comparison reads every tree object on both sides every time.
    // Rewrite tracking stays off: pair_exact_renames supplies the same
    // libgit2-visible behavior for identical blobs without similarity I/O.
    let old_tree = repo.find_tree(old_tree).map_err(internal)?;
    let new_tree = repo.find_tree(new_tree).map_err(internal)?;
    let mut options = gix::diff::Options::default();
    options.track_rewrites(None);
    let changes = repo
        .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(options))
        .map_err(internal)?;

    let mut deltas = Vec::with_capacity(changes.len());
    for change in changes {
        match change {
            gix::object::tree::diff::ChangeDetached::Addition {
                location,
                entry_mode,
                id,
                ..
            } => {
                if entry_mode.is_tree() {
                    continue;
                }
                deltas.push(Renamed::blobs(location, SectionKind::Added, None, Some(id)));
            }
            gix::object::tree::diff::ChangeDetached::Deletion {
                location,
                entry_mode,
                id,
                ..
            } => {
                if entry_mode.is_tree() {
                    continue;
                }
                deltas.push(Renamed::blobs(
                    location,
                    SectionKind::Deleted,
                    Some(id),
                    None,
                ));
            }
            gix::object::tree::diff::ChangeDetached::Modification {
                location,
                previous_entry_mode,
                previous_id,
                entry_mode,
                id,
                ..
            } => {
                // Between two trees this variant marks a diverged subtree;
                // file-level type flips arrive as deletion/addition pairs.
                if previous_entry_mode.is_tree() || entry_mode.is_tree() {
                    continue;
                }
                deltas.push(Renamed::blobs(
                    location,
                    if type_flipped(previous_entry_mode, entry_mode) {
                        SectionKind::TypeChanged
                    } else {
                        SectionKind::Modified
                    },
                    Some(previous_id),
                    Some(id),
                ));
            }
            gix::object::tree::diff::ChangeDetached::Rewrite { .. } => {
                unreachable!("rewrite tracking is disabled")
            }
        }
    }
    pair_exact_renames(&mut deltas);
    Ok(deltas)
}

fn type_flipped(a: EntryMode, b: EntryMode) -> bool {
    fn kind(m: EntryMode) -> u8 {
        if m.is_link() {
            1
        } else if m.is_commit() {
            2
        } else {
            0
        }
    }
    kind(a) != kind(b)
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_LEN).any(|b| *b == 0)
}

/// Best-effort binary sniff for `enumerate`, so `Started.sections[].binary`
/// is correct before materialization. Failures mean text, never an error.
fn sniff_delta_binary(session: &GixSession, blobs: &BlobCache, delta: &Renamed) -> bool {
    if is_image_path(delta.path.as_bytes()) {
        return blob_or_worktree_is_binary(session, blobs, delta);
    }
    blob_or_worktree_is_binary(session, blobs, delta)
}

fn blob_or_worktree_is_binary(session: &GixSession, blobs: &BlobCache, delta: &Renamed) -> bool {
    let repo = session.handle();
    if let Some(id) = delta.old_id {
        if blob_is_binary(&repo, blobs, id) {
            return true;
        }
    }
    if let Some(id) = delta.new_id {
        if blob_is_binary(&repo, blobs, id) {
            return true;
        }
    }
    if delta.worktree && !delta.symlink {
        return worktree_prefix_is_binary(session, delta.path.as_bytes());
    }
    false
}

fn blob_is_binary(repo: &gix::Repository, cache: &BlobCache, id: Oid) -> bool {
    if let Some(data) = cache.lock().unwrap().get(&id) {
        return is_binary(data);
    }
    match repo.find_blob(id) {
        Ok(blob) => is_binary(blob.data.as_bytes()),
        Err(_) => false,
    }
}

fn worktree_prefix_is_binary(session: &GixSession, path: &[u8]) -> bool {
    let full = worktree_path(session, path);
    match std::fs::File::open(&full) {
        Ok(mut file) => {
            use std::io::Read;
            let mut buf = vec![0u8; BINARY_SNIFF_LEN];
            match file.read(&mut buf) {
                Ok(0) => false,
                Ok(n) => buf[..n].contains(&0),
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

fn binary_notice_row() -> DiffRow {
    DiffRow {
        kind: DiffRowKind::BinaryNotice,
        old_lineno: None,
        new_lineno: None,
        content: "Binary file differs".to_owned(),
        raw_hex: None,
        spans: None,
    }
}

fn mime_type(path: &str) -> String {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
    .to_owned()
}

fn is_image_path(path: &[u8]) -> bool {
    let path = String::from_utf8_lossy(path);
    let Some(extension) = path.rsplit('.').next() else {
        return false;
    };
    IMAGE_EXTENSIONS
        .iter()
        .any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

fn file_header_row(meta: &SectionMeta) -> DiffRow {
    let new = meta.path.as_str();
    let old = meta.old_path.as_ref().map(|p| p.as_str()).unwrap_or(new);
    DiffRow {
        kind: DiffRowKind::FileHeader,
        old_lineno: None,
        new_lineno: None,
        content: format!("diff --git a/{old} b/{new}"),
        raw_hex: None,
        spans: None,
    }
}

fn text_row(kind: DiffRowKind, line: &[u8], old: Option<u32>, new: Option<u32>) -> DiffRow {
    match std::str::from_utf8(line) {
        Ok(valid) => DiffRow {
            kind,
            old_lineno: old,
            new_lineno: new,
            content: valid.trim_end_matches(['\r', '\n']).to_owned(),
            raw_hex: None,
            spans: None,
        },
        Err(_) => DiffRow {
            kind,
            old_lineno: old,
            new_lineno: new,
            content: String::new(),
            raw_hex: Some(hex_encode(line)),
            spans: None,
        },
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Splits bytes into lines keeping their `\n` terminator (the final line may
/// lack one), matching xdiff's line tokens.
fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (idx, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            lines.push(&bytes[start..=idx]);
            start = idx + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

struct Group {
    /// Zero-based first line included from each side.
    old_start: u32,
    new_start: u32,
    /// Exclusive ends including trailing context.
    old_end: u32,
    new_end: u32,
    changes: Vec<Hunk>,
}

/// Builds patch rows replicating libgit2/xdiff output: git-format hunk
/// headers with function-context suffixes, per-side line numbers,
/// deletions-before-insertions, context merging between nearby changes
/// (extended by `interhunk_lines`), whitespace-insensitive comparison when
/// requested, and missing-newline markers after final unterminated lines.
fn patch_rows(
    meta: &SectionMeta,
    old: &[u8],
    new: &[u8],
    context: u32,
    interhunk: u32,
    ignore_whitespace: bool,
) -> Vec<DiffRow> {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let old_unterminated = !old.is_empty() && !old.ends_with(b"\n");
    let new_unterminated = !new.is_empty() && !new.ends_with(b"\n");

    let mut rows = vec![file_header_row(meta)];
    if old_lines.is_empty() && new_lines.is_empty() {
        return rows;
    }

    // Tokens are our own line slices so hunk indices map 1:1 onto them.
    // Whitespace-insensitive mode interns normalized copies while rendering
    // keeps the original bytes.
    let mut input: InternedInput<&[u8]> = InternedInput::default();
    // Normalized token copies live for the whole function; the interner
    // copies bytes, so nothing here outlives this scope.
    let ws_before: Vec<Vec<u8>>;
    let ws_after: Vec<Vec<u8>>;
    if ignore_whitespace {
        ws_before = old_lines.iter().map(|l| remove_whitespace(l)).collect();
        ws_after = new_lines.iter().map(|l| remove_whitespace(l)).collect();
        input.update_before(ws_before.iter().map(|v| v.as_slice()));
        input.update_after(ws_after.iter().map(|v| v.as_slice()));
    } else {
        input.update_before(old_lines.iter().copied());
        input.update_after(new_lines.iter().copied());
    }
    let mut diff = ImaraDiff::compute(Algorithm::Myers, &input);
    // git runs xdl_change_compact by default; imara implements the same
    // sliding-compaction heuristic for line diffs.
    diff.postprocess_lines(&input);

    let merge_gap = context * 2 + interhunk;
    let mut groups: Vec<Group> = Vec::new();
    for hunk in diff.hunks() {
        let old_lo = hunk.before.start.saturating_sub(context);
        let new_lo = hunk.after.start.saturating_sub(context);
        let old_hi = (hunk.before.end + context).min(old_lines.len() as u32);
        let new_hi = (hunk.after.end + context).min(new_lines.len() as u32);
        match groups.last_mut() {
            Some(last)
                if hunk.before.start.saturating_sub(last.old_end) <= merge_gap
                    && hunk.after.start.saturating_sub(last.new_end) <= merge_gap =>
            {
                last.old_end = last.old_end.max(old_hi);
                last.new_end = last.new_end.max(new_hi);
                last.changes.push(hunk);
            }
            _ => groups.push(Group {
                old_start: old_lo,
                new_start: new_lo,
                old_end: old_hi,
                new_end: new_hi,
                changes: vec![hunk],
            }),
        }
    }

    // Upper bound: every line on both sides appears at most once plus one
    // hunk header per group; overshoot only wastes capacity, never time.
    let group_rows: usize = groups
        .iter()
        .map(|g| ((g.old_end - g.old_start) + (g.new_end - g.new_start)) as usize + 1)
        .sum();
    rows.reserve(group_rows);

    for group in &groups {
        // git's function-context suffix: the last line preceding the hunk.
        let heading = if group.old_start > 0 {
            String::from_utf8_lossy(old_lines[(group.old_start - 1) as usize])
                .trim_end_matches(['\r', '\n'])
                .to_owned()
        } else {
            String::new()
        };
        rows.push(DiffRow {
            kind: DiffRowKind::HunkHeader,
            old_lineno: None,
            new_lineno: None,
            content: format_hunk_header(group, &heading),
            raw_hex: None,
            spans: None,
        });

        let mut old_no = group.old_start;
        let mut new_no = group.new_start;
        let mut changes = group.changes.iter().peekable();

        loop {
            if old_no >= group.old_end && new_no >= group.new_end {
                break;
            }
            let applies_next = changes
                .peek()
                .is_some_and(|c| c.before.start == old_no && c.after.start == new_no);
            match if applies_next { changes.next() } else { None } {
                Some(change) => {
                    for line in &old_lines[change.before.start as usize..change.before.end as usize]
                    {
                        rows.push(text_row(
                            DiffRowKind::Deletion,
                            line,
                            Some(old_no + 1),
                            None,
                        ));
                        if old_no + 1 == old_lines.len() as u32 && old_unterminated {
                            push_marker(&mut rows, MarkerSide::Old, Some(old_no + 1));
                        }
                        old_no += 1;
                    }
                    for line in &new_lines[change.after.start as usize..change.after.end as usize] {
                        rows.push(text_row(
                            DiffRowKind::Addition,
                            line,
                            None,
                            Some(new_no + 1),
                        ));
                        if new_no + 1 == new_lines.len() as u32 && new_unterminated {
                            push_marker(&mut rows, MarkerSide::New, Some(new_no + 1));
                        }
                        new_no += 1;
                    }
                }
                None => {
                    rows.push(text_row(
                        DiffRowKind::Context,
                        old_lines[old_no as usize],
                        Some(old_no + 1),
                        Some(new_no + 1),
                    ));
                    // Context lines are byte-identical on both sides, so at
                    // most one side can be the unterminated final line.
                    if old_no + 1 == old_lines.len() as u32 && old_unterminated {
                        push_marker(&mut rows, MarkerSide::Shared, Some(old_no + 1));
                    } else if new_no + 1 == new_lines.len() as u32 && new_unterminated {
                        push_marker(&mut rows, MarkerSide::Shared, Some(new_no + 1));
                    }
                    old_no += 1;
                    new_no += 1;
                }
            }
        }
    }
    rows
}

/// Removes every ASCII whitespace byte, matching `git diff -w` tokenization.
fn remove_whitespace(line: &[u8]) -> Vec<u8> {
    line.iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect()
}

enum MarkerSide {
    Old,
    New,
    Shared,
}

/// libgit2's marker rows carry the origin of the OPPOSITE side plus the line
/// number of the row they follow: a missing newline on the old side yields
/// '>' (Addition), on the new side '<' (Deletion), and a shared context tail
/// '=' (Context).
fn push_marker(rows: &mut Vec<DiffRow>, side: MarkerSide, lineno: Option<u32>) {
    let (kind, old_lineno, new_lineno) = match side {
        MarkerSide::Old => (DiffRowKind::Addition, lineno, None),
        MarkerSide::New => (DiffRowKind::Deletion, None, lineno),
        MarkerSide::Shared => (DiffRowKind::Context, lineno, lineno),
    };
    rows.push(DiffRow {
        kind,
        old_lineno,
        new_lineno,
        content: NO_NEWLINE_MARKER.to_owned(),
        raw_hex: None,
        spans: None,
    });
}

/// Formats `@@ -a,b +c,d @@ [heading]` with git's elision rules: counts of
/// exactly one are omitted entirely, empty sides render as `-0,0`.
fn format_hunk_header(group: &Group, heading: &str) -> String {
    format!(
        "@@ -{} +{} @@{}",
        header_side(group.old_start, group.old_end - group.old_start),
        header_side(group.new_start, group.new_end - group.new_start),
        if heading.is_empty() {
            String::new()
        } else {
            format!(" {heading}")
        }
    )
}

fn header_side(start_zero_based: u32, count: u32) -> String {
    if count == 0 {
        "0,0".to_owned()
    } else {
        let start = start_zero_based + 1;
        if count == 1 {
            start.to_string()
        } else {
            format!("{start},{count}")
        }
    }
}

// ---------------------------------------------------------------------------
// Candidate discovery via gix's native status machinery: parallel worktree
// checks, gitignore-aware untracked discovery, and rename tracking.
// ---------------------------------------------------------------------------

/// libgit2-compatible rename settings (50% similarity, no copies).
fn rewrites() -> gix::diff::Rewrites {
    gix::diff::Rewrites {
        copies: None,
        percentage: Some(0.5),
        limit: 1000,
        track_empty: false,
    }
}

/// Staged side only: HEAD -> index, including tracked renames/copies.
fn head_vs_index_changes(repo: &gix::Repository) -> Result<Vec<Renamed>> {
    let index = crate::engines::gix::session::open_index(repo)?;
    let mut deltas = Vec::new();
    if let Some(tree_id) = head_tree_id(repo)? {
        let mut pathspec = crate::engines::gix::session::unrestricted_pathspec(repo)?;
        repo.tree_index_status(
            &tree_id,
            &index,
            Some(&mut pathspec),
            gix::status::tree_index::TrackRenames::Given(rewrites()),
            |change, _lhs, _rhs| -> std::result::Result<std::ops::ControlFlow<()>, GitError> {
                push_tree_index_change(&mut deltas, change);
                Ok(std::ops::ControlFlow::Continue(()))
            },
        )
        .map_err(internal)?;
    }
    Ok(deltas)
}

fn push_tree_index_change(out: &mut Vec<Renamed>, change: gix::diff::index::ChangeRef<'_, '_>) {
    match change {
        gix::diff::index::ChangeRef::Addition { location, id, .. } => out.push(Renamed::blobs(
            location.as_bstr().to_owned(),
            SectionKind::Added,
            None,
            Some((*id).into()),
        )),
        gix::diff::index::ChangeRef::Deletion { location, id, .. } => out.push(Renamed::blobs(
            location.as_bstr().to_owned(),
            SectionKind::Deleted,
            Some((*id).into()),
            None,
        )),
        gix::diff::index::ChangeRef::Modification {
            location,
            previous_id,
            id,
            ..
        } => out.push(Renamed::blobs(
            location.as_bstr().to_owned(),
            SectionKind::Modified,
            Some((*previous_id).into()),
            Some((*id).into()),
        )),
        gix::diff::index::ChangeRef::Rewrite {
            source_location,
            source_id,
            location,
            id,
            copy,
            ..
        } => out.push(Renamed::new(
            location.as_bstr().to_owned(),
            source_location.as_bstr().to_owned(),
            Some((*source_id).into()),
            Some((*id).into()),
            false,
            copy,
        )),
    }
}

#[derive(Debug)]
enum WtChange {
    Modified {
        old_id: Oid,
    },
    TypeChanged {
        old_id: Oid,
        new_mode: EntryMode,
    },
    Removed {
        old_id: Oid,
    },
    Renamed {
        source: BString,
        old_id: Option<Oid>,
        copied: bool,
    },
    Conflict,
}

#[derive(Default)]
struct WtCollector {
    changes: Vec<(BString, WtChange)>,
    untracked: Vec<(BString, bool)>,
}

impl<'index> VisitEntry<'index> for WtCollector {
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
        use gix::dir::entry::Kind;
        use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};
        use gix::status::plumbing::index_as_worktree_with_renames::{Entry, RewriteSource};
        match entry {
            Entry::Modification {
                entry: index_entry,
                rela_path,
                status,
                ..
            } => match status {
                EntryStatus::Conflict { .. } => {
                    self.changes
                        .push((rela_path.to_owned(), WtChange::Conflict));
                }
                EntryStatus::Change(change) => {
                    let value = match change {
                        Change::Removed => WtChange::Removed {
                            old_id: index_entry.id,
                        },
                        Change::Type { worktree_mode } => WtChange::TypeChanged {
                            old_id: index_entry.id,
                            new_mode: worktree_mode
                                .to_tree_entry_mode()
                                .unwrap_or_else(regular_file_mode),
                        },
                        Change::Modification { .. } | Change::SubmoduleModification(_) => {
                            WtChange::Modified {
                                old_id: index_entry.id,
                            }
                        }
                    };
                    self.changes.push((rela_path.to_owned(), value));
                }
                EntryStatus::NeedsUpdate(_) | EntryStatus::IntentToAdd => {}
            },
            Entry::DirectoryContents { entry, .. } => {
                if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                    let symlink = entry.disk_kind == Some(Kind::Symlink);
                    self.untracked.push((entry.rela_path.clone(), symlink));
                }
            }
            Entry::Rewrite {
                source,
                dirwalk_entry,
                copy,
                ..
            } => {
                if let RewriteSource::RewriteFromIndex {
                    source_rela_path,
                    source_entry,
                    ..
                } = source
                {
                    self.changes.push((
                        dirwalk_entry.rela_path.clone(),
                        WtChange::Renamed {
                            source: source_rela_path.as_bstr().to_owned(),
                            old_id: Some(source_entry.id),
                            copied: copy,
                        },
                    ));
                }
            }
        }
    }
}

/// Stands in for [`gix::status::index_worktree::BuiltinSubmoduleStatus`] when
/// the repository has no `.gitmodules`; building the builtin machinery costs
/// a thread-local repo conversion plus submodule discovery per diff.
#[derive(Clone)]
struct NoSubmodules;

impl SubmoduleStatus for NoSubmodules {
    type Output = <BuiltinSubmoduleStatus as SubmoduleStatus>::Output;
    type Error = std::convert::Infallible;

    fn status(
        &mut self,
        _entry: &gix::index::Entry,
        _rela_path: &BStr,
    ) -> std::result::Result<Option<Self::Output>, Self::Error> {
        Ok(None)
    }
}

fn has_gitmodules(repo: &gix::Repository, index: &gix::index::File) -> bool {
    if repo
        .workdir()
        .is_some_and(|workdir| workdir.join(".gitmodules").is_file())
    {
        return true;
    }
    index
        .entries()
        .iter()
        .any(|entry| entry.path(index) == BStr::new(b".gitmodules"))
}

fn run_index_worktree_status<S>(
    repo: &gix::Repository,
    index: &gix::index::File,
    collector: &mut WtCollector,
    options: gix::status::index_worktree::Options,
    submodule: S,
) -> Result<()>
where
    S: SubmoduleStatus<Output = gix::submodule::Status> + Send + Clone,
{
    let should_interrupt = std::sync::atomic::AtomicBool::new(false);
    repo.index_worktree_status(
        index,
        [] as [&gix::bstr::BStr; 0],
        collector,
        FastEq,
        submodule,
        &mut gix::progress::Discard,
        &should_interrupt,
        options,
    )
    .map_err(internal)?;
    Ok(())
}

fn worktree_changes(repo: &gix::Repository) -> Result<WtCollector> {
    use gix::status::index_worktree::BuiltinSubmoduleStatus;

    let index = crate::engines::gix::session::open_index(repo)?;
    let dirwalk_options = {
        let mut options = repo.dirwalk_options().map_err(internal)?;
        options.set_emit_untracked(gix::dir::walk::EmissionMode::Matching);
        options.set_emit_ignored(None);
        options
    };
    let options = gix::status::index_worktree::Options {
        sorting: Some(Sorting::ByPathCaseSensitive),
        dirwalk_options: Some(dirwalk_options),
        rewrites: Some(rewrites()),
        thread_limit: None,
    };
    let mut collector = WtCollector::default();
    if has_gitmodules(repo, &index) {
        let submodule = BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default())
            .map_err(internal)?;
        run_index_worktree_status(repo, &index, &mut collector, options, submodule)?;
    } else {
        run_index_worktree_status(repo, &index, &mut collector, options, NoSubmodules)?;
    }

    Ok(collector)
}

/// Unstaged: index versus working tree.
fn unstaged_changes(repo: &gix::Repository) -> Result<Vec<Renamed>> {
    let collector = worktree_changes(repo)?;
    let mut deltas = Vec::new();
    for (path, change) in collector.changes {
        match change {
            WtChange::Modified { old_id } => deltas.push(Renamed::worktree(
                path,
                SectionKind::Modified,
                Some(old_id),
                false,
            )),
            WtChange::TypeChanged { old_id, new_mode } => deltas.push(Renamed::worktree(
                path,
                SectionKind::TypeChanged,
                Some(old_id),
                new_mode.is_link(),
            )),
            WtChange::Removed { old_id } => deltas.push(Renamed::blobs(
                path,
                SectionKind::Deleted,
                Some(old_id),
                None,
            )),
            WtChange::Renamed {
                source,
                old_id,
                copied,
            } => deltas.push(Renamed::new(path, source, old_id, None, true, copied)),
            WtChange::Conflict => deltas.push(Renamed::worktree(
                path,
                SectionKind::Conflicted,
                None,
                false,
            )),
        }
    }
    for (path, symlink) in collector.untracked {
        deltas.push(Renamed::worktree(path, SectionKind::Added, None, symlink));
    }
    Ok(deltas)
}

#[derive(Clone, Copy)]
enum Final {
    Blob(Oid, EntryMode),
    Worktree { symlink: bool },
    Absent,
}

#[derive(Default)]
struct MergeRow {
    final_state: Option<Final>,
    rename_from: Option<(BString, Option<Oid>, bool)>,
}

/// Working tree: HEAD versus merged index+worktree truth. Only changed paths
/// are touched; HEAD blobs resolve per path instead of flattening whole
/// trees.
fn head_vs_workdir(session: &GixSession, blobs: &BlobCache) -> Result<Vec<Renamed>> {
    // The staged and worktree passes are independent; overlap them.
    let side_session = session.clone();
    let warm_cache = Arc::clone(blobs);
    let staged_thread = std::thread::Builder::new()
        .name("gix-diff::staged".into())
        .spawn(move || staged_state(&side_session, &warm_cache))
        .map_err(|e| GitError::Internal {
            message: format!("spawning staged pass failed: {e}"),
        })?;

    let repo = session.handle();
    let wt = worktree_changes(&repo)?;
    let (old_state, staged) = staged_thread
        .join()
        .map_err(|_| GitError::internal("staged pass panicked"))??;
    working_tree_merge(old_state, staged, wt)
}

fn working_tree_merge(
    old_state: StateMap,
    staged: Vec<(BString, StagedState)>,
    wt: WtCollector,
) -> Result<Vec<Renamed>> {
    let mut rows: BTreeMap<BString, MergeRow> = BTreeMap::new();

    for (path, state) in staged {
        let row = rows.entry(path).or_default();
        match state {
            StagedState::Blob(id, mode) => row.final_state = Some(Final::Blob(id, mode)),
            StagedState::Absent => row.final_state = Some(Final::Absent),
        }
    }

    for (path, change) in wt.changes {
        match change {
            WtChange::Modified { .. } | WtChange::TypeChanged { .. } => {
                rows.entry(path).or_default().final_state =
                    Some(Final::Worktree { symlink: false });
            }
            WtChange::Conflict => {
                rows.entry(path).or_default().final_state =
                    Some(Final::Worktree { symlink: false });
            }
            WtChange::Removed { .. } => {
                rows.entry(path).or_default().final_state = Some(Final::Absent);
            }
            WtChange::Renamed {
                source,
                old_id,
                copied,
            } => {
                rows.entry(source.clone()).or_default().final_state = Some(Final::Absent);
                let row = rows.entry(path).or_default();
                row.final_state = Some(Final::Worktree { symlink: false });
                row.rename_from = Some((source, old_id, copied));
            }
        }
    }
    for (path, symlink) in wt.untracked {
        let row = rows.entry(path).or_default();
        row.final_state = Some(Final::Worktree { symlink });
        row.rename_from = None;
    }

    let mut deltas = Vec::new();
    for (path, row) in rows {
        let Some(final_state) = row.final_state else {
            continue;
        };
        let old = old_state.get(&path).copied();
        if let Some((src, src_hint, copied)) = &row.rename_from {
            let source_old = old_state.get(src).map(|(_, id)| id).copied().or(*src_hint);
            let new_id = match final_state {
                Final::Blob(id, _) => Some(id),
                _ => None,
            };
            deltas.push(Renamed::new(
                path,
                src.clone(),
                source_old.or(old.map(|(_, id)| id)),
                new_id,
                matches!(final_state, Final::Worktree { .. }),
                *copied,
            ));
            continue;
        }
        match (old, final_state) {
            (None, Final::Absent) => {}
            (Some(_), Final::Absent) => deltas.push(Renamed::blobs(
                path,
                SectionKind::Deleted,
                old.map(|(_, id)| id),
                None,
            )),
            (None, Final::Blob(id, _)) => {
                deltas.push(Renamed::blobs(path, SectionKind::Added, None, Some(id)))
            }
            (Some((om, oid)), Final::Blob(nid, nm)) => {
                if oid == nid && om == nm {
                    continue;
                }
                deltas.push(Renamed::blobs(
                    path,
                    if type_flipped(om, nm) {
                        SectionKind::TypeChanged
                    } else {
                        SectionKind::Modified
                    },
                    Some(oid),
                    Some(nid),
                ));
            }
            (old_side, Final::Worktree { symlink }) => {
                let kind = match (old_side, symlink) {
                    (None, true) => SectionKind::Added,
                    (Some(_), true) => SectionKind::TypeChanged,
                    (None, false) => SectionKind::Added,
                    (Some((om, _)), false) => {
                        if type_flipped(om, regular_file_mode()) {
                            SectionKind::TypeChanged
                        } else {
                            SectionKind::Modified
                        }
                    }
                };
                deltas.push(Renamed::worktree(
                    path,
                    kind,
                    old.map(|(_, id)| id),
                    symlink,
                ));
            }
        }
    }
    Ok(deltas)
}
/// Staged-side final state per changed path; rewrite destinations carry their
/// source so the working-tree merge can keep renames intact.
enum StagedState {
    Blob(Oid, EntryMode),
    Absent,
}

fn staged_state(
    session: &GixSession,
    blobs: &BlobCache,
) -> Result<(StateMap, Vec<(BString, StagedState)>)> {
    let repo = session.handle();
    // Pure HEAD-vs-index comparison: no worktree rules involved, so direct
    // state maps beat spinning up gix's tree-index diff machinery. The
    // flattened HEAD state doubles as the merge baseline, so it is computed
    // exactly once per diff.
    let mut old: StateMap = StateMap::new();
    if let Some(tree_id) = head_tree_id(&repo)? {
        flatten_tree(&repo, tree_id, &mut old)?;
    }
    let new = index_state(&repo)?;

    let mut out = Vec::new();
    let mut paths = old.keys().chain(new.keys()).cloned().collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    for path in paths {
        match (old.get(&path), new.get(&path)) {
            (Some(_), None) => out.push((path, StagedState::Absent)),
            (None, Some((mode, id))) => out.push((path, StagedState::Blob(*id, *mode))),
            (Some((om, oid)), Some((nm, nid))) => {
                if oid == nid && om == nm {
                    continue;
                }
                out.push((path, StagedState::Blob(*nid, *nm)));
            }
            (None, None) => {}
        }
    }
    // The worktree status pass still has I/O of its own to do; loading the
    // HEAD/index blobs this diff will materialize overlaps that window.
    let warmed_old = out
        .iter()
        .filter_map(|(path, _)| old.get(path).map(|(_, id)| *id));
    let warmed_new = out.iter().filter_map(|(_, state)| match state {
        StagedState::Blob(id, _) => Some(*id),
        StagedState::Absent => None,
    });
    warm_blobs(blobs, session, warmed_old.chain(warmed_new));
    Ok((old, out))
}
