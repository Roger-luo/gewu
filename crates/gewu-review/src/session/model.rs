use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    Comment,
    RequestChanges,
}

impl Decision {
    /// How the decision reads on the summary screen.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Comment => "comment",
            Self::RequestChanges => "request changes",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Note,
    Issue,
}

impl FeedbackKind {
    /// How the kind reads on screen, where "note" and "issue" are Gewu's own
    /// words for what a reviewer calls a comment and a request.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Comment",
            Self::Issue => "Request change",
        }
    }
}

/// What a piece of feedback is about. A reviewer reads two things at once —
/// the code and the walkthrough explaining it — and an agent fixes them in
/// completely different places, so the reviewer says which they meant.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackTarget {
    /// The code under review. An agent edits the source.
    #[default]
    Code,
    /// The walkthrough Gewu generated. An agent rewrites the chunk.
    Explanation,
}

impl FeedbackTarget {
    /// How the target reads on screen and in a handover.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Explanation => "explanation",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LineAnchor {
    pub change_id: String,
    pub path: String,
    pub side: String,
    pub start_line: u32,
    pub end_line: u32,
    pub patch_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Feedback {
    pub id: String,
    pub chunk_id: String,
    pub kind: FeedbackKind,
    /// Sessions recorded before feedback could name a target read as `Code`,
    /// which is what a review comment usually means.
    #[serde(default)]
    pub target: FeedbackTarget,
    pub body: String,
    pub anchor: Option<LineAnchor>,
    pub resolved: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EventAction {
    SessionStarted {
        spec_id: String,
    },
    Visited {
        chunk_id: String,
    },
    ReviewChanged {
        chunk_id: String,
        reviewed: bool,
    },
    FeedbackAdded {
        feedback: Feedback,
    },
    FeedbackResolved {
        feedback_id: String,
        resolved: bool,
    },
    FeedbackDeleted {
        feedback_id: String,
    },
    FeedbackRetargeted {
        feedback_id: String,
        target: FeedbackTarget,
    },
    DecisionSet {
        decision: Decision,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewEvent {
    pub at_unix_ms: u128,
    #[serde(flatten)]
    action: EventAction,
}

impl ReviewEvent {
    #[must_use]
    pub fn started(spec_id: impl Into<String>) -> Self {
        Self::new(EventAction::SessionStarted {
            spec_id: spec_id.into(),
        })
    }

    #[must_use]
    pub fn visited(chunk_id: impl Into<String>) -> Self {
        Self::new(EventAction::Visited {
            chunk_id: chunk_id.into(),
        })
    }

    #[must_use]
    pub fn reviewed(chunk_id: impl Into<String>, reviewed: bool) -> Self {
        Self::new(EventAction::ReviewChanged {
            chunk_id: chunk_id.into(),
            reviewed,
        })
    }

    #[must_use]
    pub fn feedback(feedback: Feedback) -> Self {
        Self::new(EventAction::FeedbackAdded { feedback })
    }

    #[must_use]
    pub fn resolved(feedback_id: impl Into<String>, resolved: bool) -> Self {
        Self::new(EventAction::FeedbackResolved {
            feedback_id: feedback_id.into(),
            resolved,
        })
    }

    /// Retracts feedback a reviewer never meant to leave. The log keeps the
    /// record that it was written and withdrawn; the review no longer carries
    /// it, so nothing downstream reports it.
    #[must_use]
    pub fn deleted(feedback_id: impl Into<String>) -> Self {
        Self::new(EventAction::FeedbackDeleted {
            feedback_id: feedback_id.into(),
        })
    }

    /// Re-aims feedback at what the reviewer actually meant. A comment filed
    /// against the code but really about the walkthrough sends an agent to the
    /// wrong place, and only the reviewer can say which it was.
    #[must_use]
    pub fn retargeted(feedback_id: impl Into<String>, target: FeedbackTarget) -> Self {
        Self::new(EventAction::FeedbackRetargeted {
            feedback_id: feedback_id.into(),
            target,
        })
    }

    #[must_use]
    pub fn decision(decision: Decision) -> Self {
        Self::new(EventAction::DecisionSet { decision })
    }

    fn new(action: EventAction) -> Self {
        let at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        Self { at_unix_ms, action }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReviewSession {
    pub spec_id: Option<String>,
    pub visited: HashSet<String>,
    /// Most recently visited live chunk, used only to orient a resumed UI.
    pub last_visited: Option<String>,
    pub reviewed: HashSet<String>,
    pub feedback: HashMap<String, Feedback>,
    /// Feedback ids in the order they were written. A reviewer selects feedback
    /// by position, so the order they see must be the order they wrote it and
    /// must not shift between draws.
    order: Vec<String>,
    pub decision: Option<Decision>,
}

impl ReviewSession {
    pub fn apply(&mut self, event: &ReviewEvent) {
        match &event.action {
            EventAction::SessionStarted { spec_id } => {
                self.spec_id = Some(spec_id.clone());
            }
            EventAction::Visited { chunk_id } => {
                self.visited.insert(chunk_id.clone());
                self.last_visited = Some(chunk_id.clone());
            }
            EventAction::ReviewChanged { chunk_id, reviewed } => {
                if *reviewed {
                    self.reviewed.insert(chunk_id.clone());
                } else {
                    self.reviewed.remove(chunk_id);
                }
            }
            EventAction::FeedbackAdded { feedback } => {
                if self
                    .feedback
                    .insert(feedback.id.clone(), feedback.clone())
                    .is_none()
                {
                    self.order.push(feedback.id.clone());
                }
            }
            EventAction::FeedbackResolved {
                feedback_id,
                resolved,
            } => {
                if let Some(feedback) = self.feedback.get_mut(feedback_id) {
                    feedback.resolved = *resolved;
                }
            }
            EventAction::FeedbackDeleted { feedback_id } => {
                self.feedback.remove(feedback_id);
                self.order.retain(|id| id != feedback_id);
            }
            EventAction::FeedbackRetargeted {
                feedback_id,
                target,
            } => {
                if let Some(feedback) = self.feedback.get_mut(feedback_id) {
                    feedback.target = *target;
                }
            }
            EventAction::DecisionSet { decision } => self.decision = Some(*decision),
        }
    }

    /// Unresolved requests about chunks the review still holds.
    ///
    /// Re-planning can drop a chunk whose code changed, and a request attached
    /// to it becomes unreachable: no screen can resolve it, and no jump can
    /// reach it. Counting it would warn the reviewer about something they
    /// cannot act on, forever.
    #[must_use]
    pub fn live_unresolved_issues<'a>(
        &'a self,
        chunks: impl IntoIterator<Item = &'a String>,
    ) -> Vec<&'a Feedback> {
        let live: HashSet<_> = chunks.into_iter().collect();
        self.unresolved_issues()
            .filter(|feedback| live.contains(&feedback.chunk_id))
            .collect()
    }

    pub fn unresolved_issues(&self) -> impl Iterator<Item = &Feedback> {
        self.ordered()
            .filter(|item| item.kind == FeedbackKind::Issue && !item.resolved)
    }

    /// What the review amounts to, read from what the reviewer left rather than
    /// asked for on top of it.
    ///
    /// A reviewer who has filed a request is requesting changes; one who has
    /// remarked has commented; one who found nothing to say approves. Asking
    /// them to also pick from three buttons asks them to state a conclusion
    /// they have already written down, and invites the two to disagree.
    #[must_use]
    pub fn outcome<'a>(&'a self, chunks: impl IntoIterator<Item = &'a String>) -> Decision {
        let live: HashSet<_> = chunks.into_iter().collect();
        let standing: Vec<_> = self
            .ordered()
            .filter(|item| live.contains(&item.chunk_id))
            .collect();
        if standing
            .iter()
            .any(|item| item.kind == FeedbackKind::Issue && !item.resolved)
        {
            Decision::RequestChanges
        } else if standing.is_empty() {
            Decision::Approve
        } else {
            // Includes a request since settled: they raised it, which is more
            // than nothing to say.
            Decision::Comment
        }
    }

    /// Everything a reviewer left on one chunk, oldest first.
    #[must_use]
    pub fn chunk_feedback(&self, chunk_id: &str) -> Vec<&Feedback> {
        self.ordered()
            .filter(|item| item.chunk_id == chunk_id)
            .collect()
    }

    /// Everything the review still holds, oldest first. A handover reads in
    /// this order, so an agent meets the reviewer's points as they made them.
    pub fn ordered(&self) -> impl Iterator<Item = &Feedback> {
        self.order.iter().filter_map(|id| self.feedback.get(id))
    }
}

#[cfg(test)]
mod tests {
    use super::{Decision, Feedback, FeedbackKind, FeedbackTarget, ReviewEvent, ReviewSession};

    fn note(id: &str, chunk_id: &str) -> Feedback {
        Feedback {
            id: id.into(),
            chunk_id: chunk_id.into(),
            kind: FeedbackKind::Note,
            target: FeedbackTarget::Code,
            body: "Body.".into(),
            anchor: None,
            resolved: false,
        }
    }

    #[test]
    fn keeps_feedback_in_the_order_it_was_written() {
        let mut session = ReviewSession::default();
        for id in ["third", "first", "second"] {
            session.apply(&ReviewEvent::feedback(note(id, "chunk")));
        }

        let ids: Vec<_> = session
            .chunk_feedback("chunk")
            .iter()
            .map(|item| item.id.as_str())
            .collect();

        assert_eq!(ids, vec!["third", "first", "second"]);
    }

    #[test]
    fn lists_only_the_feedback_left_on_one_chunk() {
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(note("here", "chunk")));
        session.apply(&ReviewEvent::feedback(note("elsewhere", "other")));

        assert_eq!(session.chunk_feedback("chunk").len(), 1);
    }

    #[test]
    fn a_deleted_note_leaves_the_review_entirely() {
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(note("mistake", "chunk")));
        session.apply(&ReviewEvent::feedback(note("kept", "chunk")));

        session.apply(&ReviewEvent::deleted("mistake"));

        assert!(!session.feedback.contains_key("mistake"));
        let ids: Vec<_> = session
            .chunk_feedback("chunk")
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, vec!["kept"]);
    }

    #[test]
    fn the_outcome_follows_what_the_reviewer_left() {
        let chunks = ["chunk".to_owned()];
        let mut session = ReviewSession::default();
        assert_eq!(session.outcome(chunks.iter()), Decision::Approve);

        session.apply(&ReviewEvent::feedback(note("remark", "chunk")));
        assert_eq!(session.outcome(chunks.iter()), Decision::Comment);

        let mut issue = note("request", "chunk");
        issue.kind = FeedbackKind::Issue;
        session.apply(&ReviewEvent::feedback(issue));
        assert_eq!(session.outcome(chunks.iter()), Decision::RequestChanges);

        // Settled, so it no longer asks for anything — but it was still said.
        session.apply(&ReviewEvent::resolved("request", true));
        assert_eq!(session.outcome(chunks.iter()), Decision::Comment);
    }

    /// Re-planning drops chunks, and feedback left on one cannot still be
    /// holding the review open.
    #[test]
    fn feedback_on_a_dropped_chunk_does_not_decide_the_review() {
        let mut session = ReviewSession::default();
        let mut issue = note("request", "dropped");
        issue.kind = FeedbackKind::Issue;
        session.apply(&ReviewEvent::feedback(issue));

        let live = ["kept".to_owned()];

        assert_eq!(session.outcome(live.iter()), Decision::Approve);
    }

    #[test]
    fn retargeting_moves_feedback_to_what_the_reviewer_meant() {
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(note("misfiled", "chunk")));
        assert_eq!(session.feedback["misfiled"].target, FeedbackTarget::Code);

        session.apply(&ReviewEvent::retargeted(
            "misfiled",
            FeedbackTarget::Explanation,
        ));

        assert_eq!(
            session.feedback["misfiled"].target,
            FeedbackTarget::Explanation
        );
    }

    #[test]
    fn a_deleted_request_no_longer_counts_as_unresolved() {
        let mut session = ReviewSession::default();
        let mut issue = note("issue", "chunk");
        issue.kind = FeedbackKind::Issue;
        session.apply(&ReviewEvent::feedback(issue));
        assert_eq!(session.unresolved_issues().count(), 1);

        session.apply(&ReviewEvent::deleted("issue"));

        assert_eq!(session.unresolved_issues().count(), 0);
    }
}
