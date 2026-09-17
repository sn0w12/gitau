use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::mutations::{
    BranchCreateRequest, CheckoutRequest, ResetKind, ResetRequest, TagCreateRequest,
};
use git_backend::domain::{BranchInfo, Generation, RevisionSpec, TagInfo};

fn spec(value: String) -> Result<RevisionSpec, crate::commands::SerializedError> {
    RevisionSpec::parse(&value).map_err(to_serialized)
}

fn gen(value: Option<u64>) -> Option<Generation> {
    value.map(Generation)
}

#[tauri::command]
pub async fn git_create_branch(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    start_point: Option<String>,
    force: Option<bool>,
    checkout: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<BranchInfo> {
    let start_point = match start_point {
        Some(point) => Some(spec(point)?),
        None => None,
    };
    state
        .backend
        .create_branch(
            to_repo_id(repo_id),
            BranchCreateRequest {
                name,
                start_point,
                force: force.unwrap_or(false),
                checkout: checkout.unwrap_or(false),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_delete_branch(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    force: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .delete_branch(
            to_repo_id(repo_id),
            name,
            force.unwrap_or(false),
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_rename_branch(
    state: SharedState<'_>,
    repo_id: u64,
    old_name: String,
    new_name: String,
    force: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<BranchInfo> {
    state
        .backend
        .rename_branch(
            to_repo_id(repo_id),
            old_name,
            new_name,
            force.unwrap_or(false),
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_create_tag(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    target: Option<String>,
    message: Option<String>,
    force: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<TagInfo> {
    let target = match target {
        Some(point) => Some(spec(point)?),
        None => None,
    };
    state
        .backend
        .create_tag(
            to_repo_id(repo_id),
            TagCreateRequest {
                name,
                target,
                message,
                force: force.unwrap_or(false),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_delete_tag(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .delete_tag(to_repo_id(repo_id), name, gen(expected_generation))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_checkout(
    state: SharedState<'_>,
    repo_id: u64,
    target: String,
    force: Option<bool>,
    paths: Option<Vec<String>>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .checkout(
            to_repo_id(repo_id),
            CheckoutRequest {
                target: spec(target)?,
                force: force.unwrap_or(false),
                paths: paths.unwrap_or_default(),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_reset(
    state: SharedState<'_>,
    repo_id: u64,
    kind: ResetKind,
    target: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .reset(
            to_repo_id(repo_id),
            ResetRequest {
                kind,
                target: spec(target)?,
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}
