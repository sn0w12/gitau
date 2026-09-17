use git_backend::error::{GitError, SerializedError};

pub type CommandResult<T> = Result<T, SerializedError>;

pub use launch::spawn_detached;

pub fn to_serialized(err: GitError) -> SerializedError {
    SerializedError::from(&err)
}

pub fn to_session_serialized(err: crate::session::SessionError) -> SerializedError {
    SerializedError {
        code: err.code(),
        message: error_message(&err),
        retryable: false,
        detail: None,
    }
}

fn error_message(err: &crate::session::SessionError) -> String {
    err.to_string()
}

pub mod changes;
pub mod editor;
pub mod file_manager;
pub mod github;
pub mod hooks;
pub mod icons;
pub mod launch;
pub mod lfs;
pub mod mutations;
pub mod queries;
pub mod remote_info;
pub mod remotes;
pub mod repository;
pub mod session;
pub mod settings;
pub mod submodules;
pub mod workflows;
pub mod worktrees;
