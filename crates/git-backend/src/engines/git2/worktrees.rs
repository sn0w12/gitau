use std::path::{Path, PathBuf};

use git2::{BranchType, WorktreeLockStatus};

use crate::api::worktrees::{WorktreeCreateRequest, WorktreeInfo};
use crate::domain::BranchName;
use crate::error::{GitError, Result};

const PRIMARY_TREE_NAME: &str = "main";

pub fn list_worktrees(repo: &git2::Repository) -> Result<Vec<WorktreeInfo>> {
    // The primary entry always describes the repository's main worktree, never
    // whichever tree this session happens to be bound to. A session opened on a
    // linked worktree shares the common git dir, whose parent is the main
    // worktree; reporting the session's own tree here would label that linked
    // tree twice and hide where the repository actually lives.
    let session_root = canonical_or_self(repo.workdir().map(Path::to_path_buf));
    let main_root = match &session_root {
        Some(_) => repo.commondir().parent().map(Path::to_path_buf),
        None => Some(repo.commondir().to_path_buf()),
    }
    .and_then(|path| std::fs::canonicalize(&path).ok().or(Some(path)))
    .or(session_root.clone());

    let mut out = Vec::new();

    if let Some(path) = main_root.clone() {
        let is_current = session_root.as_deref() == Some(path.as_path());
        let branch = if is_current {
            head_branch_shorthand(repo)
        } else {
            git2::Repository::open(&path)
                .ok()
                .and_then(|main| head_branch_shorthand(&main))
        };
        out.push(WorktreeInfo {
            name: PRIMARY_TREE_NAME.to_owned(),
            path: path.to_string_lossy().into_owned(),
            branch,
            is_current,
            is_primary: true,
            locked_by: None,
            is_prunable: !path.exists(),
        });
    }

    let listed = repo.worktrees()?;
    let mut names: Vec<String> = Vec::new();
    for name in &listed {
        if let Some(name) = name? {
            names.push(name.to_owned());
        }
    }
    names.sort();
    for name in names {
        let Some(info) = describe_linked(repo, &name)? else {
            // The admin dir vanished between listing and reading; skip.
            continue;
        };
        out.push(info);
    }
    Ok(out)
}

pub fn create_worktree(
    repo: &git2::Repository,
    request: &WorktreeCreateRequest,
) -> Result<WorktreeInfo> {
    let name = BranchName::parse(&request.name, "worktree")?;
    if repo.find_worktree(name.as_str()).is_ok() {
        return Err(GitError::invalid_input(format!(
            "worktree `{}` already exists",
            name.as_str()
        )));
    }
    let path = match request.path.as_deref().map(str::trim) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => {
            let base = repo.workdir().ok_or_else(|| {
                GitError::invalid_input("bare repositories cannot host linked worktrees")
            })?;
            base.join(name.as_str())
        }
    };
    if path.exists() {
        return Err(GitError::invalid_input(format!(
            "path already exists: {}",
            path.display()
        )));
    }

    let mut opts = git2::WorktreeAddOptions::new();
    opts.checkout_existing(true);

    let reference = match branch_reference(repo, request) {
        Some(reference) => reference,
        None => {
            return Err(GitError::invalid_input(
                "no start point for the new worktree",
            ));
        }
    };
    opts.reference(Some(&reference));

    let wt = repo.worktree(name.as_str(), &path, Some(&opts))?;
    super::local::clear_all_repository_handles(&path);

    describe_linked(repo, wt.name()?.unwrap_or(name.as_str()))?
        .ok_or_else(|| GitError::internal("created worktree disappeared"))
}

pub fn remove_worktree(repo: &git2::Repository, name: &str, force: bool) -> Result<()> {
    let tree_name = BranchName::parse(name, "worktree")?;
    let wt = repo.find_worktree(tree_name.as_str())?;
    let path = wt.path().to_path_buf();

    if path.exists() && !force {
        return Err(GitError::invalid_input(format!(
            "working tree {} still exists; pass force to delete it",
            path.display()
        )));
    }

    let mut prune = git2::WorktreePruneOptions::new();
    // `valid(true)` tells libgit2 to prune even though the tree still
    // validates (the caller decides with `force` whether the checkout dir
    // may go). Without it prune refuses on a present working tree.
    prune.valid(true);
    if force {
        prune.locked(true);
        prune.working_tree(true);
    }
    wt.prune(Some(&mut prune))?;

    // libgit2's prune with `working_tree` removes the directory, but leave
    // a fallback so a partially-removed checkout never lingers.
    if force && path.exists() {
        super::local::clear_all_repository_handles(&path);
        std::fs::remove_dir_all(&path).map_err(GitError::Io)?;
    }
    Ok(())
}

pub fn lock_worktree(repo: &git2::Repository, name: &str, reason: &str) -> Result<()> {
    let tree_name = BranchName::parse(name, "worktree")?;
    let wt = repo.find_worktree(tree_name.as_str())?;
    wt.lock(if reason.is_empty() {
        None
    } else {
        Some(reason)
    })?;
    Ok(())
}

pub fn unlock_worktree(repo: &git2::Repository, name: &str) -> Result<()> {
    let tree_name = BranchName::parse(name, "worktree")?;
    let wt = repo.find_worktree(tree_name.as_str())?;
    wt.unlock()?;
    Ok(())
}

/// Resolves the reference (local branch) the new tree should attach to.
/// A `start_point` naming an existing local branch attaches to it; a commit
/// spec creates a new local branch `name` at that commit so the tree has an
/// attached HEAD; with no start point the new branch starts at HEAD.
fn branch_reference<'repo>(
    repo: &'repo git2::Repository,
    request: &WorktreeCreateRequest,
) -> Option<git2::Reference<'repo>> {
    let name = BranchName::parse(&request.name, "worktree").ok()?;

    if let Some(spec) = request.start_point.as_deref() {
        if let Ok(branch) = repo.find_branch(spec, BranchType::Local) {
            return Some(branch.into_reference());
        }
        let loaded = crate::streaming::pipeline::resolve_commit(
            repo,
            &crate::domain::RevisionSpec::parse(spec).ok()?,
        )
        .ok()?;
        let branch = repo.branch(name.as_str(), &loaded, false).ok()?;
        return Some(branch.into_reference());
    }

    let head = head_commit_oid(repo)?;
    let branch = repo.branch(name.as_str(), &head, false).ok()?;
    Some(branch.into_reference())
}

fn head_commit_oid<'repo>(repo: &'repo git2::Repository) -> Option<git2::Commit<'repo>> {
    repo.head().ok().and_then(|head| head.peel_to_commit().ok())
}

fn describe_linked(repo: &git2::Repository, name: &str) -> Result<Option<WorktreeInfo>> {
    let wt = repo
        .find_worktree(name)
        .map_err(|e| GitError::internal(format!("worktree `{name}` unavailable: {e}")))?;
    let wt_path = canonical_or_self(Some(wt.path().to_path_buf()));
    let Some(wt_path) = wt_path else {
        return Ok(None);
    };
    let wt_repo = git2::Repository::open_from_worktree(&wt).ok();
    let branch = wt_repo.as_ref().and_then(head_branch_shorthand);
    let locked = match wt.is_locked()? {
        WorktreeLockStatus::Unlocked => None,
        WorktreeLockStatus::Locked(reason) => reason,
    };
    let main = repo.workdir().and_then(|p| std::fs::canonicalize(p).ok());
    let is_current = main.as_ref() == Some(&wt_path);
    Ok(Some(WorktreeInfo {
        name: name.to_owned(),
        path: wt_path.to_string_lossy().into_owned(),
        branch,
        is_current,
        is_primary: false,
        locked_by: locked,
        is_prunable: !wt_path.exists(),
    }))
}

fn head_branch_shorthand(repo: &git2::Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(str::to_owned).ok())
}

fn canonical_or_self(path: Option<PathBuf>) -> Option<PathBuf> {
    path.map(|p| std::fs::canonicalize(&p).unwrap_or(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    fn repo() -> crate::engines::git2::Git2Session {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("T", "t@e").unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "init",
            &repo.find_tree(index.write_tree().unwrap()).unwrap(),
            &[],
        )
        .unwrap();
        drop(repo);
        // Keep the tempdir alive by forgetting it, mirroring TestRepo.
        std::mem::forget(dir);
        crate::engines::git2::Git2Session::new(root)
    }

    #[test]
    fn primary_entry_is_the_main_worktree_from_a_linked_session() -> Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        std::fs::write(root.join("a.txt"), "a").unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree = index.write_tree().unwrap();
        let sig = git2::Signature::now("T", "t@e").unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "init",
            &repo.find_tree(tree).unwrap(),
            &[],
        )
        .unwrap();
        let linked = root.join("linked");
        repo.worktree("linked", &linked, None).unwrap();
        drop(repo);
        std::mem::forget(dir);

        // A session bound to the linked tree must still report the main
        // worktree as primary, and must not claim it is the current tree.
        let linked_repo = git2::Repository::open_from_worktree(
            &git2::Repository::open(&root)
                .unwrap()
                .find_worktree("linked")
                .unwrap(),
        )
        .unwrap();
        let list = list_worktrees(&linked_repo)?;
        let primary = list
            .iter()
            .find(|info| info.is_primary)
            .expect("primary entry");

        assert_eq!(
            std::fs::canonicalize(&primary.path).unwrap(),
            std::fs::canonicalize(&root).unwrap()
        );
        assert!(!primary.is_current);

        let linked_entry = list
            .iter()
            .find(|info| info.name == "linked")
            .expect("linked entry");
        assert!(linked_entry.is_current);
        assert!(!linked_entry.is_primary);
        assert_ne!(primary.name, linked_entry.name);
        Ok(())
    }

    #[test]
    fn lists_primary_tree() -> Result<()> {
        let session = repo();
        let repo = session.repository()?;
        let list = list_worktrees(&repo)?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, PRIMARY_TREE_NAME);
        assert!(list[0].is_current);
        Ok(())
    }
}
