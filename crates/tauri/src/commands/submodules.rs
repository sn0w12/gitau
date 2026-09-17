use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::submodules::{SubmoduleAddRequest, SubmoduleInfo, SubmoduleUpdateRequest};
use git_backend::domain::Generation;

fn gen(value: Option<u64>) -> Option<Generation> {
    value.map(Generation)
}

#[tauri::command]
pub async fn git_list_submodules(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<SubmoduleInfo>> {
    state
        .backend
        .list_submodules(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_add_submodule(
    state: SharedState<'_>,
    repo_id: u64,
    url: String,
    path: String,
    branch: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .add_submodule(
            to_repo_id(repo_id),
            SubmoduleAddRequest { url, path, branch },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_update_submodules(
    state: SharedState<'_>,
    repo_id: u64,
    names: Option<Vec<String>>,
    recursive: Option<bool>,
    init: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .update_submodules(
            to_repo_id(repo_id),
            SubmoduleUpdateRequest {
                names: names.unwrap_or_default(),
                recursive: recursive.unwrap_or(false),
                init: init.unwrap_or(true),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}
