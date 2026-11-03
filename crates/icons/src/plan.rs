use fs_util::sanitize_component;

use crate::error::IconError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPlan {
    /// `{host}/{owner}` with host lowercased; both parts sanitized.
    pub key: String,
    pub url: String,
}

/// Derives the cache key + remote URL for a git remote string. Supports
/// `https://`, `http://`, and scp-like `git@host:owner/repo(.git)` forms.
pub fn plan_from_remote(remote: &str) -> Result<IconPlan, IconError> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let without_git_suffix = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    // Split off scheme if present.
    let (scheme, rest) = match without_git_suffix.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), without_git_suffix),
    };

    let (host, path) = if !scheme.is_empty() {
        if scheme != "https" && scheme != "http" {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        let (host, path) = rest
            .split_once(['/'])
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?;
        (host, path)
    } else {
        // scp-like syntax: git@github.com:owner/repo
        if !trimmed.contains('@') || !without_git_suffix.contains(':') {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        let after_user = without_git_suffix.split_once('@').unwrap().1;
        after_user
            .split_once(':')
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?
    };

    let host = sanitize_component(&host.to_ascii_lowercase());
    let owner = path
        .split('/')
        .find(|segment| !segment.is_empty())
        .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?;
    // Owners are case-insensitively unique on the providers we special-case,
    // so normalize to keep one cache entry per owner.
    let owner = sanitize_component(&owner.to_lowercase());
    if host.is_empty() || owner.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let url = avatar_url_for(&host, &owner);

    Ok(IconPlan {
        key: format!("{host}/{owner}"),
        url,
    })
}

/// Provider-specific owner/org avatar endpoints. Anything unrecognized
/// falls back to the site favicon.
fn avatar_url_for(host: &str, owner: &str) -> String {
    let is_github = host == "github.com" || host.ends_with(".github.com");
    let is_gitlab =
        host == "gitlab.com" || host.contains(".gitlab.") || host.starts_with("gitlab.");
    let is_bitbucket = host == "bitbucket.org";

    if is_github {
        // Works for users and orgs; redirects to the CDN avatar.
        format!("https://{host}/{owner}.png?size=128")
    } else if is_gitlab {
        // Serves the namespace's avatar, or an identicon when none exists.
        format!("https://{host}/{owner}.png")
    } else if is_bitbucket {
        format!("https://bitbucket.org/account/{owner}/avatar/128/")
    } else {
        format!("https://{host}/favicon.ico")
    }
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

    #[test]
    fn plans_github_https_remotes() {
        let plan = plan_from_remote("https://github.com/Facebook/react.git").unwrap();
        assert_eq!(plan.key, "github.com/facebook");
        assert_eq!(plan.url, "https://github.com/facebook.png?size=128");
    }

    #[test]
    fn plans_handle_suffixes_and_slashes() {
        let plan = plan_from_remote("https://github.com/rust-lang/rust/").unwrap();
        assert_eq!(plan.key, "github.com/rust-lang");
    }

    #[test]
    fn plans_scp_like_remotes() {
        let plan = plan_from_remote("git@github.com:tokio-rs/tokio.git").unwrap();
        assert_eq!(plan.key, "github.com/tokio-rs");
        assert_eq!(plan.url, "https://github.com/tokio-rs.png?size=128");
    }

    #[test]
    fn supports_gitlab_and_bitbucket() {
        let gitlab = plan_from_remote("https://gitlab.com/gitlab-org/gitlab.git").unwrap();
        assert_eq!(gitlab.key, "gitlab.com/gitlab-org");
        assert_eq!(gitlab.url, "https://gitlab.com/gitlab-org.png");

        let scp = plan_from_remote("git@gitlab.com:kernel-team/linux.git").unwrap();
        assert_eq!(scp.url, "https://gitlab.com/kernel-team.png");

        let self_hosted = plan_from_remote("https://gitlab.company.dev/team/app.git").unwrap();
        assert_eq!(self_hosted.url, "https://gitlab.company.dev/team.png");

        let bitbucket = plan_from_remote("https://bitbucket.org/atlassian/atlaskit.git").unwrap();
        assert_eq!(
            bitbucket.url,
            "https://bitbucket.org/account/atlassian/avatar/128/"
        );
    }

    #[test]
    fn unknown_hosts_fall_back_to_favicon() {
        let plan = plan_from_remote("https://git.example.org/team/project.git").unwrap();
        assert_eq!(plan.key, "git.example.org/team");
        assert_eq!(plan.url, "https://git.example.org/favicon.ico");
    }

    #[test]
    fn rejects_invalid_remotes() {
        for bad in ["", "not-a-url", "ftp://host/x/y", "https://", "git@host"] {
            assert!(plan_from_remote(bad).is_err(), "`{bad}` should not plan");
        }
    }

    #[test]
    fn sanitized_keys_cannot_escape_the_cache_dir() {
        let evil = sanitize_component("../../evil");
        assert!(!evil.contains('/'));
        assert_ne!(evil, "..");

        let plan = plan_from_remote("https://github.com/../etc/passwd").unwrap();
        assert!(plan.key.starts_with("github.com/"));
        assert!(!plan.key.contains(".."));
    }
}
