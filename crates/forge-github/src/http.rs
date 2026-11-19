use forge_core::ProviderError;
use serde::Deserialize;

pub(crate) use forge_core::http::{Body, Method, Request, Response, Transport};

pub(crate) const API_ROOT: &str = "https://api.github.com";
pub(crate) const LOGIN_ROOT: &str = "https://github.com";
pub(crate) const API_VERSION: &str = "2022-11-28";
pub(crate) const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// GitHub's maximum page size. Full pages keep `has_more` true as a
/// fallback when the `Link` header is missing.
pub(crate) const NOTIFICATIONS_PER_PAGE: u32 = 100;
pub(crate) const ISSUES_PER_PAGE: u32 = 100;
pub(crate) const MAX_ISSUE_PAGES: u32 = 10;

/// GitHub's request constructors on top of the neutral [`Request`]: the token
/// becomes the bearer header plus the API version headers that go with it.
pub(crate) trait GithubRequest {
    fn get(url: impl Into<String>, token: impl Into<String>) -> Self;
    fn query(
        url: impl Into<String>,
        token: impl Into<String>,
        query: Vec<(String, String)>,
    ) -> Self;
    /// A call to a host that takes no token, such as the OAuth endpoints.
    fn anonymous(method: Method, url: impl Into<String>) -> Self;
    fn json(method: Method, url: impl Into<String>, token: impl Into<String>, body: String)
        -> Self;
}

impl GithubRequest for Request {
    fn get(url: impl Into<String>, token: impl Into<String>) -> Self {
        Request {
            method: Method::Get,
            url: url.into(),
            query: Vec::new(),
            headers: authenticated(&token.into()),
            body: None,
            follow_redirects: true,
        }
    }

    fn query(
        url: impl Into<String>,
        token: impl Into<String>,
        query: Vec<(String, String)>,
    ) -> Self {
        Request {
            query,
            ..Request::get(url, token)
        }
    }

    fn anonymous(method: Method, url: impl Into<String>) -> Self {
        Request {
            method,
            url: url.into(),
            query: Vec::new(),
            headers: vec![
                (
                    "Accept".to_owned(),
                    "application/vnd.github+json".to_owned(),
                ),
                ("X-GitHub-Api-Version".to_owned(), API_VERSION.to_owned()),
            ],
            body: None,
            follow_redirects: true,
        }
    }

    fn json(
        method: Method,
        url: impl Into<String>,
        token: impl Into<String>,
        body: String,
    ) -> Self {
        Request {
            method,
            body: Some(Body::Json(body)),
            ..Request::get(url, token)
        }
    }
}

fn authenticated(token: &str) -> Vec<(String, String)> {
    vec![
        ("Authorization".to_owned(), bearer(token)),
        (
            "Accept".to_owned(),
            "application/vnd.github+json".to_owned(),
        ),
        ("X-GitHub-Api-Version".to_owned(), API_VERSION.to_owned()),
    ]
}

/// Sends and unwraps a successful body, mapping any other status to an error.
pub(crate) async fn send(
    transport: &dyn Transport,
    request: Request,
) -> Result<String, ProviderError> {
    send_full(transport, request)
        .await
        .map(|response| response.body)
}

/// Sends and returns the response, erroring on a non-success status.
pub(crate) async fn send_full(
    transport: &dyn Transport,
    request: Request,
) -> Result<Response, ProviderError> {
    let response = transport.send(request).await?;
    if (200..300).contains(&response.status) {
        Ok(response)
    } else {
        Err(api_error(response.status, &response.body))
    }
}

/// Fetches a job's log. The endpoint answers a 302 to a blob store holding
/// plain text: it rejects a `text/plain` accept header with 415, and the
/// redirect target rejects the API auth header, so the hop is made by hand.
pub(crate) async fn fetch_job_log(
    transport: &dyn Transport,
    token: &str,
    owner: &str,
    repo: &str,
    job_id: u64,
) -> Result<String, ProviderError> {
    let url = format!("{API_ROOT}/repos/{owner}/{repo}/actions/jobs/{job_id}/logs");
    let redirect = transport
        .send(Request {
            follow_redirects: false,
            ..Request::get(url, token)
        })
        .await?;
    let Some(location) = redirect.location().map(str::to_owned) else {
        return Err(api_error(redirect.status, &redirect.body));
    };
    let blob = transport
        .send(Request {
            method: Method::Get,
            url: location,
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
            follow_redirects: false,
        })
        .await?;
    if (200..300).contains(&blob.status) {
        Ok(blob.body)
    } else {
        Err(api_error(blob.status, &blob.body))
    }
}

/// True when another page likely exists: the `Link` header advertises it,
/// or the page came back full (headers can go missing behind proxies).
pub(crate) fn page_has_more(response: &Response, count: usize) -> bool {
    response.has_next_page() || count == NOTIFICATIONS_PER_PAGE as usize
}

pub(crate) fn api_error(status: u16, body: &str) -> ProviderError {
    #[derive(Deserialize)]
    struct RawApiError {
        #[serde(default)]
        message: Option<String>,
    }

    let message = serde_json::from_str::<RawApiError>(body)
        .ok()
        .and_then(|raw| raw.message)
        .unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        401 => ProviderError::Unauthorized {
            provider: "github".into(),
            host: "github.com".into(),
        },
        403 => ProviderError::Forbidden {
            provider: "github".into(),
            message,
        },
        404 => ProviderError::NotFound {
            provider: "github".into(),
            message,
        },
        _ => ProviderError::Api {
            provider: "github".into(),
            status,
            message,
        },
    }
}

pub(crate) fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

pub(crate) fn malformed(error: serde_json::Error) -> ProviderError {
    ProviderError::MalformedResponse {
        provider: "github".into(),
        message: error.to_string(),
    }
}

pub(crate) fn internal(error: serde_json::Error) -> ProviderError {
    ProviderError::Internal {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes_to_error_kinds() {
        assert!(matches!(
            api_error(401, r#"{"message":"Bad credentials"}"#),
            ProviderError::Unauthorized { .. }
        ));
        assert!(matches!(
            api_error(403, r#"{"message":"rate limit"}"#),
            ProviderError::Forbidden { .. }
        ));
        assert!(matches!(
            api_error(404, r#"{"message":"Not Found"}"#),
            ProviderError::NotFound { .. }
        ));
        assert!(matches!(
            api_error(422, r#"{"message":"name already exists on this account"}"#),
            ProviderError::Api { status: 422, .. }
        ));
        assert!(matches!(
            api_error(500, "oops"),
            ProviderError::Api { status: 500, .. }
        ));
    }

    #[test]
    fn falls_back_to_status_line_when_body_is_not_json() {
        match api_error(502, "<html>bad gateway</html>") {
            ProviderError::Api {
                status, message, ..
            } => {
                assert_eq!(status, 502);
                assert_eq!(message, "HTTP 502");
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn page_has_more_combines_link_header_and_full_pages() {
        let with_link = Response::new(200, "").with_header(
            "Link",
            r#"<https://api.github.com/notifications?page=2>; rel="next""#,
        );
        let plain = Response::new(200, "");
        assert!(!page_has_more(&plain, 0));
        assert!(!page_has_more(&plain, 12));
        assert!(page_has_more(&plain, NOTIFICATIONS_PER_PAGE as usize));
        assert!(page_has_more(&with_link, 3));
    }
}
