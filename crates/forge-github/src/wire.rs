use serde::{Deserialize, Serialize};

use forge_core::dto::{CreatePullRequestBody, MergePullRequestBody, PullRequestMergeMethod};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct CreatePullPayload {
    pub(crate) title: String,
    pub(crate) head: String,
    pub(crate) base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) body: Option<String>,
    pub(crate) draft: bool,
}

impl From<&CreatePullRequestBody> for CreatePullPayload {
    fn from(body: &CreatePullRequestBody) -> Self {
        Self {
            title: body.title.clone(),
            head: body.head.clone(),
            base: body.base.clone(),
            body: body.body.clone(),
            draft: body.draft,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MergePullPayload {
    pub(crate) merge_method: PullRequestMergeMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) commit_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) commit_message: Option<String>,
}

impl From<&MergePullRequestBody> for MergePullPayload {
    fn from(body: &MergePullRequestBody) -> Self {
        Self {
            merge_method: body.merge_method,
            commit_title: body.commit_title.clone(),
            commit_message: body.commit_message.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawUser {
    pub(crate) login: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) avatar_url: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawOrg {
    pub(crate) login: String,
    #[serde(default)]
    pub(crate) avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawRepo {
    pub(crate) full_name: String,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
    #[serde(default)]
    pub(crate) default_branch: Option<String>,
    #[serde(default)]
    pub(crate) clone_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawNotification {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) unread: bool,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) subject: Option<RawNotificationSubject>,
    #[serde(default)]
    pub(crate) repository: Option<RawNotificationRepo>,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawNotificationSubject {
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawNotificationRepo {
    #[serde(default)]
    pub(crate) full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSubject {
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawIssueUser {
    #[serde(default)]
    pub(crate) login: String,
    #[serde(default)]
    pub(crate) avatar_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawIssueLabel {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) color: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawIssue {
    #[serde(default)]
    pub(crate) number: u64,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) state_reason: Option<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) labels: Vec<RawIssueLabel>,
    #[serde(default)]
    pub(crate) assignees: Vec<RawIssueUser>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSearchIssueResponse {
    pub(crate) items: Vec<RawSearchIssue>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSearchIssue {
    #[serde(default)]
    pub(crate) number: u64,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) state_reason: Option<String>,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) labels: Vec<RawIssueLabel>,
    #[serde(default)]
    pub(crate) assignees: Vec<RawIssueUser>,
    #[serde(default)]
    pub(crate) comments: u64,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
    #[serde(default)]
    pub(crate) repository_url: Option<String>,
    #[serde(default)]
    pub(crate) pull_request: Option<RawSearchPullRef>,
}

/// Search items carry this block only for pull requests, and `merged_at` is
/// the sole signal that a `closed` pull request was in fact merged.
#[derive(Debug, Deserialize)]
pub(crate) struct RawSearchPullRef {
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawIssueComment {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

/// The repository payload carries far more than permissions; only the
/// access block is parsed.
#[derive(Debug, Deserialize)]
pub(crate) struct RawPermissionsEnvelope {
    #[serde(default)]
    pub(crate) permissions: Option<RawRepoPermissions>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct RawRepoPermissions {
    #[serde(default)]
    pub(crate) push: bool,
    #[serde(default)]
    pub(crate) admin: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawIssueEvent {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) event: String,
    #[serde(default)]
    pub(crate) actor: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) label: Option<RawIssueLabel>,
    #[serde(default)]
    pub(crate) assignee: Option<RawIssueUser>,
    /// Review requests name the user in this slot instead of `assignee`.
    #[serde(default)]
    pub(crate) requested_reviewer: Option<RawIssueUser>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPullRef {
    #[serde(default, rename = "ref")]
    pub(crate) git_ref: String,
    #[serde(default)]
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) label: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPull {
    #[serde(default)]
    pub(crate) number: u64,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) state_reason: Option<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) labels: Vec<RawIssueLabel>,
    #[serde(default)]
    pub(crate) assignees: Vec<RawIssueUser>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
    /// null while GitHub computes it, so the tri-state must survive.
    #[serde(default)]
    pub(crate) mergeable: Option<bool>,
    #[serde(default)]
    pub(crate) mergeable_state: Option<String>,
    #[serde(default)]
    pub(crate) head: Option<RawPullRef>,
    #[serde(default)]
    pub(crate) base: Option<RawPullRef>,
    #[serde(default)]
    pub(crate) additions: u64,
    #[serde(default)]
    pub(crate) deletions: u64,
    #[serde(default)]
    pub(crate) changed_files: u64,
    #[serde(default)]
    pub(crate) commits: u64,
}

/// The commits endpoint nests the message and dates under `commit`, with
/// `author`/`committer` holding a linked account only when the email matches
/// one, so the git-level author is what fills the gap.
#[derive(Debug, Deserialize)]
pub(crate) struct RawPullCommit {
    #[serde(default)]
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) commit: Option<RawCommitDetail>,
    #[serde(default)]
    pub(crate) author: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCommitDetail {
    #[serde(default)]
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) author: Option<RawCommitSignature>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCommitSignature {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPullReview {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) submitted_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPullReviewComment {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) user: Option<RawIssueUser>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) line: Option<u64>,
    #[serde(default)]
    pub(crate) diff_hunk: String,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
    #[serde(default)]
    pub(crate) in_reply_to_id: Option<u64>,
    pub(crate) pull_request_review_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawMergeResult {
    #[serde(default)]
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) merged: bool,
    #[serde(default)]
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCheckRun {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) conclusion: Option<String>,
    #[serde(default)]
    pub(crate) details_url: Option<String>,
    #[serde(default)]
    pub(crate) started_at: Option<String>,
    #[serde(default)]
    pub(crate) completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCheckRunOutput {
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) summary: Option<String>,
    #[serde(default)]
    pub(crate) text: Option<String>,
    #[serde(default)]
    pub(crate) annotations_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCheckAnnotation {
    #[serde(default)]
    pub(crate) path: Option<String>,
    #[serde(default)]
    pub(crate) start_line: Option<u64>,
    #[serde(default)]
    pub(crate) end_line: Option<u64>,
    #[serde(default)]
    pub(crate) start_column: Option<u64>,
    #[serde(default)]
    pub(crate) annotation_level: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawActionStep {
    #[serde(default)]
    pub(crate) number: u64,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) conclusion: Option<String>,
    #[serde(default)]
    pub(crate) started_at: Option<String>,
    #[serde(default)]
    pub(crate) completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawActionJob {
    #[serde(default)]
    pub(crate) steps: Vec<RawActionStep>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCheckRunDetail {
    #[serde(flatten)]
    pub(crate) run: RawCheckRun,
    #[serde(default)]
    pub(crate) output: Option<RawCheckRunOutput>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCheckRunsResponse {
    #[serde(default)]
    pub(crate) check_runs: Vec<RawCheckRun>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawWorkflowRun {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) event: String,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) conclusion: Option<String>,
    #[serde(default)]
    pub(crate) run_number: u64,
    #[serde(default)]
    pub(crate) head_branch: String,
    #[serde(default)]
    pub(crate) html_url: String,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawWorkflowRunsResponse {
    #[serde(default)]
    pub(crate) workflow_runs: Vec<RawWorkflowRun>,
}

/// The combined status endpoint wraps the newest status per context.
#[derive(Debug, Deserialize)]
pub(crate) struct RawCombinedStatus {
    #[serde(default)]
    pub(crate) statuses: Vec<RawCommitStatus>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawCommitStatus {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) context: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::dto::CreateRepositoryBody;

    #[test]
    fn create_body_serializes_snake_case_without_nulls() {
        let json = serde_json::to_value(CreateRepositoryBody {
            name: "repo".into(),
            description: None,
            private: true,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"name": "repo", "private": true}));
    }

    /// The IPC DTO is camelCase but GitHub's merge endpoint takes
    /// `merge_method`, so the wire form is built separately and must not
    /// inherit the IPC naming.
    #[test]
    fn merge_payload_uses_snake_case_for_the_github_wire() {
        let payload = MergePullPayload::from(&MergePullRequestBody {
            merge_method: PullRequestMergeMethod::Squash,
            commit_title: None,
            commit_message: None,
        });
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json, serde_json::json!({ "merge_method": "squash" }));

        let with_message = MergePullPayload::from(&MergePullRequestBody {
            merge_method: PullRequestMergeMethod::Rebase,
            commit_title: Some("subject".into()),
            commit_message: Some("body".into()),
        });
        assert_eq!(
            serde_json::to_value(with_message).unwrap(),
            serde_json::json!({
                "merge_method": "rebase",
                "commit_title": "subject",
                "commit_message": "body"
            })
        );
    }

    #[test]
    fn merge_body_defaults_to_a_merge_commit() {
        assert_eq!(
            MergePullRequestBody::default().merge_method,
            PullRequestMergeMethod::Merge
        );
    }
}
