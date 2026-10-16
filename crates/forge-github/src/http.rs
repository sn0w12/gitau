use std::time::Duration;

use forge_core::{ForgeFuture, ProviderError};
use serde::Deserialize;

pub(crate) const API_ROOT: &str = "https://api.github.com";
pub(crate) const LOGIN_ROOT: &str = "https://github.com";
pub(crate) const API_VERSION: &str = "2022-11-28";
pub(crate) const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// GitHub's maximum page size. Full pages keep `has_more` true as a
/// fallback when the `Link` header is missing.
pub(crate) const NOTIFICATIONS_PER_PAGE: u32 = 100;
pub(crate) const ISSUES_PER_PAGE: u32 = 100;
pub(crate) const MAX_ISSUE_PAGES: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Method {
    Get,
    Post,
    Patch,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub(crate) enum Body {
    Json(String),
    Form(Vec<(String, String)>),
}

#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub(crate) method: Method,
    pub(crate) url: String,
    pub(crate) query: Vec<(String, String)>,
    /// Present on authenticated calls; the transport turns it into the
    /// bearer header and the API version headers that go with it.
    pub(crate) token: Option<String>,
    pub(crate) body: Option<Body>,
    pub(crate) headers: Vec<(String, String)>,
    /// False only for the job log, whose redirect target rejects the API
    /// auth header and so is followed by hand.
    pub(crate) follow_redirects: bool,
}

impl Request {
    pub(crate) fn get(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            query: Vec::new(),
            token: Some(token.into()),
            body: None,
            headers: Vec::new(),
            follow_redirects: true,
        }
    }

    /// A GET with query parameters, encoded as form fields the way the API
    /// documents its own examples.
    pub(crate) fn query(
        url: impl Into<String>,
        token: impl Into<String>,
        query: Vec<(String, String)>,
    ) -> Self {
        Self {
            query,
            ..Self::get(url, token)
        }
    }

    /// A call to a host that takes no token, such as the OAuth endpoints.
    pub(crate) fn anonymous(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            query: Vec::new(),
            token: None,
            body: None,
            headers: vec![
                (
                    "Accept".to_owned(),
                    "application/vnd.github+json".to_owned(),
                ),
                ("X-GitHub-Api-Version".to_owned(), API_VERSION.to_owned()),
            ],
            follow_redirects: true,
        }
    }

    pub(crate) fn json(
        method: Method,
        url: impl Into<String>,
        token: impl Into<String>,
        body: String,
    ) -> Self {
        Self {
            method,
            url: url.into(),
            query: Vec::new(),
            token: Some(token.into()),
            body: Some(Body::Json(body)),
            headers: Vec::new(),
            follow_redirects: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) body: String,
    pub(crate) has_next_page: bool,
    pub(crate) location: Option<String>,
}

pub(crate) trait Transport: Send + Sync {
    fn send(&self, request: Request) -> ForgeFuture<Response>;
}

pub(crate) struct HttpTransport {
    client: reqwest::Client,
    no_redirect: reqwest::Client,
}

impl HttpTransport {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            client: client(timeout, reqwest::redirect::Policy::default()),
            no_redirect: client(timeout, reqwest::redirect::Policy::none()),
        }
    }

    fn build(&self, request: &Request) -> reqwest::RequestBuilder {
        let client = if request.follow_redirects {
            &self.client
        } else {
            &self.no_redirect
        };
        let mut builder = match request.method {
            Method::Get => client.get(&request.url),
            Method::Post => client.post(&request.url),
            Method::Patch => client.patch(&request.url),
            Method::Put => client.put(&request.url),
            Method::Delete => client.delete(&request.url),
        };
        if !request.query.is_empty() {
            builder = builder.query(&request.query);
        }
        if let Some(token) = &request.token {
            builder = builder
                .header("Authorization", bearer(token))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", API_VERSION);
        }
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        match &request.body {
            Some(Body::Json(body)) => builder
                .header("Content-Type", "application/json")
                .body(body.clone()),
            Some(Body::Form(fields)) => builder.form(fields),
            None => builder,
        }
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl Transport for HttpTransport {
    fn send(&self, request: Request) -> ForgeFuture<Response> {
        let builder = self.build(&request);
        Box::pin(async move {
            let response = builder.send().await.map_err(network)?;
            let status = response.status().as_u16();
            let header = |name: reqwest::header::HeaderName| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            let has_next_page = header(reqwest::header::LINK)
                .map(|link| link_next(&link))
                .unwrap_or(false);
            let location = header(reqwest::header::LOCATION);
            let body = response.text().await.map_err(network)?;
            Ok(Response {
                status,
                body,
                has_next_page,
                location,
            })
        })
    }
}

fn network(error: reqwest::Error) -> ProviderError {
    ProviderError::Network {
        message: error.to_string(),
    }
}

fn client(timeout: Duration, redirect: reqwest::redirect::Policy) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(redirect)
        .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client builds with rustls-tls")
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
    let Some(location) = redirect.location.clone() else {
        return Err(api_error(redirect.status, &redirect.body));
    };
    let blob = transport
        .send(Request {
            method: Method::Get,
            url: location,
            query: Vec::new(),
            token: None,
            body: None,
            headers: Vec::new(),
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
    response.has_next_page || count == NOTIFICATIONS_PER_PAGE as usize
}

/// True when a `Link` header advertises a next page, per RFC 8288
/// (`<url>; rel="next", ...`).
pub(crate) fn link_next(link: &str) -> bool {
    link.split(',').any(|part| {
        let mut segments = part.split(';').map(str::trim);
        let _url = segments.next();
        segments.any(|param| param.eq_ignore_ascii_case(r#"rel="next""#))
    })
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
        let response = |has_next_page| Response {
            status: 200,
            body: String::new(),
            has_next_page,
            location: None,
        };
        assert!(!page_has_more(&response(false), 0));
        assert!(!page_has_more(&response(false), 12));
        assert!(page_has_more(
            &response(false),
            NOTIFICATIONS_PER_PAGE as usize
        ));
        assert!(page_has_more(&response(true), 3));
    }

    #[test]
    fn detects_next_page_from_link_header() {
        assert!(!link_next(""));
        assert!(link_next(
            r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=5>; rel="last""#
        ));
        assert!(!link_next(
            r#"<https://api.github.com/notifications?page=1>; rel="prev", <https://api.github.com/notifications?page=1>; rel="first""#
        ));
    }
}
