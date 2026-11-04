use fs_util::sanitize_component;

use crate::error::IconError;
use crate::source::OwnerIconSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPlan {
    /// `{host}/{owner}` with host lowercased; both parts sanitized.
    pub key: String,
    pub url: String,
}

/// Derives the cache key + remote URL for a git remote string. Supports
/// `https://`, `http://`, and scp-like `git@host:owner/repo(.git)` forms.
/// Returns `None` when no provider supplies an icon URL for the host.
pub fn plan_from_remote(
    remote: &str,
    source: &dyn OwnerIconSource,
) -> Result<Option<IconPlan>, IconError> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let (host, owner) = split_remote(trimmed)?;

    let host = host.to_ascii_lowercase();
    let owner = owner.to_ascii_lowercase();
    let host_key = sanitize_component(&host);
    let owner_key = sanitize_component(&owner);
    if host_key.is_empty() || owner_key.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let Some(url) = source.icon_url(&host, &owner) else {
        return Ok(None);
    };

    Ok(Some(IconPlan {
        key: format!("{host_key}/{owner_key}"),
        url,
    }))
}

/// Rebuilds a plan from a cached `{host}/{owner}` key and the current
/// provider URL, so refresh uses the provider's latest endpoint.
pub(crate) fn plan_from_key(key: &str, source: &dyn OwnerIconSource) -> Option<IconPlan> {
    let (host, owner) = split_key(key);
    let url = source.icon_url(host, owner)?;
    Some(IconPlan {
        key: key.to_owned(),
        url,
    })
}

fn split_remote(remote: &str) -> Result<(&str, &str), IconError> {
    let without_git_suffix = remote.strip_suffix(".git").unwrap_or(remote);

    // Split off scheme if present.
    let (scheme, rest) = match without_git_suffix.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), without_git_suffix),
    };

    let (host, path) = if !scheme.is_empty() {
        if scheme != "https" && scheme != "http" {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        rest.split_once(['/'])
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?
    } else {
        // scp-like syntax: git@github.com:owner/repo
        if !remote.contains('@') || !without_git_suffix.contains(':') {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        let after_user = without_git_suffix.split_once('@').unwrap().1;
        after_user
            .split_once(':')
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?
    };

    let owner = path
        .split('/')
        .find(|segment| !segment.is_empty())
        .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?;

    Ok((host, owner))
}

pub(crate) fn split_key(key: &str) -> (&str, &str) {
    match key.split_once('/') {
        Some((host, owner)) => (host, owner),
        None => ("x", key),
    }
}

/// Per-entry file name: the sanitized owner part is unique within its
/// per-host directory, so it alone names the files.
pub(crate) fn entry_file_name(plan: &IconPlan) -> String {
    file_name_of_key(&plan.key)
}

pub(crate) fn file_name_of_key(key: &str) -> String {
    let (_, owner) = split_key(key);
    owner.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource;

    impl OwnerIconSource for FakeSource {
        fn icon_url(&self, host: &str, owner: &str) -> Option<String> {
            (host == "github.com").then(|| format!("https://github.com/{owner}.png?size=128"))
        }
    }

    #[test]
    fn plans_github_https_remotes() {
        let plan = plan_from_remote("https://github.com/Facebook/react.git", &FakeSource)
            .unwrap()
            .unwrap();
        assert_eq!(plan.key, "github.com/facebook");
        assert_eq!(plan.url, "https://github.com/facebook.png?size=128");
    }

    #[test]
    fn plans_handle_suffixes_and_slashes() {
        let plan = plan_from_remote("https://github.com/rust-lang/rust/", &FakeSource)
            .unwrap()
            .unwrap();
        assert_eq!(plan.key, "github.com/rust-lang");
    }

    #[test]
    fn plans_scp_like_remotes() {
        let plan = plan_from_remote("git@github.com:tokio-rs/tokio.git", &FakeSource)
            .unwrap()
            .unwrap();
        assert_eq!(plan.key, "github.com/tokio-rs");
        assert_eq!(plan.url, "https://github.com/tokio-rs.png?size=128");
    }

    #[test]
    fn hosts_without_a_provider_have_no_plan() {
        for remote in [
            "https://gitlab.com/gitlab-org/gitlab.git",
            "https://bitbucket.org/atlassian/atlaskit.git",
            "https://git.example.org/team/project.git",
        ] {
            assert_eq!(plan_from_remote(remote, &FakeSource).unwrap(), None);
        }
    }

    #[test]
    fn rejects_invalid_remotes() {
        for bad in ["", "not-a-url", "ftp://host/x/y", "https://", "git@host"] {
            assert!(
                plan_from_remote(bad, &FakeSource).is_err(),
                "`{bad}` should not plan"
            );
        }
    }

    #[test]
    fn sanitized_keys_cannot_escape_the_cache_dir() {
        let evil = sanitize_component("../../evil");
        assert!(!evil.contains('/'));
        assert_ne!(evil, "..");

        let plan = plan_from_remote("https://github.com/../etc/passwd", &FakeSource)
            .unwrap()
            .unwrap();
        assert!(plan.key.starts_with("github.com/"));
        assert!(!plan.key.contains(".."));
    }
}
