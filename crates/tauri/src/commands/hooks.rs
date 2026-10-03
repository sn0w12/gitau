use tauri::ipc::Channel;

use crate::commands::repository::channel_send;
use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};

#[tauri::command]
pub async fn git_list_commit_hooks(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<git_backend::api::hooks::HookInfo>> {
    state
        .backend
        .list_commit_hooks(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

/// Runs a hook and streams its output: each line reaches `on_event` as the
/// script writes it, and the command resolves with the terminal result once
/// every line has been delivered.
#[tauri::command]
pub async fn git_run_commit_hook_streamed(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
    on_event: Channel<git_backend::api::hooks::HookOutputChunk>,
) -> CommandResult<git_backend::api::hooks::HookRunResult> {
    let (sender, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let forward = tauri::async_runtime::spawn(async move {
        while let Some(chunk) = rx.recv().await {
            if channel_send(&on_event, chunk).is_err() {
                break;
            }
        }
    });

    let result = state
        .backend
        .run_commit_hook_streamed(to_repo_id(repo_id), hook, sender)
        .await;
    // Draining the forwarder first means the last line has landed before the
    // frontend sees the result, so nothing arrives out of order.
    let _ = forward.await;
    result.map_err(to_serialized)
}

#[tauri::command]
pub async fn git_read_commit_hook(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
) -> CommandResult<git_backend::api::hooks::HookContent> {
    state
        .backend
        .read_commit_hook(to_repo_id(repo_id), hook)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_write_commit_hook(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
    content: String,
) -> CommandResult<()> {
    state
        .backend
        .write_commit_hook(to_repo_id(repo_id), hook, content)
        .await
        .map_err(to_serialized)
}
