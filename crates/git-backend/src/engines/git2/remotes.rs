use std::cell::Cell;
use std::path::Path;
use std::sync::{Arc, Mutex};

use git2::{Cred, CredentialType, FetchOptions, PushOptions, RemoteCallbacks, Repository};

use crate::api::remotes::{
    ClonePhase, CloneProgress, CloneRequest, CredentialKind, CredentialRequest, FetchRequest,
    PullRequest, PushOutcome, PushRequest, RemoteAddRequest, RemoteInfo,
};
use crate::error::{GitError, Result};

fn callbacks<'cb>(
    credential: Option<&CredentialRequest>,
    token: &crate::runtime::cancellation::CancellationToken,
) -> Result<RemoteCallbacks<'cb>> {
    let mut callbacks = RemoteCallbacks::new();
    let credential = credential.cloned();
    let token = token.clone();
    callbacks.credentials(move |_url, username_from_url, allowed_types| {
        if token.is_cancelled() {
            return Err(git2::Error::from_str("cancelled"));
        }
        match &credential {
            None
            | Some(CredentialRequest {
                kind: CredentialKind::Default,
            }) => {
                if allowed_types.contains(CredentialType::DEFAULT) {
                    return Cred::default();
                }
                if let Some(user) = username_from_url {
                    if allowed_types.contains(CredentialType::USERNAME) {
                        return Cred::username(user);
                    }
                }
                Err(git2::Error::from_str("no credentials available"))
            }
            Some(CredentialRequest {
                kind: CredentialKind::UsernamePassword { username, password },
            }) => Cred::userpass_plaintext(username, password),
            Some(CredentialRequest {
                kind:
                    CredentialKind::SshKey {
                        username,
                        key_path,
                        passphrase,
                    },
            }) => Cred::ssh_key(
                username.as_str(),
                None,
                Path::new(key_path),
                passphrase.as_deref(),
            ),
            Some(CredentialRequest {
                kind:
                    CredentialKind::InMemorySshKey {
                        username,
                        key_pem,
                        passphrase,
                    },
            }) => Cred::ssh_key_from_memory(
                username.as_str(),
                None,
                key_pem.as_str(),
                passphrase.as_deref(),
            ),
        }
    });
    Ok(callbacks)
}

pub fn list(repo: &Repository) -> Result<Vec<RemoteInfo>> {
    let names = repo.remotes()?;
    let mut out = Vec::with_capacity(names.len());
    for index in 0..names.len() {
        let Some(name) = names.get(index).ok().flatten() else {
            continue;
        };
        let remote = repo.find_remote(name)?;
        out.push(RemoteInfo {
            name: name.to_owned(),
            url: remote.url().ok().map(str::to_owned),
            push_url: remote.pushurl().ok().flatten().map(str::to_owned),
        });
    }
    Ok(out)
}

pub fn add(repo: &Repository, request: &RemoteAddRequest) -> Result<()> {
    let name = crate::domain::RemoteName::parse(&request.name, "remote")?;
    repo.remote_with_fetch(
        name.as_str(),
        &request.url,
        request
            .fetch_refspec
            .as_deref()
            .unwrap_or("+refs/heads/*:refs/remotes/{name}/*"),
    )?;
    Ok(())
}

pub fn remove(repo: &Repository, name: &str) -> Result<()> {
    repo.remote_delete(name)?;
    Ok(())
}

pub fn set_url(repo: &Repository, name: &str, url: &str) -> Result<()> {
    repo.remote_set_url(name, url)?;
    Ok(())
}

pub fn fetch(
    repo: &mut Repository,
    request: &FetchRequest,
    token: &crate::runtime::cancellation::CancellationToken,
) -> Result<()> {
    if request.depth.is_some() {
        return Err(GitError::Unsupported {
            capability: "shallow fetch".into(),
        });
    }
    let mut remote = repo
        .find_remote(&request.remote)
        .map_err(|_| GitError::invalid_input(format!("unknown remote `{}`", request.remote)))?;
    let callbacks = callbacks(request.credential.as_ref(), token)?;
    let mut options = FetchOptions::new();
    options.remote_callbacks(callbacks);
    options.prune(if request.prune {
        git2::FetchPrune::On
    } else {
        git2::FetchPrune::Unspecified
    });
    options.download_tags(git2::AutotagOption::All);
    let specs: Vec<String> = request.refspecs.clone();
    let spec_refs: Vec<&str> = specs.iter().map(String::as_str).collect();
    remote.fetch(
        if spec_refs.is_empty() {
            &[][..]
        } else {
            spec_refs.as_slice()
        },
        Some(&mut options),
        None,
    )?;
    Ok(())
}

pub fn push(
    repo: &mut Repository,
    request: &PushRequest,
    token: &crate::runtime::cancellation::CancellationToken,
) -> Result<Vec<PushOutcome>> {
    let mut remote = repo
        .find_remote(&request.remote)
        .map_err(|_| GitError::invalid_input(format!("unknown remote `{}`", request.remote)))?;

    let outcomes: Arc<Mutex<Vec<PushOutcome>>> = Arc::new(Mutex::new(Vec::new()));
    let mut callbacks = callbacks(request.credential.as_ref(), token)?;
    let cell = outcomes.clone();
    callbacks.push_update_reference(move |refname, status| {
        cell.lock().unwrap().push(PushOutcome {
            reference: refname.to_owned(),
            accepted: status.is_none(),
            reason: status.map(str::to_owned),
        });
        Ok(())
    });

    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);

    let mut specs = normalize_refspecs(repo, &request.refspecs, request.force)?;
    // Push tags alongside the branch like `git push --follow-tags`, so a tag
    // created locally reaches the remote in the same push that publishes its
    // branch commit. This is how release tags fire GitHub's push-to-tag
    // workflows.
    specs.extend(follow_tag_refspecs(repo, &specs, request.force)?);
    let spec_refs: Vec<&str> = specs.iter().map(String::as_str).collect();
    remote.push(spec_refs.as_slice(), Some(&mut options))?;
    drop(options);

    if request.set_upstream {
        for outcome in outcomes.lock().unwrap().iter().filter(|o| o.accepted) {
            configure_upstream(repo, &outcome.reference, &request.remote)?;
        }
    }

    let collected = outcomes.lock().unwrap().clone();
    if collected.is_empty() {
        return Err(GitError::internal("push produced no reference updates"));
    }
    if collected.iter().all(|o| !o.accepted) {
        let reason = collected
            .iter()
            .find_map(|o| o.reason.clone())
            .unwrap_or_else(|| "rejected".into());
        return Err(GitError::Conflict {
            details: format!("push rejected: {reason}"),
        });
    }
    Ok(collected)
}

/// Builds `refs/tags/<name>:refs/tags/<name>` updates for local tags whose
/// target commit is reachable from one of the pushed branch tips. Branches
/// whose refspec names a non-head source are skipped; annotated and
/// lightweight tags qualify alike. Tags already present on the remote become
/// no-ops (identical target) or rejected non-fast-forwards (moved target), so
/// only genuinely new tags change the remote.
fn follow_tag_refspecs(repo: &Repository, specs: &[String], force: bool) -> Result<Vec<String>> {
    let mut tips: Vec<git2::Oid> = Vec::new();
    for raw in specs {
        let src = raw.strip_prefix('+').unwrap_or(raw);
        let Some(src) = src.split(':').next() else {
            continue;
        };
        let Some(src) = src.strip_prefix("refs/heads/") else {
            continue;
        };
        let Ok(reference) = repo.find_branch(src, git2::BranchType::Local) else {
            continue;
        };
        if let Ok(commit) = reference.get().peel_to_commit() {
            tips.push(commit.id());
        }
    }
    if tips.is_empty() {
        return Ok(Vec::new());
    }

    let prefix = if force { "+" } else { "" };
    let mut out = Vec::new();
    for reference in repo.references_glob("refs/tags/*")? {
        let reference = reference?;
        let Ok(full) = reference.name() else {
            continue;
        };
        let Some(short) = crate::domain::RefName::short_from_full(full) else {
            continue;
        };
        let Ok(tag_commit) = reference.peel_to_commit() else {
            continue;
        };
        let tag_oid = tag_commit.id();
        // libgit2 does not count a commit as one of its own descendants, so a
        // tag on the push tip itself must be matched by equality.
        let reachable = tips
            .iter()
            .any(|tip| *tip == tag_oid || repo.graph_descendant_of(*tip, tag_oid).unwrap_or(false));
        if reachable {
            out.push(format!("{prefix}refs/tags/{short}:refs/tags/{short}"));
        }
    }
    Ok(out)
}

fn normalize_refspecs(repo: &Repository, refspecs: &[String], force: bool) -> Result<Vec<String>> {
    fn expand(side: &str) -> String {
        if side.starts_with("refs/") || side.is_empty() {
            side.to_owned()
        } else {
            format!("refs/heads/{side}")
        }
    }

    let mut out = Vec::with_capacity(refspecs.len());
    for raw in refspecs {
        let (force_prefix, spec) = match raw.strip_prefix('+') {
            Some(rest) => ("+", rest),
            None => (if force { "+" } else { "" }, raw.as_str()),
        };
        let expanded = if let Some((src, dst)) = spec.split_once(':') {
            format!("{}{}:{}", force_prefix, expand(src), expand(dst))
        } else {
            format!("{force_prefix}{}", expand(spec))
        };
        out.push(expanded);
    }
    if out.is_empty() {
        let short = repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(str::to_owned))
            .ok_or_else(|| GitError::Conflict {
                details: "nothing to push: HEAD is unborn".into(),
            })?;
        out.push(format!(
            "{}refs/heads/{short}:refs/heads/{short}",
            if force { "+" } else { "" }
        ));
    }
    Ok(out)
}

fn configure_upstream(repo: &Repository, refspec: &str, remote: &str) -> Result<()> {
    let local_ref = refspec.split(':').next_back().unwrap_or(refspec);
    let short = crate::domain::RefName::short_from_full(local_ref).unwrap_or(local_ref);
    let mut config = repo.config()?;
    config.set_str(&format!("branch.{short}.remote"), remote)?;
    config.set_str(
        &format!("branch.{short}.merge"),
        format!("refs/heads/{short}").as_str(),
    )?;
    Ok(())
}

pub struct PullPlan {
    pub upstream_spec: crate::domain::RevisionSpec,
}

pub fn resolve_pull_upstream(repo: &Repository, request: &PullRequest) -> Result<PullPlan> {
    let branch_name = match &request.branch {
        Some(branch) => branch.clone(),
        None => {
            let head = repo.head()?;
            head.shorthand()
                .ok()
                .map(str::to_owned)
                .ok_or_else(|| GitError::Conflict {
                    details: "detached HEAD has no tracking branch".into(),
                })?
        }
    };
    let upstream_full = format!("refs/remotes/{}/{branch_name}", request.remote);
    if repo.find_reference(&upstream_full).is_err() {
        return Err(GitError::ReferenceNotFound {
            name: upstream_full,
        });
    }
    Ok(PullPlan {
        upstream_spec: crate::domain::RevisionSpec::parse(&upstream_full)?,
    })
}

pub fn clone(
    request: &CloneRequest,
    token: &crate::runtime::cancellation::CancellationToken,
    on_progress: &mut dyn FnMut(CloneProgress),
) -> Result<std::path::PathBuf> {
    if request.depth.is_some() {
        return Err(GitError::Unsupported {
            capability: "shallow clone".into(),
        });
    }
    let destination = Path::new(&request.destination);
    let last_phase = Cell::new(None::<ClonePhase>);
    let last_bucket = Cell::new(u32::MAX);
    let last_sample = Cell::new(CloneProgress::default());

    // Scoped so the builder (which borrows the throttle cells and
    // on_progress through its callbacks) drops before the checkout event.
    let repo = {
        let mut builder = git2::build::RepoBuilder::new();
        builder.bare(request.bare);
        let mut fetch_options = FetchOptions::new();
        let mut callbacks = callbacks(request.credential.as_ref(), token)?;

        // libgit2 fires many updates per second; only forward phase changes
        // and whole-percent steps so the IPC channel stays quiet.
        callbacks.transfer_progress(|progress| {
            if token.is_cancelled() {
                return false;
            }
            if let Some(sample) = next_sample(&last_phase, &last_bucket, &last_sample, &progress) {
                on_progress(sample);
            }
            true
        });
        fetch_options.remote_callbacks(callbacks);
        builder.fetch_options(fetch_options);

        let token_for_cancel = token.clone();
        builder.clone(&request.url, destination).map_err(|e| {
            if token_for_cancel.is_cancelled() {
                GitError::Cancelled
            } else {
                GitError::Network {
                    message: e.message().to_owned(),
                }
            }
        })?
    };

    // Checkout runs inside RepoBuilder::clone between the final transfer tick
    // and here; report the phase once with the fetched totals so the UI never
    // ends on a sub-1.0 bar. The terminal CloneEvent follows from the caller.
    on_progress(CloneProgress {
        phase: ClonePhase::CheckingOut,
        progress: 1.0,
        ..last_sample.get()
    });
    Ok(repo
        .path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| destination.to_path_buf()))
}

/// Maps a libgit2 transfer tick into a [`CloneProgress`], returning `None`
/// when the update is too small to be worth forwarding.
fn next_sample(
    last_phase: &Cell<Option<ClonePhase>>,
    last_bucket: &Cell<u32>,
    last_sample: &Cell<CloneProgress>,
    progress: &git2::Progress<'_>,
) -> Option<CloneProgress> {
    let total = progress.total_objects();
    let sample = if total == 0 {
        CloneProgress {
            phase: ClonePhase::Counting,
            progress: 0.0,
            objects_received: progress.received_objects() as u32,
            objects_total: None,
            received_bytes: progress.received_bytes() as u64,
        }
    } else {
        let total_f = total as f32;
        let received = progress.received_objects();
        let (phase, ratio, current) = if received < total {
            (
                ClonePhase::Receiving,
                received as f32 / total_f,
                received as u32,
            )
        } else {
            // All bytes are in; indexing into the local object database is
            // what remains before checkout.
            let indexed = progress.indexed_objects().min(total);
            (
                ClonePhase::Resolving,
                indexed as f32 / total_f,
                indexed as u32,
            )
        };
        // Receiving owns 0.05..0.90 of the bar; resolving 0.90..0.99; the
        // checkout completion event carries 1.0.
        let value = if phase == ClonePhase::Receiving {
            0.05 + 0.85 * ratio
        } else {
            0.90 + 0.09 * ratio
        };
        CloneProgress {
            phase,
            progress: value,
            objects_received: current,
            objects_total: Some(total as u32),
            received_bytes: progress.received_bytes() as u64,
        }
    };

    let bucket = (sample.progress * 100.0).floor() as u32;
    let phase_changed = last_phase.get() != Some(sample.phase);
    if !phase_changed && bucket == last_bucket.get() {
        return None;
    }
    last_phase.set(Some(sample.phase));
    last_bucket.set(bucket);
    last_sample.set(sample);
    Some(sample)
}

#[cfg(test)]
mod follow_tag_refspec_tests {
    use super::*;

    fn tree_with_file(repo: &Repository, content: &str) -> git2::Oid {
        let blob = repo.blob(content.as_bytes()).unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder.insert("file.txt", blob, 0o100644).unwrap();
        builder.write().unwrap()
    }

    fn commit(repo: &Repository, parents: &[&git2::Commit<'_>], msg: &str) -> git2::Oid {
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree = repo.find_tree(tree_with_file(repo, msg)).unwrap();
        repo.commit(None, &sig, &sig, msg, &tree, parents).unwrap()
    }

    #[test]
    fn ancestor_tags_are_pushed_alongside_the_branch() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let c1 = commit(&repo, &[], "one");
        let c1 = repo.find_commit(c1).unwrap();
        repo.tag_lightweight("v1", c1.as_object(), false).unwrap();

        let c2 = commit(&repo, &[&c1], "two");
        let c2 = repo.find_commit(c2).unwrap();
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.tag("v2", c2.as_object(), &sig, "release two", false)
            .unwrap();

        // An unrelated root commit must never be pulled into the push.
        let other = commit(&repo, &[], "other root");
        let other = repo.find_commit(other).unwrap();
        repo.tag_lightweight("v-other", other.as_object(), false)
            .unwrap();

        repo.branch("main", &c2, true).unwrap();

        let follow = follow_tag_refspecs(
            &repo,
            &["refs/heads/main:refs/heads/main".to_string()],
            false,
        )
        .unwrap();
        assert_eq!(
            follow,
            vec!["refs/tags/v1:refs/tags/v1", "refs/tags/v2:refs/tags/v2"]
        );
    }

    #[test]
    fn force_prefixes_tag_refspecs_when_requested() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let c1 = commit(&repo, &[], "one");
        let c1 = repo.find_commit(c1).unwrap();
        repo.tag_lightweight("v1", c1.as_object(), false).unwrap();
        repo.branch("main", &c1, true).unwrap();

        let follow = follow_tag_refspecs(
            &repo,
            &["refs/heads/main:refs/heads/main".to_string()],
            true,
        )
        .unwrap();
        assert_eq!(follow, vec!["+refs/tags/v1:refs/tags/v1"]);
    }

    #[test]
    fn no_branch_refspec_means_no_tags() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let c1 = commit(&repo, &[], "one");
        let c1 = repo.find_commit(c1).unwrap();
        repo.tag_lightweight("v1", c1.as_object(), false).unwrap();

        let follow =
            follow_tag_refspecs(&repo, &["refs/tags/v1:refs/tags/v1".to_string()], false).unwrap();
        assert!(follow.is_empty());
    }
}

#[cfg(test)]
mod clone_progress_tests {
    use super::*;

    #[test]
    fn counting_reports_zero_progress_without_total() {
        let sample = CloneProgress {
            phase: ClonePhase::Counting,
            ..CloneProgress::default()
        };
        assert_eq!(sample.progress, 0.0);
        assert_eq!(sample.objects_total, None);
    }

    #[test]
    fn first_tick_always_emits_even_at_zero_percent() {
        // bucket 0 differs from the initial u32::MAX sentinel, so the very
        // first tick at 0% still reaches the caller.
        let last_bucket = Cell::new(u32::MAX);
        assert_ne!(0, last_bucket.get());
    }

    #[test]
    fn percent_bounds_hold() {
        const EPSILON: f32 = 1e-4;
        for ratio in [0.0f32, 0.5, 1.0] {
            let receiving = 0.05 + 0.85 * ratio;
            assert!(receiving >= 0.05 - EPSILON);
            assert!(receiving <= 0.90 + EPSILON);
            let resolving = 0.90 + 0.09 * ratio;
            assert!(resolving >= 0.90 - EPSILON);
            assert!(resolving <= 0.99 + EPSILON);
        }
    }
}
