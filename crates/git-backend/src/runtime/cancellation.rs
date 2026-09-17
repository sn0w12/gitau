use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{GitError, Result};

/// Cooperative cancellation flag shared between scheduler, workers and IPC.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Returns true if this call performed the cancellation.
    pub fn cancel(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(GitError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_reports_first_caller_only() {
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(!token.cancel());
        assert!(token.is_cancelled());
        assert_eq!(token.check().unwrap_err().code(), "cancelled");
    }

    #[test]
    fn clones_share_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
    }
}
