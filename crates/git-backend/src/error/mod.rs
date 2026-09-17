use std::fmt;
use std::path::PathBuf;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitError {
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("not a git repository: {}", path.display())]
    NotARepository { path: PathBuf },
    #[error("repository not found: {}", path.display())]
    RepositoryNotFound { path: PathBuf },
    #[error("path escapes repository: {}", path.display())]
    PathOutsideRepository { path: PathBuf },
    #[error("invalid revision `{spec}`")]
    InvalidRevision { spec: String },
    #[error("ambiguous revision `{spec}`")]
    AmbiguousRevision { spec: String },
    #[error("object not found: {oid}")]
    ObjectNotFound { oid: String },
    #[error("reference not found: {name}")]
    ReferenceNotFound { name: String },
    #[error("permission denied: {}", path.display())]
    PermissionDenied { path: PathBuf },
    #[error("repository is locked: {details}")]
    RepositoryLocked { details: String },
    #[error("snapshot is stale: expected generation {expected}, current generation {current}")]
    StaleSnapshot { expected: u64, current: u64 },
    #[error("operation produced conflicts: {details}")]
    Conflict { details: String },
    #[error("authentication required for remote `{remote}`")]
    AuthenticationRequired { remote: String },
    #[error("network failure: {message}")]
    Network { message: String },
    #[error("github request failed ({status}): {message}")]
    GitHub { status: u16, message: String },
    #[error("operation cancelled")]
    Cancelled,
    #[error("could not move to trash: {details}")]
    Trash { details: String },
    #[error("not supported: {capability}")]
    Unsupported { capability: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{message}")]
    Internal { message: String },
}

impl GitError {
    pub fn invalid_input(reason: impl Into<String>) -> Self {
        GitError::InvalidInput {
            reason: reason.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        GitError::Internal {
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            GitError::InvalidInput { .. } => "invalidInput",
            GitError::NotARepository { .. } => "notARepository",
            GitError::RepositoryNotFound { .. } => "repositoryNotFound",
            GitError::PathOutsideRepository { .. } => "pathOutsideRepository",
            GitError::InvalidRevision { .. } => "invalidRevision",
            GitError::AmbiguousRevision { .. } => "ambiguousRevision",
            GitError::ObjectNotFound { .. } => "objectNotFound",
            GitError::ReferenceNotFound { .. } => "referenceNotFound",
            GitError::PermissionDenied { .. } => "permissionDenied",
            GitError::RepositoryLocked { .. } => "repositoryLocked",
            GitError::StaleSnapshot { .. } => "staleSnapshot",
            GitError::Conflict { .. } => "conflict",
            GitError::AuthenticationRequired { .. } => "authenticationRequired",
            GitError::Network { .. } => "network",
            GitError::GitHub { .. } => "github",
            GitError::Cancelled => "cancelled",
            GitError::Trash { .. } => "trashFailed",
            GitError::Unsupported { .. } => "unsupported",
            GitError::Io(_) => "io",
            GitError::Internal { .. } => "internal",
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            GitError::RepositoryLocked { .. } | GitError::Network { .. } => true,
            GitError::GitHub { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

impl From<git2::Error> for GitError {
    fn from(err: git2::Error) -> Self {
        use git2::ErrorCode as Code;

        let message = err.message().to_owned();
        match err.code() {
            Code::NotFound => GitError::ReferenceNotFound { name: message },
            Code::Exists => GitError::InvalidInput { reason: message },
            Code::Modified | Code::Locked | Code::Unmerged => {
                GitError::RepositoryLocked { details: message }
            }
            Code::Auth => GitError::AuthenticationRequired {
                remote: String::new(),
            },
            Code::Certificate => GitError::Network { message },
            Code::ApplyFail => GitError::Conflict { details: message },
            Code::Conflict => GitError::Conflict { details: message },
            Code::UnbornBranch => GitError::InvalidInput { reason: message },
            _ => GitError::Internal { message },
        }
    }
}

impl From<tokio::sync::AcquireError> for GitError {
    fn from(_: tokio::sync::AcquireError) -> Self {
        GitError::internal("scheduler closed")
    }
}

impl From<tokio::task::JoinError> for GitError {
    fn from(err: tokio::task::JoinError) -> Self {
        if err.is_cancelled() {
            GitError::Cancelled
        } else {
            GitError::internal(format!("worker panicked: {err}"))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SerializedError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

fn detail_of(err: &GitError) -> Option<String> {
    match err {
        GitError::InvalidInput { reason } => Some(reason.clone()),
        GitError::NotARepository { path } => Some(path.display().to_string()),
        GitError::RepositoryNotFound { path } => Some(path.display().to_string()),
        GitError::PathOutsideRepository { path } => Some(path.display().to_string()),
        GitError::InvalidRevision { spec } => Some(spec.clone()),
        GitError::AmbiguousRevision { spec } => Some(spec.clone()),
        GitError::ObjectNotFound { oid } => Some(oid.clone()),
        GitError::ReferenceNotFound { name } => Some(name.clone()),
        GitError::PermissionDenied { path } => Some(path.display().to_string()),
        GitError::RepositoryLocked { details } => Some(details.clone()),
        GitError::StaleSnapshot { expected, current } => Some(format!("{expected} != {current}")),
        GitError::Conflict { details } => Some(details.clone()),
        GitError::AuthenticationRequired { remote } => Some(remote.clone()),
        GitError::GitHub { message, .. } => Some(message.clone()),
        GitError::Trash { details } => Some(details.clone()),
        GitError::Unsupported { capability } => Some(capability.clone()),
        _ => None,
    }
}

impl From<&GitError> for SerializedError {
    fn from(err: &GitError) -> Self {
        SerializedError {
            code: err.code(),
            message: err.to_string(),
            retryable: err.retryable(),
            detail: detail_of(err),
        }
    }
}

impl Serialize for GitError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let payload = SerializedError::from(self);
        let mut state = serializer.serialize_struct("SerializedError", 3)?;
        state.serialize_field("code", &payload.code)?;
        state.serialize_field("message", &payload.message)?;
        state.serialize_field("retryable", &payload.retryable)?;
        if let Some(detail) = &payload.detail {
            state.serialize_field("detail", detail)?;
        }
        state.end()
    }
}

impl PartialEq for GitError {
    fn eq(&self, other: &Self) -> bool {
        self.code() == other.code()
    }
}

impl fmt::Display for SerializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_stable_codes() {
        let err = GitError::InvalidRevision {
            spec: "does-not-exist".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "invalidRevision");
        assert_eq!(json["detail"], "does-not-exist");
        assert!(!json["retryable"].as_bool().unwrap());
    }

    #[test]
    fn cancellation_is_not_retryable_but_distinct() {
        let err = GitError::Cancelled;
        assert_eq!(err.code(), "cancelled");
        assert!(!err.retryable());
    }

    #[test]
    fn maps_git2_lock_errors() {
        let mapped: GitError = git2::Error::from_str("index locked").into();
        assert_eq!(mapped.code(), "internal");
    }

    #[test]
    fn io_errors_convert() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: GitError = io_err.into();
        assert_eq!(err.code(), "io");
    }
}
