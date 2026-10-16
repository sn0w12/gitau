//! Provider-neutral forge abstraction.
//!
//! A forge is a hosting service with issues, pull requests, CI, notifications,
//! and repository creation: GitHub, GitLab, Gitea, and the like. This crate
//! owns the shared DTOs, the [`ForgeProvider`] trait every provider
//! implements, the [`ForgeRegistry`] that resolves a remote URL or a provider
//! id to a provider, and [`ForgeAccounts`], which owns sign-in state and
//! credentials across connections.
//!
//! Provider crates depend on this crate, plus `ansi` when they parse
//! terminal-styled logs, and nothing else in the workspace.
//!
//! # Adding a provider
//!
//! 1. Implement [`ForgeProvider`] for one type. Every data method defaults to
//!    [`ProviderError::Unsupported`], so implement only what the service
//!    serves and set the matching bits in [`Capabilities`], which is what the
//!    UI reads to hide the views and actions it cannot offer.
//! 2. Declare the hosts it serves and implement `parse_remote` so a local
//!    remote resolves to `(host, owner, repo)`, plus `web_url` for the page
//!    kinds it can address.
//! 3. Add the crate to the registry in the backend. Connections, tokens, and
//!    the profile cache are keyed by `(provider, host)` and need nothing from
//!    the provider beyond `hosts`, the `legacy_*` hooks for an upgrade from a
//!    host-less key, and `client_id`/`oauth_scopes` if it signs in by device
//!    flow.
//!
//! No UI change is needed for a provider the existing views already cover; a
//! new capability is a new [`Capabilities`] field plus the view that reads it.

pub mod accounts;
pub mod capabilities;
pub mod dto;
pub mod error;
pub mod id;
pub mod provider;
pub mod registry;
pub mod token;

pub use accounts::{AuthChallenge, AuthPoll, ForgeAccounts};
pub use capabilities::Capabilities;
pub use dto::*;
pub use error::ProviderError;
pub use id::{ConnectionKey, ConnectionProfile, ProviderDescriptor, RemoteCoords, RemoteKind};
pub use provider::{CredentialSecret, ForgeFuture, ForgeProvider};
pub use registry::ForgeRegistry;
pub use token::{KeyringTokenStore, MemoryTokenStore, TokenStore};
