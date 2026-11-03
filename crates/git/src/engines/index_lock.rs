use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::sync::{Mutex, MutexGuard};

static GATE: Mutex<()> = Mutex::new(());

thread_local! {
    static HELD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Serializes every in-process writer of a git index.
///
/// `index.lock` is the only arbiter across processes, so a writer either takes
/// it or retries. Retrying alone loses: the status engine holds the lock
/// across a re-read and rewrite of the whole file, measured at up to 25 ms on
/// a loaded machine, so a writer polling every 5 ms keeps landing in another
/// job's window and gives up on a lock this process was holding itself.
///
/// Queueing here first leaves the lock file contended only by a real git
/// process, which is the case the retry exists for. The gate is reentrant on
/// the owning thread, so a mutation that spans several libgit2 index writes
/// takes it once at the top and the `retry_locked` calls inside re-enter
/// instead of deadlocking. Another thread still waits, which is what stops a
/// status refresh from interleaving with a half-applied mutation.
pub(crate) struct IndexGate(PhantomData<*const ()>);

pub(crate) fn gate() -> IndexGate {
    DEPTH.with(|depth| {
        if depth.get() == 0 {
            // Poisoning only means a writer panicked mid-write. The index file
            // is replaced by rename, so the next writer is safe to proceed.
            let guard = GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            HELD.with(|held| *held.borrow_mut() = Some(guard));
        }
        depth.set(depth.get() + 1);
    });
    IndexGate(PhantomData)
}

impl Drop for IndexGate {
    fn drop(&mut self) {
        DEPTH.with(|depth| {
            let remaining = depth.get() - 1;
            depth.set(remaining);
            if remaining == 0 {
                HELD.with(|held| drop(held.borrow_mut().take()));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_reenters_on_the_owning_thread() {
        let _outer = gate();
        let _inner = gate();
    }

    #[test]
    fn gate_is_exclusive_across_threads() {
        let outer = gate();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _gate = gate();
            tx.send(()).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "a second thread must wait for the owning thread"
        );
        drop(outer);
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
    }
}
