use serde::{Deserialize, Serialize};

/// What a provider can do. The UI reads this to show only the views,
/// actions, and nav entries the connected provider supports, so a provider
/// that lacks CI still renders without dead controls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Capabilities {
    /// Read issue lists and detail.
    pub issues: bool,
    /// Read pull request lists, detail, reviews, and commits.
    pub pull_requests: bool,
    /// Submit reviews and review comments.
    pub reviews: bool,
    /// Read the notification inbox.
    pub notifications: bool,
    /// Read CI check runs and their logs.
    pub checks: bool,
    /// Read legacy commit statuses alongside check runs.
    pub commit_statuses: bool,
    /// Read workflow runs for a commit.
    pub workflow_runs: bool,
    /// Create and update issues, comments, labels, and assignees.
    pub issues_write: bool,
    /// Update and merge pull requests.
    pub pull_requests_write: bool,
    /// Create a repository on the provider and push to it.
    pub publish: bool,
    /// List the signed-in user's organizations for the publish owner picker.
    pub orgs: bool,
}

impl Capabilities {
    /// Every capability set, the shape a full-featured provider such as
    /// GitHub advertises.
    pub const fn all() -> Self {
        Self {
            issues: true,
            pull_requests: true,
            reviews: true,
            notifications: true,
            checks: true,
            commit_statuses: true,
            workflow_runs: true,
            issues_write: true,
            pull_requests_write: true,
            publish: true,
            orgs: true,
        }
    }
}
