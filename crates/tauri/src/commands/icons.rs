use crate::commands::{CommandResult, SerializedError};
use crate::state::{to_repo_id, SharedState};
use git_backend::icons::{CachedIcon, IconError};
use std::path::Path;

/// Resolves (and caches) the owner/org avatar for a git remote URL.
/// Returns a data URL ready for `<img src>`.
#[tauri::command]
pub async fn icon_resolve(state: SharedState<'_>, remote_url: String) -> CommandResult<CachedIcon> {
    state.icons.resolve(&remote_url).await.map_err(|error| {
        log::warn!("icon resolve failed for `{remote_url}`: {error}");
        to_serialized(error)
    })
}

/// Forces a revalidation of one remote's icon right now, bypassing TTL.
#[tauri::command]
pub async fn icon_refresh(state: SharedState<'_>, remote_url: String) -> CommandResult<CachedIcon> {
    state
        .icons
        .force_refresh(&remote_url)
        .await
        .map_err(|error| {
            log::warn!("icon refresh failed for `{remote_url}`: {error}");
            to_serialized(error)
        })
}

/// Resolves the icon for an open repository. An icon file found in the
/// worktree (favicon/logo/icon names, gitignored paths excluded) wins;
/// otherwise the owner avatar of the origin remote is used. `None` means
/// the repo has no worktree icon and no remote to fall back to.
#[tauri::command]
pub async fn git_repo_icon(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Option<CachedIcon>> {
    let local = state
        .backend
        .worktree_icon(to_repo_id(repo_id))
        .await
        .map_err(crate::commands::to_serialized)?;
    if local.is_some() {
        return Ok(local);
    }

    let remotes = state
        .backend
        .list_remotes(to_repo_id(repo_id))
        .await
        .map_err(crate::commands::to_serialized)?;
    let Some(remote_url) = remotes
        .iter()
        .find(|remote| remote.name == "origin")
        .or_else(|| remotes.first())
        .and_then(|remote| remote.url.clone())
    else {
        return Ok(None);
    };

    state
        .icons
        .resolve(&remote_url)
        .await
        .map(Some)
        .map_err(|error| {
            log::warn!("repo icon fallback failed for `{remote_url}`: {error}");
            to_serialized(error)
        })
}

/// Resolves the icon for a repository addressed by path, mirroring
/// `git_repo_icon` but without requiring an open session.
#[tauri::command]
pub async fn git_repo_icon_by_path(
    state: SharedState<'_>,
    path: String,
) -> CommandResult<Option<CachedIcon>> {
    let repo_path = Path::new(&path);
    let local = state
        .backend
        .worktree_icon_for_path(repo_path)
        .await
        .map_err(crate::commands::to_serialized)?;
    if local.is_some() {
        return Ok(local);
    }

    let remotes = state
        .backend
        .list_remotes_for_path(repo_path)
        .await
        .map_err(crate::commands::to_serialized)?;
    let Some(remote_url) = remotes
        .iter()
        .find(|remote| remote.name == "origin")
        .or_else(|| remotes.first())
        .and_then(|remote| remote.url.clone())
    else {
        return Ok(None);
    };

    state
        .icons
        .resolve(&remote_url)
        .await
        .map(Some)
        .map_err(|error| {
            log::warn!("repo icon fallback failed for `{remote_url}`: {error}");
            to_serialized(error)
        })
}

fn to_serialized(error: IconError) -> SerializedError {
    SerializedError {
        code: error.code(),
        message: error.to_string(),
        retryable: matches!(error, IconError::Network { .. }),
        detail: None,
    }
}
