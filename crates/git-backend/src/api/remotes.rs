use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CredentialKind {
    /// Ask libgit2 to use its default credential sources (agent, wincred, ...).
    #[default]
    Default,
    UsernamePassword {
        username: String,
        password: String,
    },
    SshKey {
        username: String,
        key_path: String,
        passphrase: Option<String>,
    },
    InMemorySshKey {
        username: String,
        key_pem: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRequest {
    pub kind: CredentialKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct RemoteAddRequest {
    pub name: String,
    pub url: String,
    pub fetch_refspec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FetchRequest {
    pub remote: String,
    /// Defaults to the remote's configured refspecs.
    pub refspecs: Vec<String>,
    pub prune: bool,
    pub depth: Option<u32>,
    pub credential: Option<CredentialRequest>,
}

impl Default for FetchRequest {
    fn default() -> Self {
        Self {
            remote: "origin".into(),
            refspecs: vec![],
            prune: false,
            depth: None,
            credential: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PushRequest {
    pub remote: String,
    /// e.g. `refs/heads/main:refs/heads/main` or just `main`.
    pub refspecs: Vec<String>,
    pub force: bool,
    pub set_upstream: bool,
    pub credential: Option<CredentialRequest>,
}

impl Default for PushRequest {
    fn default() -> Self {
        Self {
            remote: "origin".into(),
            refspecs: vec![],
            force: false,
            set_upstream: false,
            credential: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PullRequest {
    pub remote: String,
    pub branch: Option<String>,
    pub fast_forward_only: bool,
    pub rebase: bool,
    pub credential: Option<CredentialRequest>,
}

impl Default for PullRequest {
    fn default() -> Self {
        Self {
            remote: "origin".into(),
            branch: None,
            fast_forward_only: false,
            rebase: false,
            credential: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct CloneRequest {
    pub url: String,
    pub destination: String,
    pub bare: bool,
    pub depth: Option<u32>,
    pub credential: Option<CredentialRequest>,
}

/// Fetch/checkout stages reported while a clone runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClonePhase {
    /// Negotiating with the remote; object totals are not known yet.
    Counting,
    /// Downloading the packfile.
    Receiving,
    /// Applying the received pack into the object database.
    Resolving,
    /// Materializing the working tree.
    CheckingOut,
}

/// One progress sample from an in-flight clone. `progress` is 0..1 and
/// intentionally 0 where the phase is indeterminate (counting, checkout).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    pub phase: ClonePhase,
    pub progress: f32,
    pub objects_received: u32,
    pub objects_total: Option<u32>,
    pub received_bytes: u64,
}

impl Default for CloneProgress {
    fn default() -> Self {
        Self {
            phase: ClonePhase::Counting,
            progress: 0.0,
            objects_received: 0,
            objects_total: None,
            received_bytes: 0,
        }
    }
}

/// Terminal states mirror DiffEvent so the frontend can drive one lifecycle
/// model for every streamed operation; `completed` carries the canonical
/// repository path to open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CloneEvent {
    Progress {
        operation_id: u64,
        #[serde(flatten)]
        progress: CloneProgress,
    },
    Completed {
        operation_id: u64,
        repo_path: String,
    },
    Failed {
        operation_id: u64,
        code: String,
        message: String,
    },
    Cancelled {
        operation_id: u64,
    },
}

/// Result payload describing what a push changed per refspec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushOutcome {
    pub reference: String,
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct RemoteInfo {
    pub name: String,
    pub url: Option<String>,
    pub push_url: Option<String>,
}

#[cfg(test)]
mod clone_event_tests {
    use super::*;

    /// The frontend's protocol twin reads camelCase keys; a snake_case field
    /// on the wire deserializes as `undefined` on the JS side and silently
    /// drops the payload (this shipped once: repo_path arrived as
    /// `repo_path` and the completed clone never opened).
    #[test]
    fn clone_event_wire_keys_are_camel_case() {
        let completed = serde_json::to_value(CloneEvent::Completed {
            operation_id: 7,
            repo_path: "C:/repos/hello".into(),
        })
        .unwrap();
        assert_eq!(
            completed,
            serde_json::json!({
                "event": "completed",
                "operationId": 7,
                "repoPath": "C:/repos/hello",
            })
        );

        let progress = serde_json::to_value(CloneEvent::Progress {
            operation_id: 7,
            progress: CloneProgress {
                phase: ClonePhase::Receiving,
                progress: 0.5,
                objects_received: 10,
                objects_total: Some(20),
                received_bytes: 2048,
            },
        })
        .unwrap();
        assert_eq!(
            progress,
            serde_json::json!({
                "event": "progress",
                "operationId": 7,
                "phase": "receiving",
                "progress": 0.5,
                "objectsReceived": 10,
                "objectsTotal": 20,
                "receivedBytes": 2048,
            })
        );

        let failed = serde_json::to_value(CloneEvent::Failed {
            operation_id: 7,
            code: "network".into(),
            message: "boom".into(),
        })
        .unwrap();
        assert_eq!(
            failed,
            serde_json::json!({
                "event": "failed",
                "operationId": 7,
                "code": "network",
                "message": "boom",
            })
        );

        let cancelled = serde_json::to_value(CloneEvent::Cancelled { operation_id: 7 }).unwrap();
        assert_eq!(
            cancelled,
            serde_json::json!({ "event": "cancelled", "operationId": 7 })
        );
    }
}
