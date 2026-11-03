use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Status,
    Diff,
    History,
    Fetch,
    Push,
    Pull,
    Clone,
    Mutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub id: u64,
    pub kind: OperationKind,
    pub cancelled: bool,
    pub complete: bool,
}
