use std::sync::{Mutex, MutexGuard};

static GATE: Mutex<()> = Mutex::new(());

/// Serializes every in-process writer of a git index.
///
/// `index.lock` is the only arbiter across processes, so a writer either takes
/// it or retries. Retrying alone loses: the status engine holds the lock
/// across a re-read and rewrite of the whole file, measured at up to 25 ms on
/// a loaded machine, so a writer polling every 5 ms keeps landing in another
/// job's window and gives up on a lock this process was holding itself.
///
/// Queueing here first leaves the lock file contended only by a real git
/// process, which is the case the retry exists for. A writer holds the gate
/// for one index read-modify-write and never nests.
pub(crate) fn gate() -> MutexGuard<'static, ()> {
    // Poisoning only means a writer panicked mid-write. The index file is
    // replaced by rename, so the next writer is safe to proceed.
    GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
