use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use notify::Watcher;

use crate::error::Result;

/// Coalescing filesystem watcher. Events arriving inside the debounce window
/// collapse into a single callback invocation: the callback fires once the
/// channel stays quiet for a full window, so bursts (a directory tree being
/// rewritten, an editor saving many files) map to one `on_change`.
///
/// Events under the `is_ignored` predicate are dropped entirely. The
/// repository registers a predicate covering its git dir and gitignored
/// paths, so the app's own writes and build-tool churn (`target/`,
/// `node_modules/`) never re-bump the generation.
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _watcher: notify::RecommendedWatcher,
}

impl WatcherHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn spawn_watch(
    paths: &[PathBuf],
    debounce: Duration,
    is_ignored: impl Fn(&PathBuf) -> bool + Send + Sync + 'static,
    on_change: impl Fn() + Send + 'static,
) -> Result<WatcherHandle> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let tx_events = tx.clone();
    let mut watcher = notify::recommended_watcher(
        move |event: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = event {
                let relevant = !matches!(
                    event.kind,
                    notify::EventKind::Access(_) | notify::EventKind::Other
                );
                if relevant {
                    if let Some(path) = event.paths.first() {
                        let _ = tx_events.send(path.clone());
                    }
                }
            }
        },
    )
    .map_err(|e| crate::error::GitError::internal(format!("watcher init failed: {e}")))?;

    for path in paths {
        watcher
            .watch(path, notify::RecursiveMode::Recursive)
            .map_err(|e| {
                crate::error::GitError::internal(format!("watch failed for {path:?}: {e}"))
            })?;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let thread = std::thread::Builder::new()
        .name("git-backend-watch".into())
        .spawn(move || {
            let mut burst: Vec<PathBuf> = Vec::new();
            loop {
                if stop_clone.load(Ordering::Acquire) {
                    return;
                }
                // Wait for the first event of a burst.
                match rx.recv_timeout(debounce) {
                    Ok(path) => {
                        burst.push(path);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
                // Keep draining until the channel stays quiet for a full
                // window, then fire once for the whole burst.
                loop {
                    if stop_clone.load(Ordering::Acquire) {
                        return;
                    }
                    match rx.recv_timeout(debounce) {
                        Ok(path) => {
                            burst.push(path);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                let relevant = !burst.iter().all(&is_ignored);
                burst.clear();
                if relevant {
                    on_change();
                }
            }
        })
        .map_err(|e| crate::error::GitError::internal(format!("watch thread failed: {e}")))?;

    Ok(WatcherHandle {
        stop,
        thread: Some(thread),
        _watcher: watcher,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    #[test]
    fn coalesces_bursts_and_stops_cleanly() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("probe.txt");
        std::fs::write(&file_path, b"0").unwrap();

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_cb = hits.clone();
        let handle = spawn_watch(
            &[temp.path().to_path_buf()],
            Duration::from_millis(250),
            |_| false,
            move || {
                hits_cb.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();

        // No sleeps between writes: a scheduling gap longer than the
        // debounce window would split the burst into separate callbacks and
        // flake the assertion below.
        for i in 1..=5u8 {
            std::fs::write(&file_path, [i]).unwrap();
        }

        let deadline = Instant::now() + Duration::from_secs(3);
        while hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1);
        // The burst must collapse: 5 writes inside the window is one hit.
        // Wait past a full debounce window so any late second burst shows up.
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "burst must coalesce into a single callback"
        );

        handle.stop();
    }

    #[test]
    fn ignores_events_the_predicate_rejects() {
        let temp = tempfile::tempdir().unwrap();
        let ignored_dir = temp.path().join("target");
        std::fs::create_dir_all(&ignored_dir).unwrap();
        let ignored_file = ignored_dir.join("artifact.o");
        let tracked_file = temp.path().join("tracked.txt");
        std::fs::write(&ignored_file, b"0").unwrap();
        std::fs::write(&tracked_file, b"0").unwrap();

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_cb = hits.clone();
        let handle = spawn_watch(
            &[temp.path().to_path_buf()],
            Duration::from_millis(60),
            move |path| path.starts_with(&ignored_dir),
            move || {
                hits_cb.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();

        // Writes under the ignored prefix must not fire the callback.
        std::fs::write(&ignored_file, b"1").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        // A write to a real worktree file still does.
        std::fs::write(&tracked_file, b"1").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1);

        handle.stop();
    }
}
