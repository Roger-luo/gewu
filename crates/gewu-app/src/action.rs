use gewu_review::{Decision, FeedbackKind, FeedbackTarget};
use serde::{Deserialize, Serialize};

/// A reviewer action. Tagged by `type` in JSON.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Visit {
        chunk_id: String,
    },
    SetReviewed {
        chunk_id: String,
        reviewed: bool,
    },
    AddFeedback {
        chunk_id: String,
        kind: FeedbackKind,
        target: FeedbackTarget,
        body: String,
        anchor: Option<AnchorRef>,
    },
    ResolveFeedback {
        feedback_id: String,
        resolved: bool,
    },
    DeleteFeedback {
        feedback_id: String,
    },
    RetargetFeedback {
        feedback_id: String,
        target: FeedbackTarget,
    },
    SetDecision {
        decision: Decision,
    },
}

/// New-side line span the reviewer picked. Path, side, and patch hash are
/// filled from the review input when the action is applied.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnchorRef {
    pub change_id: String,
    pub start_line: u32,
    pub end_line: u32,
}
