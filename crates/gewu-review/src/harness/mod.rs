mod model;
mod progress;
mod proofread;
mod proofreading;
mod store;
mod tests_section;
mod validation;
mod verdicts;

pub use model::{
    Expansion, HarnessStage, HarnessState, InvestigationCheckpoint, InvestigationFinding,
    ReviewOrigin, Revision, WorkItem, WorkList,
};
pub use progress::{Verdict, next_step};
pub use proofread::{ChunkProofread, Proofread};
pub use proofreading::Proofreading;
pub use store::{HarnessStore, HarnessStoreError};
pub use tests_section::test_expansion;
pub use validation::{HarnessValidationError, HarnessValidationErrors};

pub const HARNESS_SCHEMA_VERSION: u32 = 1;

/// Beside `review-input.json`: the author's description of the change, such
/// as a pull request's title and body. Claim explainers quote it.
pub const DESCRIPTION_FILE: &str = "description.md";
