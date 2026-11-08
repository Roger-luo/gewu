use std::{io, path::PathBuf};

use gewu_review::{SessionError, ValidationErrors};
use thiserror::Error;

/// Failures from opening a workspace or dispatching an action.
///
/// Rejected actions leave session state unchanged.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing review artifact `{0}`")]
    MissingArtifact(PathBuf),
    #[error("could not read review artifacts: {0}")]
    Io(#[from] io::Error),
    #[error("invalid review JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("review validation failed:\n- {0}")]
    Validation(String),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("session belongs to review `{existing}`, not `{expected}`")]
    SessionMismatch { existing: String, expected: String },
    #[error("unknown chunk `{0}`")]
    UnknownChunk(String),
    #[error("unknown feedback `{0}`")]
    UnknownFeedback(String),
    #[error("unknown change `{0}`")]
    UnknownChange(String),
    #[error("feedback body must not be empty")]
    EmptyBody,
    #[error("anchor is outside the quoted change")]
    AnchorOutsideQuote,
}

impl AppError {
    pub(crate) fn validation(errors: ValidationErrors) -> Self {
        let details = errors
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n- ");
        Self::Validation(details)
    }
}
