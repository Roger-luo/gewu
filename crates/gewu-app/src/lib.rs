//! Interface-independent application service for Gewu.
//!
//! A [`ReviewWorkspace`] loads a review the same way the terminal UI does,
//! exposes a [`Snapshot`] for rendering, and accepts [`Action`]s that append
//! to the shared event log.

mod action;
mod error;
mod quote;
mod snapshot;
mod workspace;

pub use action::{Action, AnchorRef};
pub use error::AppError;
pub use snapshot::{
    BlockView, ChunkStatus, ChunkView, GenerationView, QuoteLine, QuoteLineKind, QuoteView,
    Snapshot, SourceView, SummaryView,
};
pub use workspace::ReviewWorkspace;
