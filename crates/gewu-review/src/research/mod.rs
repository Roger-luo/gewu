//! Incremental investigation with source-bound evidence and resumable records.
//! Structural acceptance proves attribution and byte coverage, not prose quality.

mod export;
mod model;
mod run;
mod synthesis;
mod task;

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use model::{Evidence, Finding, ResearchLedger, ResearchNotes, ResearchPolicy, ResearchTask};
pub use run::ResearchRun;
pub use synthesis::{FindingCursor, FindingPage, GroundedOverview, SupportedText};

#[derive(Debug, Error)]
pub enum ResearchError {
    #[error("research artifact belongs to a different source, manifest, or protocol")]
    Stale,
    #[error("research limits must be positive")]
    InvalidPolicy,
    #[error("research {kind} budget exhausted: {actual} exceeds {limit}")]
    Budget {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("notes do not answer the pending research request")]
    WrongRequest,
    #[error("each finding needs a nonempty summary and exact evidence from the supplied page")]
    InvalidFinding,
    #[error("overview claims must cite existing findings and contain nonempty text")]
    InvalidOverview,
    #[error("research is incomplete; not every source page has accepted findings")]
    Incomplete,
    #[error("research already accounted for the complete input")]
    Complete,
    #[error(transparent)]
    Context(#[from] crate::context::ContextError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn digest(value: &impl Serialize) -> Result<String, ResearchError> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}

fn budget(kind: &'static str, actual: usize, limit: usize) -> Result<(), ResearchError> {
    if actual > limit {
        return Err(ResearchError::Budget {
            kind,
            actual,
            limit,
        });
    }
    Ok(())
}
