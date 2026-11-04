//! Remote owner/org icon resolution with a per-owner disk cache and
//! periodic background refresh.
//!
//! Given a git remote URL (`https://github.com/{owner}/{repo}.git`,
//! `git@github.com:{owner}/{repo}.git`, ...), the service derives a stable
//! cache key from `{host}/{owner}`, asks the forge layer for the provider's
//! icon URL, downloads it once, stores the bytes beside small JSON
//! metadata, and revalidates entries older than [`IconConfig::entry_ttl`],
//! on demand (stale-while-revalidate: stale bytes are still served while
//! the refresh runs) and periodically via
//! [`IconService::run_periodic_refresh`]. Hosts with no provider, or
//! providers that serve no icon, resolve to no icon.

mod cache;
mod config;
mod error;
mod fetcher;
mod image;
mod plan;
mod service;
mod source;

pub use cache::CachedIcon;
pub use config::IconConfig;
pub use error::IconError;
pub use fetcher::{FetchFuture, HttpFetcher, IconFetcher};
pub use image::{encode_data_url, sniff_image};
pub use plan::{plan_from_remote, IconPlan};
pub use service::IconService;
pub use source::OwnerIconSource;
