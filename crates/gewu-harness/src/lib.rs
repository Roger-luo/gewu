//! Progressive review generation harness for Gewu.

pub mod artifacts;
pub mod estimate;
mod prompts;
pub mod research_state;
mod runner;

use thiserror::Error;

pub use runner::HarnessRunner;

/// Chunks one expansion may put in front of a reviewer at once.
pub const DEFAULT_MAX_CHUNKS: usize = 6;

/// Failures while driving progressive review generation.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] gewu_review::harness::HarnessStoreError),
    #[error(transparent)]
    Session(#[from] gewu_review::SessionError),
    #[error(transparent)]
    Research(#[from] gewu_review::research::ResearchError),
    #[error(transparent)]
    Git(#[from] gewu_git::GitError),
}

impl Error {
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
