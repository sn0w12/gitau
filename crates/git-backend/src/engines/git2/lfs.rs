use std::path::PathBuf;

use crate::api::lfs::{LfsFileInfo, LfsStatus};
use crate::error::{GitError, Result};

/// Git LFS handling is local-only: no engine in this stack (libgit2 or gix)
/// implements the LFS transfer API, so pointers are not fetched over the
/// wire. What we provide is visibility plus smudging files whose objects
/// are already present in the repository's local `lfs/objects` store.
pub fn lfs_status(repo: &git2::Repository) -> Result<LfsStatus> {
    let patterns = tracked_lfs_patterns(repo)?;
    if patterns.is_empty() {
        return Ok(LfsStatus {
            total: 0,
            materialized: 0,
            files: Vec::new(),
        });
    }

    let mut files = Vec::new();
    let tree = head_tree(repo)?;
    walk_tree(repo, &tree, "", &patterns, &mut files)?;

    let materialized = files.iter().filter(|f| f.materialized).count();
    Ok(LfsStatus {
        total: files.len(),
        materialized,
        files,
    })
}

pub fn lfs_smudge(repo: &git2::Repository) -> Result<usize> {
    let status = lfs_status(repo)?;
    let mut written = 0;
    for file in status.files {
        if file.materialized {
            continue;
        }
        let Some(store_path) = store_path(repo, &file.oid) else {
            continue;
        };
        if !store_path.exists() {
            continue;
        }
        let Some(workdir) = repo.workdir() else {
            continue;
        };
        let target = workdir.join(file.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = target.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                continue;
            }
        }
        if std::fs::copy(&store_path, &target).is_ok() {
            written += 1;
        }
    }
    Ok(written)
}

fn walk_tree(
    repo: &git2::Repository,
    tree: &git2::Tree<'_>,
    prefix: &str,
    patterns: &[Glob],
    out: &mut Vec<LfsFileInfo>,
) -> Result<()> {
    for entry in tree.iter() {
        let name = String::from_utf8_lossy(entry.name_bytes()).into_owned();
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let obj = entry.to_object(repo)?;
        if entry.kind() == Some(git2::ObjectType::Tree) {
            let subtree = obj.peel_to_tree()?;
            walk_tree(repo, &subtree, &path, patterns, out)?;
        } else if patterns.iter().any(|g| g.matches(&path)) {
            if let Some(info) = classify(repo, &path, &obj) {
                out.push(info);
            }
        }
    }
    Ok(())
}

fn classify(repo: &git2::Repository, path: &str, obj: &git2::Object<'_>) -> Option<LfsFileInfo> {
    // Only blobs can be LFS pointers.
    let blob = obj.peel_to_blob().ok()?;

    // The worktree file: real content if materialized, pointer text if not.
    let workdir = repo.workdir()?;
    let work_path = workdir.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));

    let materialized = std::fs::read(&work_path)
        .ok()
        .map(|bytes| pointer_oid_size(&bytes).is_none())
        .unwrap_or(false);

    let (oid, size) = pointer_oid_size(blob.content())?;
    let in_store = store_path(repo, &oid).map(|p| p.exists()).unwrap_or(false);
    Some(LfsFileInfo {
        path: path.to_owned(),
        oid,
        size,
        materialized,
        in_store,
    })
}

/// Parses an LFS pointer file (`version ...` + `oid sha256:<hex>` +
/// `size <n>`) into its oid and size.
fn pointer_oid_size(bytes: &[u8]) -> Option<(String, u64)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut oid: Option<String> = None;
    let mut size: Option<u64> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("oid sha256:") {
            oid = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("size ") {
            size = rest.trim().parse().ok();
        }
    }
    let oid = oid?;
    if oid.len() != 64 {
        return None;
    }
    let size = size?;
    if !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((oid, size))
}

fn store_path(repo: &git2::Repository, oid: &str) -> Option<PathBuf> {
    if oid.len() != 64 {
        return None;
    }
    let base = repo.path().join("lfs").join("objects");
    Some(base.join(&oid[..2]).join(&oid[..4]).join(oid))
}

/// Shorthand glob: `**` crosses `/`, `*` within a segment, `?` one char.
struct Glob {
    segments: Vec<String>,
}

impl Glob {
    fn matches(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        if self.segments.len() == 1 && !self.segments[0].contains("**") {
            // A bare pattern like `*.bin` matches at any depth.
            return parts
                .iter()
                .any(|part| segment_matches(part, &self.segments[0]));
        }
        glob_cross(&parts, &self.segments)
    }
}

fn segment_matches(name: &str, pattern: &str) -> bool {
    simple_glob(name, pattern)
}

fn simple_glob(name: &str, pattern: &str) -> bool {
    let n = name.as_bytes();
    let p = pattern.as_bytes();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star_i, mut star_j) = (usize::MAX, 0usize);
    while i < n.len() {
        if j < p.len() && (p[j] == b'?' || p[j] == n[i]) {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == b'*' {
            star_i = i;
            star_j = j;
            j += 1;
        } else if star_i != usize::MAX {
            i += 1;
            j = star_j + 1;
            star_i = i;
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == b'*' {
        j += 1;
    }
    j == p.len()
}

/// Matches segments allowing `**` to span zero or more directories.
fn glob_cross(parts: &[&str], segments: &[String]) -> bool {
    fn rec(parts: &[&str], segments: &[String], pi: usize, si: usize) -> bool {
        if si == segments.len() {
            return pi == parts.len();
        }
        if segments[si] == "**" {
            // Try consuming zero or more path parts.
            for take in 0..=(parts.len() - pi) {
                if rec(parts, segments, pi + take, si + 1) {
                    return true;
                }
            }
            return false;
        }
        if pi >= parts.len() {
            return false;
        }
        if simple_glob(parts[pi], &segments[si]) {
            rec(parts, segments, pi + 1, si + 1)
        } else {
            false
        }
    }
    rec(parts, segments, 0, 0)
}

/// Parses `.gitattributes` for `filter=lfs` patterns.
fn tracked_lfs_patterns(repo: &git2::Repository) -> Result<Vec<Glob>> {
    let Some(workdir) = repo.workdir() else {
        return Ok(Vec::new());
    };
    let Ok(content) = std::fs::read_to_string(workdir.join(".gitattributes")) else {
        return Ok(Vec::new());
    };
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let pattern = parts.next()?;
            if parts.any(|attr| attr == "filter=lfs") {
                let segments = pattern.split('/').map(|s| s.to_owned()).collect::<Vec<_>>();
                Some(Glob { segments })
            } else {
                None
            }
        })
        .collect())
}

fn head_tree(repo: &git2::Repository) -> Result<git2::Tree<'_>> {
    let head = repo
        .head()
        .map_err(|_| GitError::invalid_input("repository has no commits yet"))?;
    Ok(head.peel_to_commit()?.tree()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_oid_parsed() {
        let bytes = b"version https://git-lfs.github.com/spec/v1\r\noid sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\r\nsize 12345\r\n";
        let (oid, size) = pointer_oid_size(bytes).unwrap();
        assert_eq!(oid.len(), 64);
        assert_eq!(size, 12345);
    }

    #[test]
    fn non_pointer_is_none() {
        assert!(pointer_oid_size(b"hello world").is_none());
    }

    #[test]
    fn glob_matches() {
        let g = Glob {
            segments: vec!["**".into(), "*.bin".into()],
        };
        assert!(g.matches("a/b/thing.bin"));
        assert!(g.matches("thing.bin"));
        assert!(!g.matches("a/b/thing.png"));
    }
}
