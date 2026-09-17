use std::path::PathBuf;

use git2::build::CheckoutBuilder;
use git2::{SubmoduleIgnore, SubmoduleUpdateOptions};

use crate::api::submodules::{SubmoduleAddRequest, SubmoduleInfo, SubmoduleUpdateRequest};
use crate::domain::ObjectId;
use crate::error::{GitError, Result};

const RECURSE_DEPTH_LIMIT: usize = 8;

pub fn list_submodules(repo: &git2::Repository) -> Result<Vec<SubmoduleInfo>> {
    let mut out = Vec::new();
    for sm in repo.submodules()? {
        let name = sm.name()?.to_owned();
        let path = sm.path().to_string_lossy().into_owned();
        let url = sm.url().ok().flatten().map(str::to_owned);
        let branch = sm.branch().ok().flatten().map(str::to_owned);
        let status = repo.submodule_status(&name, SubmoduleIgnore::None)?;
        let initialized = sm.open().is_ok();
        let checked_out = initialized && status.contains(git2::SubmoduleStatus::IN_WD);
        let dirty = status.contains(git2::SubmoduleStatus::WD_INDEX_MODIFIED)
            || status.contains(git2::SubmoduleStatus::WD_WD_MODIFIED)
            || status.contains(git2::SubmoduleStatus::WD_UNTRACKED);
        // Prefer the live checkout's HEAD commit; fall back to the oid
        // recorded in the superproject index for an unopened submodule.
        let commit = observed_commit(&sm)
            .or_else(|| {
                sm.head_id()
                    .map(|oid| ObjectId::from_bytes(oid.as_bytes()).map(|oid| oid.hex()))
            })
            .transpose()?;
        out.push(SubmoduleInfo {
            name,
            path,
            url,
            branch,
            commit,
            initialized,
            checked_out,
            dirty,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn add_submodule(repo: &git2::Repository, request: &SubmoduleAddRequest) -> Result<()> {
    let rel = crate::domain::RelativePath::parse(&request.path)?;
    let path = rel.to_path_buf();
    if repo.submodules()?.iter().any(|sm| sm.path() == path) {
        return Err(GitError::invalid_input(format!(
            "submodule already exists at {request:?}"
        )));
    }

    // `repo.submodule` performs add_setup (stages .gitmodules + a gitlink
    // entry); `clone` then materializes the nested content. Calling `update`
    // here fails because the superproject index has no submodule oid yet.
    let mut sm = repo.submodule(&request.url, &path, true)?;
    let mut update = SubmoduleUpdateOptions::new();
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    update.checkout(checkout);
    sm.clone(Some(&mut update))?;

    if let Some(branch) = &request.branch {
        if let Ok(nested) = sm.open() {
            let _ = nested
                .find_branch(branch, git2::BranchType::Local)
                .map(|b| b.into_reference())
                .map(|reference| nested.set_head(reference.name().expect("branch ref")));
        }
    }

    sm.add_to_index(true)?;
    sm.add_finalize()?;
    Ok(())
}

/// After a clone or pull materializes new commits, initialize and update
/// submodules recursively and smudge LFS pointers whose objects are already
/// in the local object store. Best-effort: a nested fetch failure or missing
/// LFS object must not fail the parent operation.
pub fn integrate(repo: &git2::Repository) {
    let request = SubmoduleUpdateRequest {
        names: Vec::new(),
        recursive: true,
        init: true,
    };
    let _ = update_submodule(repo, &request);
    let _ = crate::engines::git2::lfs::lfs_smudge(repo);
}

pub fn update_submodule(repo: &git2::Repository, request: &SubmoduleUpdateRequest) -> Result<()> {
    if request.names.is_empty() {
        let names: Vec<String> = repo
            .submodules()?
            .into_iter()
            .filter_map(|sm| sm.name().ok().map(str::to_owned))
            .collect();
        update_named(repo, &names, request, 0)
    } else {
        update_named(repo, &request.names, request, 0)
    }
}

fn update_named(
    repo: &git2::Repository,
    names: &[String],
    request: &SubmoduleUpdateRequest,
    depth: usize,
) -> Result<()> {
    for name in names {
        let mut sm = repo.find_submodule(name)?;
        let mut update = SubmoduleUpdateOptions::new();
        let mut checkout = CheckoutBuilder::new();
        checkout.safe();
        update.checkout(checkout);
        update.allow_fetch(true);
        sm.update(request.init, Some(&mut update))?;

        if request.recursive && depth < RECURSE_DEPTH_LIMIT {
            if let Ok(nested) = sm.open() {
                let nested_names: Vec<String> = nested
                    .submodules()?
                    .into_iter()
                    .filter_map(|child| child.name().ok().map(str::to_owned))
                    .collect();
                if !nested_names.is_empty() {
                    update_named(&nested, &nested_names, request, depth + 1)?;
                }
            }
        }
    }
    Ok(())
}

fn observed_commit(sm: &git2::Submodule<'_>) -> Option<crate::error::Result<String>> {
    let repo = sm.open().ok()?;
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    Some(ObjectId::from_bytes(commit.id().as_bytes()).map(|oid| oid.hex()))
}

pub fn submodule_workdir(repo: &git2::Repository, name: &str) -> Result<Option<PathBuf>> {
    let sm = repo.find_submodule(name)?;
    Ok(repo.workdir().map(|wd| wd.join(sm.path())))
}
