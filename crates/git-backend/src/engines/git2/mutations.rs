use std::path::Path;

use git2::build::CheckoutBuilder;
use git2::{BranchType, IndexAddOption, ResetType, Signature};

use crate::api::mutations::{
    AmendRequest, BranchCreateRequest, CheckoutRequest, CommitRequest, ResetKind, ResetRequest,
    TagCreateRequest,
};
use crate::domain::{
    BranchInfo, CommitSummary, ObjectId, RelativePath, Signature as DomainSignature, TagInfo,
    UpstreamRef,
};
use crate::engines::git2::session::resolve_signature;
use crate::error::{GitError, Result};

pub fn stage(repo: &git2::Repository, paths: &[String], all: bool) -> Result<()> {
    let mut index = repo.index()?;
    if all || paths.iter().any(|p| p == "." || p == "*") {
        index.add_all(
            ["*"],
            IndexAddOption::DEFAULT,
            Some(&mut |_path, _matched| 0),
        )?;
    } else {
        let rels = validate_paths(paths)?;
        for rel in rels {
            let fs_path = repo
                .workdir()
                .unwrap_or(Path::new("."))
                .join(rel.to_path_buf());
            if fs_path.exists() {
                index.add_path(&rel.to_path_buf())?;
                if index.conflict_get(&rel.to_path_buf()).is_ok() {
                    index.conflict_remove(&rel.to_path_buf())?;
                }
            } else {
                match index.get_path(&rel.to_path_buf(), 0) {
                    Some(_) => {
                        index.remove_path(&rel.to_path_buf())?;
                    }
                    None => continue,
                }
            }
        }
    }
    // The gix status engine refreshes racy stat data back into the index
    // under index.lock; a stage landing in that window retries instead of
    // surfacing a spurious lock error to the user.
    retry_locked(|| index.write())?;
    Ok(())
}

pub(crate) fn retry_locked<T>(
    mut f: impl FnMut() -> std::result::Result<T, git2::Error>,
) -> Result<T> {
    const MAX_ATTEMPTS: u32 = 20;
    let mut err = None;
    for _ in 0..MAX_ATTEMPTS {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) if e.code() == git2::ErrorCode::Locked => {
                err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(GitError::RepositoryLocked {
        details: err
            .map(|e| e.message().to_owned())
            .unwrap_or_else(|| "index stayed locked".to_owned()),
    })
}

pub fn unstage(repo: &git2::Repository, paths: &[String]) -> Result<()> {
    let rels = validate_paths(paths)?;
    if rels.is_empty() {
        return Ok(());
    }
    let head_object = head_commit(repo)?.map(|commit| commit.as_object().to_owned());
    let specs: Vec<String> = rels.iter().map(|r| r.as_str().to_owned()).collect();
    repo.reset_default(head_object.as_ref(), &specs)?;
    Ok(())
}

pub fn discard(repo: &git2::Repository, paths: &[String], all: bool) -> Result<()> {
    let rels = if all {
        Vec::new()
    } else {
        validate_paths(paths)?
    };

    if all {
        repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
        return Ok(());
    }

    let mut tracked_paths: Vec<String> = Vec::new();
    for rel in &rels {
        let workdir = repo.workdir().unwrap_or(Path::new("."));
        let fs_path = workdir.join(rel.to_path_buf());
        let in_index = repo.index()?.get_path(&rel.to_path_buf(), 0).is_some();
        if !fs_path.exists() && in_index {
            tracked_paths.push(rel.as_str().to_owned());
            continue;
        }
        if !in_index && fs_path.is_file() {
            std::fs::remove_file(&fs_path).map_err(GitError::Io)?;
            remove_empty_parents(workdir, rel);
            continue;
        }
        tracked_paths.push(rel.as_str().to_owned());
    }

    if !tracked_paths.is_empty() {
        let mut builder = CheckoutBuilder::new();
        builder.force();
        for path in &tracked_paths {
            builder.path(path);
        }
        repo.checkout_head(Some(&mut builder))?;
    }
    Ok(())
}

fn remove_empty_parents(workdir: &Path, rel: &RelativePath) {
    let mut current = rel.parent();
    while let Some(parent) = current {
        let dir = workdir.join(parent.to_path_buf());
        match std::fs::read_dir(&dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    break;
                }
                let _ = std::fs::remove_dir(&dir);
            }
            Err(_) => break,
        }
        current = parent.parent();
    }
}

pub fn commit(
    repo: &git2::Repository,
    request: &CommitRequest,
) -> Result<(CommitSummary, Vec<crate::api::hooks::HookRunResult>)> {
    if request.message.trim().is_empty() {
        return Err(GitError::invalid_input("commit message must not be empty"));
    }

    if request.stage_all {
        stage(repo, &[], true)?;
    }

    let mut hook_runs = Vec::new();
    if request.run_hooks {
        super::hooks::capture_existing(repo, "pre-commit", &mut hook_runs)?;
        // pre-commit gates the commit like git's own pipeline.
        if let Some(result) = hook_runs.last() {
            if !result.success {
                return Err(GitError::Conflict {
                    details: hook_failure_details(result),
                });
            }
        }
    }

    let mut index = repo.index()?;
    retry_locked(|| index.write())?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let head = head_commit(repo)?;
    if let Some(parent) = &head {
        if parent.tree_id() == tree_id && !request.allow_empty {
            return Err(GitError::invalid_input(
                "no staged changes; use allowEmpty to force an empty commit",
            ));
        }
    }

    let committer = resolve_signature(repo)?;
    let author = match &request.author {
        Some(author) => signature_from_domain(author)?,
        None => committer.clone(),
    };

    let parents: Vec<&git2::Commit<'_>> = head.iter().collect();
    let oid = repo.commit(
        Some("HEAD"),
        &author,
        &committer,
        request.message.trim(),
        &tree,
        parents.as_slice(),
    )?;

    if request.run_hooks {
        super::hooks::capture_existing(repo, "post-commit", &mut hook_runs)?;
        // post-commit cannot undo anything; a failure is recorded and
        // surfaced instead of discarding the successful commit result.
    }
    let summary = summarize_commit(repo, oid)?;
    Ok((summary, hook_runs))
}

pub fn amend(repo: &git2::Repository, request: &AmendRequest) -> Result<CommitSummary> {
    let head = head_commit(repo)?.ok_or_else(|| GitError::invalid_input("nothing to amend"))?;
    let mut index = repo.index()?;
    retry_locked(|| index.write())?;
    let _tree_id = index.write_tree()?;

    let committer = resolve_signature(repo)?;
    let author = match &request.author {
        Some(author) => signature_from_domain(author)?,
        None => {
            let decoded_author = domain_signature_from_git(head.author());
            signature_from_domain(&decoded_author)?
        }
    };
    let message = request
        .message
        .clone()
        .unwrap_or_else(|| String::from_utf8_lossy(head.message_bytes()).into_owned());

    let oid = head.amend(
        Some("HEAD"),
        Some(&author),
        Some(&committer),
        None,
        Some(message.as_str()),
        None,
    )?;
    summarize_commit(repo, oid)
}

pub fn checkout(repo: &git2::Repository, request: &CheckoutRequest) -> Result<()> {
    if !request.paths.is_empty() {
        let rels = validate_paths(&request.paths)?;
        let obj = repo.revparse_single(request.target.as_str()).map_err(|_| {
            GitError::InvalidRevision {
                spec: request.target.as_str().to_owned(),
            }
        })?;
        let tree = obj.peel_to_tree()?;
        let mut builder = CheckoutBuilder::new();
        builder.update_index(true);
        if request.force {
            builder.force();
        } else {
            builder.safe();
        }
        for rel in rels {
            builder.path(rel.as_str());
        }
        repo.checkout_tree(tree.as_object(), Some(&mut builder))?;
        return Ok(());
    }

    let reference = repo.find_reference(request.target.as_str()).or_else(|_| {
        repo.find_branch(request.target.as_str(), BranchType::Local)
            .map(|branch| branch.into_reference())
    });

    match reference {
        Ok(reference) => {
            let commit = reference.peel_to_commit()?;
            let is_local_branch = reference
                .name()
                .map(|n| n.starts_with("refs/heads/"))
                .unwrap_or(false);
            let mut builder = CheckoutBuilder::new();
            if request.force {
                builder.force();
            }
            if is_local_branch {
                repo.set_head(reference.name().expect("named ref"))?;
                repo.checkout_tree(commit.as_object(), Some(&mut builder))?;
            } else {
                repo.set_head_detached(commit.id())?;
                repo.checkout_tree(commit.as_object(), Some(&mut builder))?;
            }
            Ok(())
        }
        Err(_) => {
            let commit = crate::streaming::pipeline::resolve_commit(repo, &request.target)?;
            repo.set_head_detached(commit.id())?;
            let mut builder = CheckoutBuilder::new();
            if request.force {
                builder.force();
            }
            repo.checkout_tree(commit.as_object(), Some(&mut builder))?;
            Ok(())
        }
    }
}

pub fn reset(repo: &git2::Repository, request: &ResetRequest) -> Result<()> {
    let commit = crate::streaming::pipeline::resolve_commit(repo, &request.target)?;
    let kind = match request.kind {
        ResetKind::Soft => ResetType::Soft,
        ResetKind::Mixed => ResetType::Mixed,
        ResetKind::Hard => ResetType::Hard,
    };
    repo.reset(commit.as_object(), kind, None)?;
    Ok(())
}

pub fn create_branch(repo: &git2::Repository, request: &BranchCreateRequest) -> Result<BranchInfo> {
    let name = crate::domain::BranchName::parse(&request.name, "branch")?;
    let target_spec = request.start_point.clone().unwrap_or_default();
    let commit = crate::streaming::pipeline::resolve_commit(repo, &target_spec)?;
    let branch = repo.branch(name.as_str(), &commit, request.force)?;

    if request.checkout {
        repo.set_head(branch.get().name().expect("branch ref"))?;
        let mut builder = CheckoutBuilder::new();
        repo.checkout_tree(commit.as_object(), Some(&mut builder))?;
    }

    branch_info(repo, name.clone())
}

pub fn delete_branch(
    repo: &git2::Repository,
    name: &crate::domain::BranchName,
    force: bool,
) -> Result<()> {
    let mut branch = repo.find_branch(name.as_str(), BranchType::Local)?;
    let current_head_name = head_commit(repo)?
        .is_some()
        .then(|| {
            repo.head()
                .ok()
                .and_then(|h| h.shorthand().ok().map(str::to_owned))
        })
        .flatten();
    if current_head_name.as_deref() == Some(name.as_str()) {
        return Err(GitError::Conflict {
            details: format!("cannot delete the checked out branch `{name}`"),
        });
    }
    if !force {
        let upstream = branch.upstream().ok();
        if let (Some(upstream), Some(local_tip)) = (&upstream, branch.get().peel_to_commit().ok()) {
            let (ahead, _behind) =
                repo.graph_ahead_behind(local_tip.id(), upstream.get().peel_to_commit()?.id())?;
            if ahead > 0 {
                return Err(GitError::Conflict {
                    details: format!(
                        "branch `{name}` has {ahead} unpushed commits; pass force to delete anyway"
                    ),
                });
            }
        }
    }
    branch.delete()?;
    Ok(())
}

pub fn rename_branch(
    repo: &git2::Repository,
    old: &crate::domain::BranchName,
    new: &crate::domain::BranchName,
    force: bool,
) -> Result<BranchInfo> {
    let mut branch = repo.find_branch(old.as_str(), BranchType::Local)?;
    branch.rename(new.as_str(), force)?;
    branch_info(repo, new.clone())
}

pub fn list_branches(repo: &git2::Repository) -> Result<Vec<BranchInfo>> {
    let head_shorthand = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(str::to_owned).ok());
    let mut out = Vec::new();
    let local = repo.branches(Some(BranchType::Local))?;
    for branch in local {
        let (branch, _) = branch?;
        let Some(name) = branch.name()?.map(str::to_owned) else {
            continue;
        };
        out.push(branch_info_named(
            repo,
            &name,
            head_shorthand.as_deref() == Some(name.as_str()),
        )?);
    }
    out.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    Ok(out)
}

fn branch_info(repo: &git2::Repository, name: crate::domain::BranchName) -> Result<BranchInfo> {
    let is_head = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(str::to_owned).ok())
        .map(|s| s == name.as_str())
        .unwrap_or(false);
    branch_info_named(repo, name.as_str(), is_head)
}

fn branch_info_named(
    repo: &git2::Repository,
    short_name: &str,
    is_head: bool,
) -> Result<BranchInfo> {
    let branch = repo.find_branch(short_name, BranchType::Local)?;
    let target_oid = branch.get().peel_to_commit()?.id();

    let upstream = match branch.upstream() {
        Ok(upstream) => build_upstream(repo, &upstream, target_oid).map(Some),
        Err(_) => Ok(None),
    }?;

    Ok(BranchInfo {
        name: crate::domain::BranchName::parse(short_name, "branch")?,
        target: ObjectId::from_bytes(target_oid.as_bytes())?,
        is_head,
        upstream,
    })
}

fn build_upstream(
    repo: &git2::Repository,
    upstream: &git2::Branch<'_>,
    local_target: git2::Oid,
) -> Result<UpstreamRef> {
    let short_upstream_name = upstream.name()?.unwrap_or("").to_owned();
    let full_upstream_name = upstream.get().name().map(str::to_owned).ok();
    let remote_and_branch: Option<(String, String)> = full_upstream_name
        .as_deref()
        .and_then(|full| full.strip_prefix("refs/remotes/"))
        .and_then(|rest| rest.split_once('/'))
        .map(|(remote, branch)| (remote.to_owned(), branch.to_owned()));
    let (remote, branch_name) = remote_and_branch.unwrap_or((String::new(), short_upstream_name));
    let (ahead, behind, target) = match upstream.get().peel_to_commit() {
        Ok(commit) => {
            let (ahead, behind) = repo
                .graph_ahead_behind(local_target, commit.id())
                .unwrap_or((0, 0));
            let target = ObjectId::from_bytes(commit.id().as_bytes()).ok();
            (ahead, behind, target)
        }
        Err(_) => (0, 0, None),
    };
    Ok(UpstreamRef {
        remote: crate::domain::RemoteName::parse(&remote, "remote")?,
        branch: crate::domain::BranchName::parse(&branch_name, "branch")?,
        ahead: ahead as u32,
        behind: behind as u32,
        target,
    })
}

pub fn list_tags(repo: &git2::Repository) -> Result<Vec<TagInfo>> {
    let references = repo.references_glob("refs/tags/*")?;
    let mut tags = Vec::new();
    for reference in references {
        let reference = reference?;
        let Ok(full_name) = reference.name() else {
            continue;
        };
        let Some(short) = crate::domain::RefName::short_from_full(full_name) else {
            continue;
        };
        let name = crate::domain::TagName::parse(short, "tag")?;
        let resolved = reference.resolve()?;
        let raw_target = resolved.target();

        let tag_object = raw_target.ok_or_else(|| GitError::internal("tag ref has no target"))?;
        let peeled = resolved.peel_to_commit().ok().map(|c| c.id());

        let annotated = resolved.peel_to_tag().ok();
        let message = annotated
            .as_ref()
            .and_then(|t| t.message().ok().flatten())
            .map(str::to_owned);
        let tagger = annotated
            .as_ref()
            .and_then(|t| t.tagger())
            .map(domain_signature_from_git);

        tags.push(TagInfo {
            name,
            target: ObjectId::from_bytes(peeled.unwrap_or(tag_object).as_bytes())?,
            tag_object: Some(ObjectId::from_bytes(tag_object.as_bytes())?),
            message,
            tagger,
        });
    }
    tags.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    Ok(tags)
}

pub fn create_tag(repo: &git2::Repository, request: &TagCreateRequest) -> Result<TagInfo> {
    let name = crate::domain::TagName::parse(&request.name, "tag")?;
    let spec = request.target.clone().unwrap_or_default();
    let commit = crate::streaming::pipeline::resolve_commit(repo, &spec)?;
    match &request.message {
        Some(message) => {
            let sig = resolve_signature(repo)?;
            repo.tag(
                name.as_str(),
                commit.as_object(),
                &sig,
                message,
                request.force,
            )?;
        }
        None => {
            repo.tag_lightweight(name.as_str(), commit.as_object(), request.force)?;
        }
    }
    list_tags(repo)?
        .into_iter()
        .find(|tag| tag.name == name)
        .ok_or_else(|| GitError::internal("tag vanished after creation"))
}

pub fn delete_tag(repo: &git2::Repository, name: &crate::domain::TagName) -> Result<()> {
    repo.tag_delete(name.as_str())?;
    Ok(())
}

pub(crate) fn head_commit<'repo>(
    repo: &'repo git2::Repository,
) -> Result<Option<git2::Commit<'repo>>> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_commit()?)),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn summarize_commit(repo: &git2::Repository, oid: git2::Oid) -> Result<CommitSummary> {
    let commit = repo.find_commit(oid)?;
    summarize_committed(repo, &commit, None)
}

/// Builds a summary from an already-loaded commit. `tags` supplies the
/// prebuilt target -> names map used by history pages; `None` resolves tags
/// for this single commit (fine for one-off summaries, wasteful in loops).
pub(crate) fn summarize_committed(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    tags: Option<&std::collections::HashMap<git2::Oid, Vec<String>>>,
) -> Result<CommitSummary> {
    let names = match tags {
        Some(map) => map.get(&commit.id()).cloned().unwrap_or_default(),
        None => crate::engines::git2::history::commit_tags(repo, commit.id())?,
    };
    summarize_with_tag_names(repo, commit, names)
}

/// Same summary from pre-resolved tag names (cache-friendly hot path).
pub(crate) fn summarize_with_tag_names(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    tag_names: Vec<String>,
) -> Result<CommitSummary> {
    let oid = commit.id();
    let message_raw = commit.message_bytes();
    let full = String::from_utf8_lossy(message_raw).into_owned();
    let (files_changed, additions, deletions) =
        crate::engines::git2::history::commit_diff_stats(repo, commit)?;
    Ok(CommitSummary {
        id: ObjectId::from_bytes(oid.as_bytes())?,
        tree_id: ObjectId::from_bytes(commit.tree_id().as_bytes())?,
        parent_ids: commit
            .parent_ids()
            .map(|pid| ObjectId::from_bytes(pid.as_bytes()))
            .collect::<Result<_>>()?,
        summary_line: full.lines().next().unwrap_or("").to_owned(),
        message: full.trim_end_matches('\n').to_owned(),
        author: domain_signature_from_git(commit.author()),
        committer: domain_signature_from_git(commit.committer()),
        files_changed,
        additions,
        deletions,
        tags: tag_names,
        match_ranges: None,
    })
}

pub(crate) fn domain_signature_from_git(sig: git2::Signature<'_>) -> DomainSignature {
    DomainSignature {
        name: sig.name().unwrap_or("").to_owned(),
        email: sig.email().unwrap_or("").to_owned(),
        time_seconds: sig.when().seconds(),
        time_offset_minutes: sig.when().offset_minutes(),
    }
}

fn signature_from_domain(sig: &DomainSignature) -> Result<Signature<'static>> {
    Ok(git2::Signature::now(&sig.name, &sig.email)?)
}

pub(crate) fn validate_paths(paths: &[String]) -> Result<Vec<RelativePath>> {
    paths.iter().map(|p| RelativePath::parse(p)).collect()
}

fn hook_failure_details(result: &crate::api::hooks::HookRunResult) -> String {
    let mut parts = vec![format!("hook `{}` failed", result.hook)];
    if let Some(code) = result.exit_code {
        parts.push(format!("exit {code}"));
    }
    for stream in [result.stdout.trim(), result.stderr.trim()] {
        if !stream.is_empty() {
            parts.push(stream.to_owned());
        }
    }
    parts.join(": ")
}
