#[derive(Debug, thiserror::Error)]
pub enum IconError {
    #[error("icon service not initialized")]
    NotInitialized,
    #[error("invalid remote url `{0}`")]
    InvalidRemote(String),
    #[error("network error fetching `{url}`: {message}")]
    Network { url: String, message: String },
    #[error("response for `{url}` is not a supported image")]
    NotAnImage { url: String },
    #[error("response for `{url}` exceeds the size limit")]
    TooLarge { url: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl IconError {
    pub fn code(&self) -> &'static str {
        match self {
            IconError::NotInitialized => "iconsNotInitialized",
            IconError::InvalidRemote(_) => "invalidRemoteUrl",
            IconError::Network { .. } => "iconNetwork",
            IconError::NotAnImage { .. } => "iconNotAnImage",
            IconError::TooLarge { .. } => "iconTooLarge",
            IconError::Io(_) => "io",
        }
    }
}
