use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::domain::OperationId;
use crate::error::{GitError, Result};
use crate::runtime::cancellation::CancellationToken;

/// Lifecycle record for an in-flight clone: unlike diff/graph operations a
/// clone holds no rows to read back, so the operation only owns its
/// cancellation token and completion flag.
pub struct CloneOperation {
    pub operation_id: OperationId,
    token: CancellationToken,
    complete: AtomicBool,
}

impl CloneOperation {
    pub fn new(operation_id: OperationId, token: CancellationToken) -> Self {
        Self {
            operation_id,
            token,
            complete: AtomicBool::new(false),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn cancel(&self) -> bool {
        !self.complete.load(Ordering::Acquire) && self.token.cancel()
    }

    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Release);
    }
}

/// Registry of live clone operations; shape mirrors the graph registry.
#[derive(Default)]
pub struct CloneOperationRegistry {
    ops: Mutex<HashMap<OperationId, Arc<CloneOperation>>>,
}

impl CloneOperationRegistry {
    pub fn insert(&self, op: Arc<CloneOperation>) {
        self.ops.lock().unwrap().insert(op.operation_id, op);
    }

    pub fn get(&self, id: OperationId) -> Result<Arc<CloneOperation>> {
        self.ops
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(GitError::ObjectNotFound {
                oid: format!("operation {id}"),
            })
    }

    /// Drops completed operations beyond the newest `keep` finished ones.
    pub fn prune_completed(&self, keep: usize) {
        let mut guard = self.ops.lock().unwrap();
        let mut completed: Vec<OperationId> = guard
            .values()
            .filter(|op| op.complete.load(Ordering::Acquire))
            .map(|op| op.operation_id)
            .collect();
        completed.sort_unstable();
        while completed.len() > keep {
            let oldest = completed.remove(0);
            guard.remove(&oldest);
        }
    }
}
