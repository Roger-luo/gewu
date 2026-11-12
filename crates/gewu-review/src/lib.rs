//! Domain primitives for human-guided code and design review.

pub mod audit;
mod budget;
pub mod context;
pub mod explainer;
mod fallback;
pub mod handover;
pub mod harness;
mod height;
mod input;
pub mod load;
mod loops;
mod opening;
mod publication;
mod reading;
pub mod research;
mod session;
mod side;
mod spec;
mod validation;

pub use budget::ChunkBudget;
pub use explainer::Explainer;
pub use handover::Handover;
pub use height::{
    CHUNK_HEIGHT_GUIDANCE, HeightBreakdown, chunk_height, explainer_height, pane_height,
    pane_text_height,
};
pub use input::{Change, ReviewInput, SourceIdentity};
pub use publication::{
    GEWU_REPOSITORY, GitHubRequest, InlineComment, IssueBundle, IssueDraft, Note, Plan,
    PullRequestDraft, Venue,
};
pub use reading::ReadingTime;
pub use session::{
    Decision, EventLog, Feedback, FeedbackKind, FeedbackTarget, LineAnchor, ReviewEvent,
    ReviewSession, SessionError,
};
pub use side::Side;
pub use spec::{
    ChangePresentation, ChunkBlock, ChunkContext, ChunkKind, GenerationStatus, Illustration,
    Overview, Quote, ReviewChunk, ReviewSpec,
};
pub use validation::{ValidatedReview, ValidationError, ValidationErrors};

pub const SCHEMA_VERSION: u32 = 4;

/// Rendered rows a chunk may occupy before it needs an explicit justification.
/// Sized so a chunk fits one modest terminal without paging, because the chunk
/// is the unit a reviewer comments on.
pub const DEFAULT_MAX_CHUNK_LINES: usize = 40;
