mod model;
mod progress;
mod store;
mod validation;

pub use model::{
    Expansion, HarnessStage, HarnessState, InvestigationCheckpoint, InvestigationFinding,
    ReviewOrigin, Revision, WorkItem, WorkList,
};
pub use progress::{Verdict, next_step};
pub use store::{HarnessStore, HarnessStoreError};
pub use validation::{HarnessValidationError, HarnessValidationErrors};

pub const HARNESS_SCHEMA_VERSION: u32 = 1;
