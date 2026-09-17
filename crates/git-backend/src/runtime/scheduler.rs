use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::error::{GitError, Result};
use crate::runtime::cancellation::CancellationToken;

/// Bounds how many blocking operations run concurrently.
#[derive(Clone)]
pub struct Scheduler {
    permits: Arc<Semaphore>,
}

impl Scheduler {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    /// Runs a blocking job on the shared pool. If cancellation fires before or
    /// during the job, the result collapses into [`GitError::Cancelled`] even
    /// when the underlying library returned something else.
    pub async fn run<T, F>(&self, token: &CancellationToken, job: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        token.check()?;
        let permit = self.permits.clone().acquire_owned().await?;
        let token_for_job = token.clone();
        let join = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = job();
            crate::engines::git2::local::clear_all_cached_handles();
            result
        });
        match join.await? {
            Ok(value) => Ok(value),
            Err(err) => {
                if token_for_job.is_cancelled() {
                    Err(GitError::Cancelled)
                } else {
                    Err(err)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_and_propagates_values() {
        let scheduler = Scheduler::new(2);
        let result = scheduler
            .run(&CancellationToken::new(), || Ok(41 + 1))
            .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn cancelled_token_short_circuits() {
        let scheduler = Scheduler::new(1);
        let token = CancellationToken::new();
        token.cancel();
        let result: Result<()> = scheduler.run(&token, || panic!("must not run")).await;
        assert_eq!(result.unwrap_err().code(), "cancelled");
    }

    #[tokio::test]
    async fn maps_errors_to_cancelled_when_token_fires_midway() {
        let scheduler = Scheduler::new(1);
        let token = CancellationToken::new();
        let token_clone = token.clone();
        let result: Result<()> = scheduler
            .run(&token, move || {
                token_clone.cancel();
                Err(GitError::invalid_input("superseded"))
            })
            .await;
        assert_eq!(result.unwrap_err().code(), "cancelled");
    }

    #[tokio::test]
    async fn limits_concurrency() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let scheduler = Scheduler::new(1);
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let tokens: Vec<_> = (0..4).map(|_| CancellationToken::new()).collect();
        let mut jobs = Vec::new();
        for token in &tokens {
            let active = active.clone();
            let max_seen = max_seen.clone();
            jobs.push(scheduler.run(token, move || {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(20));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }));
        }
        for job in jobs {
            job.await.unwrap();
        }
        assert!(max_seen.load(Ordering::SeqCst) <= 1);
    }
}
