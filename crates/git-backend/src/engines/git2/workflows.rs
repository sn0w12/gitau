use git2::build::CheckoutBuilder;
use git2::{
    AnnotatedCommit, MergeOptions, RebaseOptions, Repository, StashApplyOptions, StashFlags,
};

use crate::api::mutations::{
    ConflictFile, ConflictStyle, MergeContinueRequest, MergeRequest, OperationKind, OperationState,
    ResolveConflictRequest, ResolveSide, RevertRequest, StashAction, StashPopRequest,
    StashPushRequest,
};
use crate::domain::{CommitSummary, ObjectId, RelativePath};
use crate::engines::git2::mutations::{
    domain_signature_from_git, head_commit, retry_locked, summarize_commit,
};
use crate::engines::git2::session::resolve_signature;
use crate::engines::git2::status;
use crate::error::{GitError, Result};
use crate::streaming::model::{DiffRow, DiffRowKind};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WorkflowOutcome {
    AlreadyUpToDate,
    FastForwarded { head: String },
    Merged { commit: String },
    Conflicted { paths: Vec<String> },
    Started { total_steps: u32 },
    Progressed { remaining: u32 },
    Finished { commit: Option<String> },
    Aborted,
}

fn annotated<'repo>(
    repo: &'repo Repository,
    spec: &crate::domain::RevisionSpec,
) -> Result<AnnotatedCommit<'repo>> {
    let commit = crate::streaming::pipeline::resolve_commit(repo, spec)?;
    Ok(repo.find_annotated_commit(commit.id())?)
}

fn conflicted_paths(repo: &Repository) -> Result<Vec<String>> {
    Ok(status::conflicted_paths(repo)?
        .into_iter()
        .map(|p| p.as_str().to_owned())
        .collect())
}

pub fn merge(repo: &Repository, request: &MergeRequest) -> Result<WorkflowOutcome> {
    if merge_in_progress(repo)? {
        return Err(GitError::Conflict {
            details: "a merge is already in progress; continue or abort first".into(),
        });
    }
    let annot = annotated(repo, &request.target)?;
    let (analysis, _) = repo.merge_analysis(&[&annot])?;

    if analysis.is_up_to_date() {
        return Ok(WorkflowOutcome::AlreadyUpToDate);
    }

    if analysis.is_fast_forward() && !request.no_fast_forward {
        return fast_forward(repo, annot.id());
    }

    if request.fast_forward_only {
        return Err(GitError::Conflict {
            details: "fast-forward is not possible for this merge".into(),
        });
    }

    let mut merge_opts = MergeOptions::new();
    let mut checkout = CheckoutBuilder::new();
    repo.merge(&[&annot], Some(&mut merge_opts), Some(&mut checkout))?;

    if has_conflicts(repo)? {
        write_merge_message(repo, request.message.as_deref())?;
        return Ok(WorkflowOutcome::Conflicted {
            paths: conflicted_paths(repo)?,
        });
    }

    let commit = create_merge_commit(repo, &[&annot], request.message.as_deref())?;
    repo.cleanup_state()?;
    Ok(WorkflowOutcome::Merged { commit })
}

pub fn merge_continue(
    repo: &Repository,
    request: &MergeContinueRequest,
) -> Result<WorkflowOutcome> {
    if !merge_in_progress(repo)? {
        return Err(GitError::Conflict {
            details: "no merge is in progress".into(),
        });
    }
    if has_conflicts(repo)? {
        return Ok(WorkflowOutcome::Conflicted {
            paths: conflicted_paths(repo)?,
        });
    }
    let parents = read_operation_heads(repo, "MERGE_HEAD")?;
    let annots = parents
        .iter()
        .map(|oid| repo.find_annotated_commit(*oid))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let annot_refs: Vec<&AnnotatedCommit<'_>> = annots.iter().collect();
    let stored_message = std::fs::read_to_string(repo.path().join("MERGE_MSG")).ok();
    let message = request
        .message
        .clone()
        .or(stored_message.map(|m| m.trim_end().to_owned()));
    let commit = create_merge_commit(repo, &annot_refs, message.as_deref())?;
    repo.cleanup_state()?;
    Ok(WorkflowOutcome::Finished {
        commit: Some(commit),
    })
}

pub fn merge_abort(repo: &Repository) -> Result<WorkflowOutcome> {
    if !merge_in_progress(repo)? {
        return Err(GitError::Conflict {
            details: "no merge is in progress".into(),
        });
    }
    abort_with_hard_reset(repo)
}

pub fn operation_state(repo: &Repository) -> Result<OperationState> {
    if !merge_in_progress(repo)? {
        return Ok(OperationState {
            kind: OperationKind::None,
            message: None,
            conflict_paths: Vec::new(),
            heads: Vec::new(),
        });
    }
    let heads = read_operation_heads(repo, "MERGE_HEAD")
        .map(|oids| oids.iter().map(|oid| oid.to_string()).collect())
        .unwrap_or_default();
    let message = std::fs::read_to_string(repo.path().join("MERGE_MSG"))
        .ok()
        .map(|text| text.trim_end().to_owned())
        .filter(|text| !text.is_empty());
    Ok(OperationState {
        kind: OperationKind::Merge,
        message,
        conflict_paths: conflicted_paths(repo)?
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect(),
        heads,
    })
}

pub fn resolve_conflict(repo: &Repository, request: &ResolveConflictRequest) -> Result<()> {
    let rel = RelativePath::parse(&request.path.replace('\\', "/"))?;
    let rel_buf = rel.to_path_buf();
    let mut index = repo.index()?;
    index.read(true)?;
    let conflict = index
        .conflict_get(&rel_buf)
        .map_err(|_| GitError::invalid_input(format!("no conflict for `{}`", rel.as_str())))?;
    let entry = match request.side {
        ResolveSide::Ours => conflict.our,
        ResolveSide::Theirs => conflict.their,
    };
    let Some(entry) = entry else {
        let workdir = repo
            .workdir()
            .ok_or_else(|| GitError::invalid_input("cannot resolve in a bare repository"))?;
        let absolute = workdir.join(&rel_buf);
        if absolute.exists() {
            std::fs::remove_file(&absolute).map_err(GitError::Io)?;
        }
        index.remove_path(&rel_buf)?;
        let _ = index.conflict_remove(&rel_buf);
        retry_locked(|| index.write())?;
        return Ok(());
    };
    let blob = repo.find_blob(entry.id)?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::invalid_input("cannot resolve in a bare repository"))?;
    let absolute = workdir.join(&rel_buf);
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent).map_err(GitError::Io)?;
    }
    std::fs::write(&absolute, blob.content()).map_err(GitError::Io)?;
    #[cfg(unix)]
    if entry.mode == 0o100755 {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&absolute, permissions);
    }
    index.add_path(&rel_buf)?;
    let _ = index.conflict_remove(&rel_buf);
    retry_locked(|| index.write())?;
    Ok(())
}

/// Cap for conflict-side panes: rows are buffered, not streamed, and the
/// renderer virtualizes them, so bound the JSON payload instead of the file.
const MAX_CONFLICT_ROWS: usize = 20_000;

pub fn conflict_file(repo: &Repository, path: &str, stage: u8) -> Result<ConflictFile> {
    if ![1, 2, 3].contains(&stage) {
        return Err(GitError::invalid_input("stage must be 1, 2, or 3"));
    }
    let rel = RelativePath::parse(&path.replace('\\', "/"))?;
    let rel_buf = rel.to_path_buf();
    let index = repo.index()?;
    let entry = index.get_path(&rel_buf, stage as i32).ok_or_else(|| {
        GitError::invalid_input(format!("no stage {stage} side for `{}`", rel.as_str()))
    })?;
    let blob = repo.find_blob(entry.id)?;
    let data = blob.content().to_vec();
    let binary = data[..data.len().min(8000)].contains(&0);
    if binary {
        return Ok(ConflictFile {
            path: rel,
            stage,
            revision: ObjectId::from_bytes(entry.id.as_bytes())?,
            binary,
            rows: Vec::new(),
            styles: Vec::new(),
            truncated: false,
        });
    }
    let (mut rows, truncated) = context_rows(&data);
    let styles = crate::engines::gix::highlight::attach(&mut rows, rel.as_str(), &data, &data)
        .into_iter()
        .map(|style| ConflictStyle {
            light: style.light,
            dark: style.dark,
            b: style.bold.then_some(true),
            i: style.italic.then_some(true),
            u: style.underline.then_some(true),
        })
        .collect();
    Ok(ConflictFile {
        path: rel,
        stage,
        revision: ObjectId::from_bytes(entry.id.as_bytes())?,
        binary,
        rows,
        styles,
        truncated,
    })
}

/// One context row per line, mirroring the diff pipeline's row shape so the
/// conflict panes render through the normal diff components.
fn context_rows(data: &[u8]) -> (Vec<DiffRow>, bool) {
    let mut rows = Vec::new();
    let mut truncated = false;
    let mut lineno = 0u32;
    for line in data.split_inclusive(|byte| *byte == b'\n') {
        lineno += 1;
        if rows.len() >= MAX_CONFLICT_ROWS {
            truncated = true;
            break;
        }
        rows.push(context_row(line, lineno));
    }
    if !truncated && !data.is_empty() && !data.ends_with(b"\n") {
        rows.push(DiffRow {
            kind: DiffRowKind::Context,
            old_lineno: Some(lineno),
            new_lineno: Some(lineno),
            content: "\\ No newline at end of file".to_owned(),
            raw_hex: None,
            spans: None,
        });
    }
    (rows, truncated)
}

fn context_row(line: &[u8], lineno: u32) -> DiffRow {
    match std::str::from_utf8(line) {
        Ok(valid) => DiffRow {
            kind: DiffRowKind::Context,
            old_lineno: Some(lineno),
            new_lineno: Some(lineno),
            content: valid.trim_end_matches(['\r', '\n']).to_owned(),
            raw_hex: None,
            spans: None,
        },
        Err(_) => DiffRow {
            kind: DiffRowKind::Context,
            old_lineno: Some(lineno),
            new_lineno: Some(lineno),
            content: String::new(),
            raw_hex: Some(line.iter().map(|byte| format!("{byte:02x}")).collect()),
            spans: None,
        },
    }
}

fn fast_forward(repo: &Repository, target: git2::Oid) -> Result<WorkflowOutcome> {
    let commit = repo.find_commit(target)?;
    let mut builder = CheckoutBuilder::new();
    builder.force();
    repo.checkout_tree(commit.as_object(), Some(&mut builder))?;

    if !repo.head_detached().unwrap_or(true) {
        let short = repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(str::to_owned));
        if let Some(branch_short) = short {
            let branch_ref = format!("refs/heads/{branch_short}");
            if let Ok(mut reference) = repo.find_reference(&branch_ref) {
                reference.set_target(target, "merge: fast-forward")?;
                return Ok(WorkflowOutcome::FastForwarded {
                    head: target.to_string(),
                });
            }
        }
    }
    repo.set_head_detached(target)?;
    Ok(WorkflowOutcome::FastForwarded {
        head: target.to_string(),
    })
}

fn has_conflicts(repo: &Repository) -> Result<bool> {
    let mut index = repo.index()?;
    index.read(true)?;
    Ok(index.has_conflicts())
}

pub(crate) fn merge_in_progress(repo: &Repository) -> Result<bool> {
    Ok(state_file(repo, "MERGE_HEAD")?.is_some())
}

pub(crate) fn rebase_in_progress(repo: &Repository) -> bool {
    let git_dir = repo.path();
    git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists()
}

fn state_file(repo: &Repository, name: &str) -> Result<Option<std::path::PathBuf>> {
    let path = repo.path().join(name);
    Ok(if path.exists() { Some(path) } else { None })
}

fn write_merge_message(repo: &Repository, message: Option<&str>) -> Result<()> {
    let Some(text) = message else {
        return Ok(());
    };
    if !text.trim().is_empty() {
        let path = repo.path().join("MERGE_MSG");
        std::fs::write(path, text).map_err(GitError::Io)?;
    }
    Ok(())
}

fn create_merge_commit(
    repo: &Repository,
    parents: &[&AnnotatedCommit<'_>],
    message: Option<&str>,
) -> Result<String> {
    let signature = resolve_signature(repo)?;
    let mut index = repo.index()?;
    index.read(true)?;
    retry_locked(|| index.write())?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let head = head_commit(repo)?.ok_or_else(|| GitError::Conflict {
        details: "cannot merge onto an unborn branch".into(),
    })?;

    let default_message = format!("Merge commit '{}'", parents[0].id());
    let message = message.unwrap_or(default_message.as_str());

    let mut owned_parents: Vec<git2::Commit<'_>> = vec![head];
    for annot in parents {
        owned_parents.push(repo.find_commit(annot.id())?);
    }
    let parent_refs: Vec<&git2::Commit<'_>> = owned_parents.iter().collect();

    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        parent_refs.as_slice(),
    )?;
    Ok(oid.to_string())
}

fn read_operation_heads(repo: &Repository, file: &str) -> Result<Vec<git2::Oid>> {
    let Some(path) = state_file(repo, file)? else {
        return Err(GitError::internal(format!("{file} missing")));
    };
    let content = std::fs::read_to_string(path).map_err(GitError::Io)?;
    let mut oids = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.len() >= 40 && !trimmed.starts_with('#') {
            oids.push(
                git2::Oid::from_str(trimmed).map_err(|e| GitError::Internal {
                    message: e.to_string(),
                })?,
            );
        }
    }
    if oids.is_empty() {
        return Err(GitError::internal(format!("{file} contains no object ids")));
    }
    Ok(oids)
}

fn abort_with_hard_reset(repo: &Repository) -> Result<WorkflowOutcome> {
    let head = head_commit(repo)?.ok_or_else(|| GitError::Conflict {
        details: "nothing to reset to".into(),
    })?;
    repo.reset(head.as_object(), git2::ResetType::Hard, None)?;
    repo.cleanup_state()?;
    remove_state_files(repo);
    Ok(WorkflowOutcome::Aborted)
}

fn remove_state_files(repo: &Repository) {
    for name in ["CHERRY_PICK_HEAD", "REVERT_HEAD", "MERGE_MSG", "REVERT_MSG"] {
        let path = repo.path().join(name);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn cherry_pick(
    repo: &Repository,
    target: &crate::domain::RevisionSpec,
) -> Result<WorkflowOutcome> {
    let commit = crate::streaming::pipeline::resolve_commit(repo, target)?;
    repo.cherrypick(&commit, None)?;

    if has_conflicts(repo)? {
        return Ok(WorkflowOutcome::Conflicted {
            paths: conflicted_paths(repo)?,
        });
    }
    let summary = finish_pick(repo, "CHERRY_PICK_HEAD", None)?;
    Ok(WorkflowOutcome::Finished {
        commit: summary.map(|c| c.id.hex()),
    })
}

pub fn revert(repo: &Repository, request: &RevertRequest) -> Result<WorkflowOutcome> {
    let commit = crate::streaming::pipeline::resolve_commit(repo, &request.target)?;
    let mut revert_options = git2::RevertOptions::new();
    if request.parent_index > 0 {
        revert_options.mainline(request.parent_index);
    }
    repo.revert(&commit, Some(&mut revert_options))?;

    if has_conflicts(repo)? {
        return Ok(WorkflowOutcome::Conflicted {
            paths: conflicted_paths(repo)?,
        });
    }
    let original_summary = summarize_commit(repo, commit.id())?;
    let message = format!(
        "Revert \"{}\"\n\nThis reverts commit {}.",
        original_summary.summary_line,
        commit.id()
    );
    let summary = finish_pick(repo, "REVERT_HEAD", Some(message.as_str()))?;
    Ok(WorkflowOutcome::Finished {
        commit: summary.map(|c| c.id.hex()),
    })
}

pub fn pick_continue(repo: &Repository, state_file_name: &str) -> Result<WorkflowOutcome> {
    if has_conflicts(repo)? {
        return Ok(WorkflowOutcome::Conflicted {
            paths: conflicted_paths(repo)?,
        });
    }
    let summary = finish_pick(repo, state_file_name, None)?;
    Ok(WorkflowOutcome::Finished {
        commit: summary.map(|c| c.id.hex()),
    })
}

pub fn pick_abort(repo: &Repository) -> Result<WorkflowOutcome> {
    abort_with_hard_reset(repo)
}

fn finish_pick(
    repo: &Repository,
    state_name: &str,
    message_override: Option<&str>,
) -> Result<Option<CommitSummary>> {
    let Some(path) = state_file(repo, state_name)? else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(&path).map_err(GitError::Io)?;
    let oid_text = content.lines().next().unwrap_or("").trim();
    let picked_oid = git2::Oid::from_str(oid_text).map_err(|e| GitError::Internal {
        message: e.to_string(),
    })?;
    let picked = repo.find_commit(picked_oid)?;

    let author = match state_name {
        "CHERRY_PICK_HEAD" => {
            let original_author = domain_signature_from_git(picked.author());
            signature_from_parts(&original_author)?
        }
        _ => resolve_signature(repo)?,
    };
    let committer = resolve_signature(repo)?;
    let message = message_override
        .map(str::to_owned)
        .unwrap_or_else(|| String::from_utf8_lossy(picked.message_bytes()).into_owned());

    let mut index = repo.index()?;
    index.read(true)?;
    retry_locked(|| index.write())?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head = head_commit(repo)?.ok_or_else(|| GitError::Conflict {
        details: "HEAD vanished".into(),
    })?;

    let oid = repo.commit(Some("HEAD"), &author, &committer, &message, &tree, &[&head])?;
    std::fs::remove_file(&path).map_err(GitError::Io)?;
    remove_state_files(repo);
    repo.cleanup_state()?;
    Ok(Some(summarize_commit(repo, oid)?))
}

fn signature_from_parts(sig: &crate::domain::Signature) -> Result<git2::Signature<'static>> {
    Ok(git2::Signature::now(&sig.name, &sig.email)?)
}

pub fn stash_push(repo: &mut Repository, request: &StashPushRequest) -> Result<ObjectId> {
    if request.paths.is_empty() {
        let signature = resolve_signature(repo)?;
        let mut flags = StashFlags::empty();
        if request.include_untracked {
            flags |= StashFlags::INCLUDE_UNTRACKED;
        }
        if request.keep_index {
            flags |= StashFlags::KEEP_INDEX;
        }
        let oid = repo.stash_save2(&signature, request.message.as_deref(), Some(flags))?;
        return ObjectId::from_bytes(oid.as_bytes());
    }

    stash_push_paths(repo, request)
}

/// Path-limited stash. libgit2's own pathspec stash reverts the entire working
/// tree to HEAD, which drops unrelated edits, so build the stash commit here
/// and revert only the selected paths.
fn stash_push_paths(repo: &Repository, request: &StashPushRequest) -> Result<ObjectId> {
    if request.keep_index {
        return Err(GitError::Unsupported {
            capability: "keep-index with a path-limited stash".into(),
        });
    }

    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::invalid_input("cannot stash in a bare repository"))?
        .to_path_buf();
    let signature = resolve_signature(repo)?;
    let base = head_commit(repo)?
        .ok_or_else(|| GitError::invalid_input("cannot stash without a commit"))?;
    let base_tree = base.tree()?;
    let head_desc = head_description(repo, &base);

    let mut repo_index = repo.index()?;
    let index_tree = repo.find_tree(repo_index.write_tree_to(repo)?)?;
    let index_commit = repo.find_commit(repo.commit(
        None,
        &signature,
        &signature,
        &format!("index on {head_desc}"),
        &index_tree,
        &[&base],
    )?)?;

    let mut paths_index = git2::Index::new()?;
    paths_index.read_tree(&base_tree)?;
    let mut paths = request.paths.clone();
    paths.sort();
    paths.dedup();
    let mut remove_after = Vec::new();
    for path in &paths {
        let file = std::path::Path::new(path);
        let status = repo.status_file(file).unwrap_or(git2::Status::CURRENT);
        if status == git2::Status::CURRENT {
            return Err(GitError::invalid_input(format!(
                "no changes to stash for `{path}`"
            )));
        }
        if status.contains(git2::Status::WT_NEW) {
            if !request.include_untracked {
                return Err(GitError::invalid_input(format!(
                    "`{path}` is untracked; include untracked files to stash it"
                )));
            }
            remove_after.push(path.clone());
        }
        if status.contains(git2::Status::WT_DELETED) {
            let _ = paths_index.remove_path(file);
            continue;
        }
        let absolute = workdir.join(path);
        if absolute.is_file() {
            paths_index.add(&index_entry(
                path,
                repo.blob_path(&absolute)?,
                file_mode(&absolute),
            ))?;
        } else {
            let _ = paths_index.remove_path(file);
        }
    }
    let paths_tree = repo.find_tree(paths_index.write_tree_to(repo)?)?;

    let message = match request.message.as_deref() {
        Some(message) if !message.trim().is_empty() => format!("On {head_desc}: {message}"),
        _ => format!("WIP on {head_desc}"),
    };
    let stash = repo.commit(
        None,
        &signature,
        &signature,
        &message,
        &paths_tree,
        &[&base, &index_commit],
    )?;

    repo.reference_ensure_log("refs/stash")?;
    let reflog_message = message.lines().next().unwrap_or("WIP");
    repo.reference("refs/stash", stash, true, reflog_message)?;

    let mut checkout = CheckoutBuilder::new();
    checkout.force().remove_untracked(false);
    for path in &paths {
        checkout.path(path.as_str());
    }
    repo.checkout_head(Some(&mut checkout))?;
    for path in &remove_after {
        let _ = std::fs::remove_file(workdir.join(path));
    }
    repo.index()?.read(true)?;

    ObjectId::from_bytes(stash.as_bytes())
}

fn head_description(repo: &Repository, commit: &git2::Commit<'_>) -> String {
    let branch = repo
        .head()
        .ok()
        .filter(|head| head.is_branch())
        .and_then(|head| head.shorthand().ok().map(str::to_owned))
        .unwrap_or_else(|| "(no branch)".to_owned());
    let short = &commit.id().to_string()[..7];
    let summary = commit.summary().ok().flatten().unwrap_or("");
    format!("{branch}: {short} {summary}").trim_end().to_owned()
}

fn index_entry(path: &str, id: git2::Oid, mode: u32) -> git2::IndexEntry {
    git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

#[cfg(unix)]
fn file_mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.permissions().mode() & 0o111 != 0 => 0o100755,
        _ => 0o100644,
    }
}

#[cfg(not(unix))]
fn file_mode(_path: &std::path::Path) -> u32 {
    0o100644
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub commit: String,
}

pub fn stash_list(repo: &mut Repository) -> Result<Vec<StashEntry>> {
    let mut entries = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        entries.push(StashEntry {
            index,
            message: message.to_owned(),
            commit: oid.to_string(),
        });
        true
    })?;
    Ok(entries)
}

pub fn stash_pop(repo: &mut Repository, request: &StashPopRequest) -> Result<()> {
    let mut options = StashApplyOptions::default();
    match request.action {
        StashAction::Pop => repo.stash_pop(request.index, Some(&mut options))?,
        StashAction::Apply => repo.stash_apply(request.index, Some(&mut options))?,
        StashAction::Drop => repo.stash_drop(request.index)?,
    }
    if has_conflicts(repo)? {
        return Err(GitError::Conflict {
            details: "stash apply produced conflicts".into(),
        });
    }
    Ok(())
}

pub fn start_rebase(
    repo: &Repository,
    upstream: &crate::domain::RevisionSpec,
    onto: Option<&crate::domain::RevisionSpec>,
) -> Result<WorkflowOutcome> {
    let upstream_annot = annotated(repo, upstream)?;
    let onto_annot = match onto {
        Some(spec) => Some(annotated(repo, spec)?),
        None => None,
    };
    let signature = resolve_signature(repo)?;
    let mut rebase_options = RebaseOptions::new();
    let mut rebase = repo.rebase(
        None,
        Some(&upstream_annot),
        onto_annot.as_ref(),
        Some(&mut rebase_options),
    )?;
    let total = rebase.len() as u32;
    drive_rebase(&mut rebase, &signature)?;
    Ok(WorkflowOutcome::Started { total_steps: total })
}

pub fn continue_rebase(repo: &Repository) -> Result<WorkflowOutcome> {
    if !rebase_in_progress(repo) {
        return Err(GitError::Conflict {
            details: "no rebase is in progress".into(),
        });
    }
    let signature = resolve_signature(repo)?;
    let mut rebase_options = RebaseOptions::new();
    let mut rebase = repo.open_rebase(Some(&mut rebase_options))?;
    let remaining = rebase.len() as u32;
    drive_rebase(&mut rebase, &signature)?;
    Ok(WorkflowOutcome::Progressed { remaining })
}

pub fn abort_rebase(repo: &Repository) -> Result<WorkflowOutcome> {
    if !rebase_in_progress(repo) {
        return Err(GitError::Conflict {
            details: "no rebase is in progress".into(),
        });
    }
    if let Ok(mut rebase_options) = repo.open_rebase(Some(&mut RebaseOptions::new())) {
        rebase_options.abort()?;
    } else {
        abort_with_hard_reset(repo)?;
    }
    Ok(WorkflowOutcome::Aborted)
}

fn drive_rebase(rebase: &mut git2::Rebase<'_>, signature: &git2::Signature<'_>) -> Result<()> {
    while let Some(operation) = rebase.next() {
        let operation = operation?;
        if operation.kind() == Some(git2::RebaseOperationType::Exec) {
            continue;
        }
        rebase.commit(None, signature, None)?;
    }
    rebase.finish(Some(signature))?;
    Ok(())
}
