use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::device_flow::{DeviceCodeResponse, TokenPoll, parse_device_code, parse_token_poll};
use super::{AccountProfile, GitHubError, GithubNotification, GithubOrg, NotificationPage};

/// Object-safe async surface: boxed futures let tests inject fakes without
/// a network.
pub type GithubFuture<T> =
    Pin<Box<dyn Future<Output = std::result::Result<T, GitHubError>> + Send>>;

/// Minimal GitHub REST surface used by the integration.
pub trait GithubApi: Send + Sync {
    fn request_device_code(
        &self,
        client_id: &str,
        scopes: &str,
    ) -> GithubFuture<DeviceCodeResponse>;
    fn poll_token(&self, client_id: &str, device_code: &str) -> GithubFuture<TokenPoll>;
    fn authenticated_user(&self, token: &str) -> GithubFuture<AccountProfile>;
    fn list_orgs(&self, token: &str) -> GithubFuture<Vec<GithubOrg>>;
    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepoBody,
    ) -> GithubFuture<CreatedRepository>;
    fn list_notifications(&self, token: &str, page: u32) -> GithubFuture<NotificationPage>;
    fn mark_notification_read(&self, token: &str, thread_id: &str) -> GithubFuture<()>;
    fn mark_all_notifications_read(&self, token: &str) -> GithubFuture<()>;
    /// Resolves a notification subject API URL to its web URL by fetching
    /// the subject and reading `html_url`. Returns `None` when the subject
    /// is gone (404). Used for types with no static URL mapping, like
    /// releases (addressed by tag, not id) and check suites.
    fn fetch_subject_html_url(
        &self,
        token: &str,
        subject_url: &str,
    ) -> GithubFuture<Option<String>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateRepoBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedRepository {
    pub full_name: String,
    pub html_url: String,
    pub default_branch: Option<String>,
    /// HTTPS clone URL as reported by GitHub; publish wires this up as
    /// `origin`. Tests substitute a local path here.
    pub clone_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    login: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawOrg {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRepo {
    full_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    clone_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotification {
    id: String,
    #[serde(default)]
    unread: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    subject: Option<RawNotificationSubject>,
    #[serde(default)]
    repository: Option<RawNotificationRepo>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotificationSubject {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotificationRepo {
    #[serde(default)]
    full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSubject {
    #[serde(default)]
    html_url: Option<String>,
}

/// Converts an API subject URL into a best-effort web URL, so rows link
/// out without a second round trip. Only types with a proven static
/// mapping qualify; releases (addressed by tag, not id) and check suites
/// (no web equivalent) yield `None` and resolve on click instead.
fn subject_html_url(api_url: Option<&str>, subject_type: &str) -> Option<String> {
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

fn map_notification(raw: RawNotification) -> GithubNotification {
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
    GithubNotification {
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

#[derive(Debug, Deserialize)]
struct RawApiError {
    #[serde(default)]
    message: Option<String>,
}

const API_ROOT: &str = "https://api.github.com";
const LOGIN_ROOT: &str = "https://github.com";
const API_VERSION: &str = "2022-11-28";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// GitHub's maximum page size. Full pages keep `has_more` true as a
/// fallback when the `Link` header is missing.
const NOTIFICATIONS_PER_PAGE: u32 = 100;

pub struct HttpGithubApi {
    client: reqwest::Client,
}

impl HttpGithubApi {
    pub fn new(timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with rustls-tls");
        Self { client }
    }
}

impl Default for HttpGithubApi {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

async fn send(request: reqwest::RequestBuilder) -> std::result::Result<String, GitHubError> {
    send_full(request).await.map(|(body, _)| body)
}

async fn send_full(
    request: reqwest::RequestBuilder,
) -> std::result::Result<(String, reqwest::header::HeaderMap), GitHubError> {
    let response = request
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .map_err(|error| GitHubError::Network {
            message: error.to_string(),
        })?;
    if status.is_success() {
        Ok((body, headers))
    } else {
        Err(api_error(status.as_u16(), &body))
    }
}

/// True when another page likely exists: the `Link` header advertises it,
/// or the page came back full (headers can go missing behind proxies).
fn page_has_more(headers: &reqwest::header::HeaderMap, count: usize) -> bool {
    has_next_page(headers) || count == NOTIFICATIONS_PER_PAGE as usize
}

/// True when the response `Link` header advertises a next page, per
/// RFC 8288 (`<url>; rel="next", ...`).
fn has_next_page(headers: &reqwest::header::HeaderMap) -> bool {
    let Some(link) = headers.get(reqwest::header::LINK) else {
        return false;
    };
    let Ok(link) = link.to_str() else {
        return false;
    };
    link.split(',').any(|part| {
        let mut segments = part.split(';').map(str::trim);
        let _url = segments.next();
        segments.any(|param| param.eq_ignore_ascii_case(r#"rel="next""#))
    })
}

fn api_error(status: u16, body: &str) -> GitHubError {
    let message = serde_json::from_str::<RawApiError>(body)
        .ok()
        .and_then(|raw| raw.message)
        .unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        401 => GitHubError::Unauthorized,
        403 => GitHubError::Forbidden { message },
        404 => GitHubError::NotFound { message },
        _ => GitHubError::Api { status, message },
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn malformed(error: serde_json::Error) -> GitHubError {
    GitHubError::MalformedResponse {
        message: error.to_string(),
    }
}

fn internal(error: serde_json::Error) -> GitHubError {
    GitHubError::Internal {
        message: error.to_string(),
    }
}

impl GithubApi for HttpGithubApi {
    fn request_device_code(
        &self,
        client_id: &str,
        scopes: &str,
    ) -> GithubFuture<DeviceCodeResponse> {
        let client = self.client.clone();
        let client_id = client_id.to_owned();
        let scopes = scopes.to_owned();
        Box::pin(async move {
            let form = [("client_id", client_id), ("scope", scopes)];
            let body = send(
                client
                    .post(format!("{LOGIN_ROOT}/login/device/code"))
                    .form(&form),
            )
            .await?;
            parse_device_code(&body).map_err(|message| GitHubError::MalformedResponse { message })
        })
    }

    fn poll_token(&self, client_id: &str, device_code: &str) -> GithubFuture<TokenPoll> {
        let client = self.client.clone();
        let client_id = client_id.to_owned();
        let device_code = device_code.to_owned();
        Box::pin(async move {
            let form = [
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", DEVICE_GRANT_TYPE.to_owned()),
            ];
            let body = send(
                client
                    .post(format!("{LOGIN_ROOT}/login/oauth/access_token"))
                    .form(&form),
            )
            .await?;
            parse_token_poll(&body).map_err(|message| GitHubError::MalformedResponse { message })
        })
    }

    fn authenticated_user(&self, token: &str) -> GithubFuture<AccountProfile> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let body = send(
                client
                    .get(format!("{API_ROOT}/user"))
                    .header("Authorization", authorization),
            )
            .await?;
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

    fn list_orgs(&self, token: &str) -> GithubFuture<Vec<GithubOrg>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let body = send(
                client
                    .get(format!("{API_ROOT}/user/orgs"))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: Vec<RawOrg> = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw
                .into_iter()
                .map(|org| GithubOrg {
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
        body: &CreateRepoBody,
    ) -> GithubFuture<CreatedRepository> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = match owner {
            Some(owner) => format!("{API_ROOT}/orgs/{owner}/repos"),
            None => format!("{API_ROOT}/user/repos"),
        };
        let payload = serde_json::to_vec(body).map_err(internal);
        Box::pin(async move {
            let payload = payload?;
            let text = send(
                client
                    .post(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawRepo = serde_json::from_str(&text).map_err(malformed)?;
            Ok(CreatedRepository {
                full_name: raw.full_name,
                html_url: raw.html_url.unwrap_or_default(),
                default_branch: raw.default_branch,
                clone_url: raw.clone_url,
            })
        })
    }

    fn list_notifications(&self, token: &str, page: u32) -> GithubFuture<NotificationPage> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let page = page.max(1);
        Box::pin(async move {
            let (body, headers) = send_full(
                client
                    .get(format!(
                        "{API_ROOT}/notifications?all=true&participating=false&per_page={NOTIFICATIONS_PER_PAGE}&page={page}"
                    ))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: Vec<RawNotification> = serde_json::from_str(&body).map_err(malformed)?;
            let notifications: Vec<GithubNotification> =
                raw.into_iter().map(map_notification).collect();
            let has_more = page_has_more(&headers, notifications.len());
            Ok(NotificationPage {
                notifications,
                page,
                has_more,
            })
        })
    }

    fn mark_notification_read(&self, token: &str, thread_id: &str) -> GithubFuture<()> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/notifications/threads/{thread_id}");
        Box::pin(async move {
            send(client.patch(url).header("Authorization", authorization)).await?;
            Ok(())
        })
    }

    fn mark_all_notifications_read(&self, token: &str) -> GithubFuture<()> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let body = serde_json::json!({
                "last_read_at": format!(
                    "{}",
                    chrono_lite_rfc3339(now)
                )
            });
            send(
                client
                    .put(format!("{API_ROOT}/notifications"))
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            )
            .await?;
            Ok(())
        })
    }

    fn fetch_subject_html_url(
        &self,
        token: &str,
        subject_url: &str,
    ) -> GithubFuture<Option<String>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let subject_url = subject_url.to_owned();
        Box::pin(async move {
            if !subject_url.starts_with("https://api.github.com/") {
                return Err(GitHubError::Internal {
                    message: "refusing to fetch a non-GitHub subject URL".into(),
                });
            }
            let body = match send(
                client
                    .get(subject_url)
                    .header("Authorization", authorization),
            )
            .await
            {
                Ok(body) => body,
                // The subject is gone; the row falls back to the repo page.
                Err(GitHubError::NotFound { .. }) => return Ok(None),
                Err(error) => return Err(error),
            };
            let raw: RawSubject = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw.html_url.filter(|url| !url.is_empty()))
        })
    }
}

/// Minimal UTC timestamp formatter, avoiding a chrono dependency for one
/// `last_read_at` field.
fn chrono_lite_rfc3339(epoch_secs: u64) -> String {
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
fn civil_from_days(z: i64) -> (i64, i64, i64) {
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
    fn maps_status_codes_to_error_kinds() {
        assert_eq!(
            api_error(401, r#"{"message":"Bad credentials"}"#).code(),
            "authenticationRequired"
        );
        assert!(matches!(
            api_error(403, r#"{"message":"rate limit"}"#),
            GitHubError::Forbidden { .. }
        ));
        assert!(matches!(
            api_error(404, r#"{"message":"Not Found"}"#),
            GitHubError::NotFound { .. }
        ));
        assert!(matches!(
            api_error(422, r#"{"message":"name already exists on this account"}"#),
            GitHubError::Api { status: 422, .. }
        ));
        assert!(matches!(
            api_error(500, "oops"),
            GitHubError::Api { status: 500, .. }
        ));
    }

    #[test]
    fn falls_back_to_status_line_when_body_is_not_json() {
        match api_error(502, "<html>bad gateway</html>") {
            GitHubError::Api { status, message } => {
                assert_eq!(status, 502);
                assert_eq!(message, "HTTP 502");
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn create_body_serializes_snake_case_without_nulls() {
        let json = serde_json::to_value(CreateRepoBody {
            name: "repo".into(),
            description: None,
            private: true,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"name": "repo", "private": true}));
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
    fn page_has_more_combines_link_header_and_full_pages() {
        use reqwest::header::{HeaderMap, HeaderValue, LINK};

        let headers = HeaderMap::new();
        assert!(!page_has_more(&headers, 0));
        assert!(!page_has_more(&headers, 12));
        assert!(page_has_more(&headers, NOTIFICATIONS_PER_PAGE as usize));

        let mut linked = HeaderMap::new();
        linked.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=2>; rel="next""#,
            ),
        );
        assert!(page_has_more(&linked, 3));
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
    fn rfc3339_formatter_matches_known_date() {
        // 2026-01-01T00:00:00Z
        assert_eq!(chrono_lite_rfc3339(1_767_225_600), "2026-01-01T00:00:00Z");
        assert_eq!(chrono_lite_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn detects_next_page_from_link_header() {
        use reqwest::header::{HeaderMap, HeaderValue, LINK};

        let mut headers = HeaderMap::new();
        assert!(!has_next_page(&headers));

        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=5>; rel="last""#,
            ),
        );
        assert!(has_next_page(&headers));

        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=1>; rel="prev", <https://api.github.com/notifications?page=1>; rel="first""#,
            ),
        );
        assert!(!has_next_page(&headers));
    }
}
