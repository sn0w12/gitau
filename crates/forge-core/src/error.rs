use crate::id::ConnectionKey;

/// Failure of a provider call, transport, or authentication step. Carries no
/// workspace types so provider crates stay independent; the backend maps it
/// into its own error at the IPC boundary.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("network failure: {message}")]
    Network { message: String },
    #[error("authentication required for {provider} at {host}")]
    Unauthorized { provider: String, host: String },
    #[error("{provider} forbade the request: {message}")]
    Forbidden { provider: String, message: String },
    #[error("{provider} has no such resource: {message}")]
    NotFound { provider: String, message: String },
    #[error("{provider} request failed ({status}): {message}")]
    Api {
        provider: String,
        status: u16,
        message: String,
    },
    #[error("{provider} response could not be read: {message}")]
    MalformedResponse { provider: String, message: String },
    #[error("the provider does not support this operation")]
    Unsupported,
    #[error("operation cancelled")]
    Cancelled,
    #[error("token storage failure: {message}")]
    Keyring { message: String },
    #[error("no provider registered as {provider}")]
    UnknownProvider { provider: String },
    #[error("internal error: {message}")]
    Internal { message: String },
    #[error("io error: {message}")]
    Io { message: String },
}

impl ProviderError {
    /// `true` when retrying the same call may succeed.
    pub fn retryable(&self) -> bool {
        match self {
            ProviderError::Network { .. } => true,
            ProviderError::Api { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }

    pub fn unauthorized(key: &ConnectionKey) -> Self {
        ProviderError::Unauthorized {
            provider: key.provider.clone(),
            host: key.host.clone(),
        }
    }
}

impl From<std::io::Error> for ProviderError {
    fn from(error: std::io::Error) -> Self {
        ProviderError::Io {
            message: error.to_string(),
        }
    }
}
