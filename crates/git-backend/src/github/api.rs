use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::device_flow::{DeviceCodeResponse, TokenPoll, parse_device_code, parse_token_poll};
use super::{AccountProfile, GitHubError, GithubOrg};

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
struct RawApiError {
    #[serde(default)]
    message: Option<String>,
}

const API_ROOT: &str = "https://api.github.com";
const LOGIN_ROOT: &str = "https://github.com";
const API_VERSION: &str = "2022-11-28";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

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
    let response = request
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| GitHubError::Network {
            message: error.to_string(),
        })?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(api_error(status.as_u16(), &body))
    }
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
}
