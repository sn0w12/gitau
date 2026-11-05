pub mod backend;
pub mod icon_source;

pub use backend::{
    Backend, BackendConfig, CreateRepositoryResult, HISTORY_STATS_PRUNE_AFTER, OpenedRepository,
    REMOTE_INFO_PRUNE_AFTER, RemoveRepositoryResult,
};
pub use icon_source::ForgeIconSource;
