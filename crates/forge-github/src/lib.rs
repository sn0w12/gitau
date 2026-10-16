//! GitHub forge provider.
//!
//! Wraps the GitHub REST surface in the provider-neutral
//! [`forge_core::ForgeProvider`] trait. Everything GitHub-specific (hosts,
//! device-flow endpoints, URL shapes, OAuth scopes) lives here; the rest of
//! the app talks only to the trait.

mod device_flow;
mod http;
mod job_log;
mod map;
mod provider;
mod wire;

use std::sync::Arc;
use std::time::Duration;

use http::{HttpTransport, Transport};

pub use job_log::split_job_log_for_test;

const HOST: &str = "github.com";
const OAUTH_SCOPES: &str = "repo read:user workflow notifications";
const BUILTIN_CLIENT_ID: &str = "Ov23liIkWKR8TopkqSKP";

/// Development override so the app can run against a personal OAuth App
/// without rebuilding.
fn resolve_client_id() -> String {
    std::env::var("GITAU_GITHUB_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| BUILTIN_CLIENT_ID.to_owned())
}

pub struct GithubProvider {
    transport: Arc<dyn Transport>,
}

impl GithubProvider {
    pub fn new(timeout: Duration) -> Self {
        Self {
            transport: Arc::new(HttpTransport::new(timeout)),
        }
    }

    /// Provider over a caller-supplied transport, so tests can drive the
    /// request shapes without a network.
    #[cfg(test)]
    pub(crate) fn with_transport(transport: Arc<dyn Transport>) -> Self {
        Self { transport }
    }
}

impl Default for GithubProvider {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

/// Parses `owner/repo` out of a GitHub remote. Accepts `https://`,
/// `http://`, `git://`, `ssh://git@`, and scp-style `git@host:owner/repo`.
/// Returns `None` for any other host or shape.
fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let rest = ["https://", "http://", "git://", "ssh://"]
        .iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .unwrap_or(url);
    let rest = rest.strip_prefix("git@").unwrap_or(rest);
    let (host, path) = rest.split_once(['/', ':'])?;
    let host = host.trim_start_matches("www.");
    if !host.eq_ignore_ascii_case(HOST) {
        return None;
    }
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some((owner.to_ascii_lowercase(), repo.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes() {
        for url in [
            "https://github.com/Facebook/React.git",
            "http://github.com/facebook/react",
            "git@github.com:facebook/react.git",
            "ssh://git@github.com:facebook/react.git",
            "github.com/facebook/react",
        ] {
            assert_eq!(
                parse_github_remote(url),
                Some(("facebook".to_owned(), "react".to_owned())),
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_other_hosts_and_shapes() {
        assert_eq!(parse_github_remote("https://gitlab.com/o/r"), None);
        assert_eq!(parse_github_remote("https://github.com/just-owner"), None);
        assert_eq!(parse_github_remote(""), None);
    }
}
