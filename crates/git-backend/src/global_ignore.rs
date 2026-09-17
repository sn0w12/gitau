use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::error::{GitError, Result};

/// Merged excludes file name inside the app-managed `gitignore` directory.
const MERGED_FILE_NAME: &str = "global-excludes";

/// Process-wide merged excludes file. Every gix handle applies it as an
/// in-memory `core.excludesFile` override, so fresh sessions and long-lived
/// ones observe setting changes without being recreated.
static GLOBAL_EXCLUDES: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Warn-once guard for override failures: the merged path is app-controlled
/// and practically always representable, so a failure is a persistent
/// condition not worth repeating on every handle.
static OVERRIDE_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_global_excludes_file(path: Option<PathBuf>) {
    *GLOBAL_EXCLUDES.write().unwrap() = path;
    OVERRIDE_WARNED.store(false, std::sync::atomic::Ordering::Release);
}

pub(crate) fn snapshot() -> Option<PathBuf> {
    GLOBAL_EXCLUDES.read().unwrap().clone()
}

/// Applies the process-wide excludes override to a single gix handle.
/// In-memory only: nothing is written to any repository config, and handles
/// opened while no override is set are untouched.
pub(crate) fn apply_to_repo(repo: &mut gix::Repository) -> Result<()> {
    let Some(path) = snapshot() else {
        return Ok(());
    };
    let value = format!("core.excludesFile={}", path.display());
    let mut snapshot = repo.config_snapshot_mut();
    snapshot
        .append_config([value], gix::config::Source::Api)
        .map_err(|error| GitError::internal(error.to_string()))?;
    snapshot
        .commit()
        .map_err(|error| GitError::internal(error.to_string()))?;
    Ok(())
}

pub(crate) fn warn_once(message: String) {
    if !OVERRIDE_WARNED.swap(true, std::sync::atomic::Ordering::AcqRel) {
        log::warn!("{message}");
    }
}

/// Locates the user's git system excludes file: an explicit
/// `core.excludesFile` first, otherwise git's default location. The file
/// itself may not exist; callers treat a missing file as empty.
pub fn resolve_system_excludes_file() -> Option<PathBuf> {
    if let Ok(config) = git2::Config::open_default()
        && let Ok(raw) = config.get_str("core.excludesFile")
    {
        return Some(expand_home(Path::new(raw)));
    }
    config_home().map(|home| home.join("git").join("ignore"))
}

/// Rebuilds the merged excludes file from the system file plus the setting
/// content. Returns `None` when both sides are empty, in which case the
/// engines keep their default ignore behavior. The write is skipped when
/// the on-disk content already matches.
pub fn ensure_merged_excludes_file(dir: &Path, app_content: &str) -> Result<Option<PathBuf>> {
    let system_content = resolve_system_excludes_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let Some(merged) = merge_contents(&system_content, app_content) else {
        let _ = std::fs::remove_file(dir.join(MERGED_FILE_NAME));
        return Ok(None);
    };
    let path = dir.join(MERGED_FILE_NAME);
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current != merged {
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, merged)?;
    }
    Ok(Some(path))
}

/// Combines both halves into one excludes file. The system half comes first
/// so app-managed rules refine it; within one file later lines (including
/// `!` negations) win over earlier ones.
fn merge_contents(system_content: &str, app_content: &str) -> Option<String> {
    let system = normalize(system_content);
    let app = normalize(app_content);
    match (system.trim().is_empty(), app.trim().is_empty()) {
        (true, true) => None,
        (false, true) => Some(format!("{system}\n")),
        (true, false) => Some(format!("# gitau global gitignore\n{app}\n")),
        (false, false) => Some(format!("{system}\n# gitau global gitignore\n{app}\n")),
    }
}

fn normalize(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_matches('\n')
        .to_owned()
}

fn expand_home(path: &Path) -> PathBuf {
    let mut components = path.components();
    let is_home =
        matches!(components.next(), Some(std::path::Component::Normal(first)) if first == "~");
    if !is_home {
        return path.to_path_buf();
    }
    let Some(home) = home_dir() else {
        return path.to_path_buf();
    };
    let mut expanded = home;
    expanded.extend(components);
    expanded
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(dir);
        if dir.is_absolute() {
            return Some(dir);
        }
    }
    home_dir().map(|home| home.join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_both_sides_means_no_override() {
        assert_eq!(merge_contents("", ""), None);
        assert_eq!(merge_contents("\n", "  \n"), None);
    }

    #[test]
    fn system_only_passes_through() {
        assert_eq!(merge_contents("*.log\n", ""), Some("*.log\n".to_owned()));
    }

    #[test]
    fn app_only_gets_a_marker_header() {
        assert_eq!(
            merge_contents("", "*.tmp\n"),
            Some("# gitau global gitignore\n*.tmp\n".to_owned())
        );
    }

    #[test]
    fn system_comes_first_so_app_rules_refine_it() {
        assert_eq!(
            merge_contents("*.log\n", "keep.log\n!keep.log\n"),
            Some("*.log\n# gitau global gitignore\nkeep.log\n!keep.log\n".to_owned())
        );
    }

    #[test]
    fn crlf_is_normalized() {
        assert_eq!(
            merge_contents("*.log\r\n", "*.tmp\r\n"),
            Some("*.log\n# gitau global gitignore\n*.tmp\n".to_owned())
        );
    }

    #[test]
    fn ensure_skips_rewrite_when_content_matches() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_merged_excludes_file(dir.path(), "*.tmp\n")
            .unwrap()
            .expect("merged file");
        let before = std::fs::metadata(&first).unwrap().modified().unwrap();
        let second = ensure_merged_excludes_file(dir.path(), "*.tmp\n")
            .unwrap()
            .expect("merged file");
        assert_eq!(first, second);
        let after = std::fs::metadata(&second).unwrap().modified().unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn ensure_returns_none_and_clears_stale_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = ensure_merged_excludes_file(dir.path(), "*.tmp\n")
            .unwrap()
            .expect("merged file");
        assert!(path.exists());
        assert_eq!(ensure_merged_excludes_file(dir.path(), "").unwrap(), None);
        assert!(!path.exists());
    }
}
