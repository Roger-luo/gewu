use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkbenchError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Git(#[from] gewu_git::GitError),
    #[error(transparent)]
    Harness(#[from] gewu_harness::Error),
    #[error(transparent)]
    App(#[from] gewu_app::AppError),
    #[error(transparent)]
    Agent(#[from] gewu_agent::AgentError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl WorkbenchError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    /// Whether this is the caller's mistake rather than a failure, for
    /// transports that distinguish the two.
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        matches!(self, Self::NotFound(_) | Self::Invalid(_))
    }
}
