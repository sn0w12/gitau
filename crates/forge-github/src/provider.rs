use std::path::{Path, PathBuf};

use forge_core::dto::{
    AccountProfile, ActionStep, CheckRun, CheckRunDetail, CheckRunLog, CommitStatus,
    CreatePullRequestBody, CreateRepositoryBody, CreatedRepository, IssueComment, IssueDetail,
    IssueEvent, Label, MergePullRequestBody, MergePullRequestResult, Notification,
    NotificationPage, Org, PullRequestCommit, PullRequestDetail, PullRequestListItem,
    PullRequestReview, PullRequestReviewComment, RepoPermissions, RepositoryMetadata,
    SearchIssueItem, SearchIssuePage, SearchPullRequestPage, UpdateIssueBody,
    UpdatePullRequestBody, User, WorkflowRun,
};
use forge_core::{
    AuthChallenge, AuthPoll, Capabilities, ForgeFuture, ForgeProvider, ProviderError, RemoteKind,
};

use crate::device_flow::{parse_device_code, parse_token_poll, DeviceCodeResponse, TokenPoll};
use crate::http::{
    fetch_job_log, internal, malformed, page_has_more, send, send_full, Method, Request, Transport,
    API_ROOT, DEVICE_GRANT_TYPE, ISSUES_PER_PAGE, LOGIN_ROOT, MAX_ISSUE_PAGES,
    NOTIFICATIONS_PER_PAGE,
};
use crate::job_log::{collapse_teardown, slice_log_by_steps};
use crate::map::{
    chrono_lite_rfc3339, dedup_users, latest_per_name, map_check_run, map_check_run_detail,
    map_commit_status, map_issue_comment, map_issue_detail, map_issue_event, map_issue_label,
    map_notification, map_pull_commit, map_pull_detail, map_pull_review, map_pull_review_comment,
    map_repo_metadata, map_search_issue_item, map_search_pull_item, map_workflow_run,
    parse_state_reason,
};
use crate::wire::{
    CreatePullPayload, MergePullPayload, RawActionJob, RawCheckAnnotation, RawCheckRunDetail,
    RawCheckRunsResponse, RawCombinedStatus, RawIssue, RawIssueComment, RawIssueEvent,
    RawMergeResult, RawNotification, RawOrg, RawPermissionsEnvelope, RawPull, RawPullCommit,
    RawPullReview, RawPullReviewComment, RawRepo, RawSearchIssue, RawSearchIssueResponse,
    RawSubject, RawUser, RawWorkflowRunsResponse,
};
use crate::{parse_github_remote, resolve_client_id, GithubProvider, HOST, OAUTH_SCOPES};

/// Annotations live on their own endpoint, not inside the check run, and are
/// paginated. A run that fails a suite reports one annotation per failed test.
async fn list_check_annotations(
    transport: &dyn Transport,
    token: &str,
    owner: &str,
    repo: &str,
    check_run_id: u64,
) -> Vec<RawCheckAnnotation> {
    let mut collected = Vec::new();
    for page in 1..=MAX_ISSUE_PAGES {
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/check-runs/{check_run_id}/annotations");
        let query = vec![
            ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
            ("page".to_owned(), page.to_string()),
        ];
        let Ok(response) = send_full(transport, Request::query(url, token, query)).await else {
            // Annotations are supplementary; a failed read leaves the run
            // readable rather than failing the whole detail call.
            break;
        };
        let Ok(parsed) = serde_json::from_str::<Vec<RawCheckAnnotation>>(&response.body) else {
            break;
        };
        let count = parsed.len();
        collected.extend(parsed);
        if !page_has_more(&response, count) {
            break;
        }
    }
    collected
}

/// `list_issues` and `list_pull_requests` hit the same endpoint and differ
/// only in the `is:` qualifier and the projection, so both build their query
/// here. Returns the raw items plus whether another page exists.
async fn search_repo(
    transport: &dyn Transport,
    token: &str,
    terms: &[String],
    page: u32,
) -> Result<(Vec<RawSearchIssue>, bool), ProviderError> {
    let query = vec![
        ("q".to_owned(), terms.join(" ")),
        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
        ("page".to_owned(), page.to_string()),
        ("sort".to_owned(), "created".to_owned()),
        ("order".to_owned(), "desc".to_owned()),
    ];
    let response = send_full(
        transport,
        Request::query(format!("{API_ROOT}/search/issues"), token, query),
    )
    .await?;
    let raw: RawSearchIssueResponse = serde_json::from_str(&response.body).map_err(malformed)?;
    let has_more = page_has_more(&response, raw.items.len());
    Ok((raw.items, has_more))
}

/// Qualifiers shared by both repo-scoped listings: scope to the repository,
/// narrow to one state when asked, and require every named label.
fn repo_search_terms(
    qualifier: &str,
    owner: &str,
    repo: &str,
    state: &str,
    labels: &[String],
) -> Vec<String> {
    let mut terms = vec![qualifier.to_owned(), format!("repo:{owner}/{repo}")];
    if state == "open" || state == "closed" {
        terms.push(format!("state:{state}"));
    }
    for label in labels {
        terms.push(format!("label:\"{label}\""));
    }
    terms
}

impl ForgeProvider for GithubProvider {
    fn id(&self) -> &'static str {
        "github"
    }

    fn label(&self) -> &'static str {
        "GitHub"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::all()
    }

    fn hosts(&self) -> &'static [&'static str] {
        &[HOST]
    }

    fn image_hosts(&self) -> &'static [&'static str] {
        &[
            HOST,
            "githubusercontent.com",
            "githubassets.com",
            "githubapp.com",
        ]
    }

    fn parse_remote(&self, url: &str) -> Option<(String, String, String)> {
        parse_github_remote(url).map(|(owner, repo)| (HOST.to_owned(), owner, repo))
    }

    fn web_url(
        &self,
        host: &str,
        owner: &str,
        repo: &str,
        kind: RemoteKind,
        number: u64,
    ) -> Option<String> {
        let path = match kind {
            RemoteKind::Issue => format!("issues/{number}"),
            RemoteKind::PullRequest => format!("pull/{number}"),
            RemoteKind::CheckSuite => return None,
            RemoteKind::Commit | RemoteKind::Release | RemoteKind::Repository => return None,
        };
        Some(format!("https://{host}/{owner}/{repo}/{path}"))
    }

    fn credential_username(&self) -> &'static str {
        "x-access-token"
    }

    fn legacy_token_ids(&self) -> &'static [&'static str] {
        &["github"]
    }

    fn legacy_profile_files(&self, dir: &Path) -> Vec<(String, PathBuf)> {
        vec![(HOST.to_owned(), dir.join("github-account.json"))]
    }

    fn client_id(&self) -> Option<String> {
        Some(resolve_client_id())
    }

    fn oauth_scopes(&self) -> &'static str {
        OAUTH_SCOPES
    }

    fn request_device_code(
        &self,
        client_id: &str,
        scopes: &str,
    ) -> ForgeFuture<(AuthChallenge, String)> {
        let transport = self.transport.clone();
        let request = Request {
            body: Some(crate::http::Body::Form(vec![
                ("client_id".to_owned(), client_id.to_owned()),
                ("scope".to_owned(), scopes.to_owned()),
            ])),
            ..Request::anonymous(Method::Post, format!("{LOGIN_ROOT}/login/device/code"))
        };
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let DeviceCodeResponse {
                device_code,
                user_code,
                verification_uri,
                expires_in_secs,
                ..
            } = parse_device_code(&body).map_err(|message| ProviderError::MalformedResponse {
                provider: "github".into(),
                message,
            })?;
            Ok((
                AuthChallenge {
                    user_code,
                    verification_uri,
                    expires_in_secs,
                },
                device_code,
            ))
        })
    }

    fn poll_token(&self, client_id: &str, device_code: &str) -> ForgeFuture<AuthPoll> {
        let transport = self.transport.clone();
        let request = Request {
            body: Some(crate::http::Body::Form(vec![
                ("client_id".to_owned(), client_id.to_owned()),
                ("device_code".to_owned(), device_code.to_owned()),
                ("grant_type".to_owned(), DEVICE_GRANT_TYPE.to_owned()),
            ])),
            ..Request::anonymous(
                Method::Post,
                format!("{LOGIN_ROOT}/login/oauth/access_token"),
            )
        };
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let poll =
                parse_token_poll(&body).map_err(|message| ProviderError::MalformedResponse {
                    provider: "github".into(),
                    message,
                })?;
            Ok(match poll {
                TokenPoll::Authorized {
                    access_token,
                    scopes,
                } => AuthPoll::Authorized {
                    token: access_token,
                    scopes,
                },
                TokenPoll::Pending { retry_after_secs } => AuthPoll::Pending { retry_after_secs },
                TokenPoll::Expired => AuthPoll::Expired,
                TokenPoll::Denied { message } => AuthPoll::Denied { message },
            })
        })
    }

    fn authenticated_user(&self, token: &str) -> ForgeFuture<AccountProfile> {
        let transport = self.transport.clone();
        let request = Request::get(format!("{API_ROOT}/user"), token);
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: RawUser = serde_json::from_str(&body).map_err(malformed)?;
            Ok(AccountProfile {
                login: raw.login,
                name: raw.name,
                avatar_url: raw.avatar_url.unwrap_or_default(),
                html_url: raw.html_url.unwrap_or_default(),
                scopes: vec![],
                connected_at_ms: 0,
            })
        })
    }

    fn list_orgs(&self, token: &str) -> ForgeFuture<Vec<Org>> {
        let transport = self.transport.clone();
        let request = Request::get(format!("{API_ROOT}/user/orgs"), token);
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: Vec<RawOrg> = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw
                .into_iter()
                .map(|org| Org {
                    login: org.login,
                    avatar_url: org.avatar_url,
                })
                .collect())
        })
    }

    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepositoryBody,
    ) -> ForgeFuture<CreatedRepository> {
        let transport = self.transport.clone();
        let url = match owner {
            Some(owner) => format!("{API_ROOT}/orgs/{owner}/repos"),
            None => format!("{API_ROOT}/user/repos"),
        };
        let payload = serde_json::to_string(body).map_err(internal);
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Post, url, token, payload?);
            let text = send(&*transport, request).await?;
            let raw: RawRepo = serde_json::from_str(&text).map_err(malformed)?;
            Ok(CreatedRepository {
                full_name: raw.full_name,
                html_url: raw.html_url.unwrap_or_default(),
                default_branch: raw.default_branch,
                clone_url: raw.clone_url,
            })
        })
    }

    fn list_notifications(&self, token: &str, page: u32) -> ForgeFuture<NotificationPage> {
        let transport = self.transport.clone();
        let page = page.max(1);
        let request = Request::query(
            format!("{API_ROOT}/notifications"),
            token,
            vec![
                ("all".to_owned(), "true".to_owned()),
                ("participating".to_owned(), "false".to_owned()),
                ("per_page".to_owned(), NOTIFICATIONS_PER_PAGE.to_string()),
                ("page".to_owned(), page.to_string()),
            ],
        );
        Box::pin(async move {
            let response = send_full(&*transport, request).await?;
            let raw: Vec<RawNotification> =
                serde_json::from_str(&response.body).map_err(malformed)?;
            let notifications: Vec<Notification> = raw.into_iter().map(map_notification).collect();
            let has_more = page_has_more(&response, notifications.len());
            Ok(NotificationPage {
                notifications,
                page,
                has_more,
            })
        })
    }

    fn mark_notification_read(&self, token: &str, thread_id: &str) -> ForgeFuture<()> {
        let transport = self.transport.clone();
        let request = Request {
            method: Method::Patch,
            ..Request::get(
                format!("{API_ROOT}/notifications/threads/{thread_id}"),
                token,
            )
        };
        Box::pin(async move {
            send(&*transport, request).await?;
            Ok(())
        })
    }

    fn mark_all_notifications_read(&self, token: &str) -> ForgeFuture<()> {
        let transport = self.transport.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let payload = serde_json::json!({
            "last_read_at": chrono_lite_rfc3339(now).to_string()
        })
        .to_string();
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(
                Method::Put,
                format!("{API_ROOT}/notifications"),
                token,
                payload,
            );
            send(&*transport, request).await?;
            Ok(())
        })
    }

    fn fetch_subject_html_url(
        &self,
        token: &str,
        subject_url: &str,
    ) -> ForgeFuture<Option<String>> {
        let transport = self.transport.clone();
        let subject_url = subject_url.to_owned();
        let token = token.to_owned();
        Box::pin(async move {
            if !subject_url.starts_with("https://api.github.com/") {
                return Err(ProviderError::Internal {
                    message: "refusing to fetch a non-GitHub subject URL".into(),
                });
            }
            let body = match send(&*transport, Request::get(subject_url, token)).await {
                Ok(body) => body,
                // The subject is gone; the row falls back to the repo page.
                Err(ProviderError::NotFound { .. }) => return Ok(None),
                Err(error) => return Err(error),
            };
            let raw: RawSubject = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw.html_url.filter(|url| !url.is_empty()))
        })
    }

    fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> ForgeFuture<SearchIssuePage> {
        let transport = self.transport.clone();
        let token = token.to_owned();
        let terms = repo_search_terms("is:issue", owner, repo, state, labels);
        let repo_full_name = format!("{owner}/{repo}");
        let page = page.max(1);
        Box::pin(async move {
            let (items, has_more) = search_repo(&*transport, &token, &terms, page).await?;
            let items: Vec<SearchIssueItem> = items
                .into_iter()
                .map(|item| {
                    let labels: Vec<Label> = item.labels.iter().map(map_issue_label).collect();
                    let assignees: Vec<User> = item
                        .assignees
                        .into_iter()
                        .map(|user| User {
                            login: user.login,
                            avatar_url: user.avatar_url,
                        })
                        .collect();
                    SearchIssueItem {
                        number: item.number,
                        title: item.title,
                        state: item.state,
                        state_reason: parse_state_reason(item.state_reason.as_deref()),
                        labels,
                        comment_count: item.comments,
                        assignees,
                        author: item.user.map(|user| user.login).unwrap_or_default(),
                        updated_at: item.updated_at.unwrap_or_default(),
                        html_url: item.html_url.unwrap_or_default(),
                        repo_full_name: repo_full_name.clone(),
                    }
                })
                .collect();
            Ok(SearchIssuePage {
                items,
                page,
                has_more,
            })
        })
    }

    fn get_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<IssueDetail> {
        let transport = self.transport.clone();
        let request = Request::get(
            format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}"),
            token,
        );
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: RawIssue = serde_json::from_str(&body).map_err(malformed)?;
            // The issue endpoint has no participants list; the detail
            // carries author + assignees and the UI merges comment authors.
            let mut participants = vec![raw
                .user
                .as_ref()
                .map(|user| User {
                    login: user.login.clone(),
                    avatar_url: user.avatar_url.clone(),
                })
                .unwrap_or_default()];
            participants.extend(raw.assignees.iter().map(|user| User {
                login: user.login.clone(),
                avatar_url: user.avatar_url.clone(),
            }));
            Ok(map_issue_detail(raw, dedup_users(participants)))
        })
    }

    fn list_issue_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<Vec<IssueComment>> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/comments");
        let token = token.to_owned();
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let request = Request::query(
                    url.clone(),
                    &token,
                    vec![
                        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                        ("page".to_owned(), page.to_string()),
                    ],
                );
                let response = send_full(&*transport, request).await?;
                let raw: Vec<RawIssueComment> =
                    serde_json::from_str(&response.body).map_err(malformed)?;
                let has_more = page_has_more(&response, raw.len());
                items.extend(raw.into_iter().map(map_issue_comment));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_issue_events(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<Vec<IssueEvent>> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/events");
        let token = token.to_owned();
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let request = Request::query(
                    url.clone(),
                    &token,
                    vec![
                        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                        ("page".to_owned(), page.to_string()),
                    ],
                );
                let response = send_full(&*transport, request).await?;
                let raw: Vec<RawIssueEvent> =
                    serde_json::from_str(&response.body).map_err(malformed)?;
                let has_more = page_has_more(&response, raw.len());
                items.extend(raw.into_iter().map(map_issue_event));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn create_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> ForgeFuture<IssueComment> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/comments");
        let payload = serde_json::json!({ "body": body }).to_string();
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Post, url, token, payload);
            let text = send(&*transport, request).await?;
            let raw: RawIssueComment = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_comment(raw))
        })
    }

    fn update_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdateIssueBody,
    ) -> ForgeFuture<IssueDetail> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}");
        let payload = serde_json::to_string(body).map_err(internal);
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Patch, url, token, payload?);
            let text = send(&*transport, request).await?;
            let raw: RawIssue = serde_json::from_str(&text).map_err(malformed)?;
            let participants = dedup_users(raw.assignees.iter().map(|user| User {
                login: user.login.clone(),
                avatar_url: user.avatar_url.clone(),
            }));
            Ok(map_issue_detail(raw, participants))
        })
    }

    fn create_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        labels: &[String],
    ) -> ForgeFuture<IssueDetail> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues");
        let mut payload = serde_json::json!({ "title": title });
        if let Some(body) = body {
            payload["body"] = serde_json::Value::String(body.to_owned());
        }
        if !labels.is_empty() {
            payload["labels"] = serde_json::json!(labels);
        }
        let payload = payload.to_string();
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Post, url, token, payload);
            let text = send(&*transport, request).await?;
            let raw: RawIssue = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_detail(raw, vec![]))
        })
    }

    fn update_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> ForgeFuture<IssueComment> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/comments/{comment_id}");
        let payload = serde_json::json!({ "body": body }).to_string();
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Patch, url, token, payload);
            let text = send(&*transport, request).await?;
            let raw: RawIssueComment = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_comment(raw))
        })
    }

    fn delete_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> ForgeFuture<()> {
        let transport = self.transport.clone();
        let request = Request {
            method: Method::Delete,
            ..Request::get(
                format!("{API_ROOT}/repos/{owner}/{repo}/issues/comments/{comment_id}"),
                token,
            )
        };
        Box::pin(async move {
            send(&*transport, request).await?;
            Ok(())
        })
    }

    fn repo_permissions(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> ForgeFuture<RepoPermissions> {
        let transport = self.transport.clone();
        let request = Request::get(format!("{API_ROOT}/repos/{owner}/{repo}"), token);
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: RawPermissionsEnvelope = serde_json::from_str(&body).map_err(malformed)?;
            let permissions = raw.permissions.unwrap_or_default();
            Ok(RepoPermissions {
                push: permissions.push,
                admin: permissions.admin,
            })
        })
    }

    fn repo_metadata(
        &self,
        _host: &str,
        owner: &str,
        repo: &str,
    ) -> ForgeFuture<Option<RepositoryMetadata>> {
        let transport = self.transport.clone();
        let request = Request::anonymous(Method::Get, format!("{API_ROOT}/repos/{owner}/{repo}"));
        Box::pin(async move {
            let response = transport.send(request).await?;
            // A private or missing repository answers 404 for an anonymous
            // read, which is not an error: there is just nothing to show.
            if !(200..300).contains(&response.status) {
                return Ok(None);
            }
            Ok(map_repo_metadata(&response.body))
        })
    }

    fn list_pull_requests(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> ForgeFuture<SearchPullRequestPage> {
        let transport = self.transport.clone();
        let token = token.to_owned();
        let terms = repo_search_terms("is:pr", owner, repo, state, labels);
        let repo_full_name = format!("{owner}/{repo}");
        let page = page.max(1);
        Box::pin(async move {
            let (items, has_more) = search_repo(&*transport, &token, &terms, page).await?;
            let items = items
                .into_iter()
                .map(|item| map_search_pull_item(item, Some(repo_full_name.clone())))
                .collect();
            Ok(SearchPullRequestPage {
                items,
                page,
                has_more,
            })
        })
    }

    fn get_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<PullRequestDetail> {
        let transport = self.transport.clone();
        let request = Request::get(
            format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}"),
            token,
        );
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: RawPull = serde_json::from_str(&body).map_err(malformed)?;
            Ok(map_pull_detail(raw))
        })
    }

    fn list_pull_reviews(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<Vec<PullRequestReview>> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/reviews");
        let token = token.to_owned();
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let request = Request::query(
                    url.clone(),
                    &token,
                    vec![
                        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                        ("page".to_owned(), page.to_string()),
                    ],
                );
                let response = send_full(&*transport, request).await?;
                let raw: Vec<RawPullReview> =
                    serde_json::from_str(&response.body).map_err(malformed)?;
                let has_more = page_has_more(&response, raw.len());
                items.extend(raw.into_iter().map(map_pull_review));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_pull_commits(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<Vec<PullRequestCommit>> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/commits");
        let token = token.to_owned();
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let request = Request::query(
                    url.clone(),
                    &token,
                    vec![
                        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                        ("page".to_owned(), page.to_string()),
                    ],
                );
                let response = send_full(&*transport, request).await?;
                let raw: Vec<RawPullCommit> =
                    serde_json::from_str(&response.body).map_err(malformed)?;
                let has_more = page_has_more(&response, raw.len());
                items.extend(raw.into_iter().map(map_pull_commit));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_pull_review_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> ForgeFuture<Vec<PullRequestReviewComment>> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/comments");
        let token = token.to_owned();
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let request = Request::query(
                    url.clone(),
                    &token,
                    vec![
                        ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                        ("page".to_owned(), page.to_string()),
                    ],
                );
                let response = send_full(&*transport, request).await?;
                let raw: Vec<RawPullReviewComment> =
                    serde_json::from_str(&response.body).map_err(malformed)?;
                let has_more = page_has_more(&response, raw.len());
                items.extend(raw.into_iter().map(map_pull_review_comment));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn create_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        body: &CreatePullRequestBody,
    ) -> ForgeFuture<PullRequestDetail> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls");
        let payload = serde_json::to_string(&CreatePullPayload::from(body)).map_err(internal);
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Post, url, token, payload?);
            let response = send_full(&*transport, request).await?;
            let raw: RawPull = serde_json::from_str(&response.body).map_err(malformed)?;
            Ok(map_pull_detail(raw))
        })
    }

    fn update_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdatePullRequestBody,
    ) -> ForgeFuture<PullRequestDetail> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}");
        let payload = serde_json::to_string(body).map_err(internal);
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Patch, url, token, payload?);
            let response = send_full(&*transport, request).await?;
            let raw: RawPull = serde_json::from_str(&response.body).map_err(malformed)?;
            Ok(map_pull_detail(raw))
        })
    }

    fn merge_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &MergePullRequestBody,
    ) -> ForgeFuture<MergePullRequestResult> {
        let transport = self.transport.clone();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/merge");
        let payload = serde_json::to_string(&MergePullPayload::from(body)).map_err(internal);
        let token = token.to_owned();
        Box::pin(async move {
            let request = Request::json(Method::Put, url, token, payload?);
            let response = send_full(&*transport, request).await?;
            let raw: RawMergeResult = serde_json::from_str(&response.body).map_err(malformed)?;
            Ok(MergePullRequestResult {
                sha: raw.sha,
                merged: raw.merged,
                message: raw.message,
            })
        })
    }

    fn search_issues(&self, token: &str, page: u32) -> ForgeFuture<SearchIssuePage> {
        let transport = self.transport.clone();
        let page = page.max(1);
        let request = Request::query(
            format!("{API_ROOT}/search/issues"),
            token,
            vec![
                (
                    "q".to_owned(),
                    "is:issue involves:@me sort:updated-desc".to_owned(),
                ),
                ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                ("page".to_owned(), page.to_string()),
            ],
        );
        Box::pin(async move {
            let response = send_full(&*transport, request).await?;
            let raw: RawSearchIssueResponse =
                serde_json::from_str(&response.body).map_err(malformed)?;
            let items: Vec<SearchIssueItem> =
                raw.items.into_iter().map(map_search_issue_item).collect();
            let has_more = page_has_more(&response, items.len());
            Ok(SearchIssuePage {
                items,
                page,
                has_more,
            })
        })
    }

    fn search_pull_requests(&self, token: &str, page: u32) -> ForgeFuture<SearchPullRequestPage> {
        let transport = self.transport.clone();
        let page = page.max(1);
        let request = Request::query(
            format!("{API_ROOT}/search/issues"),
            token,
            vec![
                (
                    "q".to_owned(),
                    "is:pr involves:@me sort:updated-desc".to_owned(),
                ),
                ("per_page".to_owned(), ISSUES_PER_PAGE.to_string()),
                ("page".to_owned(), page.to_string()),
            ],
        );
        Box::pin(async move {
            let response = send_full(&*transport, request).await?;
            let raw: RawSearchIssueResponse =
                serde_json::from_str(&response.body).map_err(malformed)?;
            let items: Vec<PullRequestListItem> = raw
                .items
                .into_iter()
                .map(|item| map_search_pull_item(item, None))
                .collect();
            let has_more = page_has_more(&response, items.len());
            Ok(SearchPullRequestPage {
                items,
                page,
                has_more,
            })
        })
    }

    fn list_check_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> ForgeFuture<Vec<CheckRun>> {
        let transport = self.transport.clone();
        let request = Request::query(
            format!("{API_ROOT}/repos/{owner}/{repo}/commits/{sha}/check-runs"),
            token,
            vec![("per_page".to_owned(), ISSUES_PER_PAGE.to_string())],
        );
        Box::pin(async move {
            let response = send_full(&*transport, request).await?;
            let raw: RawCheckRunsResponse =
                serde_json::from_str(&response.body).map_err(malformed)?;
            let runs: Vec<CheckRun> = raw.check_runs.into_iter().map(map_check_run).collect();
            Ok(latest_per_name(
                runs,
                |run| run.name.as_str(),
                |run| (run.started_at.clone(), run.id),
            ))
        })
    }

    fn get_check_run(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> ForgeFuture<CheckRunDetail> {
        let transport = self.transport.clone();
        let token = token.to_owned();
        let owner = owner.to_owned();
        let repo = repo.to_owned();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/check-runs/{check_run_id}");
        Box::pin(async move {
            let body = send(&*transport, Request::get(url, &token)).await?;
            let raw: RawCheckRunDetail = serde_json::from_str(&body).map_err(malformed)?;
            let annotations =
                list_check_annotations(&*transport, &token, &owner, &repo, check_run_id).await;
            Ok(map_check_run_detail(raw, annotations))
        })
    }

    fn get_check_run_log(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> ForgeFuture<CheckRunLog> {
        let transport = self.transport.clone();
        let token = token.to_owned();
        let owner = owner.to_owned();
        let repo = repo.to_owned();
        Box::pin(async move {
            // A GitHub Actions check run and its job share one id, so the
            // check run id addresses the job directly.
            let job_id = check_run_id;
            let job_url = format!("{API_ROOT}/repos/{owner}/{repo}/actions/jobs/{job_id}");
            let job_body = send(&*transport, Request::get(job_url, &token)).await?;
            let job: RawActionJob = serde_json::from_str(&job_body).map_err(malformed)?;

            let log = match fetch_job_log(&*transport, &token, &owner, &repo, job_id).await {
                Ok(text) => text,
                Err(error) => {
                    return Ok(CheckRunLog {
                        unavailable: Some(error.to_string()),
                        ..Default::default()
                    });
                }
            };

            let steps: Vec<ActionStep> = job
                .steps
                .into_iter()
                .map(|step| ActionStep {
                    number: step.number,
                    name: step.name.unwrap_or_default(),
                    status: step.status.unwrap_or_default(),
                    conclusion: step.conclusion,
                    started_at: step.started_at,
                    completed_at: step.completed_at,
                    log: String::new(),
                    spans_by_line: Vec::new(),
                })
                .collect();
            let mut steps = collapse_teardown(steps, &log);
            let mut table = ansi::AnsiTable::default();
            let buckets = slice_log_by_steps(&steps, &log, &mut table);
            for (step, bucket) in steps.iter_mut().zip(buckets) {
                bucket.into_step(step);
            }
            Ok(CheckRunLog {
                steps,
                styles: table.styles().to_vec(),
                raw: log,
                unavailable: None,
            })
        })
    }

    fn list_workflow_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> ForgeFuture<Vec<WorkflowRun>> {
        let transport = self.transport.clone();
        let request = Request::query(
            format!("{API_ROOT}/repos/{owner}/{repo}/actions/runs"),
            token,
            vec![
                ("head_sha".to_owned(), sha.to_owned()),
                ("per_page".to_owned(), "50".to_owned()),
            ],
        );
        Box::pin(async move {
            let response = send_full(&*transport, request).await?;
            let raw: RawWorkflowRunsResponse =
                serde_json::from_str(&response.body).map_err(malformed)?;
            let runs: Vec<WorkflowRun> = raw
                .workflow_runs
                .into_iter()
                .map(map_workflow_run)
                .collect();
            Ok(latest_per_name(
                runs,
                |run| run.name.as_str(),
                |run| (run.created_at.clone(), run.id),
            ))
        })
    }

    fn list_commit_statuses(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> ForgeFuture<Vec<CommitStatus>> {
        let transport = self.transport.clone();
        let request = Request::get(
            format!("{API_ROOT}/repos/{owner}/{repo}/commits/{sha}/status"),
            token,
        );
        Box::pin(async move {
            let body = send(&*transport, request).await?;
            let raw: RawCombinedStatus = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw.statuses.into_iter().map(map_commit_status).collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{Body, Response};
    use std::sync::{Arc, Mutex};

    /// Transport that records the requests it is given and replays scripted
    /// responses, so request shapes can be asserted without a network.
    #[derive(Default)]
    struct Recorder {
        requests: Mutex<Vec<Request>>,
        responses: Mutex<Vec<Response>>,
    }

    impl Recorder {
        fn new(responses: Vec<Response>) -> Arc<Self> {
            Arc::new(Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses),
            })
        }

        fn sent(&self) -> Vec<Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Transport for Recorder {
        fn send(&self, request: Request) -> ForgeFuture<Response> {
            self.requests.lock().unwrap().push(request);
            let mut responses = self.responses.lock().unwrap();
            let response = if responses.len() > 1 {
                responses.remove(0)
            } else {
                responses.first().cloned().unwrap_or_else(|| ok(""))
            };
            Box::pin(async move { Ok(response) })
        }
    }

    fn ok(body: impl Into<String>) -> Response {
        Response {
            status: 200,
            body: body.into(),
            has_next_page: false,
            location: None,
        }
    }

    fn provider(recorder: &Arc<Recorder>) -> GithubProvider {
        GithubProvider::with_transport(recorder.clone())
    }

    #[test]
    fn advertises_its_image_hosts() {
        assert_eq!(
            GithubProvider::default().image_hosts(),
            &[
                "github.com",
                "githubusercontent.com",
                "githubassets.com",
                "githubapp.com",
            ]
        );
    }

    fn param(request: &Request, name: &str) -> Option<String> {
        request
            .query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    fn form_fields(request: &Request) -> Vec<(String, String)> {
        match request.body.as_ref() {
            Some(Body::Form(fields)) => fields.clone(),
            other => panic!("expected a form body, got {other:?}"),
        }
    }

    fn json_body(request: &Request) -> serde_json::Value {
        match request.body.as_ref() {
            Some(Body::Json(body)) => serde_json::from_str(body).unwrap(),
            other => panic!("expected a json body, got {other:?}"),
        }
    }

    #[test]
    fn repo_search_terms_scope_to_one_kind_and_repository() {
        let terms = repo_search_terms("is:pr", "octocat", "repo", "open", &[]);
        assert_eq!(terms, vec!["is:pr", "repo:octocat/repo", "state:open"]);

        let with_labels = repo_search_terms("is:issue", "octocat", "repo", "all", &["bug".into()]);
        assert_eq!(
            with_labels,
            vec!["is:issue", "repo:octocat/repo", "label:\"bug\""]
        );
    }

    #[tokio::test]
    async fn list_issues_scopes_the_search_to_the_repository() {
        let recorder = Recorder::new(vec![ok(r#"{"items":[],"total_count":0}"#)]);
        let issues = provider(&recorder)
            .list_issues("tok", "octocat", "repo", "open", &["bug".into()], 2)
            .await
            .unwrap();

        assert_eq!(issues.page, 2);
        let request = &recorder.sent()[0];
        assert_eq!(request.url, format!("{API_ROOT}/search/issues"));
        assert_eq!(request.token.as_deref(), Some("tok"));
        assert_eq!(
            param(request, "q").as_deref(),
            Some("is:issue repo:octocat/repo state:open label:\"bug\"")
        );
        assert_eq!(param(request, "per_page").as_deref(), Some("100"));
        assert_eq!(param(request, "page").as_deref(), Some("2"));
    }

    #[test]
    fn device_flow_uses_the_oauth_host_without_a_token() {
        let recorder = Recorder::new(vec![ok(
            r#"{"device_code":"dev","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#,
        )]);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let challenge = runtime
            .block_on(provider(&recorder).request_device_code("client-1", "repo"))
            .unwrap();

        assert_eq!(challenge.0.user_code, "ABCD-1234");
        let request = &recorder.sent()[0];
        assert_eq!(request.url, format!("{LOGIN_ROOT}/login/device/code"));
        assert!(request.token.is_none());
        assert_eq!(
            form_fields(request),
            vec![
                ("client_id".to_owned(), "client-1".to_owned()),
                ("scope".to_owned(), "repo".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn create_repository_targets_the_org_or_user_endpoint() {
        let recorder = Recorder::new(vec![ok(
            r#"{"full_name":"acme/demo","html_url":"https://github.com/acme/demo"}"#,
        )]);
        let body = CreateRepositoryBody {
            name: "demo".into(),
            description: None,
            private: true,
        };
        let provider = provider(&recorder);

        provider
            .create_repository("tok", Some("acme"), &body)
            .await
            .unwrap();
        assert_eq!(
            recorder.sent()[0].url,
            format!("{API_ROOT}/orgs/acme/repos")
        );

        provider
            .create_repository("tok", None, &body)
            .await
            .unwrap();
        assert_eq!(recorder.sent()[1].url, format!("{API_ROOT}/user/repos"));
    }

    #[tokio::test]
    async fn create_pull_posts_the_snake_case_pull_body() {
        let recorder = Recorder::new(vec![ok(
            r#"{"number":9,"title":"Add thing","state":"open","html_url":"https://github.com/octocat/repo/pull/9"}"#,
        )]);
        let pull = provider(&recorder)
            .create_pull(
                "tok",
                "octocat",
                "repo",
                &CreatePullRequestBody {
                    title: "Add thing".into(),
                    head: "feature/thing".into(),
                    base: "main".into(),
                    body: Some("Does the thing".into()),
                    draft: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(pull.number, 9);
        let request = &recorder.sent()[0];
        assert!(matches!(request.method, Method::Post));
        assert_eq!(request.url, format!("{API_ROOT}/repos/octocat/repo/pulls"));
        assert_eq!(
            json_body(request),
            serde_json::json!({
                "title": "Add thing",
                "head": "feature/thing",
                "base": "main",
                "body": "Does the thing",
                "draft": true,
            })
        );
    }

    #[tokio::test]
    async fn merge_pull_puts_the_snake_case_merge_body() {
        let recorder = Recorder::new(vec![ok(
            r#"{"sha":"abc","merged":true,"message":"Pull Request merged"}"#,
        )]);
        let result = provider(&recorder)
            .merge_pull(
                "tok",
                "octocat",
                "repo",
                7,
                &MergePullRequestBody {
                    merge_method: forge_core::dto::PullRequestMergeMethod::Squash,
                    commit_title: Some("squashed".into()),
                    commit_message: None,
                },
            )
            .await
            .unwrap();

        assert!(result.merged);
        let request = &recorder.sent()[0];
        assert!(matches!(request.method, Method::Put));
        assert_eq!(
            request.url,
            format!("{API_ROOT}/repos/octocat/repo/pulls/7/merge")
        );
        assert_eq!(
            json_body(request),
            serde_json::json!({"merge_method": "squash", "commit_title": "squashed"})
        );
    }

    #[tokio::test]
    async fn job_log_follows_the_redirect_without_the_auth_header() {
        let recorder = Recorder::new(vec![
            ok(r#"{"steps":[]}"#),
            Response {
                status: 302,
                body: String::new(),
                has_next_page: false,
                location: Some("https://blob.example/log.txt".into()),
            },
            ok("hello"),
        ]);
        let log = provider(&recorder)
            .get_check_run_log("tok", "octocat", "repo", 42)
            .await
            .unwrap();

        assert_eq!(log.raw, "hello");
        let sent = recorder.sent();
        assert!(sent[1].token.is_some());
        assert!(!sent[1].follow_redirects);
        assert_eq!(sent[2].url, "https://blob.example/log.txt");
        assert!(sent[2].token.is_none());
    }

    #[tokio::test]
    async fn repo_metadata_reads_public_fields_without_a_token() {
        let recorder = Recorder::new(vec![ok(
            r#"{"description":"a git client","stargazers_count":12,"forks_count":3}"#,
        )]);
        let metadata = provider(&recorder)
            .repo_metadata("github.com", "octocat", "repo")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(metadata.description.as_deref(), Some("a git client"));
        assert_eq!(metadata.stars, Some(12));
        assert_eq!(metadata.forks, Some(3));

        let request = &recorder.sent()[0];
        assert_eq!(request.url, format!("{API_ROOT}/repos/octocat/repo"));
        assert!(request.token.is_none());
    }

    #[tokio::test]
    async fn an_invisible_repository_has_no_metadata() {
        let recorder = Recorder::new(vec![Response {
            status: 404,
            body: r#"{"message":"Not Found"}"#.into(),
            has_next_page: false,
            location: None,
        }]);

        let metadata = provider(&recorder)
            .repo_metadata("github.com", "octocat", "private")
            .await
            .unwrap();

        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn a_full_page_of_notifications_reports_more() {
        let body = serde_json::json!({
            "id": "1",
            "unread": true,
            "reason": "mention",
            "subject": { "title": "t", "type": "Issue", "url": "https://api.github.com/x" },
            "repository": { "full_name": "octocat/repo" },
            "updated_at": "2026-01-01T00:00:00Z",
        });
        let items = vec![body; NOTIFICATIONS_PER_PAGE as usize];
        let recorder = Recorder::new(vec![ok(serde_json::Value::Array(items).to_string())]);

        let page = provider(&recorder)
            .list_notifications("tok", 0)
            .await
            .unwrap();

        assert_eq!(page.page, 1);
        assert!(page.has_more);
        assert_eq!(page.notifications.len(), NOTIFICATIONS_PER_PAGE as usize);
    }
}
