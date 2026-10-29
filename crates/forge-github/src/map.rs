use crate::wire::*;

use forge_core::dto::{
    CheckAnnotation, CheckRun, CheckRunDetail, CheckRunOutput, CommitStatus, IssueComment,
    IssueDetail, IssueEvent, Label, Notification, PullRequestCommit, PullRequestDetail,
    PullRequestListItem, PullRequestRef, PullRequestReview, PullRequestReviewComment,
    RepositoryMetadata, SearchIssueItem, ThreadStateReason, User, WorkflowRun,
};

/// Maps GitHub's `state_reason` string. The REST API spells one value with a
/// space (`not planned`) and the enum documentation with an underscore, so
/// both are accepted.
pub(crate) fn parse_state_reason(raw: Option<&str>) -> ThreadStateReason {
    match raw.map(str::trim) {
        Some("completed") => ThreadStateReason::Completed,
        Some("not planned" | "not_planned") => ThreadStateReason::NotPlanned,
        Some("duplicate") => ThreadStateReason::Duplicate,
        _ => ThreadStateReason::None,
    }
}

pub(crate) fn map_check_run(raw: RawCheckRun) -> CheckRun {
    CheckRun {
        id: raw.id,
        name: raw.name,
        status: raw.status,
        conclusion: raw.conclusion,
        details_url: raw.details_url,
        started_at: raw.started_at.unwrap_or_default(),
        completed_at: raw.completed_at,
    }
}

pub(crate) fn map_check_annotation(raw: RawCheckAnnotation) -> CheckAnnotation {
    CheckAnnotation {
        path: raw.path.unwrap_or_default(),
        start_line: raw.start_line,
        end_line: raw.end_line,
        start_column: raw.start_column,
        annotation_level: raw.annotation_level.unwrap_or_default(),
        message: raw.message.unwrap_or_default(),
        title: raw.title.unwrap_or_default(),
    }
}

pub(crate) fn map_check_run_detail(
    raw: RawCheckRunDetail,
    annotations: Vec<RawCheckAnnotation>,
) -> CheckRunDetail {
    let output = raw.output.unwrap_or(RawCheckRunOutput {
        title: None,
        summary: None,
        text: None,
        annotations_count: None,
    });
    CheckRunDetail {
        run: map_check_run(raw.run),
        output: CheckRunOutput {
            title: output.title.unwrap_or_default(),
            summary: output.summary.unwrap_or_default(),
            text: output.text.unwrap_or_default(),
            annotations_count: output.annotations_count.unwrap_or_default(),
        },
        annotations: annotations.into_iter().map(map_check_annotation).collect(),
    }
}

pub(crate) fn map_workflow_run(raw: RawWorkflowRun) -> WorkflowRun {
    WorkflowRun {
        id: raw.id,
        name: raw.name,
        event: raw.event,
        status: raw.status,
        conclusion: raw.conclusion,
        run_number: raw.run_number,
        head_branch: raw.head_branch,
        html_url: raw.html_url,
        created_at: raw.created_at.unwrap_or_default(),
    }
}

pub(crate) fn map_commit_status(raw: RawCommitStatus) -> CommitStatus {
    CommitStatus {
        id: raw.id,
        context: raw.context,
        state: raw.state,
        description: raw.description.unwrap_or_default(),
    }
}

/// One row per name, newest wins. A commit often gets the same workflow twice,
/// once for the branch push and once for the pull request, and GitHub shows
/// only the latest of each; ids only ever grow, so they settle ties when two
/// runs report the same start.
pub(crate) fn latest_per_name<T>(
    items: impl IntoIterator<Item = T>,
    name: impl Fn(&T) -> &str,
    rank: impl Fn(&T) -> (String, u64),
) -> Vec<T> {
    let mut newest: Vec<(String, (String, u64), T)> = Vec::new();
    for item in items {
        let key = name(&item).to_owned();
        let stamp = rank(&item);
        match newest.iter_mut().find(|(existing, _, _)| *existing == key) {
            Some(slot) => {
                if stamp > slot.1 {
                    slot.1 = stamp;
                    slot.2 = item;
                }
            }
            None => newest.push((key, stamp, item)),
        }
    }
    newest.into_iter().map(|(_, _, item)| item).collect()
}

pub(crate) fn map_issue_user(raw: Option<RawIssueUser>) -> User {
    let raw = raw.unwrap_or(RawIssueUser {
        login: String::new(),
        avatar_url: String::new(),
    });
    User {
        login: raw.login,
        avatar_url: raw.avatar_url,
    }
}

pub(crate) fn map_issue_label(raw: &RawIssueLabel) -> Label {
    Label {
        name: raw.name.clone(),
        color: raw.color.clone(),
    }
}

/// The repository a search item belongs to. Search carries
/// `repository_url` but no repository name, and an item's web URL names the
/// repository two segments ahead of the `issues` or `pull` path segment.
pub(crate) fn search_item_repo_full_name(raw: &RawSearchIssue) -> String {
    raw.html_url
        .as_deref()
        .and_then(|url| {
            let parts: Vec<&str> = url.split('/').collect();
            let pos = parts
                .iter()
                .position(|part| *part == "issues" || *part == "pull")?;
            if pos >= 2 {
                Some(format!("{}/{}", parts[pos - 2], parts[pos - 1]))
            } else {
                None
            }
        })
        .or_else(|| {
            raw.repository_url.as_deref().and_then(|url| {
                let prefix = "https://api.github.com/repos/";
                url.strip_prefix(prefix)
                    .map(|rest| rest.trim_end_matches('/').to_owned())
            })
        })
        .unwrap_or_default()
}

pub(crate) fn map_search_issue_item(raw: RawSearchIssue) -> SearchIssueItem {
    let repo_full_name = search_item_repo_full_name(&raw);
    SearchIssueItem {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        state_reason: parse_state_reason(raw.state_reason.as_deref()),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        comment_count: raw.comments,
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| User {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        author: raw.user.map(|user| user.login).unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        repo_full_name,
    }
}

/// Pull requests off search items. `merged_at` is the only thing separating
/// a merged pull request from a merely closed one, since both report
/// `state: "closed"`. A repository-scoped search names the repo itself, since
/// the item URL carries no repository name for it.
pub(crate) fn map_search_pull_item(
    raw: RawSearchIssue,
    repo_full_name: Option<String>,
) -> PullRequestListItem {
    let repo_full_name = match repo_full_name {
        Some(name) => name,
        None => search_item_repo_full_name(&raw),
    };
    PullRequestListItem {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        state_reason: parse_state_reason(raw.state_reason.as_deref()),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        comment_count: raw.comments,
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| User {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        author: map_issue_user(raw.user),
        updated_at: raw.updated_at.unwrap_or_default(),
        merged_at: raw
            .pull_request
            .and_then(|reference| reference.merged_at)
            .filter(|at| !at.is_empty()),
        html_url: raw.html_url.unwrap_or_default(),
        repo_full_name,
    }
}

pub(crate) fn map_issue_detail(raw: RawIssue, participants: Vec<User>) -> IssueDetail {
    IssueDetail {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        state_reason: parse_state_reason(raw.state_reason.as_deref()),
        body: raw.body.unwrap_or_default(),
        author: map_issue_user(raw.user),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| User {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        participants,
        created_at: raw.created_at.unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

pub(crate) fn map_issue_comment(raw: RawIssueComment) -> IssueComment {
    IssueComment {
        id: raw.id,
        author: map_issue_user(raw.user),
        body: raw.body.unwrap_or_default(),
        created_at: raw.created_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

/// Query pairs for listing issues. An empty `labels` param filters to
/// unlabeled issues on GitHub's side, so it is only sent when filtering.
/// Order-preserving dedup by login, dropping empty logins. `dedup_by`
/// only collapses adjacent entries, so it cannot be used here.
pub(crate) fn dedup_users(users: impl IntoIterator<Item = User>) -> Vec<User> {
    let mut seen = std::collections::HashSet::new();
    users
        .into_iter()
        .filter(|user| !user.login.is_empty() && seen.insert(user.login.clone()))
        .collect()
}

pub(crate) fn map_issue_event(raw: RawIssueEvent) -> IssueEvent {
    let (actor, actor_avatar_url) = raw
        .actor
        .map(|user| (user.login, user.avatar_url))
        .unwrap_or_default();
    IssueEvent {
        id: raw.id,
        kind: raw.event,
        actor,
        actor_avatar_url,
        created_at: raw.created_at.unwrap_or_default(),
        label: raw.label.as_ref().map(|label| label.name.clone()),
        label_color: raw.label.as_ref().map(|label| label.color.clone()),
        assignee: raw
            .assignee
            .or(raw.requested_reviewer)
            .map(|user| user.login),
    }
}

pub(crate) fn empty_pull_ref() -> PullRequestRef {
    PullRequestRef {
        r#ref: String::new(),
        sha: String::new(),
        label: String::new(),
    }
}

pub(crate) fn map_pull_ref(raw: Option<RawPullRef>) -> PullRequestRef {
    raw.map(|reference| PullRequestRef {
        r#ref: reference.git_ref,
        sha: reference.sha,
        label: reference.label,
    })
    .unwrap_or_else(empty_pull_ref)
}

/// GitHub sends `mergeable` as a nullable boolean, so the tri-state survives
/// the wire intact: null means the merge is still being computed and the UI
/// has to re-read before it can offer the merge button.
pub(crate) fn map_pull_detail(raw: RawPull) -> PullRequestDetail {
    PullRequestDetail {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        state_reason: parse_state_reason(raw.state_reason.as_deref()),
        body: raw.body.unwrap_or_default(),
        author: map_issue_user(raw.user),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| User {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        created_at: raw.created_at.unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        draft: raw.draft,
        merged_at: raw.merged_at.filter(|at| !at.is_empty()),
        mergeable: raw.mergeable,
        mergeable_state: raw.mergeable_state.unwrap_or_default(),
        head: map_pull_ref(raw.head),
        base: map_pull_ref(raw.base),
        additions: raw.additions,
        deletions: raw.deletions,
        changed_files: raw.changed_files,
        commits: raw.commits,
    }
}

/// Commits come back oldest first, and a commit made outside GitHub has no
/// linked account, so the git author name stands in for the login.
pub(crate) fn map_pull_commit(raw: RawPullCommit) -> PullRequestCommit {
    let detail = raw.commit.unwrap_or_else(|| RawCommitDetail {
        message: String::new(),
        author: None,
    });
    let signature = detail.author.unwrap_or_else(|| RawCommitSignature {
        name: String::new(),
        date: None,
    });
    let account = raw.author.map(|user| (user.login, user.avatar_url));
    let (login, avatar_url) = account.unwrap_or((signature.name, String::new()));
    PullRequestCommit {
        sha: raw.sha,
        message: detail.message,
        author: User { login, avatar_url },
        authored_at: signature.date.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

pub(crate) fn map_pull_review(raw: RawPullReview) -> PullRequestReview {
    PullRequestReview {
        id: raw.id,
        author: map_issue_user(raw.user),
        state: raw.state,
        body: raw.body.unwrap_or_default(),
        submitted_at: raw.submitted_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

pub(crate) fn map_pull_review_comment(raw: RawPullReviewComment) -> PullRequestReviewComment {
    PullRequestReviewComment {
        id: raw.id,
        author: map_issue_user(raw.user),
        body: raw.body.unwrap_or_default(),
        path: raw.path,
        line: raw.line,
        diff_hunk: raw.diff_hunk,
        created_at: raw.created_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        in_reply_to_id: raw.in_reply_to_id,
        pull_request_review_id: raw.pull_request_review_id,
    }
}

/// Converts an API subject URL into a best-effort web URL, so rows link
/// out without a second round trip. Only types with a proven static
/// mapping qualify; releases (addressed by tag, not id) and check suites
/// (no web equivalent) yield `None` and resolve on click instead.
pub(crate) fn subject_html_url(api_url: Option<&str>, subject_type: &str) -> Option<String> {
    match subject_type {
        "Issue" | "PullRequest" | "Commit" | "Discussion" => {}
        _ => return None,
    }
    let url = api_url?;
    let rest = url.strip_prefix("https://api.github.com/repos/")?;
    let mut html = String::from("https://github.com/");
    html.push_str(
        &rest
            .replace("/pulls/", "/pull/")
            .replace("/commits/", "/commit/"),
    );
    Some(html)
}

pub(crate) fn map_notification(raw: RawNotification) -> Notification {
    let subject_url = raw.subject.as_ref().and_then(|s| s.url.as_deref());
    let subject_type = raw
        .subject
        .as_ref()
        .and_then(|s| s.kind.clone())
        .unwrap_or_else(|| "Unknown".to_owned());
    let repo_full_name = raw
        .repository
        .as_ref()
        .and_then(|r| r.full_name.clone())
        .unwrap_or_default();
    Notification {
        id: raw.id,
        unread: raw.unread,
        reason: raw.reason.unwrap_or_else(|| "subscribed".to_owned()),
        subject_title: raw
            .subject
            .as_ref()
            .and_then(|s| s.title.clone())
            .unwrap_or_default(),
        subject_type: subject_type.clone(),
        repo_full_name,
        html_url: subject_html_url(subject_url, &subject_type),
        subject_url: subject_url.map(str::to_owned),
        updated_at: raw.updated_at.unwrap_or_default(),
    }
}

/// Public repository fields. GitHub omits any it does not know, and an empty
/// description is treated as absent so the caller can fall back to the README.
pub(crate) fn map_repo_metadata(body: &str) -> Option<RepositoryMetadata> {
    #[derive(serde::Deserialize)]
    struct RawRepoMetadata {
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        stargazers_count: Option<u64>,
        #[serde(default)]
        forks_count: Option<u64>,
    }

    let raw: RawRepoMetadata = serde_json::from_str(body).ok()?;
    Some(RepositoryMetadata {
        description: raw.description.filter(|text| !text.is_empty()),
        stars: raw.stargazers_count,
        forks: raw.forks_count,
    })
}

/// Minimal UTC timestamp formatter, avoiding a chrono dependency for one
/// `last_read_at` field.
pub(crate) fn chrono_lite_rfc3339(epoch_secs: u64) -> String {
    let days = epoch_secs / 86_400;
    let rem = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's civil_from_days, valid for all post-1970 dates.
pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pull_requests_and_keeps_merge_state() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 42,
            "title": "Add a thing",
            "state": "closed",
            "user": { "login": "octocat", "avatar_url": "https://a/1" },
            "labels": [{ "name": "enhancement", "color": "a2eeef" }],
            "assignees": [{ "login": "hubot", "avatar_url": "https://a/2" }],
            "comments": 3,
            "updated_at": "2026-09-01T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42",
            "pull_request": { "merged_at": "2026-09-02T09:00:00Z" }
        }))
        .unwrap();
        let mapped = map_search_pull_item(raw, Some("octocat/repo".to_owned()));
        assert_eq!(mapped.number, 42);
        assert_eq!(mapped.author.login, "octocat");
        assert_eq!(mapped.author.avatar_url, "https://a/1");
        assert_eq!(mapped.assignees.len(), 1);
        assert_eq!(mapped.labels[0].name, "enhancement");
        assert_eq!(mapped.merged_at.as_deref(), Some("2026-09-02T09:00:00Z"));
        assert_eq!(mapped.repo_full_name, "octocat/repo");
    }

    /// `state_reason` is the only thing that separates an issue closed as
    /// fixed from one closed as stale, since both report `state: "closed"`.
    #[test]
    fn carries_the_close_reason_onto_search_items() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 3,
            "title": "Flaky test",
            "state": "closed",
            "state_reason": "not planned"
        }))
        .unwrap();
        assert_eq!(
            map_search_issue_item(raw).state_reason,
            ThreadStateReason::NotPlanned
        );
    }

    /// An open thread carries no reason, and neither does anything GitHub
    /// reports that this build does not know.
    #[test]
    fn unrecognised_close_reasons_fall_back_to_none() {
        assert_eq!(parse_state_reason(None), ThreadStateReason::None);
        assert_eq!(
            parse_state_reason(Some("reopened")),
            ThreadStateReason::None
        );
        assert_eq!(
            parse_state_reason(Some("completed")),
            ThreadStateReason::Completed
        );
        assert_eq!(
            parse_state_reason(Some("not_planned")),
            ThreadStateReason::NotPlanned
        );
        assert_eq!(
            parse_state_reason(Some("duplicate")),
            ThreadStateReason::Duplicate
        );
    }

    /// A cross-repo search has no repo name to hand the mapper, so the
    /// repository comes from the item's own URL.
    #[test]
    fn cross_repo_pull_request_names_its_repository_from_the_url() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 9,
            "title": "From somewhere else",
            "state": "open",
            "html_url": "https://github.com/hubot/tools/pull/9"
        }))
        .unwrap();
        assert_eq!(
            map_search_pull_item(raw, None).repo_full_name,
            "hubot/tools"
        );
    }

    /// A closed pull request with no merge time was abandoned, not merged.
    #[test]
    fn closed_pull_request_without_merge_time_has_no_merged_at() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 7,
            "title": "Abandoned",
            "state": "closed",
            "html_url": "https://github.com/octocat/repo/pull/7",
            "pull_request": { "merged_at": null }
        }))
        .unwrap();
        assert_eq!(map_search_pull_item(raw, None).merged_at, None);
    }

    #[test]
    fn maps_pull_detail_including_refs_and_merge_verdict() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 42,
            "title": "Add a thing",
            "state": "open",
            "body": "why",
            "user": { "login": "octocat", "avatar_url": "https://a/1" },
            "labels": [{ "name": "enhancement", "color": "a2eeef" }],
            "assignees": [{ "login": "hubot", "avatar_url": "https://a/2" }],
            "created_at": "2026-09-01T12:00:00Z",
            "updated_at": "2026-09-02T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42",
            "draft": true,
            "merged_at": null,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": { "ref": "feature", "sha": "aaa", "label": "octocat:feature" },
            "base": { "ref": "main", "sha": "bbb", "label": "octocat:main" },
            "additions": 10,
            "deletions": 3,
            "changed_files": 2,
            "commits": 4
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.number, 42);
        assert!(mapped.draft);
        assert_eq!(mapped.merged_at, None);
        assert_eq!(mapped.mergeable, Some(true));
        assert_eq!(mapped.mergeable_state, "clean");
        assert_eq!(mapped.head.r#ref, "feature");
        assert_eq!(mapped.base.r#ref, "main");
        assert_eq!(mapped.additions, 10);
        assert_eq!(mapped.commits, 4);
    }

    /// GitHub computes mergeability in the background and reports null until
    /// it finishes, so the tri-state has to survive the mapping.
    #[test]
    fn keeps_pending_mergeability_distinct_from_a_verdict() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 1,
            "title": "t",
            "state": "open",
            "mergeable": null,
            "mergeable_state": "unknown"
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.mergeable, None);
        assert_eq!(mapped.mergeable_state, "unknown");
    }

    #[test]
    fn pull_detail_tolerates_missing_refs_and_body() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 9,
            "title": "sparse",
            "state": "open"
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.body, "");
        assert_eq!(mapped.head.r#ref, "");
        assert_eq!(mapped.base.label, "");
        assert_eq!(mapped.additions, 0);
    }

    /// A review request names the user in `requested_reviewer`, not
    /// `assignee`, and the UI renders both through the same slot.
    #[test]
    fn review_request_events_reuse_the_assignee_slot() {
        let raw: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 5,
            "event": "review_requested",
            "actor": { "login": "octocat", "avatar_url": "https://a/1" },
            "created_at": "2026-09-01T12:00:00Z",
            "requested_reviewer": { "login": "hubot", "avatar_url": "https://a/2" }
        }))
        .unwrap();
        assert_eq!(map_issue_event(raw).assignee.as_deref(), Some("hubot"));
    }

    #[test]
    fn maps_review_comments_with_optional_line_and_reply() {
        let raw: RawPullReviewComment = serde_json::from_value(serde_json::json!({
            "id": 77,
            "user": { "login": "hubot", "avatar_url": "https://a/2" },
            "body": "nit",
            "path": "src/lib.rs",
            "line": 42,
            "diff_hunk": "@@ -1 +1 @@\n-old\n+new",
            "created_at": "2026-09-03T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42#discussion_r77",
            "in_reply_to_id": 70,
            "pull_request_review_id": 80
        }))
        .unwrap();
        let mapped = map_pull_review_comment(raw);
        assert_eq!(mapped.path, "src/lib.rs");
        assert_eq!(mapped.line, Some(42));
        assert_eq!(mapped.in_reply_to_id, Some(70));
        assert_eq!(mapped.pull_request_review_id, Some(80));
        assert!(mapped.diff_hunk.contains("+new"));
    }

    /// An outdated review comment reports no line, so the option must stay
    /// optional instead of defaulting to a bogus line 0.
    #[test]
    fn outdated_review_comments_have_no_line() {
        let raw: RawPullReviewComment = serde_json::from_value(serde_json::json!({
            "id": 78,
            "user": { "login": "hubot", "avatar_url": "" },
            "body": "old",
            "path": "src/lib.rs",
            "diff_hunk": "",
            "created_at": "2026-09-03T12:00:00Z",
            "html_url": ""
        }))
        .unwrap();
        assert_eq!(map_pull_review_comment(raw).line, None);
    }

    /// CodeRabbit and other reporters that never moved to the checks API
    /// only ever publish here, so the page has to read this endpoint too.
    #[test]
    fn maps_commit_statuses_with_their_descriptions() {
        let raw: RawCombinedStatus = serde_json::from_value(serde_json::json!({
            "state": "pending",
            "statuses": [
                {
                    "id": 55178427953u64,
                    "context": "CodeRabbit",
                    "state": "pending",
                    "description": "Review in progress",
                    "target_url": null
                },
                {
                    "id": 55178427954u64,
                    "context": "vercel",
                    "state": "success",
                    "description": null
                }
            ]
        }))
        .unwrap();
        let mapped: Vec<CommitStatus> = raw.statuses.into_iter().map(map_commit_status).collect();
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].context, "CodeRabbit");
        assert_eq!(mapped[0].state, "pending");
        assert_eq!(mapped[0].description, "Review in progress");
        assert_eq!(mapped[1].description, "");
    }

    /// A branch push and the pull request each trigger the same workflow, so
    /// one commit carries the same check name twice. GitHub shows only the
    /// latest of each, and a stale failing run must not outvote it.
    #[test]
    fn collapses_repeated_names_to_the_latest_run() {
        let push = CheckRun {
            id: 109444055402,
            name: "require-label".to_owned(),
            status: "completed".to_owned(),
            conclusion: Some("failure".to_owned()),
            details_url: Some(
                "https://github.com/octocat/repo/actions/runs/36579628281/job/1".to_owned(),
            ),
            started_at: "2026-09-29T14:02:28Z".to_owned(),
            completed_at: None,
        };
        let pull_request = CheckRun {
            id: 109444066282,
            name: "require-label".to_owned(),
            status: "completed".to_owned(),
            conclusion: Some("success".to_owned()),
            details_url: Some(
                "https://github.com/octocat/repo/actions/runs/36579631210/job/2".to_owned(),
            ),
            started_at: "2026-09-29T14:02:29Z".to_owned(),
            completed_at: None,
        };
        let other = CheckRun {
            id: 109444140071,
            name: "rust".to_owned(),
            started_at: "2026-09-29T14:02:38Z".to_owned(),
            ..push.clone()
        };
        let collapsed = latest_per_name(
            vec![push, pull_request, other],
            |run| run.name.as_str(),
            |run| (run.started_at.clone(), run.id),
        );
        assert_eq!(collapsed.len(), 2);
        let kept = collapsed
            .iter()
            .find(|run| run.name == "require-label")
            .unwrap();
        assert_eq!(kept.id, 109444066282);
        assert_eq!(kept.conclusion.as_deref(), Some("success"));
    }

    #[test]
    fn maps_pull_reviews() {
        let raw: RawPullReview = serde_json::from_value(serde_json::json!({
            "id": 3,
            "user": { "login": "hubot", "avatar_url": "https://a/2" },
            "state": "CHANGES_REQUESTED",
            "body": "please fix",
            "submitted_at": "2026-09-03T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42#pullrequestreview-3"
        }))
        .unwrap();
        let mapped = map_pull_review(raw);
        assert_eq!(mapped.state, "CHANGES_REQUESTED");
        assert_eq!(mapped.author.login, "hubot");
        assert_eq!(mapped.body, "please fix");
    }

    #[test]
    fn maps_pull_commits() {
        let raw: RawPullCommit = serde_json::from_value(serde_json::json!({
            "sha": "abc123",
            "commit": {
                "message": "Fix the thing\n\nWhy it mattered.",
                "author": { "name": "Hubot", "date": "2026-09-03T12:00:00Z" }
            },
            "author": { "login": "hubot", "avatar_url": "https://a/2" },
            "html_url": "https://github.com/octocat/repo/commit/abc123"
        }))
        .unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.sha, "abc123");
        assert_eq!(mapped.message, "Fix the thing\n\nWhy it mattered.");
        assert_eq!(mapped.author.login, "hubot");
        assert_eq!(mapped.author.avatar_url, "https://a/2");
        assert_eq!(mapped.authored_at, "2026-09-03T12:00:00Z");
        assert_eq!(
            mapped.html_url,
            "https://github.com/octocat/repo/commit/abc123"
        );
    }

    /// A commit whose email matches no account has no `author`, so the git
    /// author name is what the row shows.
    #[test]
    fn unlinked_pull_commit_falls_back_to_the_git_author() {
        let raw: RawPullCommit = serde_json::from_value(serde_json::json!({
            "sha": "def456",
            "commit": {
                "message": "From a laptop",
                "author": { "name": "Someone Else", "date": "2026-09-04T09:30:00Z" }
            },
            "author": null
        }))
        .unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.author.login, "Someone Else");
        assert_eq!(mapped.author.avatar_url, "");
        assert_eq!(mapped.html_url, "");
    }

    /// A commit payload with nothing but a sha still deserializes, so one
    /// malformed row cannot take the whole list down.
    #[test]
    fn pull_commit_tolerates_a_missing_commit_object() {
        let raw: RawPullCommit =
            serde_json::from_value(serde_json::json!({ "sha": "000" })).unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.sha, "000");
        assert_eq!(mapped.message, "");
        assert_eq!(mapped.author.login, "");
        assert_eq!(mapped.authored_at, "");
    }

    #[test]
    fn maps_public_repository_metadata() {
        let metadata = map_repo_metadata(
            r#"{"description":"a git client","stargazers_count":7,"forks_count":2}"#,
        )
        .unwrap();
        assert_eq!(metadata.description.as_deref(), Some("a git client"));
        assert_eq!(metadata.stars, Some(7));
        assert_eq!(metadata.forks, Some(2));

        // An empty description is treated as absent so the caller falls back
        // to the README instead of showing an empty line.
        let blank = map_repo_metadata(r#"{"description":""}"#).unwrap();
        assert_eq!(blank.description, None);
        assert_eq!(blank.stars, None);

        assert!(map_repo_metadata("not json").is_none());
    }

    #[test]
    fn maps_notification_threads() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({
            "id": "123",
            "unread": true,
            "reason": "mention",
            "subject": {
                "title": "Fix the bug",
                "type": "PullRequest",
                "url": "https://api.github.com/repos/octocat/repo/pulls/42"
            },
            "repository": { "full_name": "octocat/repo" },
            "updated_at": "2026-09-01T12:00:00Z"
        }))
        .unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.id, "123");
        assert!(mapped.unread);
        assert_eq!(mapped.reason, "mention");
        assert_eq!(mapped.subject_title, "Fix the bug");
        assert_eq!(mapped.repo_full_name, "octocat/repo");
        assert_eq!(
            mapped.html_url.as_deref(),
            Some("https://github.com/octocat/repo/pull/42")
        );
    }
    #[test]
    fn notification_mapping_tolerates_missing_fields() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({"id": "9"})).unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.reason, "subscribed");
        assert!(!mapped.unread);
        assert_eq!(mapped.html_url, None);
    }

    #[test]
    fn release_notifications_keep_subject_url_for_on_click_resolution() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({
            "id": "7",
            "unread": true,
            "reason": "subscribed",
            "subject": {
                "title": "v2.0",
                "type": "Release",
                "url": "https://api.github.com/repos/octocat/repo/releases/7"
            },
            "repository": { "full_name": "octocat/repo" },
            "updated_at": "2026-09-01T12:00:00Z"
        }))
        .unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.html_url, None);
        assert_eq!(
            mapped.subject_url.as_deref(),
            Some("https://api.github.com/repos/octocat/repo/releases/7")
        );
    }

    #[test]
    fn subject_url_mapping_rejects_non_api_hosts() {
        assert_eq!(subject_html_url(None, "Issue"), None);
        assert_eq!(
            subject_html_url(Some("https://example.com/repos/o/r/issues/1"), "Issue"),
            None
        );
        assert_eq!(
            subject_html_url(Some("https://api.github.com/repos/o/r/issues/7"), "Issue").as_deref(),
            Some("https://github.com/o/r/issues/7")
        );
    }

    #[test]
    fn subject_url_mapping_only_covers_directly_linked_types() {
        let api = "https://api.github.com/repos/o/r/pulls/42";
        assert_eq!(
            subject_html_url(Some(api), "PullRequest").as_deref(),
            Some("https://github.com/o/r/pull/42")
        );
        assert_eq!(subject_html_url(Some(api), "Release"), None);
        assert_eq!(subject_html_url(Some(api), "CheckSuite"), None);
        assert_eq!(subject_html_url(Some(api), "Unknown"), None);
    }

    #[test]
    fn participants_dedup_is_global_and_order_preserving() {
        let users = vec![
            User {
                login: "b".into(),
                avatar_url: String::new(),
            },
            User {
                login: "a".into(),
                avatar_url: String::new(),
            },
            User {
                login: "b".into(),
                avatar_url: "other".into(),
            },
            User {
                login: String::new(),
                avatar_url: String::new(),
            },
        ];
        let deduped = dedup_users(users);
        let logins: Vec<&str> = deduped.iter().map(|user| user.login.as_str()).collect();
        assert_eq!(logins, ["b", "a"]);
    }

    #[test]
    fn issue_event_mapping_carries_label_and_assignee() {
        let labeled: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 1,
            "event": "labeled",
            "actor": { "login": "octocat", "avatar_url": "https://a/u/1" },
            "created_at": "2026-09-01T12:00:00Z",
            "label": { "name": "bug", "color": "d73a4a" }
        }))
        .unwrap();
        let mapped = map_issue_event(labeled);
        assert_eq!(mapped.kind, "labeled");
        assert_eq!(mapped.actor, "octocat");
        assert_eq!(mapped.label.as_deref(), Some("bug"));
        assert_eq!(mapped.label_color.as_deref(), Some("d73a4a"));
        assert_eq!(mapped.assignee, None);

        let assigned: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 2,
            "event": "assigned",
            "actor": { "login": "octocat", "avatar_url": "" },
            "created_at": "2026-09-01T12:00:00Z",
            "assignee": { "login": "k" }
        }))
        .unwrap();
        let mapped = map_issue_event(assigned);
        assert_eq!(mapped.assignee.as_deref(), Some("k"));
        assert_eq!(mapped.label, None);
    }

    #[test]
    fn rfc3339_formatter_matches_known_date() {
        // 2026-01-01T00:00:00Z
        assert_eq!(chrono_lite_rfc3339(1_767_225_600), "2026-01-01T00:00:00Z");
        assert_eq!(chrono_lite_rfc3339(0), "1970-01-01T00:00:00Z");
    }
}
