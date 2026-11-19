//! Provider-neutral HTTP transport.
//!
//! Providers build [`Request`]s, send them through a [`Transport`], and read
//! [`Response`]s; error mapping stays in the provider, because status codes
//! mean different things per service. [`HttpTransport`] is the reqwest-backed
//! implementation, and [`ConditionalTransport`] wraps any transport with a
//! conditional-GET cache so unchanged reads cost no body and, on services
//! that exempt `304 Not Modified` (GitHub does), no rate limit.
//!
//! A provider needs neither to know about `ETag`, `Last-Modified`, or the
//! cache: it wraps its transport once and sends ordinary GETs.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::{ForgeFuture, ProviderError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Put,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Json(String),
    Form(Vec<(String, String)>),
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub query: Vec<(String, String)>,
    /// Sent as-is, so the provider owns authentication and API-version
    /// headers. The conditional cache hashes them into its key.
    pub headers: Vec<(String, String)>,
    pub body: Option<Body>,
    /// False for endpoints whose redirect target rejects the provider's auth
    /// header and so must be followed by hand.
    pub follow_redirects: bool,
}

impl Request {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
            follow_redirects: true,
        }
    }

    pub fn with_query(mut self, query: Vec<(String, String)>) -> Self {
        self.query = query;
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_json_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(Body::Json(body.into()));
        self
    }

    pub fn with_form_body(mut self, fields: Vec<(String, String)>) -> Self {
        self.body = Some(Body::Form(fields));
        self
    }

    pub fn with_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            headers: Vec::new(),
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn location(&self) -> Option<&str> {
        self.header("location")
    }

    /// True when the `Link` header advertises a next page, per RFC 8288.
    pub fn has_next_page(&self) -> bool {
        self.header("link").is_some_and(link_next)
    }
}

pub trait Transport: Send + Sync {
    fn send(&self, request: Request) -> ForgeFuture<Response>;
}

impl<T: Transport + ?Sized> Transport for Arc<T> {
    fn send(&self, request: Request) -> ForgeFuture<Response> {
        (**self).send(request)
    }
}

pub struct HttpTransport {
    client: reqwest::Client,
    no_redirect: reqwest::Client,
}

impl HttpTransport {
    pub fn new(timeout: Duration) -> Self {
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
            let headers = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                })
                .collect();
            let body = response.text().await.map_err(network)?;
            Ok(Response {
                status,
                body,
                headers,
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

/// True when a `Link` header advertises a next page, per RFC 8288
/// (`<url>; rel="next", ...`).
pub fn link_next(link: &str) -> bool {
    link.split(',').any(|part| {
        let mut segments = part.split(';').map(str::trim);
        let _url = segments.next();
        segments.any(|param| param.eq_ignore_ascii_case(r#"rel="next""#))
    })
}

/// Entries kept before the least recently used one is dropped.
const MAX_ENTRIES: usize = 128;
/// Bodies above this are not cached; they are the reads least likely to
/// repeat and the ones that would dominate memory.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Total cached body and header bytes across all entries.
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
struct CachedResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
    etag: Option<String>,
    last_modified: Option<String>,
}

impl CachedResponse {
    fn bytes(&self) -> usize {
        self.body.len()
            + self
                .headers
                .iter()
                .map(|(name, value)| name.len() + value.len())
                .sum::<usize>()
            + self.etag.as_ref().map_or(0, String::len)
            + self.last_modified.as_ref().map_or(0, String::len)
    }
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, CachedResponse>,
    order: VecDeque<String>,
    bytes: usize,
}

impl Cache {
    fn get(&mut self, key: &str) -> Option<CachedResponse> {
        let entry = self.entries.get(key)?.clone();
        self.touch(key);
        Some(entry)
    }

    fn insert(&mut self, key: String, entry: CachedResponse) {
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes -= previous.bytes();
        }
        self.order.retain(|existing| existing != &key);
        self.bytes += entry.bytes();
        self.entries.insert(key.clone(), entry);
        self.order.push_back(key);
        self.evict();
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    fn touch(&mut self, key: &str) {
        if let Some(index) = self.order.iter().position(|existing| existing == key) {
            if let Some(entry) = self.order.remove(index) {
                self.order.push_back(entry);
            }
        }
    }

    fn evict(&mut self) {
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.bytes -= entry.bytes();
            }
        }
    }
}

/// Adds conditional GETs to any transport: a cached `ETag` or
/// `Last-Modified` becomes `If-None-Match` or `If-Modified-Since`, and a
/// `304` is replayed from the cached body. Any non-GET, successful or not,
/// clears the cache: a write whose outcome is unknown may still have landed,
/// so a GET issued afterwards revalidates instead of replaying old bytes.
pub struct ConditionalTransport<T> {
    inner: T,
    cache: Arc<Mutex<Cache>>,
}

impl<T: Transport> ConditionalTransport<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(Cache::default())),
        }
    }
}

impl<T: Transport> Transport for ConditionalTransport<T> {
    fn send(&self, request: Request) -> ForgeFuture<Response> {
        let cacheable = request.method == Method::Get;
        let key = cache_key(&request);
        let cached = if cacheable {
            lock(&self.cache).get(&key)
        } else {
            None
        };
        let mut request = request;
        if let Some(entry) = &cached {
            if let Some(etag) = &entry.etag {
                request
                    .headers
                    .push(("If-None-Match".to_owned(), etag.clone()));
            } else if let Some(modified) = &entry.last_modified {
                request
                    .headers
                    .push(("If-Modified-Since".to_owned(), modified.clone()));
            }
        }

        let pending = self.inner.send(request);
        let cache = Arc::clone(&self.cache);
        Box::pin(async move {
            let result = pending.await;
            if !cacheable {
                lock(&cache).clear();
                return result;
            }
            Ok(replay(&cache, key, cached, result?))
        })
    }
}

fn lock(cache: &Mutex<Cache>) -> MutexGuard<'_, Cache> {
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The cache key is the URL, query, and request headers. Headers carry the
/// access token, so two connections never read each other's cached body.
fn cache_key(request: &Request) -> String {
    let mut hasher = DefaultHasher::new();
    request.url.hash(&mut hasher);
    let mut query: Vec<&(String, String)> = request.query.iter().collect();
    query.sort();
    query.hash(&mut hasher);
    let mut headers: Vec<&(String, String)> = request.headers.iter().collect();
    headers.sort();
    headers.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn replay(
    cache: &Mutex<Cache>,
    key: String,
    cached: Option<CachedResponse>,
    response: Response,
) -> Response {
    if response.status == 304 {
        let Some(mut entry) = cached else {
            return response;
        };
        entry.headers = merge_headers(&entry.headers, &response.headers);
        if let Some(etag) = response.header("etag") {
            entry.etag = Some(etag.to_owned());
        }
        if let Some(modified) = response.header("last-modified") {
            entry.last_modified = Some(modified.to_owned());
        }
        let replayed = Response {
            status: entry.status,
            body: entry.body.clone(),
            headers: entry.headers.clone(),
        };
        lock(cache).insert(key, entry);
        return replayed;
    }

    let validator = response.header("etag").map(str::to_owned);
    let modified = response.header("last-modified").map(str::to_owned);
    let cacheable = (200..300).contains(&response.status)
        && response.body.len() <= MAX_BODY_BYTES
        && (validator.is_some() || modified.is_some())
        && !response.header("cache-control").is_some_and(no_store);
    if cacheable {
        lock(cache).insert(
            key,
            CachedResponse {
                status: response.status,
                body: response.body.clone(),
                headers: response.headers.clone(),
                etag: validator,
                last_modified: modified,
            },
        );
    }
    response
}

fn merge_headers(stored: &[(String, String)], fresh: &[(String, String)]) -> Vec<(String, String)> {
    let mut merged = stored.to_vec();
    for (name, value) in fresh {
        match merged
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            Some(slot) => slot.1.clone_from(value),
            None => merged.push((name.clone(), value.clone())),
        }
    }
    merged
}

fn no_store(cache_control: &str) -> bool {
    cache_control
        .split(',')
        .any(|directive| directive.trim().eq_ignore_ascii_case("no-store"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://api.example.test/repos/o/r/issues";

    struct Scripted {
        requests: Mutex<Vec<Request>>,
        responses: Mutex<VecDeque<Result<Response, ProviderError>>>,
    }

    impl Scripted {
        fn new(responses: Vec<Response>) -> Arc<Self> {
            Self::script(responses.into_iter().map(Ok).collect())
        }

        fn script(responses: Vec<Result<Response, ProviderError>>) -> Arc<Self> {
            Arc::new(Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into()),
            })
        }

        fn sent(&self) -> Vec<Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Transport for Scripted {
        fn send(&self, request: Request) -> ForgeFuture<Response> {
            self.requests.lock().unwrap().push(request);
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("scripted response");
            Box::pin(async move { response })
        }
    }

    fn authed_get(url: &str, token: &str) -> Request {
        Request::new(Method::Get, url).header("Authorization", format!("Bearer {token}"))
    }

    fn etag(tag: &str) -> Response {
        Response::new(200, "first").with_header("ETag", tag)
    }

    #[tokio::test]
    async fn replays_the_cached_body_when_the_server_answers_304() {
        let scripted = Scripted::new(vec![etag("\"v1\""), Response::new(304, "")]);
        let transport = ConditionalTransport::new(scripted.clone());

        let first = transport.send(authed_get(URL, "tok")).await.unwrap();
        assert_eq!((first.status, first.body.as_str()), (200, "first"));

        let second = transport.send(authed_get(URL, "tok")).await.unwrap();
        assert_eq!((second.status, second.body.as_str()), (200, "first"));

        let sent = scripted.sent();
        assert_eq!(sent[0].header_value("if-none-match"), None);
        assert_eq!(sent[1].header_value("if-none-match"), Some("\"v1\""));
    }

    #[tokio::test]
    async fn refreshes_the_validator_from_the_304() {
        let scripted = Scripted::new(vec![
            etag("\"v1\""),
            Response::new(304, "").with_header("ETag", "\"v2\""),
            etag("\"v2\""),
        ]);
        let transport = ConditionalTransport::new(scripted.clone());

        for _ in 0..3 {
            transport.send(authed_get(URL, "tok")).await.unwrap();
        }

        let sent = scripted.sent();
        assert_eq!(sent[1].header_value("if-none-match"), Some("\"v1\""));
        assert_eq!(sent[2].header_value("if-none-match"), Some("\"v2\""));
    }

    #[tokio::test]
    async fn never_shares_an_entry_between_credentials() {
        let scripted = Scripted::new(vec![etag("\"v1\""), etag("\"v1\"")]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "one")).await.unwrap();
        transport.send(authed_get(URL, "two")).await.unwrap();

        assert_eq!(scripted.sent()[1].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn a_write_clears_the_cache() {
        let scripted = Scripted::new(vec![etag("\"v1\""), Response::new(200, ""), etag("\"v1\"")]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        transport
            .send(Request::new(Method::Post, URL))
            .await
            .unwrap();
        transport.send(authed_get(URL, "tok")).await.unwrap();

        let sent = scripted.sent();
        assert_eq!(sent[1].header_value("if-none-match"), None);
        assert_eq!(sent[2].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn a_failed_write_clears_the_cache() {
        let scripted = Scripted::script(vec![
            Ok(etag("\"v1\"")),
            Err(ProviderError::Network {
                message: "connection reset".into(),
            }),
            Ok(etag("\"v1\"")),
        ]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        assert!(transport
            .send(Request::new(Method::Post, URL))
            .await
            .is_err());
        transport.send(authed_get(URL, "tok")).await.unwrap();

        assert_eq!(scripted.sent()[2].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn responses_without_a_validator_are_not_stored() {
        let scripted = Scripted::new(vec![Response::new(200, "one"), Response::new(200, "two")]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        let second = transport.send(authed_get(URL, "tok")).await.unwrap();

        assert_eq!(second.body, "two");
        assert_eq!(scripted.sent()[1].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn falls_back_to_last_modified() {
        let scripted = Scripted::new(vec![
            Response::new(200, "first")
                .with_header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
            Response::new(304, ""),
        ]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        let second = transport.send(authed_get(URL, "tok")).await.unwrap();

        assert_eq!(second.body, "first");
        let sent = scripted.sent();
        assert_eq!(sent[1].header_value("if-none-match"), None);
        assert_eq!(
            sent[1].header_value("if-modified-since"),
            Some("Wed, 21 Oct 2015 07:28:00 GMT")
        );
    }

    #[tokio::test]
    async fn keys_entries_by_query() {
        let scripted = Scripted::new(vec![etag("\"v1\""), etag("\"v1\"")]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport
            .send(authed_get(URL, "tok").with_query(vec![("page".into(), "1".into())]))
            .await
            .unwrap();
        transport
            .send(authed_get(URL, "tok").with_query(vec![("page".into(), "2".into())]))
            .await
            .unwrap();

        assert_eq!(scripted.sent()[1].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn keeps_cached_headers_the_304_omits() {
        let scripted = Scripted::new(vec![
            etag("\"v1\"").with_header("Link", r#"<https://api.example.test/p2>; rel="next""#),
            Response::new(304, ""),
        ]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        let replayed = transport.send(authed_get(URL, "tok")).await.unwrap();

        assert!(replayed.has_next_page());
    }

    #[tokio::test]
    async fn honours_no_store() {
        let scripted = Scripted::new(vec![
            etag("\"v1\"").with_header("Cache-Control", "private, no-store"),
            etag("\"v1\""),
        ]);
        let transport = ConditionalTransport::new(scripted.clone());

        transport.send(authed_get(URL, "tok")).await.unwrap();
        transport.send(authed_get(URL, "tok")).await.unwrap();

        assert_eq!(scripted.sent()[1].header_value("if-none-match"), None);
    }

    #[tokio::test]
    async fn evicts_the_least_recently_used_entry() {
        let mut scripted_responses: Vec<Response> = (0..MAX_ENTRIES)
            .map(|index| Response::new(200, "x").with_header("ETag", format!("\"v{index}\"")))
            .collect();
        scripted_responses.push(Response::new(304, ""));
        scripted_responses.push(Response::new(200, "new").with_header("ETag", "\"new\""));
        scripted_responses.push(Response::new(200, "again"));
        scripted_responses.push(Response::new(304, ""));
        let scripted = Scripted::new(scripted_responses);
        let transport = ConditionalTransport::new(scripted.clone());

        for index in 0..MAX_ENTRIES {
            let url = format!("{URL}/{index}");
            transport.send(authed_get(&url, "tok")).await.unwrap();
        }
        let first = format!("{URL}/0");
        transport.send(authed_get(&first, "tok")).await.unwrap();
        transport
            .send(authed_get(&format!("{URL}/new"), "tok"))
            .await
            .unwrap();
        transport
            .send(authed_get(&format!("{URL}/1"), "tok"))
            .await
            .unwrap();
        transport.send(authed_get(&first, "tok")).await.unwrap();

        let sent = scripted.sent();
        assert_eq!(
            sent[MAX_ENTRIES].header_value("if-none-match"),
            Some("\"v0\"")
        );
        assert_eq!(sent[MAX_ENTRIES + 2].header_value("if-none-match"), None);
        assert_eq!(
            sent[MAX_ENTRIES + 3].header_value("if-none-match"),
            Some("\"v0\"")
        );
    }

    #[test]
    fn detects_next_page_from_link_header() {
        assert!(!link_next(""));
        assert!(link_next(
            r#"<https://api.example.test/p2>; rel="next", <https://api.example.test/p5>; rel="last""#
        ));
        assert!(!link_next(
            r#"<https://api.example.test/p1>; rel="prev", <https://api.example.test/p1>; rel="first""#
        ));
    }

    #[test]
    fn reads_location_and_link_headers_case_insensitively() {
        let response = Response::new(302, "")
            .with_header("Location", "https://blob.example/log.txt")
            .with_header("Link", r#"<https://api.example.test/p2>; rel="next""#);
        assert_eq!(response.location(), Some("https://blob.example/log.txt"));
        assert!(response.has_next_page());
        assert_eq!(response.header("location"), response.location());
    }
}
