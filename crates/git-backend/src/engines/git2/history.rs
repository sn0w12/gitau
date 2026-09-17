use std::collections::HashMap;
use std::path::PathBuf;

use git2::{DiffOptions, Sort};
use rayon::prelude::*;
use std::sync::Arc;

use crate::api::history::HistoryPageQuery;
use crate::domain::history::HistoryPage;
use crate::domain::{CommitSummary, Generation, SnapshotId};
use crate::engines::git2::local;
use crate::engines::git2::mutations;
use crate::engines::git2::session::Git2Session;
use crate::error::{GitError, Result};

pub fn history_page(
    session: &Git2Session,
    query: &HistoryPageQuery,
    snapshot_id: SnapshotId,
    generation: Generation,
    cache: &mut crate::runtime::history_cache::HistoryCache,
) -> Result<HistoryPage> {
    if query.limit == 0 || query.limit > 10_000 {
        return Err(GitError::invalid_input("limit must be in 1..=10000"));
    }
    session.with_repository(|repo| {
        // An unborn HEAD (fresh `git init`, empty bare repo) has no
        // history; report an empty page instead of `invalid revision HEAD`.
        // Explicit revisions still resolve strictly below.
        let tip = if is_default_revision(query.revision.as_ref()) {
            match default_tip(repo)? {
                Some(tip) => tip,
                None => {
                    return Ok(HistoryPage {
                        snapshot_id,
                        generation,
                        commits: Vec::new(),
                        has_more: false,
                    });
                }
            }
        } else {
            resolve_tip(repo, query.revision.as_ref())?
        };
        let mut exclusions = Vec::with_capacity(query.exclude_reachable_from.len());
        for spec in &query.exclude_reachable_from {
            exclusions.push(oid_bytes(resolve_tip(repo, Some(spec))?));
        }

        // The OID list is immutable once walked; pages become slices instead
        // of re-walking and discarding `skip` commits every call.
        let walk_key = crate::runtime::history_cache::WalkKey {
            tip: Some(oid_bytes(tip)),
            exclusions,
        };
        let list = match cache.walk(&walk_key) {
            Some(list) => list,
            None => {
                let mut walk = repo.revwalk()?;
                walk.push(tip)?;
                // TOPOLOGICAL keeps children before parents when timestamps tie (fast
                // scripted histories often share one second).
                walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
                for spec in &query.exclude_reachable_from {
                    let hidden = resolve_tip(repo, Some(spec))?;
                    walk.hide(hidden)?;
                }
                let mut oids = Vec::new();
                for info in walk {
                    if oids.len() >= crate::runtime::history_cache::MAX_WALK_ENTRIES {
                        break;
                    }
                    oids.push(info.map_err(|e| GitError::Internal {
                        message: e.to_string(),
                    })?);
                }
                let list = Arc::new(oids);
                cache.store_walk(walk_key.clone(), list.clone());
                list
            }
        };

        let tags = match cache.tags() {
            Some(tags) => tags,
            None => {
                let raw = tag_map(repo)?;
                let mut converted: HashMap<[u8; 20], Vec<String>> =
                    HashMap::with_capacity(raw.len());
                for (oid, names) in raw {
                    converted.insert(oid_bytes(oid), names);
                }
                let tags = Arc::new(converted);
                cache.store_tags(tags.clone());
                tags
            }
        };

        let search = crate::engines::search::commit_search(query.search.as_deref());

        // Search filters the full cached walk before paginating, so `skip`
        // and `has_more` count only matching commits. Matching reads only
        // the cheap commit header (not diff stats), keeping the first search
        // cheap; later identical searches reuse cached summaries.
        let filtered: Option<Vec<git2::Oid>> = if search.is_empty() {
            None
        } else {
            let mut matches = Vec::new();
            for oid in list.iter() {
                if commit_matches_search(repo, *oid, &search)? {
                    matches.push(*oid);
                }
            }
            Some(matches)
        };
        let effective: &[git2::Oid] = match &filtered {
            Some(f) => f.as_slice(),
            None => list.as_slice(),
        };

        let start = query.skip as usize;
        let end = (start + query.limit as usize).min(effective.len());
        let has_more = effective.len() > end;
        let page_oids = &effective[start.min(effective.len())..end];

        let mut slots: Vec<Option<Arc<CommitSummary>>> =
            (0..page_oids.len()).map(|_| None).collect();
        let mut missing: Vec<(usize, git2::Oid)> = Vec::new();
        for (idx, oid) in page_oids.iter().enumerate() {
            match cache.summary(&oid_bytes(*oid)) {
                Some(summary) => slots[idx] = Some(summary),
                None => missing.push((idx, *oid)),
            }
        }

        let computed = summarize_page(repo, &missing, &tags)?;
        for (idx, arc) in computed {
            cache.store_summary(oid_bytes(page_oids[idx]), arc.clone());
            slots[idx] = Some(arc);
        }

        let mut commits = slots
            .into_iter()
            .map(|slot| -> Result<CommitSummary> {
                let arc = slot.ok_or_else(|| GitError::internal("summary slot unfilled"))?;
                Ok((*arc).clone())
            })
            .collect::<Result<Vec<_>>>()?;

        if !search.is_empty() {
            for commit in &mut commits {
                let ranges = search.ranges_for(
                    &commit.summary_line.to_lowercase(),
                    &commit.author.name.to_lowercase(),
                );
                commit.match_ranges = if ranges.is_empty() {
                    None
                } else {
                    Some(ranges)
                };
            }
        }

        Ok(HistoryPage {
            snapshot_id,
            generation,
            commits,
            has_more,
        })
    })
}

/// Per-commit stats against the first parent (root commits diff the empty
/// tree). Line counts come from libgit2's diff stats, so binary files count
/// toward `files_changed` but contribute no insertions/deletions.
pub(crate) fn commit_diff_stats(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
) -> Result<(u32, u64, u64)> {
    let parent_tree = commit.parent(0).ok().map(|p| p.tree()).transpose()?;
    let tree = commit.tree()?;
    let diff = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&tree),
        Some(&mut DiffOptions::new()),
    )?;
    let stats = diff.stats()?;
    Ok((
        stats.files_changed() as u32,
        stats.insertions() as u64,
        stats.deletions() as u64,
    ))
}

/// Tag names whose target (peeled for annotated tags) resolves to `oid`.
pub(crate) fn commit_tags(repo: &git2::Repository, oid: git2::Oid) -> Result<Vec<String>> {
    Ok(tag_map(repo)?.remove(&oid).unwrap_or_default())
}

/// Builds one map from peeled tag target -> tag names for the whole repo.
/// Cheaper than resolving per commit when walking a page.
fn tag_map(repo: &git2::Repository) -> Result<HashMap<git2::Oid, Vec<String>>> {
    let mut map: HashMap<git2::Oid, Vec<String>> = HashMap::new();
    let references = repo.references_glob("refs/tags/*")?;
    for reference in references {
        let reference = reference?;
        let name = reference.shorthand()?.to_owned();
        // Peel annotated tags down to their commit; skip tags pointing at
        // trees/blobs, which never belong to a commit's summary.
        let target = match reference.peel_to_commit() {
            Ok(commit) => commit.id(),
            Err(_) => continue,
        };
        map.entry(target).or_default().push(name);
    }
    for names in map.values_mut() {
        names.sort();
    }
    Ok(map)
}

/// Summarizes missing commits in parallel. libgit2 handles are not `Sync`, so
/// each worker thread uses its own cached handle; output pairs keep the input
/// index order.
fn summarize_page(
    repo: &git2::Repository,
    missing: &[(usize, git2::Oid)],
    tags: &HashMap<[u8; 20], Vec<String>>,
) -> Result<Vec<(usize, Arc<CommitSummary>)>> {
    let git_dir: PathBuf = repo.path().to_path_buf();
    let results: Vec<Result<(usize, Arc<CommitSummary>)>> = missing
        .par_iter()
        .map(|(idx, oid)| {
            local::with_repository(&git_dir, |wrepo| {
                let commit = wrepo.find_commit(*oid)?;
                let names = tags.get(oid.as_bytes()).cloned().unwrap_or_default();
                let summary = mutations::summarize_with_tag_names(wrepo, &commit, names)?;
                Ok((*idx, Arc::new(summary)))
            })
        })
        .collect();
    results.into_iter().collect()
}

pub(crate) fn oid_bytes(oid: git2::Oid) -> [u8; 20] {
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(oid.as_bytes());
    bytes
}

/// Matches a commit's message and author identity against the search query.
/// Reads only the commit header, never diff stats.
fn commit_matches_search(
    repo: &git2::Repository,
    oid: git2::Oid,
    search: &crate::engines::search::CommitSearch,
) -> Result<bool> {
    let commit = repo.find_commit(oid)?;
    let sig = commit.author();
    let name = sig.name().unwrap_or("").to_lowercase();
    let email = sig.email().unwrap_or("").to_lowercase();
    let message = commit.message().unwrap_or("").to_lowercase();
    Ok(search.matches(&[&name, &email, &message]))
}

/// True when the query walks the default tip: no revision, or HEAD itself.
pub(crate) fn is_default_revision(revision: Option<&crate::domain::RevisionSpec>) -> bool {
    revision.map(|spec| spec.as_str() == "HEAD").unwrap_or(true)
}

/// Resolves the default walk tip, or `None` when HEAD is unborn. Any other
/// resolution failure stays an `InvalidRevision`.
pub(crate) fn default_tip(repo: &git2::Repository) -> Result<Option<git2::Oid>> {
    match repo.revparse_single("HEAD") {
        Ok(object) => Ok(Some(object.peel_to_commit()?.id())),
        Err(_) if is_unborn_head(repo) => Ok(None),
        Err(_) => Err(GitError::InvalidRevision {
            spec: "HEAD".to_owned(),
        }),
    }
}

fn is_unborn_head(repo: &git2::Repository) -> bool {
    matches!(
        repo.head(),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch
    )
}

pub(crate) fn resolve_tip(
    repo: &git2::Repository,
    revision: Option<&crate::domain::RevisionSpec>,
) -> Result<git2::Oid> {
    let spec = revision.map(|r| r.as_str()).unwrap_or("HEAD");
    let reference = repo
        .revparse_single(spec)
        .map_err(|_| GitError::InvalidRevision {
            spec: spec.to_owned(),
        })?;
    Ok(reference.peel_to_commit()?.id())
}
