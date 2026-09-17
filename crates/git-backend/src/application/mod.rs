pub mod backend;

pub use backend::{
    Backend, BackendConfig, CreateRepositoryResult, OpenedRepository, REMOTE_INFO_PRUNE_AFTER,
    RemoveRepositoryResult,
};
